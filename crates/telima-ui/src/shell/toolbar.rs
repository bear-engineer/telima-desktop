//! Window toolbar (canvas 07-09): profile dropdown, drag area, lifecycle buttons or the
//! running command's progress, divider, refresh, new profile. Every global action lives
//! here once; the header and the sidebar carry none. Copy comes from telima-app.
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ClickEvent, Context, Div, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseMoveEvent, ParentElement, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window, WindowControlArea, deferred, div, px,
};
use telima_app::AppCommand;
use telima_app::profiles::Lifecycle;
use telima_app::view::StatusKind;
use telima_app::view::tray::{
    BusyOp, CommandItem, NEW_PROFILE_MENU, ToolbarActions, profile_picker_label, toolbar_actions,
};
use telima_core::profile::ProfileName;

use super::{MainView, Screen, elapsed_text, half_step, status_of};
use crate::components::button::{ButtonVariant, button_with_icon, icon_button};
use crate::components::icons::{Icon, icon};
use crate::components::status::status_shape;
use crate::components::tooltip::text_tip;
use crate::style::{hsla, space};
use crate::theme::{Palette, Rgb, font, metric};

/// One profile row of the toolbar dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuRow {
    pub name: String,
    pub kind: StatusKind,
    /// The profile shown in the window (check mark).
    pub current: bool,
}

/// A start, stop or restart sent from this window that ended; the tray shows the last failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleEnd {
    pub name: ProfileName,
    pub cmd: Lifecycle,
    pub ok: bool,
}

/// Icon drawn on a toolbar button. Commands without a toolbar button (open window, quit)
/// never reach it; they share the new-profile glyph.
pub fn lifecycle_icon(cmd: &AppCommand) -> Icon {
    match cmd {
        AppCommand::Start(_) => Icon::Play,
        AppCommand::Stop(_) => Icon::Stop,
        AppCommand::Restart(_) => Icon::Restart,
        AppCommand::Refresh => Icon::Refresh,
        AppCommand::NewProfile | AppCommand::OpenWindow | AppCommand::Quit => Icon::Plus,
    }
}

/// Element id and debug selector of a toolbar button.
fn command_id(cmd: &AppCommand) -> &'static str {
    match cmd {
        AppCommand::Start(_) => "toolbar-start",
        AppCommand::Stop(_) => "toolbar-stop",
        AppCommand::Restart(_) => "toolbar-restart",
        AppCommand::Refresh => "toolbar-refresh",
        AppCommand::NewProfile => "toolbar-new-profile",
        AppCommand::OpenWindow | AppCommand::Quit => "toolbar-other",
    }
}

impl MainView {
    /// What runs on `name`, for the tray: the kind of a start/stop/restart sent from this
    /// window, `Other` for anything else (create, edit, delete), `None` when idle.
    pub fn busy_op(&self, name: &ProfileName, cx: &App) -> Option<BusyOp> {
        if !self.busy(name) {
            return None;
        }
        let lifecycle = self
            .progress
            .get(name)
            .filter(|_| self.pending.contains(name) && !self.placeholders.contains_key(name))
            .map(|(p, _)| BusyOp::Lifecycle(p.read(cx).cmd()));
        Some(lifecycle.unwrap_or(BusyOp::Other))
    }

    /// Start, stop and restart results since the last call, oldest first. Cancels and
    /// creates are left out.
    pub fn take_lifecycle_ends(&mut self) -> Vec<LifecycleEnd> {
        std::mem::take(&mut self.ended)
    }

    /// Keyboard-highlighted row of the open dropdown: a profile index, or one past the last
    /// profile for 새 프로필….
    pub fn profile_menu_highlight(&self) -> Option<usize> {
        self.menu_highlight.filter(|_| self.menu_open)
    }

    /// Selects `name` once the profile list arrives (a reopened window keeps the last
    /// profile). Falls back to the first profile when `name` is gone.
    pub fn prefer(&mut self, name: ProfileName) {
        self.selected = Some(name);
    }

    /// Toolbar buttons for the selected profile; no lifecycle buttons outside the profile
    /// screen, for a create placeholder Colima does not list, or while a command runs.
    pub fn toolbar_actions(&self) -> ToolbarActions {
        let selected = self
            .selected
            .clone()
            .filter(|n| self.screen() == Screen::Profile && !self.unlisted(n))
            .and_then(|n| self.detail().map(|d| (n, d.kind)));
        toolbar_actions(selected.as_ref().map(|(n, k)| (n, *k)))
    }

