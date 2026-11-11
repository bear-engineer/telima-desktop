//! Fake adapters that record calls. Used by unit tests here and by UI tests downstream.
use std::sync::Mutex;

use telima_core::container::{
    Container, DiskUsage, Image, LogLine, LogStream, PruneReport, Volume,
};
use telima_core::ports::{ContainerEngine, PortResult, ProgressSink, VmManager};
use telima_core::profile::{
    DeleteOptions, Profile, ProfileName, ProfileSpec, ProgressEvent, ProgressLevel, RuntimeInfo,
};
use telima_core::version::ColimaVersion;

#[derive(Default)]
pub struct FakeVm {
    pub profiles: Mutex<Vec<Profile>>,
    pub specs: Mutex<Vec<ProfileSpec>>,
    pub calls: Mutex<Vec<String>>,
    /// Lines emitted to the progress sink by start/stop/restart/create.
    pub progress_lines: Vec<String>,
    /// Answer of `runtime_info`.
    pub info: RuntimeInfo,
    /// Answer of `colima_version`; `None` behaves like a missing Colima.
    pub version: Option<ColimaVersion>,
}

impl FakeVm {
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }
    fn emit(&self, progress: ProgressSink<'_>) {
        for line in &self.progress_lines {
            progress(ProgressEvent {
                level: ProgressLevel::Info,
                message: line.clone(),
            });
        }
    }
}

impl VmManager for FakeVm {
    async fn list(&self) -> PortResult<Vec<Profile>> {
        self.record("list".into());
        Ok(self.profiles.lock().unwrap().clone())
    }
    async fn spec(&self, name: &ProfileName) -> PortResult<ProfileSpec> {
        self.record(format!("spec {name}"));
        self.specs
            .lock()
            .unwrap()
            .iter()
            .find(|s| &s.name == name)
            .cloned()
            .ok_or_else(|| telima_core::ports::PortError::NotFound(name.to_string()))
    }
    async fn create(&self, spec: &ProfileSpec, progress: ProgressSink<'_>) -> PortResult<()> {
        self.record(format!("create {}", spec.name));
        self.emit(progress);
        Ok(())
    }
    async fn start(&self, name: &ProfileName, progress: ProgressSink<'_>) -> PortResult<()> {
        self.record(format!("start {name}"));
        self.emit(progress);
        Ok(())
    }
    async fn stop(&self, name: &ProfileName, progress: ProgressSink<'_>) -> PortResult<()> {
        self.record(format!("stop {name}"));
        self.emit(progress);
        Ok(())
    }
    async fn restart(&self, name: &ProfileName, progress: ProgressSink<'_>) -> PortResult<()> {
        self.record(format!("restart {name}"));
        self.emit(progress);
        Ok(())
    }
    async fn update(&self, spec: &ProfileSpec) -> PortResult<()> {
        self.record(format!("update {}", spec.name));
        Ok(())
    }
    async fn delete(&self, name: &ProfileName, options: DeleteOptions) -> PortResult<()> {
        let data = if options.data { " --data" } else { "" };
        self.record(format!("delete {name}{data}"));
        Ok(())
    }
    async fn colima_version(&self) -> PortResult<ColimaVersion> {
        self.record("colima_version".into());
        self.version.ok_or_else(|| {
            telima_core::ports::PortError::Unavailable("colima is not installed".into())
        })
    }
    async fn runtime_info(&self, name: &ProfileName) -> PortResult<RuntimeInfo> {
        self.record(format!("runtime_info {name}"));
        Ok(self.info.clone())
    }
    async fn prune_cache(&self) -> PortResult<()> {
        self.record("prune_cache".into());
        Ok(())
    }
}

#[derive(Default)]
pub struct FakeEngine {
    pub containers: Mutex<Vec<Container>>,
    pub images: Mutex<Vec<Image>>,
    pub volumes: Mutex<Vec<Volume>>,
    pub log_lines: Vec<String>,
    pub calls: Mutex<Vec<String>>,
}

impl FakeEngine {
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }
}

impl ContainerEngine for FakeEngine {
    async fn containers(&self) -> PortResult<Vec<Container>> {
        Ok(self.containers.lock().unwrap().clone())
    }
    async fn restart_container(&self, id: &str) -> PortResult<()> {
        self.record(format!("restart {id}"));
        Ok(())
    }
    async fn start_container(&self, id: &str) -> PortResult<()> {
        self.record(format!("start {id}"));
        Ok(())
    }
    async fn stop_container(&self, id: &str) -> PortResult<()> {
        self.record(format!("stop {id}"));
        Ok(())
    }
    async fn remove_container(&self, id: &str, force: bool) -> PortResult<()> {
        self.record(format!("remove {id} force={force}"));
        Ok(())
    }
    async fn logs(
        &self,
        id: &str,
        follow: bool,
        sink: &mut (dyn FnMut(LogLine) + Send),
    ) -> PortResult<()> {
        self.record(format!("logs {id} follow={follow}"));
        for l in &self.log_lines {
            sink(LogLine {
                stream: LogStream::Stdout,
                text: l.clone(),
                timestamp: None,
            });
        }
        Ok(())
    }
    async fn images(&self) -> PortResult<Vec<Image>> {
        Ok(self.images.lock().unwrap().clone())
    }
    async fn remove_image(&self, id: &str) -> PortResult<()> {
        self.record(format!("remove_image {id}"));
        Ok(())
    }
    async fn prune_images(&self) -> PortResult<PruneReport> {
        self.record("prune_images".into());
        Ok(PruneReport::default())
    }
    async fn volumes(&self) -> PortResult<Vec<Volume>> {
        Ok(self.volumes.lock().unwrap().clone())
    }
    async fn remove_volume(&self, name: &str) -> PortResult<()> {
        self.record(format!("remove_volume {name}"));
        Ok(())
    }
    async fn prune_volumes(&self) -> PortResult<PruneReport> {
        self.record("prune_volumes".into());
        Ok(PruneReport::default())
    }
    async fn disk_usage(&self) -> PortResult<DiskUsage> {
        Ok(DiskUsage::default())
    }
}
