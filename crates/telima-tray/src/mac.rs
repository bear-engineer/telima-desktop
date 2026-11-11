//! macOS: tray-icon(muda 메뉴). GPUI run 콜백의 메인 스레드에서 만든다.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use telima_app::AppCommand;
use telima_app::view::tray::TrayModel;
use tray_icon::menu::accelerator::{Accelerator, Code, Modifiers};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::icon_state;
use crate::icons::{TrayIcons, decode_png, macos_png};
use crate::menu::{MenuNode, menu_nodes};

pub struct Inner {
    tray: TrayIcon,
    commands: Arc<Mutex<Vec<AppCommand>>>,
}

fn icon(model: &TrayModel) -> Option<Icon> {
    let image = decode_png(macos_png(icon_state(model))).ok()?;
    Icon::from_rgba(image.data, image.width, image.height).ok()
}

/// 메뉴 항목 id는 `commands`의 위치다.
fn build_menu(nodes: &[MenuNode], commands: &mut Vec<AppCommand>) -> Option<Menu> {
    let menu = Menu::new();
    append_all(&menu, nodes, commands)?;
    Some(menu)
}

trait Container {
    fn add(&self, item: &dyn tray_icon::menu::IsMenuItem) -> Option<()>;
}
impl Container for Menu {
    fn add(&self, item: &dyn tray_icon::menu::IsMenuItem) -> Option<()> {
        self.append(item).ok()
    }
}
impl Container for Submenu {
    fn add(&self, item: &dyn tray_icon::menu::IsMenuItem) -> Option<()> {
        self.append(item).ok()
    }
}

/// Shortcut hint drawn next to a menu item: ⌘Q on `telima 종료` (canvas 10). A status-item
/// menu only answers its key equivalents while it is open and tracking the keyboard, so this
/// never fires together with the app's cmd-q binding (crates/telima/src/quit.rs).
fn accelerator(command: &AppCommand) -> Option<Accelerator> {
    (*command == AppCommand::Quit).then(|| Accelerator::new(Modifiers::META, Code::KeyQ))
}

fn append_all(
    parent: &impl Container,
    nodes: &[MenuNode],
    commands: &mut Vec<AppCommand>,
) -> Option<()> {
    for node in nodes {
        match node {
            MenuNode::Separator => parent.add(&PredefinedMenuItem::separator())?,
            MenuNode::Item {
                label,
                enabled,
                command,
            } => {
                let item = match command {
                    Some(command) => {
                        let id = MenuId::new(commands.len().to_string());
                        commands.push(command.clone());
                        MenuItem::with_id(id, label, *enabled, accelerator(command))
                    }
                    None => MenuItem::new(label, *enabled, None),
                };
                parent.add(&item)?;
            }
            MenuNode::Submenu { label, items } => {
                let sub = Submenu::new(label, true);
                append_all(&sub, items, commands)?;
                parent.add(&sub)?;
            }
        }
    }
    Some(())
}

impl Inner {
    pub fn new(model: &TrayModel, _icons: TrayIcons, tx: Sender<AppCommand>) -> Option<Self> {
        let mut commands = Vec::new();
        let menu = build_menu(&menu_nodes(model), &mut commands)?;
        let commands = Arc::new(Mutex::new(commands));
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon_templated(icon(model)?)
            .with_tooltip(icon_state(model).tooltip())
            .with_menu_on_left_click(true)
            .build()
            .ok()?;
        let table = Arc::clone(&commands);
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let Ok(index) = event.id.0.parse::<usize>() else {
                return;
            };
            let command = table.lock().ok().and_then(|t| t.get(index).cloned());
            if let Some(command) = command {
                let _ = tx.send(command);
            }
        }));
        Some(Self { tray, commands })
    }

    pub fn update(&mut self, model: &TrayModel) {
        let mut commands = Vec::new();
        if let Some(menu) = build_menu(&menu_nodes(model), &mut commands) {
            if let Ok(mut table) = self.commands.lock() {
                *table = commands;
            }
            self.tray.set_menu(Some(Box::new(menu)));
        }
        if let Some(icon) = icon(model) {
            let _ = self.tray.set_icon_templated(Some(icon));
        }
        let _ = self.tray.set_tooltip(Some(icon_state(model).tooltip()));
    }
}

#[cfg(test)]
mod tests {
    use telima_app::AppCommand;
    use telima_core::profile::ProfileName;
    use tray_icon::menu::accelerator::{Accelerator, Code, Modifiers};

    use super::accelerator;

    #[test]
    fn only_quit_carries_the_cmd_q_hint() {
        assert_eq!(
            accelerator(&AppCommand::Quit),
            Some(Accelerator::new(Modifiers::META, Code::KeyQ))
        );
        let dev = ProfileName::parse("dev").unwrap();
        for cmd in [
            AppCommand::OpenWindow,
            AppCommand::Refresh,
            AppCommand::NewProfile,
            AppCommand::Start(dev.clone()),
            AppCommand::Stop(dev.clone()),
            AppCommand::Restart(dev),
        ] {
            assert_eq!(accelerator(&cmd), None, "{cmd:?}");
        }
    }
}
