//! The NHL keys games by US Eastern local date, so "today" for every
//! schedule, mirror, and cache lookup is the ET calendar date, not UTC.
//! Using UTC skips the late-evening eastern slate for the ~4 hours
//! between midnight UTC and midnight ET.

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::{America::New_York, Tz};

pub const DATE_FORMAT: &str = "%Y-%m-%d";

pub fn hockey_now() -> DateTime<Tz> {
    Utc::now().with_timezone(&New_York)
}

pub fn hockey_today_date() -> NaiveDate {
    hockey_now().date_naive()
}

pub fn hockey_today() -> String {
    hockey_today_date().format(DATE_FORMAT).to_string()
}

/// The calendar day before `date` (YYYY-MM-DD), or `None` if `date`
/// doesn't parse.
pub fn day_before(date: &str) -> Option<String> {
    NaiveDate::parse_from_str(date, DATE_FORMAT)
        .ok()?
        .pred_opt()
        .map(|d| d.format(DATE_FORMAT).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_before_crosses_month_and_year() {
        assert_eq!(day_before("2026-03-01").as_deref(), Some("2026-02-28"));
        assert_eq!(day_before("2026-01-01").as_deref(), Some("2025-12-31"));
    }

    #[test]
    fn day_before_rejects_garbage() {
        assert_eq!(day_before("----------"), None);
        assert_eq!(day_before("2026-13-01"), None);
    }
}
