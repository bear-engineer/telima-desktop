//! Modal overlay: scrim (CSS --scrim, M14) and dialog frame, fading in over `motion::FAST`
//! with ease-out (M13). Reduced motion renders the end state (GPUI `with_animation`).
use gpui_kit::{
    Animation, AnimationElement, AnimationExt, Div, ElementId, IntoElement, ParentElement, Styled,
    div, px,
};

use super::hsla_alpha;
use crate::style::hsla;
use crate::theme::{Palette, metric, motion};

/// Full-window backdrop. Place it last in a `relative` root.
pub fn scrim(p: &Palette) -> Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(hsla_alpha(p.scrim))
}

/// Dialog surface of `width` px (`metric::DIALOG_W` or `metric::DIALOG_W_SM`).
pub fn dialog_frame(width: f32, p: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .w(px(width))
        .p(px(metric::SPACE[3]))
        .gap(px(metric::SPACE[2]))
        .rounded(px(metric::RADIUS_LG))
        .border_1()
        .border_color(hsla(p.border))
        .bg(hsla(p.bg_raised))
        .text_color(hsla(p.text))
        .shadow_lg()
}

/// Opacity fade-in over `motion::FAST`, ease-out.
pub fn fade_in(id: impl Into<ElementId>, el: Div) -> AnimationElement<Div> {
    el.with_animation(
        id,
        Animation::new(motion::FAST).with_easing(motion::ease_out),
        |el, t| el.opacity(t),
    )
}

/// Scrim + centred dialog frame holding `content`, fading in together.
pub fn modal(
    id: impl Into<ElementId>,
    width: f32,
    p: &Palette,
    content: impl IntoElement,
) -> AnimationElement<Div> {
    fade_in(id, scrim(p).child(dialog_frame(width, p).child(content)))
}
