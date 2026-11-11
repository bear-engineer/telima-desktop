//! Containers / logs / images / volumes of one profile, driven by FakeEngine.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use gpui_kit::{AppContext, Entity, Modifiers, TestAppContext, VisualTestContext, px, size};
use telima_app::runtime::{App, BoxFuture, DynEngine, Spawner};
use telima_app::testing::{FakeEngine, FakeVm};
use telima_core::container::{Container, ContainerState, Image, Volume};
use telima_core::ports::PortError;
use telima_core::profile::{HostOs, ProfileName};

use super::{EnginePanel, ListState, StatusFilter};
use crate::shell::Tab;
use crate::style::space;
use crate::theme::{font, metric};

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
        ports: Vec::new(),
        created_at: None,
        started_at: None,
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

fn fake_app() -> (Arc<FakeEngine>, Arc<QueueSpawner>, Arc<AtomicBool>, App) {
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
    (engine, spawner, fail, app)
}

fn fixture(cx: &mut TestAppContext, tab: Tab) -> Fixture {
    let (engine, spawner, fail, app) = fake_app();
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
    assert!(
        f.engine
            .calls()
            .contains(&"remove c1 force=true".to_string())
    );
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
        (
            log.follow,
            log.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>(),
        )
    });
    assert!(follow);
    assert_eq!(lines, ["ready to accept connections"]);
    assert!(
        f.engine
            .calls()
            .contains(&"logs c1 follow=true".to_string())
    );

    f.view.update(cx, |v, cx| v.set_follow(false, cx));
    f.settle(cx);
    assert!(
        f.engine
            .calls()
            .contains(&"logs c1 follow=false".to_string())
    );
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
            ("unknown".to_string(), false),
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
    let ready = f.view.read_with(
        cx,
        |v, _| matches!(v.containers(), ListState::Ready(r) if r.len() == 2),
    );
    assert!(ready);
}

#[gpui_kit::test]
fn empty_list_is_ready_with_no_rows(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.engine.containers.lock().unwrap().clear();
    f.settle(cx);
    let empty = f.view.read_with(
        cx,
        |v, _| matches!(v.containers(), ListState::Ready(r) if r.is_empty()),
    );
    assert!(empty);
}

