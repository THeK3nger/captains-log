use std::sync::LazyLock;

use chrono::{DateTime, Duration, Utc};
/*
 * Stardate module. Computes the current stardate based on the current date.
 *
 * The forumula is a complete fabrication on my part. But I thought it would be fun.
 *
 * The stardate is calculated as follows:
 * - The epoch is set to September 8, 1966 (the premiere date of the original Star Trek series).
 * - Each day since the epoch is counted as 1 stardate unit.
 * - Each second within the day adds a fractional component to the stardate.
 */

static EPOCH: LazyLock<DateTime<Utc>> = LazyLock::new(|| {
    DateTime::parse_from_rfc3339("1966-09-08T00:00:00Z")
        .expect("Invalid epoch date")
        .with_timezone(&Utc)
});

///How many seconds in a day?
const SECONDS_IN_A_DAY: i64 = 86400;

pub trait Stardate {
    fn to_stardate(&self) -> f64;
    fn from_stardate(sd: f64) -> DateTime<Utc>;
}

impl Stardate for DateTime<Utc> {
    fn to_stardate(&self) -> f64 {
        let duration = *self - *EPOCH;
        let days = duration.num_days();
        let seconds = duration.num_seconds() - days * SECONDS_IN_A_DAY;
        (days as f64) + ((seconds as f64) / (SECONDS_IN_A_DAY as f64))
    }

    fn from_stardate(sd: f64) -> DateTime<Utc> {
        let total_days = sd.floor() as i64;
        let fractional_day = sd - (total_days as f64);
        let total_seconds = (fractional_day * 86400.0).round() as i64;

        *EPOCH + Duration::days(total_days) + Duration::seconds(total_seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike};

    #[test]
    fn test_to_stardate() {
        let dt = Utc.ymd(2025, 9, 15).and_hms(15, 30, 0);
        let sd = dt.to_stardate();
        println!("{}", sd);
        assert!((sd - 21557.645883).abs() < 0.0001);
    }

    #[test]
    fn test_from_stardate() {
        let sd = 21557.645883;
        let dt = DateTime::<Utc>::from_stardate(sd);
        // Check that the date is approximately correct, let's ignore seconds for simplicity
        assert_eq!(dt.date(), Utc.ymd(2025, 9, 15));
        assert_eq!(dt.time().hour(), 15);
        assert_eq!(dt.time().minute(), 30);
    }

    #[test]
    fn test_round_trip() {
        let original_dt = Utc.ymd(2024, 6, 1).and_hms(12, 0, 0);
        let sd = original_dt.to_stardate();
        let converted_dt = DateTime::<Utc>::from_stardate(sd);
        assert_eq!(original_dt, converted_dt);
    }
}
