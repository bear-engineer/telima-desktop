//! Containers / images / volumes of one profile, plus the container log viewer.
//!
//! [`EnginePanel`] holds the list states and runs the engine use cases on the
//! [`Telima`] facade. Like the shell, callbacks only push into an inbox that
//! [`EnginePanel::pump`] applies on the GPUI thread. Rendering lives in `render.rs`.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::{Context, FocusHandle, Task};
use telima_app::AppError;
use telima_app::runtime::{App as Telima, Task as Job};
use telima_app::view::notice::{Notice, error_text, notice};
use telima_app::view::{
    Confirm, ContainerRow, ImageRow, VolumeRow, confirm_prune_images, confirm_prune_volumes,
    confirm_remove_container, container_rows, image_rows, volume_rows,
};
use telima_core::container::{Container, ContainerState, Image, LogLine, PruneReport, Volume};
use telima_core::profile::ProfileName;

use crate::shell::Tab;

mod copy;
mod icons;
mod model;
mod render;

pub use model::{
    FilterCounts, LineTone, LogRow, RowActions, StatusFilter, container_status, filter_rows,
    line_tone, log_rows, row_actions, split_timestamp, volume_chip,
};

/// How often the view drains finished use-case results.
const POLL: Duration = Duration::from_millis(50);
/// Lines kept in the log viewer; older ones are dropped.
const LOG_KEEP: usize = 5_000;

/// State of one list. `Ready(vec![])` is the empty state.
#[derive(Debug, Clone, PartialEq)]
pub enum ListState<T> {
    /// No profile selected, nothing asked yet.
    Idle,
    Loading,
    Ready(Vec<T>),
    Failed(String),
}

/// What a confirmed dialog runs.
#[derive(Debug, Clone, PartialEq)]
enum Action {
    RemoveContainer {
        id: String,
        force: bool,
    },
    /// Removes stopped containers one by one (never forced).
    RemoveContainers(Vec<String>),
    PruneImages,
    PruneVolumes,
}

/// A destructive confirmation: copy from `telima_app::view` and the items it touches.
#[derive(Debug, Clone, PartialEq)]
pub struct Dialog {
    pub confirm: Confirm,
    /// Image ids / volume names / container name the action removes.
    pub items: Vec<String>,
    action: Action,
}

/// Console-style log viewer of one container.
#[derive(Debug, Clone, PartialEq)]
pub struct LogPanel {
    pub container: String,
    pub name: String,
    /// Image and short id shown next to the name in the panel header.
    pub image: String,
    pub follow: bool,
    /// The 시각 switch: show the timestamp column.
    pub show_time: bool,
    /// The 로그 검색 query; blank shows every line.
    pub query: String,
    pub lines: Vec<LogLine>,
    pub error: Option<String>,
    stream: u64,
}

enum Msg {
    Containers(Result<Vec<Container>, AppError>),
    Images(Result<Vec<Image>, AppError>),
    Volumes(Result<Vec<Volume>, AppError>),
    ImagePreview(Result<Vec<Image>, AppError>),
    VolumePreview(Result<Vec<Volume>, AppError>),
    /// An action finished; reload the tab it changed.
    Done(Tab, Result<(), AppError>),
    Pruned(Tab, Result<PruneReport, AppError>),
    Log(u64, LogLine),
    LogEnd(u64, Result<(), AppError>),
}

/// Messages carry the profile epoch so answers for a previous profile are dropped.
type Inbox = Arc<Mutex<Vec<(u64, Msg)>>>;

pub struct EnginePanel {
    app: Telima,
    inbox: Inbox,
    epoch: u64,
    profile: Option<ProfileName>,
    tab: Tab,
    containers: ListState<ContainerRow>,
    images: ListState<ImageRow>,
    volumes: ListState<VolumeRow>,
    dialog: Option<Dialog>,
    /// Last action error, shown above the list.
    notice: Option<Notice>,
    query: String,
    filter: StatusFilter,
    selected: Option<String>,
    menu: Option<String>,
    /// Keyboard focus of the toolbar filter field.
    focus: FocusHandle,
    /// Keyboard focus of the log panel, so Escape closes it (N6).
    log_focus: FocusHandle,
    /// Keyboard focus of the 로그 검색 field.
    log_search_focus: FocusHandle,
    /// Keyboard focus of the open ⋯ menu, so Escape closes it.
    menu_focus: FocusHandle,
    /// Keyboard focus of the confirm dialog, so Escape closes it (F2 live).
    pub(super) dialog_focus: FocusHandle,
    /// Set when a dialog opens until the next render moves focus into it.
    pub(super) dialog_focus_on_open: bool,
    /// The host window draws the dialog over the whole window ([`Self::dialog_overlay`]).
    hosted: bool,
    /// The host shows its own dialog; no second one opens (one dialog at a time).
    blocked: bool,
    log: Option<LogPanel>,
    log_job: Option<Job>,
    streams: u64,
    /// Last measured panel width (px); picks the containers table columns.
    width: f32,
    _poll: Task<()>,
}

