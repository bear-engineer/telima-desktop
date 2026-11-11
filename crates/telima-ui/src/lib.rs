//! GPUI views. Knows view models from `telima-app`, never adapters.
pub mod shell;
pub mod theme;
use gpui_kit::component::v_flex;
use gpui_kit::*;

/// Product name shown in the window. Single definition used by the title bar and tests.
pub const APP_NAME: &str = "telima";

/// Root view of the main window.
#[derive(Default)]
pub struct RootView;

impl Render for RootView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex().size_full().p_8().child(
            div()
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .child(APP_NAME),
        )
    }
}

/// Window options for the main window.
pub fn main_window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(1080.), px(720.)), cx)),
        titlebar: Some(TitlebarOptions {
            title: Some(APP_NAME.into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::RootView;
    use gpui_kit::{AppContext, TestAppContext};

    #[gpui_kit::test]
    fn root_view_can_be_created(cx: &mut TestAppContext) {
        let view = cx.new(|_| RootView);
        view.read_with(cx, |_, _| ());
    }
}
