use std::path::PathBuf;
use std::sync::Mutex;

use telima_colima::cli::{ColimaCli, CommandOutput, CommandRunner, RunError, create_args};
use telima_core::ports::{PortError, VmManager};
use telima_core::profile::{
    MountType, ProfileName, ProfileSpec, ProfileStatus, ProgressEvent, ProgressLevel, Runtime,
    VmType,
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

struct Fake {
    calls: Mutex<Vec<Vec<String>>>,
    respond: Box<dyn Fn(&[String]) -> Reply + Send + Sync>,
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
    c.delete(&n).await.unwrap();
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
    assert!(matches!(c.delete(&n).await, Err(PortError::NotFound(_))));
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
async fn update_edits_only_cpu_memory_disk() {
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
        .replace("disk: 100", "disk: 120");
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
