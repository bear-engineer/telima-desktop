//! Containers / logs / images / volumes of one profile, driven by FakeEngine.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use gpui_kit::{AppContext, Entity, TestAppContext};
use telima_app::runtime::{App, BoxFuture, DynEngine, Spawner};
use telima_app::testing::{FakeEngine, FakeVm};
use telima_core::container::{Container, ContainerState, Image, Volume};
use telima_core::ports::PortError;
use telima_core::profile::{HostOs, ProfileName};

use super::{EnginePanel, ListState};
use crate::shell::Tab;

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

fn container(id: &str, name: &str, state: ContainerState) -> Container {
    Container {
        id: id.into(),
        name: name.into(),
        image: "postgres:17".into(),
        state,
        status_text: "Up 3 hours".into(),
    }
}

fn image(id: &str, tags: &[&str], size: u64) -> Image {
    Image {
        id: id.into(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        size_bytes: size,
        containers: None,
    }
}

fn volume(name: &str, refs: Option<u64>, size: u64) -> Volume {
    Volume {
        name: name.into(),
        size_bytes: Some(size),
        ref_count: refs,
    }
}

struct Fixture {
    engine: Arc<FakeEngine>,
    spawner: Arc<QueueSpawner>,
    fail: Arc<AtomicBool>,
    view: Entity<EnginePanel>,
}

impl Fixture {
    fn settle(&self, cx: &mut TestAppContext) {
        self.spawner.run_all();
        self.view.update(cx, |v, cx| v.pump(cx));
    }
}

fn fixture(cx: &mut TestAppContext, tab: Tab) -> Fixture {
    let engine = FakeEngine {
        log_lines: vec!["ready to accept connections".into()],
        ..Default::default()
    };
    *engine.containers.lock().unwrap() = vec![
        container("c2", "old-job", ContainerState::Exited),
        container("c1", "pg-main", ContainerState::Running),
    ];
    *engine.images.lock().unwrap() = vec![
        image("small", &["alpine:3"], 5 << 20),
        image("dangling", &[], 50 << 20),
        image("big", &["postgres:17"], 400 << 20),
    ];
    *engine.volumes.lock().unwrap() = vec![
        volume("pgdata", Some(1), 10 << 20),
        volume("stale", Some(0), 3 << 20),
        volume("unknown", None, 1 << 20),
    ];
    let engine = Arc::new(engine);
    let fail = Arc::new(AtomicBool::new(false));
    let spawner = Arc::new(QueueSpawner::default());
    let (e, f) = (engine.clone(), fail.clone());
    let app = App::new(
        Arc::new(FakeVm::default()),
        HostOs::MacOs,
        move |_: &ProfileName| {
            if f.load(Ordering::SeqCst) {
                return Err(PortError::Unavailable("docker.sock".into()));
            }
            let engine: Arc<dyn DynEngine> = e.clone();
            Ok(engine)
        },
        spawner.clone(),
    );
    let view = cx.new(|cx| EnginePanel::new(app, cx));
    let fx = Fixture {
        engine,
        spawner,
        fail,
        view,
    };
    let profile = ProfileName::parse("dev").unwrap();
    fx.view
        .update(cx, |v, cx| v.set_target(Some(profile), tab, cx));
    fx
}

#[gpui_kit::test]
fn list_is_loading_until_the_use_case_answers(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    let loading = f
        .view
        .read_with(cx, |v, _| matches!(v.containers(), ListState::Loading));
    assert!(loading);
}

#[gpui_kit::test]
fn container_rows_render_running_first(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.settle(cx);
    let rows = f.view.read_with(cx, |v, _| match v.containers() {
        ListState::Ready(rows) => rows
            .iter()
            .map(|r| (r.name.clone(), r.image.clone(), r.state_label))
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    });
    assert_eq!(
        rows,
        [
            ("pg-main".to_string(), "postgres:17".to_string(), "실행 중"),
            ("old-job".to_string(), "postgres:17".to_string(), "종료됨"),
        ]
    );
}

#[gpui_kit::test]
fn restart_calls_the_use_case(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.settle(cx);
    f.view.update(cx, |v, cx| v.restart("c1", cx));
    f.settle(cx);
    assert!(f.engine.calls().contains(&"restart c1".to_string()));
}

#[gpui_kit::test]
fn removing_a_running_container_needs_an_explicit_force_confirm(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.settle(cx);
    f.view.update(cx, |v, cx| v.request_remove("c1", cx));
    let body = f
        .view
        .read_with(cx, |v, _| v.dialog().map(|d| d.confirm.body.clone()));
    assert!(body.unwrap().contains("강제로"));
    f.settle(cx);
    assert!(!f.engine.calls().iter().any(|c| c.starts_with("remove")));

    f.view.update(cx, |v, cx| v.accept(cx));
    f.settle(cx);
    assert!(f.engine.calls().contains(&"remove c1 force=true".to_string()));
    assert!(f.view.read_with(cx, |v, _| v.dialog().is_none()));
}

#[gpui_kit::test]
fn dismissing_the_confirm_removes_nothing(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.settle(cx);
    f.view.update(cx, |v, cx| v.request_remove("c2", cx));
    f.view.update(cx, |v, cx| v.dismiss(cx));
    f.settle(cx);
    assert!(!f.engine.calls().iter().any(|c| c.starts_with("remove")));
}

#[gpui_kit::test]
fn log_viewer_follows_and_can_stop_following(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.settle(cx);
    f.view.update(cx, |v, cx| v.open_logs("c1", cx));
    f.settle(cx);
    let (follow, lines) = f.view.read_with(cx, |v, _| {
        let log = v.log().unwrap();
        (log.follow, log.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>())
    });
    assert!(follow);
    assert_eq!(lines, ["ready to accept connections"]);
    assert!(f.engine.calls().contains(&"logs c1 follow=true".to_string()));

    f.view.update(cx, |v, cx| v.set_follow(false, cx));
    f.settle(cx);
    assert!(f.engine.calls().contains(&"logs c1 follow=false".to_string()));
}

#[gpui_kit::test]
fn images_are_sorted_by_size(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Images);
    f.settle(cx);
    let ids = f.view.read_with(cx, |v, _| match v.images() {
        ListState::Ready(rows) => rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
        _ => Vec::new(),
    });
    assert_eq!(ids, ["big", "dangling", "small"]);
}

