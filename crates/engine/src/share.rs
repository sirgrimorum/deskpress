//! The trip shared phone to phone (decision 0034): a phone sends its facts, when each was kept and
//! its pack's files; another phone holding the same pack takes what is newer.

use std::collections::{HashMap, HashSet};

use crate::validate::patterns::leaves;
use crate::value::{Map, Value, quote, show, text};
use crate::yaml;

/// Facts of the phone, not the trip: who holds it, until when, and who gets it back.
pub const PERSONAL: [&str; 3] = ["holder", "holder_until", "returns_to"];

/// What a phone takes of a trip sent to it: facts with when each was kept, and the pack's files
/// with its `updated` when the pack sent is newer.
#[derive(Debug, Default, PartialEq)]
pub struct Taken {
    pub facts: Map,
    pub stamps: Vec<(String, f64)>,
    pub files: Vec<(String, String)>,
    pub updated: String,
}

/// The trip as text: the pack's `id` and `updated`, every fact but the personal ones with when
/// each was kept in `stamps`, and the pack's `files` a trip carries when it says when it was updated.
pub fn share(id: &str, updated: &str, facts: &Map, stamps: &Map, files: &Map) -> String {
    let none = Map::default();
    let files = if updated.is_empty() { &none } else { files };
    let mut out = format!("deskpress: trip\npack: {}\nupdated: {}\n", quote(id), quote(updated));
    for (name, part) in [("facts", facts), ("stamps", stamps), ("files", files)] {
        let lines: String = part
            .iter()
            .filter(|(k, _)| if name == "files" { carried(k) } else { !PERSONAL.contains(k) })
            .map(|(k, v)| format!("  {}: {}\n", quote(k), show(Some(v))))
            .collect();
        out +=
            &if lines.is_empty() { format!("{name}: {{}}\n") } else { format!("{name}:\n{lines}") };
    }
    out
}

/// What this phone takes of `sent`, a trip another phone shared: each fact kept there later than
/// here, or not here at all, and the pack's files when theirs was updated later. Refused when it
/// is not a trip, or another pack's.
pub fn take(
    sent: &str,
    id: &str,
    updated: &str,
    facts: &Map,
    stamps: &Map,
) -> Result<Taken, String> {
    let doc = yaml::parse(sent).ok().filter(|d| text(d.get("deskpress")) == "trip");
    let doc = doc.ok_or("this is not a trip DeskPress shared")?;
    let pack = text(doc.get("pack"));
    if pack != id {
        return Err(format!("this is the trip {}: open it first, then take it", quote(&pack)));
    }
    let none = Map::default();
    let part = |k| doc.get(k).and_then(Value::as_map).unwrap_or(&none);
    let (mine, my_stamps, their_stamps) = (index(facts), index(stamps), index(part("stamps")));
    let when = |at: &HashMap<&str, &Value>, k| match at.get(k) {
        Some(Value::Number(n)) => Some(*n),
        _ => None,
    };
    let mut taken = Taken::default();
    let mut keys: HashSet<String> =
        facts.keys().chain(part("facts").keys()).map(str::to_owned).collect();
    for (k, v) in part("facts").iter().filter(|(k, _)| !PERSONAL.contains(k)) {
        let at = when(&their_stamps, k);
        let key = if k.starts_with("note.") {
            note(k, v, facts, &mut keys)
        } else {
            // Theirs wins when it was kept later; one with no stamp only fills a gap.
            let newer = mine.get(k).is_none_or(|m| *m != v && at > when(&my_stamps, k));
            newer.then(|| k.to_owned())
        };
        if let Some(key) = key {
            taken.stamps.extend(at.map(|at| (key.clone(), at)));
            taken.facts.0.push((key, v.clone()));
        }
    }
    let theirs = text(doc.get("updated"));
    let files = part("files").iter().filter_map(|(p, v)| match v {
        Value::String(t) if carried(p) => Some((p.to_owned(), t.clone())),
        _ => None,
    });
    let files: Vec<_> = files.collect();
    if theirs.as_str() > updated && !files.is_empty() {
        (taken.files, taken.updated) = (files, theirs);
    }
    Ok(taken)
}

