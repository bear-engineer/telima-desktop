//! Port traits implemented by adapters (Colima CLI, Docker Engine API).
//! Futures are `Send` so adapters can run on a background runtime.
use std::future::Future;

use crate::container::{Container, DiskUsage, Image, LogLine, PruneReport, Volume};
use crate::profile::{Profile, ProfileName, ProfileSpec, ProgressEvent};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PortError {
    /// The tool or socket is not there (Colima not installed, VM stopped, ...).
    #[error("unavailable: {0}")]
    Unavailable(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Failed(String),
}

pub type PortResult<T> = Result<T, PortError>;

/// Receives progress lines while a long command runs.
pub type ProgressSink<'a> = &'a mut (dyn FnMut(ProgressEvent) + Send);

/// Manages Colima profiles (VMs).
pub trait VmManager: Send + Sync {
    fn list(&self) -> impl Future<Output = PortResult<Vec<Profile>>> + Send;
    fn spec(&self, name: &ProfileName) -> impl Future<Output = PortResult<ProfileSpec>> + Send;
    fn create(
        &self,
        spec: &ProfileSpec,
        progress: ProgressSink<'_>,
    ) -> impl Future<Output = PortResult<()>> + Send;
    fn start(
        &self,
        name: &ProfileName,
        progress: ProgressSink<'_>,
    ) -> impl Future<Output = PortResult<()>> + Send;
    fn stop(
        &self,
        name: &ProfileName,
        progress: ProgressSink<'_>,
    ) -> impl Future<Output = PortResult<()>> + Send;
    fn restart(
        &self,
        name: &ProfileName,
        progress: ProgressSink<'_>,
    ) -> impl Future<Output = PortResult<()>> + Send;
    /// Persist new settings; they apply on the next start.
    fn update(&self, spec: &ProfileSpec) -> impl Future<Output = PortResult<()>> + Send;
    fn delete(&self, name: &ProfileName) -> impl Future<Output = PortResult<()>> + Send;
}

/// Containers, images and volumes on one profile's engine.
pub trait ContainerEngine: Send + Sync {
    fn containers(&self) -> impl Future<Output = PortResult<Vec<Container>>> + Send;
    fn restart_container(&self, id: &str) -> impl Future<Output = PortResult<()>> + Send;
    fn remove_container(
        &self,
        id: &str,
        force: bool,
    ) -> impl Future<Output = PortResult<()>> + Send;
    /// Stream log lines into `sink` until the container stops (follow) or the backlog ends.
    fn logs(
        &self,
        id: &str,
        follow: bool,
        sink: &mut (dyn FnMut(LogLine) + Send),
    ) -> impl Future<Output = PortResult<()>> + Send;
    fn images(&self) -> impl Future<Output = PortResult<Vec<Image>>> + Send;
    fn remove_image(&self, id: &str) -> impl Future<Output = PortResult<()>> + Send;
    fn prune_images(&self) -> impl Future<Output = PortResult<PruneReport>> + Send;
    fn volumes(&self) -> impl Future<Output = PortResult<Vec<Volume>>> + Send;
    fn remove_volume(&self, name: &str) -> impl Future<Output = PortResult<()>> + Send;
    fn prune_volumes(&self) -> impl Future<Output = PortResult<PruneReport>> + Send;
    fn disk_usage(&self) -> impl Future<Output = PortResult<DiskUsage>> + Send;
}
