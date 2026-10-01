//! Dates and times as a pack writes them: `YYYY-MM-DD`, `HH:MM` and `YYYY-MM-DDTHH:MM`, local to the
//! pack's timezone. In that shape text order is time order, so they stay text, and only moving a
//! stamp needs arithmetic. The host passes the clock already in the pack's zone, and in any other
//! zone the pack names, so the engine carries no timezone database.

use crate::validate::patterns::{days_in_month, is_real_date, is_stamp};

/// A `YYYY-MM-DDTHH:MM` on a day the calendar has.
pub fn real(stamp: &str) -> bool {
    is_stamp(stamp) && is_real_date(&stamp[..10])
}

fn number(s: &str) -> u32 {
    s.parse().unwrap_or_default()
}

/// The day after a real `YYYY-MM-DD`.
pub fn next_day(date: &str) -> String {
    let (y, m, d) = (number(&date[..4]), number(&date[5..7]), number(&date[8..10]));
    let (y, m, d) = if d < days_in_month(y, m) {
        (y, m, d + 1)
    } else if m < 12 {
        (y, m + 1, 1)
    } else {
        (y + 1, 1, 1)
    };
    format!("{y:04}-{m:02}-{d:02}")
}

/// The minute after a `YYYY-MM-DDTHH:MM`, which may be the next day.
pub fn next_minute(stamp: &str) -> String {
    later(stamp, 1)
}

/// A `YYYY-MM-DDTHH:MM` some minutes on.
pub fn later(stamp: &str, by: u32) -> String {
    shift(stamp, i64::from(by))
}

/// Minutes from 1970-01-01T00:00 to a `YYYY-MM-DDTHH:MM`, by the proleptic Gregorian calendar.
pub fn minutes(stamp: &str) -> i64 {
    let (y, m, d) = (number(&stamp[..4]), number(&stamp[5..7]), number(&stamp[8..10]));
    let (y, m) = (i64::from(if m <= 2 { y - 1 } else { y }), i64::from(m));
    let era = y.div_euclid(400);
    let year = y - era * 400;
    let day_of_year = (153 * ((m + 9) % 12) + 2) / 5 + i64::from(d) - 1;
    let day_of_era = year * 365 + year / 4 - year / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    days * 1440 + i64::from(number(&stamp[11..13]) * 60 + number(&stamp[14..16]))
}

/// A `YYYY-MM-DDTHH:MM` moved by some minutes, either way.
pub fn shift(stamp: &str, by: i64) -> String {
    let total = minutes(stamp) + by;
    let (days, minute) = (total.div_euclid(1440), total.rem_euclid(1440));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year + year / 4 - year / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let d = day_of_year - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = year + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}", minute / 60, minute % 60)
}

/// A date alone as the first minute of that day; a stamp as it is.
pub fn start(at: &str) -> String {
    if at.len() == 10 { format!("{at}T00:00") } else { at.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_day_turns_months_years_and_leap_days() {
        assert_eq!(next_day("2026-04-11"), "2026-04-12");
        assert_eq!(next_day("2026-04-30"), "2026-05-01");
        assert_eq!(next_day("2026-12-31"), "2027-01-01");
        assert_eq!(next_day("2028-02-28"), "2028-02-29");
        assert_eq!(next_day("2026-02-28"), "2026-03-01");
    }

    #[test]
    fn the_next_minute_turns_hours_and_midnight() {
        assert_eq!(next_minute("2026-04-11T18:00"), "2026-04-11T18:01");
        assert_eq!(next_minute("2026-04-11T18:59"), "2026-04-11T19:00");
        assert_eq!(next_minute("2026-12-31T23:59"), "2027-01-01T00:00");
        assert_eq!(later("2026-04-11T23:00", 60 * 49), "2026-04-14T00:00");
    }

    #[test]
    fn a_stamp_shifts_either_way_across_days_months_and_years() {
        assert_eq!(minutes("1970-01-01T00:00"), 0);
        assert_eq!(minutes("1969-12-31T23:59"), -1);
        assert_eq!(minutes("2026-10-03T02:00") - minutes("2026-10-02T19:00"), 7 * 60);
        assert_eq!(shift("2026-10-02T19:00", 7 * 60), "2026-10-03T02:00");
        assert_eq!(shift("2026-03-01T01:00", -120), "2026-02-28T23:00");
        assert_eq!(shift("2028-03-01T00:00", -1), "2028-02-29T23:59");
        assert_eq!(shift("2027-01-01T00:30", -60), "2026-12-31T23:30");
        assert_eq!(shift("2000-02-29T12:00", 0), "2000-02-29T12:00");
    }

    #[test]
    fn a_date_starts_at_its_first_minute() {
        assert_eq!(start("2026-04-11"), "2026-04-11T00:00");
        assert_eq!(start("2026-04-11T17:30"), "2026-04-11T17:30");
    }
}
