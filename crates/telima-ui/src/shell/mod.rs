//! Main window shell: title bar, profile sidebar, profile detail, engine tabs.
//!
//! Use-case callbacks run on the spawner's thread, so they only push a [`Msg`] into the
//! inbox; [`MainView::pump`] applies them on the GPUI thread (a poll task calls it).
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use telima_app::AppError;
use telima_app::profiles::Lifecycle;
use telima_app::runtime::App as Telima;
use telima_app::view::{
    ProfileDetail, ProfileRow, StatusKind, confirm_delete_profile, profile_detail, profile_rows,
};
use telima_core::profile::{Profile, ProfileName, ProfileSpec};

use crate::engine_views::EnginePanel;
use crate::profile_form::{
    DeleteConfirm, DeleteEvent, FormEvent, ProfileEditor, ProgressEnd, StartProgress,
};
use crate::style::{hsla, palette, space};
use crate::theme::{Palette, font, metric};

/// How often the view drains finished use-case results.
const POLL: Duration = Duration::from_millis(50);

/// Result of a use case, sent from the spawner thread to the view.
enum Msg {
    Profiles(Result<Vec<Profile>, AppError>),
    /// Stored spec for the edit form.
    Spec(ProfileName, Result<ProfileSpec, AppError>),
    /// What a delete would remove: container count, volume count, volume bytes.
    DeleteFacts(ProfileName, usize, usize, u64),
}

type Inbox = Arc<Mutex<Vec<Msg>>>;

/// Tabs under the profile detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Containers,
    Images,
    Volumes,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Containers, Tab::Images, Tab::Volumes];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Containers => "컨테이너",
            Tab::Images => "이미지",
            Tab::Volumes => "볼륨",
        }
    }
}

/// Root view of the main window once wired with the [`Telima`] facade.
pub struct MainView {
    app: Telima,
    inbox: Inbox,
    profiles: Vec<Profile>,
    selected: Option<ProfileName>,
    /// Profiles with a lifecycle command sent but not finished.
    pending: HashSet<ProfileName>,
    error: Option<String>,
    tab: Tab,
    engine: Entity<EnginePanel>,
    /// Create/edit form; replaces the tab body while open.
    editor: Option<(Entity<ProfileEditor>, Subscription)>,
    /// Log of the last start/stop/restart per profile; the selected one's is shown.
    progress: HashMap<ProfileName, (Entity<StartProgress>, Subscription)>,
    /// Typed-name delete confirmation, drawn over the window.
    delete_confirm: Option<(Entity<DeleteConfirm>, Subscription)>,
    _poll: Task<()>,
}