fn index(m: &Map) -> HashMap<&str, &Value> {
    m.iter().collect()
}

/// Where a note sent lands, since a note is never written over: nowhere when this phone has that
/// text in that minute, else its own key, or the next number free in that minute.
fn note(k: &str, v: &Value, facts: &Map, keys: &mut HashSet<String>) -> Option<String> {
    let minute = k.rfind('.').map_or(k, |i| &k[..=i]);
    if facts.iter().any(|(m, w)| m.starts_with(minute) && w == v) {
        return None;
    }
    if !facts.iter().any(|(m, _)| m == k) {
        return Some(k.to_owned());
    }
    let mut n = 0;
    while keys.contains(&format!("{minute}{n}")) {
        n += 1;
    }
    let free = format!("{minute}{n}");
    keys.insert(free.clone());
    Some(free)
}

/// A file a trip carries: text inside the pack, at most eight folders down, none of them hidden.
fn carried(path: &str) -> bool {
    let ext = path.rsplit_once('.').map_or("", |(_, e)| e).to_ascii_lowercase();
    let segments: Vec<&str> = path.split('/').collect();
    let plain = segments.iter().all(|s| !s.is_empty() && !s.starts_with('.') && !s.contains('\\'));
    ["yaml", "yml", "json"].contains(&ext.as_str()) && segments.len() <= 9 && plain && !leaves(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml::parse;

    fn map(src: &str) -> Map {
        parse(src).unwrap().as_map().cloned().unwrap_or_default()
    }

    fn sent(facts: &str, stamps: &str, files: &str) -> String {
        share("lisbon", "2026-04-10", &map(facts), &map(stamps), &map(files))
    }

    #[test]
    fn a_trip_goes_as_text_without_the_phones_own_facts() {
        let text = sent(
            "{holder: ana, car: {lat: 38.7, lon: -9.1}, note.a: \"line\\none\"}",
            "{holder: 1, car: 1790000000000}",
            "{pack.yaml: \"pack: {}\\n\"}",
        );
        assert_eq!(
            text,
            r#"deskpress: trip
pack: "lisbon"
updated: "2026-04-10"
facts:
  "car": {"lat": 38.7, "lon": -9.1}
  "note.a": "line\none"
stamps:
  "car": 1790000000000
files:
  "pack.yaml": "pack: {}\n"
"#
        );
        let bare = share("lisbon", "", &Map::default(), &Map::default(), &map("{pack.yaml: x}"));
        assert!(bare.ends_with("facts: {}\nstamps: {}\nfiles: {}\n"), "{bare}");
    }

    #[test]
    fn what_is_not_a_trip_or_is_another_packs_is_refused() {
        let none = Map::default();
        for text in ["", "a: [", "deskpress: theme", "- trip"] {
            let why = take(text, "lisbon", "", &none, &none).unwrap_err();
            assert_eq!(why, "this is not a trip DeskPress shared", "{text:?}");
        }
        let other = share("porto", "", &none, &none, &none);
        let why = take(&other, "lisbon", "", &none, &none).unwrap_err();
        assert_eq!(why, "this is the trip \"porto\": open it first, then take it");
    }

    #[test]
    fn each_fact_kept_later_wins_and_one_with_no_stamp_only_fills_a_gap() {
        let theirs = sent(
            "{a: 1, b: 2, c: 3, d: 4, e: 5, f: 6, g: 7, h: 8, holder: ana}",
            "{a: 20, b: 5, c: 9, e: 1, g: 10, h: 2}",
            "{}",
        );
        let mine = map("{a: 0, b: 0, c: 3, d: 0, f: 0, g: 0, h: 0, holder: tomas}");
        let stamps = map("{a: 10, b: 10, c: 1, f: 3, g: 10}");
        let taken = take(&theirs, "lisbon", "2026-04-10", &mine, &stamps).unwrap();
        // a is newer there; b older; c the same; d unstamped over mine; e new here; f unstamped;
        // g kept at the same time, so neither moves; h stamped there over one unstamped here.
        assert_eq!(taken.facts, map("{a: 1, e: 5, h: 8}"));
        let stamp = |k: &str, at| (k.to_owned(), at);
        assert_eq!(taken.stamps, [stamp("a", 20.0), stamp("e", 1.0), stamp("h", 2.0)]);
        assert_eq!((taken.files, taken.updated), (vec![], String::new()));
        // A trip with no stamps, from a phone that kept none, and a personal fact it should not hold.
        let bare = "deskpress: trip\npack: lisbon\nfacts: {i: 7, holder: ana, returns_to: x}\n";
        let gap = take(bare, "lisbon", "", &mine, &stamps).unwrap();
        assert_eq!((gap.facts, gap.stamps), (map("{i: 7}"), vec![]));
        let personal = map("{holder: a, holder_until: b, returns_to: c}");
        let out = share("lisbon", "", &personal, &personal, &mine);
        assert!(!out.contains("holder") && !out.contains("returns_to"), "{out}");
    }

    #[test]
    fn two_phones_notes_add_up_and_one_already_here_is_not_taken_twice() {
        let m = "note.2026-10-01T09:12";
        let theirs = sent(
            &format!(
                "{{'{m}.0': same, '{m}.1': theirs, '{m}.2': also, 'note.2026-10-01T09:30.1': b}}"
            ),
            &format!("{{'{m}.1': 5}}"),
            "{}",
        );
        let mine = map(&format!("{{'{m}.0': same, '{m}.1': mine, '{m}.3': also}}"));
        let taken = take(&theirs, "lisbon", "", &mine, &Map::default()).unwrap();
        // The clash takes the first number free in its minute; a note here already is skipped.
        let want = format!("{{'{m}.4': theirs, 'note.2026-10-01T09:30.1': b}}");
        assert_eq!(taken.facts, map(&want));
        assert_eq!(taken.stamps, [(format!("{m}.4"), 5.0)]);
        // Taken again, nothing is new.
        let mut after = mine.clone();
        after.0.extend(taken.facts.0);
        assert_eq!(take(&theirs, "lisbon", "", &after, &Map::default()).unwrap().facts, map("{}"));
    }

    #[test]
    fn the_packs_files_come_only_when_it_was_updated_later_and_stay_inside_it() {
        let none = Map::default();
        let files = map(
            "{pack.yaml: new, days/a.yaml: a, ../out.yaml: x, /etc/x: y, b.yaml: [1], c.JSON: c,
              docs/passport.pdf: x, .claude/s.json: x, 'a\\b.yaml': x, '': x, a//b.yaml: x,
              a/b/c/d/e/f/g/h/i.yaml: x, a/b/c/d/e/f/g/h/i/j.yaml: x, C:/x.yaml: x}",
        );
        let newer = share("lisbon", "2026-04-10T09:00", &none, &none, &files);
        let taken = take(&newer, "lisbon", "2026-04-10", &none, &none).unwrap();
        let file = |p: &str, t: &str| (p.to_owned(), t.to_owned());
        let deep = file("a/b/c/d/e/f/g/h/i.yaml", "x");
        let want = [file("pack.yaml", "new"), file("days/a.yaml", "a"), file("c.JSON", "c"), deep];
        assert_eq!(taken.files, want);
        assert_eq!(taken.updated, "2026-04-10T09:00");
        for mine in ["2026-04-10T09:00", "2026-04-11"] {
            let same = take(&newer, "lisbon", mine, &none, &none).unwrap();
            assert_eq!(same.files, [], "{mine}");
        }
        let empty = share("lisbon", "2026-04-11", &none, &none, &map("{../x: y}"));
        let taken = take(&empty, "lisbon", "2026-04-10", &none, &none).unwrap();
        assert_eq!((taken.files, taken.updated), (vec![], String::new()));
    }
}
