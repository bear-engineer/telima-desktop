//! Points in time as plain numbers, so the domain needs no clock or date library.

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
