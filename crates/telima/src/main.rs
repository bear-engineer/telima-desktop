//! Composition root: builds adapters, injects them, starts the GPUI application.

use gpui_kit::AppContext;

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        let options = telima_ui::main_window_options(cx);
        gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| telima_ui::RootView))
            .expect("open main window");
    });
}
