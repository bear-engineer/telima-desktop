//! Main window shell: sidebar of profiles, detail, lifecycle buttons.
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use gpui_kit::{AppContext, Entity, TestAppContext};
use telima_app::runtime::{App, BoxFuture, DynEngine, Spawner};
use telima_app::testing::{FakeEngine, FakeVm};
use telima_core::container::{Container, ContainerState};
use telima_core::profile::{
    HostOs, MountType, Profile, ProfileName, ProfileSpec, ProfileStatus, Runtime, VmType,
};

use super::{MainView, Tab};
use crate::engine_views::ListState;
use crate::profile_form::Mode;

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

fn spec(name: &str) -> ProfileSpec {
    ProfileSpec {
        name: ProfileName::parse(name).unwrap(),
        cpus: 4,
        memory_gib: 8,
        disk_gib: 100,
        vm_type: VmType::Vz,
        mount_type: MountType::Virtiofs,
        runtime: Runtime::Docker,
        rosetta: false,
        network_address: false,
    }
}

fn container(id: &str, name: &str) -> Container {
    Container {
        id: id.into(),
        name: name.into(),
        image: "alpine:3".into(),
        state: ContainerState::Running,
        status_text: String::new(),
    }
}

struct Fixture {
    vm: Arc<FakeVm>,
    spawner: Arc<QueueSpawner>,
    /// Profiles the engine factory was asked for.
    asked: Arc<Mutex<Vec<String>>>,
    view: Entity<MainView>,
}

impl Fixture {
    /// Runs queued use cases and lets the shell (and its child views) apply the results.
    fn settle(&self, cx: &mut TestAppContext) {
        self.spawner.run_all();
        self.view.update(cx, |v, cx| v.pump(cx));
    }
}

fn fixture(cx: &mut TestAppContext) -> Fixture {
    let vm = FakeVm {
        progress_lines: vec!["starting colima".into(), "done".into()],
        ..Default::default()
    };
    vm.specs.lock().unwrap().push(spec("work"));
    *vm.profiles.lock().unwrap() = vec![
        profile("work", ProfileStatus::Stopped),
        profile("default", ProfileStatus::Running),
        profile("alpha", ProfileStatus::Stopped),
    ];
    let vm = Arc::new(vm);
    let engine = FakeEngine::default();
    *engine.containers.lock().unwrap() = vec![container("c1", "pg-main")];
    let engine = Arc::new(engine);
    let spawner = Arc::new(QueueSpawner::default());
    let asked = Arc::new(Mutex::new(Vec::new()));
    let seen = asked.clone();
    let app = App::new(
        vm.clone(),
        HostOs::MacOs,
        move |profile: &ProfileName| {
            seen.lock().unwrap().push(profile.to_string());
            let engine: Arc<dyn DynEngine> = engine.clone();
            Ok(engine)
        },
        spawner.clone(),
    );
    let view = cx.new(|cx| MainView::new(app, cx));
    let f = Fixture {
        vm,
        spawner,
        asked,
        view,
    };
    f.settle(cx);
    f
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

#[gpui_kit::test]
fn delete_opens_the_typed_name_confirm_and_deleting_reloads_the_list(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("work", cx));
    assert!(f.view.read_with(cx, |v, _| v.delete_confirm().is_none()));

    f.view.update(cx, |v, cx| v.delete(cx));
    f.settle(cx);
    let confirm = f
        .view
        .read_with(cx, |v, _| v.delete_confirm().cloned())
        .expect("삭제 opens the confirm");
    assert!(!confirm.read_with(cx, |c, _| c.can_delete()));

    confirm.update(cx, |c, cx| c.set_typed("work", cx));
    confirm.update(cx, |c, cx| c.confirm(cx));
    *f.vm.profiles.lock().unwrap() = vec![profile("alpha", ProfileStatus::Stopped)];
    f.settle(cx);
    f.settle(cx);
    assert!(f.vm.calls().contains(&"delete work".to_string()));
    assert!(f.view.read_with(cx, |v, _| v.delete_confirm().is_none()));
    let names: Vec<String> = f
        .view
        .read_with(cx, |v, _| v.rows().iter().map(|r| r.name.clone()).collect());
    assert_eq!(names, ["alpha"]);
}

#[gpui_kit::test]
fn start_shows_progress_lines_in_the_detail_area(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("work", cx));
    f.view.update(cx, |v, cx| v.start(cx));
    let progress = f
        .view
        .read_with(cx, |v, _| v.progress().cloned())
        .expect("start shows the progress panel");
    f.spawner.run_all();
    progress.update(cx, |p, cx| p.pump(cx));
    let lines = progress.read_with(cx, |p, _| p.lines());
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("starting colima"), "{lines:?}");
    assert!(lines[1].ends_with("done"), "{lines:?}");
    assert!(f.vm.calls().contains(&"start work".to_string()));
}

#[gpui_kit::test]
fn switching_to_the_containers_tab_loads_the_selected_profiles_containers(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("work", cx));
    f.view.update(cx, |v, cx| v.set_tab(Tab::Images, cx));
    f.view.update(cx, |v, cx| v.set_tab(Tab::Containers, cx));
    f.settle(cx);
    let engine = f.view.read_with(cx, |v, _| v.engine().clone());
    let names: Vec<String> = engine.read_with(cx, |e, _| match e.containers() {
        ListState::Ready(rows) => rows.iter().map(|r| r.name.clone()).collect(),
        other => panic!("containers not loaded: {other:?}"),
    });
    assert_eq!(names, ["pg-main"]);
    assert!(f.asked.lock().unwrap().contains(&"work".to_string()));
}

#[gpui_kit::test]
fn edit_opens_the_editor_with_the_stored_spec(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("work", cx));
    f.view.update(cx, |v, cx| v.edit(cx));
    f.settle(cx);
    let editor = f
        .view
        .read_with(cx, |v, _| v.editor().cloned())
        .expect("편집 opens the editor");
    let mode = editor.read_with(cx, |e, _| e.mode().clone());
    assert_eq!(
        mode,
        Mode::Edit {
            current: spec("work")
        }
    );
    assert!(f.vm.calls().contains(&"spec work".to_string()));
}

#[gpui_kit::test]
fn new_profile_opens_an_empty_create_form(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.new_profile(cx));
    let editor = f
        .view
        .read_with(cx, |v, _| v.editor().cloned())
        .expect("새 프로필 opens the editor");
    assert_eq!(editor.read_with(cx, |e, _| e.mode().clone()), Mode::Create);
}
