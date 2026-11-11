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
use telima_app::profiles::{Lifecycle, VersionCheck};
use telima_app::runtime::App as Telima;
use telima_app::view::daily::{
    MountRow, VersionFooter, VersionNotice, VersionNoticeSeverity, confirm_prune_cache, mount_rows,
    version_footer, version_notice,
};
use telima_app::view::notice::{NoticeKind, error_text, notice};
use telima_app::view::time_text::{uptime_label, uptime_value_label};
use telima_app::view::{
    Confirm, PROFILE_DELETE_NOTE, ProfileDetail, ProfileRow, StatusKind, confirm_delete_profile,
    engine_unavailable, profile_actions, profile_detail, profile_rows,
};
use telima_core::profile::{Profile, ProfileName, ProfileSpec, ProfileStatus, RuntimeInfo};

use crate::components::banner::{BannerTone, banner};
use crate::components::button::{ButtonVariant, button, button_with_icon, icon_button};
use crate::components::chip::{label_chip, status_chip};
use crate::components::clock;
use crate::components::dialog::{fade_in, modal, scrim};
use crate::components::icons::{Icon, icon};
use crate::components::paragraph::paragraph;
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

/// Status of a placeholder row whose create failed; any non-Colima status shows as 오류.
const CREATE_FAILED: &str = "Failed";