impl EnginePanel {
    /// Records the measured panel width; re-renders only when the table columns change.
    /// Measured during prepaint, where a notify is dropped with the frame being drawn and the
    /// window waited for the next input event; a changed column set re-renders right after.
    pub(super) fn set_width(&mut self, w: f32, cx: &mut Context<Self>) {
        let changed = model::container_columns(w) != model::container_columns(self.width);
        self.width = w;
        if changed {
            let this = cx.entity().downgrade();
            cx.defer(move |cx| {
                this.update(cx, |_, cx| cx.notify()).ok();
            });
        }
    }

    pub fn new(app: Telima, cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                if this.update(cx, |view, cx| view.pump(cx)).is_err() {
                    break;
                }
            }
        });
        Self {
            app,
            inbox: Inbox::default(),
            epoch: 0,
            profile: None,
            tab: Tab::Containers,
            containers: ListState::Idle,
            images: ListState::Idle,
            volumes: ListState::Idle,
            dialog: None,
            notice: None,
            query: String::new(),
            filter: StatusFilter::All,
            selected: None,
            menu: None,
            focus: cx.focus_handle(),
            log_focus: cx.focus_handle(),
            log_search_focus: cx.focus_handle(),
            menu_focus: cx.focus_handle(),
            dialog_focus: cx.focus_handle(),
            dialog_focus_on_open: false,
            hosted: false,
            blocked: false,
            log: None,
            log_job: None,
            streams: 0,
            width: crate::theme::metric::WINDOW_W
                - crate::theme::metric::SIDEBAR
                - crate::theme::metric::INSPECTOR,
            _poll: poll,
        }
    }

    pub fn containers(&self) -> &ListState<ContainerRow> {
        &self.containers
    }

    pub fn images(&self) -> &ListState<ImageRow> {
        &self.images
    }

    pub fn volumes(&self) -> &ListState<VolumeRow> {
        &self.volumes
    }

    pub fn dialog(&self) -> Option<&Dialog> {
        self.dialog.as_ref()
    }

    pub fn log(&self) -> Option<&LogPanel> {
        self.log.as_ref()
    }

    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn status_filter(&self) -> StatusFilter {
        self.filter
    }

    pub fn tab(&self) -> Tab {
        self.tab
    }

    /// Shows `tab` of `profile`. A new profile clears every list; an unloaded tab loads.
    pub fn set_target(&mut self, profile: Option<ProfileName>, tab: Tab, cx: &mut Context<Self>) {
        if profile != self.profile {
            self.epoch += 1;
            self.profile = profile;
            self.containers = ListState::Idle;
            self.images = ListState::Idle;
            self.volumes = ListState::Idle;
            self.dialog = None;
            self.notice = None;
            self.selected = None;
            self.menu = None;
            self.close_logs(cx);
        }
        self.tab = tab;
        let idle = match tab {
            Tab::Containers => matches!(self.containers, ListState::Idle),
            Tab::Images => matches!(self.images, ListState::Idle),
            Tab::Volumes => matches!(self.volumes, ListState::Idle),
        };
        if idle {
            self.load(tab, cx);
        }
        cx.notify();
    }

    /// Reloads the current tab (the error state's retry button).
    pub fn retry(&mut self, cx: &mut Context<Self>) {
        self.notice = None;
        self.load(self.tab, cx);
    }

    fn sender(&self) -> impl Fn(Msg) + Send + Sync + Clone + 'static {
        let (inbox, epoch) = (self.inbox.clone(), self.epoch);
        move |m| inbox.lock().unwrap().push((epoch, m))
    }

    fn load(&mut self, tab: Tab, cx: &mut Context<Self>) {
        let Some(profile) = self.profile.clone() else {
            return;
        };
        let send = self.sender();
        match tab {
            Tab::Containers => {
                self.containers = ListState::Loading;
                self.app
                    .containers(profile, move |r| send(Msg::Containers(r)));
            }
            Tab::Images => {
                self.images = ListState::Loading;
                self.app.images(profile, move |r| send(Msg::Images(r)));
            }
            Tab::Volumes => {
                self.volumes = ListState::Loading;
                self.app.volumes(profile, move |r| send(Msg::Volumes(r)));
            }
        }
        cx.notify();
    }

    /// Applies finished use-case results. Called by the poll task (and by tests).
    pub fn pump(&mut self, cx: &mut Context<Self>) {
        let msgs = std::mem::take(&mut *self.inbox.lock().unwrap());
        if msgs.is_empty() {
            return;
        }
        for (epoch, msg) in msgs {
            if epoch == self.epoch {
                self.apply(msg, cx);
            }
        }
        cx.notify();
    }

    fn apply(&mut self, msg: Msg, cx: &mut Context<Self>) {
        fn list<T, R>(r: Result<Vec<T>, AppError>, rows: fn(&[T]) -> Vec<R>) -> ListState<R> {
            match r {
                Ok(v) => ListState::Ready(rows(&v)),
                Err(e) => ListState::Failed(error_text(&e)),
            }
        }
        match msg {
            Msg::Containers(r) => {
                self.containers = list(r, |c: &[Container]| container_rows(c, now()))
            }
            Msg::Images(r) => self.images = list(r, image_rows),
            Msg::Volumes(r) => self.volumes = list(r, volume_rows),
            Msg::ImagePreview(Ok(list)) => {
                self.open_dialog(Dialog {
                    confirm: confirm_prune_images(&list),
                    items: list.into_iter().map(|i| i.id).collect(),
                    action: Action::PruneImages,
                });
            }
            Msg::VolumePreview(Ok(list)) => {
                self.open_dialog(Dialog {
                    confirm: confirm_prune_volumes(&list),
                    items: list.into_iter().map(|v| v.name).collect(),
                    action: Action::PruneVolumes,
                });
            }
            Msg::ImagePreview(Err(e)) | Msg::VolumePreview(Err(e)) => {
                self.notice = Some(notice(&e));
            }
            Msg::Done(tab, r) => {
                if let Err(e) = r {
                    self.notice = Some(notice(&e));
                }
                self.load(tab, cx);
            }
            Msg::Pruned(tab, r) => {
                if let Err(e) = r {
                    self.notice = Some(notice(&e));
                }
                self.load(tab, cx);
            }
            Msg::Log(stream, line) => {
                if let Some(log) = self.log.as_mut().filter(|l| l.stream == stream) {
                    log.lines.push(line);
                    if log.lines.len() > LOG_KEEP {
                        let extra = log.lines.len() - LOG_KEEP;
                        log.lines.drain(..extra);
                    }
                }
            }
            Msg::LogEnd(stream, r) => {
                if let Some(log) = self.log.as_mut().filter(|l| l.stream == stream) {
                    log.error = r.err().map(|e| error_text(&e));
                }
            }
        }
    }

    /// Container rows after the toolbar filter.
    pub fn visible_containers(&self) -> Vec<ContainerRow> {
        match &self.containers {
            ListState::Ready(rows) => filter_rows(rows, &self.query, self.filter)
                .into_iter()
                .cloned()
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Segment counts of the loaded containers.
    pub fn filter_counts(&self) -> FilterCounts {
        match &self.containers {
            ListState::Ready(rows) => FilterCounts::of(rows),
            _ => FilterCounts::default(),
        }
    }

    pub fn set_status_filter(&mut self, filter: StatusFilter, cx: &mut Context<Self>) {
        self.filter = filter;
        cx.notify();
    }

    pub fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        cx.notify();
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Selects a row; its icon actions stay visible without hover.
    pub fn select(&mut self, id: &str, cx: &mut Context<Self>) {
        self.selected = Some(id.to_owned());
        cx.notify();
    }

    /// Row whose ⋯ menu is open.
    pub fn menu(&self) -> Option<&str> {
        self.menu.as_deref()
    }

    pub fn toggle_menu(&mut self, id: &str, cx: &mut Context<Self>) {
        self.menu = match self.menu.as_deref() {
            Some(open) if open == id => None,
            _ => Some(id.to_owned()),
        };
        self.selected = Some(id.to_owned());
        cx.notify();
    }

    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    /// Opens the confirm for removing every exited container ("종료된 N개 삭제").
    pub fn request_remove_exited(&mut self, cx: &mut Context<Self>) {
        let ListState::Ready(rows) = &self.containers else {
            return;
        };
        let ids = model::exited_ids(rows);
        if ids.is_empty() {
            return;
        }
        let items = rows
            .iter()
            .filter(|r| ids.contains(&r.id))
            .map(|r| r.name.clone())
            .collect();
        self.open_dialog(Dialog {
            confirm: copy::confirm_remove_exited(ids.len()),
            items,
            action: Action::RemoveContainers(ids),
        });
        cx.notify();
    }

    fn container(&self, id: &str) -> Option<&ContainerRow> {
        match &self.containers {
            ListState::Ready(rows) => rows.iter().find(|r| r.id == id),
            _ => None,
        }
    }

    pub fn start(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(profile) = self.profile.clone() else {
            return;
        };
        let send = self.sender();
        self.app.start_container(profile, id.to_owned(), move |r| {
            send(Msg::Done(Tab::Containers, r))
        });
        cx.notify();
    }

    pub fn stop(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(profile) = self.profile.clone() else {
            return;
        };
        let send = self.sender();
        self.app.stop_container(profile, id.to_owned(), move |r| {
            send(Msg::Done(Tab::Containers, r))
        });
        cx.notify();
    }

    pub fn restart(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(profile) = self.profile.clone() else {
            return;
        };
        let send = self.sender();
        self.app
            .restart_container(profile, id.to_owned(), move |r| {
                send(Msg::Done(Tab::Containers, r))
            });
        cx.notify();
    }

    /// Opens the remove confirmation. A running container is removed with force, and the
    /// dialog says so; nothing runs until [`EnginePanel::accept`].
    pub fn request_remove(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(row) = self.container(id) else {
            return;
        };
        let force = matches!(
            row.state,
            ContainerState::Running | ContainerState::Paused | ContainerState::Restarting
        );
        self.open_dialog(Dialog {
            confirm: confirm_remove_container(&row.name, row.state),
            items: vec![row.name.clone()],
            action: Action::RemoveContainer {
                id: id.to_owned(),
                force,
            },
        });
        cx.notify();
    }

    /// Asks for the prune preview; the dialog opens when it arrives.
    pub fn request_image_prune(&mut self, cx: &mut Context<Self>) {
        if let Some(profile) = self.profile.clone() {
            let send = self.sender();
            self.app
                .image_prune_preview(profile, move |r| send(Msg::ImagePreview(r)));
        }
        cx.notify();
    }

    pub fn request_volume_prune(&mut self, cx: &mut Context<Self>) {
        if let Some(profile) = self.profile.clone() {
            let send = self.sender();
            self.app
                .volume_prune_preview(profile, move |r| send(Msg::VolumePreview(r)));
        }
        cx.notify();
    }

    /// Runs the open dialog's action (only when the copy allows it).
    pub fn accept(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        let Some(profile) = self.profile.clone() else {
            return;
        };
        if !dialog.confirm.can_confirm {
            cx.notify();
            return;
        }
        let send = self.sender();
        match dialog.action {
            Action::RemoveContainer { id, force } => {
                self.app.remove_container(profile, id, force, move |r| {
                    send(Msg::Done(Tab::Containers, r))
                });
            }
            Action::RemoveContainers(ids) => {
                for id in ids {
                    let send = send.clone();
                    self.app
                        .remove_container(profile.clone(), id, false, move |r| {
                            send(Msg::Done(Tab::Containers, r))
                        });
                }
            }
            Action::PruneImages => {
                self.app
                    .prune_images(profile, move |r| send(Msg::Pruned(Tab::Images, r)));
            }
            Action::PruneVolumes => {
                self.app
                    .prune_volumes(profile, move |r| send(Msg::Pruned(Tab::Volumes, r)));
            }
        }
        cx.notify();
    }

    /// Opens `d` unless the host already shows a dialog; focus moves into it on render.
    fn open_dialog(&mut self, d: Dialog) {
        if self.blocked || self.dialog.is_some() {
            return;
        }
        self.dialog = Some(d);
        self.dialog_focus_on_open = true;
    }

    /// The host draws the confirm over the whole window via [`Self::dialog_overlay`].
    pub fn host_dialog(&mut self) {
        self.hosted = true;
    }

    /// Whether the host shows a dialog of its own; blocks opening this panel's confirm.
    pub fn set_blocked(&mut self, blocked: bool) {
        self.blocked = blocked;
    }

    pub(super) fn is_hosted(&self) -> bool {
        self.hosted
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        cx.notify();
    }

    /// Opens the log viewer for a container, following new lines.
    pub fn open_logs(&mut self, id: &str, cx: &mut Context<Self>) {
        let (name, image) = self.container(id).map_or_else(
            || (id.to_owned(), String::new()),
            |r| (r.name.clone(), r.image.clone()),
        );
        self.menu = None;
        self.selected = Some(id.to_owned());
        self.stream_logs(id.to_owned(), name, image, true, cx);
    }

    /// The 시각 switch of the log panel.
    pub fn set_show_time(&mut self, on: bool, cx: &mut Context<Self>) {
        if let Some(log) = self.log.as_mut() {
            log.show_time = on;
        }
        cx.notify();
    }

    /// The 로그 검색 query of the open log panel.
    pub fn log_query(&self) -> &str {
        self.log.as_ref().map_or("", |l| l.query.as_str())
    }

    pub fn set_log_query(&mut self, query: String, cx: &mut Context<Self>) {
        if let Some(log) = self.log.as_mut() {
            log.query = query;
        }
        cx.notify();
    }

    /// Log lines matching the query, with 시각 in the viewer's local offset.
    pub fn log_rows(&self) -> Vec<LogRow<'_>> {
        self.log
            .as_ref()
            .map_or_else(Vec::new, |l| log_rows(&l.lines, &l.query, utc_offset()))
    }

    /// Turns follow on or off; the stream restarts from the start of the log.
    pub fn set_follow(&mut self, follow: bool, cx: &mut Context<Self>) {
        let Some(log) = self.log.as_ref() else {
            return;
        };
        if log.follow == follow {
            return;
        }
        let (id, name, image) = (log.container.clone(), log.name.clone(), log.image.clone());
        self.stream_logs(id, name, image, follow, cx);
    }

    pub fn close_logs(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = self.log_job.take() {
            job.cancel();
        }
        self.log = None;
        cx.notify();
    }

    fn stream_logs(
        &mut self,
        id: String,
        name: String,
        image: String,
        follow: bool,
        cx: &mut Context<Self>,
    ) {
        let show_time = self.log.as_ref().is_none_or(|l| l.show_time);
        let query = self
            .log
            .as_ref()
            .filter(|l| l.container == id)
            .map(|l| l.query.clone())
            .unwrap_or_default();
        self.close_logs(cx);
        let Some(profile) = self.profile.clone() else {
            return;
        };
        self.streams += 1;
        let stream = self.streams;
        self.log = Some(LogPanel {
            container: id.clone(),
            name,
            image,
            follow,
            show_time,
            query,
            lines: Vec::new(),
            error: None,
            stream,
        });
        let (line, end) = (self.sender(), self.sender());
        self.log_job = Some(self.app.logs(
            profile,
            id,
            follow,
            move |l| line(Msg::Log(stream, l)),
            move |r| end(Msg::LogEnd(stream, r)),
        ));
        cx.notify();
    }
}

#[cfg(test)]
mod tests;

/// Wall-clock time for relative labels ("3시간 전"). Views read the clock; core and app do not.
fn now() -> telima_core::time::UnixMillis {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    telima_core::time::UnixMillis(ms)
}

/// The viewer's UTC offset in seconds for log 시각, read once from `date +%z` (std has no
/// local-time API and the workspace takes no tz crate). Falls back to UTC (0) if `date` fails.
fn utc_offset() -> i32 {
    static OFFSET: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *OFFSET.get_or_init(|| {
        std::process::Command::new("date")
            .arg("+%z")
            .output()
            .ok()
            .and_then(|o| parse_utc_offset(String::from_utf8_lossy(&o.stdout).trim()))
            .unwrap_or(0)
    })
}

/// `+0900` -> 32400, `-0330` -> -12600.
fn parse_utc_offset(s: &str) -> Option<i32> {
    let (sign, digits) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => return None,
    };
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (h, m): (i32, i32) = (digits[..2].parse().ok()?, digits[2..].parse().ok()?);
    Some(sign * (h * 3600 + m * 60))
}
