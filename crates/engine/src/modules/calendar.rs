//! The calendar events the plan asks for (decision 0021). Times stay local to the zone the pack
//! gives the block, or the pack's own when it gives none; the host turns them into instants.

use super::{Run, list};
use crate::clock::{later, start};
use crate::engine::Event;
use crate::value::{Map, text};

/// The first sentence of a block's text, without its full stop.
fn title(text: &str) -> String {
    text.split_once(". ").map_or(text, |(first, _)| first).trim_end_matches('.').to_owned()
}

/// FNV-1a over the fields, which is enough to tell an event that changed.
fn fingerprint(fields: &[&str]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in fields.join("\u{1f}").bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

impl Run<'_> {
    /// One event per timed block of the holder's, or of everyone's when nobody holds the phone.
    /// A day's option counts only once it is chosen, so until then only the rest is written.
    pub fn events(
        &self,
        days: Option<&str>,
        places: Option<&str>,
        alerts: Option<&str>,
    ) -> Vec<Event> {
        let data = |from: Option<&str>| from.and_then(|f| self.keymap.root(self.content, f));
        let days = list(data(days));
        let (places, alerts) = (data(places), list(data(alerts)));
        let mut events = Vec::new();
        for day in days {
            let date = self.date_of(day);
            let chosen = format!("{date}.option-{}.", self.stored_choice(&date));
            let (_, timed) = self.day(days, day);
            let timed: Vec<Map> = timed
                .into_iter()
                .filter(|b| {
                    let event = text(b.get("event"));
                    !event.contains(".option-") || event.starts_with(&chosen)
                })
                .collect();
            for (n, b) in timed.iter().enumerate() {
                let time = text(b.get("time"));
                let begin = format!("{date}T{time}");
                let until = text(b.get("until"));
                let zone = text(b.get("zone"));
                let after = timed[n + 1..].iter().find(|b| text(b.get("time")) > time);
                let (end, end_zone) = match (!until.is_empty(), after) {
                    (true, _) => self.ends(&date, b),
                    (false, Some(a)) => {
                        (format!("{date}T{}", text(a.get("time"))), text(a.get("zone")))
                    }
                    (false, None) => (later(&begin, 60), zone.clone()),
                };
                let words = text(b.get("text"));
                let place = places.and_then(|p| p.get(&text(b.get("place"))));
                let location: Vec<String> = ["name", "address"]
                    .iter()
                    .map(|k| text(place.and_then(|p| self.read(p, "place", k))))
                    .filter(|s| !s.is_empty())
                    .collect();
                let mut notes = vec![words.clone()];
                let mut reminder = String::new();
                for alert in alerts {
                    let field = |k: &str| text(self.read(alert, "alert", k));
                    let (at, clock) = (field("at"), field("time"));
                    let at = if at.len() == 10 && !clock.is_empty() {
                        format!("{at}T{clock}")
                    } else {
                        start(&at)
                    };
                    if at != begin {
                        continue;
                    }
                    let lines = ["title", "detail", "action"].map(field);
                    notes.push(
                        lines.into_iter().filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n"),
                    );
                    // `start` of an empty field is empty.
                    let from = start(&field("notify_from"));
                    if !from.is_empty() && from <= begin && (reminder.is_empty() || from < reminder)
                    {
                        reminder = from;
                    }
                }
                let (title, location, notes) =
                    (title(&words), location.join(", "), notes.join("\n\n"));
                let id = text(b.get("event"));
                let fields =
                    [&id, &begin, &end, &zone, &end_zone, &title, &location, &notes, &reminder];
                let print = fingerprint(&fields.map(String::as_str));
                events.push(Event {
                    id,
                    start: begin,
                    end,
                    zone,
                    end_zone,
                    title,
                    location,
                    notes,
                    reminder,
                    fingerprint: print,
                });
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::World;
    use crate::validate::Keymap;
    use crate::value::Value;
    use crate::yaml::parse;

    const CONTENT: &str = r#"days:
  - date: 2026-04-11
    title: Arrive
    blocks:
      - ["", "A note"]
      - ["09:00", "Land. Passport queue.", {for: [rita]}]
      - ["11:15", "Museum", {place: azulejo, until: "13:00"}]
      - ["11:15", "Coffee", {place: cais, until: "11:00"}]
      - ["18:40", "Ferry.", {place: odd, for: [tomas]}]
  - date: 2026-04-12
    title: Coast or hill
    fixed: [["08:30", "Car"], ["20:00", "Dinner"]]
    options:
      - {id: coast, name: Coast, recommended: true, blocks: [["09:30", "Leave"]]}
      - {id: hill, name: Hill, blocks: [["09:00", "Train"]]}
places:
  azulejo: {name: Museum, address: "1 Tile St"}
  cais: {name: Ferry}
  odd: {kind: pier}
stops:
  - {at: "2026-04-11T11:15", title: Bags, detail: Lockers only, notify_from: 2026-04-11T10:45}
  - {at: 2026-04-11, time: "11:15", title: Tickets, notify_from: 2026-04-11T10:15}
  - {at: "2026-04-11T11:15", title: Late, notify_from: 2026-04-11T12:00}
  - {at: "2026-04-11T11:15", action: Queue early}
  - {at: 2026-04-11, title: Other}
"#;

    fn events(holder: &str, store: &[(&str, &str)], manifest: &str) -> Vec<Event> {
        let content = parse(CONTENT).unwrap().as_map().cloned().unwrap_or_default();
        let manifest = parse(manifest).unwrap();
        let store = store.iter().map(|(k, v)| ((*k).to_owned(), Value::String((*v).to_owned())));
        let world = World {
            now: "2026-04-10T10:00".into(),
            holder: holder.into(),
            store: Map(store.collect()),
            ..World::default()
        };
        let run = Run::new(Keymap::of(&manifest), &content, &world, Map::default());
        run.events(Some("days"), Some("places"), Some("stops"))
    }

    fn ids(events: &[Event]) -> Vec<&str> {
        events.iter().map(|e| e.id.as_str()).collect()
    }

    #[test]
    fn without_days_there_are_no_events() {
        let content = Map::default();
        let world = World { now: "2026-04-10T10:00".into(), ..World::default() };
        let run = Run::new(Keymap::of(&Value::Null), &content, &world, Map::default());
        assert!(run.events(None, None, None).is_empty());
    }

    #[test]
    fn an_option_is_written_only_once_chosen_and_a_block_only_for_its_people() {
        let all = events("", &[], "");
        let expected = ["2026-04-11.blocks.1", "2026-04-11.blocks.2", "2026-04-11.blocks.3"];
        let fixed = ["2026-04-11.blocks.4", "2026-04-12.fixed.0", "2026-04-12.fixed.1"];
        assert_eq!(ids(&all), [&expected[..], &fixed[..]].concat());
        let rita = events("rita", &[("choice.2026-04-12", "hill")], "");
        let hill = ["2026-04-12.fixed.0", "2026-04-12.option-hill.0", "2026-04-12.fixed.1"];
        assert_eq!(ids(&rita), [&expected[..], &hill[..]].concat());
        let stored = events("", &[("holder", "tomas")], "");
        assert_eq!(ids(&stored)[..2], ["2026-04-11.blocks.2", "2026-04-11.blocks.3"]);
    }

    #[test]
    fn an_event_ends_at_its_until_else_the_next_block_else_an_hour_on() {
        let all = events("", &[], "");
        let span = |n: usize| (all[n].start.as_str(), all[n].end.as_str());
        assert_eq!(span(0), ("2026-04-11T09:00", "2026-04-11T11:15"));
        assert_eq!(span(1), ("2026-04-11T11:15", "2026-04-11T13:00"));
        // An `until` reading before the block ends it the next day.
        assert_eq!(span(2), ("2026-04-11T11:15", "2026-04-12T11:00"));
        assert_eq!(span(3), ("2026-04-11T18:40", "2026-04-11T19:40"));
    }

    #[test]
    fn the_title_is_the_first_sentence_and_the_place_and_alerts_fill_the_rest() {
        let all = events("", &[], "");
        assert_eq!([all[0].title.as_str(), all[3].title.as_str()], ["Land", "Ferry"]);
        assert_eq!(all[0].notes, "Land. Passport queue.");
        assert_eq!(
            [all[1].location.as_str(), all[2].location.as_str()],
            ["Museum, 1 Tile St", "Ferry"]
        );
        assert_eq!([all[0].location.as_str(), all[3].location.as_str()], ["", ""]);
        assert_eq!(all[1].notes, "Museum\n\nBags\nLockers only\n\nTickets\n\nLate\n\nQueue early");
        assert_eq!(all[1].reminder, "2026-04-11T10:15");
        assert_eq!(all[0].reminder, "");
    }

    #[test]
    fn an_event_is_local_to_its_blocks_zone_and_ends_in_its_until_zone() {
        let content = r#"days:
  - date: 2026-10-03
    zone: Asia/Tokyo
    blocks:
      - ["09:00", "Land", {until: "10:00", until_zone: Asia/Seoul}]
      - ["11:00", "Train", {zone: Asia/Seoul}]
      - ["12:00", "Walk"]
"#;
        let content = parse(content).unwrap().as_map().cloned().unwrap_or_default();
        let world = World { now: "2026-10-01T10:00".into(), ..World::default() };
        let run = Run::new(Keymap::of(&Value::Null), &content, &world, Map::default());
        let all = run.events(Some("days"), None, None);
        let zones = |e: &Event| (e.zone.clone(), e.end_zone.clone());
        assert_eq!(zones(&all[0]), ("Asia/Tokyo".into(), "Asia/Seoul".into()));
        assert_eq!(zones(&all[1]), ("Asia/Seoul".into(), "Asia/Tokyo".into()));
        assert_eq!(zones(&all[2]), ("Asia/Tokyo".into(), "Asia/Tokyo".into()));
        assert_eq!(zones(&events("", &[], "")[0]), (String::new(), String::new()));
    }

    #[test]
    fn a_renamed_key_is_read_through_the_keymap_and_changes_the_fingerprint() {
        let plain = events("", &[], "");
        let renamed = events("", &[], "keymap: {place: {address: kind}}");
        assert_eq!(renamed[3].location, "pier");
        assert_ne!(plain[3].fingerprint, renamed[3].fingerprint);
        assert_eq!(plain[0].fingerprint, renamed[0].fingerprint);
        assert_eq!(plain[0].fingerprint.len(), 16);
    }
}
