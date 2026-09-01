// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: Apache-2.0

use maud::{DOCTYPE, Markup, Render, html};

use crate::{
    APP_SCRIPT_PATH, APP_STYLESHEET_PATH, APPLE_TOUCH_ICON_PATH, FAVICON_ICO_PATH,
    FAVICON_SVG_PATH, GLOBAL_STYLESHEET_PATH, MANIFEST_PATH,
};

pub struct Document<'a> {
    title: &'a str,
    body: Markup,
    head: Option<Markup>,
    assets: u8,
}

const SCRIPT: u8 = 1 << 0;
const MANIFEST: u8 = 1 << 1;
const SVG_ICON: u8 = 1 << 2;
const APPLE_TOUCH_ICON: u8 = 1 << 3;

impl<'a> Document<'a> {
    #[must_use]
    pub fn new(title: &'a str, body: Markup) -> Self {
        Self {
            title,
            body,
            head: None,
            assets: 0,
        }
    }

    #[must_use]
    pub fn with_head(mut self, head: Markup) -> Self {
        self.head = Some(head);
        self
    }

    #[must_use]
    pub const fn with_script(mut self) -> Self {
        self.assets |= SCRIPT;
        self
    }

    #[must_use]
    pub const fn with_manifest(mut self) -> Self {
        self.assets |= MANIFEST;
        self
    }

    #[must_use]
    pub const fn with_svg_icon(mut self) -> Self {
        self.assets |= SVG_ICON;
        self
    }

    #[must_use]
    pub const fn with_apple_touch_icon(mut self) -> Self {
        self.assets |= APPLE_TOUCH_ICON;
        self
    }
}

impl Render for Document<'_> {
    fn render(&self) -> Markup {
        html! {
            (DOCTYPE)
            html lang="en" {
                head {
                    meta charset="UTF-8";
                    meta name="viewport" content="width=device-width,initial-scale=1";

                    title { (self.title) }

                    link rel="icon" href=(FAVICON_ICO_PATH);
                    @if self.assets & SVG_ICON != 0 {
                        link rel="icon" href=(FAVICON_SVG_PATH) type="image/svg+xml" sizes="any";
                    }
                    @if self.assets & APPLE_TOUCH_ICON != 0 {
                        link rel="apple-touch-icon" href=(APPLE_TOUCH_ICON_PATH);
                    }

                    @if self.assets & MANIFEST != 0 {
                        link rel="manifest" href=(MANIFEST_PATH);
                    }

                    link rel="stylesheet" href=(GLOBAL_STYLESHEET_PATH);
                    link rel="stylesheet" href=(APP_STYLESHEET_PATH);

                    @if self.assets & SCRIPT != 0 {
                        script src=(APP_SCRIPT_PATH) defer {}
                    }

                    @if let Some(head) = &self.head {
                        (head)
                    }
                }
                body { (&self.body) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_shared_and_application_assets_without_inline_code() {
        let rendered = Document::new("<Service>", html! { main { "ok" } })
            .with_script()
            .with_manifest()
            .with_svg_icon()
            .with_apple_touch_icon()
            .with_head(html! { meta name="generator" content="test"; })
            .render()
            .into_string();

        assert!(rendered.starts_with("<!DOCTYPE html>"));
        assert!(rendered.contains("&lt;Service&gt;"));
        assert!(rendered.contains(GLOBAL_STYLESHEET_PATH));
        assert!(rendered.contains(APP_STYLESHEET_PATH));
        assert!(rendered.contains(APP_SCRIPT_PATH));
        assert!(rendered.contains(FAVICON_ICO_PATH));
        assert!(rendered.contains(FAVICON_SVG_PATH));
        assert!(rendered.contains(APPLE_TOUCH_ICON_PATH));
        assert!(rendered.contains(MANIFEST_PATH));
        assert!(rendered.contains("name=\"generator\""));
        assert!(!rendered.contains("<style"));
    }
}
