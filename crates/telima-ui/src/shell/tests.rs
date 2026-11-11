//! Main window shell: sidebar of profiles, detail, lifecycle buttons.
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use gpui_kit::{AppContext, Entity, TestAppContext};
use telima_app::runtime::{App, BoxFuture, DynEngine, Spawner};
use telima_app::testing::{FakeEngine, FakeVm};
use telima_core::container::{Container, ContainerState};
use telima_core::profile::{
    HostOs, Mount, MountType, Profile, ProfileName, ProfileSpec, ProfileStatus, Runtime,
    RuntimeInfo, VmType,
};
use telima_core::time::UnixMillis;
use telima_core::version::{ColimaVersion, LAST_TESTED};

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
        mounts: Vec::new(),
        kubernetes: false,
    }
}

fn container(id: &str, name: &str) -> Container {
    Container {
        id: id.into(),
        name: name.into(),
        image: "alpine:3".into(),
        state: ContainerState::Running,
        status_text: String::new(),
        ports: Vec::new(),
        created_at: None,
        started_at: None,
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
    let f = fixture_unsettled(cx);
    f.settle(cx);
    f
}

fn fixture_unsettled(cx: &mut TestAppContext) -> Fixture {
    fixture_vm(cx, |_| {})
}

/// Started 3 h 12 min 30 s ago, Colima 0.10.3, Docker 29.5.2.
fn running_info() -> RuntimeInfo {
    let now = crate::components::clock::now();
    RuntimeInfo {
        started_at: Some(UnixMillis(now.0 - (3 * 3600 + 12 * 60 + 30) * 1000)),
        colima: Some(ColimaVersion::new(0, 10, 3)),
        docker_client: Some("28.0.1".into()),
        docker_server: Some("29.5.2".into()),
    }
}

fn fixture_vm(cx: &mut TestAppContext, tweak: impl FnOnce(&mut FakeVm)) -> Fixture {
    let (vm, spawner, asked, app) = parts(tweak);
    let view = cx.new(|cx| MainView::new(app, cx));
    Fixture {
        vm,
        spawner,
        asked,
        view,
    }
}

type Parts = (Arc<FakeVm>, Arc<QueueSpawner>, Arc<Mutex<Vec<String>>>, App);

/// Fake VM, engine and spawner wired into an app facade, without a view.
fn parts(tweak: impl FnOnce(&mut FakeVm)) -> Parts {
    let mut vm = FakeVm {
        progress_lines: vec!["starting colima".into(), "done".into()],
        info: running_info(),
        version: Some(LAST_TESTED),
        ..Default::default()
    };
    vm.specs.lock().unwrap().push(spec("work"));
    *vm.profiles.lock().unwrap() = vec![
        profile("work", ProfileStatus::Stopped),
        profile("default", ProfileStatus::Running),
        profile("alpha", ProfileStatus::Stopped),
    ];
    tweak(&mut vm);
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
    (vm, spawner, asked, app)
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

// --- telima-design-r1: layout states (H1 H2 H6 M1 M9 M10 M12) ---

use super::{HeaderAction, Msg, Screen};
use telima_app::AppError;
use telima_core::ports::PortError;

#[gpui_kit::test]
fn shows_loading_until_the_first_list_arrives(cx: &mut TestAppContext) {
    let f = fixture_unsettled(cx);
    assert_eq!(f.view.read_with(cx, |v, _| v.screen()), Screen::Loading);
    f.settle(cx);
    assert_eq!(f.view.read_with(cx, |v, _| v.screen()), Screen::Profile);
}

#[gpui_kit::test]
fn empty_list_shows_the_empty_state(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.vm.profiles.lock().unwrap().clear();
    f.view.update(cx, |v, _| v.refresh());
    f.settle(cx);
    assert_eq!(f.view.read_with(cx, |v, _| v.screen()), Screen::Empty);
}

#[gpui_kit::test]
fn colima_not_installed_has_its_own_state_with_a_korean_hint(cx: &mut TestAppContext) {
    let f = fixture_unsettled(cx);
    f.view.update(cx, |v, cx| {
        v.inbox
            .lock()
            .unwrap()
            .push(Msg::Profiles(Err(AppError::Port(PortError::Unavailable(
                "colima not installed".into(),
            )))));
        v.pump(cx);
    });
    match f.view.read_with(cx, |v, _| v.screen()) {
        Screen::NotInstalled { message, hint } => {
            assert_eq!(message, "Colima가 설치되어 있지 않습니다.");
            assert_eq!(hint, Some("brew install colima"));
        }
        other => panic!("expected NotInstalled, got {other:?}"),
    }
}

#[gpui_kit::test]
fn errors_are_shown_in_korean_not_as_to_string(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| {
        v.inbox
            .lock()
            .unwrap()
            .push(Msg::Profiles(Err(AppError::Port(PortError::Failed(
                "exit status 1".into(),
            )))));
        v.pump(cx);
    });
    let err = f.view.read_with(cx, |v, _| v.error().map(str::to_owned));
    assert_eq!(
        err.as_deref(),
        Some("작업을 완료하지 못했습니다. 잠시 후 다시 시도해 주세요.")
    );
}

#[gpui_kit::test]
fn header_shows_only_state_valid_actions(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("default", cx));
    assert_eq!(
        f.view.read_with(cx, |v, _| v.header_actions()),
        [HeaderAction::Restart, HeaderAction::Stop]
    );
    f.view.update(cx, |v, cx| v.select("work", cx));
    assert_eq!(
        f.view.read_with(cx, |v, _| v.header_actions()),
        [HeaderAction::Start]
    );
    f.view.update(cx, |v, cx| v.start(cx));
    assert!(f.view.read_with(cx, |v, _| v.header_actions()).is_empty());
}

