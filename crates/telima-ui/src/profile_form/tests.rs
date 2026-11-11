//! Profile editor, delete confirmation and start progress log.
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use gpui_kit::{AppContext, Entity, Modifiers, TestAppContext, px, size};
use telima_app::profiles::Lifecycle;
use telima_app::runtime::{App, BoxFuture, DynEngine, Spawner};
use telima_app::testing::{FakeEngine, FakeVm};
use telima_app::view::confirm_delete_profile;
use telima_core::profile::{HostOs, MountType, ProfileName, ProfileSpec, Runtime, VmType};

use super::progress::CURRENT_STEP_FILL;
use super::{DeleteConfirm, DeleteEvent, Field, ProfileEditor, StartProgress};
use crate::theme::metric;

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

fn setup(lines: &[&str]) -> (Arc<FakeVm>, Arc<QueueSpawner>, App) {
    let vm = Arc::new(FakeVm {
        progress_lines: lines.iter().map(|l| l.to_string()).collect(),
        ..FakeVm::default()
    });
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
    (vm, spawner, app)
}

fn spec(name: &str) -> ProfileSpec {
    ProfileSpec {
        name: ProfileName::parse(name).unwrap(),
        cpus: 2,
        memory_gib: 2,
        disk_gib: 20,
        vm_type: VmType::Vz,
        mount_type: MountType::Virtiofs,
        runtime: Runtime::Docker,
        rosetta: false,
        network_address: false,
    }
}

#[gpui_kit::test]
fn invalid_spec_shows_a_violation_and_does_not_call_create(cx: &mut TestAppContext) {
    let (vm, spawner, app) = setup(&[]);
    let form: Entity<ProfileEditor> = cx.new(|cx| ProfileEditor::create(app, HostOs::MacOs, cx));
    form.update(cx, |f, cx| {
        f.set_name("telima-test-a", cx);
        f.set_cpus(0, cx);
        f.submit(cx);
    });
    spawner.run_all();
    form.update(cx, |f, cx| f.pump(cx));

    let msgs = form.read_with(cx, |f, _| f.violations_for(Field::Cpus));
    assert_eq!(msgs.len(), 1, "cpus=0 must show one inline violation");
    assert!(!vm.calls().iter().any(|c| c.starts_with("create")));

    form.update(cx, |f, cx| {
        f.set_cpus(2, cx);
        f.submit(cx);
    });
    spawner.run_all();
    form.update(cx, |f, cx| f.pump(cx));
    assert!(vm.calls().contains(&"create telima-test-a".to_string()));
}

#[gpui_kit::test]
fn create_offers_only_the_rule_options_for_the_host(cx: &mut TestAppContext) {
    let (_, _, app) = setup(&[]);
    let form = cx.new(|cx| ProfileEditor::create(app, HostOs::Linux, cx));
    let (vms, mounts) = form.read_with(cx, |f, _| {
        (f.vm_options().to_vec(), f.mount_options().to_vec())
    });
    assert_eq!(vms, [VmType::Qemu]);
    assert!(!mounts.contains(&MountType::Virtiofs));
    assert!(!form.read_with(cx, |f, _| f.rosetta_available()));
}

#[gpui_kit::test]
fn edit_locks_immutable_fields_and_disk_only_grows(cx: &mut TestAppContext) {
    let (vm, spawner, app) = setup(&[]);
    vm.specs.lock().unwrap().push(spec("dev"));
    let form = cx.new(|cx| ProfileEditor::edit(app, HostOs::MacOs, spec("dev"), cx));

    form.read_with(cx, |f, _| {
        assert!(f.is_locked(Field::Name));
        assert!(f.is_locked(Field::VmType));
        assert!(!f.is_locked(Field::Cpus));
    });
    form.update(cx, |f, cx| {
        f.set_name("other", cx);
        f.set_vm_type(VmType::Qemu, cx);
    });
    form.read_with(cx, |f, _| {
        assert_eq!(f.name(), "dev");
        assert_eq!(f.draft().vm_type, VmType::Vz);
    });

    form.update(cx, |f, cx| {
        f.set_disk(10, cx);
        f.submit(cx);
    });
    spawner.run_all();
    form.update(cx, |f, cx| f.pump(cx));
    assert_eq!(
        form.read_with(cx, |f, _| f.violations_for(Field::Disk).len()),
        1
    );
    assert!(!vm.calls().iter().any(|c| c.starts_with("update")));

    form.update(cx, |f, cx| {
        f.set_disk(40, cx);
        f.submit(cx);
    });
    spawner.run_all();
    form.update(cx, |f, cx| f.pump(cx));
    assert!(vm.calls().contains(&"update dev".to_string()));
}

