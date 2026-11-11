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

#[cfg(test)]
mod tests {
    //! The traits have no logic; these tests pin the shape adapters and use cases build on.
    use super::*;
    use std::pin::pin;
    use std::sync::Mutex;
    use std::task::{Context, Poll, Waker};

    use crate::profile::RuntimeInfo;
    use crate::time::UnixMillis;
    use crate::version::ColimaVersion;

    fn block_on<F: Future>(fut: F) -> F::Output {
        let mut fut = pin!(fut);
        let mut cx = Context::from_waker(Waker::noop());
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(out) => out,
            Poll::Pending => panic!("test futures never wait"),
        }
    }

    fn name(raw: &str) -> ProfileName {
        ProfileName::parse(raw).unwrap()
    }

    #[derive(Default)]
    struct Recorder(Mutex<Vec<String>>);

    impl Recorder {
        fn note(&self, call: String) {
            self.0.lock().unwrap().push(call);
        }
        fn calls(&self) -> Vec<String> {
            self.0.lock().unwrap().clone()
        }
    }

    impl VmManager for Recorder {
        async fn list(&self) -> PortResult<Vec<Profile>> {
            Ok(Vec::new())
        }
        async fn spec(&self, name: &ProfileName) -> PortResult<ProfileSpec> {
            Err(PortError::NotFound(name.to_string()))
        }
        async fn create(&self, _: &ProfileSpec, _: ProgressSink<'_>) -> PortResult<()> {
            Ok(())
        }
        async fn start(&self, _: &ProfileName, _: ProgressSink<'_>) -> PortResult<()> {
            Ok(())
        }
        async fn stop(&self, _: &ProfileName, _: ProgressSink<'_>) -> PortResult<()> {
            Ok(())
        }
        async fn restart(&self, _: &ProfileName, _: ProgressSink<'_>) -> PortResult<()> {
            Ok(())
        }
        async fn update(&self, _: &ProfileSpec) -> PortResult<()> {
            Ok(())
        }
        async fn delete(&self, name: &ProfileName, options: DeleteOptions) -> PortResult<()> {
            self.note(format!("delete {name} data={}", options.data));
            Ok(())
        }
        async fn colima_version(&self) -> PortResult<ColimaVersion> {
            Ok(ColimaVersion::new(0, 10, 3))
        }
        async fn runtime_info(&self, name: &ProfileName) -> PortResult<RuntimeInfo> {
            self.note(format!("runtime_info {name}"));
            Ok(RuntimeInfo {
                started_at: Some(UnixMillis::from_secs(7)),
                colima: Some(ColimaVersion::new(0, 10, 3)),
                docker_client: Some("29.9.3".into()),
                docker_server: None,
            })
        }
        async fn prune_cache(&self) -> PortResult<()> {
            self.note("prune_cache".into());
            Ok(())
        }
    }

    impl ContainerEngine for Recorder {
        async fn containers(&self) -> PortResult<Vec<Container>> {
            Ok(Vec::new())
        }
        async fn start_container(&self, id: &str) -> PortResult<()> {
            self.note(format!("start {id}"));
            Ok(())
        }
        async fn stop_container(&self, id: &str) -> PortResult<()> {
            self.note(format!("stop {id}"));
            Ok(())
        }
        async fn restart_container(&self, _: &str) -> PortResult<()> {
            Ok(())
        }
        async fn remove_container(&self, _: &str, _: bool) -> PortResult<()> {
            Ok(())
        }
        async fn logs(
            &self,
            _: &str,
            _: bool,
            _: &mut (dyn FnMut(LogLine) + Send),
        ) -> PortResult<()> {
            Ok(())
        }
        async fn images(&self) -> PortResult<Vec<Image>> {
            Ok(Vec::new())
        }
        async fn remove_image(&self, _: &str) -> PortResult<()> {
            Ok(())
        }
        async fn prune_images(&self) -> PortResult<PruneReport> {
            Ok(PruneReport::default())
        }
        async fn volumes(&self) -> PortResult<Vec<Volume>> {
            Ok(Vec::new())
        }
        async fn remove_volume(&self, _: &str) -> PortResult<()> {
            Ok(())
        }
        async fn prune_volumes(&self) -> PortResult<PruneReport> {
            Ok(PruneReport::default())
        }
        async fn disk_usage(&self) -> PortResult<DiskUsage> {
            Ok(DiskUsage::default())
        }
    }

    #[test]
    fn delete_passes_the_data_option_through() {
        let vm = Recorder::default();
        block_on(vm.delete(&name("telima-test-a"), DeleteOptions::default())).unwrap();
        block_on(vm.delete(&name("telima-test-a"), DeleteOptions { data: true })).unwrap();
        assert_eq!(
            vm.calls(),
            [
                "delete telima-test-a data=false",
                "delete telima-test-a data=true"
            ]
        );
    }

    #[test]
    fn vm_reports_version_and_runtime_info_and_prunes_the_cache() {
        let vm = Recorder::default();
        assert_eq!(
            block_on(vm.colima_version()),
            Ok(ColimaVersion::new(0, 10, 3))
        );
        let info = block_on(vm.runtime_info(&name("telima-test-a"))).unwrap();
        assert_eq!(info.started_at, Some(UnixMillis::from_secs(7)));
        assert_eq!(info.docker_server, None);
        block_on(vm.prune_cache()).unwrap();
        assert_eq!(vm.calls(), ["runtime_info telima-test-a", "prune_cache"]);
    }

    #[test]
    fn engine_starts_and_stops_a_container_by_id() {
        let engine = Recorder::default();
        block_on(engine.start_container("abc")).unwrap();
        block_on(engine.stop_container("abc")).unwrap();
        assert_eq!(engine.calls(), ["start abc", "stop abc"]);
    }
}