/// Result of a use case, sent from the spawner thread to the view.
enum Msg {
    Profiles(Result<Vec<Profile>, AppError>),
    /// Stored spec for the edit form.
    Spec(ProfileName, Result<ProfileSpec, AppError>),
    /// Stored spec for the inspector (VM 종류, 마운트); errors leave the values unknown.
    Config(ProfileName, Result<ProfileSpec, AppError>),
    /// What a delete would remove: container count, volume count, volume bytes.
    DeleteFacts(ProfileName, usize, Vec<Option<u64>>),
    /// Start time and versions of a profile (uptime, sidebar footer).
    Runtime(ProfileName, Result<RuntimeInfo, AppError>),
    /// Installed Colima version, checked once at startup.
    Version(Result<VersionCheck, AppError>),
    /// `colima prune` finished.
    Pruned(Result<(), AppError>),
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
    /// Stored specs, loaded when a profile is selected (inspector values, N2).
    configs: HashMap<ProfileName, ProfileSpec>,
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
    /// Start time and versions per profile, reloaded on select and list refresh.
    runtime: HashMap<ProfileName, RuntimeInfo>,
    /// Startup Colima version check; `None` until answered or when it failed.
    version: Option<VersionCheck>,
    /// The untested-version warning was closed (blocking notices cannot be).
    notice_dismissed: bool,
    /// Open 캐시 정리 confirmation.
    prune_confirm: Option<Confirm>,
    /// Focus of the 캐시 정리 confirmation, so Escape reaches it (F2).
    prune_focus: FocusHandle,
    /// Set by [`Self::prune`] until the next render moves focus into the confirmation.
    prune_focus_on_open: bool,
    /// Profiles being created, shown in the sidebar until `colima ls` lists them (F1). A
    /// failed or cancelled create keeps its row until the user closes its log.
    placeholders: HashMap<ProfileName, Profile>,
    /// Docker server version last reported by any profile; the footer keeps it while the
    /// selected profile's engine is down (F10).
    last_docker_server: Option<String>,
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
            configs: HashMap::new(),
            pending: HashSet::new(),
            error: None,
            tab: Tab::Containers,
            engine: cx.new(|cx| {
                let mut panel = EnginePanel::new(app.clone(), cx);
                panel.host_dialog();
                panel
            }),
            editor: None,
            progress: HashMap::new(),
            delete_confirm: None,
            runtime: HashMap::new(),
            version: None,
            notice_dismissed: false,
            prune_confirm: None,
            prune_focus: cx.focus_handle(),
            prune_focus_on_open: false,
            placeholders: HashMap::new(),
            last_docker_server: None,
            _poll: poll,
        };
        view.refresh();
        let inbox = view.inbox.clone();
        view.app
            .check_colima_version(move |r| inbox.lock().unwrap().push(Msg::Version(r)));
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
                    for (name, p) in &self.placeholders {
                        if !list.iter().any(|q| &q.name == name) {
                            list.push(p.clone());
                        }
                    }
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
                    self.load_config();
                    self.load_runtime();
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
                Msg::Config(name, Ok(spec)) => {
                    self.configs.insert(name, spec);
                }
                Msg::Config(_, Err(_)) => {}
                Msg::Runtime(name, Ok(info)) => {
                    if info.docker_server.is_some() {
                        self.last_docker_server = info.docker_server.clone();
                    }
                    self.runtime.insert(name, info);
                }
                // Unknown uptime and versions show as —.
                Msg::Runtime(name, Err(_)) => {
                    self.runtime.remove(&name);
                }
                Msg::Version(Ok(check)) => self.version = Some(check),
                // A missing Colima has its own screen (profile list error).
                Msg::Version(Err(_)) => {}
                Msg::Pruned(Ok(())) => {}
                Msg::Pruned(Err(e)) => self.error = Some(error_text(&e)),
                Msg::DeleteFacts(name, containers, volume_sizes) => {
                    if self.selected.as_ref() == Some(&name) && self.delete_confirm.is_none() {
                        let copy = confirm_delete_profile(
                            &name,
                            containers,
                            &volume_sizes,
                            telima_core::profile::DeleteOptions::default(),
                        );
                        let app = self.app.clone();
                        let confirm = cx.new(|cx| {
                            DeleteConfirm::new(app, name, copy, cx)
                                .with_facts(containers, volume_sizes.clone())
                        });
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
    /// A stopped or busy profile gets a state message instead of a load (N1, N2).
    fn sync_engine(&mut self, cx: &mut Context<Self>) {
        let (profile, tab) = (self.selected.clone(), self.tab);
        let unavailable = profile.as_ref().and_then(|name| {
            let status = self.list().iter().find(|p| &p.name == name)?.status.clone();
            engine_unavailable(&status, self.busy(name))
        });
        self.engine
            .update(cx, |e, cx| e.set_target(profile, unavailable, tab, cx));
    }

    /// Loads the selected profile's stored spec for the inspector, once per profile.
    fn load_config(&mut self) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        if self.configs.contains_key(&name) {
            return;
        }
        let inbox = self.inbox.clone();
        let key = name.clone();
        self.app.profile_spec(name, move |r| {
            inbox.lock().unwrap().push(Msg::Config(key, r))
        });
    }

    /// Loads the selected profile's start time and versions.
    fn load_runtime(&mut self) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let inbox = self.inbox.clone();
        let key = name.clone();
        self.app.runtime_info(name, move |r| {
            inbox.lock().unwrap().push(Msg::Runtime(key, r))
        });
    }

    /// Runtime info of the selected profile while it runs and no command is changing it.
    fn running_info(&self) -> Option<&RuntimeInfo> {
        let name = self.selected.as_ref()?;
        let p = self.list().iter().find(|p| &p.name == name)?;
        if p.status != ProfileStatus::Running || self.busy(name) {
            return None;
        }
        self.runtime.get(name)
    }

    /// `가동 3시간 12분` for the selected profile while it runs (not while it stops, F3).
    pub fn uptime(&self) -> Option<String> {
        uptime_label(self.running_info()?, clock::now())
    }

    /// Status chip text of the profile header: the running command's status (`중지 중`)
    /// while its progress runs, else the VM state (F3: one chip, not two).
    pub fn header_status_label(&self, cx: &App) -> Option<String> {
        if let Some(p) = self.progress().map(|p| p.read(cx))
            && p.is_running()
        {
            return Some(p.status_label().to_owned());
        }
        self.detail().map(|d| d.status_label.to_string())
    }

    /// Colima and Docker versions for the sidebar footer (selected profile; Colima falls
    /// back to the startup check, Docker to the last server version seen, F10).
    pub fn version_footer(&self) -> VersionFooter {
        let mut info = self
            .selected
            .as_ref()
            .and_then(|n| self.runtime.get(n))
            .cloned()
            .unwrap_or_default();
        if info.colima.is_none() {
            info.colima = self.version.map(|v| v.version);
        }
        if info.docker_server.is_none() {
            info.docker_server = self.last_docker_server.clone();
        }
        version_footer(&info)
    }

    /// Startup notice about the installed Colima version, unless dismissed.
    pub fn version_notice(&self) -> Option<VersionNotice> {
        let check = self.version?;
        let n = version_notice(check.version, check.support)?;
        let hidden = self.notice_dismissed && n.severity == VersionNoticeSeverity::Warning;
        (!hidden).then_some(n)
    }

    /// Closes the untested-version warning; a too-old notice stays.
    pub fn dismiss_version_notice(&mut self, cx: &mut Context<Self>) {
        self.notice_dismissed = true;
        cx.notify();
    }

    /// Open 캐시 정리 confirmation.
    pub fn prune_confirm(&self) -> Option<&Confirm> {
        self.prune_confirm.as_ref()
    }

    /// Opens the 캐시 정리 confirmation; nothing runs yet. It takes focus on the next render.
    pub fn prune(&mut self, cx: &mut Context<Self>) {
        if self.dialog_open(cx) {
            return;
        }
        self.prune_confirm = Some(confirm_prune_cache());
        self.prune_focus_on_open = true;
        cx.notify();
    }

    /// Any dialog is open: delete, 캐시 정리 or the engine panel's confirm. One at a time.
    fn dialog_open(&self, cx: &App) -> bool {
        self.delete_confirm.is_some()
            || self.prune_confirm.is_some()
            || self.engine.read(cx).dialog().is_some()
    }

    /// True while keyboard focus is inside the 캐시 정리 confirmation.
    pub fn prune_has_focus(&self, window: &Window) -> bool {
        self.prune_focus.is_focused(window)
    }

    pub fn cancel_prune(&mut self, cx: &mut Context<Self>) {
        self.prune_confirm = None;
        cx.notify();
    }

    /// Runs `colima prune` and closes the confirmation.
    pub fn confirm_prune(&mut self, cx: &mut Context<Self>) {
        if self.prune_confirm.take().is_none_or(|c| !c.can_confirm) {
            cx.notify();
            return;
        }
        self.error = None;
        let inbox = self.inbox.clone();
        self.app
            .prune_cache(move |r| inbox.lock().unwrap().push(Msg::Pruned(r)));
        cx.notify();
    }

    fn open_editor(&mut self, editor: Entity<ProfileEditor>, cx: &mut Context<Self>) {
        let sub = cx.subscribe(&editor, |v, _, ev: &FormEvent, cx| {
            v.editor = None;
            v.creating = false;
            match ev {
                FormEvent::Saved(name) => {
                    v.selected = Some(name.clone());
                    // The spec may have changed: reload it with the list.
                    v.configs.remove(name);
                    v.refresh();
                }
                FormEvent::Creating { spec, progress } => {
                    v.show_create(spec, progress.clone(), cx);
                }
                FormEvent::Cancelled => {}
            }
            cx.notify();
        });
        self.editor = Some((editor, sub));
        cx.notify();
    }

    /// Lists and selects a profile being created and shows its start progress (F1).
    fn show_create(
        &mut self,
        spec: &ProfileSpec,
        progress: Entity<StartProgress>,
        cx: &mut Context<Self>,
    ) {
        let name = spec.name.clone();
        let placeholder = Profile {
            name: name.clone(),
            status: ProfileStatus::Stopped,
            arch: "—".to_owned(),
            cpus: spec.cpus,
            memory_bytes: u64::from(spec.memory_gib) << 30,
            disk_bytes: u64::from(spec.disk_gib) << 30,
            address: None,
            runtime: spec.runtime,
        };
        if let Some(list) = &mut self.profiles
            && !list.iter().any(|p| p.name == name)
        {
            list.push(placeholder.clone());
            list.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
        }
        self.placeholders.insert(name.clone(), placeholder);
        self.selected = Some(name.clone());
        self.configs.remove(&name);
        self.error = None;
        self.sync_engine(cx);
        self.track(name, progress, cx);
    }

    /// Marks `name` busy and shows `progress` until it ends.
    fn track(
        &mut self,
        name: ProfileName,
        progress: Entity<StartProgress>,
        cx: &mut Context<Self>,
    ) {
        self.pending.insert(name.clone());
        self.started.insert(name.clone(), Instant::now());
        let key = name.clone();
        let sub = cx.subscribe(&progress, move |v, _, ev: &ProgressEnd, cx| {
            v.pending.remove(&key);
            v.started.remove(&key);
            // A clean finish needs no log; failures and cancels stay until closed, and so
            // does the sidebar row of a create that Colima does not list.
            match ev {
                ProgressEnd::Done => {
                    v.progress.remove(&key);
                    v.placeholders.remove(&key);
                }
                ProgressEnd::Failed(_) => v.mark_create_failed(&key),
                ProgressEnd::Cancelled => {}
            }
            v.refresh();
            cx.notify();
        });
        self.progress.insert(name, (progress, sub));
        self.sync_engine(cx);
        cx.notify();
    }

    /// Shows the placeholder row of a failed create in the error state.
    fn mark_create_failed(&mut self, name: &ProfileName) {
        let Some(placeholder) = self.placeholders.get_mut(name) else {
            return;
        };
        let before = placeholder.clone();
        placeholder.status = ProfileStatus::Other(CREATE_FAILED.to_owned());
        let failed = placeholder.clone();
        if let Some(row) = self.profiles.iter_mut().flatten().find(|p| **p == before) {
            *row = failed;
        }
    }

    /// `name` is only a placeholder row: Colima does not list it, so no command applies.
    fn unlisted(&self, name: &ProfileName) -> bool {
        self.placeholders
            .get(name)
            .is_some_and(|ph| self.list().iter().any(|p| p == ph))
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
        let mut d = profile_detail(p, self.configs.get(name), self.busy(name));
        if self.unlisted(name) {
            d.actions = profile_actions(d.kind, true);
        }
        Some(d)
    }

    pub fn select(&mut self, name: &str, cx: &mut Context<Self>) {
        if let Some(p) = self.list().iter().find(|p| p.name.as_str() == name) {
            self.selected = Some(p.name.clone());
            self.editor = None;
            self.creating = false;
            self.sync_engine(cx);
            self.load_config();
            self.load_runtime();
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
        if self.busy(&name) || self.unlisted(&name) {
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
        if self.busy(&name) || self.unlisted(&name) || self.dialog_open(cx) {
            return;
        }
        let inbox = self.inbox.clone();
        let app = self.app.clone();
        let key = name.clone();
        self.app.containers(name, move |containers| {
            let containers = containers.map(|c| c.len()).unwrap_or(0);
            app.volumes(key.clone(), move |volumes| {
                let volume_sizes = volumes
                    .map(|v| v.iter().map(|v| v.size_bytes).collect())
                    .unwrap_or_default();
                inbox
                    .lock()
                    .unwrap()
                    .push(Msg::DeleteFacts(key, containers, volume_sizes));
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
            // A failed create's row goes too, unless Colima lists the profile.
            if self.placeholders.remove(&name).is_some() {
                self.refresh();
            }
            cx.notify();
        }
    }

    fn run(&mut self, cmd: Lifecycle, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        if self.busy(&name) || self.unlisted(&name) {
            return;
        }
        self.error = None;
        self.editor = None;
        self.creating = false;
        let app = self.app.clone();
        let target = name.clone();
        let progress = cx.new(|cx| StartProgress::new(app, target, cmd, cx));
        self.track(name, progress, cx);
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
    /// Canvas icon drawn before the label (M1).
    pub fn icon(self) -> Icon {
        match self {
            HeaderAction::Start => Icon::Play,
            HeaderAction::Restart => Icon::Restart,
            HeaderAction::Stop => Icon::Stop,
        }
    }

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
    /// Host directories mounted into the VM (empty: none or spec not loaded).
    pub mounts: Vec<MountRow>,
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
        let spec = self.selected.as_ref().and_then(|n| self.configs.get(n));
        let kubernetes = spec.map_or("—", |s| {
            if s.kubernetes {
                "사용"
            } else {
                "사용 안 함"
            }
        });
        Some(Inspector {
            mounts: spec.map(|s| mount_rows(&s.mounts)).unwrap_or_default(),
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
                (
                    "가동 시간",
                    self.running_info()
                        .and_then(|i| uptime_value_label(i, clock::now()))
                        .unwrap_or_else(|| "—".to_owned()),
                ),
                ("Kubernetes", kubernetes.to_owned()),
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

/// Caption-styled sentence that wraps only between Korean words (N5).
fn note(text: &str, p: &Palette) -> Div {
    paragraph(text)
        .text_size(px(font::SMALL.size))
        .text_color(hsla(p.text_2))
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
                                icon_button("new-profile", icon(Icon::Plus, hsla(p.text_2)), p)
                                    .on_click(cx.listener(|v, _, _, cx| v.new_profile(cx))),
                            ),
                    )
                    .children(skeleton)
                    .children(rows),
            )
            .child({
                let versions = self.version_footer();
                v_flex()
                    .mt_auto()
                    .p(space(1))
                    .pl(space(3))
                    .gap(half_step())
                    .border_t_1()
                    .border_color(hsla(p.border))
                    .child(mono(versions.colima, p.text_2))
                    .child(mono(versions.docker, p.text_2))
                    .child(
                        h_flex()
                            .justify_between()
                            .child(mono(
                                concat!("telima ", env!("CARGO_PKG_VERSION")),
                                p.text_2,
                            ))
                            .child(
                                button("prune-cache", "캐시 정리", ButtonVariant::Ghost, p)
                                    .debug_selector(|| "prune-cache".to_string())
                                    .on_click(cx.listener(|v, _, _, cx| v.prune(cx))),
                            ),
                    )
            })
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
                // Inline form at the page inset; the crumb carries 새 프로필 (N4).
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .p(space(4))
                    .child(body)
                    .into_any_element()
            }
            Screen::Profile => self.profile_pane(p, cx),
        }
    }

    fn error_banner(&self, p: &Palette) -> Option<Div> {
        self.error.clone().map(|e| {
            banner(BannerTone::Error, p)
                .mx(space(4))
                .mb(space(2))
                .child(e)
        })
    }

    /// Colima version notice: too old blocks with an upgrade hint; untested can be closed.
    fn version_banner(&self, p: &Palette, cx: &mut Context<Self>) -> Option<Div> {
        let n = self.version_notice()?;
        let tone = match n.severity {
            VersionNoticeSeverity::Blocking => BannerTone::Error,
            VersionNoticeSeverity::Warning => BannerTone::Warning,
        };
        let closable = n.severity == VersionNoticeSeverity::Warning;
        Some(
            banner(tone, p)
                .mx(space(4))
                .mb(space(2))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(n.title))
                .child(paragraph(&n.message).flex_1().min_w_0())
                .when_some(n.hint, |el, h| el.child(mono(h, p.text)))
                .when(closable, |el| {
                    el.child(
                        button("dismiss-version", "닫기", ButtonVariant::Ghost, p)
                            .on_click(cx.listener(|v, _, _, cx| v.dismiss_version_notice(cx))),
                    )
                }),
        )
    }

    /// 캐시 정리 confirmation in the shared dialog frame.
    fn prune_dialog(&self, p: &Palette, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let c = self.prune_confirm.clone()?;
        let confirm = if c.can_confirm {
            button("prune-confirm", c.confirm_label, ButtonVariant::Primary, p)
                .on_click(cx.listener(|v, _, _, cx| v.confirm_prune(cx)))
        } else {
            crate::components::button::button_disabled("prune-confirm", c.confirm_label, p)
        };
        Some(modal(
            "prune-confirm-scrim",
            metric::DIALOG_W_SM,
            p,
            v_flex()
                .id("prune-confirm")
                .occlude()
                .track_focus(&self.prune_focus)
                .on_click(cx.listener(|v, _, window, cx| window.focus(&v.prune_focus, cx)))
                .on_key_down(cx.listener(|v, ev: &KeyDownEvent, _, cx| {
                    if ev.keystroke.key == "escape" {
                        v.cancel_prune(cx);
                    }
                }))
                .gap(space(2))
                .child(heading(c.title, p))
                .child(note(&c.body, p))
                .child(
                    h_flex()
                        .justify_end()
                        .gap(space(1))
                        .child(
                            button("prune-cancel", "취소", ButtonVariant::Default, p)
                                .on_click(cx.listener(|v, _, _, cx| v.cancel_prune(cx))),
                        )
                        .child(confirm),
                ),
        ))
    }

    fn profile_pane(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(d) = self.detail() else {
            return self.centred(caption("프로필을 선택하세요.", p));
        };
        let actions = self.header_actions().into_iter().map(|act| {
            button_with_icon(
                SharedString::from(format!("header-{}", act.label())),
                act.icon(),
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
                self.header_status_label(cx)
                    .unwrap_or_else(|| d.status_label.to_string()),
                p,
            ))
            .when_some(self.uptime(), |el, up| el.child(label_chip(up, p)))
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
            // Inline edit form at the page inset; the header carries the profile (N4).
            Some(editor) => div()
                .size_full()
                .p(space(4))
                .child(editor.clone())
                .into_any_element(),
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
                .debug_selector(|| "delete".to_string())
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
                        .child(caption("디렉터리 마운트", p))
                        .when(ins.mounts.is_empty(), |el| {
                            el.child(note(telima_app::view::daily::MOUNTS_EMPTY, p))
                        })
                        .children(ins.mounts.into_iter().map(|m| {
                            h_flex()
                                .gap(space(1))
                                .child(mono(m.path, p.text).flex_1().min_w_0().truncate())
                                .child(caption(m.mode, p).flex_none())
                        })),
                )
                .child(
                    v_flex()
                        .py(space(1))
                        .gap(half_step())
                        .child(caption("docker 소켓", p))
                        .child(mono(ins.socket, p.text)),
                )
                .child(note(
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
                        .child(note(PROFILE_DELETE_NOTE, p))
                        .child(div().child(delete)),
                ),
        )
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(window);
        if self.prune_confirm.is_some() && std::mem::take(&mut self.prune_focus_on_open) {
            window.focus(&self.prune_focus, cx);
        }
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
        let own_dialog = self.delete_confirm.is_some() || self.prune_confirm.is_some();
        let engine_dialog = self.engine.update(cx, |e, cx| {
            e.set_blocked(own_dialog);
            e.dialog_overlay(window, cx)
        });
        let screen = self.screen();
        let main = v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(hsla(p.bg))
            .child(self.titlebar(p))
            .children(self.version_banner(p, cx))
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
            .children(engine_dialog)
            .children(overlay)
            .children(self.prune_dialog(p, cx))
    }
}

#[cfg(test)]
mod tests;
