//! Profile use cases (D1-D4): list, create, start/stop/restart, update, delete.
use std::collections::HashSet;
use std::sync::Mutex;

use telima_core::ports::{ProgressSink, VmManager};
use telima_core::profile::{HostOs, Profile, ProfileName, ProfileSpec};
use telima_core::rules;

use crate::AppError;

/// Runs profile use cases against a [`VmManager`]. One operation per profile at a time.
pub struct ProfileService<V> {
    vm: V,
    os: HostOs,
    busy: Mutex<HashSet<ProfileName>>,
}

/// Which lifecycle command to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    Start,
    Stop,
    Restart,
}

impl<V: VmManager> ProfileService<V> {
    pub fn new(vm: V, os: HostOs) -> Self {
        Self {
            vm,
            os,
            busy: Mutex::new(HashSet::new()),
        }
    }

    pub fn vm(&self) -> &V {
        &self.vm
    }

    /// Profiles sorted by name.
    pub async fn list(&self) -> Result<Vec<Profile>, AppError> {
        let mut profiles = self.vm.list().await?;
        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(profiles)
    }

    /// Validate against the rules, then create. Nothing reaches the VM when invalid.
    pub async fn create(
        &self,
        spec: &ProfileSpec,
        progress: ProgressSink<'_>,
    ) -> Result<(), AppError> {
        rules::validate_new(spec, self.os).map_err(AppError::Invalid)?;
        let _guard = self.claim(&spec.name)?;
        Ok(self.vm.create(spec, progress).await?)
    }

    pub async fn run(
        &self,
        name: &ProfileName,
        cmd: Lifecycle,
        progress: ProgressSink<'_>,
    ) -> Result<(), AppError> {
        let _guard = self.claim(name)?;
        match cmd {
            Lifecycle::Start => self.vm.start(name, progress).await?,
            Lifecycle::Stop => self.vm.stop(name, progress).await?,
            Lifecycle::Restart => self.vm.restart(name, progress).await?,
        }
        Ok(())
    }

    /// Change settings of an existing profile; immutable fields and disk shrink are refused.
    pub async fn update(&self, next: &ProfileSpec) -> Result<(), AppError> {
        let _guard = self.claim(&next.name)?;
        let current = self.vm.spec(&next.name).await?;
        rules::validate_update(&current, next, self.os).map_err(AppError::Invalid)?;
        Ok(self.vm.update(next).await?)
    }

    /// Delete only when `typed` equals the profile name.
    pub async fn delete(&self, name: &ProfileName, typed: &str) -> Result<(), AppError> {
        if typed != name.as_str() {
            return Err(AppError::ConfirmationMismatch);
        }
        let _guard = self.claim(name)?;
        Ok(self.vm.delete(name).await?)
    }

    /// Mark `name` busy until the returned guard drops.
    fn claim(&self, name: &ProfileName) -> Result<BusyGuard<'_>, AppError> {
        let mut busy = self.busy.lock().unwrap();
        if !busy.insert(name.clone()) {
            return Err(AppError::Busy(name.clone()));
        }
        Ok(BusyGuard {
            busy: &self.busy,
            name: name.clone(),
        })
    }

    /// True while an operation runs on `name`.
    pub fn is_busy(&self, name: &ProfileName) -> bool {
        self.busy.lock().unwrap().contains(name)
    }
}

