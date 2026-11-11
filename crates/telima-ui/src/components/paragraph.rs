//! Korean-safe paragraph (N5). GPUI breaks CJK text between any two syllables, so Korean
//! words split mid-word ("지워집니/다."). A paragraph lays out each eojeol (space-separated
//! word) as a no-wrap run inside a wrapping row: lines break only between words.
//! Text style (size, color, weight) is inherited from the caller.
use gpui_kit::{Div, ParentElement, Styled, div, px};

use crate::theme::metric;

/// Space-separated words of `text`, in order; runs of whitespace count once.
pub fn eojeols(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}

/// Wrapping row of `text`'s words. Words are never split; the gap between them is
/// the smallest spacing step, about one space at caption sizes.
pub fn paragraph(text: impl AsRef<str>) -> Div {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .min_w_0()
        .gap_x(px(metric::SPACE[0]))
        .children(
            eojeols(text.as_ref())
                .into_iter()
                .map(|w| div().flex_none().whitespace_nowrap().child(w.to_owned())),
        )
}
