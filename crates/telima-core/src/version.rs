//! Colima version and the range this app supports. Single source of truth:
//! the startup check and the release-watch workflow both read the constants below.
use std::fmt;

/// A Colima release number (`major.minor.patch`). Ordering is numeric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColimaVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// Oldest Colima release the app works with. Older ones get an "update needed" notice.
pub const MIN_SUPPORTED: ColimaVersion = ColimaVersion::new(0, 10, 3);

/// Newest Colima release the contract tests were run against. Newer ones get a soft warning.
/// The release-watch workflow reads this line; keep it a single `ColimaVersion::new(..)` call.
pub const LAST_TESTED: ColimaVersion = ColimaVersion::new(0, 10, 3);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid colima version: {0:?}")]
pub struct InvalidColimaVersion(pub String);

impl ColimaVersion {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse `0.10.3` or `v0.10.3`. A pre-release or build suffix (`-rc1`, `+abc`) is ignored,
    /// so a release candidate counts as its release.
    pub fn parse(raw: &str) -> Result<Self, InvalidColimaVersion> {
        let invalid = || InvalidColimaVersion(raw.to_owned());
        let text = raw.trim();
        let text = text.strip_prefix('v').unwrap_or(text);
        let core = text.split(['-', '+']).next().unwrap_or(text);
        let mut parts = core.split('.').map(|part| {
            let digits = !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
            digits.then(|| part.parse::<u32>().ok()).flatten()
        });
        let mut next = || parts.next().flatten().ok_or_else(invalid);
        let version = Self::new(next()?, next()?, next()?);
        if parts.next().is_some() {
            return Err(invalid());
        }
        Ok(version)
    }
}

impl fmt::Display for ColimaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Where a Colima version sits relative to the range this app is tested with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionSupport {
    /// Within `MIN_SUPPORTED..=LAST_TESTED`.
    Supported,
    /// Below `MIN_SUPPORTED`: the user must update Colima.
    TooOld,
    /// Above `LAST_TESTED`: probably fine, not verified yet.
    Untested,
}

pub fn version_support(version: ColimaVersion) -> VersionSupport {
    if version < MIN_SUPPORTED {
        VersionSupport::TooOld
    } else if version > LAST_TESTED {
        VersionSupport::Untested
    } else {
        VersionSupport::Supported
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_v_prefixed_versions() {
        assert_eq!(
            ColimaVersion::parse("0.10.3"),
            Ok(ColimaVersion::new(0, 10, 3))
        );
        assert_eq!(
            ColimaVersion::parse("v0.10.3"),
            Ok(ColimaVersion::new(0, 10, 3))
        );
        assert_eq!(
            ColimaVersion::parse(" 1.2.0\n"),
            Ok(ColimaVersion::new(1, 2, 0))
        );
    }

    #[test]
    fn ignores_pre_release_and_build_suffixes() {
        assert_eq!(
            ColimaVersion::parse("0.11.0-rc1"),
            Ok(ColimaVersion::new(0, 11, 0))
        );
        assert_eq!(
            ColimaVersion::parse("0.11.0+abc"),
            Ok(ColimaVersion::new(0, 11, 0))
        );
    }

    #[test]
    fn rejects_text_that_is_not_a_version() {
        for bad in ["", "0.10", "0.10.3.1", "a.b.c", "HEAD-1234", "0.-1.3", "v"] {
            assert_eq!(
                ColimaVersion::parse(bad),
                Err(InvalidColimaVersion(bad.to_owned())),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn displays_as_dotted_numbers() {
        assert_eq!(ColimaVersion::new(0, 10, 3).to_string(), "0.10.3");
    }

    #[test]
    fn orders_numerically_not_as_text() {
        assert!(ColimaVersion::new(0, 9, 9) < ColimaVersion::new(0, 10, 0));
        assert!(ColimaVersion::new(0, 10, 3) < ColimaVersion::new(1, 0, 0));
    }

    #[test]
    fn supported_range_is_well_formed() {
        assert!(MIN_SUPPORTED <= LAST_TESTED);
    }

    #[test]
    fn both_ends_of_the_range_are_supported() {
        assert_eq!(version_support(MIN_SUPPORTED), VersionSupport::Supported);
        assert_eq!(version_support(LAST_TESTED), VersionSupport::Supported);
    }

    #[test]
    fn below_the_minimum_is_too_old() {
        assert_eq!(
            version_support(ColimaVersion::new(0, 0, 0)),
            VersionSupport::TooOld
        );
        assert_eq!(
            version_support(ColimaVersion::new(0, 10, 2)),
            VersionSupport::TooOld
        );
    }

    #[test]
    fn above_the_last_tested_is_untested() {
        let next_patch =
            ColimaVersion::new(LAST_TESTED.major, LAST_TESTED.minor, LAST_TESTED.patch + 1);
        let next_major = ColimaVersion::new(LAST_TESTED.major + 1, 0, 0);
        assert_eq!(version_support(next_patch), VersionSupport::Untested);
        assert_eq!(version_support(next_major), VersionSupport::Untested);
    }
}
