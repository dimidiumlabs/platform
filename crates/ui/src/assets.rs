// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: Apache-2.0

use std::borrow::Cow;

pub const ASSET_PREFIX: &str = "/-/assets";

pub const ROBOTS_PATH: &str = "/robots.txt";
pub const FAVICON_ICO_PATH: &str = "/favicon.ico";
pub const FAVICON_SVG_PATH: &str = "/-/assets/favicon.svg";
pub const APPLE_TOUCH_ICON_PATH: &str = "/apple-touch-icon.png";

pub const MANIFEST_PATH: &str = "/-/assets/manifest.webmanifest";

pub const APP_SCRIPT_PATH: &str = "/-/assets/app.js";
pub const APP_STYLESHEET_PATH: &str = "/-/assets/app.css";
pub const GLOBAL_STYLESHEET_PATH: &str = "/-/assets/global.css";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachePolicy {
    Revalidate,
    Immutable,
}

#[derive(Debug, Clone)]
pub struct Asset {
    path: &'static str,
    content_type: &'static str,
    bytes: Cow<'static, [u8]>,
    cache_policy: CachePolicy,
}

impl Asset {
    #[must_use]
    pub const fn embedded(
        path: &'static str,
        content_type: &'static str,
        bytes: &'static [u8],
        cache_policy: CachePolicy,
    ) -> Self {
        Self {
            path,
            content_type,
            bytes: Cow::Borrowed(bytes),
            cache_policy,
        }
    }

    #[must_use]
    pub fn owned(
        path: &'static str,
        content_type: &'static str,
        bytes: Vec<u8>,
        cache_policy: CachePolicy,
    ) -> Self {
        Self {
            path,
            content_type,
            bytes: Cow::Owned(bytes),
            cache_policy,
        }
    }

    #[must_use]
    pub const fn path(&self) -> &'static str {
        self.path
    }

    #[must_use]
    pub const fn content_type(&self) -> &'static str {
        self.content_type
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub const fn cache_policy(&self) -> CachePolicy {
        self.cache_policy
    }
}

pub const FONT_PREFIX: &str = "/-/assets/fonts";

/// Registers a revalidated CSS asset with the canonical content type.
#[must_use]
pub const fn css(path: &'static str, bytes: &'static [u8]) -> Asset {
    Asset::embedded(
        path,
        "text/css; charset=utf-8",
        bytes,
        CachePolicy::Revalidate,
    )
}

/// Registers a revalidated classic JavaScript asset with the canonical content type.
#[must_use]
pub const fn javascript(path: &'static str, bytes: &'static [u8]) -> Asset {
    Asset::embedded(
        path,
        "text/javascript; charset=utf-8",
        bytes,
        CachePolicy::Revalidate,
    )
}

/// Registers a revalidated UTF-8 plain-text asset.
#[must_use]
pub const fn text(path: &'static str, bytes: &'static [u8]) -> Asset {
    Asset::embedded(
        path,
        "text/plain; charset=utf-8",
        bytes,
        CachePolicy::Revalidate,
    )
}

/// Registers a revalidated image asset.
#[must_use]
pub const fn image(path: &'static str, content_type: &'static str, bytes: &'static [u8]) -> Asset {
    Asset::embedded(path, content_type, bytes, CachePolicy::Revalidate)
}

/// Registers a versioned WOFF2 asset with immutable caching.
#[must_use]
pub const fn font(path: &'static str, bytes: &'static [u8]) -> Asset {
    Asset::embedded(path, "font/woff2", bytes, CachePolicy::Immutable)
}

#[must_use]
pub fn foundation_assets() -> Vec<Asset> {
    vec![
        css(GLOBAL_STYLESHEET_PATH, crate::GLOBAL_STYLESHEET),
        font(
            "/-/assets/fonts/ibm-plex-sans-variable-0.2.0-roman.woff2",
            include_bytes!("../assets/fonts/ibm-plex-sans-variable-roman.woff2"),
        ),
        font(
            "/-/assets/fonts/ibm-plex-sans-variable-0.2.0-italic.woff2",
            include_bytes!("../assets/fonts/ibm-plex-sans-variable-italic.woff2"),
        ),
        font(
            "/-/assets/fonts/ibm-plex-mono-variable-1.0.0-roman.woff2",
            include_bytes!("../assets/fonts/ibm-plex-mono-variable-roman.woff2"),
        ),
        font(
            "/-/assets/fonts/ibm-plex-mono-variable-1.0.0-italic.woff2",
            include_bytes!("../assets/fonts/ibm-plex-mono-variable-italic.woff2"),
        ),
        font(
            "/-/assets/fonts/ibm-plex-math-1.1.0-regular.woff2",
            include_bytes!("../assets/fonts/ibm-plex-math-regular.woff2"),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foundation_assets_use_canonical_paths_and_versioned_fonts() {
        let assets = foundation_assets();
        assert_eq!(assets[0].path(), GLOBAL_STYLESHEET_PATH);
        assert!(assets[0].bytes().starts_with(b"@layer global,components;"));
        assert!(
            assets[0]
                .bytes()
                .windows(b"@font-face".len())
                .any(|window| window == b"@font-face")
        );
        assert_eq!(assets.len(), 6);
        assert!(assets.iter().skip(1).all(|asset| {
            asset.path().starts_with(FONT_PREFIX)
                && ["0.2.0", "1.0.0", "1.1.0"]
                    .iter()
                    .any(|version| asset.path().contains(version))
                && asset.cache_policy() == CachePolicy::Immutable
                && !asset.bytes().is_empty()
        }));
    }
}