    /// Status of the selected profile's running command and its elapsed `mm:ss`.
    fn progress_parts(&self, cx: &App) -> Option<(String, Option<String>)> {
        let name = self.selected.as_ref()?;
        if self.screen() != Screen::Profile || !self.busy(name) {
            return None;
        }
        let status = self.header_status_label(cx)?;
        Some((status, self.started.get(name).map(|t| elapsed_text(*t))))
    }

    /// Progress text replacing the lifecycle buttons while a command runs (canvas 09).
    pub fn toolbar_progress(&self, cx: &App) -> Option<String> {
        self.progress_parts(cx)
            .map(|(status, elapsed)| match elapsed {
                Some(t) => format!("{status} · {t}"),
                None => status,
            })
    }

    /// Rows of the open profile dropdown, sorted by name like the sidebar.
    pub fn profile_menu(&self) -> Option<Vec<MenuRow>> {
        self.menu_open.then(|| {
            self.rows()
                .into_iter()
                .map(|r| MenuRow {
                    current: self.selected.as_ref().is_some_and(|s| s.as_str() == r.name),
                    name: r.name,
                    kind: r.kind,
                })
                .collect()
        })
    }

    pub fn toggle_profile_menu(&mut self, cx: &mut Context<Self>) {
        self.menu_open = !self.menu_open;
        self.menu_focus_on_open = self.menu_open;
        let current = self
            .rows()
            .iter()
            .position(|r| self.selected.as_ref().is_some_and(|s| s.as_str() == r.name));
        self.menu_highlight = Some(current.unwrap_or(0));
        cx.notify();
    }

    /// Escape closes the open dropdown, up/down move the highlight, Enter picks it.
    fn profile_menu_key(&mut self, key: &str, cx: &mut Context<Self>) {
        let rows = self.rows();
        // One past the last profile is 새 프로필….
        let at = self.menu_highlight.unwrap_or(0).min(rows.len());
        match key {
            "escape" => self.close_profile_menu(cx),
            "up" => self.menu_highlight = Some(at.saturating_sub(1)),
            "down" => self.menu_highlight = Some((at + 1).min(rows.len())),
            "enter" => match rows.get(at) {
                Some(row) => self.pick_profile(&row.name, cx),
                None => self.run_command(&AppCommand::NewProfile, cx),
            },
            _ => return,
        }
        cx.notify();
    }

    fn close_profile_menu(&mut self, cx: &mut Context<Self>) {
        self.menu_open = false;
        cx.notify();
    }

    /// Selects a dropdown row and closes the menu.
    pub fn pick_profile(&mut self, name: &str, cx: &mut Context<Self>) {
        self.menu_open = false;
        self.select(name, cx);
        cx.notify();
    }

    /// Runs a toolbar (or tray) command. Lifecycle commands select their profile first.
    pub fn run_command(&mut self, cmd: &AppCommand, cx: &mut Context<Self>) {
        self.menu_open = false;
        let lifecycle = match cmd {
            AppCommand::Start(n) => Some((n, Lifecycle::Start)),
            AppCommand::Stop(n) => Some((n, Lifecycle::Stop)),
            AppCommand::Restart(n) => Some((n, Lifecycle::Restart)),
            AppCommand::Refresh => {
                self.refresh();
                None
            }
            AppCommand::NewProfile => {
                self.new_profile(cx);
                None
            }
            AppCommand::OpenWindow | AppCommand::Quit => None,
        };
        if let Some((name, lc)) = lifecycle {
            if self.selected.as_ref() != Some(name) {
                self.select(name.as_str(), cx);
            }
            if self.selected.as_ref() == Some(name) {
                self.run(lc, cx);
            }
        }
        cx.notify();
    }

