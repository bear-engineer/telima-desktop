//! Main window shell: title bar, profile sidebar, profile detail, engine tabs.
//!
//! Use-case callbacks run on the spawner's thread, so they only push a [`Msg`] into the
//! inbox; [`MainView::pump`] applies them on the GPUI thread (a poll task calls it).
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use telima_app::AppError;
use telima_app::profiles::Lifecycle;
use telima_app::runtime::App as Telima;
use telima_app::view::notice::{NoticeKind, error_text, notice};
use telima_app::view::{
    ProfileDetail, ProfileRow, StatusKind, confirm_delete_profile, profile_detail, profile_rows,
};
use telima_core::profile::{Profile, ProfileName, ProfileSpec};

use crate::components::button::{ButtonVariant, button, icon_button};
use crate::components::chip::status_chip;
use crate::components::dialog::{fade_in, scrim};
use crate::components::skeleton::skeleton_row;
use crate::components::status::{Status, status_shape};
use crate::engine_views::{EnginePanel, ListState};
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
    /// `None` until the first `list_profiles` answer (loading skeleton).
    profiles: Option<Vec<Profile>>,
    /// Set when the profile list failed because Colima is missing.
    not_installed: Option<(String, Option<&'static str>)>,
    /// The open editor is the create form (it replaces the header).
    creating: bool,
    /// When each pending lifecycle command was sent (busy row elapsed time).
    started: HashMap<ProfileName, Instant>,
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
            profiles: None,
            not_installed: None,
            creating: false,
            started: HashMap::new(),
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
                    self.profiles = Some(list);
                    self.not_installed = None;
                    self.sync_engine(cx);
                }
                Msg::Profiles(Err(e)) => {
                    let n = notice(&e);
                    if n.kind == NoticeKind::NotInstalled {
                        self.not_installed = Some((n.message, n.hint));
                    } else {
                        self.error = Some(n.message);
                    }
                }
                Msg::Spec(name, Ok(spec)) => {
                    if self.selected.as_ref() == Some(&name) {
                        let (app, os) = (self.app.clone(), self.app.host_os());
                        let editor = cx.new(|cx| ProfileEditor::edit(app, os, spec, cx));
                        self.open_editor(editor, cx);
                    }
                }
                Msg::Spec(_, Err(e)) => self.error = Some(error_text(&e)),
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
            v.creating = false;
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
        profile_rows(self.list(), |n| self.busy(n))
    }

    /// Detail of the selected profile.
    pub fn detail(&self) -> Option<ProfileDetail> {
        let name = self.selected.as_ref()?;
        let p = self.list().iter().find(|p| &p.name == name)?;
        Some(profile_detail(p, None, self.busy(name)))
    }

    pub fn select(&mut self, name: &str, cx: &mut Context<Self>) {
        if let Some(p) = self.list().iter().find(|p| p.name.as_str() == name) {
            self.selected = Some(p.name.clone());
            self.editor = None;
            self.creating = false;
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
        self.creating = true;
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
        self.started.insert(name.clone(), Instant::now());
        self.error = None;
        self.editor = None;
        self.creating = false;
        let app = self.app.clone();
        let key = name.clone();
        let target = name.clone();
        let progress = cx.new(|cx| StartProgress::new(app, target, cmd, cx));
        let sub = cx.subscribe(&progress, move |v, _, ev: &ProgressEnd, cx| {
            v.pending.remove(&key);
            v.started.remove(&key);
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

/// What the window shows (one state at a time).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    /// First profile list not answered yet: skeleton rows.
    Loading,
    /// Colima is missing: Korean message and an install hint.
    NotInstalled {
        message: String,
        hint: Option<&'static str>,
    },
    /// No profiles: empty state with a 새 프로필 button.
    Empty,
    /// The selected profile: header, tabs, inspector.
    Profile,
    /// The create form replaces the header; no profile actions.
    Create,
}

/// Lifecycle buttons in the profile header (only the ones valid for the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderAction {
    Start,
    Restart,
    Stop,
}

impl HeaderAction {
    fn label(self) -> &'static str {
        match self {
            HeaderAction::Start => "시작",
            HeaderAction::Restart => "재시작",
            HeaderAction::Stop => "중지",
        }
    }
}

/// The 구성 inspector of the selected profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspector {
    /// Label and value, in canvas order.
    pub rows: Vec<(&'static str, String)>,
    pub socket: String,
    pub can_edit: bool,
    pub can_delete: bool,
}

/// Docker socket of a profile, as shown to the user (`~/.colima/<profile>/docker.sock`).
fn docker_socket(profile: &str) -> String {
    format!("~/.colima/{profile}/docker.sock")
}

/// Status shape and chip for a view status.
fn status_of(kind: StatusKind) -> Status {
    match kind {
        StatusKind::Running => Status::Running,
        StatusKind::Stopped => Status::Stopped,
        StatusKind::Busy => Status::Busy,
        StatusKind::Error => Status::Error,
    }
}

/// `mm:ss` since a lifecycle command was sent.
fn elapsed_text(since: Instant) -> String {
    let s = since.elapsed().as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}

impl MainView {
    fn list(&self) -> &[Profile] {
        self.profiles.as_deref().unwrap_or(&[])
    }

    /// What the window shows now.
    pub fn screen(&self) -> Screen {
        if let Some((message, hint)) = &self.not_installed {
            return Screen::NotInstalled {
                message: message.clone(),
                hint: *hint,
            };
        }
        if self.creating {
            return Screen::Create;
        }
        match &self.profiles {
            None => Screen::Loading,
            Some(list) if list.is_empty() => Screen::Empty,
            Some(_) => Screen::Profile,
        }
    }

    /// Korean error banner text, if any.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Breadcrumb after 프로필 › in the main title bar.
    pub fn crumb(&self) -> Option<String> {
        match self.screen() {
            Screen::Create => Some("새 프로필".to_owned()),
            Screen::Profile => self.selected.as_ref().map(|n| n.to_string()),
            _ => None,
        }
    }

    /// Header buttons valid for the selected profile's state, in canvas order.
    pub fn header_actions(&self) -> Vec<HeaderAction> {
        if self.screen() != Screen::Profile {
            return Vec::new();
        }
        let Some(a) = self.detail().map(|d| d.actions) else {
            return Vec::new();
        };
        [
            (a.start, HeaderAction::Start),
            (a.restart, HeaderAction::Restart),
            (a.stop, HeaderAction::Stop),
        ]
        .into_iter()
        .filter_map(|(ok, act)| ok.then_some(act))
        .collect()
    }

    /// 구성 inspector rows of the selected profile.
    pub fn inspector(&self) -> Option<Inspector> {
        let d = self.detail()?;
        Some(Inspector {
            socket: docker_socket(&d.name),
            rows: vec![
                ("런타임", d.runtime),
                ("VM 종류", d.vm_type),
                ("마운트", d.mount_type),
                ("아키텍처", d.arch),
                ("CPU", d.cpus.to_string()),
                ("메모리", d.memory),
                ("디스크", d.disk),
                ("IP 주소", d.address),
            ],
            can_edit: d.actions.edit,
            can_delete: d.actions.delete,
        })
    }

    /// Item count shown next to a tab label, once its list has loaded.
    pub fn tab_count(&self, tab: Tab, cx: &App) -> Option<usize> {
        fn len<T>(s: &ListState<T>) -> Option<usize> {
            match s {
                ListState::Ready(rows) => Some(rows.len()),
                _ => None,
            }
        }
        let e = self.engine.read(cx);
        match tab {
            Tab::Containers => len(e.containers()),
            Tab::Images => len(e.images()),
            Tab::Volumes => len(e.volumes()),
        }
    }

    fn act(&mut self, action: HeaderAction, cx: &mut Context<Self>) {
        match action {
            HeaderAction::Start => self.start(cx),
            HeaderAction::Restart => self.restart(cx),
            HeaderAction::Stop => self.stop(cx),
        }
    }
}

/// Small secondary text (labels, captions).
fn caption(text: impl Into<SharedString>, p: &Palette) -> Div {
    div()
        .text_size(px(font::SMALL.size))
        .text_color(hsla(p.text_2))
        .child(text.into())
}

/// Monospace value text.
fn mono(text: impl Into<SharedString>, color: crate::theme::Rgb) -> Div {
    div()
        .font_family(font::MONO)
        .text_size(px(font::CODE.size))
        .text_color(hsla(color))
        .child(text.into())
}

/// Section heading (13 px semibold).
fn heading(text: impl Into<SharedString>, p: &Palette) -> Div {
    div()
        .text_size(px(font::BODY.size))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(hsla(p.text))
        .child(text.into())
}

/// Half a spacing step: the 2 px gap between sidebar rows.
fn half_step() -> Pixels {
    px(metric::SPACE[0] / 2.0)
}

impl MainView {
    fn sidebar(&self, p: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected.clone();
        let loading = self.profiles.is_none() && self.not_installed.is_none();
        let rows = self.rows().into_iter().map(|row| {
            let is_sel = selected.as_ref().is_some_and(|s| s.as_str() == row.name);
            let name = row.name.clone();
            let busy_since = (row.kind == StatusKind::Busy)
                .then(|| self.started.iter().find(|(n, _)| n.as_str() == row.name))
                .flatten()
                .map(|(_, t)| *t);
            let trailing = match busy_since {
                Some(t) => mono(elapsed_text(t), p.busy),
                None if row.kind == StatusKind::Busy => mono(row.resource_line, p.busy),
                None => mono(row.resource_line, p.text_2),
            };
            h_flex()
                .id(SharedString::from(format!("profile-{}", row.name)))
                .h(px(metric::ROW))
                .px(space(1))
                .gap(space(1))
                .rounded(px(metric::RADIUS_MD))
                .when(is_sel, |d| d.bg(hsla(p.bg_selected)))
                .when(!is_sel, |d| {
                    d.hover(|s| s.bg(hsla(p.bg_hover)))
                        .active(|s| s.bg(hsla(p.bg_pressed)))
                })
                .cursor_pointer()
                .on_click(cx.listener(move |v, _, _, cx| v.select(&name, cx)))
                .child(status_shape(
                    SharedString::from(format!("profile-status-{}", row.name)),
                    status_of(row.kind),
                    p,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(font::BODY.size))
                        .text_color(hsla(p.text))
                        .when(is_sel, |d| d.font_weight(FontWeight::MEDIUM))
                        .child(row.name),
                )
                .child(trailing)
        });
        let skeleton = loading.then(|| {
            v_flex()
                .gap(half_step())
                .child(skeleton_row(&[0.6, 0.25], p))
                .child(skeleton_row(&[0.45, 0.25], p))
                .child(skeleton_row(&[0.55, 0.25], p))
        });
        v_flex()
            .w(px(metric::SIDEBAR))
            .flex_none()
            .h_full()
            .bg(hsla(p.bg_chrome))
            .border_r_1()
            .border_color(hsla(p.border))
            // Room for the traffic lights (transparent title bar).
            .child(div().h(px(metric::TITLEBAR)).flex_none())
            .child(
                v_flex()
                    .px(space(1))
                    .py(space(0))
                    .gap(half_step())
                    .child(
                        h_flex()
                            .h(px(metric::ROW))
                            .pl(space(1))
                            .justify_between()
                            .child(caption("프로필", p).font_weight(FontWeight::MEDIUM))
                            .child(
                                icon_button("new-profile", "+", p)
                                    .on_click(cx.listener(|v, _, _, cx| v.new_profile(cx))),
                            ),
                    )
                    .children(skeleton)
                    .children(rows),
            )
            .child(
                h_flex()
                    .mt_auto()
                    .p(space(1))
                    .pl(space(3))
                    .border_t_1()
                    .border_color(hsla(p.border))
                    .child(mono(
                        concat!("telima ", env!("CARGO_PKG_VERSION")),
                        p.text_2,
                    )),
            )
    }

    /// Main-column title bar: 프로필 › name (H2).
    fn titlebar(&self, p: &Palette) -> impl IntoElement {
        h_flex()
            .h(px(metric::TITLEBAR))
            .flex_none()
            .pl(space(4))
            .pr(space(1))
            .gap(space(1))
            .bg(hsla(p.bg))
            .text_size(px(font::BODY.size))
            .child(div().text_color(hsla(p.text_2)).child("프로필"))
            .when_some(self.crumb(), |el, name| {
                el.child(div().text_color(hsla(p.text_2)).child("›")).child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(hsla(p.text))
                        .child(name),
                )
            })
    }

    /// Centred message used by the empty / not-installed / no-selection states.
    fn centred(&self, content: impl IntoElement) -> AnyElement {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .child(content)
            .into_any_element()
    }

    fn state_pane(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        match self.screen() {
            Screen::Loading => v_flex()
                .flex_1()
                .p(space(4))
                .gap(space(1))
                .child(skeleton_row(&[0.3], p))
                .child(skeleton_row(&[0.9], p))
                .child(skeleton_row(&[0.8], p))
                .child(skeleton_row(&[0.85], p))
                .into_any_element(),
            Screen::NotInstalled { message, hint } => self.centred(
                v_flex()
                    .items_center()
                    .gap(space(1))
                    .child(heading(message, p))
                    .child(caption(
                        "터미널에서 아래 명령으로 설치한 뒤 다시 열어 주세요.",
                        p,
                    ))
                    .when_some(hint, |el, h| el.child(mono(h, p.text))),
            ),
            Screen::Empty => self.centred(
                v_flex()
                    .items_center()
                    .gap(space(2))
                    .child(heading("프로필이 없습니다.", p))
                    .child(caption("새 프로필을 만들어 Colima VM을 시작하세요.", p))
                    .child(
                        button("empty-new-profile", "새 프로필", ButtonVariant::Primary, p)
                            .on_click(cx.listener(|v, _, _, cx| v.new_profile(cx))),
                    ),
            ),
            Screen::Create => {
                let body = self
                    .editor()
                    .cloned()
                    .map(|e| e.into_any_element())
                    .unwrap_or_else(|| div().into_any_element());
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        h_flex().px(space(4)).pt(space(3)).pb(space(2)).child(
                            div()
                                .text_size(px(font::TITLE.size))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("새 프로필"),
                        ),
                    )
                    .child(div().flex_1().min_h_0().child(body))
                    .into_any_element()
            }
            Screen::Profile => self.profile_pane(p, cx),
        }
    }

    fn error_banner(&self, p: &Palette) -> Option<Div> {
        self.error.clone().map(|e| {
            div()
                .mx(space(4))
                .mb(space(2))
                .px(space(2))
                .py(space(1))
                .rounded(px(metric::RADIUS_MD))
                .bg(hsla(p.err_bg))
                .border_1()
                .border_color(hsla(p.err_border))
                .text_color(hsla(p.err))
                .text_size(px(font::SMALL.size))
                .child(e)
        })
    }

    fn profile_pane(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(d) = self.detail() else {
            return self.centred(caption("프로필을 선택하세요.", p));
        };
        let actions = self.header_actions().into_iter().map(|act| {
            button(
                SharedString::from(format!("header-{}", act.label())),
                act.label(),
                ButtonVariant::Default,
                p,
            )
            .on_click(cx.listener(move |v, _, _, cx| v.act(act, cx)))
        });
        let header = h_flex()
            .px(space(4))
            .pt(space(3))
            .pb(space(2))
            .gap(space(1))
            .child(
                div()
                    .text_size(px(font::TITLE.size))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(hsla(p.text))
                    .child(d.name.clone()),
            )
            .child(status_chip(
                "header-status",
                status_of(d.kind),
                d.status_label,
                p,
            ))
            .child(div().flex_1())
            .children(actions);
        let tabs = Tab::ALL.into_iter().map(|t| {
            let active = t == self.tab;
            let count = self.tab_count(t, cx);
            h_flex()
                .id(t.label())
                .h(px(metric::TAB_H))
                .px(half_step())
                .mr(space(3))
                .gap(space(0))
                .border_b_2()
                .border_color(if active {
                    hsla(p.text)
                } else {
                    gpui_kit::transparent_black()
                })
                .cursor_pointer()
                .text_size(px(font::BODY.size))
                .font_weight(FontWeight::MEDIUM)
                .text_color(hsla(if active { p.text } else { p.text_2 }))
                .hover(|s| s.text_color(hsla(p.text)))
                .on_click(cx.listener(move |v, _, _, cx| v.set_tab(t, cx)))
                .child(t.label())
                .when_some(count, |el, n| {
                    el.child(
                        div()
                            .font_family(font::MONO)
                            .text_size(px(font::CODE.size))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(hsla(p.text_2))
                            .child(n.to_string()),
                    )
                })
        });
        let body: AnyElement = match self.editor() {
            Some(editor) => editor.clone().into_any_element(),
            None => self.engine.clone().into_any_element(),
        };
        let progress = self.progress().cloned().map(|log| {
            let ended = !log.read(cx).is_running();
            v_flex()
                .flex_none()
                .max_h(px(metric::LOG_PANEL_H))
                .border_b_1()
                .border_color(hsla(p.border))
                .when(ended, |el| {
                    el.child(
                        h_flex().px(space(3)).pt(space(1)).justify_end().child(
                            button("close-progress", "닫기", ButtonVariant::Ghost, p)
                                .on_click(cx.listener(|v, _, _, cx| v.close_progress(cx))),
                        ),
                    )
                })
                .child(log)
        });
        h_flex()
            .flex_1()
            .min_h_0()
            .items_start()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(header)
                    .children(self.error_banner(p))
                    .children(progress)
                    .when(self.editor.is_none(), |el| {
                        el.child(
                            h_flex()
                                .px(space(4))
                                .border_b_1()
                                .border_color(hsla(p.border))
                                .children(tabs),
                        )
                    })
                    .child(div().flex_1().min_h_0().child(body)),
            )
            .children(self.inspector_pane(p, cx))
            .into_any_element()
    }

    /// 260 px 구성 aside: kv rows, 편집 in its header, docker socket, delete zone (H1).
    fn inspector_pane(&self, p: &Palette, cx: &mut Context<Self>) -> Option<Div> {
        let ins = self.inspector()?;
        let edit = if ins.can_edit {
            button("edit", "편집", ButtonVariant::Default, p)
                .on_click(cx.listener(|v, _, _, cx| v.edit(cx)))
        } else {
            crate::components::button::button_disabled("edit", "편집", p)
        };
        let delete = if ins.can_delete {
            button("delete", "프로필 삭제", ButtonVariant::Danger, p)
                .on_click(cx.listener(|v, _, _, cx| v.delete(cx)))
        } else {
            crate::components::button::button_disabled("delete", "프로필 삭제", p)
        };
        let kv = ins.rows.into_iter().map(|(label, value)| {
            h_flex()
                .min_h(px(metric::ROW))
                .gap(space(1))
                .child(caption(label, p).w(px(metric::KV_LABEL)).flex_none())
                .child(mono(value, p.text).min_w_0().truncate())
        });
        Some(
            v_flex()
                .w(px(metric::INSPECTOR))
                .flex_none()
                .h_full()
                .pt(space(2))
                .px(space(3))
                .pb(space(3))
                .bg(hsla(p.bg_chrome))
                .border_l_1()
                .border_color(hsla(p.border))
                .child(
                    h_flex()
                        .h(px(metric::ROW))
                        .mb(space(0))
                        .justify_between()
                        .child(heading("구성", p))
                        .child(edit),
                )
                .children(kv)
                .child(
                    v_flex()
                        .py(space(1))
                        .gap(half_step())
                        .child(caption("docker 소켓", p))
                        .child(mono(ins.socket, p.text)),
                )
                .child(caption(
                    "CPU·메모리·디스크를 바꾸면 다시 시작해야 적용됩니다.",
                    p,
                ))
                .child(
                    v_flex()
                        .mt_auto()
                        .pt(space(2))
                        .gap(space(1))
                        .border_t_1()
                        .border_color(hsla(p.border))
                        .child(heading("프로필 삭제", p))
                        .child(caption(
                            "VM 디스크와 컨테이너, 볼륨이 함께 지워집니다. 되돌릴 수 없습니다.",
                            p,
                        ))
                        .child(div().child(delete)),
                ),
        )
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(window);
        let overlay = self.delete_confirm().cloned().map(|confirm| {
            fade_in(
                "delete-confirm-scrim",
                scrim(p)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().id("delete-confirm").occlude().child(confirm)),
            )
        });
        let screen = self.screen();
        let main = v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(hsla(p.bg))
            .child(self.titlebar(p))
            .when(screen != Screen::Profile, |el| {
                el.children(self.error_banner(p))
            })
            .child(self.state_pane(p, cx));
        h_flex()
            .relative()
            .size_full()
            .bg(hsla(p.bg))
            .font_family(font::SANS)
            .text_color(hsla(p.text))
            .child(self.sidebar(p, cx))
            .child(main)
            .children(overlay)
    }
}

#[cfg(test)]
mod tests;
