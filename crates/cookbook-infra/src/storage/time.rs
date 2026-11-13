use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;

pub fn to_millis(time: SystemTime) -> anyhow::Result<i64> {
    let elapsed = time
        .duration_since(UNIX_EPOCH)
        .context("timestamp is before the Unix epoch")?;

    i64::try_from(elapsed.as_millis()).context("timestamp is too large for SQLite")
}

pub fn from_millis(millis: i64) -> anyhow::Result<SystemTime> {
    let millis = u64::try_from(millis).context("timestamp is before the Unix epoch")?;

    Ok(UNIX_EPOCH + Duration::from_millis(millis))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_millisecond_timestamps() {
        let time = UNIX_EPOCH + Duration::from_millis(1_700_000_000_123);

        assert_eq!(from_millis(to_millis(time).unwrap()).unwrap(), time);
    }

    #[test]
    fn rejects_timestamps_before_the_epoch() {
        assert!(to_millis(UNIX_EPOCH - Duration::from_secs(1)).is_err());
        assert!(from_millis(-1).is_err());
    }
}