impl MainView {
    pub fn new(app: Telima, cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                if this.update(cx, |view, cx| view.pump(cx)).is_err() {
                    break;
                }
            }
        });
        let mut view = Self {
            app: app.clone(),
            inbox: Inbox::default(),
            profiles: Vec::new(),
            selected: None,
            pending: HashSet::new(),
            error: None,
            tab: Tab::Containers,
            engine: cx.new(|cx| EnginePanel::new(app.clone(), cx)),
            editor: None,
            progress: HashMap::new(),
            delete_confirm: None,
            _poll: poll,
        };
        view.refresh();
        view
    }

    /// Facade shared with child views.
    pub fn app(&self) -> &Telima {
        &self.app
    }

    /// Currently selected profile.
    pub fn selected(&self) -> Option<&ProfileName> {
        self.selected.as_ref()
    }

    pub fn tab(&self) -> Tab {
        self.tab
    }

    /// Containers / images / volumes of the selected profile.
    pub fn engine(&self) -> &Entity<EnginePanel> {
        &self.engine
    }

    /// Open create/edit form.
    pub fn editor(&self) -> Option<&Entity<ProfileEditor>> {
        self.editor.as_ref().map(|(e, _)| e)
    }

    /// Log of the selected profile's last lifecycle command.
    pub fn progress(&self) -> Option<&Entity<StartProgress>> {
        self.progress.get(self.selected.as_ref()?).map(|(e, _)| e)
    }

    /// Open delete confirmation.
    pub fn delete_confirm(&self) -> Option<&Entity<DeleteConfirm>> {
        self.delete_confirm.as_ref().map(|(e, _)| e)
    }

    /// Reloads the profile list.
    pub fn refresh(&mut self) {
        let inbox = self.inbox.clone();
        self.app
            .list_profiles(move |r| inbox.lock().unwrap().push(Msg::Profiles(r)));
    }

    /// Applies finished use-case results, its own and the child views'. Called by the poll
    /// task (and by tests). Child views also poll themselves; pumping twice is harmless.
    pub fn pump(&mut self, cx: &mut Context<Self>) {
        self.engine.update(cx, |e, cx| e.pump(cx));
        for (p, _) in self.progress.values() {
            p.update(cx, |p, cx| p.pump(cx));
        }
        if let Some((e, _)) = &self.editor {
            e.update(cx, |e, cx| e.pump(cx));
        }
        if let Some((d, _)) = &self.delete_confirm {
            d.update(cx, |d, cx| d.pump(cx));
        }
        let msgs = std::mem::take(&mut *self.inbox.lock().unwrap());
        if msgs.is_empty() {
            return;
        }
        for msg in msgs {
            match msg {
                Msg::Profiles(Ok(mut list)) => {
                    list.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
                    let keep = self
                        .selected
                        .as_ref()
                        .is_some_and(|s| list.iter().any(|p| &p.name == s));
                    if !keep {
                        self.selected = list.first().map(|p| p.name.clone());
                    }
                    self.profiles = list;
                    self.sync_engine(cx);
                }
                Msg::Profiles(Err(e)) => self.error = Some(e.to_string()),
                Msg::Spec(name, Ok(spec)) => {
                    if self.selected.as_ref() == Some(&name) {
                        let (app, os) = (self.app.clone(), self.app.host_os());
                        let editor = cx.new(|cx| ProfileEditor::edit(app, os, spec, cx));
                        self.open_editor(editor, cx);
                    }
                }
                Msg::Spec(_, Err(e)) => self.error = Some(e.to_string()),
                Msg::DeleteFacts(name, containers, volumes, bytes) => {
                    if self.selected.as_ref() == Some(&name) && self.delete_confirm.is_none() {
                        let copy = confirm_delete_profile(&name, containers, volumes, bytes);
                        let app = self.app.clone();
                        let confirm = cx.new(|cx| DeleteConfirm::new(app, name, copy, cx));
                        let sub = cx.subscribe(&confirm, |v, _, ev: &DeleteEvent, cx| {
                            v.delete_confirm = None;
                            if let DeleteEvent::Deleted(_) = ev {
                                v.refresh();
                            }
                            cx.notify();
                        });
                        self.delete_confirm = Some((confirm, sub));
                    }
                }
            }
        }
        cx.notify();
    }

    /// Points the engine panel at the selected profile and tab.
    fn sync_engine(&mut self, cx: &mut Context<Self>) {
        let (profile, tab) = (self.selected.clone(), self.tab);
        self.engine
            .update(cx, |e, cx| e.set_target(profile, tab, cx));
    }

    fn open_editor(&mut self, editor: Entity<ProfileEditor>, cx: &mut Context<Self>) {
        let sub = cx.subscribe(&editor, |v, _, ev: &FormEvent, cx| {
            v.editor = None;
            if let FormEvent::Saved(name) = ev {
                v.selected = Some(name.clone());
                v.refresh();
            }
            cx.notify();
        });
        self.editor = Some((editor, sub));
        cx.notify();
    }

    fn busy(&self, name: &ProfileName) -> bool {
        self.pending.contains(name) || self.app.is_busy(name)
    }

    /// Sidebar rows, sorted by name.
    pub fn rows(&self) -> Vec<ProfileRow> {
        profile_rows(&self.profiles, |n| self.busy(n))
    }

    /// Detail of the selected profile.
    pub fn detail(&self) -> Option<ProfileDetail> {
        let name = self.selected.as_ref()?;
        let p = self.profiles.iter().find(|p| &p.name == name)?;
        Some(profile_detail(p, None, self.busy(name)))
    }

    pub fn select(&mut self, name: &str, cx: &mut Context<Self>) {
        if let Some(p) = self.profiles.iter().find(|p| p.name.as_str() == name) {
            self.selected = Some(p.name.clone());
            self.editor = None;
            self.sync_engine(cx);
            cx.notify();
        }
    }

    pub fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        self.sync_engine(cx);
        cx.notify();
    }

    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.run(Lifecycle::Start, cx);
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.run(Lifecycle::Stop, cx);
    }

    pub fn restart(&mut self, cx: &mut Context<Self>) {
        self.run(Lifecycle::Restart, cx);
    }

    /// Fetches the stored spec of the selected profile, then opens the edit form.
    pub fn edit(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        if self.busy(&name) {
            return;
        }
        self.error = None;
        let inbox = self.inbox.clone();
        let key = name.clone();
        self.app
            .profile_spec(name, move |r| inbox.lock().unwrap().push(Msg::Spec(key, r)));
        cx.notify();
    }

    /// Opens the create form.
    pub fn new_profile(&mut self, cx: &mut Context<Self>) {
        let (app, os) = (self.app.clone(), self.app.host_os());
        let editor = cx.new(|cx| ProfileEditor::create(app, os, cx));
        self.open_editor(editor, cx);
    }

    /// Counts what the selected profile's delete would remove, then opens the typed-name
    /// confirmation. A stopped VM has no reachable engine: its counts show as 0.
    pub fn delete(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        if self.busy(&name) || self.delete_confirm.is_some() {
            return;
        }
        let inbox = self.inbox.clone();
        let app = self.app.clone();
        let key = name.clone();
        self.app.containers(name, move |containers| {
            let containers = containers.map(|c| c.len()).unwrap_or(0);
            app.volumes(key.clone(), move |volumes| {
                let (count, bytes) = volumes
                    .map(|v| (v.len(), v.iter().filter_map(|v| v.size_bytes).sum()))
                    .unwrap_or((0, 0));
                inbox
                    .lock()
                    .unwrap()
                    .push(Msg::DeleteFacts(key, containers, count, bytes));
            });
        });
        cx.notify();
    }

    /// Hides an ended progress log.
    pub fn close_progress(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let ended = self
            .progress
            .get(&name)
            .is_some_and(|(p, _)| !p.read(cx).is_running());
        if ended {
            self.progress.remove(&name);
            cx.notify();
        }
    }

    fn run(&mut self, cmd: Lifecycle, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        if self.busy(&name) {
            return;
        }
        self.pending.insert(name.clone());
        self.error = None;
        self.editor = None;
        let app = self.app.clone();
        let key = name.clone();
        let target = name.clone();
        let progress = cx.new(|cx| StartProgress::new(app, target, cmd, cx));
        let sub = cx.subscribe(&progress, move |v, _, ev: &ProgressEnd, cx| {
            v.pending.remove(&key);
            // A clean finish needs no log; failures and cancels stay until closed.
            if *ev == ProgressEnd::Done {
                v.progress.remove(&key);
            }
            v.refresh();
            cx.notify();
        });
        self.progress.insert(name, (progress, sub));
        cx.notify();
    }
}