#[gpui_kit::test]
fn image_prune_preview_lists_only_dangling_with_count_and_size(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Images);
    f.settle(cx);
    f.view.update(cx, |v, cx| v.request_image_prune(cx));
    f.settle(cx);
    let d = f.view.read_with(cx, |v, _| v.dialog().cloned()).unwrap();
    assert_eq!(d.items, ["dangling"]);
    assert!(d.confirm.body.contains("1개"));
    assert!(d.confirm.confirm_label.contains("MB") || d.confirm.confirm_label.contains("MiB"));
    f.view.update(cx, |v, cx| v.accept(cx));
    f.settle(cx);
    assert!(f.engine.calls().contains(&"prune_images".to_string()));
}

#[gpui_kit::test]
fn volumes_mark_in_use_and_prune_preview_lists_only_unused(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Volumes);
    f.settle(cx);
    let marks = f.view.read_with(cx, |v, _| match v.volumes() {
        ListState::Ready(rows) => rows
            .iter()
            .map(|r| (r.name.clone(), r.in_use))
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    });
    assert_eq!(
        marks,
        [
            ("pgdata".to_string(), true),
            ("stale".to_string(), false),
            ("unknown".to_string(), true),
        ]
    );
    f.view.update(cx, |v, cx| v.request_volume_prune(cx));
    f.settle(cx);
    let d = f.view.read_with(cx, |v, _| v.dialog().cloned()).unwrap();
    assert_eq!(d.items, ["stale"]);
    f.view.update(cx, |v, cx| v.accept(cx));
    f.settle(cx);
    assert!(f.engine.calls().contains(&"prune_volumes".to_string()));
}

#[gpui_kit::test]
fn error_state_shows_retry_and_retry_reloads(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.fail.store(true, Ordering::SeqCst);
    f.settle(cx);
    let failed = f
        .view
        .read_with(cx, |v, _| matches!(v.containers(), ListState::Failed(_)));
    assert!(failed);

    f.fail.store(false, Ordering::SeqCst);
    f.view.update(cx, |v, cx| v.retry(cx));
    f.settle(cx);
    let ready = f
        .view
        .read_with(cx, |v, _| matches!(v.containers(), ListState::Ready(r) if r.len() == 2));
    assert!(ready);
}

#[gpui_kit::test]
fn empty_list_is_ready_with_no_rows(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.engine.containers.lock().unwrap().clear();
    f.settle(cx);
    let empty = f
        .view
        .read_with(cx, |v, _| matches!(v.containers(), ListState::Ready(r) if r.is_empty()));
    assert!(empty);
}
