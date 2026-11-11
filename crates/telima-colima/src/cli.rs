//! `VmManager` over the `colima` CLI. Process execution sits behind [`CommandRunner`].
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use telima_core::ports::{PortError, PortResult, ProgressSink, VmManager};
use telima_core::profile::{
    DeleteOptions, Mount, MountType, Profile, ProfileName, ProfileSpec, ProfileStatus,
    ProgressLevel, Runtime, RuntimeInfo, VmType,
};
use telima_core::time::UnixMillis;
use telima_core::version::ColimaVersion;

use crate::locate::locate_colima;
use crate::parse::{parse_log_line, parse_ls, parse_status, parse_version, parse_version_info};

/// Result of a finished process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// Exit code; `None` when killed by a signal.
    pub code: Option<i32>,
    pub stdout: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    #[error("program not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Io(String),
}

/// Runs one process. `argv[0]` is the program. Each stderr line goes to `on_stderr` as it arrives.
pub trait CommandRunner: Send + Sync {
    fn run(
        &self,
        argv: &[String],
        on_stderr: &mut (dyn FnMut(&str) + Send),
    ) -> impl Future<Output = Result<CommandOutput, RunError>> + Send;
}

/// Real runner on tokio.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioRunner;

impl CommandRunner for TokioRunner {
    async fn run(
        &self,
        argv: &[String],
        on_stderr: &mut (dyn FnMut(&str) + Send),
    ) -> Result<CommandOutput, RunError> {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
        let (program, rest) = argv
            .split_first()
            .ok_or_else(|| RunError::Io("empty argv".into()))?;
        let mut child = tokio::process::Command::new(program)
            .args(rest)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => RunError::NotFound(program.clone()),
                _ => RunError::Io(format!("{program}: {e}")),
            })?;
        let mut out = child.stdout.take().expect("stdout piped");
        let mut err = BufReader::new(child.stderr.take().expect("stderr piped")).lines();
        let read_out = async {
            let mut buf = Vec::new();
            out.read_to_end(&mut buf).await.map(|_| buf)
        };
        let read_err = async {
            while let Some(line) = err.next_line().await? {
                on_stderr(&line);
            }
            Ok::<(), std::io::Error>(())
        };
        let (stdout, errs) = tokio::join!(read_out, read_err);
        let io = |e: std::io::Error| RunError::Io(e.to_string());
        let stdout = stdout.map_err(io)?;
        errs.map_err(io)?;
        let status = child.wait().await.map_err(io)?;
        Ok(CommandOutput {
            code: status.code(),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
        })
    }
}

fn vm_type_arg(v: VmType) -> &'static str {
    match v {
        VmType::Vz => "vz",
        VmType::Qemu => "qemu",
        VmType::Krunkit => "krunkit",
    }
}

fn mount_type_arg(m: MountType) -> &'static str {
    match m {
        MountType::Virtiofs => "virtiofs",
        MountType::Sshfs => "sshfs",
        MountType::NineP => "9p",
    }
}

fn runtime_arg(r: Runtime) -> &'static str {
    match r {
        Runtime::Docker => "docker",
        Runtime::Containerd => "containerd",
        Runtime::Incus => "incus",
    }
}

