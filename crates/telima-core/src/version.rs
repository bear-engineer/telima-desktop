//! Colima version and the range this app supports. Single source of truth:
//! the startup check and the release-watch workflow both read the constants below.

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
