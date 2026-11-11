//! Window toolbar (canvas 07-09): profile dropdown, drag area, lifecycle buttons or the
//! running command's progress, divider, refresh, new profile. Copy comes from telima-app.
use gpui_kit::{App, Context};
use telima_app::AppCommand;
use telima_app::view::StatusKind;
use telima_app::view::tray::{ToolbarActions, toolbar_actions};

use super::MainView;
use crate::components::icons::Icon;

/// One profile row of the toolbar dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuRow {
    pub name: String,
    pub kind: StatusKind,
    /// The profile shown in the window (check mark).
    pub current: bool,
}

/// Icon drawn on a toolbar button.
pub fn lifecycle_icon(_cmd: &AppCommand) -> Icon {
    Icon::Play
}

impl MainView {
    /// Toolbar buttons for the selected profile (RED stub).
    pub fn toolbar_actions(&self) -> ToolbarActions {
        toolbar_actions(None)
    }

    /// Progress text replacing the lifecycle buttons while a command runs (RED stub).
    pub fn toolbar_progress(&self, _cx: &App) -> Option<String> {
        None
    }

    /// Rows of the open profile dropdown (RED stub).
    pub fn profile_menu(&self) -> Option<Vec<MenuRow>> {
        None
    }

    pub fn toggle_profile_menu(&mut self, _cx: &mut Context<Self>) {}

    pub fn pick_profile(&mut self, _name: &str, _cx: &mut Context<Self>) {}

    pub fn run_command(&mut self, _cmd: &AppCommand, _cx: &mut Context<Self>) {}
}