/// argv (without the program) for `colima start` creating a new profile.
pub fn create_args(spec: &ProfileSpec) -> Vec<String> {
    let mut a: Vec<String> = [
        "start",
        spec.name.as_str(),
        "--cpus",
        &spec.cpus.to_string(),
        "--memory",
        &spec.memory_gib.to_string(),
        "--disk",
        &spec.disk_gib.to_string(),
        "--vm-type",
        vm_type_arg(spec.vm_type),
        "--mount-type",
        mount_type_arg(spec.mount_type),
        "--runtime",
        runtime_arg(spec.runtime),
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if spec.rosetta {
        a.push("--vz-rosetta".into());
    }
    if spec.network_address {
        a.push("--network-address".into());
    }
    for m in &spec.mounts {
        a.push("--mount".into());
        a.push(if m.writable {
            format!("{}:w", m.path)
        } else {
            m.path.clone()
        });
    }
    if spec.kubernetes {
        a.push("--kubernetes".into());
    }
    a
}

/// Name of the lima instance behind a colima profile (`colima` for default, else `colima-<name>`).
pub fn lima_instance_name(name: &ProfileName) -> String {
    if name.as_str() == "default" {
        "colima".to_owned()
    } else {
        format!("colima-{name}")
    }
}

/// argv (without the program) for `colima delete`.
fn delete_args(name: &ProfileName, options: DeleteOptions) -> Vec<String> {
    let mut a = vec!["delete".to_owned(), "-f".to_owned()];
    if options.data {
        a.push("--data".into());
    }
    a.push(name.to_string());
    a
}

/// Replace the top-level block `key:` (its line plus following indented/blank lines) with `new`.
/// Comment lines before the key and everything after the block are kept.
fn yaml_replace_block(text: &str, key: &str, new: &str) -> String {
    let prefix = format!("{key}:");
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let Some(start) = lines.iter().position(|l| l.starts_with(&prefix)) else {
        let mut out = text.to_owned();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(new);
        return out;
    };
    let mut end = start + 1;
    while end < lines.len() && lines[end].starts_with([' ', '\t', '-']) {
        end += 1;
    }
    // keep the blank lines that separate this block from the next key
    while end > start + 1 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    let mut out: String = lines[..start].concat();
    out.push_str(new);
    out.push_str(&lines[end..].concat());
    out
}

fn yaml_set_mounts(text: &str, mounts: &[Mount]) -> String {
    let new = if mounts.is_empty() {
        "mounts: []\n".to_owned()
    } else {
        let mut s = String::from("mounts:\n");
        for m in mounts {
            s.push_str(&format!(
                "  - location: {}\n    writable: {}\n",
                m.path, m.writable
            ));
        }
        s
    };
    yaml_replace_block(text, "mounts", &new)
}

/// Set `enabled:` inside the top-level `kubernetes:` block; append the block when missing.
fn yaml_set_kubernetes(text: &str, enabled: bool) -> String {
    yaml_set_nested(text, "kubernetes", "enabled", enabled)
}

/// Sets `block:\n  key: value` (first occurrence inside the top-level `block`), keeping the
/// rest of the file; appends the key to the block, or the whole block, when missing.
fn yaml_set_nested(text: &str, block: &str, key: &str, value: impl std::fmt::Display) -> String {
    let head = format!("{block}:");
    let field = format!("{key}:");
    let mut in_block = false;
    let mut done = false;
    let mut out: String = text
        .split_inclusive('\n')
        .map(|l| {
            if l.starts_with(&head) {
                in_block = true;
            } else if in_block && !l.starts_with([' ', '\t']) && !l.trim().is_empty() {
                in_block = false;
            } else if in_block && !done && l.trim_start().starts_with(&field) {
                done = true;
                let indent = &l[..l.len() - l.trim_start().len()];
                let nl = if l.ends_with('\n') { "\n" } else { "" };
                return format!("{indent}{key}: {value}{nl}");
            }
            l.to_owned()
        })
        .collect();
    if done {
        return out;
    }
    if let Some(at) = out.lines().position(|l| l.starts_with(&head)) {
        let mut lines: Vec<String> = out.split_inclusive('\n').map(str::to_owned).collect();
        lines.insert(at + 1, format!("  {key}: {value}\n"));
        return lines.concat();
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("{head}\n  {key}: {value}\n"));
    out
}

fn yaml_kubernetes_enabled(text: &str) -> bool {
    let mut in_block = false;
    for l in text.lines() {
        if l.starts_with("kubernetes:") {
            in_block = true;
        } else if in_block && !l.starts_with([' ', '\t']) && !l.trim().is_empty() {
            in_block = false;
        } else if in_block && let Some(v) = l.trim().strip_prefix("enabled:") {
            return strip_value(v) == "true";
        }
    }
    false
}

fn yaml_mounts(text: &str) -> Vec<Mount> {
    let mut mounts: Vec<Mount> = vec![];
    let mut in_block = false;
    for l in text.lines() {
        if l.starts_with("mounts:") {
            in_block = true;
        } else if in_block && !l.starts_with([' ', '\t', '-']) && !l.trim().is_empty() {
            in_block = false;
        } else if in_block {
            let t = l.trim().trim_start_matches("- ");
            if let Some(v) = t.strip_prefix("location:") {
                mounts.push(Mount {
                    path: strip_value(v).to_owned(),
                    writable: false,
                });
            } else if let Some(v) = t.strip_prefix("writable:")
                && let Some(last) = mounts.last_mut()
            {
                last.writable = strip_value(v) == "true";
            }
        }
    }
    mounts
}

// ---- colima.yaml: only the few top-level keys we read or write ----

fn strip_value(raw: &str) -> &str {
    let v = raw.split(" #").next().unwrap_or(raw).trim();
    v.trim_matches(|c| c == '"' || c == '\'')
}

/// Value of a top-level `key: value` line.
fn yaml_top(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        l.strip_prefix(key)
            .and_then(|r| r.strip_prefix(':'))
            .map(|r| strip_value(r).to_owned())
    })
}

