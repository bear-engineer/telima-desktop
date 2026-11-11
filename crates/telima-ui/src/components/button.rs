//! Buttons (CSS .btn): 28 px, `border_strong` border, weight 500 (L2). Danger is an outline
//! button and primary hover keeps the label >= 4.5:1 (H7).
use gpui_kit::{
    Div, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    Stateful, StatefulInteractiveElement, Styled, div, px,
};

use crate::style::hsla;
use crate::theme::{Palette, Rgb, font, metric};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Default,
    Primary,
    Danger,
    Ghost,
}

/// Colors of one button variant. `None` background or border means transparent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonColors {
    pub bg: Option<Rgb>,
    pub border: Option<Rgb>,
    pub hover: Rgb,
    pub pressed: Rgb,
    pub text: Rgb,
}

pub fn button_colors(variant: ButtonVariant, p: &Palette) -> ButtonColors {
    match variant {
        ButtonVariant::Default => ButtonColors {
            bg: Some(p.bg_raised),
            border: Some(p.border_strong),
            hover: p.bg_hover,
            pressed: p.bg_pressed,
            text: p.text,
        },
        ButtonVariant::Primary => ButtonColors {
            bg: Some(p.accent),
            border: Some(p.accent),
            hover: p.accent_hover,
            pressed: p.accent_pressed,
            text: p.on_accent,
        },
        ButtonVariant::Danger => ButtonColors {
            bg: Some(p.bg_raised),
            border: Some(p.err_border),
            hover: p.bg_hover,
            pressed: p.err_bg,
            text: p.err,
        },
        ButtonVariant::Ghost => ButtonColors {
            bg: None,
            border: None,
            hover: p.bg_hover,
            pressed: p.bg_pressed,
            text: p.text_2,
        },
    }
}

fn base(id: impl Into<ElementId>, p: &Palette) -> Stateful<Div> {
    let focus = hsla(p.focus);
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(metric::SPACE[0]))
        .h(px(metric::BTN))
        .rounded(px(metric::RADIUS_MD))
        .border_1()
        .text_size(px(font::BODY.size))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .focus_visible(move |s| s.border_color(focus))
}

fn styled(el: Stateful<Div>, c: ButtonColors) -> Stateful<Div> {
    let el = match c.bg {
        Some(bg) => el.bg(hsla(bg)),
        None => el,
    };
    let border = c.border.map(hsla).unwrap_or(gpui_kit::transparent_black());
    let (hover, pressed) = (hsla(c.hover), hsla(c.pressed));
    el.border_color(border)
        .text_color(hsla(c.text))
        .hover(move |s| s.bg(hover))
        .active(move |s| s.bg(pressed))
}

/// Text button. Callers add `.on_click(..)`.
pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    variant: ButtonVariant,
    p: &Palette,
) -> Stateful<Div> {
    styled(
        base(id, p).px(px(metric::BTN_PAD_X)),
        button_colors(variant, p),
    )
    .child(label.into().into_any_element())
}

/// Text button with a leading line icon in the label color (CSS `.btn > .ic`).
pub fn button_with_icon(
    id: impl Into<ElementId>,
    kind: super::icons::Icon,
    label: impl Into<SharedString>,
    variant: ButtonVariant,
    p: &Palette,
) -> Stateful<Div> {
    let c = button_colors(variant, p);
    styled(base(id, p).px(px(metric::BTN_PAD_X)), c)
        .child(super::icons::icon(kind, hsla(c.text)))
        .child(label.into().into_any_element())
}

/// Disabled text button (CSS .btn[disabled]): no hover, no press, no click.
pub fn button_disabled(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    p: &Palette,
) -> Stateful<Div> {
    base(id, p)
        .px(px(metric::BTN_PAD_X))
        .border_color(hsla(p.border))
        .text_color(hsla(p.text_disabled))
        .child(label.into().into_any_element())
}

/// Square icon button (CSS .btn-icon), ghost colors.
pub fn icon_button(id: impl Into<ElementId>, icon: impl IntoElement, p: &Palette) -> Stateful<Div> {
    styled(
        base(id, p).w(px(metric::BTN)),
        button_colors(ButtonVariant::Ghost, p),
    )
    .child(icon)
}