    /// Main-column toolbar (replaces the 프로필 › name title bar).
    pub(super) fn toolbar(&self, p: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let actions = self.toolbar_actions();
        let progress = self.progress_parts(cx).map(|(status, elapsed)| {
            h_flex()
                .id("toolbar-progress")
                .debug_selector(|| "toolbar-progress".to_string())
                .flex_none()
                .px(space(0))
                .gap(space(0))
                .whitespace_nowrap()
                .font_weight(FontWeight::MEDIUM)
                .text_color(hsla(p.busy))
                .child(status)
                .when_some(elapsed, |el, t| {
                    el.child("·").child(
                        div()
                            .font_family(font::MONO)
                            .text_size(px(font::CODE.size))
                            .child(t),
                    )
                })
        });
        let lifecycle: Vec<_> = actions
            .lifecycle
            .into_iter()
            .map(|item| {
                let variant = match item.command {
                    AppCommand::Start(_) => ButtonVariant::Primary,
                    _ => ButtonVariant::Default,
                };
                let el = button_with_icon(
                    command_id(&item.command),
                    lifecycle_icon(&item.command),
                    item.label,
                    variant,
                    p,
                );
                command_button(el, item.command, cx)
            })
            .collect();
        let refresh = ghost_button(actions.refresh, p, cx);
        let new_profile = ghost_button(actions.new_profile, p, cx);
        h_flex()
            .h(px(metric::TITLEBAR))
            .flex_none()
            .pl(space(2))
            .pr(space(1))
            .gap(space(1))
            .bg(hsla(p.bg))
            .text_size(px(font::BODY.size))
            .children(self.profile_dropdown(p, cx))
            .child(drag_area())
            .children(lifecycle)
            .children(progress)
            .child(
                div()
                    .debug_selector(|| "toolbar-divider".to_string())
                    .flex_none()
                    .h(px(metric::DIVIDER_H))
                    .mx(space(0))
                    .border_l_1()
                    .border_color(hsla(p.border)),
            )
            .child(refresh)
            .child(new_profile)
    }

    /// Current profile (status, name, chevron) and, when open, its menu (canvas 08).
    fn profile_dropdown(&self, p: &Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let name = self.selected.as_ref()?;
        let row = self.rows().into_iter().find(|r| r.name == name.as_str())?;
        let open = self.menu_open;
        let button = h_flex()
            .id("toolbar-profile")
            .debug_selector(|| "toolbar-profile".to_string())
            .h(px(metric::BTN))
            .max_w(px(metric::MENU_W))
            .px(space(1))
            .gap(space(0))
            .rounded(px(metric::RADIUS_MD))
            .cursor_pointer()
            .font_weight(FontWeight::MEDIUM)
            .text_color(hsla(p.text))
            .when(open, |d| d.bg(hsla(p.bg_hover)))
            .hover(|s| s.bg(hsla(p.bg_hover)))
            .active(|s| s.bg(hsla(p.bg_pressed)))
            .tooltip(text_tip(profile_picker_label(&row.name, row.kind).into()))
            .on_click(cx.listener(|v, _, _, cx| v.toggle_profile_menu(cx)))
            .child(status_shape(
                "toolbar-profile-status",
                status_of(row.kind),
                p,
            ))
            .child(div().min_w_0().truncate().child(row.name))
            .child(icon(Icon::Chevron, hsla(p.text_2)));
        let menu = self.profile_menu().map(|rows| {
            let highlight = self.profile_menu_highlight();
            let last = rows.len();
            let items: Vec<_> = rows
                .into_iter()
                .enumerate()
                .map(|(i, r)| self.menu_row(r, highlight == Some(i), p, cx))
                .collect();
            let new_profile = menu_item(SharedString::from("profile-menu-new"), p)
                .when(highlight == Some(last), |d| d.bg(hsla(menu_hover(p))))
                .on_click(cx.listener(|v, _, _, cx| {
                    v.run_command(&AppCommand::NewProfile, cx);
                }))
                .child(check_slot(false, p))
                .child(icon(Icon::Plus, hsla(p.text_2)))
                .child(div().flex_1().child(NEW_PROFILE_MENU));
            deferred(
                v_flex()
                    .id("profile-menu")
                    .track_focus(&self.menu_focus)
                    .on_key_down(cx.listener(|v, ev: &KeyDownEvent, _, cx| {
                        v.profile_menu_key(ev.keystroke.key.as_str(), cx);
                    }))
                    .occlude()
                    .absolute()
                    .top(px(metric::BTN + metric::SPACE[0]))
                    .left_0()
                    .w(px(metric::MENU_W))
                    .p(space(0))
                    .gap(half_step())
                    .bg(hsla(p.bg_raised))
                    .border_1()
                    .border_color(hsla(p.border_strong))
                    .rounded(px(metric::RADIUS_LG))
                    .shadow_lg()
                    .children(items)
                    .child(
                        div()
                            .my(half_step())
                            .border_t_1()
                            .border_color(hsla(p.border)),
                    )
                    .child(new_profile),
            )
            .with_priority(1)
        });
        Some(
            div()
                .relative()
                .flex_none()
                .min_w_0()
                .child(button)
                .children(menu)
                .into_any_element(),
        )
    }