/// `network:` block's `address:` value.
fn yaml_network_address(text: &str) -> bool {
    let mut in_net = false;
    for l in text.lines() {
        if l.starts_with("network:") {
            in_net = true;
        } else if in_net && !l.starts_with(' ') && !l.trim().is_empty() && !l.starts_with('#') {
            in_net = false;
        } else if in_net && let Some(v) = l.trim().strip_prefix("address:") {
            return strip_value(v) == "true";
        }
    }
    false
}

/// Replace a top-level scalar (append when missing); everything else is kept byte for byte.
fn yaml_set_top(text: &str, key: &str, value: impl std::fmt::Display) -> String {
    let prefix = format!("{key}:");
    let mut found = false;
    let mut out: String = text
        .split_inclusive('\n')
        .map(|l| {
            if !found && l.starts_with(&prefix) {
                found = true;
                let nl = if l.ends_with('\n') { "\n" } else { "" };
                format!("{key}: {value}{nl}")
            } else {
                l.to_owned()
            }
        })
        .collect();
    if !found {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!("{key}: {value}\n"));
    }
    out
}

fn parse_vm_type(s: &str) -> Option<VmType> {
    match s {
        "vz" => Some(VmType::Vz),
        "qemu" => Some(VmType::Qemu),
        "krunkit" => Some(VmType::Krunkit),
        _ => None,
    }
}

fn parse_mount_type(s: &str) -> Option<MountType> {
    match s {
        "virtiofs" => Some(MountType::Virtiofs),
        "sshfs" => Some(MountType::Sshfs),
        "9p" => Some(MountType::NineP),
        _ => None,
    }
}

fn parse_runtime(s: &str) -> Option<Runtime> {
    match s {
        "docker" => Some(Runtime::Docker),
        "containerd" => Some(Runtime::Containerd),
        "incus" => Some(Runtime::Incus),
        _ => None,
    }
}

fn gib_ceil(v: &str) -> Option<u32> {
    v.parse::<f64>()
        .ok()
        .filter(|f| *f >= 0.0)
        .map(|f| f.ceil() as u32)
}

fn bytes_to_gib(b: u64) -> u32 {
    b.div_ceil(1 << 30) as u32
}

pub struct ColimaCli<R: CommandRunner = TokioRunner> {
    runner: R,
    bin: PathBuf,
    home: PathBuf,
}

impl ColimaCli<TokioRunner> {
    /// Find `colima` (PATH + fixed fallbacks) and use `~/.colima`.
    pub fn discover() -> PortResult<Self> {
        let env = |k: &str| std::env::var(k).ok();
        let bin = locate_colima(env, |p| p.is_file())
            .ok_or_else(|| PortError::Unavailable("colima is not installed".into()))?;
        let home = env("HOME")
            .filter(|h| !h.is_empty())
            .ok_or_else(|| PortError::Unavailable("HOME is not set".into()))?;
        Ok(Self::new(
            TokioRunner,
            bin,
            Path::new(&home).join(".colima"),
        ))
    }
}

