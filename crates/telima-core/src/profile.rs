//! Colima profile entities and value objects.
use std::fmt;
use std::time::Duration;

use crate::time::UnixMillis;
use crate::version::ColimaVersion;

/// Host operating system. Decides which VM options exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOs {
    MacOs,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmType {
    Vz,
    Qemu,
    Krunkit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountType {
    Virtiofs,
    Sshfs,
    NineP,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runtime {
    Docker,
    Containerd,
    Incus,
}

/// Validated profile name: 1-32 chars of `a-z`, `0-9`, `-`, not starting with `-`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProfileName(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid profile name: {0:?}")]
pub struct InvalidProfileName(pub String);

impl ProfileName {
    pub const MAX_LEN: usize = 32;

    pub fn parse(raw: &str) -> Result<Self, InvalidProfileName> {
        let valid_chars = raw
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if raw.is_empty() || raw.len() > Self::MAX_LEN || raw.starts_with('-') || !valid_chars {
            return Err(InvalidProfileName(raw.to_owned()));
        }
        Ok(Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What the user asks for when creating or editing a profile.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileSpec {
    pub name: ProfileName,
    pub cpus: u32,
    pub memory_gib: u32,
    pub disk_gib: u32,
    pub vm_type: VmType,
    pub mount_type: MountType,
    pub runtime: Runtime,
    pub rosetta: bool,
    pub network_address: bool,
    /// Host directories shared with the VM. Empty means Colima's default (the home directory).
    pub mounts: Vec<Mount>,
    /// Run Kubernetes (k3s) inside the VM.
    pub kubernetes: bool,
}

/// A host directory mounted into the VM at the same path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Mount {
    pub path: String,
    pub writable: bool,
}

/// What `delete` removes besides the VM itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeleteOptions {
    /// Also delete the data disk (images, containers, volumes). Off by default.
    pub data: bool,
}

/// Facts about a running profile that `list` does not report. Each is `None` when unknown.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeInfo {
    pub started_at: Option<UnixMillis>,
    pub colima: Option<ColimaVersion>,
    /// Docker client version as reported, without a leading `v`.
    pub docker_client: Option<String>,
    /// Docker server (engine in the VM) version as reported, without a leading `v`.
    pub docker_server: Option<String>,
}

impl RuntimeInfo {
    /// How long the profile has been running at `now`.
    pub fn uptime(&self, now: UnixMillis) -> Option<Duration> {
        self.started_at.map(|started| started.elapsed_until(now))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileStatus {
    Running,
    Stopped,
    /// Status text the adapter did not recognise, kept for display.
    Other(String),
}

/// A profile as reported by Colima.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub name: ProfileName,
    pub status: ProfileStatus,
    pub arch: String,
    pub cpus: u32,
    pub memory_bytes: u64,
    pub disk_bytes: u64,
    pub address: Option<String>,
    pub runtime: Runtime,
}

/// One line of progress from a long-running VM command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressEvent {
    pub level: ProgressLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressLevel {
    Info,
    Warn,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uptime_is_measured_from_the_start_time() {
        let info = RuntimeInfo {
            started_at: Some(UnixMillis::from_secs(1_000)),
            ..RuntimeInfo::default()
        };
        assert_eq!(
            info.uptime(UnixMillis::from_secs(1_000 + 3 * 3600 + 12 * 60)),
            Some(Duration::from_secs(3 * 3600 + 12 * 60))
        );
    }

    #[test]
    fn uptime_is_unknown_without_a_start_time() {
        let info = RuntimeInfo {
            colima: Some(ColimaVersion::new(0, 10, 3)),
            docker_client: Some("29.9.3".into()),
            docker_server: Some("29.5.2".into()),
            ..RuntimeInfo::default()
        };
        assert_eq!(info.uptime(UnixMillis::from_secs(5)), None);
    }

    #[test]
    fn delete_keeps_data_by_default() {
        assert_eq!(DeleteOptions::default(), DeleteOptions { data: false });
    }

    #[test]
    fn accepts_simple_names() {
        for ok in ["default", "telima-test-a", "dev2", "a"] {
            assert_eq!(ProfileName::parse(ok).unwrap().as_str(), ok);
        }
    }

    #[test]
    fn rejects_bad_names() {
        let too_long = "a".repeat(ProfileName::MAX_LEN + 1);
        for bad in [
            "",
            "-lead",
            "Upper",
            "has space",
            "under_score",
            "dot.name",
            too_long.as_str(),
        ] {
            assert!(
                ProfileName::parse(bad).is_err(),
                "{bad:?} should be rejected"
            );
        }
    }
}
