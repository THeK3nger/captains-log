//! Timezone-aware date helpers shared by the CLI, journal, import and export.
//!
//! Every function takes an `Option<Tz>`: `Some` is the configured timezone,
//! `None` means the system local timezone.

use anyhow::{Context, Result};
use chrono::{
    DateTime, Datelike, Days, FixedOffset, Local, NaiveDate, NaiveDateTime, TimeZone, Utc,
};
use chrono_tz::Tz;

/// Interpret a naive local time in `tz` as UTC. Ambiguous times (DST
/// fall-back) resolve to the earlier instant; times that don't exist (DST
/// gap) yield `None`.
pub fn localize(naive: NaiveDateTime, tz: Option<Tz>) -> Option<DateTime<Utc>> {
    match tz {
        Some(tz) => tz
            .from_local_datetime(&naive)
            .earliest()
            .map(|dt| dt.to_utc()),
        None => Local
            .from_local_datetime(&naive)
            .earliest()
            .map(|dt| dt.to_utc()),
    }
}

/// Convert a UTC timestamp to `tz`.
pub fn to_local(utc: &DateTime<Utc>, tz: Option<Tz>) -> DateTime<FixedOffset> {
    match tz {
        Some(tz) => utc.with_timezone(&tz).fixed_offset(),
        None => utc.with_timezone(&Local).fixed_offset(),
    }
}

/// The calendar date of a UTC timestamp in `tz`.
pub fn local_date(utc: &DateTime<Utc>, tz: Option<Tz>) -> NaiveDate {
    to_local(utc, tz).date_naive()
}

/// The current calendar date in `tz`.
pub fn today(tz: Option<Tz>) -> NaiveDate {
    local_date(&Utc::now(), tz)
}

/// First instant of `date` in `tz`, as UTC. If midnight doesn't exist (DST
/// gap), the first valid hour of the day is used.
pub fn start_of_day_utc(date: NaiveDate, tz: Option<Tz>) -> Result<DateTime<Utc>> {
    for hour in 0..24 {
        let naive = date
            .and_hms_opt(hour, 0, 0)
            .expect("hour is always in range");
        if let Some(utc) = localize(naive, tz) {
            return Ok(utc);
        }
    }
    anyhow::bail!("Could not resolve start of day for {date}")
}

/// First instant of the day after `date` in `tz`, as UTC.
pub fn next_day_start_utc(date: NaiveDate, tz: Option<Tz>) -> Result<DateTime<Utc>> {
    let next_day = date
        .checked_add_days(Days::new(1))
        .context("Failed to calculate next day")?;
    start_of_day_utc(next_day, tz)
}

/// Number of days in the given month, or `None` if year/month is invalid.
pub fn days_in_month(year: i32, month: u32) -> Option<u32> {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1)?;
    NaiveDate::from_ymd_opt(next_year, next_month, 1)?
        .pred_opt()
        .map(|last_day| last_day.day())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_midnight_falls_back_to_first_valid_hour() {
        // Havana skips 00:00-01:00 on the spring-forward day
        let date = NaiveDate::from_ymd_opt(2026, 3, 8).expect("valid date");
        let start = start_of_day_utc(date, Some(chrono_tz::America::Havana)).expect("resolves");
        assert_eq!(start.to_rfc3339(), "2026-03-08T05:00:00+00:00");
    }

    #[test]
    fn days_in_month_handles_leap_years_and_december() {
        assert_eq!(days_in_month(2024, 2), Some(29));
        assert_eq!(days_in_month(2025, 2), Some(28));
        assert_eq!(days_in_month(2025, 12), Some(31));
        assert_eq!(days_in_month(2025, 4), Some(30));
        assert_eq!(days_in_month(2025, 13), None);
        assert_eq!(days_in_month(2025, 0), None);
    }

    #[test]
    fn local_date_follows_the_timezone() {
        let utc = DateTime::parse_from_rfc3339("2026-05-01T22:30:00Z")
            .unwrap()
            .to_utc();
        assert_eq!(
            local_date(&utc, Some(chrono_tz::Europe::Rome)),
            NaiveDate::from_ymd_opt(2026, 5, 2).unwrap()
        );
    }
}
