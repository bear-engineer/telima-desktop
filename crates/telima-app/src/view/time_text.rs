//! Time copy for uptime, container start and log lines.

use std::time::Duration;

use telima_core::profile::RuntimeInfo;
use telima_core::time::UnixMillis;

const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

/// `가동 3시간 12분`. Two units at most; an empty second unit is left out.
pub fn uptime_text(uptime: Duration) -> String {
    format!("가동 {}", uptime_value(uptime))
}

/// `3시간 12분` without the 가동 prefix (the inspector row is already labelled 가동 시간).
pub fn uptime_value(uptime: Duration) -> String {
    let secs = uptime.as_secs();
    if secs < MINUTE {
        return "1분 미만".to_owned();
    }
    let (days, hours, minutes) = (secs / DAY, secs % DAY / HOUR, secs % HOUR / MINUTE);
    if days > 0 {
        pair(days, "일", hours, "시간")
    } else if hours > 0 {
        pair(hours, "시간", minutes, "분")
    } else {
        format!("{minutes}분")
    }
}

fn pair(major: u64, major_unit: &str, minor: u64, minor_unit: &str) -> String {
    if minor == 0 {
        format!("{major}{major_unit}")
    } else {
        format!("{major}{major_unit} {minor}{minor_unit}")
    }
}

/// Uptime copy of a profile, `None` when the start time is unknown.
pub fn uptime_label(info: &RuntimeInfo, now: UnixMillis) -> Option<String> {
    info.uptime(now).map(uptime_text)
}

/// [`uptime_value`] of a profile, `None` when the start time is unknown.
pub fn uptime_value_label(info: &RuntimeInfo, now: UnixMillis) -> Option<String> {
    info.uptime(now).map(uptime_value)
}

/// `5분 전`, `3시간 전`, `2일 전`. A start time in the future (clock skew) reads as just now.
pub fn relative_time_text(then: UnixMillis, now: UnixMillis) -> String {
    let secs = then.elapsed_until(now).as_secs();
    if secs < MINUTE {
        "방금 전".to_owned()
    } else if secs < HOUR {
        format!("{}분 전", secs / MINUTE)
    } else if secs < DAY {
        format!("{}시간 전", secs / HOUR)
    } else {
        format!("{}일 전", secs / DAY)
    }
}

/// `14:03:22` in the viewer's UTC offset (seconds). Empty when the line has no timestamp.
pub fn log_time_text(timestamp: Option<UnixMillis>, utc_offset_secs: i32) -> String {
    let Some(ts) = timestamp else {
        return String::new();
    };
    let day_secs = (ts.as_secs() + i64::from(utc_offset_secs)).rem_euclid(DAY as i64);
    format!(
        "{:02}:{:02}:{:02}",
        day_secs / HOUR as i64,
        day_secs % HOUR as i64 / MINUTE as i64,
        day_secs % MINUTE as i64
    )
}

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
    fn uptime_value_has_no_prefix_for_the_inspector_row() {
        // F6: the 가동 시간 row must not read "가동 가동 …".
        assert_eq!(uptime_value(mins(3 * 60 + 12)), "3시간 12분");
        assert_eq!(uptime_value(Duration::from_secs(30)), "1분 미만");
        assert_eq!(uptime_text(mins(3 * 60 + 12)), "가동 3시간 12분");
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
