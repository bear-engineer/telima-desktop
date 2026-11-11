//! Composition root: builds adapters, injects them, starts the GPUI application.

mod dev_override;
mod tray;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::{AppContext, QuitMode};
use telima_app::profiles::Lifecycle;
use telima_app::runtime::{App, BoxFuture, DynEngine, DynVm, PROFILE_POLL_INTERVAL, Spawner};
use telima_app::view::tray::{BusyOp, TrayError, tray_model};
use telima_app::{AppCommand, AppError};
use telima_colima::cli::{ColimaCli, TokioRunner};
use telima_core::ports::PortError;
use telima_core::profile::{HostOs, Profile, ProfileName};
use telima_docker::DockerEngine;
use telima_tray::{TrayHandle, TrayIcons};
use telima_ui::shell::MainView;

/// How often the tray timer looks for menu commands.
const TICK: Duration = Duration::from_millis(200);

#[cfg(target_os = "macos")]
const HOST_OS: HostOs = HostOs::MacOs;
#[cfg(target_os = "linux")]
const HOST_OS: HostOs = HostOs::Linux;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
compile_error!("telima supports macOS and Linux only");

/// Runs use-case futures on the binary's tokio runtime. GPUI never polls tokio futures itself.
struct TokioSpawner(tokio::runtime::Handle);

impl Spawner for TokioSpawner {
    fn spawn(&self, work: BoxFuture<'static, ()>) {
        drop(self.0.spawn(work));
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// Colima adapter. When discovery fails, the CLI is pointed at a missing binary so every call
/// surfaces `PortError::Unavailable` and the UI shows the not-installed state.
/// The main window of this process, if it is open.
fn main_window(cx: &gpui_kit::App) -> Option<gpui_kit::WindowHandle<MainView>> {
    cx.windows()
        .into_iter()
        .find_map(|w| w.downcast::<MainView>())
}

fn open_main_window(app: &App, cx: &mut gpui_kit::App) {
    let options = telima_ui::main_window_options(cx);
    let app = app.clone();
    gpui_kit::open_window(options, cx, move |_, cx| {
        cx.new(|cx| MainView::new(app.clone(), cx))
    })
    .expect("open main window");
}

/// Brings the main window to the front, or opens a new one after it was closed.
fn show_main_window(app: &App, cx: &mut gpui_kit::App) {
    match main_window(cx) {
        Some(window) => {
            if window.update(cx, |_, w, _| w.activate_window()).is_err() {
                open_main_window(app, cx);
            }
        }
        None => open_main_window(app, cx),
    }
    cx.activate(true);
}

/// Shows `CLOSE_NOTICE` through the OS notification centre; best effort, errors are ignored.
fn notify_close() {
    use telima_app::view::tray::{CLOSE_NOTICE_BODY, CLOSE_NOTICE_TITLE};
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            CLOSE_NOTICE_BODY.replace('"', "\\\""),
            CLOSE_NOTICE_TITLE
        );
        drop(
            std::process::Command::new("osascript")
                .args(["-e", &script])
                .spawn(),
        );
    }
    #[cfg(target_os = "linux")]
    drop(
        std::process::Command::new("notify-send")
            .args([CLOSE_NOTICE_TITLE, CLOSE_NOTICE_BODY])
            .spawn(),
    );
}

/// Drives the `tray::Shell` plan on the GPUI app.
struct GpuiShell<'a> {
    app: &'a App,
    cx: &'a mut gpui_kit::App,
    tracked: &'a mut tray::Tracked,
    poll_now: &'a mut bool,
}

impl tray::Shell for GpuiShell<'_> {
    fn window_open(&self) -> bool {
        main_window(self.cx).is_some()
    }
    fn show_window(&mut self) {
        show_main_window(self.app, self.cx);
    }
    fn track(&mut self, name: ProfileName, cmd: Lifecycle) {
        self.tracked.insert(name, cmd);
    }
    fn run(&mut self, cmd: &AppCommand) {
        if let Some(window) = main_window(self.cx) {
            window
                .update(self.cx, |view, _, cx| view.run_command(cmd, cx))
                .ok();
        }
    }
    fn poll_now(&mut self) {
        *self.poll_now = true;
    }
    fn quit(&mut self) {
        self.cx.quit();
    }
}

