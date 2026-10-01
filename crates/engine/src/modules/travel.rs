//! The travel between two blocks (decision 0032): a block the day gets between two places, from
//! the pack's own time for that leg, else estimated from where the two places are. Never moved.

use super::{Run, chart, plan, region};
use crate::clock::{minutes, shift};
use crate::value::{Map, Value, text};

/// How a day gets from one place to the next, when it says.
pub(crate) const TRAVEL: [&str; 3] = ["walking", "driving", "transit"];

/// Types that are the way between two places themselves, so no leg goes next to one.
const MOVING: [&str; 5] = ["train", "driving", "walking", "flight", "transfer"];

/// How much longer the road is than the straight line, about.
const ROAD: f64 = 1.3;

/// Minutes for `metres` in a straight line, by how the day travels; with nothing said, a walk up
/// to two kilometres of road and a drive past that. Rounded up to five, being a guess.
fn estimate(travel: &str, metres: f64) -> i64 {
    let road = metres * ROAD;
    let travel = match travel {
        "" if road <= 2000.0 => "walking",
        "" => "driving",
        t => t,
    };
    // Metres a minute, and the minutes to get going: the car parked, the stop reached.
    let (pace, start) = match travel {
        "driving" => (1000.0, 10.0),
        "transit" => (333.0, 10.0),
        _ => (75.0, 0.0),
    };
    (((start + road / pace) / 5.0).ceil() as i64 * 5).max(5)
}

