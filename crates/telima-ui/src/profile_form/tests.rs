//! Profile editor, delete confirmation and start progress log.
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use gpui_kit::{AppContext, Entity, TestAppContext};
use telima_app::profiles::Lifecycle;
use telima_app::runtime::{App, BoxFuture, DynEngine, Spawner};
use telima_app::testing::{FakeEngine, FakeVm};
use telima_app::view::confirm_delete_profile;
use telima_core::profile::{HostOs, MountType, ProfileName, ProfileSpec, Runtime, VmType};

use super::{DeleteConfirm, Field, ProfileEditor, StartProgress};

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
    let (vms, mounts) = form.read_with(cx, |f, _| (f.vm_options().to_vec(), f.mount_options().to_vec()));
    assert_eq!(vms, [VmType::Qemu]);
    assert!(!mounts.contains(&MountType::Virtiofs));
    assert!(!form.read_with(cx, |f, _| f.rosetta_available()));
}

#[gpui_kit::test]
fn edit_locks_immutable_fields_and_disk_only_grows(cx: &mut TestAppContext) {
    let (vm, spawner, app) = setup(&[]);
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
    assert_eq!(form.read_with(cx, |f, _| f.violations_for(Field::Disk).len()), 1);
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
