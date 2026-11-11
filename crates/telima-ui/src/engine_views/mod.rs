//! Containers / images / volumes of one profile, plus the container log viewer.
//!
//! [`EnginePanel`] holds the list states and runs the engine use cases on the
//! [`Telima`] facade. Like the shell, callbacks only push into an inbox that
//! [`EnginePanel::pump`] applies on the GPUI thread. Rendering lives in `render.rs`.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::{Context, Task};
use telima_app::AppError;
use telima_app::runtime::{App as Telima, Task as Job};
use telima_app::view::{
    Confirm, ContainerRow, ImageRow, VolumeRow, confirm_prune_images, confirm_prune_volumes,
    confirm_remove_container, container_rows, image_rows, volume_rows,
};
use telima_core::container::{Container, ContainerState, Image, LogLine, PruneReport, Volume};
use telima_core::profile::ProfileName;

use crate::shell::Tab;

mod render;

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
    RemoveContainer { id: String, force: bool },
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
    pub follow: bool,
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
    notice: Option<String>,
    log: Option<LogPanel>,
    log_job: Option<Job>,
    streams: u64,
    _poll: Task<()>,
}

impl EnginePanel {
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
            log: None,
            log_job: None,
            streams: 0,
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

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
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
                Err(e) => ListState::Failed(e.to_string()),
            }
        }
        match msg {
            Msg::Containers(r) => self.containers = list(r, container_rows),
            Msg::Images(r) => self.images = list(r, image_rows),
            Msg::Volumes(r) => self.volumes = list(r, volume_rows),
            Msg::ImagePreview(Ok(list)) => {
                self.dialog = Some(Dialog {
                    confirm: confirm_prune_images(&list),
                    items: list.into_iter().map(|i| i.id).collect(),
                    action: Action::PruneImages,
                });
            }
            Msg::VolumePreview(Ok(list)) => {
                self.dialog = Some(Dialog {
                    confirm: confirm_prune_volumes(&list),
                    items: list.into_iter().map(|v| v.name).collect(),
                    action: Action::PruneVolumes,
                });
            }
            Msg::ImagePreview(Err(e)) | Msg::VolumePreview(Err(e)) => {
                self.notice = Some(e.to_string());
            }
            Msg::Done(tab, r) => {
                if let Err(e) = r {
                    self.notice = Some(e.to_string());
                }
                self.load(tab, cx);
            }
            Msg::Pruned(tab, r) => {
                if let Err(e) = r {
                    self.notice = Some(e.to_string());
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
                    log.error = r.err().map(|e| e.to_string());
                }
            }
        }
    }

    fn container(&self, id: &str) -> Option<&ContainerRow> {
        match &self.containers {
            ListState::Ready(rows) => rows.iter().find(|r| r.id == id),
            _ => None,
        }
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
        self.dialog = Some(Dialog {
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

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        cx.notify();
    }

    /// Opens the log viewer for a container, following new lines.
    pub fn open_logs(&mut self, id: &str, cx: &mut Context<Self>) {
        let name = self
            .container(id)
            .map_or_else(|| id.to_owned(), |r| r.name.clone());
        self.stream_logs(id.to_owned(), name, true, cx);
    }

    /// Turns follow on or off; the stream restarts from the start of the log.
    pub fn set_follow(&mut self, follow: bool, cx: &mut Context<Self>) {
        let Some(log) = self.log.as_ref() else {
            return;
        };
        if log.follow == follow {
            return;
        }
        let (id, name) = (log.container.clone(), log.name.clone());
        self.stream_logs(id, name, follow, cx);
    }

    pub fn close_logs(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = self.log_job.take() {
            job.cancel();
        }
        self.log = None;
        cx.notify();
    }

    fn stream_logs(&mut self, id: String, name: String, follow: bool, cx: &mut Context<Self>) {
        self.close_logs(cx);
        let Some(profile) = self.profile.clone() else {
            return;
        };
        self.streams += 1;
        let stream = self.streams;
        self.log = Some(LogPanel {
            container: id.clone(),
            name,
            follow,
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