#[gpui_kit::test]
fn filter_query_and_status_group_narrow_the_visible_rows(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.settle(cx);
    let visible = |cx: &mut TestAppContext| {
        f.view.read_with(cx, |v, _| {
            v.visible_containers()
                .into_iter()
                .map(|r| r.name)
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(visible(cx), ["pg-main", "old-job"]);
    f.view
        .update(cx, |v, cx| v.set_status_filter(StatusFilter::Exited, cx));
    assert_eq!(visible(cx), ["old-job"]);
    f.view.update(cx, |v, cx| {
        v.set_status_filter(StatusFilter::All, cx);
        v.set_query("PG".into(), cx);
    });
    assert_eq!(visible(cx), ["pg-main"]);
}

#[gpui_kit::test]
fn removing_exited_containers_confirms_then_removes_each(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.settle(cx);
    f.view.update(cx, |v, cx| v.request_remove_exited(cx));
    let items = f
        .view
        .read_with(cx, |v, _| v.dialog().map(|d| d.items.clone()))
        .unwrap();
    assert_eq!(items, ["old-job"]);
    assert!(!f.engine.calls().iter().any(|c| c.starts_with("remove")));
    f.view.update(cx, |v, cx| v.accept(cx));
    f.settle(cx);
    assert!(
        f.engine
            .calls()
            .contains(&"remove c2 force=false".to_string())
    );
    assert!(!f.engine.calls().iter().any(|c| c.starts_with("remove c1")));
}

#[gpui_kit::test]
fn list_errors_are_korean_not_debug_text(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.fail.store(true, Ordering::SeqCst);
    f.settle(cx);
    let msg = f.view.read_with(cx, |v, _| match v.containers() {
        ListState::Failed(m) => m.clone(),
        _ => String::new(),
    });
    assert!(msg.contains("연결할 수 없습니다"), "{msg}");
    assert!(!msg.contains("unavailable") && !msg.contains("docker.sock"));
}

#[gpui_kit::test]
fn selecting_a_row_keeps_its_actions_visible(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.settle(cx);
    assert_eq!(
        f.view.read_with(cx, |v, _| v.selected().map(str::to_owned)),
        None
    );
    f.view.update(cx, |v, cx| v.select("c1", cx));
    assert_eq!(
        f.view.read_with(cx, |v, _| v.selected().map(str::to_owned)),
        Some("c1".to_string())
    );
    f.view.update(cx, |v, cx| v.toggle_menu("c1", cx));
    assert_eq!(
        f.view.read_with(cx, |v, _| v.menu().map(str::to_owned)),
        Some("c1".to_string())
    );
    f.view.update(cx, |v, cx| v.toggle_menu("c1", cx));
    assert_eq!(
        f.view.read_with(cx, |v, _| v.menu().map(str::to_owned)),
        None
    );
}

/// N1: at the window width (and at the main pane width between sidebar and inspector) the
/// data columns must leave room for the row actions, inside the row and aligned with the header.
#[gpui_kit::test]
fn container_row_actions_get_width_inside_the_row(cx: &mut TestAppContext) {
    let (_, spawner, _, app) = fake_app();
    let (view, cx) = cx.add_window_view(|_, cx| EnginePanel::new(app, cx));
    let profile = ProfileName::parse("dev").unwrap();
    view.update(cx, |v, cx| v.set_target(Some(profile), Tab::Containers, cx));
    let main = metric::WINDOW_W - metric::SIDEBAR - metric::INSPECTOR;
    for w in [metric::WINDOW_W, main] {
        cx.simulate_resize(size(px(w), px(metric::WINDOW_H)));
        spawner.run_all();
        view.update(cx, |v, cx| {
            v.pump(cx);
            v.select("c1", cx);
        });
        cx.run_until_parked();
        let row = cx.debug_bounds("ctr-row-c1").expect("row rendered");
        let actions = cx.debug_bounds("ctr-actions-c1").expect("actions rendered");
        let head = cx
            .debug_bounds("ctr-header-actions")
            .expect("header slot rendered");
        assert!(row.size.width <= px(w), "row {row:?} wider than {w}");
        assert!(
            actions.size.width >= px(metric::BTN_SM * 2.0),
            "actions slot {actions:?} too narrow at {w}"
        );
        assert!(
            actions.left() >= row.left() && actions.right() <= row.right(),
            "actions {actions:?} outside row {row:?} at {w}"
        );
        // N7: the actions slot sits flush with the row's inner right edge (no dead strip).
        assert_eq!(
            actions.right(),
            row.right() - space(1),
            "actions {actions:?} not flush right in row {row:?} at {w}"
        );
        assert_eq!(head.left(), actions.left(), "header misaligned at {w}");
        assert_eq!(head.size.width, actions.size.width);
        for col in 0..super::copy::CONTAINER_COLS.len() {
            let name: &'static str = Box::leak(format!("ctr-cell-c1-{col}").into_boxed_str());
            let hname: &'static str = Box::leak(format!("ctr-header-{col}").into_boxed_str());
            let c = cx.debug_bounds(name).expect("cell rendered");
            let h = cx.debug_bounds(hname).expect("header cell rendered");
            assert!(c.size.width > px(0.), "column {col} has no width at {w}");
            assert_eq!(
                (c.left(), c.size.width),
                (h.left(), h.size.width),
                "column {col} at {w}"
            );
        }
        // N7: the status column (3 = 상태) fits "실행 중 · 2시간".
        let need = status_text_width(cx);
        let status = cx.debug_bounds("ctr-cell-c1-3").expect("status cell");
        assert!(
            status.size.width >= need,
            "status cell {status:?} narrower than {need:?} at {w}"
        );
        // F7: the 포트 column (4) fits one full mapping without an ellipsis.
        let need = mono_text_width(cx, "9999→9999/tcp");
        let port = cx.debug_bounds("ctr-cell-c1-4").expect("port cell");
        assert!(
            port.size.width >= need,
            "port cell {port:?} narrower than {need:?} at {w}"
        );
        // M4: 포트/시작 columns are back now that rows carry the data.
        let labels: Vec<_> = super::copy::CONTAINER_COLS.iter().map(|c| c.0).collect();
        assert_eq!(labels, ["이름", "ID", "이미지", "상태", "포트", "시작"]);
    }
}

fn status_text_width(cx: &mut VisualTestContext) -> gpui_kit::Pixels {
    cx.update(|window, _| {
        let s = "실행 중 · 2시간";
        let run = window.text_style().to_run(s.len());
        window
            .text_system()
            .layout_line(s, px(font::BODY.size), &[run], None)
            .width
    })
}

fn mono_text_width(cx: &mut VisualTestContext, s: &str) -> gpui_kit::Pixels {
    cx.update(|window, _| {
        let mut style = window.text_style();
        style.font_family = font::MONO.into();
        let run = style.to_run(s.len());
        window
            .text_system()
            .layout_line(s, px(font::CODE.size), &[run], None)
            .width
    })
}

/// F7: several mappings show the first plus a count; one mapping or none stays as is.
#[test]
fn f7_port_cell_shows_the_first_mapping_plus_a_count() {
    use super::model::port_cell;
    assert_eq!(port_cell("9999→9999/tcp"), "9999→9999/tcp");
    assert_eq!(port_cell("—"), "—");
    assert_eq!(
        port_cell("8080→8080/tcp, 443→8443/tcp, 53/udp"),
        "8080→8080/tcp +2"
    );
}

/// F8: while 로그 검색 is active the footer counts the shown lines over the total.
#[test]
fn f8_log_count_shows_filtered_over_total() {
    use super::model::log_count_text;
    assert_eq!(log_count_text(17, 17, false), "17줄");
    assert_eq!(log_count_text(6, 17, true), "6 / 17줄");
    assert_eq!(log_count_text(0, 1204, true), "0 / 1,204줄");
}

fn sel(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// A panel at the main pane width with one container whose image name is 120 characters.
fn long_image_panel(cx: &mut TestAppContext) -> (Entity<EnginePanel>, &mut VisualTestContext) {
    let (engine, spawner, _, app) = fake_app();
    engine.containers.lock().unwrap()[1].image = format!("registry.example/{}", "x".repeat(103));
    assert_eq!(engine.containers.lock().unwrap()[1].image.len(), 120);
    let (view, cx) = cx.add_window_view(|_, cx| EnginePanel::new(app, cx));
    let main = metric::WINDOW_W - metric::SIDEBAR - metric::INSPECTOR;
    cx.simulate_resize(size(px(main), px(metric::WINDOW_H)));
    let profile = ProfileName::parse("dev").unwrap();
    view.update(cx, |v, cx| v.set_target(Some(profile), Tab::Containers, cx));
    spawner.run_all();
    view.update(cx, |v, cx| {
        v.pump(cx);
        v.select("c1", cx);
    });
    cx.run_until_parked();
    (view, cx)
}

/// Opens the log panel the way a user does: ⋯ then 로그 보기.
fn open_logs_by_menu(cx: &mut VisualTestContext) {
    let more = cx.debug_bounds("ctr-more-c1").expect("⋯ button");
    cx.simulate_click(more.center(), Modifiers::none());
    cx.run_until_parked();
    let item = cx
        .debug_bounds(sel("menu-logs-c1".into()))
        .expect("로그 보기 item");
    cx.simulate_click(item.center(), Modifiers::none());
    cx.run_until_parked();
}

/// N6: with a 120-char image the header text ellipsizes and the close button stays in the panel.
#[gpui_kit::test]
fn log_header_keeps_its_close_button_inside_the_panel(cx: &mut TestAppContext) {
    let (view, cx) = long_image_panel(cx);
    open_logs_by_menu(cx);
    assert!(
        view.read_with(cx, |v, _| v.log().is_some()),
        "log panel opened"
    );
    let panel = cx.debug_bounds("log-panel").expect("log panel");
    let close = cx.debug_bounds("log-close").expect("close button");
    let follow = cx.debug_bounds("log-switches").expect("switches");
    assert!(
        close.left() >= panel.left() && close.right() <= panel.right(),
        "close {close:?} outside panel {panel:?}"
    );
    assert!(close.size.width > px(0.), "close button squeezed");
    assert!(
        follow.left() >= panel.left() && follow.right() <= close.left(),
        "switches {follow:?} not before close {close:?}"
    );
}

/// N6: Escape closes the log panel.
#[gpui_kit::test]
fn escape_closes_the_log_panel(cx: &mut TestAppContext) {
    let (view, cx) = long_image_panel(cx);
    open_logs_by_menu(cx);
    assert!(
        view.read_with(cx, |v, _| v.log().is_some()),
        "log panel opened"
    );
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |v, _| v.log().is_none()),
        "Escape left the panel open"
    );
}

/// L6: the ⋯ menu opens under its button, right-aligned to it.
#[gpui_kit::test]
fn row_menu_opens_under_its_button_right_aligned(cx: &mut TestAppContext) {
    let (_view, cx) = long_image_panel(cx);
    let more = cx.debug_bounds("ctr-more-c1").expect("⋯ button");
    cx.simulate_click(more.center(), Modifiers::none());
    cx.run_until_parked();
    let menu = cx.debug_bounds("row-menu").expect("menu");
    assert_eq!(
        menu.right(),
        more.right(),
        "menu {menu:?} not right-aligned to {more:?}"
    );
    assert!(
        menu.top() >= more.bottom() && menu.top() - more.bottom() <= space(0),
        "menu {menu:?} not directly under {more:?}"
    );
}

/// M3: start and stop run the use cases.
#[gpui_kit::test]
fn start_and_stop_call_the_use_cases(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.settle(cx);
    f.view.update(cx, |v, cx| v.start("c2", cx));
    f.view.update(cx, |v, cx| v.stop("c1", cx));
    f.settle(cx);
    let calls = f.engine.calls();
    assert!(calls.contains(&"start c2".to_string()), "{calls:?}");
    assert!(calls.contains(&"stop c1".to_string()), "{calls:?}");
}

/// M3: a running row offers 중지, an exited row offers 시작, never both.
#[gpui_kit::test]
fn row_icon_actions_are_valid_for_the_state(cx: &mut TestAppContext) {
    let (view, cx) = long_image_panel(cx);
    // Actions of an unselected row are hidden, so check each row while it is selected.
    for (id, start, stop) in [("c1", false, true), ("c2", true, false)] {
        view.update(cx, |v, cx| v.select(id, cx));
        cx.run_until_parked();
        let shown = |cx: &mut VisualTestContext, what: &str| {
            cx.debug_bounds(sel(format!("ctr-{what}-{id}"))).is_some()
        };
        assert_eq!(shown(cx, "start"), start, "시작 on {id}");
        assert_eq!(shown(cx, "stop"), stop, "중지 on {id}");
    }
}

/// M4: the 포트/시작 cells show the row data.
#[gpui_kit::test]
fn ports_and_started_come_from_the_row(cx: &mut TestAppContext) {
    let f = fixture(cx, Tab::Containers);
    f.engine.containers.lock().unwrap()[1].ports = vec![telima_core::container::PortBinding {
        host_ip: None,
        host_port: Some(8080),
        container_port: 80,
        protocol: telima_core::container::PortProtocol::Tcp,
    }];
    f.settle(cx);
    let ports = f.view.read_with(cx, |v, _| {
        v.visible_containers()
            .into_iter()
            .map(|r| r.ports)
            .collect::<Vec<_>>()
    });
    assert_eq!(ports, ["8080→80/tcp", "—"]);
}

/// L6 leftover: Escape closes the row ⋯ menu.
#[gpui_kit::test]
fn escape_closes_the_row_menu(cx: &mut TestAppContext) {
    let (view, cx) = long_image_panel(cx);
    let more = cx.debug_bounds("ctr-more-c1").expect("⋯ button");
    cx.simulate_click(more.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.menu().is_some()), "menu opened");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |v, _| v.menu().is_none()),
        "Escape left the menu open"
    );
}