/// Status shape: filled dot running, hollow ring stopped, arc busy, triangle error.
fn status_shape(kind: StatusKind, p: &Palette) -> Div {
    let size = px(8.);
    let base = div().size(size).flex_none();
    match kind {
        StatusKind::Running => base.rounded_full().bg(hsla(p.ok)),
        StatusKind::Stopped => base.rounded_full().border_1().border_color(hsla(p.idle)),
        StatusKind::Busy => base
            .rounded_full()
            .border_1()
            .border_color(hsla(p.track))
            .border_t_1()
            .border_r_1(),
        StatusKind::Error => div()
            .flex_none()
            .text_size(size)
            .line_height(size)
            .text_color(hsla(p.err))
            .child("▲"),
    }
}

fn button(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    primary: bool,
    p: &Palette,
) -> Stateful<Div> {
    let (bg, fg) = match (enabled, primary) {
        (false, _) => (p.bg_chrome, p.text_disabled),
        (true, true) => (p.accent, p.on_accent),
        (true, false) => (p.bg_raised, p.text),
    };
    let el = div()
        .id(id)
        .h(px(metric::ROW))
        .px(space(2))
        .flex()
        .items_center()
        .rounded(px(metric::RADIUS_MD))
        .border_1()
        .border_color(hsla(p.border))
        .bg(hsla(bg))
        .text_color(hsla(fg))
        .text_size(px(font::BODY.size))
        .child(label);
    if enabled {
        let hover = if primary { p.accent_text } else { p.bg_hover };
        el.cursor_pointer().hover(move |s| s.bg(hsla(hover)))
    } else {
        el
    }
}