#[gpui_kit::test]
fn delete_is_disabled_until_the_typed_name_matches(cx: &mut TestAppContext) {
    let (vm, spawner, app) = setup(&[]);
    let name = ProfileName::parse("dev").unwrap();
    let copy = confirm_delete_profile(&name, 2, 1, 1 << 30);
    let dialog = cx.new(|cx| DeleteConfirm::new(app, name, copy, cx));

    assert!(!dialog.read_with(cx, |d, _| d.can_delete()));
    dialog.update(cx, |d, cx| {
        d.set_typed("de", cx);
        d.confirm(cx);
    });
    spawner.run_all();
    assert!(!vm.calls().iter().any(|c| c.starts_with("delete")));

    dialog.update(cx, |d, cx| d.set_typed("dev", cx));
    assert!(dialog.read_with(cx, |d, _| d.can_delete()));
    dialog.update(cx, |d, cx| d.confirm(cx));
    spawner.run_all();
    dialog.update(cx, |d, cx| d.pump(cx));
    assert!(vm.calls().contains(&"delete dev".to_string()));
}

#[gpui_kit::test]
fn start_progress_lines_appear_and_cancel_stops_the_log(cx: &mut TestAppContext) {
    let (vm, spawner, app) = setup(&["starting colima", "runtime: docker"]);
    let name = ProfileName::parse("telima-test-a").unwrap();
    let log = cx.new(|cx| StartProgress::new(app, name, Lifecycle::Start, cx));
    assert!(log.read_with(cx, |l, _| l.lines().is_empty()));
    assert!(log.read_with(cx, |l, _| l.is_running()));

    spawner.run_all();
    log.update(cx, |l, cx| l.pump(cx));
    let lines = log.read_with(cx, |l, _| l.lines());
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("INFO["), "{}", lines[0]);
    assert!(lines[0].ends_with("starting colima"));
    assert!(vm.calls().contains(&"start telima-test-a".to_string()));
    assert!(!log.read_with(cx, |l, _| l.is_running()));
    assert_eq!(log.read_with(cx, |l, _| l.elapsed_label()).len(), 5);
}

#[gpui_kit::test]
fn cancel_marks_the_log_cancelled(cx: &mut TestAppContext) {
    let (_, spawner, app) = setup(&["starting colima"]);
    let name = ProfileName::parse("telima-test-a").unwrap();
    let log = cx.new(|cx| StartProgress::new(app, name, Lifecycle::Start, cx));
    log.update(cx, |l, cx| l.cancel(cx));
    spawner.run_all();
    log.update(cx, |l, cx| l.pump(cx));
    assert!(!log.read_with(cx, |l, _| l.is_running()));
    assert!(log.read_with(cx, |l, _| l.is_cancelled()));
    assert!(log.read_with(cx, |l, _| l.lines().is_empty()));
}

#[gpui_kit::test]
fn start_progress_maps_log_lines_to_the_six_steps(cx: &mut TestAppContext) {
    let (_, spawner, app) = setup(&["checking existing profile config", "booting vm"]);
    let name = ProfileName::parse("telima-test-a").unwrap();
    let log = cx.new(|cx| StartProgress::new(app, name, Lifecycle::Start, cx));
    assert_eq!(log.read_with(cx, |l, _| l.step_progress().current), 0);
    spawner.run_all();
    log.update(cx, |l, cx| l.pump(cx));
    let steps = log.read_with(cx, |l, _| l.step_progress());
    assert_eq!(steps.current, 3);
    assert_eq!(steps.text(), "3/6");
    assert_eq!(steps.label, "VM 부팅");
}

