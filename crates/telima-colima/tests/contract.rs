//! Talks to a real Colima. Run only by the verify node:
//! `cargo test -p telima-colima --test contract -- --ignored --nocapture`
//! Uses the throwaway profile `telima-test-a`; never touches `default`.
use telima_colima::cli::ColimaCli;
use telima_core::ports::VmManager;
use telima_core::profile::{
    DeleteOptions, MountType, ProfileName, ProfileSpec, ProfileStatus, Runtime, VmType,
};

const TEST_PROFILE: &str = "telima-test-a";

/// Deletes the test profile when dropped, so it goes away even if an assertion panics.
struct Cleanup(std::path::PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::process::Command::new(&self.0)
            .args(["delete", "-f", "--data", TEST_PROFILE])
            .stdin(std::process::Stdio::null())
            .output();
    }
}

fn host_spec() -> ProfileSpec {
    let (vm_type, mount_type) = if cfg!(target_os = "macos") {
        (VmType::Vz, MountType::Virtiofs)
    } else {
        (VmType::Qemu, MountType::Sshfs)
    };
    ProfileSpec {
        name: ProfileName::parse(TEST_PROFILE).unwrap(),
        cpus: 2,
        memory_gib: 2,
        disk_gib: 10,
        vm_type,
        mount_type,
        runtime: Runtime::Docker,
        rosetta: false,
        network_address: false,
        mounts: vec![],
        kubernetes: false,
    }
}

#[tokio::test]
#[ignore = "creates a real Colima VM"]
async fn create_list_stop_delete_roundtrip() {
    let cli = ColimaCli::discover().expect("colima installed");
    let bin = telima_colima::locate::locate_colima(|k| std::env::var(k).ok(), |p| p.is_file())
        .expect("colima path");
    let _cleanup = Cleanup(bin);
    let spec = host_spec();

    let mut lines = 0usize;
    cli.create(&spec, &mut |_| lines += 1)
        .await
        .expect("create");
    assert!(lines > 0, "expected progress lines");

    let listed = cli.list().await.expect("list");
    let me = listed
        .iter()
        .find(|p| p.name == spec.name)
        .expect("profile listed");
    assert_eq!(me.status, ProfileStatus::Running);

    let info = cli.runtime_info(&spec.name).await.expect("runtime_info");
    let started = info.started_at.expect("start time from ha.pid");
    assert!(
        started.elapsed_until(telima_core::time::UnixMillis(now_millis()))
            < std::time::Duration::from_secs(1800),
        "started_at should be recent"
    );
    assert!(info.docker_server.is_some(), "docker server version");
    assert_eq!(
        info.colima,
        Some(cli.colima_version().await.expect("version"))
    );

    cli.stop(&spec.name, &mut |_| {}).await.expect("stop");
    let listed = cli.list().await.expect("list after stop");
    let me = listed.iter().find(|p| p.name == spec.name).unwrap();
    assert_eq!(me.status, ProfileStatus::Stopped);

    let stopped = cli.runtime_info(&spec.name).await.expect("stopped info");
    assert_eq!(stopped, telima_core::profile::RuntimeInfo::default());

    cli.delete(&spec.name, DeleteOptions { data: true })
        .await
        .expect("delete --data");
    let listed = cli.list().await.expect("list after delete");
    assert!(listed.iter().all(|p| p.name != spec.name));
    let disks = std::path::Path::new(&std::env::var("HOME").unwrap()).join(".colima/_lima/_disks");
    assert!(
        !disks.join(TEST_PROFILE).exists()
            && !disks.join(format!("colima-{TEST_PROFILE}")).exists(),
        "--data must remove the data disk"
    );
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
