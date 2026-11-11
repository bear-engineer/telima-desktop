//! Async bridge between views and use cases. See docs/adr/0001-async-bridge.md.
//!
//! Views call [`App`] with `Send` callbacks. The work runs on whatever [`Spawner`] the binary
//! supplies (a tokio runtime in production), so this crate and the UI stay free of tokio.
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use futures::future::{AbortHandle, abortable};
use telima_core::container::{Container, DiskUsage, Image, LogLine, PruneReport, Volume};
use telima_core::ports::{ContainerEngine, PortResult, ProgressSink, VmManager};
use telima_core::profile::{HostOs, Profile, ProfileName, ProfileSpec, ProgressEvent};

use crate::AppError;
use crate::engine::EngineService;
use crate::profiles::{Lifecycle, ProfileService};

/// An owned, `Send` future behind a pointer.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Sink for container log lines, the engine-side twin of [`ProgressSink`].
pub type LogSink<'a> = &'a mut (dyn FnMut(LogLine) + Send);

/// Runs background work to completion. The binary implements this over its tokio runtime.
pub trait Spawner: Send + Sync {
    fn spawn(&self, work: BoxFuture<'static, ()>);
}

/// Object-safe form of [`VmManager`]. Every `VmManager` is one.
pub trait DynVm: Send + Sync {
    fn list(&self) -> BoxFuture<'_, PortResult<Vec<Profile>>>;
    fn spec<'a>(&'a self, name: &'a ProfileName) -> BoxFuture<'a, PortResult<ProfileSpec>>;
    fn create<'a>(
        &'a self,
        spec: &'a ProfileSpec,
        progress: ProgressSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>>;
    fn start<'a>(
        &'a self,
        name: &'a ProfileName,
        progress: ProgressSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>>;
    fn stop<'a>(
        &'a self,
        name: &'a ProfileName,
        progress: ProgressSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>>;
    fn restart<'a>(
        &'a self,
        name: &'a ProfileName,
        progress: ProgressSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>>;
    fn update<'a>(&'a self, spec: &'a ProfileSpec) -> BoxFuture<'a, PortResult<()>>;
    fn delete<'a>(&'a self, name: &'a ProfileName) -> BoxFuture<'a, PortResult<()>>;
}

impl<T: VmManager> DynVm for T {
    fn list(&self) -> BoxFuture<'_, PortResult<Vec<Profile>>> {
        Box::pin(VmManager::list(self))
    }
    fn spec<'a>(&'a self, name: &'a ProfileName) -> BoxFuture<'a, PortResult<ProfileSpec>> {
        Box::pin(VmManager::spec(self, name))
    }
    fn create<'a>(
        &'a self,
        spec: &'a ProfileSpec,
        progress: ProgressSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>> {
        Box::pin(VmManager::create(self, spec, progress))
    }
    fn start<'a>(
        &'a self,
        name: &'a ProfileName,
        progress: ProgressSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>> {
        Box::pin(VmManager::start(self, name, progress))
    }
    fn stop<'a>(
        &'a self,
        name: &'a ProfileName,
        progress: ProgressSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>> {
        Box::pin(VmManager::stop(self, name, progress))
    }
    fn restart<'a>(
        &'a self,
        name: &'a ProfileName,
        progress: ProgressSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>> {
        Box::pin(VmManager::restart(self, name, progress))
    }
    fn update<'a>(&'a self, spec: &'a ProfileSpec) -> BoxFuture<'a, PortResult<()>> {
        Box::pin(VmManager::update(self, spec))
    }
    fn delete<'a>(&'a self, name: &'a ProfileName) -> BoxFuture<'a, PortResult<()>> {
        Box::pin(VmManager::delete(self, name))
    }
}

/// Object-safe form of [`ContainerEngine`]. Every `ContainerEngine` is one.
pub trait DynEngine: Send + Sync {
    fn containers(&self) -> BoxFuture<'_, PortResult<Vec<Container>>>;
    fn restart_container<'a>(&'a self, id: &'a str) -> BoxFuture<'a, PortResult<()>>;
    fn remove_container<'a>(&'a self, id: &'a str, force: bool) -> BoxFuture<'a, PortResult<()>>;
    fn logs<'a>(
        &'a self,
        id: &'a str,
        follow: bool,
        sink: LogSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>>;
    fn images(&self) -> BoxFuture<'_, PortResult<Vec<Image>>>;
    fn remove_image<'a>(&'a self, id: &'a str) -> BoxFuture<'a, PortResult<()>>;
    fn prune_images(&self) -> BoxFuture<'_, PortResult<PruneReport>>;
    fn volumes(&self) -> BoxFuture<'_, PortResult<Vec<Volume>>>;
    fn remove_volume<'a>(&'a self, name: &'a str) -> BoxFuture<'a, PortResult<()>>;
    fn prune_volumes(&self) -> BoxFuture<'_, PortResult<PruneReport>>;
    fn disk_usage(&self) -> BoxFuture<'_, PortResult<DiskUsage>>;
}

