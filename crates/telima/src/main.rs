//! Composition root: builds adapters, injects them, starts the GPUI application.

use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::AppContext;
use telima_app::runtime::{App, BoxFuture, DynEngine, DynVm, Spawner};
use telima_colima::cli::{ColimaCli, TokioRunner};
use telima_core::ports::PortError;
use telima_core::profile::{HostOs, ProfileName};
use telima_docker::DockerEngine;
use telima_ui::shell::MainView;

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

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("start tokio runtime");
    let handle = runtime.handle().clone();

    let home = home_dir();
    let vm: Arc<dyn DynVm> = Arc::new(colima(home.as_ref()));
    let spawner: Arc<dyn Spawner> = Arc::new(TokioSpawner(handle.clone()));
    let engine_home = home.clone();
    let engine_handle = handle.clone();
    let app = App::new(
        vm,
        HOST_OS,
        move |profile| engine_for(&engine_handle, engine_home.as_ref(), profile),
        spawner,
    );

    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        telima_ui::init(cx);
        let options = telima_ui::main_window_options(cx);
        let app = app.clone();
        gpui_kit::open_window(options, cx, move |_, cx| {
            cx.new(|cx| MainView::new(app.clone(), cx))
        })
        .expect("open main window");
    });

    // Keep the runtime alive until the GPUI loop returns.
    drop(runtime);
}