impl<R: CommandRunner> ColimaCli<R> {
    /// `bin`: colima executable; `home`: the colima config dir (normally `~/.colima`).
    pub fn new(runner: R, bin: PathBuf, home: PathBuf) -> Self {
        Self { runner, bin, home }
    }

    fn yaml_path(&self, name: &ProfileName) -> PathBuf {
        self.home.join(name.as_str()).join("colima.yaml")
    }

    /// Run `colima <args>`; returns stdout. Stderr lines go to `progress` and the last ones
    /// become the error message on a non-zero exit.
    async fn exec(
        &self,
        args: Vec<String>,
        mut progress: Option<ProgressSink<'_>>,
    ) -> PortResult<String> {
        let mut argv = vec![self.bin.to_string_lossy().into_owned()];
        argv.extend(args);
        let mut last_error: Option<String> = None;
        let mut last_line: Option<String> = None;
        let mut on_line = |line: &str| {
            if line.trim().is_empty() {
                return;
            }
            let ev = parse_log_line(line);
            if ev.level == ProgressLevel::Error {
                last_error = Some(ev.message.clone());
            }
            last_line = Some(ev.message.clone());
            if let Some(p) = progress.as_mut() {
                p(ev);
            }
        };
        let out = self
            .runner
            .run(&argv, &mut on_line)
            .await
            .map_err(|e| match e {
                RunError::NotFound(p) => PortError::Unavailable(format!("not found: {p}")),
                RunError::Io(m) => PortError::Failed(m),
            })?;
        if out.code == Some(0) {
            return Ok(out.stdout);
        }
        let why = last_error
            .or(last_line)
            .unwrap_or_else(|| "no output".into());
        let code = out.code.map_or("signal".to_owned(), |c| c.to_string());
        Err(PortError::Failed(format!(
            "colima {} failed (exit {code}): {why}",
            argv.get(1).map_or("", String::as_str)
        )))
    }

    async fn find(&self, name: &ProfileName) -> PortResult<Profile> {
        self.list()
            .await?
            .into_iter()
            .find(|p| &p.name == name)
            .ok_or_else(|| PortError::NotFound(format!("profile {name}")))
    }

    async fn run_on_existing(
        &self,
        verb: &str,
        name: &ProfileName,
        progress: ProgressSink<'_>,
    ) -> PortResult<()> {
        self.find(name).await?;
        self.exec(vec![verb.into(), name.to_string()], Some(progress))
            .await
            .map(drop)
    }
}

impl<R: CommandRunner> VmManager for ColimaCli<R> {
    async fn list(&self) -> PortResult<Vec<Profile>> {
        let out = self.exec(vec!["ls".into(), "-j".into()], None).await?;
        parse_ls(&out).map_err(|e| PortError::Failed(e.to_string()))
    }

