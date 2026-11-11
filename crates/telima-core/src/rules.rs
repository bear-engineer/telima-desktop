//! Single source of truth for Colima profile rules (allowed options per OS, immutable fields).
//! UI locking, validation and adapters all read from here.
//! The supported Colima version range lives in `crate::version`.
use crate::profile::{HostOs, MountType, ProfileSpec, Runtime, VmType};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleViolation {
    #[error("{0:?} VM is not available on this OS")]
    VmTypeUnavailable(VmType),
    #[error("{mount:?} mounts are not available with {vm:?}")]
    MountTypeUnavailable { vm: VmType, mount: MountType },
    #[error("Rosetta needs the vz VM type on macOS")]
    RosettaUnavailable,
    #[error("only the docker runtime is supported")]
    RuntimeUnsupported(Runtime),
    #[error("{field} must be at least {min}")]
    TooSmall { field: &'static str, min: u32 },
    #[error("{0} cannot be changed after the profile is created")]
    Immutable(&'static str),
    #[error("disk can only grow (from {from} GiB to {to} GiB requested)")]
    DiskShrink { from: u32, to: u32 },
    #[error("mount path {0:?} must be absolute")]
    MountPathNotAbsolute(String),
    #[error("mount path {0:?} is listed more than once")]
    DuplicateMount(String),
    #[error("Kubernetes needs the docker runtime (got {0:?})")]
    KubernetesNeedsDocker(Runtime),
}

/// VM types offered on `os`.
pub fn vm_types(os: HostOs) -> &'static [VmType] {
    match os {
        HostOs::MacOs => &[VmType::Vz, VmType::Qemu, VmType::Krunkit],
        HostOs::Linux => &[VmType::Qemu],
    }
}

/// Mount types offered for `vm`.
pub fn mount_types(vm: VmType) -> &'static [MountType] {
    match vm {
        VmType::Vz | VmType::Krunkit => &[MountType::Virtiofs, MountType::Sshfs, MountType::NineP],
        VmType::Qemu => &[MountType::Sshfs, MountType::NineP],
    }
}

/// Runtimes this app can manage. Colima offers more; the UI shows only these.
pub const SUPPORTED_RUNTIMES: &[Runtime] = &[Runtime::Docker];

/// Smallest accepted value for each resource field.
const MIN_CPUS: u32 = 1;
const MIN_MEMORY_GIB: u32 = 1;
const MIN_DISK_GIB: u32 = 1;

