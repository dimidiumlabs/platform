// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fmt, sync::Arc};

use axum::{
    Router,
    body::Body,
    extract::Extension,
    http::{Response, StatusCode, Uri, header},
    routing::get,
};
use dimidiumlabs_ui::{
    APPLE_TOUCH_ICON_PATH, ASSET_PREFIX, Asset, CachePolicy, FAVICON_ICO_PATH, ROBOTS_PATH,
    foundation_assets,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetCatalogError {
    InvalidPath(&'static str),
    DuplicatePath(&'static str),
}

impl fmt::Display for AssetCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(path) => write!(formatter, "invalid UI asset path {path:?}"),
            Self::DuplicatePath(path) => write!(formatter, "duplicate UI asset path {path:?}"),
        }
    }
}

impl std::error::Error for AssetCatalogError {}

#[derive(Debug)]
pub struct AssetCatalog {
    assets: BTreeMap<&'static str, Asset>,
}

impl AssetCatalog {
    /// Builds a catalog containing the shared foundation plus application assets.
    ///
    /// # Errors
    /// Returns an error for duplicate paths or paths outside the canonical asset namespace and
    /// explicitly supported root resources.
    pub fn new(
        application_assets: impl IntoIterator<Item = Asset>,
    ) -> Result<Self, AssetCatalogError> {
        let mut assets = BTreeMap::new();
        for asset in foundation_assets().into_iter().chain(application_assets) {
            let path = asset.path();
            if !valid_path(path) {
                return Err(AssetCatalogError::InvalidPath(path));
            }
            if assets.insert(path, asset).is_some() {
                return Err(AssetCatalogError::DuplicatePath(path));
            }
        }
        Ok(Self { assets })
    }

    #[must_use]
    pub fn get(&self, path: &str) -> Option<&Asset> {
        self.assets.get(path)
    }

    pub fn router<S>(self) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        Router::new()
            .route(FAVICON_ICO_PATH, get(serve_asset))
            .route(ROBOTS_PATH, get(serve_asset))
            .route(APPLE_TOUCH_ICON_PATH, get(serve_asset))
            .route("/-/assets/{*path}", get(serve_asset))
            .layer(Extension(Arc::new(self)))
    }
}

fn valid_path(path: &str) -> bool {
    path.strip_prefix(ASSET_PREFIX)
        .is_some_and(|suffix| suffix.starts_with('/') && suffix.len() > 1)
        || matches!(path, FAVICON_ICO_PATH | ROBOTS_PATH | APPLE_TOUCH_ICON_PATH)
}

async fn serve_asset(Extension(catalog): Extension<Arc<AssetCatalog>>, uri: Uri) -> Response<Body> {
    let Some(asset) = catalog.get(uri.path()) else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .expect("empty not-found response is valid");
    };

    let cache_control = match asset.cache_policy() {
        CachePolicy::Revalidate => "no-cache",
        CachePolicy::Immutable => "public, max-age=31536000, immutable",
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, asset.content_type())
        .header(header::CACHE_CONTROL, cache_control)
        .body(Body::from(asset.bytes().to_vec()))
        .expect("registered UI asset response is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: Asset = Asset::embedded(
        "/-/assets/app.css",
        "text/css",
        b"body{}",
        CachePolicy::Revalidate,
    );

    #[test]
    fn rejects_invalid_and_duplicate_paths() {
        assert!(matches!(
            AssetCatalog::new([Asset::embedded(
                "/elsewhere.css",
                "text/css",
                b"",
                CachePolicy::Revalidate,
            )]),
            Err(AssetCatalogError::InvalidPath("/elsewhere.css"))
        ));
        assert!(matches!(
            AssetCatalog::new([APP.clone(), APP]),
            Err(AssetCatalogError::DuplicatePath("/-/assets/app.css"))
        ));
    }
}
