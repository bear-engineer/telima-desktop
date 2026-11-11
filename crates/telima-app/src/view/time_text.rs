//! Time copy for uptime, container start and log lines.

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use telima_core::profile::RuntimeInfo;
    use telima_core::time::UnixMillis;

    use super::*;

    const fn mins(m: u64) -> Duration {
        Duration::from_secs(m * 60)
    }

    #[test]
    fn uptime_shows_hours_and_minutes() {
        assert_eq!(uptime_text(mins(3 * 60 + 12)), "가동 3시간 12분");
    }

    #[test]
    fn uptime_drops_empty_minutes_and_scales_up_and_down() {
        assert_eq!(uptime_text(Duration::from_secs(20)), "가동 1분 미만");
        assert_eq!(uptime_text(mins(45)), "가동 45분");
        assert_eq!(uptime_text(mins(3 * 60)), "가동 3시간");
        assert_eq!(uptime_text(mins(26 * 60 + 30)), "가동 1일 2시간");
        assert_eq!(uptime_text(mins(48 * 60 + 10)), "가동 2일");
    }

    #[test]
    fn uptime_label_needs_a_start_time() {
        let now = UnixMillis::from_secs(10_000);
        let info = RuntimeInfo {
            started_at: Some(UnixMillis::from_secs(10_000 - 3 * 3600 - 12 * 60)),
            ..RuntimeInfo::default()
        };
        assert_eq!(uptime_label(&info, now).as_deref(), Some("가동 3시간 12분"));
        assert_eq!(uptime_label(&RuntimeInfo::default(), now), None);
    }

    #[test]
    fn relative_time_is_coarse_and_clamps_clock_skew() {
        let now = UnixMillis::from_secs(1_000_000);
        let ago = |s: i64| relative_time_text(UnixMillis::from_secs(1_000_000 - s), now);
        assert_eq!(ago(10), "방금 전");
        assert_eq!(ago(5 * 60), "5분 전");
        assert_eq!(ago(3 * 3600 + 20 * 60), "3시간 전");
        assert_eq!(ago(2 * 86_400 + 5), "2일 전");
        assert_eq!(
            relative_time_text(UnixMillis::from_secs(1_000_100), now),
            "방금 전"
        );
    }

    #[test]
    fn log_time_is_wall_clock_in_the_given_offset() {
        // 2026-11-11 05:03:22 UTC
        let ts = UnixMillis(1_794_373_402_000);
        assert_eq!(log_time_text(Some(ts), 0), "05:03:22");
        assert_eq!(log_time_text(Some(ts), 9 * 3600), "14:03:22");
        assert_eq!(log_time_text(Some(ts), -6 * 3600), "23:03:22");
        assert_eq!(log_time_text(None, 0), "");
    }
}