/// Creates the tray and the timer that feeds it. Without a tray host the app keeps its
/// default quit behaviour (macOS: explicit; Linux: last window closed).
fn wire_tray(app: App, cx: &mut gpui_kit::App) {
    let model = tray_model(&[], |_| None, None);
    let Some((mut handle, commands)) = TrayHandle::new(&model, TrayIcons::default()) else {
        eprintln!("telima: no tray available; closing the window quits the app");
        return;
    };
    cx.set_quit_mode(QuitMode::Explicit);
    {
        let app = app.clone();
        REOPEN.with(|r| *r.borrow_mut() = Some(Rc::new(move |cx| show_main_window(&app, cx))));
    }
    let notice = Rc::new(RefCell::new(tray::CloseNotice::default()));
    cx.on_window_closed(move |cx, _| {
        let left = cx.windows().len();
        if notice.borrow_mut().take(true, left) {
            notify_close();
        }
    })
    .detach();

    cx.spawn(async move |cx| {
        let polled: PolledProfiles = Arc::default();
        let mut profiles: Vec<Profile> = Vec::new();
        let mut list_failed = false;
        let mut tracked = tray::Tracked::default();
        let mut poll_now = true;
        let mut last_poll = Instant::now();
        let mut was_busy = false;
        loop {
            cx.background_executor().timer(TICK).await;
            cx.update(|cx| {
                while let Ok(cmd) = commands.try_recv() {
                    let mut shell = GpuiShell {
                        app: &app,
                        cx,
                        tracked: &mut tracked,
                        poll_now: &mut poll_now,
                    };
                    tray::apply(&cmd, &mut shell);
                }
                if let Some(result) = polled.lock().unwrap().take() {
                    list_failed = result.is_err();
                    if let Ok(list) = result {
                        profiles = list;
                    }
                }
                let busy_now = profiles.iter().any(|p| app.is_busy(&p.name));
                // A finished command changes the status: read it right away.
                if was_busy && !busy_now {
                    poll_now = true;
                }
                was_busy = busy_now;
                if poll_now || last_poll.elapsed() >= PROFILE_POLL_INTERVAL {
                    poll_now = false;
                    last_poll = Instant::now();
                    let slot = polled.clone();
                    drop(app.refresh_profiles(move |r| *slot.lock().unwrap() = Some(r)));
                }
                let error = list_failed.then_some(TrayError::List);
                let busy: HashMap<ProfileName, BusyOp> = profiles
                    .iter()
                    .filter_map(|p| {
                        let op = tracked.busy_op(&p.name, app.is_busy(&p.name))?;
                        Some((p.name.clone(), op))
                    })
                    .collect();
                let model = tray_model(&profiles, |n| busy.get(n).copied(), error.as_ref());
                handle.update(&model);
            });
        }
    })
    .detach();
}

fn colima(home: Option<&PathBuf>) -> ColimaCli<TokioRunner> {
    match ColimaCli::discover() {
        Ok(cli) => cli,
        Err(err) => {
            eprintln!("telima: colima unavailable: {err}");
            let config = home.map(|h| h.join(".colima")).unwrap_or_default();
            ColimaCli::new(TokioRunner, PathBuf::from("colima"), config)
        }
    }
}

/// Docker engine of one profile, on its socket under `~/.colima/<profile>/docker.sock`.
fn engine_for(
    runtime: &tokio::runtime::Handle,
    home: Option<&PathBuf>,
    profile: &ProfileName,
) -> Result<Arc<dyn DynEngine>, PortError> {
    let home = home.ok_or_else(|| PortError::Unavailable("HOME is not set".into()))?;
    let socket = home
        .join(".colima")
        .join(profile.as_str())
        .join("docker.sock");
    let socket = socket
        .to_str()
        .ok_or_else(|| PortError::Failed("socket path is not UTF-8".into()))?;
    // bollard's connector expects a tokio context.
    let _guard = runtime.enter();
    Ok(Arc::new(DockerEngine::connect(socket)?))
}

type ReopenFn = Rc<dyn Fn(&mut gpui_kit::App)>;
type PolledProfiles = Arc<Mutex<Option<Result<Vec<Profile>, AppError>>>>;

thread_local! {
    /// Dock click handler; set by `wire_tray`. GPUI runs it on the main thread.
    static REOPEN: RefCell<Option<ReopenFn>> =
        const { std::cell::RefCell::new(None) };
}

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("start tokio runtime");
    let handle = runtime.handle().clone();

    let home = home_dir();
    let vm: Arc<dyn DynVm> = Arc::new(colima(home.as_ref()));
    let version_env = std::env::var("TELIMA_COLIMA_VERSION").ok();
    let vm =
        dev_override::VersionOverride::wrap(vm, cfg!(debug_assertions), version_env.as_deref());
    let spawner: Arc<dyn Spawner> = Arc::new(TokioSpawner(handle.clone()));
    let engine_home = home.clone();
    let engine_handle = handle.clone();
    let app = App::new(
        vm,
        HOST_OS,
        move |profile| engine_for(&engine_handle, engine_home.as_ref(), profile),
        spawner,
    );

    let application = gpui_kit::application();
    application.on_reopen(|cx| {
        if let Some(open) = REOPEN.with(|r| r.borrow().clone()) {
            open(cx);
        }
    });
    application.run(move |cx| {
        gpui_kit::init(cx);
        telima_ui::init(cx);
        open_main_window(&app, cx);
        wire_tray(app.clone(), cx);
    });

    // Keep the runtime alive until the GPUI loop returns.
    drop(runtime);
}
