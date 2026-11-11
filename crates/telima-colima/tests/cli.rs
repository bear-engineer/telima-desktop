use std::path::PathBuf;
use std::sync::Mutex;

use telima_colima::cli::{
    ColimaCli, CommandOutput, CommandRunner, RunError, create_args, lima_instance_name,
};
use telima_core::ports::{PortError, VmManager};
use telima_core::profile::{
    DeleteOptions, Mount, MountType, ProfileName, ProfileSpec, ProfileStatus, ProgressEvent,
    ProgressLevel, Runtime, VmType,
};

const BIN: &str = "/fake/colima";
const LS_A: &str = r#"{"name":"alpha","status":"Stopped","arch":"aarch64","cpus":2,"memory":2147483648,"disk":10737418240,"runtime":"docker"}"#;

struct Reply {
    stderr: Vec<&'static str>,
    stdout: String,
    result: Result<i32, RunError>,
}

fn ok(stdout: &str) -> Reply {
    Reply {
        stderr: vec![],
        stdout: stdout.into(),
        result: Ok(0),
    }
}

type Responder = Box<dyn Fn(&[String]) -> Reply + Send + Sync>;

struct Fake {
    calls: Mutex<Vec<Vec<String>>>,
    respond: Responder,
}

impl Fake {
    fn new(respond: impl Fn(&[String]) -> Reply + Send + Sync + 'static) -> Self {
        Self {
            calls: Mutex::new(vec![]),
            respond: Box::new(respond),
        }
    }
    /// `colima ls -j` answers with `LS_A`; everything else succeeds silently.
    fn with_alpha() -> Self {
        Self::new(|argv| {
            if argv.get(1).map(String::as_str) == Some("ls") {
                ok(LS_A)
            } else {
                ok("")
            }
        })
    }
    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }
}

impl CommandRunner for &Fake {
    async fn run(
        &self,
        argv: &[String],
        on_stderr: &mut (dyn FnMut(&str) + Send),
    ) -> Result<CommandOutput, RunError> {
        self.calls.lock().unwrap().push(argv.to_vec());
        let reply = (self.respond)(argv);
        for l in &reply.stderr {
            on_stderr(l);
        }
        reply.result.map(|code| CommandOutput {
            code: Some(code),
            stdout: reply.stdout,
        })
    }
}

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn name(s: &str) -> ProfileName {
    ProfileName::parse(s).unwrap()
}

fn spec(n: &str) -> ProfileSpec {
    ProfileSpec {
        name: name(n),
        cpus: 2,
        memory_gib: 2,
        disk_gib: 10,
        vm_type: VmType::Vz,
        mount_type: MountType::Virtiofs,
        runtime: Runtime::Docker,
        rosetta: false,
        network_address: false,
        mounts: vec![],
        kubernetes: false,
    }
}

fn tmp_home(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("telima-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cli(fake: &Fake, home: PathBuf) -> ColimaCli<&Fake> {
    ColimaCli::new(fake, PathBuf::from(BIN), home)
}

#[test]
fn create_args_minimal() {
    assert_eq!(
        create_args(&spec("alpha")),
        args(&[
            "start",
            "alpha",
            "--cpus",
            "2",
            "--memory",
            "2",
            "--disk",
            "10",
            "--vm-type",
            "vz",
            "--mount-type",
            "virtiofs",
            "--runtime",
            "docker"
        ])
    );
}

#[test]
fn create_args_flags_and_names() {
    let mut s = spec("beta");
    s.vm_type = VmType::Qemu;
    s.mount_type = MountType::NineP;
    s.rosetta = true;
    s.network_address = true;
    let a = create_args(&s);
    assert!(a.windows(2).any(|w| w == ["--vm-type", "qemu"]));
    assert!(a.windows(2).any(|w| w == ["--mount-type", "9p"]));
    assert!(a.ends_with(&args(&["--vz-rosetta", "--network-address"])));
}

#[tokio::test]
async fn create_runs_start_and_streams_progress() {
    let fake = Fake::new(|_| Reply {
        stderr: vec![
            r#"time="2026-11-11T10:00:00+09:00" level=info msg="starting colima [profile=alpha]""#,
            r#"time="2026-11-11T10:00:01+09:00" level=warning msg="slow disk""#,
        ],
        stdout: String::new(),
        result: Ok(0),
    });
    let c = cli(&fake, tmp_home("create"));
    let mut events: Vec<ProgressEvent> = vec![];
    c.create(&spec("alpha"), &mut |e| events.push(e))
        .await
        .unwrap();
    let mut want = vec![BIN.to_string()];
    want.extend(create_args(&spec("alpha")));
    assert_eq!(fake.calls(), vec![want]);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].level, ProgressLevel::Info);
    assert_eq!(events[0].message, "starting colima [profile=alpha]");
    assert_eq!(events[1].level, ProgressLevel::Warn);
}