#[gpui_kit::test]
fn create_mode_replaces_the_header_and_hides_profile_actions(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("default", cx));
    f.view.update(cx, |v, cx| v.new_profile(cx));
    assert_eq!(f.view.read_with(cx, |v, _| v.screen()), Screen::Create);
    assert!(f.view.read_with(cx, |v, _| v.header_actions()).is_empty());
    assert_eq!(
        f.view.read_with(cx, |v, _| v.crumb()).as_deref(),
        Some("새 프로필")
    );
    f.view.update(cx, |v, cx| v.select("work", cx));
    assert_eq!(f.view.read_with(cx, |v, _| v.screen()), Screen::Profile);
}

#[gpui_kit::test]
fn title_crumb_names_the_selected_profile(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("work", cx));
    assert_eq!(
        f.view.read_with(cx, |v, _| v.crumb()).as_deref(),
        Some("work")
    );
}

#[gpui_kit::test]
fn inspector_lists_the_config_and_the_docker_socket(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("work", cx));
    let ins = f
        .view
        .read_with(cx, |v, _| v.inspector())
        .expect("구성 aside");
    let labels: Vec<&str> = ins.rows.iter().map(|(l, _)| *l).collect();
    assert_eq!(
        labels,
        [
            "런타임",
            "VM 종류",
            "마운트",
            "아키텍처",
            "CPU",
            "메모리",
            "디스크",
            "IP 주소",
            "가동 시간",
            "Kubernetes"
        ]
    );
    assert_eq!(ins.socket, "~/.colima/work/docker.sock");
    assert!(ins.can_edit && ins.can_delete);
}

#[gpui_kit::test]
fn tabs_show_counts_of_loaded_lists(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("work", cx));
    f.settle(cx);
    let n = f
        .view
        .read_with(cx, |v, cx| v.tab_count(Tab::Containers, cx));
    assert_eq!(n, Some(1));
    let none = f.view.read_with(cx, |v, cx| v.tab_count(Tab::Volumes, cx));
    assert_eq!(none, None);
}

#[gpui_kit::test]
fn inspector_shows_vm_type_and_mount_from_the_stored_spec(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("work", cx));
    f.settle(cx);
    let ins = f.view.read_with(cx, |v, _| v.inspector()).unwrap();
    let value = |label: &str| {
        ins.rows
            .iter()
            .find(|(l, _)| *l == label)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(value("VM 종류").as_deref(), Some("vz"));
    assert_eq!(value("마운트").as_deref(), Some("virtiofs"));
    // No spec stored for alpha: the values stay unknown.
    f.view.update(cx, |v, cx| v.select("alpha", cx));
    f.settle(cx);
    let ins = f.view.read_with(cx, |v, _| v.inspector()).unwrap();
    assert!(ins.rows.iter().any(|(l, v)| *l == "VM 종류" && v == "—"));
}

#[test]
fn header_actions_carry_canvas_icons() {
    use super::HeaderAction;
    use crate::components::icons::Icon;
    assert_eq!(HeaderAction::Start.icon(), Icon::Play);
    assert_eq!(HeaderAction::Stop.icon(), Icon::Stop);
    assert_eq!(HeaderAction::Restart.icon(), Icon::Restart);
}

