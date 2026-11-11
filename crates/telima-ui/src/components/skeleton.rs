//! Loading placeholder row (CSS .skel): ROW tall, one bar per column.
use gpui_kit::{Div, ParentElement, Styled, div, px, relative};

use crate::style::hsla;
use crate::theme::{Palette, metric};

/// `widths` are fractions (0..=1) of the row width, one bar each.
pub fn skeleton_row(widths: &[f32], p: &Palette) -> Div {
    let bar = hsla(p.skeleton);
    div()
        .flex()
        .items_center()
        .gap(px(metric::SPACE[2]))
        .h(px(metric::ROW))
        .px(px(metric::SPACE[1]))
        .children(widths.iter().map(|w| {
            div()
                .h(px(metric::SKELETON_H))
                .w(relative(*w))
                .rounded(px(metric::RADIUS_SM))
                .bg(bar)
        }))
}
