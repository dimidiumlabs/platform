// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: Apache-2.0

mod assets;
mod middleware;

pub use assets::{AssetCatalog, AssetCatalogError};
pub use middleware::{DEFAULT_CSP, UiLayer, harden_response};

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode, header},
    };
    use dimidiumlabs_ui::{APP_STYLESHEET_PATH, Asset, CachePolicy};
    use tower::ServiceExt;

    use crate::{AssetCatalog, DEFAULT_CSP, UiLayer};

    fn router() -> Router {
        AssetCatalog::new([Asset::embedded(
            APP_STYLESHEET_PATH,
            "text/css; charset=utf-8",
            b"body{color:black}",
            CachePolicy::Revalidate,
        )])
        .unwrap()
        .router()
        .layer(UiLayer::default())
    }

    #[tokio::test]
    async fn assets_keep_get_head_etag_cache_and_security_contract() {
        let router = router();
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(APP_STYLESHEET_PATH)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
        assert_eq!(
            response.headers()[header::CONTENT_SECURITY_POLICY],
            DEFAULT_CSP
        );
        assert_eq!(
            response.headers()[header::X_CONTENT_TYPE_OPTIONS],
            "nosniff"
        );
        let etag = response.headers()[header::ETAG].clone();
        let length = response.headers()[header::CONTENT_LENGTH].clone();

        let head = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::HEAD)
                    .uri(APP_STYLESHEET_PATH)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(head.headers()[header::ETAG], etag);
        assert_eq!(head.headers()[header::CONTENT_LENGTH], length);
        assert!(
            to_bytes(head.into_body(), usize::MAX)
                .await
                .unwrap()
                .is_empty()
        );

        let cached = router
            .oneshot(
                Request::builder()
                    .uri(APP_STYLESHEET_PATH)
                    .header(header::IF_NONE_MATCH, &etag)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cached.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(cached.headers()[header::ETAG], etag);
        assert_eq!(
            cached.headers()[header::CONTENT_SECURITY_POLICY],
            DEFAULT_CSP
        );
    }

    #[tokio::test]
    async fn versioned_fonts_are_immutable() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/-/assets/fonts/ibm-plex-sans-variable-0.2.0-roman.woff2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
    }
}
