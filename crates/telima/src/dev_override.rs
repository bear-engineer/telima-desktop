//! Debug-only override of the Colima version the startup check sees (`TELIMA_COLIMA_VERSION`).

use std::sync::Arc;

use telima_app::runtime::{BoxFuture, DynVm};
use telima_core::ports::{PortResult, ProgressSink};
use telima_core::profile::{DeleteOptions, Profile, ProfileName, ProfileSpec, RuntimeInfo};
use telima_core::version::ColimaVersion;

/// Version to report instead of the real one: only in debug builds (`debug`), only when it parses.
pub fn version_override(debug: bool, raw: Option<&str>) -> Option<ColimaVersion> {
    if !debug {
        return None;
    }
    ColimaVersion::parse(raw?).ok()
}

/// Wraps the real VM manager and replaces only `colima_version`; everything else passes through.
pub struct VersionOverride {
    inner: Arc<dyn DynVm>,
    version: ColimaVersion,
}

impl VersionOverride {
    /// Wrap `vm` when `TELIMA_COLIMA_VERSION` is set and valid in a debug build; else return it as is.
    pub fn wrap(vm: Arc<dyn DynVm>, debug: bool, raw: Option<&str>) -> Arc<dyn DynVm> {
        match version_override(debug, raw) {
            Some(version) => {
                eprintln!("telima: TELIMA_COLIMA_VERSION={version} (debug override)");
                Arc::new(Self { inner: vm, version })
            }
            None => vm,
        }
    }
}

impl DynVm for VersionOverride {
    fn list(&self) -> BoxFuture<'_, PortResult<Vec<Profile>>> {
        self.inner.list()
    }
    fn spec<'a>(&'a self, name: &'a ProfileName) -> BoxFuture<'a, PortResult<ProfileSpec>> {
        self.inner.spec(name)
    }
    fn create<'a>(
        &'a self,
        spec: &'a ProfileSpec,
        progress: ProgressSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>> {
        self.inner.create(spec, progress)
    }
    fn start<'a>(
        &'a self,
        name: &'a ProfileName,
        progress: ProgressSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>> {
        self.inner.start(name, progress)
    }
    fn stop<'a>(
        &'a self,
        name: &'a ProfileName,
        progress: ProgressSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>> {
        self.inner.stop(name, progress)
    }
    fn restart<'a>(
        &'a self,
        name: &'a ProfileName,
        progress: ProgressSink<'a>,
    ) -> BoxFuture<'a, PortResult<()>> {
        self.inner.restart(name, progress)
    }
    fn update<'a>(&'a self, spec: &'a ProfileSpec) -> BoxFuture<'a, PortResult<()>> {
        self.inner.update(spec)
    }
    fn delete<'a>(
        &'a self,
        name: &'a ProfileName,
        options: DeleteOptions,
    ) -> BoxFuture<'a, PortResult<()>> {
        self.inner.delete(name, options)
    }
    fn colima_version(&self) -> BoxFuture<'_, PortResult<ColimaVersion>> {
        let version = self.version;
        Box::pin(async move { Ok(version) })
    }
    fn runtime_info<'a>(&'a self, name: &'a ProfileName) -> BoxFuture<'a, PortResult<RuntimeInfo>> {
        self.inner.runtime_info(name)
    }
    fn prune_cache(&self) -> BoxFuture<'_, PortResult<()>> {
        self.inner.prune_cache()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_build_parses_the_override() {
        assert_eq!(
            version_override(true, Some("0.9.1")),
            Some(ColimaVersion::new(0, 9, 1))
        );
        assert_eq!(
            version_override(true, Some(" v1.2.3 ")),
            Some(ColimaVersion::new(1, 2, 3))
        );
    }

    #[test]
    fn release_build_ignores_the_override() {
        assert_eq!(version_override(false, Some("0.9.1")), None);
    }

    #[test]
    fn missing_or_bad_value_is_no_override() {
        assert_eq!(version_override(true, None), None);
        assert_eq!(version_override(true, Some("abc")), None);
        assert_eq!(version_override(true, Some("1.2")), None);
    }
}