    async fn spec(&self, name: &ProfileName) -> PortResult<ProfileSpec> {
        let profile = self.find(name).await?;
        let text = std::fs::read_to_string(self.yaml_path(name)).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PortError::NotFound(format!("colima.yaml of {name}"))
            } else {
                PortError::Failed(format!("reading colima.yaml: {e}"))
            }
        })?;
        let num = |key: &str| yaml_top(&text, key).as_deref().and_then(gib_ceil);
        let vm_type = yaml_top(&text, "vmType")
            .as_deref()
            .and_then(parse_vm_type)
            .unwrap_or(if cfg!(target_os = "macos") {
                VmType::Vz
            } else {
                VmType::Qemu
            });
        let mount_type = yaml_top(&text, "mountType")
            .as_deref()
            .and_then(parse_mount_type)
            .unwrap_or(if vm_type == VmType::Qemu {
                MountType::Sshfs
            } else {
                MountType::Virtiofs
            });
        Ok(ProfileSpec {
            name: name.clone(),
            cpus: num("cpu").unwrap_or(profile.cpus),
            memory_gib: num("memory").unwrap_or_else(|| bytes_to_gib(profile.memory_bytes)),
            disk_gib: num("disk").unwrap_or_else(|| bytes_to_gib(profile.disk_bytes)),
            vm_type,
            mount_type,
            runtime: yaml_top(&text, "runtime")
                .as_deref()
                .and_then(parse_runtime)
                .unwrap_or(profile.runtime),
            rosetta: yaml_top(&text, "rosetta").as_deref() == Some("true"),
            network_address: yaml_network_address(&text),
            mounts: yaml_mounts(&text),
            kubernetes: yaml_kubernetes_enabled(&text),
        })
    }

    async fn create(&self, spec: &ProfileSpec, progress: ProgressSink<'_>) -> PortResult<()> {
        self.exec(create_args(spec), Some(progress)).await.map(drop)
    }

    async fn start(&self, name: &ProfileName, progress: ProgressSink<'_>) -> PortResult<()> {
        self.run_on_existing("start", name, progress).await
    }

    async fn stop(&self, name: &ProfileName, progress: ProgressSink<'_>) -> PortResult<()> {
        self.run_on_existing("stop", name, progress).await
    }

    async fn restart(&self, name: &ProfileName, progress: ProgressSink<'_>) -> PortResult<()> {
        self.run_on_existing("restart", name, progress).await
    }

    async fn update(&self, spec: &ProfileSpec) -> PortResult<()> {
        self.find(&spec.name).await?;
        let path = self.yaml_path(&spec.name);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| PortError::Failed(format!("reading colima.yaml: {e}")))?;
        let text = yaml_set_top(&text, "cpu", spec.cpus);
        let text = yaml_set_top(&text, "memory", spec.memory_gib);
        let text = yaml_set_top(&text, "disk", spec.disk_gib);
        let text = yaml_set_mounts(&text, &spec.mounts);
        let text = yaml_set_kubernetes(&text, spec.kubernetes);
        let text = yaml_set_top(&text, "mountType", mount_type_arg(spec.mount_type));
        let text = yaml_set_top(&text, "rosetta", spec.rosetta);
        let text = yaml_set_nested(&text, "network", "address", spec.network_address);
        std::fs::write(&path, text)
            .map_err(|e| PortError::Failed(format!("writing colima.yaml: {e}")))
    }

    async fn delete(&self, name: &ProfileName, options: DeleteOptions) -> PortResult<()> {
        self.find(name).await?;
        self.exec(delete_args(name, options), None).await.map(drop)
    }

    async fn colima_version(&self) -> PortResult<ColimaVersion> {
        let out = self.exec(vec!["version".into()], None).await?;
        parse_version(&out).map_err(|e| PortError::Failed(e.to_string()))
    }

    /// Start time: colima 0.10.3 has no uptime field, so use the mtime of the lima instance's
    /// `ha.pid` (written when the host agent starts, i.e. at VM start); `None` when unreadable.
    async fn runtime_info(&self, name: &ProfileName) -> PortResult<RuntimeInfo> {
        let profile = self.find(name).await?;
        if profile.status != ProfileStatus::Running {
            return Ok(RuntimeInfo::default());
        }
        let status = self
            .exec(
                vec![
                    "status".into(),
                    "-p".into(),
                    name.to_string(),
                    "--json".into(),
                ],
                None,
            )
            .await?;
        parse_status(&status).map_err(|e| PortError::Failed(e.to_string()))?;
        let version = self
            .exec(vec!["version".into(), "-p".into(), name.to_string()], None)
            .await?;
        let v = parse_version_info(&version).map_err(|e| PortError::Failed(e.to_string()))?;
        Ok(RuntimeInfo {
            started_at: self.started_at(name),
            colima: Some(v.colima),
            docker_client: v.docker_client,
            docker_server: v.docker_server,
        })
    }

    async fn prune_cache(&self) -> PortResult<()> {
        self.exec(vec!["prune".into(), "-f".into()], None)
            .await
            .map(drop)
    }
}

impl<R: CommandRunner> ColimaCli<R> {
    fn started_at(&self, name: &ProfileName) -> Option<UnixMillis> {
        let pid = self
            .home
            .join("_lima")
            .join(lima_instance_name(name))
            .join("ha.pid");
        let modified = std::fs::metadata(pid).ok()?.modified().ok()?;
        let ms = modified
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis();
        Some(UnixMillis(i64::try_from(ms).ok()?))
    }
}