impl<T: ContainerEngine> DynEngine for T {
    fn containers(&self) -> BoxFuture<'_, PortResult<Vec<Container>>> {
        Box::pin(ContainerEngine::containers(self))
    }
    fn restart_container<'a>(&'a self, id: &'a str) -> BoxFuture<'a, PortResult<()>> {
        Box::pin(ContainerEngine::restart_container(self, id))
    }
    fn remove_container<'a>(&'a self, id: &'a str, force: bool) -> BoxFuture<'a, PortResult<()>> {
        Box::pin(ContainerEngine::remove_container(self, id, force))
    }
    fn logs<'a>(
        &'a self,
        id: &'a str,
        follow: bool,
        sink: LogSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>> {
        Box::pin(ContainerEngine::logs(self, id, follow, sink))
    }
    fn images(&self) -> BoxFuture<'_, PortResult<Vec<Image>>> {
        Box::pin(ContainerEngine::images(self))
    }
    fn remove_image<'a>(&'a self, id: &'a str) -> BoxFuture<'a, PortResult<()>> {
        Box::pin(ContainerEngine::remove_image(self, id))
    }
    fn prune_images(&self) -> BoxFuture<'_, PortResult<PruneReport>> {
        Box::pin(ContainerEngine::prune_images(self))
    }
    fn volumes(&self) -> BoxFuture<'_, PortResult<Vec<Volume>>> {
        Box::pin(ContainerEngine::volumes(self))
    }
    fn remove_volume<'a>(&'a self, name: &'a str) -> BoxFuture<'a, PortResult<()>> {
        Box::pin(ContainerEngine::remove_volume(self, name))
    }
    fn prune_volumes(&self) -> BoxFuture<'_, PortResult<PruneReport>> {
        Box::pin(ContainerEngine::prune_volumes(self))
    }
    fn disk_usage(&self) -> BoxFuture<'_, PortResult<DiskUsage>> {
        Box::pin(ContainerEngine::disk_usage(self))
    }
}

/// Lets the generic use cases run over a type-erased VM manager.
struct SharedVm(Arc<dyn DynVm>);

impl VmManager for SharedVm {
    async fn list(&self) -> PortResult<Vec<Profile>> {
        self.0.list().await
    }
    async fn spec(&self, name: &ProfileName) -> PortResult<ProfileSpec> {
        self.0.spec(name).await
    }
    async fn create(&self, spec: &ProfileSpec, progress: ProgressSink<'_>) -> PortResult<()> {
        self.0.create(spec, progress).await
    }
    async fn start(&self, name: &ProfileName, progress: ProgressSink<'_>) -> PortResult<()> {
        self.0.start(name, progress).await
    }
    async fn stop(&self, name: &ProfileName, progress: ProgressSink<'_>) -> PortResult<()> {
        self.0.stop(name, progress).await
    }
    async fn restart(&self, name: &ProfileName, progress: ProgressSink<'_>) -> PortResult<()> {
        self.0.restart(name, progress).await
    }
    async fn update(&self, spec: &ProfileSpec) -> PortResult<()> {
        self.0.update(spec).await
    }
    async fn delete(&self, name: &ProfileName) -> PortResult<()> {
        self.0.delete(name).await
    }
}

/// Lets the generic use cases run over a type-erased engine.
struct SharedEngine(Arc<dyn DynEngine>);