/// Minutes as a trip time reads: `20 min`, `1 h`, `1 h 15 min`.
fn said(m: i64) -> String {
    match (m / 60, m % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

impl Run<'_> {
    /// The day as its page draws it: a leg before each block a different place comes before, and
    /// every timed entry with `lasts`, its minutes, to its `until` or else to the next one.
    pub(super) fn page(&self, date: &str, day: &Value, blocks: &[Map]) -> Vec<Map> {
        let travel = self.keymap.value("travel", self.read(day, "day", "travel"));
        let mut page: Vec<Map> = Vec::with_capacity(blocks.len() * 2);
        let mut last: Option<usize> = None;
        for b in blocks {
            if text(b.get("time")).is_empty() {
                page.push(b.clone());
                continue;
            }
            if let Some(leg) = last.and_then(|i| self.leg(&page[i], b, &travel)) {
                page.push(leg);
            }
            last = Some(page.len());
            page.push(b.clone());
        }
        let starts: Vec<Option<String>> = (page.iter())
            .map(|b| (!text(b.get("time")).is_empty()).then(|| self.begins(date, b)))
            .collect();
        for (i, begin) in starts.iter().enumerate() {
            let Some(begin) = begin.as_ref().filter(|_| page[i].get("lasts").is_none()) else {
                continue;
            };
            let until = text(page[i].get("until"));
            let end = if until.is_empty() {
                let next = starts[i + 1..].iter().flatten().next();
                next.cloned().unwrap_or_else(|| shift(begin, 60))
            } else {
                self.ending(date, &page[i])
            };
            let lasts = (minutes(&end) - minutes(begin)).max(0);
            page[i].set("lasts", Value::Number(lasts as f64));
        }
        page
    }

    /// The way from block `a` to block `b`, arriving at `b`'s hour: both at places, not the same
    /// one, on one clock, neither a way of moving itself, and a time to give it.
    fn leg(&self, a: &Map, b: &Map, travel: &str) -> Option<Map> {
        let (from, to) = (text(a.get("place")), text(b.get("place")));
        let moving = |m: &Map| MOVING.contains(&text(m.get("type")).as_str());
        let apart =
            from.is_empty() || to.is_empty() || from == to || a.get("zone") != b.get("zone");
        if apart || moving(a) || moving(b) {
            return None;
        }
        let places = self.places?;
        let (start, end) = (places.get(&from)?, places.get(&to)?);
        let given = |p: &Value, other: &str| {
            let m = plan::whole(self.read(p, "place", "legs")?.get(other));
            (m > 0).then_some(m)
        };
        let (lasts, duration) = match given(start, &to).or_else(|| given(end, &from)) {
            Some(m) => (m, said(m)),
            None => {
                let at = |id: &str, p: &Value| region(id, self.read(p, "place", "at")?);
                let (p, q) = (at(&from, start)?, at(&to, end)?);
                let m = estimate(travel, chart::apart(p.lat, p.lon, q.lat, q.lon));
                (m, format!("≈ {}", said(m)))
            }
        };
        let until = text(b.get("time"));
        let time = plan::moved(&until, -lasts);
        let name = Some(text(self.read(end, "place", "name"))).filter(|n| !n.is_empty());
        // Setting off before the block it leaves is over: the day does not fit as it stands.
        let over = Some(text(a.get("until"))).filter(|u| !u.is_empty());
        let late = time < over.unwrap_or_else(|| text(a.get("time")));
        let mut leg = Map::default();
        leg.set("time", Value::String(time));
        leg.set("until", Value::String(until));
        leg.set(
            "text",
            Value::String(format!("→ {}", name.unwrap_or_else(|| to.replace('_', " ")))),
        );
        leg.set("duration", Value::String(duration));
        leg.set("lasts", Value::Number(lasts as f64));
        leg.set("leg", Value::Bool(true));
        leg.set("late", Value::Bool(late));
        leg.set("from", Value::String(from));
        leg.set("to", Value::String(to));
        if let Some(zone) = b.get("zone") {
            leg.set("zone", zone.clone());
        }
        Some(leg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::World;
    use crate::validate::Keymap;
    use crate::value::show;
    use crate::yaml::parse;

    #[test]
    fn an_estimate_walks_a_short_way_drives_a_long_one_and_rounds_up() {
        assert_eq!(estimate("", 0.0), 5);
        assert_eq!(estimate("", 1000.0), 20);
        assert_eq!(estimate("", 60_000.0), 90);
        assert_eq!(estimate("walking", 60_000.0), 1040);
        assert_eq!(estimate("transit", 3000.0), 25);
        assert_eq!(estimate("driving", 1000.0), 15);
        // Two kilometres of road is still a walk; past them, a drive.
        assert_eq!(estimate("", 2000.0 / ROAD), 30);
        assert_eq!(estimate("", 2001.0 / ROAD), 15);
    }

    #[test]
    fn a_trip_time_reads_in_hours_and_minutes() {
        assert_eq!(said(20), "20 min");
        assert_eq!(said(60), "1 h");
        assert_eq!(said(75), "1 h 15 min");
    }

    /// Two museums a kilometre apart, a cafe and a market with no position, the market with a
    /// time of its own to the old museum.
    const PLACES: &str = r#"places:
  old: {name: The old museum, at: {lat: 40.0, lon: 0.0}}
  new: {name: "", at: {lat: 40.01, lon: 0.0}}
  cafe: {name: The cafe}
  market: {name: The market, legs: {old: 25, cafe: 0}}
"#;

    /// The page of a day with these blocks, as `time text {meta}` lines.
    fn page(day: &str, blocks: &str) -> Vec<String> {
        let content = parse(PLACES).unwrap().as_map().cloned().unwrap();
        let world = World { now: "2026-04-11T08:00".into(), ..World::default() };
        let mut run = Run::new(Keymap::of(&Value::Null), &content, &world, Map::default());
        run.places = content.get("places");
        let day = parse(day).unwrap();
        let blocks: Vec<Map> = parse(blocks)
            .unwrap()
            .as_list()
            .unwrap()
            .iter()
            .map(|b| b.as_map().cloned().unwrap())
            .collect();
        let shown = |b: &Map| {
            let mut b = b.clone();
            let line = format!("{} {}", text(b.get("time")), text(b.get("text")));
            b.0.retain(|(k, _)| !["time", "text", "place", "from", "to"].contains(&k.as_str()));
            format!("{line} {}", show(Some(&Value::Map(b))))
        };
        run.page("2026-04-11", &day, &blocks).iter().map(shown).collect()
    }

    #[test]
    fn a_leg_comes_between_two_places_from_the_packs_time_or_an_estimate() {
        let lines = page(
            "{}",
            r#"
- {text: A note}
- {time: "09:00", text: Old, place: old, until: "10:00"}
- {time: "10:30", text: New, place: new}
- {time: "12:00", text: Market, place: market}
- {time: "12:10", text: Old again, place: old}
"#,
        );
        assert_eq!(
            lines,
            [
                r#" A note {}"#,
                r#"09:00 Old {"until": "10:00", "lasts": 60}"#,
                r#"10:10 → new {"until": "10:30", "duration": "≈ 20 min", "lasts": 20, "leg": true, "late": false}"#,
                r#"10:30 New {"lasts": 90}"#,
                r#"12:00 Market {"lasts": 0}"#,
                r#"11:45 → The old museum {"until": "12:10", "duration": "25 min", "lasts": 25, "leg": true, "late": true}"#,
                r#"12:10 Old again {"lasts": 60}"#,
            ]
        );
    }

    #[test]
    fn a_packs_time_reads_both_ways_and_a_leg_before_the_last_block_ends_is_late() {
        let lines = page(
            "{}",
            r#"
- {time: "09:00", text: Old, place: old, until: "10:20"}
- {time: "10:30", text: New, place: new}
- {time: "11:00", text: Old, place: old}
- {time: "12:00", text: Market, place: market}
"#,
        );
        assert!(lines[1].contains(r#""late": true"#), "{lines:#?}");
        assert!(lines[5].starts_with("11:35 → The market"), "{lines:#?}");
        assert!(lines[5].contains(r#""duration": "25 min""#), "{lines:#?}");
    }

    #[test]
    fn no_leg_next_to_a_way_of_moving_a_stay_another_clock_or_a_place_with_no_time() {
        let lines = page(
            "travel: transit",
            r#"
- {time: "09:00", text: Old, place: old, zone: Europe/Madrid}
- {time: "10:00", text: New, place: new, zone: Europe/Paris}
- {time: "11:00", text: Still new, place: new, zone: Europe/Paris}
- {time: "12:00", text: Drive, place: cafe, type: driving, zone: Europe/Paris}
- {time: "13:00", text: Cafe, place: cafe, zone: Europe/Paris}
- {time: "14:00", text: Market, place: market, zone: Europe/Paris}
- {time: "15:00", text: Nowhere, place: nowhere, zone: Europe/Paris}
- {time: "15:30", text: Cafe again, place: cafe, zone: Europe/Paris}
- {time: "16:00", text: Unplaced, zone: Europe/Paris}
"#,
        );
        assert!(lines.iter().all(|l| !l.contains("→")), "{lines:#?}");
    }

    #[test]
    fn a_day_that_says_how_it_travels_is_estimated_that_way_and_a_leg_keeps_its_clock() {
        let lines = page(
            "travel: driving",
            r#"
- {time: "09:00", text: Old, place: old, zone: Europe/Madrid}
- {time: "10:00", text: New, place: new, zone: Europe/Madrid}
"#,
        );
        assert!(lines[1].starts_with("09:45 → new"), "{lines:#?}");
        assert!(lines[1].contains(r#""duration": "≈ 15 min""#), "{lines:#?}");
        assert!(lines[1].ends_with(r#""zone": "Europe/Madrid"}"#), "{lines:#?}");
    }

    #[test]
    fn with_no_places_a_page_is_the_blocks_with_their_lengths() {
        let content = Map::default();
        let world = World { now: "2026-04-11T08:00".into(), ..World::default() };
        let run = Run::new(Keymap::of(&Value::Null), &content, &world, Map::default());
        let blocks = [
            parse("{time: \"09:00\", place: old}").unwrap().as_map().cloned().unwrap(),
            parse("{time: \"10:00\", place: new}").unwrap().as_map().cloned().unwrap(),
        ];
        let page = run.page("2026-04-11", &Value::Null, &blocks);
        assert_eq!(page.len(), 2);
        assert_eq!(show(page[1].get("lasts")), "60");
    }
}