#[gpui_kit::test]
fn header_chip_and_inspector_show_the_uptime_of_a_running_profile(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("default", cx));
    f.settle(cx);
    assert!(f.vm.calls().contains(&"runtime_info default".to_owned()));
    let uptime = f.view.read_with(cx, |v, _| v.uptime());
    assert_eq!(uptime.as_deref(), Some("가동 3시간 12분"));
    let ins = f.view.read_with(cx, |v, _| v.inspector()).unwrap();
    assert!(
        ins.rows
            .iter()
            .any(|(l, v)| *l == "가동 시간" && v == "3시간 12분"),
        "F6: the row label already says 가동"
    );
    // A stopped profile has no uptime.
    f.view.update(cx, |v, cx| v.select("work", cx));
    f.settle(cx);
    assert_eq!(f.view.read_with(cx, |v, _| v.uptime()), None);
    let ins = f.view.read_with(cx, |v, _| v.inspector()).unwrap();
    assert!(ins.rows.iter().any(|(l, v)| *l == "가동 시간" && v == "—"));
}

#[gpui_kit::test]
fn sidebar_footer_shows_colima_and_docker_versions(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("default", cx));
    f.settle(cx);
    let footer = f.view.read_with(cx, |v, _| v.version_footer());
    assert_eq!(footer.colima, "Colima 0.10.3");
    assert_eq!(footer.docker, "Docker 29.5.2");
}

#[gpui_kit::test]
fn footer_falls_back_to_the_startup_colima_version(cx: &mut TestAppContext) {
    let f = fixture_vm(cx, |vm| vm.info = RuntimeInfo::default());
    f.settle(cx);
    let footer = f.view.read_with(cx, |v, _| v.version_footer());
    assert_eq!(footer.colima, format!("Colima {LAST_TESTED}"));
    assert_eq!(footer.docker, "Docker —");
}

#[gpui_kit::test]
fn supported_colima_shows_no_version_notice(cx: &mut TestAppContext) {
    let f = fixture(cx);
    assert!(f.vm.calls().contains(&"colima_version".to_owned()));
    assert_eq!(f.view.read_with(cx, |v, _| v.version_notice()), None);
}

#[gpui_kit::test]
fn too_old_colima_shows_a_blocking_notice_that_cannot_be_dismissed(cx: &mut TestAppContext) {
    use telima_app::view::daily::VersionNoticeSeverity;
    let f = fixture_vm(cx, |vm| vm.version = Some(ColimaVersion::new(0, 9, 0)));
    f.settle(cx);
    let n = f.view.read_with(cx, |v, _| v.version_notice()).unwrap();
    assert_eq!(n.severity, VersionNoticeSeverity::Blocking);
    assert_eq!(n.title, "Colima 업데이트 필요");
    assert_eq!(n.hint, Some("brew upgrade colima"));
    f.view.update(cx, |v, cx| v.dismiss_version_notice(cx));
    assert!(f.view.read_with(cx, |v, _| v.version_notice()).is_some());
}

#[gpui_kit::test]
fn untested_colima_shows_a_soft_warning_that_can_be_dismissed(cx: &mut TestAppContext) {
    use telima_app::view::daily::VersionNoticeSeverity;
    let f = fixture_vm(cx, |vm| vm.version = Some(ColimaVersion::new(99, 0, 0)));
    f.settle(cx);
    let n = f.view.read_with(cx, |v, _| v.version_notice()).unwrap();
    assert_eq!(n.severity, VersionNoticeSeverity::Warning);
    assert_eq!(n.title, "검증되지 않은 Colima 버전");
    f.view.update(cx, |v, cx| v.dismiss_version_notice(cx));
    assert_eq!(f.view.read_with(cx, |v, _| v.version_notice()), None);
}

#[gpui_kit::test]
fn prune_asks_first_and_confirming_runs_the_cache_prune(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.prune(cx));
    let confirm = f
        .view
        .read_with(cx, |v, _| v.prune_confirm().cloned())
        .unwrap();
    assert_eq!(confirm.title, "캐시 정리");
    f.settle(cx);
    assert!(!f.vm.calls().contains(&"prune_cache".to_owned()));
    f.view.update(cx, |v, cx| v.confirm_prune(cx));
    f.settle(cx);
    assert!(f.vm.calls().contains(&"prune_cache".to_owned()));
    assert!(f.view.read_with(cx, |v, _| v.prune_confirm().is_none()));
}

#[gpui_kit::test]
fn cancelling_the_prune_confirm_runs_nothing(cx: &mut TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.prune(cx));
    f.view.update(cx, |v, cx| v.cancel_prune(cx));
    f.settle(cx);
    assert!(f.view.read_with(cx, |v, _| v.prune_confirm().is_none()));
    assert!(!f.vm.calls().contains(&"prune_cache".to_owned()));
}

