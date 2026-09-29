//! The shapes a pack's plain values have to take: ids, slugs, dates, times.

fn all(s: &str, ok: impl Fn(u8) -> bool) -> bool {
    s.bytes().all(ok)
}

fn digits(s: &str) -> bool {
    !s.is_empty() && all(s, |b| b.is_ascii_digit())
}

/// `[a-z_][a-z0-9_]*`: what a person, option, point or document is called in the data.
pub(crate) fn is_id(s: &str) -> bool {
    s.bytes().next().is_some_and(|b| b.is_ascii_lowercase() || b == b'_')
        && all(s, |b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Lowercase words of letters and digits joined by single dashes.
pub(crate) fn is_slug(s: &str) -> bool {
    s.split('-').all(|w| !w.is_empty() && all(w, |b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

/// `Area/City`, `Area/Region/City`, or `UTC`.
pub(crate) fn is_timezone(s: &str) -> bool {
    let parts: Vec<&str> = s.split('/').collect();
    let word = |w: &str, extra: &[u8]| {
        !w.is_empty() && all(w, |b| b.is_ascii_alphabetic() || b == b'_' || extra.contains(&b))
    };
    s == "UTC"
        || ((2..=3).contains(&parts.len())
            && word(parts[0], b"")
            && parts[1..].iter().all(|p| word(p, b"+-0123456789")))
}

/// `es`, `pt-BR`, `zh-Hant-TW`.
pub(crate) fn is_language(s: &str) -> bool {
    let mut parts = s.split('-');
    let first = parts.next().unwrap_or_default();
    (2..=3).contains(&first.len())
        && all(first, |b| b.is_ascii_lowercase())
        && parts.all(|p| (2..=8).contains(&p.len()) && all(p, |b| b.is_ascii_alphanumeric()))
}

/// The year, month and day of `YYYY-MM-DD`, without asking whether the day exists.
fn date_parts(s: &str) -> Option<(u32, u32, u32)> {
    let [y, m, d]: [&str; 3] = s.split('-').collect::<Vec<_>>().try_into().ok()?;
    if y.len() != 4 || m.len() != 2 || d.len() != 2 || ![y, m, d].iter().all(|p| digits(p)) {
        return None;
    }
    let n = |p: &str| p.parse().unwrap_or_default();
    Some((n(y), n(m), n(d)))
}

/// `YYYY-MM-DD`, in shape only.
pub(crate) fn is_date(s: &str) -> bool {
    date_parts(s).is_some()
}

/// `YYYY-MM-DD` and a day the calendar has.
pub(crate) fn is_real_date(s: &str) -> bool {
    date_parts(s).is_some_and(|(y, m, d)| real_date(y, m, d))
}

fn real_date(y: u32, m: u32, d: u32) -> bool {
    y > 0 && (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)
}

/// How many days month `m` (1 to 12) of year `y` has.
pub(crate) fn days_in_month(y: u32, m: u32) -> u32 {
    let leap = (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400);
    [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m as usize - 1]
}

/// `HH:MM` on a 24 hour clock.
pub(crate) fn is_time(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 5
        && s.is_ascii()
        && digits(&s[..2])
        && digits(&s[3..])
        && b[2] == b':'
        && s[..2] <= *"23"
        && b[3] <= b'5'
}

/// A path that leaves the pack folder: absolute, on a drive, or climbing out with `..`.
pub(crate) fn leaves(path: &str) -> bool {
    let path = path.replace('\\', "/");
    let b = path.as_bytes();
    let drive = b.first().is_some_and(u8::is_ascii_alphabetic) && b.get(1) == Some(&b':');
    path.starts_with('/') || drive || path.split('/').any(|s| s == "..")
}

/// A number to dial: an optional `+`, then digits that spaces, dashes, dots and brackets may
/// group. At least three digits, so an emergency number passes.
pub(crate) fn is_phone(s: &str) -> bool {
    let rest = s.strip_prefix('+').unwrap_or(s);
    let grouping = |b: u8| b.is_ascii_digit() || b" -.()".contains(&b);
    all(rest, grouping) && rest.bytes().filter(u8::is_ascii_digit).count() >= 3
}

/// `YYYY-MM-DDTHH:MM`.
pub(crate) fn is_stamp(s: &str) -> bool {
    s.split_once('T').is_some_and(|(date, time)| is_date(date) && is_time(time))
}

/// Text that says it still has to be checked, and renders as a hole until it is.
pub(crate) fn to_confirm(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("[to confirm]") || lower.contains("[por confirmar]")
}

/// A key like `closed__2026_08_15` shows only on that date. Returns whether the date is real,
/// or nothing when the key has no date suffix.
pub(crate) fn dated_key(key: &str) -> Option<bool> {
    let tail = key.get(key.len().checked_sub(12)?..)?;
    let b = tail.as_bytes();
    if !tail.is_ascii() {
        return None;
    }
    let shape = &tail[..2] == "__" && b[6] == b'_' && b[9] == b'_';
    if !shape || !digits(&tail[2..6]) || !digits(&tail[7..9]) || !digits(&tail[10..]) {
        return None;
    }
    let n = |r: std::ops::Range<usize>| tail[r].parse().unwrap_or_default();
    Some(real_date(n(2..6), n(7..9), n(10..12)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_slugs() {
        for bad in ["", "Rita", "1st", "a-b", "a b", "é"] {
            assert!(!is_id(bad), "{bad}");
        }
        for good in ["rita", "_x", "day_2"] {
            assert!(is_id(good), "{good}");
        }
        for bad in ["", "-a", "a-", "a--b", "A", "a_b"] {
            assert!(!is_slug(bad), "{bad}");
        }
        for good in ["a", "one-day", "trip-2026"] {
            assert!(is_slug(good), "{good}");
        }
    }

    #[test]
    fn timezones_and_languages() {
        for bad in
            ["", "Europe", "Europe/", "/Madrid", "Europe/Ma drid", "A/B/C/D", "Eu1/Madrid", "utc"]
        {
            assert!(!is_timezone(bad), "{bad}");
        }
        for good in ["UTC", "Europe/Madrid", "America/Argentina/Buenos_Aires", "Etc/GMT+3"] {
            assert!(is_timezone(good), "{good}");
        }
        for bad in ["", "e", "espa", "ES", "pt-B", "pt-BRAZILIAN", "pt-B_"] {
            assert!(!is_language(bad), "{bad}");
        }
        for good in ["es", "pt-BR", "zh-Hant-TW", "ast"] {
            assert!(is_language(good), "{good}");
        }
    }

    #[test]
    fn dates() {
        for bad in ["", "2026-4-11", "2026-04-11-1", "20260-4-11", "2026-04-1x", "2026/04/11"] {
            assert!(!is_date(bad), "{bad}");
        }
        for unreal in
            ["2026-02-30", "2026-13-01", "2026-00-10", "2026-04-00", "0000-01-01", "2100-02-29"]
        {
            assert!(is_date(unreal) && !is_real_date(unreal), "{unreal}");
        }
        for real in ["2026-04-11", "2024-02-29", "2000-02-29", "2026-12-31"] {
            assert!(is_real_date(real), "{real}");
        }
    }

    #[test]
    fn times_and_stamps() {
        for bad in
            ["", "9:00", "24:00", "30:00", "12:60", "12-00", "ab:cd", "12:0x", "12:000", "aé:00"]
        {
            assert!(!is_time(bad), "{bad}");
        }
        for good in ["00:00", "09:30", "19:59", "23:59"] {
            assert!(is_time(good), "{good}");
        }
        assert!(is_stamp("2026-04-11T18:40"));
        for bad in ["2026-04-11", "2026-04-11 18:40", "2026-04-11T25:00", "x-04-11T18:40"] {
            assert!(!is_stamp(bad), "{bad}");
        }
    }

    #[test]
    fn to_confirm_is_a_marker_in_either_language() {
        assert!(to_confirm("price [To Confirm]"));
        assert!(to_confirm("precio [por confirmar]"));
        assert!(!to_confirm("to confirm"));
    }

    #[test]
    fn a_key_can_end_in_a_date() {
        assert_eq!(dated_key("short"), None);
        assert_eq!(dated_key("closed_2026_02_300"), None);
        assert_eq!(dated_key("closed__2026-02_30"), None);
        assert_eq!(dated_key("closed__2026_0x_30"), None);
        assert_eq!(dated_key("closed__20x6_02_30"), None);
        assert_eq!(dated_key("closed__2026_02_3x"), None);
        assert_eq!(dated_key("éééééééééééé_"), None);
        assert_eq!(dated_key("__2026_0é_3"), None);
        assert_eq!(dated_key("closed__2026_02_30"), Some(false));
        assert_eq!(dated_key("__2026_02_28"), Some(true));
    }
}