impl ContainerEngine for SharedEngine {
    async fn containers(&self) -> PortResult<Vec<Container>> {
        self.0.containers().await
    }
    async fn restart_container(&self, id: &str) -> PortResult<()> {
        self.0.restart_container(id).await
    }
    async fn remove_container(&self, id: &str, force: bool) -> PortResult<()> {
        self.0.remove_container(id, force).await
    }
    async fn logs(&self, id: &str, follow: bool, sink: LogSink<'_>) -> PortResult<()> {
        self.0.logs(id, follow, sink).await
    }
    async fn images(&self) -> PortResult<Vec<Image>> {
        self.0.images().await
    }
    async fn remove_image(&self, id: &str) -> PortResult<()> {
        self.0.remove_image(id).await
    }
    async fn prune_images(&self) -> PortResult<PruneReport> {
        self.0.prune_images().await
    }
    async fn volumes(&self) -> PortResult<Vec<Volume>> {
        self.0.volumes().await
    }
    async fn remove_volume(&self, name: &str) -> PortResult<()> {
        self.0.remove_volume(name).await
    }
    async fn prune_volumes(&self) -> PortResult<PruneReport> {
        self.0.prune_volumes().await
    }
    async fn disk_usage(&self) -> PortResult<DiskUsage> {
        self.0.disk_usage().await
    }
}

/// Handle to work started through [`App`]. Dropping it lets the work finish.
#[derive(Debug, Clone)]
pub struct Task(AbortHandle);

impl Task {
    /// Stop the work at its next await point. The `done` callback is not called afterwards.
    pub fn cancel(&self) {
        self.0.abort();
    }
}

type EngineFactory = dyn Fn(&ProfileName) -> PortResult<Arc<dyn DynEngine>> + Send + Sync;

struct Inner {
    os: HostOs,
    profiles: ProfileService<SharedVm>,
    engine_for: Box<EngineFactory>,
    spawner: Arc<dyn Spawner>,
}

impl Inner {
    fn engine(&self, profile: &ProfileName) -> Result<EngineService<SharedEngine>, AppError> {
        Ok(EngineService::new(SharedEngine((self.engine_for)(
            profile,
        )?)))
    }
}

/// Everything a view may ask of the application. Cheap to clone, safe to share across threads.
///
/// Each method returns at once; the use case runs on the [`Spawner`] and `done` receives the
/// result there. Callbacks run on the spawner's thread, so they hand the value to the UI thread
/// (a channel) instead of touching view state.
#[derive(Clone)]
pub struct App {
    inner: Arc<Inner>,
}

