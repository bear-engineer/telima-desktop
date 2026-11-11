//! Small building blocks shared by the form views. Colors and sizes come from `theme`.
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use crate::style::{hsla, space};
use crate::theme::{Palette, font, metric};

/// Push button. Disabled buttons get no hover and no click handler (the caller decides).
pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
    tone: Tone,
    p: &Palette,
) -> Stateful<Div> {
    let (bg, fg, border) = match (enabled, tone) {
        (false, _) => (p.bg_chrome, p.text_disabled, p.border),
        (true, Tone::Primary) => (p.accent, p.on_accent, p.accent),
        (true, Tone::Danger) => (p.err, p.on_accent, p.err),
        (true, Tone::Plain) => (p.bg_raised, p.text, p.border),
    };
    let el = div()
        .id(id.into())
        .h(px(metric::ROW))
        .px(space(2))
        .flex()
        .items_center()
        .rounded(px(metric::RADIUS_MD))
        .border_1()
        .border_color(hsla(border))
        .bg(hsla(bg))
        .text_color(hsla(fg))
        .text_size(px(font::BODY.size))
        .child(label.into());
    if enabled {
        let hover = match tone {
            Tone::Primary => p.accent_text,
            Tone::Danger => p.err_border,
            Tone::Plain => p.bg_hover,
        };
        el.cursor_pointer().hover(move |s| s.bg(hsla(hover)))
    } else {
        el
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Primary,
    Danger,
}

/// One-line text box drawn from a string and a focus handle. Key handling is the caller's.
pub fn text_box(
    id: impl Into<ElementId>,
    value: &str,
    placeholder: &str,
    locked: bool,
    focus: &FocusHandle,
    window: &Window,
    p: &Palette,
) -> Stateful<Div> {
    let focused = focus.is_focused(window);
    let (text, color) = if value.is_empty() {
        (placeholder.to_owned(), p.text_disabled)
    } else if locked {
        (value.to_owned(), p.text_2)
    } else {
        (value.to_owned(), p.text)
    };
    div()
        .id(id.into())
        .track_focus(focus)
        .h(px(metric::ROW))
        .px(space(2))
        .flex()
        .items_center()
        .rounded(px(metric::RADIUS_MD))
        .border_1()
        .border_color(hsla(if focused { p.focus } else { p.border }))
        .bg(hsla(if locked { p.bg_chrome } else { p.bg }))
        .text_color(hsla(color))
        .text_size(px(font::BODY.size))
        .child(text)
}

/// Applies a key press to a one-line text value. Returns true when the value changed.
pub fn edit_text(value: &mut String, ev: &KeyDownEvent) -> bool {
    let k = &ev.keystroke;
    if k.modifiers.platform || k.modifiers.control {
        return false;
    }
    match k.key.as_str() {
        "backspace" => value.pop().is_some(),
        _ => match &k.key_char {
            Some(ch) if !ch.chars().any(char::is_control) => {
                value.push_str(ch);
                true
            }
            _ => false,
        },
    }
}

/// Label column + control + inline violations under it.
pub fn field_row(
    label: &'static str,
    control: impl IntoElement,
    hint: Option<&'static str>,
    errors: Vec<String>,
    p: &Palette,
) -> Div {
    v_flex()
        .gap(space(0))
        .child(
            h_flex()
                .gap(space(3))
                .child(
                    div()
                        .w(px(96.0))
                        .text_color(hsla(p.text_2))
                        .text_size(px(font::BODY.size))
                        .child(label),
                )
                .child(control),
        )
        .children(hint.map(|h| {
            div()
                .pl(px(96.0 + metric::SPACE[2]))
                .text_color(hsla(p.text_2))
                .text_size(px(font::SMALL.size))
                .child(h)
        }))
        .children(errors.into_iter().map(|e| {
            div()
                .pl(px(96.0 + metric::SPACE[2]))
                .text_color(hsla(p.err))
                .text_size(px(font::SMALL.size))
                .child(format!("▲ {e}"))
        }))
}