#[gpui_kit::test]
fn switch_and_segment_clicks_change_the_draft(cx: &mut TestAppContext) {
    let (_, _, app) = setup(&[]);
    let (form, cx) = cx.add_window_view(|_, cx| ProfileEditor::create(app, HostOs::MacOs, cx));
    cx.run_until_parked();
    assert!(!form.read_with(cx, |f, _| f.draft().network_address));
    let b = cx
        .debug_bounds("switch-network-address")
        .expect("network address switch rendered");
    cx.simulate_click(b.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(form.read_with(cx, |f, _| f.draft().network_address));

    let b = cx.debug_bounds("switch-network-address").unwrap();
    cx.simulate_click(b.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(!form.read_with(cx, |f, _| f.draft().network_address));

    let before = form.read_with(cx, |f, _| f.draft().mount_type);
    let b = cx
        .debug_bounds("segment-mount-1")
        .expect("mount segments rendered");
    cx.simulate_click(b.center(), Modifiers::none());
    cx.run_until_parked();
    assert_ne!(form.read_with(cx, |f, _| f.draft().mount_type), before);
}

#[gpui_kit::test]
fn delete_dialog_takes_focus_on_open_and_escape_closes_it(cx: &mut TestAppContext) {
    // N3: no click first; the dialog must own focus so Escape reaches it.
    let (_, _, app) = setup(&[]);
    let name = ProfileName::parse("dev").unwrap();
    let copy = confirm_delete_profile(&name, 0, 0, 0);
    let (dialog, cx) = cx.add_window_view(|_, cx| DeleteConfirm::new(app, name, copy, cx));
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    cx.update(|_, cx| {
        cx.subscribe(&dialog, move |_, ev: &DeleteEvent, _| {
            sink.lock().unwrap().push(ev.clone())
        })
        .detach()
    });
    cx.run_until_parked();
    assert!(
        cx.update(|window, cx| dialog.read(cx).has_focus(window)),
        "the dialog must take focus when it opens"
    );
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(*events.lock().unwrap(), [DeleteEvent::Cancelled]);
}

#[gpui_kit::test]
fn current_start_step_bar_is_partly_filled(cx: &mut TestAppContext) {
    // M8: Starting.dc.html draws the current step's bar 45% filled in busy.
    let (_, spawner, app) = setup(&["booting vm"]);
    let name = ProfileName::parse("telima-test-a").unwrap();
    let (log, cx) = cx.add_window_view(|_, cx| StartProgress::new(app, name, Lifecycle::Start, cx));
    spawner.run_all();
    log.update(cx, |l, cx| l.pump(cx));
    cx.run_until_parked();
    let current = log.read_with(cx, |l, _| l.step_progress().current);
    assert_eq!(current, 3);
    const TRACKS: [&str; 6] = [
        "step-track-0",
        "step-track-1",
        "step-track-2",
        "step-track-3",
        "step-track-4",
        "step-track-5",
    ];
    const FILLS: [&str; 6] = [
        "step-fill-0",
        "step-fill-1",
        "step-fill-2",
        "step-fill-3",
        "step-fill-4",
        "step-fill-5",
    ];
    let mut ratio = |i: usize| {
        let track = cx.debug_bounds(TRACKS[i]).expect("track");
        let fill = cx.debug_bounds(FILLS[i]).expect("fill");
        f32::from(fill.size.width) / f32::from(track.size.width)
    };
    assert!((ratio(current - 1) - CURRENT_STEP_FILL).abs() < 0.01);
    assert!((ratio(0) - 1.0).abs() < 0.01, "done steps are full");
    assert!(ratio(current) < 0.01, "pending steps are empty");
}

#[test]
fn profile_form_draws_the_error_triangle_shape_not_a_glyph() {
    // L4: errors use components::status (Triangle), not a text glyph.
    for (file, src) in [
        ("delete.rs", include_str!("delete.rs")),
        ("editor.rs", include_str!("editor.rs")),
        ("widgets.rs", include_str!("widgets.rs")),
        ("progress.rs", include_str!("progress.rs")),
    ] {
        assert!(!src.contains('\u{25B2}'), "{file} still draws the ▲ glyph");
    }
}

/// L7: the inline create form is capped at the old 420 dialog width even in a wide window, and
/// its buttons sit at the form's right edge, not the window's.
#[gpui_kit::test]
fn inline_form_is_capped_and_buttons_sit_at_its_right_edge(cx: &mut TestAppContext) {
    let (_, _, app) = setup(&[]);
    let (_, cx) = cx.add_window_view(|_, cx| ProfileEditor::create(app, HostOs::MacOs, cx));
    cx.simulate_resize(size(px(metric::WINDOW_W), px(metric::WINDOW_H)));
    cx.run_until_parked();
    let frame = cx
        .debug_bounds("profile-form-frame")
        .expect("form rendered");
    let cancel = cx.debug_bounds("form-cancel").expect("cancel rendered");
    let submit = cx.debug_bounds("form-submit").expect("submit rendered");
    assert!(
        frame.size.width <= px(metric::DIALOG_W),
        "form {frame:?} wider than {}",
        metric::DIALOG_W
    );
    assert!(submit.right() <= frame.right(), "submit outside form");
    assert!(cancel.left() >= frame.left(), "cancel outside form");
    assert!(
        frame.right() - submit.right() < px(metric::BTN_PAD_X * 4.0),
        "buttons not right-aligned to the form: frame {frame:?}, submit {submit:?}"
    );
}
