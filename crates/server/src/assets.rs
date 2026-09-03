// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::Extension,
    http::{Response, StatusCode, Uri, header},
    routing::get,
};
use dimidiumlabs_ui::{
    APPLE_TOUCH_ICON_PATH, ASSET_PREFIX, AssetLookup, AssetsCatalog, FAVICON_ICO_PATH, ROBOTS_PATH,
};

use crate::service::AssetsLayer;

/// Builds the Axum serving adapter for one composed, transport-agnostic asset catalog.
pub fn assets_router<S>(catalog: Arc<AssetsCatalog>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route(FAVICON_ICO_PATH, get(serve_asset))
        .route(ROBOTS_PATH, get(serve_asset))
        .route(APPLE_TOUCH_ICON_PATH, get(serve_asset))
        .route("/-/assets/{*path}", get(serve_asset))
        .layer(AssetsLayer::new(Arc::clone(&catalog)))
        .layer(Extension(catalog))
}

pub(crate) fn lookup_uri(catalog: &AssetsCatalog, path: &str) -> Option<AssetLookup> {
    let name = match path {
        FAVICON_ICO_PATH => "favicon.ico",
        ROBOTS_PATH => "robots.txt",
        APPLE_TOUCH_ICON_PATH => "apple-touch-icon.png",
        _ => path.strip_prefix(ASSET_PREFIX)?.strip_prefix('/')?,
    };
    let asset = catalog.lookup(name)?;
    if asset.asset().name() != asset.asset().fingerprinted_name() && !asset.is_fingerprinted() {
        return None;
    }
    Some(asset)
}

async fn serve_asset(
    Extension(catalog): Extension<Arc<AssetsCatalog>>,
    uri: Uri,
) -> Response<Body> {
    let Some(asset) = lookup_uri(&catalog, uri.path()).map(AssetLookup::asset) else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .expect("empty not-found response is valid");
    };

    let bytes = asset.bytes();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, asset.content_type())
        .header(header::CONTENT_LENGTH, bytes.len())
        .body(Body::from(bytes))
        .expect("registered UI asset response is valid")
}

#[cfg(test)]
mod tests {
    use dimidiumlabs_ui::{Asset, AssetKind};

    use super::*;

    const INTEGRITY: &str =
        "sha384-us70yumzLF2TpSodT8MxNxbLn5LCPNNUhaDDHTqXZZcBW+y9KnFu0zoe9CWl0mvS";
    const ASSETS: &[Asset] = &[
        Asset::new(
            AssetKind::Stylesheet,
            "app.css",
            "app.0123456789abcdef.css",
            dimidiumlabs_ui::CachePolicy::Immutable,
            b"body{}",
            INTEGRITY,
        ),
        Asset::new(
            AssetKind::Static {
                content_type: "image/x-icon",
            },
            "favicon.ico",
            "favicon.ico",
            dimidiumlabs_ui::CachePolicy::Revalidate,
            b"icon",
            INTEGRITY,
        ),
    ];

    #[test]
    fn maps_original_fingerprinted_and_conventional_root_names() {
        let catalog = AssetsCatalog::new().with(ASSETS).unwrap();
        assert!(lookup_uri(&catalog, "/-/assets/app.css").is_none());
        let fingerprinted = lookup_uri(&catalog, "/-/assets/app.0123456789abcdef.css").unwrap();
        assert!(fingerprinted.is_fingerprinted());
        assert_eq!(
            lookup_uri(&catalog, FAVICON_ICO_PATH)
                .unwrap()
                .asset()
                .name(),
            "favicon.ico"
        );
        assert!(lookup_uri(&catalog, "/outside/app.css").is_none());
    }
}
