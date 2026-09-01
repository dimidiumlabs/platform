// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: Apache-2.0

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    body::Body,
    http::{HeaderValue, Method, Request, Response, StatusCode, header},
};
use tower::{Layer, Service};

pub const DEFAULT_CSP: &str = "default-src 'self'; base-uri 'none'; img-src 'self'; font-src 'self'; style-src 'self'; script-src 'self'; object-src 'none'; frame-ancestors 'none'";

#[derive(Debug, Clone)]
pub struct UiLayer {
    content_security_policy: HeaderValue,
}

impl UiLayer {
    #[must_use]
    pub fn new(content_security_policy: HeaderValue) -> Self {
        Self {
            content_security_policy,
        }
    }
}

impl Default for UiLayer {
    fn default() -> Self {
        Self::new(HeaderValue::from_static(DEFAULT_CSP))
    }
}

impl<S> Layer<S> for UiLayer {
    type Service = UiService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        UiService {
            inner,
            content_security_policy: self.content_security_policy.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UiService<S> {
    inner: S,
    content_security_policy: HeaderValue,
}

impl<S> Service<Request<Body>> for UiService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let method = request.method().clone();
        let if_none_match = request.headers().get(header::IF_NONE_MATCH).cloned();
        let content_security_policy = self.content_security_policy.clone();
        let replacement = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, replacement);

        Box::pin(async move {
            let response = inner.call(request).await?;
            Ok(harden_response(method, if_none_match, response, &content_security_policy).await)
        })
    }
}

/// Applies the shared UI cache and security policy to a completed response.
///
/// This buffers successful response bodies to calculate their entity tag. Callers must only use it
/// for bounded UI responses and static assets, never for streaming or download endpoints.
///
/// # Panics
/// Panics only if an internally generated ASCII CRC32 entity tag or decimal content length cannot
/// be represented as an HTTP header value.
pub async fn harden_response(
    method: Method,
    if_none_match: Option<HeaderValue>,
    response: Response<Body>,
    content_security_policy: &HeaderValue,
) -> Response<Body> {
    let status = response.status();
    let (mut parts, body) = response.into_parts();

    parts.headers.insert(
        header::CONTENT_SECURITY_POLICY,
        content_security_policy.clone(),
    );
    parts.headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );

    if !status.is_success() {
        return Response::from_parts(parts, body);
    }

    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("failed to buffer UI response: {error}");
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_SECURITY_POLICY, content_security_policy)
                .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
                .body(Body::empty())
                .expect("empty error response is valid");
        }
    };

    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&bytes);

    let etag = format!("\"{:08x}\"", hasher.finalize());
    let etag_header = HeaderValue::from_str(&etag).expect("CRC32 entity tag is valid");
    let cache_control = parts
        .headers
        .get(header::CACHE_CONTROL)
        .cloned()
        .unwrap_or(HeaderValue::from_static("no-cache"));

    if matches!(method, Method::GET | Method::HEAD)
        && if_none_match
            .as_ref()
            .is_some_and(|value| value.as_bytes() == etag.as_bytes())
    {
        return Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(header::ETAG, etag_header)
            .header(header::CACHE_CONTROL, cache_control)
            .header(header::CONTENT_SECURITY_POLICY, content_security_policy)
            .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
            .body(Body::empty())
            .expect("empty not-modified response is valid");
    }

    parts.headers.insert(header::ETAG, etag_header);
    parts
        .headers
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-cache"));
    parts.headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string()).expect("response length is valid"),
    );

    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(bytes)
    };
    Response::from_parts(parts, body)
}