#[gpui_kit::test]
fn inspector_lists_mounts_and_kubernetes_from_the_stored_spec(cx: &mut TestAppContext) {
    use telima_app::view::daily::MountRow;
    let f = fixture_vm(cx, |vm| {
        let mut s = vm.specs.lock().unwrap();
        s[0].mounts = vec![Mount {
            path: "/opt/src".into(),
            writable: true,
        }];
        s[0].kubernetes = true;
    });
    f.settle(cx);
    f.view.update(cx, |v, cx| v.select("work", cx));
    f.settle(cx);
    let ins = f.view.read_with(cx, |v, _| v.inspector()).unwrap();
    assert_eq!(
        ins.mounts,
        [MountRow {
            path: "/opt/src".into(),
            mode: "읽기·쓰기"
        }]
    );
    assert!(
        ins.rows
            .iter()
            .any(|(l, v)| *l == "Kubernetes" && v == "사용")
    );
    f.view.update(cx, |v, cx| v.select("alpha", cx));
    f.settle(cx);
    let ins = f.view.read_with(cx, |v, _| v.inspector()).unwrap();
    assert!(ins.mounts.is_empty());
}

// --- telima-l6 review fixes (F1 F2 F3 F10) ---

#[gpui_kit::test]
fn creating_a_profile_hands_off_to_the_start_progress(cx: &mut TestAppContext) {
    // F1: no frozen "저장 중…" form; the new profile shows the shell's StartProgress.
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.new_profile(cx));
    let editor = f.view.read_with(cx, |v, _| v.editor().cloned()).unwrap();
    editor.update(cx, |e, cx| {
        e.set_name("telima-test-a", cx);
        e.submit(cx);
    });
    assert_eq!(f.view.read_with(cx, |v, _| v.screen()), Screen::Profile);
    assert!(f.view.read_with(cx, |v, _| v.editor().is_none()));
    assert_eq!(
        f.view
            .read_with(cx, |v, _| v.selected().map(|n| n.to_string()))
            .as_deref(),
        Some("telima-test-a")
    );
    let row = f
        .view
        .read_with(cx, |v, _| v.rows())
        .into_iter()
        .find(|r| r.name == "telima-test-a")
        .expect("the new profile is in the sidebar while it is created");
    assert_eq!(row.kind, telima_app::view::StatusKind::Busy);
    let progress = f
        .view
        .read_with(cx, |v, _| v.progress().cloned())
        .expect("create shows the start progress");
    assert_eq!(
        progress.read_with(cx, |p, _| p.name().to_string()),
        "telima-test-a"
    );
    f.spawner.run_all();
    progress.update(cx, |p, cx| p.pump(cx));
    assert_eq!(progress.read_with(cx, |p, _| p.lines()).len(), 2);
    assert!(f.vm.calls().contains(&"create telima-test-a".to_string()));
}

#[gpui_kit::test]
fn prune_confirm_takes_focus_and_escape_closes_it(cx: &mut TestAppContext) {
    // F2: Escape closes 캐시 정리 like the delete dialog (N3).
    let (vm, spawner, _, app) = parts(|_| {});
    let (view, cx) = cx.add_window_view(|_, cx| MainView::new(app, cx));
    spawner.run_all();
    view.update(cx, |v, cx| v.pump(cx));
    view.update(cx, |v, cx| v.prune(cx));
    cx.run_until_parked();
    assert!(
        cx.update(|window, cx| view.read(cx).prune_has_focus(window)),
        "the prune confirm must take focus when it opens"
    );
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.prune_confirm().is_none()));
    assert!(!vm.calls().contains(&"prune_cache".to_owned()));
}

#[gpui_kit::test]
fn stopping_hides_the_uptime_chip_and_the_header_chip_names_the_command(cx: &mut TestAppContext) {
    // F3: one name/chip pair; no 가동 chip while the profile is stopping.
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("default", cx));
    f.settle(cx);
    assert!(f.view.read_with(cx, |v, _| v.uptime()).is_some());
    f.view.update(cx, |v, cx| v.stop(cx));
    assert_eq!(f.view.read_with(cx, |v, _| v.uptime()), None);
    assert_eq!(
        f.view
            .read_with(cx, |v, cx| v.header_status_label(cx))
            .as_deref(),
        Some("중지 중")
    );
}

#[gpui_kit::test]
fn footer_keeps_the_last_known_docker_version_for_a_stopped_profile(cx: &mut TestAppContext) {
    // F10: a stopped profile has no Docker server; keep the last version seen.
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("default", cx));
    f.settle(cx);
    f.view.update(cx, |v, cx| v.select("work", cx));
    f.settle(cx);
    f.view.update(cx, |v, cx| {
        v.inbox.lock().unwrap().push(Msg::Runtime(
            ProfileName::parse("work").unwrap(),
            Ok(RuntimeInfo {
                docker_client: Some("28.0.1".into()),
                ..RuntimeInfo::default()
            }),
        ));
        v.pump(cx);
    });
    let footer = f.view.read_with(cx, |v, _| v.version_footer());
    assert_eq!(footer.docker, "Docker 29.5.2");
}
