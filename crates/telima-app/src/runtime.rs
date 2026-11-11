//! Async bridge between views and use cases. See docs/adr/0001-async-bridge.md.

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
}
