//! `VmManager` over the `colima` CLI. Process execution sits behind [`CommandRunner`].
use std::future::Future;
use std::path::PathBuf;

use telima_core::ports::{PortError, PortResult, ProgressSink, VmManager};
use telima_core::profile::{Profile, ProfileName, ProfileSpec};

/// Result of a finished process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// Exit code; `None` when killed by a signal.
    pub code: Option<i32>,
    pub stdout: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    #[error("program not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Io(String),
}

/// Runs one process. `argv[0]` is the program. Each stderr line goes to `on_stderr` as it arrives.
pub trait CommandRunner: Send + Sync {
    fn run(
        &self,
        argv: &[String],
        on_stderr: &mut (dyn FnMut(&str) + Send),
    ) -> impl Future<Output = Result<CommandOutput, RunError>> + Send;
}

/// Real runner on tokio.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioRunner;

impl CommandRunner for TokioRunner {
    async fn run(
        &self,
        _argv: &[String],
        _on_stderr: &mut (dyn FnMut(&str) + Send),
    ) -> Result<CommandOutput, RunError> {
        todo!()
    }
}

/// argv (without the program) for `colima start` creating a new profile.
pub fn create_args(_spec: &ProfileSpec) -> Vec<String> {
    todo!()
}

pub struct ColimaCli<R: CommandRunner = TokioRunner> {
    _runner: R,
    _bin: PathBuf,
    _home: PathBuf,
}

impl ColimaCli<TokioRunner> {
    /// Find `colima` (PATH + fixed fallbacks) and use `~/.colima`.
    pub fn discover() -> PortResult<Self> {
        todo!()
    }
}

impl<R: CommandRunner> ColimaCli<R> {
    /// `bin`: colima executable; `home`: the colima config dir (normally `~/.colima`).
    pub fn new(runner: R, bin: PathBuf, home: PathBuf) -> Self {
        Self {
            _runner: runner,
            _bin: bin,
            _home: home,
        }
    }
}

impl<R: CommandRunner> VmManager for ColimaCli<R> {
    async fn list(&self) -> PortResult<Vec<Profile>> {
        Err(PortError::Failed("todo".into()))
    }
    async fn spec(&self, _name: &ProfileName) -> PortResult<ProfileSpec> {
        todo!()
    }
    async fn create(&self, _spec: &ProfileSpec, _progress: ProgressSink<'_>) -> PortResult<()> {
        todo!()
    }
    async fn start(&self, _name: &ProfileName, _progress: ProgressSink<'_>) -> PortResult<()> {
        todo!()
    }
    async fn stop(&self, _name: &ProfileName, _progress: ProgressSink<'_>) -> PortResult<()> {
        todo!()
    }
    async fn restart(&self, _name: &ProfileName, _progress: ProgressSink<'_>) -> PortResult<()> {
        todo!()
    }
    async fn update(&self, _spec: &ProfileSpec) -> PortResult<()> {
        todo!()
    }
    async fn delete(&self, _name: &ProfileName) -> PortResult<()> {
        todo!()
    }
}