/// M7: log rows carry the line's real timestamp in the viewer's offset and follow the
/// 로그 검색 query (app log filter: case-insensitive, blank = all).
#[test]
fn log_rows_show_timestamps_and_filter_by_query() {
    use telima_core::container::{LogLine, LogStream};
    use telima_core::time::UnixMillis;
    let line = |text: &str, ts: Option<i64>| LogLine {
        stream: LogStream::Stdout,
        text: text.into(),
        timestamp: ts.map(UnixMillis),
    };
    // 2026-11-14T05:14:02Z
    let lines = [
        line("LOG: ready", Some(1_794_633_242_000)),
        line("ERROR: boom", None),
    ];
    let rows = super::model::log_rows(&lines, "", 9 * 3600);
    let got: Vec<_> = rows.iter().map(|r| (r.time.as_str(), r.text)).collect();
    assert_eq!(got, [("14:14:02", "LOG: ready"), ("", "ERROR: boom")]);
    let rows = super::model::log_rows(&lines, "error", 0);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text, "ERROR: boom");
}

/// M7: typing into 로그 검색 narrows the visible log lines.
#[gpui_kit::test]
fn log_search_field_filters_lines(cx: &mut TestAppContext) {
    let (view, cx) = long_image_panel(cx);
    open_logs_by_menu(cx);
    spawner_settle(&view, cx);
    let field = cx.debug_bounds("log-search").expect("로그 검색 field");
    // F8: the field is the canvas search width, not the form select width.
    assert_eq!(field.size.width, px(metric::FIELD_SEARCH));
    cx.simulate_click(field.center(), Modifiers::none());
    cx.simulate_keystrokes("z z z");
    cx.run_until_parked();
    let (query, shown) = view.read_with(cx, |v, _| (v.log_query().to_owned(), v.log_rows().len()));
    assert_eq!(query, "zzz");
    assert_eq!(shown, 0);
    view.update(cx, |v, cx| v.set_log_query(String::new(), cx));
    assert!(view.read_with(cx, |v, _| v.log_query().is_empty()));
}

fn spawner_settle(view: &Entity<EnginePanel>, cx: &mut VisualTestContext) {
    view.update(cx, |v, cx| v.pump(cx));
    cx.run_until_parked();
}

/// M13: the log panel opens with the motion tokens, not literal timings.
#[test]
fn log_panel_uses_the_motion_tokens() {
    use crate::theme::motion;
    let a = super::render::log_open_animation();
    assert_eq!(a.duration, motion::BASE);
    for t in [0.25_f32, 0.5, 0.75] {
        assert_eq!((a.easing)(t), motion::ease_out(t));
    }
}
