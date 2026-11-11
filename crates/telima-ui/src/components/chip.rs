//! Status chip (CSS .chip): shape + label on the status background.
use gpui_kit::{
    Div, ElementId, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, px,
};

use super::status::{Status, status_shape};
use crate::style::hsla;
use crate::theme::{Palette, font, metric};

pub fn status_chip(
    id: impl Into<ElementId>,
    status: Status,
    label: impl Into<SharedString>,
    p: &Palette,
) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(metric::SPACE[0]))
        .h(px(metric::CHIP_H))
        .pl(px(metric::CHIP_PAD_START))
        .pr(px(metric::SPACE[1]))
        .rounded(px(metric::RADIUS_SM))
        .bg(hsla(status.chip_bg(p)))
        .text_color(hsla(status.color(p)))
        .text_size(px(font::SMALL.size))
        .line_height(px(font::SMALL.line_height))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .child(status_shape(id, status, p))
        .child(label.into().into_any_element())
}

/// Neutral chip with a label only (facts next to a status chip, e.g. uptime).
pub fn label_chip(label: impl Into<SharedString>, p: &Palette) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(metric::CHIP_H))
        .px(px(metric::SPACE[1]))
        .rounded(px(metric::RADIUS_SM))
        .bg(hsla(p.bg_sunken))
        .text_color(hsla(p.text_2))
        .text_size(px(font::SMALL.size))
        .line_height(px(font::SMALL.line_height))
        .whitespace_nowrap()
        .child(label.into().into_any_element())
}
