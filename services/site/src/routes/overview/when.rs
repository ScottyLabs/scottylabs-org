//! Dates and times in words for "Needs attention", in Pittsburgh time like the rest of the portal (src/lib/portal/dates.ts).

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Utc, Weekday};

/// The wall clock in Pittsburgh. US Eastern time is UTC-4 from 2 AM on the second Sunday in March to 2 AM on the first Sunday in November, and UTC-5 otherwise.
pub fn eastern(at: DateTime<Utc>) -> NaiveDateTime {
    let year = at.year();
    // 2 AM standard time is 07:00 UTC; 2 AM daylight time is 06:00 UTC.
    let starts = utc_on(sunday(year, 3, 2), 7);
    let ends = utc_on(sunday(year, 11, 1), 6);
    let daylight =
        matches!((starts, ends), (Some(starts), Some(ends)) if at >= starts && at < ends);
    let offset = if daylight { 4 } else { 5 };
    (at - Duration::hours(offset)).naive_utc()
}

fn sunday(year: i32, month: u32, nth: u8) -> Option<NaiveDate> {
    NaiveDate::from_weekday_of_month_opt(year, month, Weekday::Sun, nth)
}

fn utc_on(day: Option<NaiveDate>, hour: u32) -> Option<DateTime<Utc>> {
    day.and_then(|day| day.and_hms_opt(hour, 0, 0))
        .map(|time| time.and_utc())
}

/// "10:20 AM".
pub fn clock(at: DateTime<Utc>) -> String {
    eastern(at).format("%-I:%M %p").to_string()
}

/// "Nov 3", or "Nov 3, 2025" in another year.
pub fn day(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let local = eastern(at);
    if local.year() == eastern(now).year() {
        local.format("%b %-d").to_string()
    } else {
        local.format("%b %-d, %Y").to_string()
    }
}

/// The time for something earlier today, otherwise the day.
pub fn day_or_clock(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    if eastern(at).date() == eastern(now).date() {
        clock(at)
    } else {
        day(at, now)
    }
}

/// "August", or "August 2025" in another year.
pub fn month(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let local = eastern(at);
    if local.year() == eastern(now).year() {
        local.format("%B").to_string()
    } else {
        local.format("%B %Y").to_string()
    }
}

fn plural(count: i64, unit: &str) -> String {
    if count == 1 {
        format!("1 {unit}")
    } else {
        format!("{count} {unit}s")
    }
}

/// Rounds `value / unit` to the nearest whole number.
fn rounded(value: i64, unit: i64) -> i64 {
    (value + unit / 2) / unit
}

/// "in 2 days", "3 hours ago", "tomorrow", "just now". Same rules as `formatRelative` in dates.ts.
pub fn relative(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    let delta = (at - now).num_seconds();
    let distance = delta.abs();
    let future = delta > 0;
    let phrase = |amount: String| {
        if future {
            format!("in {amount}")
        } else {
            format!("{amount} ago")
        }
    };
    if distance < MINUTE {
        return "just now".into();
    }
    if distance < HOUR {
        return phrase(format!("{} min", rounded(distance, MINUTE)));
    }
    if distance < DAY {
        return phrase(plural(rounded(distance, HOUR).max(1), "hour"));
    }
    let days = rounded(distance, DAY);
    if days == 1 {
        return if future { "tomorrow" } else { "yesterday" }.into();
    }
    if days < 45 {
        return phrase(plural(days, "day"));
    }
    phrase(plural(rounded(days, 30), "month"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text).unwrap().to_utc()
    }

    #[test]
    fn eastern_time_follows_daylight_saving() {
        // Summer: UTC-4.
        assert_eq!(clock(utc("2026-07-01T14:20:00Z")), "10:20 AM");
        // Winter: UTC-5.
        assert_eq!(clock(utc("2026-11-24T15:20:00Z")), "10:20 AM");
        // 2026 changes on March 8 and November 1.
        assert_eq!(clock(utc("2026-03-08T06:59:00Z")), "1:59 AM");
        assert_eq!(clock(utc("2026-03-08T07:00:00Z")), "3:00 AM");
        assert_eq!(clock(utc("2026-11-01T05:59:00Z")), "1:59 AM");
        assert_eq!(clock(utc("2026-11-01T06:00:00Z")), "1:00 AM");
    }

    #[test]
    fn days_and_months_read_naturally() {
        let now = utc("2026-11-24T20:00:00Z");
        assert_eq!(day(utc("2026-11-03T15:00:00Z"), now), "Nov 3");
        assert_eq!(day(utc("2025-11-03T15:00:00Z"), now), "Nov 3, 2025");
        // 2 AM UTC on Nov 4 is still Nov 3 in Pittsburgh.
        assert_eq!(day(utc("2026-11-04T02:00:00Z"), now), "Nov 3");
        assert_eq!(day_or_clock(utc("2026-11-24T15:20:00Z"), now), "10:20 AM");
        assert_eq!(day_or_clock(utc("2026-11-20T15:20:00Z"), now), "Nov 20");
        assert_eq!(month(utc("2026-08-10T15:00:00Z"), now), "August");
        assert_eq!(month(utc("2025-08-10T15:00:00Z"), now), "August 2025");
    }

    #[test]
    fn relative_times_match_the_portal() {
        let now = utc("2026-11-24T20:00:00Z");
        let at = |minutes: i64| now + Duration::minutes(minutes);
        assert_eq!(relative(at(0), now), "just now");
        assert_eq!(relative(at(12), now), "in 12 min");
        assert_eq!(relative(at(-120), now), "2 hours ago");
        assert_eq!(relative(at(60), now), "in 1 hour");
        assert_eq!(relative(at(24 * 60), now), "tomorrow");
        assert_eq!(relative(at(-24 * 60), now), "yesterday");
        assert_eq!(relative(at(2 * 24 * 60 + 60), now), "in 2 days");
        assert_eq!(relative(at(-120 * 24 * 60), now), "4 months ago");
    }
}