impl MainView {
    fn sidebar(&self, p: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected.clone();
        let rows = self.rows().into_iter().map(|row| {
            let is_sel = selected.as_ref().is_some_and(|s| s.as_str() == row.name);
            let name = row.name.clone();
            h_flex()
                .id(SharedString::from(format!("profile-{}", row.name)))
                .h(px(metric::ROW))
                .px(space(2))
                .gap(space(1))
                .rounded(px(metric::RADIUS_SM))
                .when(is_sel, |d| d.bg(hsla(p.bg_selected)))
                .when(!is_sel, |d| d.hover(|s| s.bg(hsla(p.bg_hover))))
                .cursor_pointer()
                .on_click(cx.listener(move |v, _, _, cx| v.select(&name, cx)))
                .child(status_shape(row.kind, p))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(font::BODY.size))
                        .text_color(hsla(p.text))
                        .child(row.name),
                )
                .child(
                    div()
                        .text_size(px(font::SMALL.size))
                        .font_family(font::MONO)
                        .text_color(hsla(p.text_2))
                        .child(row.resource_line),
                )
        });
        v_flex()
            .w(px(metric::SIDEBAR))
            .flex_none()
            .h_full()
            .p(space(1))
            .gap(px(2.))
            .bg(hsla(p.bg_chrome))
            .border_r_1()
            .border_color(hsla(p.border))
            .child(
                h_flex()
                    .px(space(2))
                    .py(space(1))
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(font::SMALL.size))
                            .text_color(hsla(p.text_2))
                            .child("프로필"),
                    )
                    .child(
                        button("new-profile", "새 프로필", true, false, p)
                            .on_click(cx.listener(|v, _, _, cx| v.new_profile(cx))),
                    ),
            )
            .children(rows)
    }

    fn detail_pane(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(d) = self.detail() else {
            let msg = if self.profiles.is_empty() {
                "프로필이 없습니다."
            } else {
                "프로필을 선택하세요."
            };
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(hsla(p.text_2))
                .child(msg)
                .into_any_element();
        };
        let a = d.actions;
        let fact = |label: &'static str, value: String| {
            v_flex()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(font::SMALL.size))
                        .text_color(hsla(p.text_2))
                        .child(label),
                )
                .child(
                    div()
                        .font_family(font::MONO)
                        .text_size(px(font::CODE.size))
                        .text_color(hsla(p.text))
                        .child(value),
                )
        };
        let tabs = Tab::ALL.into_iter().map(|t| {
            let active = t == self.tab;
            div()
                .id(t.label())
                .px(space(2))
                .h(px(metric::ROW))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_size(px(font::BODY.size))
                .text_color(hsla(if active { p.text } else { p.text_2 }))
                .when(active, |d| d.border_b_2().border_color(hsla(p.accent)))
                .on_click(cx.listener(move |v, _, _, cx| v.set_tab(t, cx)))
                .child(t.label())
        });
        let body: AnyElement = match self.editor() {
            Some(editor) => editor.clone().into_any_element(),
            None => self.engine.clone().into_any_element(),
        };
        let progress = self.progress().cloned().map(|log| {
            let ended = !log.read(cx).is_running();
            v_flex()
                .flex_none()
                .max_h(px(metric::ROW * 12.))
                .border_b_1()
                .border_color(hsla(p.border))
                .when(ended, |el| {
                    el.child(
                        h_flex().px(space(3)).pt(space(1)).justify_end().child(
                            button("close-progress", "닫기", true, false, p)
                                .on_click(cx.listener(|v, _, _, cx| v.close_progress(cx))),
                        ),
                    )
                })
                .child(log)
        });
        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(hsla(p.bg))
            .child(
                v_flex()
                    .p(space(4))
                    .gap(space(3))
                    .border_b_1()
                    .border_color(hsla(p.border))
                    .child(
                        h_flex()
                            .gap(space(1))
                            .child(status_shape(d.kind, p))
                            .child(
                                div()
                                    .text_size(px(font::TITLE.size))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(hsla(p.text))
                                    .child(d.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(font::SMALL.size))
                                    .text_color(hsla(p.text_2))
                                    .child(d.status_label),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap(space(5))
                            .child(fact("CPU", d.cpus.to_string()))
                            .child(fact("메모리", d.memory.clone()))
                            .child(fact("디스크", d.disk.clone()))
                            .child(fact("런타임", d.runtime.clone()))
                            .child(fact("아키텍처", d.arch.clone()))
                            .child(fact("주소", d.address.clone())),
                    )
                    .child(
                        h_flex()
                            .gap(space(1))
                            .child(
                                button("start", "시작", a.start, true, p).when(a.start, |b| {
                                    b.on_click(cx.listener(|v, _, _, cx| v.start(cx)))
                                }),
                            )
                            .child(button("stop", "중지", a.stop, false, p).when(a.stop, |b| {
                                b.on_click(cx.listener(|v, _, _, cx| v.stop(cx)))
                            }))
                            .child(
                                button("restart", "재시작", a.restart, false, p)
                                    .when(a.restart, |b| {
                                        b.on_click(cx.listener(|v, _, _, cx| v.restart(cx)))
                                    }),
                            )
                            .child(button("edit", "편집", a.edit, false, p).when(a.edit, |b| {
                                b.on_click(cx.listener(|v, _, _, cx| v.edit(cx)))
                            }))
                            .child(
                                button("delete", "삭제", a.delete, false, p).when(a.delete, |b| {
                                    b.on_click(cx.listener(|v, _, _, cx| v.delete(cx)))
                                }),
                            ),
                    )
                    .when_some(self.error.clone(), |el, e| {
                        el.child(
                            div()
                                .p(space(1))
                                .rounded(px(metric::RADIUS_SM))
                                .bg(hsla(p.err_bg))
                                .border_1()
                                .border_color(hsla(p.err_border))
                                .text_color(hsla(p.err))
                                .text_size(px(font::SMALL.size))
                                .child(e),
                        )
                    }),
            )
            .children(progress)
            .when(self.editor.is_none(), |el| {
                el.child(
                    h_flex()
                        .px(space(3))
                        .border_b_1()
                        .border_color(hsla(p.border))
                        .children(tabs),
                )
            })
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(window);
        let overlay = self.delete_confirm().cloned().map(|confirm| {
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(hsla(p.bg_chrome).opacity(0.6))
                .child(div().id("delete-confirm").occlude().child(confirm))
        });
        v_flex()
            .relative()
            .size_full()
            .bg(hsla(p.bg))
            .font_family(font::SANS)
            .text_color(hsla(p.text))
            .child(
                h_flex()
                    .h(px(metric::TITLEBAR))
                    .flex_none()
                    .px(space(4))
                    .bg(hsla(p.bg_chrome))
                    .border_b_1()
                    .border_color(hsla(p.border))
                    .justify_center()
                    .text_size(px(font::BODY.size))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(crate::APP_NAME),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .child(self.sidebar(p, cx))
                    .child(self.detail_pane(p, cx)),
            )
            .children(overlay)
    }
}

#[cfg(test)]
mod tests;
