//! GPUI views. Knows view models from `telima-app`, never adapters.
pub mod components;
pub mod engine_views;
pub mod fonts;
pub mod profile_form;
pub mod shell;
pub mod style;
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

/// App-wide setup for telima-ui. Call once at startup, after `gpui_kit::init`.
/// Registers the bundled Plex fonts; on failure the OS fallback fonts are used.
pub fn init(cx: &mut App) {
    let fonts = fonts::FONTS
        .iter()
        .map(|(_, bytes)| std::borrow::Cow::Borrowed(*bytes))
        .collect();
    if let Err(err) = cx.text_system().add_fonts(fonts) {
        eprintln!("telima: bundled fonts not registered: {err}");
    }
}

/// Window options for the main window. The title bar is transparent: the traffic lights sit in
/// the sidebar's titlebar strip and the main column draws its own titlebar.
pub fn main_window_options(cx: &App) -> WindowOptions {
    use theme::metric;
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(
            size(px(metric::WINDOW_W), px(metric::WINDOW_H)),
            cx,
        )),
        titlebar: Some(TitlebarOptions {
            title: Some(APP_NAME.into()),
            appears_transparent: true,
            traffic_light_position: Some(point(
                px(metric::TRAFFIC_LIGHT_X),
                px(metric::TRAFFIC_LIGHT_Y),
            )),
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

    #[gpui_kit::test]
    fn titlebar_is_transparent_with_traffic_lights_in_the_sidebar_strip(cx: &mut TestAppContext) {
        use crate::theme::metric;
        use gpui_kit::{point, px};
        let options = cx.update(|cx| super::main_window_options(cx));
        let titlebar = options.titlebar.expect("titlebar options");
        assert!(titlebar.appears_transparent);
        assert_eq!(
            titlebar.traffic_light_position,
            Some(point(
                px(metric::TRAFFIC_LIGHT_X),
                px(metric::TRAFFIC_LIGHT_Y)
            ))
        );
    }
}
