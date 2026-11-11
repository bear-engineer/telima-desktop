//! Switch for booleans (CSS .switch, M11): 28x16 track, 12 px knob, accent when on.
use gpui_kit::{
    App, Div, ElementId, InteractiveElement, ParentElement, Stateful, StatefulInteractiveElement,
    Styled, Window, div, px,
};

use crate::style::hsla;
use crate::theme::{Palette, metric};

/// Knob's left offset inside the track.
pub fn knob_offset(on: bool) -> f32 {
    let inset = (metric::SWITCH_H - metric::SWITCH_KNOB) / 2.0;
    if on {
        metric::SWITCH_W - metric::SWITCH_KNOB - inset
    } else {
        inset
    }
}

/// `on_toggle(new_value)` fires on click. Debug selector: `switch-<id>`.
pub fn switch(
    id: &'static str,
    on: bool,
    p: &Palette,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let track = hsla(if on { p.accent } else { p.border_strong });
    let focus = hsla(p.focus);
    let inset = (metric::SWITCH_H - metric::SWITCH_KNOB) / 2.0;
    div()
        .id(ElementId::Name(id.into()))
        .debug_selector(move || format!("switch-{id}"))
        .relative()
        .flex_none()
        .w(px(metric::SWITCH_W))
        .h(px(metric::SWITCH_H))
        .rounded_full()
        .bg(track)
        .focus_visible(move |s| s.border_1().border_color(focus))
        .on_click(move |_, window, cx| on_toggle(!on, window, cx))
        .child(
            div()
                .absolute()
                .top(px(inset))
                .left(px(knob_offset(on)))
                .size(px(metric::SWITCH_KNOB))
                .rounded_full()
                .bg(hsla(p.on_accent)),
        )
}
