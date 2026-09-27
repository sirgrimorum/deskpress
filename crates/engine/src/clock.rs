//! Dates and times as a pack writes them: `YYYY-MM-DD`, `HH:MM` and `YYYY-MM-DDTHH:MM`, local to the
//! pack's timezone. In that shape text order is time order, so they stay text, and only the next
//! day and the next minute need arithmetic. The host passes the clock already in the pack's zone.

use crate::validate::patterns::days_in_month;

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
    let (date, time) = stamp.split_at(11);
    let minutes = number(&time[..2]) * 60 + number(&time[3..]) + 1;
    if minutes == 24 * 60 {
        return format!("{}T00:00", next_day(&date[..10]));
    }
    format!("{date}{:02}:{:02}", minutes / 60, minutes % 60)
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
    }

    #[test]
    fn a_date_starts_at_its_first_minute() {
        assert_eq!(start("2026-04-11"), "2026-04-11T00:00");
        assert_eq!(start("2026-04-11T17:30"), "2026-04-11T17:30");
    }
}
