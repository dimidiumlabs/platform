// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: Apache-2.0

pub mod assets;
pub mod components;

pub use assets::{
    APP_SCRIPT_PATH, APP_STYLESHEET_PATH, APPLE_TOUCH_ICON_PATH, ASSET_PREFIX, Asset, CachePolicy,
    FAVICON_ICO_PATH, FAVICON_SVG_PATH, FONT_PREFIX, GLOBAL_STYLESHEET_PATH, MANIFEST_PATH,
    ROBOTS_PATH, css, font, foundation_assets, image, javascript, text,
};
pub use components::Document;

pub const GLOBAL_STYLESHEET: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/stylesheet.css"));

#[cfg(test)]
mod tests {
    #[test]
    fn global_styles_provide_semantic_typographic_rhythm() {
        let styles = include_str!("styles/global.css");

        for declaration in [
            "--readable-line-width: 32em",
            "font-size: 1.125rem",
            "line-height: 1.4",
            "@media (min-width: 40rem)",
            "margin-bottom: 1rem",
            "--list-item-spacing: 0.875rem",
        ] {
            assert!(styles.contains(declaration), "missing {declaration}");
        }

        for selector in [
            "h1,\nh2,\nh3,\nh4,\nh5,\nh6",
            ":is(ul, ol)",
            "blockquote",
            "figure",
            "figcaption",
            "pre code",
        ] {
            assert!(styles.contains(selector), "missing {selector}");
        }
    }
}
