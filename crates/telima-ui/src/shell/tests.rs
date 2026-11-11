//! Main window shell: sidebar of profiles, detail, lifecycle buttons.
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use gpui_kit::{AppContext, Entity, TestAppContext};
use telima_app::runtime::{App, BoxFuture, DynEngine, Spawner};
use telima_app::testing::{FakeEngine, FakeVm};
use telima_core::profile::{HostOs, Profile, ProfileName, ProfileStatus, Runtime};

use super::MainView;

#[derive(Default)]
struct QueueSpawner(Mutex<Vec<BoxFuture<'static, ()>>>);

impl Spawner for QueueSpawner {
    fn spawn(&self, work: BoxFuture<'static, ()>) {
        self.0.lock().unwrap().push(work);
    }
}

impl QueueSpawner {
    fn run_all(&self) {
        loop {
            let queued = std::mem::take(&mut *self.0.lock().unwrap());
            if queued.is_empty() {
                break;
            }
            for work in queued {
                block_on(work);
            }
        }
    }
}

fn profile(name: &str, status: ProfileStatus) -> Profile {
    Profile {
        name: ProfileName::parse(name).unwrap(),
        status,
        arch: "aarch64".into(),
        cpus: 4,
        memory_bytes: 8 << 30,
        disk_bytes: 100 << 30,
        address: None,
        runtime: Runtime::Docker,
    }
}

struct Fixture {
    vm: Arc<FakeVm>,
    spawner: Arc<QueueSpawner>,
    view: Entity<MainView>,
}

fn fixture(cx: &mut TestAppContext) -> Fixture {
    let vm = FakeVm::default();
    *vm.profiles.lock().unwrap() = vec![
        profile("work", ProfileStatus::Stopped),
        profile("default", ProfileStatus::Running),
        profile("alpha", ProfileStatus::Stopped),
    ];
    let vm = Arc::new(vm);
    let engine = Arc::new(FakeEngine::default());
    let spawner = Arc::new(QueueSpawner::default());
    let app = App::new(
        vm.clone(),
        HostOs::MacOs,
        move |_: &ProfileName| {
            let engine: Arc<dyn DynEngine> = engine.clone();
            Ok(engine)
        },
        spawner.clone(),
    );
    let view = cx.new(|cx| MainView::new(app, cx));
    spawner.run_all();
    view.update(cx, |v, cx| v.pump(cx));
    Fixture { vm, spawner, view }
}

#[gpui_kit::test]
fn sidebar_lists_profiles_sorted_by_name(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let names: Vec<String> = f
        .view
        .read_with(cx, |v, _| v.rows().iter().map(|r| r.name.clone()).collect());
    assert_eq!(names, ["alpha", "default", "work"]);
}

#[gpui_kit::test]
fn first_profile_is_selected_and_selecting_shows_its_detail(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let first = f.view.read_with(cx, |v, _| v.detail().map(|d| d.name));
    assert_eq!(first.as_deref(), Some("alpha"));

    f.view.update(cx, |v, cx| v.select("default", cx));
    let detail = f.view.read_with(cx, |v, _| v.detail()).unwrap();
    assert_eq!(detail.name, "default");
    assert_eq!(detail.status_label, "실행 중");
    assert!(detail.actions.stop && !detail.actions.start);
}

#[gpui_kit::test]
fn start_calls_the_use_case_for_the_selected_profile(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("work", cx));
    f.view.update(cx, |v, cx| v.start(cx));
    f.spawner.run_all();
    f.view.update(cx, |v, cx| v.pump(cx));
    assert!(f.vm.calls().contains(&"start work".to_string()));
}

#[gpui_kit::test]
fn buttons_are_disabled_while_the_profile_is_busy(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("work", cx));
    f.view.update(cx, |v, cx| v.start(cx));

    let busy = f.view.read_with(cx, |v, _| v.detail()).unwrap().actions;
    assert!(!busy.start && !busy.stop && !busy.restart && !busy.edit && !busy.delete);

    f.spawner.run_all();
    f.view.update(cx, |v, cx| v.pump(cx));
    let idle = f.view.read_with(cx, |v, _| v.detail()).unwrap().actions;
    assert!(idle.edit && idle.delete);
}