#[tokio::test]
async fn lifecycle_argv() {
    let fake = Fake::with_alpha();
    let c = cli(&fake, tmp_home("life"));
    let n = name("alpha");
    c.start(&n, &mut |_| {}).await.unwrap();
    c.stop(&n, &mut |_| {}).await.unwrap();
    c.restart(&n, &mut |_| {}).await.unwrap();
    c.delete(&n, DeleteOptions::default()).await.unwrap();
    let non_ls: Vec<Vec<String>> = fake.calls().into_iter().filter(|a| a[1] != "ls").collect();
    assert_eq!(
        non_ls,
        vec![
            args(&[BIN, "start", "alpha"]),
            args(&[BIN, "stop", "alpha"]),
            args(&[BIN, "restart", "alpha"]),
            args(&[BIN, "delete", "-f", "alpha"]),
        ]
    );
}

#[tokio::test]
async fn list_uses_ls_json() {
    let fake = Fake::with_alpha();
    let c = cli(&fake, tmp_home("list"));
    let got = c.list().await.unwrap();
    assert_eq!(fake.calls(), vec![args(&[BIN, "ls", "-j"])]);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].name, name("alpha"));
    assert_eq!(got[0].status, ProfileStatus::Stopped);
}

#[tokio::test]
async fn unknown_profile_is_not_found_without_running_command() {
    let fake = Fake::with_alpha();
    let c = cli(&fake, tmp_home("nf"));
    let n = name("ghost");
    assert!(matches!(
        c.start(&n, &mut |_| {}).await,
        Err(PortError::NotFound(_))
    ));
    assert!(matches!(
        c.stop(&n, &mut |_| {}).await,
        Err(PortError::NotFound(_))
    ));
    assert!(matches!(
        c.delete(&n, DeleteOptions::default()).await,
        Err(PortError::NotFound(_))
    ));
    assert!(matches!(c.spec(&n).await, Err(PortError::NotFound(_))));
    assert!(fake.calls().iter().all(|a| a[1] == "ls"));
}

#[tokio::test]
async fn missing_binary_is_unavailable() {
    let fake = Fake::new(|_| Reply {
        stderr: vec![],
        stdout: String::new(),
        result: Err(RunError::NotFound(BIN.into())),
    });
    let c = cli(&fake, tmp_home("missing"));
    assert!(matches!(c.list().await, Err(PortError::Unavailable(_))));
    assert!(matches!(
        c.create(&spec("alpha"), &mut |_| {}).await,
        Err(PortError::Unavailable(_))
    ));
}

