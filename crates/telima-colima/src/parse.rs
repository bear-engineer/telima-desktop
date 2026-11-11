//! Pure parsers for `colima` CLI output. No I/O.
use telima_core::profile::{
    Profile, ProfileName, ProfileStatus, ProgressEvent, ProgressLevel, Runtime,
};
use telima_core::version::ColimaVersion;

/// Failure to read a piece of `colima` output.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("line {line}: invalid JSON ({message})")]
    Json { line: usize, message: String },
    #[error("unknown runtime: {0:?}")]
    Runtime(String),
    #[error("invalid profile name: {0:?}")]
    Name(String),
    #[error("cannot read colima version from {0:?}")]
    Version(String),
}

/// Parse `colima ls -j` output (one JSON object per line).
pub fn parse_ls(output: &str) -> Result<Vec<Profile>, ParseError> {
    let mut profiles = Vec::new();
    for (idx, raw) in output.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).map_err(|e| ParseError::Json {
            line: idx + 1,
            message: e.to_string(),
        })?;
        profiles.push(profile_from_json(&v)?);
    }
    Ok(profiles)
}

fn profile_from_json(v: &serde_json::Value) -> Result<Profile, ParseError> {
    let name_raw = str_field(v, "name");
    let name = ProfileName::parse(name_raw).map_err(|_| ParseError::Name(name_raw.to_owned()))?;
    let status = match str_field(v, "status") {
        "Running" => ProfileStatus::Running,
        "Stopped" => ProfileStatus::Stopped,
        other => ProfileStatus::Other(other.to_owned()),
    };
    let runtime = match str_field(v, "runtime") {
        "docker" => Runtime::Docker,
        "containerd" => Runtime::Containerd,
        "incus" => Runtime::Incus,
        other => return Err(ParseError::Runtime(other.to_owned())),
    };
    let address = v
        .get("address")
        .and_then(|a| a.as_str())
        .filter(|a| !a.is_empty())
        .map(str::to_owned);
    Ok(Profile {
        name,
        status,
        arch: str_field(v, "arch").to_owned(),
        cpus: u32::try_from(u64_field(v, "cpus")).unwrap_or(u32::MAX),
        memory_bytes: u64_field(v, "memory"),
        disk_bytes: u64_field(v, "disk"),
        address,
        runtime,
    })
}

fn str_field<'a>(v: &'a serde_json::Value, key: &str) -> &'a str {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("")
}

fn u64_field(v: &serde_json::Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}

/// Parse one stderr line of a long-running colima command.
/// `time=... level=<l> msg="..."` yields level and message; any other line is Info with the whole text.
pub fn parse_log_line(line: &str) -> ProgressEvent {
    let plain = || ProgressEvent {
        level: ProgressLevel::Info,
        message: line.trim().to_owned(),
    };
    let Some(level_raw) = line
        .split_whitespace()
        .find_map(|t| t.strip_prefix("level="))
    else {
        return plain();
    };
    let Some(msg_start) = line.find("msg=\"") else {
        return plain();
    };
    let level = match level_raw {
        "warn" | "warning" => ProgressLevel::Warn,
        "error" | "fatal" => ProgressLevel::Error,
        _ => ProgressLevel::Info,
    };
    let mut message = String::new();
    let mut chars = line[msg_start + 5..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => break,
            '\\' => {
                if let Some(next) = chars.next() {
                    message.push(next);
                }
            }
            _ => message.push(c),
        }
    }
    ProgressEvent { level, message }
}

/// What `colima version` prints: colima's own version plus the docker versions it lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    pub colima: ColimaVersion,
    pub docker_client: Option<String>,
    pub docker_server: Option<String>,
}

/// Parse the first line of `colima version` output, e.g. `colima version 0.10.3`.
pub fn parse_version(output: &str) -> Result<ColimaVersion, ParseError> {
    let first = output.lines().next().unwrap_or("");
    let bad = || ParseError::Version(first.to_owned());
    let token = first
        .split_whitespace()
        .find(|t| {
            t.trim_start_matches('v')
                .starts_with(|c: char| c.is_ascii_digit())
        })
        .ok_or_else(bad)?;
    ColimaVersion::parse(token).map_err(|_| bad())
}

/// Parse the whole `colima version` output (`client:` / `server:` lines are optional).
pub fn parse_version_info(output: &str) -> Result<VersionInfo, ParseError> {
    let colima = parse_version(output)?;
    let field = |key: &str| {
        output.lines().find_map(|l| {
            l.strip_prefix(key)
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        })
    };
    Ok(VersionInfo {
        colima,
        docker_client: field("client:"),
        docker_server: field("server:"),
    })
}

/// The fields of `colima status -p <name> --json` we use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusInfo {
    pub runtime: Runtime,
    pub kubernetes: bool,
    pub address: Option<String>,
    pub cpus: u32,
}

/// Parse `colima status --json` output (one object; no uptime field exists in 0.10.3).
pub fn parse_status(output: &str) -> Result<StatusInfo, ParseError> {
    let v: serde_json::Value =
        serde_json::from_str(output.trim()).map_err(|e| ParseError::Json {
            line: 1,
            message: e.to_string(),
        })?;
    let runtime = match str_field(&v, "runtime") {
        "docker" => Runtime::Docker,
        "containerd" => Runtime::Containerd,
        "incus" => Runtime::Incus,
        other => return Err(ParseError::Runtime(other.to_owned())),
    };
    Ok(StatusInfo {
        runtime,
        kubernetes: v
            .get("kubernetes")
            .and_then(|k| k.as_bool())
            .unwrap_or(false),
        address: Some(str_field(&v, "ip_address"))
            .filter(|a| !a.is_empty())
            .map(str::to_owned),
        cpus: u32::try_from(u64_field(&v, "cpu")).unwrap_or(u32::MAX),
    })
}
