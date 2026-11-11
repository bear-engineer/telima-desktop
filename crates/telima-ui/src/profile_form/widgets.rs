//! Small building blocks shared by the form views. Colors and sizes come from `theme`.
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use crate::components;
use crate::components::paragraph::paragraph;
use crate::components::status::{Status, status_shape};
use crate::style::{hsla, space};
use crate::theme::{Palette, font, metric};

/// Push button from the shared component. Disabled buttons get no hover and no click
/// handler (the caller adds `.on_click` only when enabled).
pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
    tone: Tone,
    p: &Palette,
) -> Stateful<Div> {
    if enabled {
        components::button::button(id, label, tone, p)
    } else {
        components::button::button_disabled(id, label, p)
    }
}

pub use components::button::ButtonVariant as Tone;

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
        .h(px(metric::BTN))
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
                        .w(px(metric::FIELD_LABEL))
                        .flex_none()
                        .text_color(hsla(p.text_2))
                        .text_size(px(font::BODY.size))
                        .child(label),
                )
                .child(control),
        )
        .children(hint.map(|h| hint_line(h, p).pl(px(metric::FIELD_LABEL + metric::SPACE[2]))))
        .children(errors.into_iter().enumerate().map(|(i, e)| {
            div()
                .pl(px(metric::FIELD_LABEL + metric::SPACE[2]))
                .child(error_line(format!("field-error-{label}-{i}"), e, p))
        }))
}

/// Secondary note in small muted text (field hints, consequences, restart notice).
pub fn hint_line(text: impl Into<SharedString>, p: &Palette) -> Div {
    div()
        .text_color(hsla(p.text_2))
        .text_size(px(font::SMALL.size))
        .child(paragraph(text.into()))
}

/// Error message led by the status triangle (L4), wrapping between words (N5).
pub fn error_line(id: impl Into<SharedString>, text: impl AsRef<str>, p: &Palette) -> Div {
    h_flex()
        .gap(space(0))
        .items_start()
        .text_color(hsla(p.err))
        .text_size(px(font::SMALL.size))
        .child(status_shape(ElementId::Name(id.into()), Status::Error, p))
        .child(paragraph(text).flex_1())
}