    fn menu_row(
        &self,
        r: MenuRow,
        highlighted: bool,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let status = status_of(r.kind);
        let status_color = match r.kind {
            StatusKind::Stopped => p.text_2,
            _ => status.color(p),
        };
        let name = r.name.clone();
        menu_item(SharedString::from(format!("profile-menu-{}", r.name)), p)
            .when(highlighted, |d| d.bg(hsla(menu_hover(p))))
            .on_click(cx.listener(move |v, _, _, cx| v.pick_profile(&name, cx)))
            .child(check_slot(r.current, p))
            .child(status_shape(
                SharedString::from(format!("profile-menu-status-{}", r.name)),
                status,
                p,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(r.current, |d| d.font_weight(FontWeight::MEDIUM))
                    .child(r.name),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(font::SMALL.size))
                    .text_color(hsla(status_color))
                    .child(r.kind.label()),
            )
    }

    /// Transparent full-window layer under the open menu: a click anywhere else closes it.
    pub(super) fn menu_backdrop(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        self.menu_open.then(|| {
            div()
                .id("profile-menu-backdrop")
                .absolute()
                .inset_0()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|v, _, _, cx| v.close_profile_menu(cx)),
                )
        })
    }
}

/// Wires a toolbar button to its command, with a debug selector named like its id.
fn command_button(el: Stateful<Div>, cmd: AppCommand, cx: &mut Context<MainView>) -> Stateful<Div> {
    let id = command_id(&cmd);
    el.debug_selector(move || id.to_string())
        .on_click(cx.listener(move |v, _, _, cx| v.run_command(&cmd, cx)))
}

/// Square ghost icon button (refresh, new profile); its label is the tooltip.
fn ghost_button(item: CommandItem, p: &Palette, cx: &mut Context<MainView>) -> Stateful<Div> {
    let glyph = icon(lifecycle_icon(&item.command), hsla(p.text_2));
    command_button(
        icon_button(command_id(&item.command), glyph, p),
        item.command,
        cx,
    )
    .tooltip(text_tip(item.label.into()))
}

/// Empty toolbar space: drags the window, double-click zooms (macOS titlebar behaviour).
fn drag_area() -> Stateful<Div> {
    div()
        .id("toolbar-drag")
        .debug_selector(|| "toolbar-drag".to_string())
        .flex_1()
        .h_full()
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_move(|ev: &MouseMoveEvent, window: &mut Window, _| {
            // macOS drags a transparent titlebar natively; other platforms move on request.
            if !cfg!(target_os = "macos") && ev.pressed_button == Some(MouseButton::Left) {
                window.start_window_move();
            }
        })
        .on_click(|ev: &ClickEvent, window: &mut Window, _| {
            if ev.click_count() == 2 {
                if cfg!(target_os = "macos") {
                    window.titlebar_double_click();
                } else {
                    window.zoom_window();
                }
            }
        })
}

/// Dropdown row frame (CSS .row inside the menu).
fn menu_item(id: SharedString, p: &Palette) -> Stateful<Div> {
    let debug = id.to_string();
    h_flex()
        .id(id)
        .debug_selector(move || debug.clone())
        .h(px(metric::ROW))
        .px(space(1))
        .gap(space(1))
        .rounded(px(metric::RADIUS_MD))
        .cursor_pointer()
        .text_size(px(font::BODY.size))
        .text_color(hsla(p.text))
        .hover(|s| s.bg(hsla(menu_hover(p))))
        .active(|s| s.bg(hsla(p.bg_pressed)))
}

/// Hover and keyboard-highlight colour of a dropdown row. The menu sits on `bg_raised`;
/// where that equals `bg_hover` (dark theme) the row would not change, so use `bg_pressed`.
fn menu_hover(p: &Palette) -> Rgb {
    if p.bg_hover == p.bg_raised {
        p.bg_pressed
    } else {
        p.bg_hover
    }
}

/// Icon-sized slot holding the check mark of the current profile.
fn check_slot(checked: bool, p: &Palette) -> Div {
    div()
        .flex_none()
        .size(px(metric::ICON))
        .when(checked, |d| d.child(icon(Icon::Check, hsla(p.text))))
}

#[cfg(test)]
mod menu_hover_tests {
    use super::menu_hover;
    use crate::theme::{Appearance, palette};

    #[test]
    fn dropdown_row_highlight_differs_from_menu_background_in_both_themes() {
        for a in [Appearance::Light, Appearance::Dark] {
            let p = palette(a);
            assert_ne!(menu_hover(p), p.bg_raised, "{a:?}");
        }
    }
}
