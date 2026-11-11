//! Single source of truth for Colima profile rules (allowed options per OS, immutable fields).
//! UI locking, validation and adapters all read from here.
use crate::profile::{HostOs, MountType, ProfileSpec, Runtime, VmType};

/// Lowest Colima version this app is tested against.
pub const MIN_COLIMA_VERSION: (u32, u32, u32) = (0, 10, 0);

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
    if errs.is_empty() { Ok(()) } else { Err(errs) }
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

type FieldChanged = fn(&ProfileSpec, &ProfileSpec) -> bool;

/// Fields Colima cannot change after creation. The UI locks exactly these.
pub const IMMUTABLE_FIELDS: &[(&str, FieldChanged)] = &[
    ("name", |a, b| a.name != b.name),
    ("vm_type", |a, b| a.vm_type != b.vm_type),
    ("runtime", |a, b| a.runtime != b.runtime),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::ProfileName;

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
