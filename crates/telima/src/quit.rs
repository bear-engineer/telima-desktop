//! 앱 종료: 단축키(macOS ⌘Q, Linux Ctrl+Q)와 macOS 앱 메뉴의 'telima 종료'.
//!
//! 트레이가 있으면 `QuitMode::Explicit`이라 창을 닫아도 앱이 남는다. 창이 없어도 단축키가
//! 듣도록 앱 메뉴 항목에 같은 동작을 건다(메뉴가 키 바인딩을 단축키로 보인다).

use gpui_kit::{App, KeyBinding, Menu, MenuItem, actions};
use telima_app::view::tray::TRAY_QUIT;

actions!(telima, [Quit]);

/// 이 OS의 종료 단축키(GPUI 키 표기).
#[cfg(target_os = "macos")]
pub const QUIT_KEYS: &str = "cmd-q";
#[cfg(not(target_os = "macos"))]
pub const QUIT_KEYS: &str = "ctrl-q";

/// 앱 메뉴: 'telima' 메뉴 하나에 'telima 종료' 하나.
pub fn app_menus() -> Vec<Menu> {
    vec![Menu {
        name: telima_ui::APP_NAME.into(),
        items: vec![MenuItem::action(TRAY_QUIT, Quit)],
        disabled: false,
    }]
}

/// 종료 동작·단축키·앱 메뉴를 등록한다.
pub fn install(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.bind_keys([KeyBinding::new(QUIT_KEYS, Quit, None)]);
    // macOS shows it as the app menu; Linux keeps it unused.
    cx.set_menus(app_menus());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quit_shortcut_is_cmd_q_on_macos_and_ctrl_q_on_linux() {
        let want = if cfg!(target_os = "macos") {
            "cmd-q"
        } else {
            "ctrl-q"
        };
        assert_eq!(QUIT_KEYS, want);
    }

    #[test]
    fn app_menu_holds_only_the_quit_item() {
        let menus = app_menus();
        assert_eq!(menus.len(), 1);
        assert_eq!(menus[0].name.as_ref(), telima_ui::APP_NAME);
        assert_eq!(menus[0].items.len(), 1);
        match &menus[0].items[0] {
            MenuItem::Action { name, action, .. } => {
                assert_eq!(name.as_ref(), TRAY_QUIT);
                assert!(action.partial_eq(&Quit), "{}", action.name());
            }
            _ => panic!("expected the quit action item"),
        }
    }
}
