//! Debug-only override of the Colima version the startup check sees (`TELIMA_COLIMA_VERSION`).

use telima_core::version::ColimaVersion;

/// Version to report instead of the real one: only in debug builds (`debug`), only when it parses.
pub fn version_override(debug: bool, raw: Option<&str>) -> Option<ColimaVersion> {
    let _ = (debug, raw);
    None
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
