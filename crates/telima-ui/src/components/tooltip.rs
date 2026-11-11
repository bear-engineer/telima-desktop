//! Text tooltip in the telima palette (gpui-component's Tooltip follows its own theme, so it
//! stayed light in the dark theme).
use gpui_kit::{
    AnyView, App, AppContext, Context, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, px,
};

use crate::style::{hsla, palette, space};
use crate::theme::{font, metric};

/// Tooltip view: raised surface, border and text from the window's palette.
pub struct TextTip {
    text: SharedString,
}

impl Render for TextTip {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let p = palette(window);
        div()
            .px(space(1))
            .py(space(0))
            .rounded(px(metric::RADIUS_SM))
            .border_1()
            .border_color(hsla(p.border))
            .bg(hsla(p.bg_raised))
            .text_color(hsla(p.text))
            .text_size(px(font::SMALL.size))
            .shadow_md()
            .child(self.text.clone())
    }
}

/// Builder for `.tooltip(..)` showing `text`.
pub fn text_tip(text: SharedString) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |_, cx| {
        let text = text.clone();
        cx.new(|_| TextTip { text }).into()
    }
}
