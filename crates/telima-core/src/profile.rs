//! Colima profile entities and value objects.
use std::fmt;

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
        let _ = raw;
        todo!()
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
