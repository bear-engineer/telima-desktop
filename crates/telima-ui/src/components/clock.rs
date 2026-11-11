//! Wall clock for views (core and app have no clock): uptime, relative times.
use telima_core::time::UnixMillis;

/// Now, in milliseconds since the Unix epoch (0 if the clock is before the epoch).
pub fn now() -> UnixMillis {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    UnixMillis(ms)
}
