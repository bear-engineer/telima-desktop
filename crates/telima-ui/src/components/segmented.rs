//! Segmented control (Containers.dc.html:92-96): one bordered group, the selected segment on
//! `bg_pressed` with `text`, the others `text_2` (M11). No accent fill.
use std::rc::Rc;

use gpui_kit::{
    App, Div, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::style::hsla;
use crate::theme::{Palette, font, metric};

/// `on_select(index)` fires when an unselected segment is clicked. Each segment carries the
/// debug selector `segment-<id>-<index>`.
pub fn segmented(
    id: &'static str,
    options: Vec<SharedString>,
    selected: usize,
    p: &Palette,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> Div {
    let on_select = Rc::new(on_select);
    let (hover, pressed) = (hsla(p.bg_hover), hsla(p.bg_pressed));
    let focus = hsla(p.focus);
    div()
        .flex()
        .flex_none()
        .h(px(metric::BTN))
        .p(px(metric::SEGMENT_INSET))
        .gap(px(metric::SEGMENT_INSET))
        .rounded(px(metric::RADIUS_MD))
        .border_1()
        .border_color(hsla(p.border_strong))
        .bg(hsla(p.bg_raised))
        .children(options.into_iter().enumerate().map(|(i, label)| {
            let on = i == selected;
            let handler = on_select.clone();
            div()
                .id(ElementId::NamedInteger(id.into(), i as u64))
                .debug_selector(move || format!("segment-{id}-{i}"))
                .flex()
                .items_center()
                .px(px(metric::SPACE[1]))
                .rounded(px(metric::RADIUS_SM))
                .border_1()
                .border_color(gpui_kit::transparent_black())
                .text_size(px(font::BODY.size))
                .font_weight(FontWeight::MEDIUM)
                .text_color(hsla(if on { p.text } else { p.text_2 }))
                .when_some_bg(on.then_some(pressed))
                .hover(move |s| if on { s } else { s.bg(hover) })
                .active(move |s| s.bg(pressed))
                .focus_visible(move |s| s.border_color(focus))
                .on_click(move |_, window, cx| {
                    if !on {
                        handler(i, window, cx)
                    }
                })
                .child(label.into_any_element())
        }))
}

trait WhenSomeBg: Styled + Sized {
    fn when_some_bg(self, bg: Option<gpui_kit::Hsla>) -> Self {
        match bg {
            Some(bg) => self.bg(bg),
            None => self,
        }
    }
}

impl<T: Styled> WhenSomeBg for T {}
