//! Linux: ksni(StatusNotifierItem). 호스트가 없으면 만들지 않는다.

use std::sync::mpsc::Sender;

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{StandardItem, SubMenu};
use ksni::{Icon, MenuItem, ToolTip, Tray};
use telima_app::AppCommand;
use telima_app::view::tray::{TrayIconState, TrayModel};

use crate::icon_state;
use crate::icons::{PanelVariant, TrayIcons, decode_png, linux_png, rgba_to_argb};
use crate::menu::{MenuNode, menu_nodes};

struct SniTray {
    tx: Sender<AppCommand>,
    nodes: Vec<MenuNode>,
    state: TrayIconState,
    panel: PanelVariant,
}

fn convert(nodes: &[MenuNode], tx: &Sender<AppCommand>) -> Vec<MenuItem<SniTray>> {
    nodes
        .iter()
        .map(|node| match node {
            MenuNode::Separator => MenuItem::Separator,
            MenuNode::Item {
                label,
                enabled,
                command,
            } => {
                let tx = tx.clone();
                let command = command.clone();
                StandardItem {
                    label: label.clone(),
                    enabled: *enabled,
                    activate: Box::new(move |_: &mut SniTray| {
                        if let Some(command) = &command {
                            let _ = tx.send(command.clone());
                        }
                    }),
                    ..Default::default()
                }
                .into()
            }
            MenuNode::Submenu { label, items } => SubMenu {
                label: label.clone(),
                submenu: convert(items, tx),
                ..Default::default()
            }
            .into(),
        })
        .collect()
}

impl Tray for SniTray {
    fn id(&self) -> String {
        "telima".into()
    }

    fn title(&self) -> String {
        "telima".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        [22, 24]
            .into_iter()
            .filter_map(|size| {
                let image = decode_png(linux_png(self.state, size, self.panel)).ok()?;
                Some(Icon {
                    width: i32::try_from(image.width).ok()?,
                    height: i32::try_from(image.height).ok()?,
                    data: rgba_to_argb(&image),
                })
            })
            .collect()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: self.state.tooltip().into(),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        convert(&self.nodes, &self.tx)
    }
}

pub struct Inner {
    handle: Handle<SniTray>,
}

impl Inner {
    pub fn new(model: &TrayModel, icons: TrayIcons, tx: Sender<AppCommand>) -> Option<Self> {
        let tray = SniTray {
            tx: tx.clone(),
            nodes: menu_nodes(model),
            state: icon_state(model),
            panel: icons.panel,
        };
        let handle = tray.spawn().ok()?;
        Some(Self { handle })
    }

    pub fn update(&mut self, model: &TrayModel) {
        let nodes = menu_nodes(model);
        let state = icon_state(model);
        let _ = self.handle.update(|tray| {
            tray.nodes = nodes;
            tray.state = state;
        });
    }
}