impl App {
    /// `engine_for` returns the engine of one profile (each profile has its own socket).
    pub fn new(
        vm: Arc<dyn DynVm>,
        os: HostOs,
        engine_for: impl Fn(&ProfileName) -> PortResult<Arc<dyn DynEngine>> + Send + Sync + 'static,
        spawner: Arc<dyn Spawner>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                os,
                profiles: ProfileService::new(SharedVm(vm), os),
                engine_for: Box::new(engine_for),
                spawner,
            }),
        }
    }

    fn run<T, F>(
        &self,
        work: impl FnOnce(Arc<Inner>) -> F + Send + 'static,
        done: impl FnOnce(Result<T, AppError>) + Send + 'static,
    ) -> Task
    where
        F: Future<Output = Result<T, AppError>> + Send,
    {
        let inner = self.inner.clone();
        let (work, handle) = abortable(async move { done(work(inner).await) });
        self.inner.spawner.spawn(Box::pin(async move {
            // Err means the task was cancelled; there is nobody left to tell.
            let _ = work.await;
        }));
        Task(handle)
    }

    /// True while an operation runs on `name`.
    pub fn is_busy(&self, name: &ProfileName) -> bool {
        self.inner.profiles.is_busy(name)
    }

    /// Host OS the profile rules are checked against (the editor needs it).
    pub fn host_os(&self) -> HostOs {
        self.inner.os
    }

    /// Stored spec of `name` (`VmManager::spec`), the starting point of the edit form.
    pub fn profile_spec(
        &self,
        name: ProfileName,
        done: impl FnOnce(Result<ProfileSpec, AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            |app| async move { Ok(VmManager::spec(app.profiles.vm(), &name).await?) },
            done,
        )
    }

    pub fn list_profiles(
        &self,
        done: impl FnOnce(Result<Vec<Profile>, AppError>) + Send + 'static,
    ) -> Task {
        self.run(|app| async move { app.profiles.list().await }, done)
    }

    pub fn create_profile(
        &self,
        spec: ProfileSpec,
        mut progress: impl FnMut(ProgressEvent) + Send + 'static,
        done: impl FnOnce(Result<(), AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            |app| async move { app.profiles.create(&spec, &mut progress).await },
            done,
        )
    }

    pub fn run_profile(
        &self,
        name: ProfileName,
        cmd: Lifecycle,
        mut progress: impl FnMut(ProgressEvent) + Send + 'static,
        done: impl FnOnce(Result<(), AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            move |app| async move { app.profiles.run(&name, cmd, &mut progress).await },
            done,
        )
    }

    pub fn update_profile(
        &self,
        next: ProfileSpec,
        done: impl FnOnce(Result<(), AppError>) + Send + 'static,
    ) -> Task {
        self.run(|app| async move { app.profiles.update(&next).await }, done)
    }

    /// Deletes only when `typed` equals the profile name.
    pub fn delete_profile(
        &self,
        name: ProfileName,
        typed: String,
        done: impl FnOnce(Result<(), AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            |app| async move { app.profiles.delete(&name, &typed).await },
            done,
        )
    }

    pub fn containers(
        &self,
        profile: ProfileName,
        done: impl FnOnce(Result<Vec<Container>, AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            |app| async move { app.engine(&profile)?.containers().await },
            done,
        )
    }

    pub fn restart_container(
        &self,
        profile: ProfileName,
        id: String,
        done: impl FnOnce(Result<(), AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            |app| async move { app.engine(&profile)?.restart_container(&id).await },
            done,
        )
    }

    /// A running container is removed only with `force`.
    pub fn remove_container(
        &self,
        profile: ProfileName,
        id: String,
        force: bool,
        done: impl FnOnce(Result<(), AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            move |app| async move { app.engine(&profile)?.remove_container(&id, force).await },
            done,
        )
    }

    /// Streams log lines to `line`. With `follow`, runs until the container stops or the
    /// returned [`Task`] is cancelled.
    pub fn logs(
        &self,
        profile: ProfileName,
        id: String,
        follow: bool,
        mut line: impl FnMut(LogLine) + Send + 'static,
        done: impl FnOnce(Result<(), AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            move |app| async move { app.engine(&profile)?.logs(&id, follow, &mut line).await },
            done,
        )
    }

    pub fn images(
        &self,
        profile: ProfileName,
        done: impl FnOnce(Result<Vec<Image>, AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            |app| async move { app.engine(&profile)?.images().await },
            done,
        )
    }

    pub fn image_prune_preview(
        &self,
        profile: ProfileName,
        done: impl FnOnce(Result<Vec<Image>, AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            |app| async move { app.engine(&profile)?.image_prune_preview().await },
            done,
        )
    }

    pub fn prune_images(
        &self,
        profile: ProfileName,
        done: impl FnOnce(Result<PruneReport, AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            |app| async move { app.engine(&profile)?.prune_images().await },
            done,
        )
    }

    pub fn volumes(
        &self,
        profile: ProfileName,
        done: impl FnOnce(Result<Vec<Volume>, AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            |app| async move { app.engine(&profile)?.volumes().await },
            done,
        )
    }

    pub fn volume_prune_preview(
        &self,
        profile: ProfileName,
        done: impl FnOnce(Result<Vec<Volume>, AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            |app| async move { app.engine(&profile)?.volume_prune_preview().await },
            done,
        )
    }

    /// Removes a volume that no container references.
    pub fn remove_volume(
        &self,
        profile: ProfileName,
        name: String,
        done: impl FnOnce(Result<(), AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            |app| async move { app.engine(&profile)?.remove_volume(&name).await },
            done,
        )
    }

    pub fn prune_volumes(
        &self,
        profile: ProfileName,
        done: impl FnOnce(Result<PruneReport, AppError>) + Send + 'static,
    ) -> Task {
        self.run(
            |app| async move { app.engine(&profile)?.prune_volumes().await },
            done,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use futures::executor::block_on;
    use telima_core::container::{Container, ContainerState};
    use telima_core::ports::PortError;
    use telima_core::profile::{
        HostOs, MountType, Profile, ProfileName, ProfileSpec, ProfileStatus, Runtime, VmType,
    };

    use super::*;
    use crate::AppError;
    use crate::profiles::Lifecycle;
    use crate::testing::{FakeEngine, FakeVm};

    /// Holds spawned work until the test runs it, like a view waiting on the background runtime.
    #[derive(Default)]
    struct QueueSpawner(Mutex<Vec<BoxFuture<'static, ()>>>);

    impl Spawner for QueueSpawner {
        fn spawn(&self, work: BoxFuture<'static, ()>) {
            self.0.lock().unwrap().push(work);
        }
    }

    impl QueueSpawner {
        fn run_all(&self) {
            let queued = std::mem::take(&mut *self.0.lock().unwrap());
            for work in queued {
                block_on(work);
            }
        }
    }

    /// Runs each piece of work on its own thread, like the binary's runtime does.
    struct ThreadSpawner;

    impl Spawner for ThreadSpawner {
        fn spawn(&self, work: BoxFuture<'static, ()>) {
            std::thread::spawn(move || block_on(work));
        }
    }

    struct Fixture {
        app: App,
        vm: Arc<FakeVm>,
        engine: Arc<FakeEngine>,
        spawner: Arc<QueueSpawner>,
        /// Profile names the engine factory was asked for.
        asked: Arc<Mutex<Vec<String>>>,
    }

    fn fixture(vm: FakeVm, engine: FakeEngine, os: HostOs) -> Fixture {
        let vm = Arc::new(vm);
        let engine = Arc::new(engine);
        let spawner = Arc::new(QueueSpawner::default());
        let asked = Arc::new(Mutex::new(Vec::new()));
        let app = App::new(
            vm.clone(),
            os,
            {
                let engine = engine.clone();
                let asked = asked.clone();
                move |profile: &ProfileName| {
                    asked.lock().unwrap().push(profile.to_string());
                    let engine: Arc<dyn DynEngine> = engine.clone();
                    Ok(engine)
                }
            },
            spawner.clone(),
        );
        Fixture {
            app,
            vm,
            engine,
            spawner,
            asked,
        }
    }

    fn mac() -> Fixture {
        fixture(FakeVm::default(), FakeEngine::default(), HostOs::MacOs)
    }

    /// A place for a callback to put its value, and the callback that fills it.
    fn slot<T: Send + 'static>() -> (Arc<Mutex<Option<T>>>, impl FnOnce(T) + Send + 'static) {
        let slot = Arc::new(Mutex::new(None));
        let fill = slot.clone();
        (slot, move |value| *fill.lock().unwrap() = Some(value))
    }

    fn name(s: &str) -> ProfileName {
        ProfileName::parse(s).unwrap()
    }

    fn profile(n: &str) -> Profile {
        Profile {
            name: name(n),
            status: ProfileStatus::Stopped,
            arch: "aarch64".into(),
            cpus: 2,
            memory_bytes: 2 << 30,
            disk_bytes: 20 << 30,
            address: None,
            runtime: Runtime::Docker,
        }
    }

    fn spec(n: &str) -> ProfileSpec {
        ProfileSpec {
            name: name(n),
            cpus: 2,
            memory_gib: 2,
            disk_gib: 20,
            vm_type: VmType::Vz,
            mount_type: MountType::Virtiofs,
            runtime: Runtime::Docker,
            rosetta: true,
            network_address: false,
        }
    }

    fn container(id: &str, name: &str, state: ContainerState) -> Container {
        Container {
            id: id.into(),
            name: name.into(),
            image: "alpine:3".into(),
            state,
            status_text: String::new(),
        }
    }

    #[test]
    fn app_is_clone_send_sync() {
        fn check<T: Clone + Send + Sync + 'static>() {}
        check::<App>();
        check::<Task>();
    }

    #[test]
    fn list_profiles_runs_on_the_spawner_and_delivers_sorted_result() {
        let f = mac();
        *f.vm.profiles.lock().unwrap() = vec![profile("zeta"), profile("alpha")];
        let (got, done) = slot();
        f.app.list_profiles(done);
        assert!(f.vm.calls().is_empty(), "nothing runs on the caller");
        assert!(got.lock().unwrap().is_none());
        f.spawner.run_all();
        let names: Vec<_> = got
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|p| p.name.to_string())
            .collect();
        assert_eq!(names, ["alpha", "zeta"]);
    }

    #[test]
    fn run_profile_streams_progress_then_reports_done() {
        let vm = FakeVm {
            progress_lines: vec!["booting".into(), "ready".into()],
            ..Default::default()
        };
        let f = fixture(vm, FakeEngine::default(), HostOs::MacOs);
        let lines = Arc::new(Mutex::new(Vec::new()));
        let (got, done) = slot();
        f.app.run_profile(
            name("dev"),
            Lifecycle::Start,
            {
                let lines = lines.clone();
                move |e| lines.lock().unwrap().push(e.message)
            },
            done,
        );
        f.spawner.run_all();
        assert_eq!(*lines.lock().unwrap(), ["booting", "ready"]);
        assert_eq!(got.lock().unwrap().take(), Some(Ok(())));
        assert_eq!(f.vm.calls(), ["start dev"]);
        assert!(!f.app.is_busy(&name("dev")));
    }

    #[test]
    fn create_profile_with_invalid_spec_reports_error_and_never_reaches_vm() {
        let f = fixture(FakeVm::default(), FakeEngine::default(), HostOs::Linux);
        let (got, done) = slot();
        f.app.create_profile(spec("telima-test-a"), |_| {}, done);
        f.spawner.run_all();
        assert!(matches!(
            got.lock().unwrap().take(),
            Some(Err(AppError::Invalid(_)))
        ));
        assert!(f.vm.calls().is_empty());
    }

    #[test]
    fn update_and_delete_reach_vm() {
        let f = mac();
        f.vm.specs.lock().unwrap().push(spec("dev"));
        let (updated, done) = slot();
        f.app.update_profile(
            ProfileSpec {
                memory_gib: 8,
                ..spec("dev")
            },
            done,
        );
        let (refused, done) = slot();
        f.app.delete_profile(name("dev"), "de".into(), done);
        let (deleted, done) = slot();
        f.app.delete_profile(name("dev"), "dev".into(), done);
        f.spawner.run_all();
        assert_eq!(updated.lock().unwrap().take(), Some(Ok(())));
        assert_eq!(
            refused.lock().unwrap().take(),
            Some(Err(AppError::ConfirmationMismatch))
        );
        assert_eq!(deleted.lock().unwrap().take(), Some(Ok(())));
        assert_eq!(f.vm.calls(), ["spec dev", "update dev", "delete dev"]);
    }

    #[test]
    fn containers_use_the_engine_of_the_named_profile() {
        let f = mac();
        *f.engine.containers.lock().unwrap() = vec![
            container("1", "b-exited", ContainerState::Exited),
            container("2", "z-running", ContainerState::Running),
        ];
        let (got, done) = slot();
        f.app.containers(name("dev"), done);
        f.spawner.run_all();
        let names: Vec<_> = got
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, ["z-running", "b-exited"]);
        assert_eq!(*f.asked.lock().unwrap(), ["dev"]);
    }

    #[test]
    fn logs_stream_lines_to_the_callback() {
        let engine = FakeEngine {
            log_lines: vec!["hello".into(), "world".into()],
            ..Default::default()
        };
        let f = fixture(FakeVm::default(), engine, HostOs::MacOs);
        let lines = Arc::new(Mutex::new(Vec::new()));
        let (got, done) = slot();
        f.app.logs(
            name("dev"),
            "c1".into(),
            true,
            {
                let lines = lines.clone();
                move |l| lines.lock().unwrap().push(l.text)
            },
            done,
        );
        f.spawner.run_all();
        assert_eq!(*lines.lock().unwrap(), ["hello", "world"]);
        assert_eq!(got.lock().unwrap().take(), Some(Ok(())));
        assert_eq!(f.engine.calls(), ["logs c1 follow=true"]);
    }

    #[test]
    fn engine_actions_reach_engine() {
        let f = mac();
        let (restarted, done) = slot();
        f.app.restart_container(name("dev"), "c1".into(), done);
        let (removed, done) = slot();
        f.app.remove_container(name("dev"), "c1".into(), true, done);
        let (pruned_images, done) = slot();
        f.app.prune_images(name("dev"), done);
        let (pruned_volumes, done) = slot();
        f.app.prune_volumes(name("dev"), done);
        f.spawner.run_all();
        assert_eq!(restarted.lock().unwrap().take(), Some(Ok(())));
        assert_eq!(removed.lock().unwrap().take(), Some(Ok(())));
        assert!(matches!(pruned_images.lock().unwrap().take(), Some(Ok(_))));
        assert!(matches!(pruned_volumes.lock().unwrap().take(), Some(Ok(_))));
        assert_eq!(
            f.engine.calls(),
            [
                "restart c1",
                "remove c1 force=true",
                "prune_images",
                "prune_volumes"
            ]
        );
    }

    #[test]
    fn unavailable_engine_is_reported_to_the_callback() {
        let app = App::new(
            Arc::new(FakeVm::default()),
            HostOs::MacOs,
            |profile: &ProfileName| Err(PortError::Unavailable(format!("{profile} is stopped"))),
            Arc::new(ThreadSpawner),
        );
        let (tx, rx) = mpsc::channel();
        app.images(name("dev"), move |result| tx.send(result).unwrap());
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            Err(AppError::Port(PortError::Unavailable(
                "dev is stopped".into()
            )))
        );
    }

    #[test]
    fn results_cross_threads() {
        let vm = Arc::new(FakeVm::default());
        *vm.profiles.lock().unwrap() = vec![profile("dev")];
        let app = App::new(
            vm,
            HostOs::MacOs,
            |_: &ProfileName| Err(PortError::Unavailable("no engine".into())),
            Arc::new(ThreadSpawner),
        );
        let (tx, rx) = mpsc::channel();
        app.clone()
            .list_profiles(move |result| tx.send(result).unwrap());
        let profiles = rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        assert_eq!(profiles, [profile("dev")]);
    }

    #[test]
    fn profile_spec_is_read_from_the_vm_on_the_spawner() {
        let f = mac();
        f.vm.specs.lock().unwrap().push(spec("dev"));
        let (got, done) = slot();
        f.app.profile_spec(name("dev"), done);
        assert!(f.vm.calls().is_empty(), "nothing runs on the caller");
        f.spawner.run_all();
        assert_eq!(got.lock().unwrap().take(), Some(Ok(spec("dev"))));
        assert_eq!(f.vm.calls(), ["spec dev"]);
    }

    #[test]
    fn profile_spec_of_an_unknown_profile_is_an_error() {
        let f = mac();
        let (got, done) = slot();
        f.app.profile_spec(name("ghost"), done);
        f.spawner.run_all();
        assert!(matches!(got.lock().unwrap().take(), Some(Err(_))));
    }

    #[test]
    fn host_os_is_the_one_given_to_new() {
        assert_eq!(mac().app.host_os(), HostOs::MacOs);
        let linux = fixture(FakeVm::default(), FakeEngine::default(), HostOs::Linux);
        assert_eq!(linux.app.host_os(), HostOs::Linux);
    }

    #[test]
    fn cancelled_task_never_runs_and_never_calls_back() {
        let f = mac();
        let (got, done) = slot();
        let task = f
            .app
            .run_profile(name("dev"), Lifecycle::Start, |_| {}, done);
        task.cancel();
        f.spawner.run_all();
        assert!(got.lock().unwrap().is_none());
        assert!(f.vm.calls().is_empty());
    }

    #[test]
    fn l6_app_exposes_status_version_container_start_stop_prune_and_delete_options() {
        use telima_core::profile::{DeleteOptions, RuntimeInfo};
        use telima_core::version::{ColimaVersion, VersionSupport};

        let vm = FakeVm {
            info: RuntimeInfo {
                docker_server: Some("29.5.2".into()),
                ..RuntimeInfo::default()
            },
            version: Some(ColimaVersion::new(0, 10, 3)),
            ..FakeVm::default()
        };
        let f = fixture(vm, FakeEngine::default(), HostOs::MacOs);
        let (info, done) = slot();
        f.app.runtime_info(name("dev"), done);
        let (check, done) = slot();
        f.app.check_colima_version(done);
        let (started, done) = slot();
        f.app.start_container(name("dev"), "c1".into(), done);
        let (stopped, done) = slot();
        f.app.stop_container(name("dev"), "c1".into(), done);
        let (pruned, done) = slot();
        f.app.prune_cache(done);
        let (deleted, done) = slot();
        f.app
            .delete_profile(name("dev"), "dev".into(), DeleteOptions { data: true }, done);
        f.spawner.run_all();

        let info = info.lock().unwrap().take().unwrap().unwrap();
        assert_eq!(info.docker_server.as_deref(), Some("29.5.2"));
        let check = check.lock().unwrap().take().unwrap().unwrap();
        assert_eq!(check.support, VersionSupport::Supported);
        assert!(started.lock().unwrap().take().unwrap().is_ok());
        assert!(stopped.lock().unwrap().take().unwrap().is_ok());
        assert!(pruned.lock().unwrap().take().unwrap().is_ok());
        assert!(deleted.lock().unwrap().take().unwrap().is_ok());
        assert_eq!(f.engine.calls(), ["start c1", "stop c1"]);
        assert!(f.vm.calls().contains(&"delete dev --data".to_string()));
        assert!(f.vm.calls().contains(&"prune_cache".to_string()));
    }
}