struct BusyGuard<'a> {
    busy: &'a Mutex<HashSet<ProfileName>>,
    name: ProfileName,
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.busy.lock().unwrap().remove(&self.name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeVm;
    use futures::executor::block_on;
    use telima_core::profile::{DeleteOptions, MountType, ProfileStatus, Runtime, RuntimeInfo, VmType};
    use telima_core::version::{ColimaVersion, MIN_SUPPORTED, VersionSupport};
    use telima_core::rules::RuleViolation;

    fn name(s: &str) -> ProfileName {
        ProfileName::parse(s).unwrap()
    }

    fn profile(n: &str, status: ProfileStatus) -> Profile {
        Profile {
            name: name(n),
            status,
            arch: "aarch64".into(),
            cpus: 2,
            memory_bytes: 2 << 30,
            disk_bytes: 20 << 30,
            address: None,
            runtime: Runtime::Docker,
        }
    }

    fn spec(n: &str) -> ProfileSpec {
        ProfileSpec {
            name: name(n),
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

    fn service(vm: FakeVm) -> ProfileService<FakeVm> {
        ProfileService::new(vm, HostOs::MacOs)
    }

    fn sink() -> impl FnMut(telima_core::profile::ProgressEvent) + Send {
        |_| {}
    }

    #[test]
    fn d1_list_is_sorted_by_name() {
        let vm = FakeVm::default();
        *vm.profiles.lock().unwrap() = vec![
            profile("zeta", ProfileStatus::Stopped),
            profile("alpha", ProfileStatus::Running),
        ];
        let names: Vec<_> = block_on(service(vm).list())
            .unwrap()
            .into_iter()
            .map(|p| p.name.to_string())
            .collect();
        assert_eq!(names, ["alpha", "zeta"]);
    }

    #[test]
    fn d2_create_valid_spec_reaches_vm_and_streams_progress() {
        let vm = FakeVm {
            progress_lines: vec!["starting".into(), "done".into()],
            ..Default::default()
        };
        let svc = service(vm);
        let mut seen = Vec::new();
        block_on(svc.create(&spec("telima-test-a"), &mut |e| seen.push(e.message))).unwrap();
        assert_eq!(svc.vm().calls(), ["create telima-test-a"]);
        assert_eq!(seen, ["starting", "done"]);
    }

    #[test]
    fn d2_create_invalid_spec_never_reaches_vm() {
        let svc = ProfileService::new(FakeVm::default(), HostOs::Linux);
        let err = block_on(svc.create(&spec("telima-test-a"), &mut sink())).unwrap_err();
        assert!(
            matches!(err, AppError::Invalid(ref v) if v.contains(&RuleViolation::VmTypeUnavailable(VmType::Vz)))
        );
        assert!(svc.vm().calls().is_empty());
    }

    #[test]
    fn d3_lifecycle_commands_reach_vm_and_release_busy() {
        let svc = service(FakeVm::default());
        let n = name("dev");
        for cmd in [Lifecycle::Start, Lifecycle::Stop, Lifecycle::Restart] {
            block_on(svc.run(&n, cmd, &mut sink())).unwrap();
        }
        assert_eq!(svc.vm().calls(), ["start dev", "stop dev", "restart dev"]);
        assert!(!svc.is_busy(&n));
    }

    #[test]
    fn d3_second_operation_on_busy_profile_is_refused() {
        let svc = service(FakeVm::default());
        let n = name("dev");
        svc.busy.lock().unwrap().insert(n.clone());
        assert_eq!(
            block_on(svc.run(&n, Lifecycle::Start, &mut sink())),
            Err(AppError::Busy(n.clone()))
        );
        assert!(svc.vm().calls().is_empty());
    }

    #[test]
    fn d4_update_allows_memory_change() {
        let vm = FakeVm::default();
        vm.specs.lock().unwrap().push(spec("dev"));
        let svc = service(vm);
        block_on(svc.update(&ProfileSpec {
            memory_gib: 8,
            ..spec("dev")
        }))
        .unwrap();
        assert_eq!(svc.vm().calls(), ["spec dev", "update dev"]);
    }

    #[test]
    fn d4_update_refuses_vm_type_change_and_disk_shrink() {
        let vm = FakeVm::default();
        vm.specs.lock().unwrap().push(spec("dev"));
        let svc = service(vm);
        let next = ProfileSpec {
            vm_type: VmType::Krunkit,
            disk_gib: 10,
            ..spec("dev")
        };
        let err = block_on(svc.update(&next)).unwrap_err();
        assert!(
            matches!(err, AppError::Invalid(ref v) if v.contains(&RuleViolation::Immutable("vm_type")) && v.contains(&RuleViolation::DiskShrink { from: 20, to: 10 }))
        );
        assert_eq!(svc.vm().calls(), ["spec dev"]);
    }

    #[test]
    fn d4_delete_needs_typed_name() {
        let svc = service(FakeVm::default());
        let n = name("dev");
        assert_eq!(
            block_on(svc.delete(&n, "de")),
            Err(AppError::ConfirmationMismatch)
        );
        assert!(svc.vm().calls().is_empty());
        block_on(svc.delete(&n, "dev", DeleteOptions::default())).unwrap();
        assert_eq!(svc.vm().calls(), ["delete dev"]);
    }

    #[test]
    fn l6_delete_passes_the_data_option() {
        let svc = service(FakeVm::default());
        block_on(svc.delete(&name("dev"), "dev", DeleteOptions { data: true })).unwrap();
        assert_eq!(svc.vm().calls(), ["delete dev --data"]);
    }

    #[test]
    fn l6_runtime_info_comes_from_the_vm() {
        let vm = FakeVm {
            info: RuntimeInfo {
                docker_server: Some("29.5.2".into()),
                ..RuntimeInfo::default()
            },
            ..FakeVm::default()
        };
        let svc = service(vm);
        let info = block_on(svc.runtime_info(&name("dev"))).unwrap();
        assert_eq!(info.docker_server.as_deref(), Some("29.5.2"));
        assert_eq!(svc.vm().calls(), ["runtime_info dev"]);
    }

    #[test]
    fn l6_version_check_classifies_the_installed_colima() {
        let check = |v| {
            let vm = FakeVm {
                version: Some(v),
                ..FakeVm::default()
            };
            block_on(service(vm).colima_version_check()).unwrap()
        };
        assert_eq!(check(MIN_SUPPORTED).support, VersionSupport::Supported);
        assert_eq!(
            check(ColimaVersion::new(0, 1, 0)),
            VersionCheck {
                version: ColimaVersion::new(0, 1, 0),
                support: VersionSupport::TooOld
            }
        );
        assert_eq!(
            check(ColimaVersion::new(9, 0, 0)).support,
            VersionSupport::Untested
        );
    }

    #[test]
    fn l6_version_check_reports_missing_colima() {
        let err = block_on(service(FakeVm::default()).colima_version_check()).unwrap_err();
        assert!(matches!(err, AppError::Port(_)));
    }

    #[test]
    fn l6_prune_cache_reaches_the_vm() {
        let svc = service(FakeVm::default());
        block_on(svc.prune_cache()).unwrap();
        assert_eq!(svc.vm().calls(), ["prune_cache"]);
    }
}