#[tokio::test]
async fn nonzero_exit_is_failed_with_last_message() {
    let fake = Fake::new(|argv| {
        if argv[1] == "ls" {
            return ok(LS_A);
        }
        Reply {
            stderr: vec![r#"time="x" level=fatal msg="error starting vm: boom""#],
            stdout: String::new(),
            result: Ok(1),
        }
    });
    let c = cli(&fake, tmp_home("fail"));
    match c.start(&name("alpha"), &mut |_| {}).await {
        Err(PortError::Failed(m)) => assert!(m.contains("boom"), "{m}"),
        other => panic!("{other:?}"),
    }
}

const YAML: &str = "# colima config\ncpu: 8\ndisk: 100\nmemory: 8\nruntime: docker\nnetwork:\n  address: true\n  dns: []\nvmType: vz\nrosetta: true\nmountType: virtiofs\ncpuType: \"\"\n";

fn write_yaml(home: &std::path::Path, n: &str, body: &str) -> PathBuf {
    let dir = home.join(n);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("colima.yaml");
    std::fs::write(&p, body).unwrap();
    p
}

#[tokio::test]
async fn spec_reads_colima_yaml() {
    let fake = Fake::with_alpha();
    let home = tmp_home("spec");
    write_yaml(&home, "alpha", YAML);
    let s = cli(&fake, home).spec(&name("alpha")).await.unwrap();
    assert_eq!(s.cpus, 8);
    assert_eq!(s.memory_gib, 8);
    assert_eq!(s.disk_gib, 100);
    assert_eq!(s.vm_type, VmType::Vz);
    assert_eq!(s.mount_type, MountType::Virtiofs);
    assert_eq!(s.runtime, Runtime::Docker);
    assert!(s.rosetta);
    assert!(s.network_address);
}

#[tokio::test]
async fn update_edits_editable_fields_only() {
    let fake = Fake::with_alpha();
    let home = tmp_home("upd");
    let path = write_yaml(&home, "alpha", YAML);
    let mut s = spec("alpha");
    s.cpus = 4;
    s.memory_gib = 6;
    s.disk_gib = 120;
    s.vm_type = VmType::Qemu; // must be ignored
    cli(&fake, home).update(&s).await.unwrap();
    let got = std::fs::read_to_string(path).unwrap();
    let want = YAML
        .replace("cpu: 8", "cpu: 4")
        .replace("memory: 8", "memory: 6")
        .replace("disk: 100", "disk: 120")
        .replace("  address: true", "  address: false")
        .replace("rosetta: true", "rosetta: false")
        + "mounts: []\nkubernetes:\n  enabled: false\n"; // absent keys are appended
    assert_eq!(got, want);
    assert!(
        fake.calls().iter().all(|a| a[1] == "ls"),
        "update must not run colima start"
    );
}

#[tokio::test]
async fn update_unknown_profile_is_not_found() {
    let fake = Fake::with_alpha();
    let c = cli(&fake, tmp_home("updnf"));
    assert!(matches!(
        c.update(&spec("ghost")).await,
        Err(PortError::NotFound(_))
    ));
}

const LS_A_RUNNING: &str = r#"{"name":"alpha","status":"Running","arch":"aarch64","cpus":2,"memory":2147483648,"disk":10737418240,"runtime":"docker"}"#;
const STATUS_JSON: &str = include_str!("fixtures/status_running.json");
const VERSION_FULL: &str = include_str!("fixtures/colima_version_full.txt");

fn running_fake() -> Fake {
    Fake::new(|argv| match argv.get(1).map(String::as_str) {
        Some("ls") => ok(LS_A_RUNNING),
        Some("status") => ok(STATUS_JSON),
        Some("version") => ok(VERSION_FULL),
        _ => ok(""),
    })
}

fn mount(path: &str, writable: bool) -> Mount {
    Mount {
        path: path.into(),
        writable,
    }
}

#[test]
fn create_args_mounts_and_kubernetes() {
    let mut s = spec("alpha");
    s.mounts = vec![mount("/Users/example/src", true), mount("/data", false)];
    s.kubernetes = true;
    let a = create_args(&s);
    assert!(
        a.windows(2)
            .any(|w| w == ["--mount", "/Users/example/src:w"])
    );
    assert!(a.windows(2).any(|w| w == ["--mount", "/data"]));
    assert!(a.contains(&"--kubernetes".to_string()));
}

#[test]
fn create_args_without_mounts_has_neither_flag() {
    let a = create_args(&spec("alpha"));
    assert!(!a.iter().any(|x| x == "--mount" || x == "--kubernetes"));
}

#[test]
fn lima_instance_names() {
    assert_eq!(lima_instance_name(&name("default")), "colima");
    assert_eq!(lima_instance_name(&name("alpha")), "colima-alpha");
}

#[tokio::test]
async fn delete_with_data_passes_flag() {
    let fake = Fake::with_alpha();
    let c = cli(&fake, tmp_home("deldata"));
    c.delete(&name("alpha"), DeleteOptions { data: true })
        .await
        .unwrap();
    let last = fake.calls().pop().unwrap();
    assert_eq!(last, args(&[BIN, "delete", "-f", "--data", "alpha"]));
}

#[tokio::test]
async fn version_runs_colima_version() {
    let fake = running_fake();
    let c = cli(&fake, tmp_home("ver"));
    let v = c.colima_version().await.unwrap();
    assert_eq!(v, telima_core::version::ColimaVersion::new(0, 10, 3));
    assert_eq!(fake.calls(), vec![args(&[BIN, "version"])]);
}

#[tokio::test]
async fn prune_runs_forced_prune_only() {
    let fake = Fake::with_alpha();
    let c = cli(&fake, tmp_home("prune"));
    c.prune_cache().await.unwrap();
    assert_eq!(fake.calls(), vec![args(&[BIN, "prune", "-f"])]);
}

#[tokio::test]
async fn runtime_info_reads_versions_and_pid_file_time() {
    let fake = running_fake();
    let home = tmp_home("info");
    let dir = home.join("_lima/colima-alpha");
    std::fs::create_dir_all(&dir).unwrap();
    let pid = std::fs::File::create(dir.join("ha.pid")).unwrap();
    let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    pid.set_modified(t).unwrap();
    let info = cli(&fake, home).runtime_info(&name("alpha")).await.unwrap();
    assert_eq!(
        info.started_at,
        Some(telima_core::time::UnixMillis(1_700_000_000_000))
    );
    assert_eq!(info.docker_server.as_deref(), Some("v29.5.2"));
    assert_eq!(info.docker_client.as_deref(), Some("v29.9.3"));
    assert_eq!(
        info.colima,
        Some(telima_core::version::ColimaVersion::new(0, 10, 3))
    );
    let calls = fake.calls();
    assert!(calls.contains(&args(&[BIN, "status", "-p", "alpha", "--json"])));
    assert!(calls.contains(&args(&[BIN, "version", "-p", "alpha"])));
}

#[tokio::test]
async fn runtime_info_without_pid_file_has_no_start_time() {
    let fake = running_fake();
    let info = cli(&fake, tmp_home("nopid"))
        .runtime_info(&name("alpha"))
        .await
        .unwrap();
    assert_eq!(info.started_at, None);
    assert!(info.colima.is_some());
}

#[tokio::test]
async fn runtime_info_of_stopped_profile_is_empty_and_runs_nothing_else() {
    let fake = Fake::with_alpha();
    let info = cli(&fake, tmp_home("stopped"))
        .runtime_info(&name("alpha"))
        .await
        .unwrap();
    assert_eq!(info, telima_core::profile::RuntimeInfo::default());
    assert!(fake.calls().iter().all(|a| a[1] == "ls"));
}

#[tokio::test]
async fn runtime_info_unknown_profile_is_not_found() {
    let fake = Fake::with_alpha();
    let c = cli(&fake, tmp_home("infonf"));
    assert!(matches!(
        c.runtime_info(&name("ghost")).await,
        Err(PortError::NotFound(_))
    ));
}

const YAML_MK: &str = "cpu: 2\nkubernetes:\n  # Enable kubernetes.\n  enabled: false\n  version: v1\n\n# Mounts\nmounts:\n  - location: /old\n    writable: true\n\nvmType: vz\n";

#[tokio::test]
async fn spec_reads_mounts_and_kubernetes() {
    let fake = Fake::with_alpha();
    let home = tmp_home("specmk");
    write_yaml(
        &home,
        "alpha",
        &YAML_MK.replace("enabled: false", "enabled: true"),
    );
    let s = cli(&fake, home).spec(&name("alpha")).await.unwrap();
    assert!(s.kubernetes);
    assert_eq!(s.mounts, vec![mount("/old", true)]);
}

#[tokio::test]
async fn update_writes_mounts_and_kubernetes_keeping_the_rest() {
    let fake = Fake::with_alpha();
    let home = tmp_home("updmk");
    let path = write_yaml(&home, "alpha", YAML_MK);
    let mut s = spec("alpha");
    s.cpus = 2;
    s.memory_gib = 2;
    s.disk_gib = 10;
    s.kubernetes = true;
    s.mounts = vec![mount("/a", false), mount("/b", true)];
    cli(&fake, home).update(&s).await.unwrap();
    let got = std::fs::read_to_string(path).unwrap();
    assert!(got.contains("  enabled: true\n  version: v1\n"), "{got}");
    assert!(got.contains(
        "mounts:\n  - location: /a\n    writable: false\n  - location: /b\n    writable: true\n\nvmType: vz\n"
    ), "{got}");
    assert!(got.contains("# Mounts\n"));
    assert!(!got.contains("/old"));
}

#[tokio::test]
async fn update_with_no_mounts_writes_empty_list() {
    let fake = Fake::with_alpha();
    let home = tmp_home("updempty");
    let path = write_yaml(&home, "alpha", YAML_MK);
    let mut s = spec("alpha");
    s.cpus = 2;
    cli(&fake, home).update(&s).await.unwrap();
    let got = std::fs::read_to_string(path).unwrap();
    assert!(got.contains("mounts: []\n\nvmType: vz\n"), "{got}");
}

#[tokio::test]
async fn update_writes_mount_type_rosetta_and_network_address() {
    let fake = Fake::with_alpha();
    let home = tmp_home("updmrn");
    let path = write_yaml(&home, "alpha", YAML);
    let mut s = spec("alpha");
    s.mount_type = MountType::Sshfs;
    s.rosetta = false;
    s.network_address = false;
    cli(&fake, home.clone()).update(&s).await.unwrap();
    let got = std::fs::read_to_string(&path).unwrap();
    assert!(got.contains("mountType: sshfs\n"), "{got}");
    assert!(got.contains("rosetta: false\n"), "{got}");
    assert!(
        got.contains("network:\n  address: false\n  dns: []\n"),
        "{got}"
    );
    assert!(
        got.contains("vmType: vz\n"),
        "vmType is not editable: {got}"
    );
    let back = cli(&fake, home).spec(&name("alpha")).await.unwrap();
    assert_eq!(back.mount_type, MountType::Sshfs);
    assert!(!back.rosetta);
    assert!(!back.network_address);
}

#[tokio::test]
async fn update_adds_network_block_when_missing() {
    let fake = Fake::with_alpha();
    let home = tmp_home("updnet");
    let path = write_yaml(&home, "alpha", "cpu: 2\nmemory: 2\ndisk: 10\n");
    let mut s = spec("alpha");
    s.network_address = true;
    cli(&fake, home).update(&s).await.unwrap();
    let got = std::fs::read_to_string(path).unwrap();
    assert!(got.contains("network:\n  address: true\n"), "{got}");
    assert!(got.contains("mountType: virtiofs\n"), "{got}");
}
