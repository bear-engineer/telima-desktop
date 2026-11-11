//! Points in time as plain numbers, so the domain needs no clock or date library.
use std::time::Duration;

/// Milliseconds since the Unix epoch (UTC). Adapters convert engine and CLI time formats to this;
/// views format it. The current time is always passed in, never read here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnixMillis(pub i64);

impl UnixMillis {
    pub const fn from_secs(secs: i64) -> Self {
        Self(secs.saturating_mul(1000))
    }

    /// Whole seconds since the epoch, rounded down.
    pub const fn as_secs(self) -> i64 {
        self.0.div_euclid(1000)
    }

    /// Time from `self` to `now`; zero when `now` is earlier (clock skew between host and VM).
    pub fn elapsed_until(self, now: UnixMillis) -> Duration {
        let millis = now.0.saturating_sub(self.0);
        Duration::from_millis(u64::try_from(millis).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_between_seconds_and_millis() {
        assert_eq!(UnixMillis::from_secs(12), UnixMillis(12_000));
        assert_eq!(UnixMillis(12_999).as_secs(), 12);
    }

    #[test]
    fn elapsed_is_the_gap_to_now() {
        let started = UnixMillis::from_secs(100);
        assert_eq!(
            started.elapsed_until(UnixMillis::from_secs(160)),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn elapsed_is_zero_when_now_is_earlier() {
        let started = UnixMillis::from_secs(100);
        assert_eq!(
            started.elapsed_until(UnixMillis::from_secs(40)),
            Duration::ZERO
        );
    }

    #[test]
    fn orders_by_time() {
        assert!(UnixMillis(1) < UnixMillis(2));
    }
}