/// Check a spec for a new profile.
pub fn validate_new(spec: &ProfileSpec, os: HostOs) -> Result<(), Vec<RuleViolation>> {
    let mut errs = Vec::new();
    if !vm_types(os).contains(&spec.vm_type) {
        errs.push(RuleViolation::VmTypeUnavailable(spec.vm_type));
    } else if !mount_types(spec.vm_type).contains(&spec.mount_type) {
        errs.push(RuleViolation::MountTypeUnavailable {
            vm: spec.vm_type,
            mount: spec.mount_type,
        });
    }
    if spec.rosetta && !(os == HostOs::MacOs && spec.vm_type == VmType::Vz) {
        errs.push(RuleViolation::RosettaUnavailable);
    }
    if !SUPPORTED_RUNTIMES.contains(&spec.runtime) {
        errs.push(RuleViolation::RuntimeUnsupported(spec.runtime));
    }
    for (field, value, min) in [
        ("cpus", spec.cpus, MIN_CPUS),
        ("memory_gib", spec.memory_gib, MIN_MEMORY_GIB),
        ("disk_gib", spec.disk_gib, MIN_DISK_GIB),
    ] {
        if value < min {
            errs.push(RuleViolation::TooSmall { field, min });
        }
    }
    if spec.kubernetes && spec.runtime != KUBERNETES_RUNTIME {
        errs.push(RuleViolation::KubernetesNeedsDocker(spec.runtime));
    }
    check_mounts(spec, &mut errs);
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

/// The only runtime this app enables Kubernetes with.
pub const KUBERNETES_RUNTIME: Runtime = Runtime::Docker;

/// Shape checks only: each path absolute, no path twice. Whether the directory exists on the
/// host is the adapter's concern.
fn check_mounts(spec: &ProfileSpec, errs: &mut Vec<RuleViolation>) {
    let mut seen: Vec<&str> = Vec::new();
    for mount in &spec.mounts {
        if !mount.path.starts_with('/') {
            errs.push(RuleViolation::MountPathNotAbsolute(mount.path.clone()));
            continue;
        }
        let key = mount_key(&mount.path);
        if seen.contains(&key) {
            let dup = RuleViolation::DuplicateMount(key.to_owned());
            if !errs.contains(&dup) {
                errs.push(dup);
            }
        } else {
            seen.push(key);
        }
    }
}

/// A mount path without trailing slashes (`/a/b/` and `/a/b` are the same mount; `/` stays `/`).
fn mount_key(path: &str) -> &str {
    match path.trim_end_matches('/') {
        "" => "/",
        trimmed => trimmed,
    }
}

/// Check an edit of an existing profile (`current` → `next`).
pub fn validate_update(
    current: &ProfileSpec,
    next: &ProfileSpec,
    os: HostOs,
) -> Result<(), Vec<RuleViolation>> {
    let mut errs = Vec::new();
    for (field, changed) in IMMUTABLE_FIELDS {
        if changed(current, next) {
            errs.push(RuleViolation::Immutable(field));
        }
    }
    if next.disk_gib < current.disk_gib {
        errs.push(RuleViolation::DiskShrink {
            from: current.disk_gib,
            to: next.disk_gib,
        });
    }
    if let Err(more) = validate_new(next, os) {
        for e in more {
            if !errs.contains(&e) {
                errs.push(e);
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

pub type FieldChanged = fn(&ProfileSpec, &ProfileSpec) -> bool;

/// Fields Colima cannot change after creation. The UI locks exactly these.
pub const IMMUTABLE_FIELDS: &[(&str, FieldChanged)] = &[
    ("name", |a, b| a.name != b.name),
    ("vm_type", |a, b| a.vm_type != b.vm_type),
    ("runtime", |a, b| a.runtime != b.runtime),
];

/// Fields that can change, but only take effect when the VM starts again. Editing one of these
/// on a running profile needs a restart; on a stopped profile the next start applies it.
/// Together with `IMMUTABLE_FIELDS` this covers every field of `ProfileSpec`.
pub const RESTART_FIELDS: &[(&str, FieldChanged)] = &[
    ("cpus", |a, b| a.cpus != b.cpus),
    ("memory_gib", |a, b| a.memory_gib != b.memory_gib),
    ("disk_gib", |a, b| a.disk_gib != b.disk_gib),
    ("mount_type", |a, b| a.mount_type != b.mount_type),
    ("rosetta", |a, b| a.rosetta != b.rosetta),
    ("network_address", |a, b| {
        a.network_address != b.network_address
    }),
    ("mounts", |a, b| a.mounts != b.mounts),
    ("kubernetes", |a, b| a.kubernetes != b.kubernetes),
];

/// Names of the changed fields (`current` → `next`) that need a restart to take effect,
/// in `RESTART_FIELDS` order. Empty when nothing restart-bound changed.
pub fn restart_required(current: &ProfileSpec, next: &ProfileSpec) -> Vec<&'static str> {
    RESTART_FIELDS
        .iter()
        .filter(|(_, changed)| changed(current, next))
        .map(|(field, _)| *field)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Mount, ProfileName};

    fn mac_spec() -> ProfileSpec {
        ProfileSpec {
            name: ProfileName::parse("telima-test-a").unwrap(),
            cpus: 2,
            memory_gib: 2,
            disk_gib: 20,
            vm_type: VmType::Vz,
            mount_type: MountType::Virtiofs,
            runtime: Runtime::Docker,
            rosetta: true,
            network_address: false,
            mounts: Vec::new(),
            kubernetes: false,
        }
    }

    fn mount(path: &str, writable: bool) -> Mount {
        Mount {
            path: path.to_owned(),
            writable,
        }
    }

    #[test]
    fn absolute_distinct_mounts_pass() {
        let spec = ProfileSpec {
            mounts: vec![mount("/Users/me/src", true), mount("/tmp/data", false)],
            ..mac_spec()
        };
        assert_eq!(validate_new(&spec, HostOs::MacOs), Ok(()));
    }

    #[test]
    fn mount_path_must_be_absolute() {
        for bad in ["", "src", "./src", "~/src", " /lead-space"] {
            let spec = ProfileSpec {
                mounts: vec![mount(bad, true)],
                ..mac_spec()
            };
            assert_eq!(
                validate_new(&spec, HostOs::MacOs).unwrap_err(),
                vec![RuleViolation::MountPathNotAbsolute(bad.to_owned())],
                "{bad:?}"
            );
        }
    }

    #[test]
    fn duplicate_mounts_are_rejected_once_ignoring_trailing_slash_and_writable() {
        let spec = ProfileSpec {
            mounts: vec![
                mount("/a/b", true),
                mount("/a/b/", false),
                mount("/a/b", false),
                mount("/c", true),
            ],
            ..mac_spec()
        };
        assert_eq!(
            validate_new(&spec, HostOs::MacOs).unwrap_err(),
            vec![RuleViolation::DuplicateMount("/a/b".to_owned())]
        );
    }

    #[test]
    fn root_mount_is_absolute_and_distinct_from_others() {
        let spec = ProfileSpec {
            mounts: vec![mount("/", false), mount("/a", true)],
            ..mac_spec()
        };
        assert_eq!(validate_new(&spec, HostOs::MacOs), Ok(()));
    }

    #[test]
    fn kubernetes_is_allowed_with_docker() {
        let spec = ProfileSpec {
            kubernetes: true,
            ..mac_spec()
        };
        assert_eq!(validate_new(&spec, HostOs::MacOs), Ok(()));
    }

    #[test]
    fn kubernetes_is_rejected_with_other_runtimes() {
        let spec = ProfileSpec {
            kubernetes: true,
            runtime: Runtime::Incus,
            ..mac_spec()
        };
        let errs = validate_new(&spec, HostOs::MacOs).unwrap_err();
        assert!(errs.contains(&RuleViolation::KubernetesNeedsDocker(Runtime::Incus)));
    }

    #[test]
    fn update_may_change_mounts_and_kubernetes() {
        let next = ProfileSpec {
            mounts: vec![mount("/a", true)],
            kubernetes: true,
            ..mac_spec()
        };
        assert_eq!(validate_update(&mac_spec(), &next, HostOs::MacOs), Ok(()));
    }

    #[test]
    fn update_checks_the_new_mounts() {
        let next = ProfileSpec {
            mounts: vec![mount("rel", true)],
            ..mac_spec()
        };
        assert_eq!(
            validate_update(&mac_spec(), &next, HostOs::MacOs).unwrap_err(),
            vec![RuleViolation::MountPathNotAbsolute("rel".to_owned())]
        );
    }

    #[test]
    fn unchanged_spec_needs_no_restart() {
        assert!(restart_required(&mac_spec(), &mac_spec()).is_empty());
    }

    #[test]
    fn mounts_and_kubernetes_changes_need_a_restart() {
        let next = ProfileSpec {
            mounts: vec![mount("/a", true)],
            kubernetes: true,
            ..mac_spec()
        };
        assert_eq!(
            restart_required(&mac_spec(), &next),
            vec!["mounts", "kubernetes"]
        );
    }

    #[test]
    fn writable_flag_change_alone_needs_a_restart() {
        let cur = ProfileSpec {
            mounts: vec![mount("/a", true)],
            ..mac_spec()
        };
        let next = ProfileSpec {
            mounts: vec![mount("/a", false)],
            ..mac_spec()
        };
        assert_eq!(restart_required(&cur, &next), vec!["mounts"]);
    }

    #[test]
    fn resource_and_option_changes_need_a_restart() {
        let next = ProfileSpec {
            cpus: 4,
            memory_gib: 8,
            disk_gib: 40,
            mount_type: MountType::Sshfs,
            rosetta: false,
            network_address: true,
            ..mac_spec()
        };
        assert_eq!(
            restart_required(&mac_spec(), &next),
            vec![
                "cpus",
                "memory_gib",
                "disk_gib",
                "mount_type",
                "rosetta",
                "network_address"
            ]
        );
    }

    #[test]
    fn every_restart_field_is_mutable() {
        for (field, _) in RESTART_FIELDS {
            assert!(
                IMMUTABLE_FIELDS.iter().all(|(name, _)| name != field),
                "{field} is listed as both immutable and restart-applied"
            );
        }
    }

    fn linux_spec() -> ProfileSpec {
        ProfileSpec {
            vm_type: VmType::Qemu,
            mount_type: MountType::Sshfs,
            rosetta: false,
            ..mac_spec()
        }
    }

    #[test]
    fn linux_offers_qemu_only() {
        assert_eq!(vm_types(HostOs::Linux), &[VmType::Qemu]);
        assert!(vm_types(HostOs::MacOs).contains(&VmType::Vz));
    }

    #[test]
    fn virtiofs_needs_vz() {
        assert!(mount_types(VmType::Vz).contains(&MountType::Virtiofs));
        assert!(!mount_types(VmType::Qemu).contains(&MountType::Virtiofs));
    }

    #[test]
    fn valid_specs_pass() {
        assert_eq!(validate_new(&mac_spec(), HostOs::MacOs), Ok(()));
        assert_eq!(validate_new(&linux_spec(), HostOs::Linux), Ok(()));
    }

    #[test]
    fn vz_rejected_on_linux() {
        let errs = validate_new(&mac_spec(), HostOs::Linux).unwrap_err();
        assert!(errs.contains(&RuleViolation::VmTypeUnavailable(VmType::Vz)));
        assert!(errs.contains(&RuleViolation::RosettaUnavailable));
    }

    #[test]
    fn virtiofs_with_qemu_rejected() {
        let spec = ProfileSpec {
            vm_type: VmType::Qemu,
            rosetta: false,
            ..mac_spec()
        };
        let errs = validate_new(&spec, HostOs::MacOs).unwrap_err();
        assert_eq!(
            errs,
            vec![RuleViolation::MountTypeUnavailable {
                vm: VmType::Qemu,
                mount: MountType::Virtiofs
            }]
        );
    }

    #[test]
    fn only_docker_runtime_supported() {
        let spec = ProfileSpec {
            runtime: Runtime::Containerd,
            ..mac_spec()
        };
        assert_eq!(
            validate_new(&spec, HostOs::MacOs).unwrap_err(),
            vec![RuleViolation::RuntimeUnsupported(Runtime::Containerd)]
        );
    }

    #[test]
    fn minimum_resources() {
        let spec = ProfileSpec {
            cpus: 0,
            memory_gib: 0,
            disk_gib: 0,
            ..mac_spec()
        };
        let errs = validate_new(&spec, HostOs::MacOs).unwrap_err();
        assert!(errs.contains(&RuleViolation::TooSmall {
            field: "cpus",
            min: 1
        }));
        assert!(errs.contains(&RuleViolation::TooSmall {
            field: "memory_gib",
            min: 1
        }));
        assert!(errs.contains(&RuleViolation::TooSmall {
            field: "disk_gib",
            min: 1
        }));
    }

    #[test]
    fn update_may_change_cpu_and_memory_and_grow_disk() {
        let next = ProfileSpec {
            cpus: 4,
            memory_gib: 8,
            disk_gib: 40,
            ..mac_spec()
        };
        assert_eq!(validate_update(&mac_spec(), &next, HostOs::MacOs), Ok(()));
    }

    #[test]
    fn update_rejects_immutable_changes_and_disk_shrink() {
        let next = ProfileSpec {
            vm_type: VmType::Qemu,
            mount_type: MountType::Sshfs,
            rosetta: false,
            runtime: Runtime::Docker,
            disk_gib: 10,
            ..mac_spec()
        };
        let errs = validate_update(&mac_spec(), &next, HostOs::MacOs).unwrap_err();
        assert!(errs.contains(&RuleViolation::Immutable("vm_type")));
        assert!(errs.contains(&RuleViolation::DiskShrink { from: 20, to: 10 }));
    }

    #[test]
    fn update_rejects_rename() {
        let next = ProfileSpec {
            name: ProfileName::parse("other").unwrap(),
            ..mac_spec()
        };
        assert!(
            validate_update(&mac_spec(), &next, HostOs::MacOs)
                .unwrap_err()
                .contains(&RuleViolation::Immutable("name"))
        );
    }
}
