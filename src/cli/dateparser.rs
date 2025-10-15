use chrono::{Datelike, Duration, Local, NaiveDate};

/// Parse relative date strings into NaiveDate
///
/// A relative date string can be:
/// - "today", "yesterday", "tomorrow"
/// - "last week", "last month", "last year"
/// - "next week", "next month", "next year"
/// - "X days ago", "X days from now"
/// - "X weeks ago", "X weeks from now"
/// - "this week" -> It is interpreted as the nearest Monday (start of the week)
pub fn parse_relative_date(input: &str) -> Result<NaiveDate, String> {
    let input = input.trim().to_lowercase();
    let today = Local::now().date_naive();

    match input.as_str() {
        // Absolute dates
        "today" => Ok(today),
        "yesterday" => Ok(today - Duration::days(1)),
        "tomorrow" => Ok(today + Duration::days(1)),

        // This week
        "this week" => {
            let weekday = today.weekday().num_days_from_monday() as i64;
            Ok(today - Duration::days(weekday))
        }

        // Last relative dates
        "last week" => Ok(today - Duration::weeks(1)),
        "last month" => {
            let mut year = today.year();
            let mut month = today.month() as i32 - 1;

            if month == 0 {
                month = 12;
                year -= 1;
            }

            let day = today.day().min(days_in_month(year, month as u32));
            NaiveDate::from_ymd_opt(year, month as u32, day).ok_or("Invalid date".to_string())
        }
        "last year" => {
            let year = today.year() - 1;
            let day = if today.month() == 2 && today.day() == 29 {
                28 // Handle leap year edge case
            } else {
                today.day()
            };
            NaiveDate::from_ymd_opt(year, today.month(), day).ok_or("Invalid date".to_string())
        }

        // Next relative dates
        "next week" => Ok(today + Duration::weeks(1)),
        "next month" => {
            let mut year = today.year();
            let mut month = today.month() as i32 + 1;

            if month == 13 {
                month = 1;
                year += 1;
            }

            let day = today.day().min(days_in_month(year, month as u32));
            NaiveDate::from_ymd_opt(year, month as u32, day).ok_or("Invalid date".to_string())
        }
        "next year" => {
            let year = today.year() + 1;
            let day = if today.month() == 2 && today.day() == 29 {
                28 // Handle leap year edge case
            } else {
                today.day()
            };
            NaiveDate::from_ymd_opt(year, today.month(), day).ok_or("Invalid date".to_string())
        }

        // Days ago/from now
        s if s.ends_with("days ago") || s.ends_with("day ago") => {
            let parts: Vec<&str> = s.split_whitespace().collect();
            if let Some(num_str) = parts.first() {
                if let Ok(days) = num_str.parse::<i64>() {
                    return Ok(today - Duration::days(days));
                }
            }
            Err(format!("Could not parse: {}", input))
        }
        s if s.ends_with("days from now") || s.ends_with("day from now") => {
            let parts: Vec<&str> = s.split_whitespace().collect();
            if let Some(num_str) = parts.first() {
                if let Ok(days) = num_str.parse::<i64>() {
                    return Ok(today + Duration::days(days));
                }
            }
            Err(format!("Could not parse: {}", input))
        }

        // Weeks ago/from now
        s if s.ends_with("weeks ago") || s.ends_with("week ago") => {
            let parts: Vec<&str> = s.split_whitespace().collect();
            if let Some(num_str) = parts.first() {
                if let Ok(weeks) = num_str.parse::<i64>() {
                    return Ok(today - Duration::weeks(weeks));
                }
            }
            Err(format!("Could not parse: {}", input))
        }
        s if s.ends_with("weeks from now") || s.ends_with("week from now") => {
            let parts: Vec<&str> = s.split_whitespace().collect();
            if let Some(num_str) = parts.first() {
                if let Ok(weeks) = num_str.parse::<i64>() {
                    return Ok(today + Duration::weeks(weeks));
                }
            }
            Err(format!("Could not parse: {}", input))
        }

        // Maybe it is not a relative date, try parsing as YYYY-MM-DD.
        _ => chrono::NaiveDate::parse_from_str(&input, "%Y-%m-%d")
            .map_err(|_| format!("Could not parse: {}", input)),
    }
}

/// Helper function to get days in a month
fn days_in_month(year: i32, month: u32) -> u32 {
    NaiveDate::from_ymd_opt(
        if month == 12 { year + 1 } else { year },
        if month == 12 { 1 } else { month + 1 },
        1,
    )
    .unwrap()
    .pred_opt()
    .unwrap()
    .day()
}
