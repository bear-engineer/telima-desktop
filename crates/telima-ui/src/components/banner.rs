//! Inline notice banner (error / warning) under a page header.
use gpui_kit::{Div, Styled, div, px};

use crate::style::hsla;
use crate::theme::{Palette, Rgb, font, metric};

/// How loud a banner is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerTone {
    /// Something failed or blocks use.
    Error,
    /// Works, but the user should know.
    Warning,
}

/// Token colors of a banner tone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BannerColors {
    pub bg: Rgb,
    pub border: Rgb,
    pub text: Rgb,
}

pub fn banner_colors(tone: BannerTone, p: &Palette) -> BannerColors {
    match tone {
        BannerTone::Error => BannerColors {
            bg: p.err_bg,
            border: p.err_border,
            text: p.err,
        },
        BannerTone::Warning => BannerColors {
            bg: p.busy_bg,
            border: p.border,
            text: p.busy,
        },
    }
}

/// Banner box; callers add the text (and an optional action) as children.
pub fn banner(tone: BannerTone, p: &Palette) -> Div {
    let c = banner_colors(tone, p);
    div()
        .flex()
        .items_center()
        .gap(px(metric::SPACE[1]))
        .px(px(metric::SPACE[2]))
        .py(px(metric::SPACE[1]))
        .rounded(px(metric::RADIUS_MD))
        .bg(hsla(c.bg))
        .border_1()
        .border_color(hsla(c.border))
        .text_color(hsla(c.text))
        .text_size(px(font::SMALL.size))
}
