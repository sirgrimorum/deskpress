//! The body clock across the trip (decision 0033): each day's steps to wake, sleep, seek or keep
//! out of the light and take coffee or not, the one on now, the next, and a bed time the plan runs past.

use super::{Run, holds, list};
use crate::value::{Map, Value, text, truthy};

/// What a step asks of the body; a pack in another language maps its words in `keymap.values.do`.
pub(crate) const DOES: [&str; 7] = ["wake", "bed", "sleep", "light", "dark", "coffee", "no_coffee"];

/// Blocks a bed time may fall in without a clash: a night's own, or one spent being carried.
const RESTING: [&str; 5] = ["night", "lodging", "flight", "train", "transfer"];

/// A step and the stamp it begins at.
type Begun = (String, Map);

impl Run<'_> {
    /// The steps of a jet-lag day for the holder, each with its place in the list, canonical keys
    /// and the day's zone when it has none of its own.
    pub(super) fn steps(&self, day: &Value) -> Vec<(usize, Map)> {
        let (holder, zone) = (self.holder_id(), self.read(day, "jet_lag", "zone"));
        let steps = list(self.read(day, "jet_lag", "steps")).iter().enumerate();
        let step = |(n, s): (usize, &Value)| {
            let mut m = self.canon(s, "step");
            if let Some(zone) = zone.filter(|_| m.get("zone").is_none()) {
                m.set("zone", zone.clone());
            }
            if let Some(does) = m.get("do") {
                m.set("do", Value::String(self.keymap.value("do", Some(does))));
            }
            holds(&m, &holder).then_some((n, m))
        };
        steps.filter_map(step).collect()
    }

    /// `jet_lag` is today's entry in its own zone: its steps with a `state`, `now` the last one
    /// begun and still going (to its `until`, else the next step), `next` the first to come, and
    /// a bed or sleep step with `clash`, the first block of today's plan still going or to come.
    pub(super) fn jet_lag(&mut self, data: Option<&Value>) {
        let now = self.world.now.clone();
        let date_of = |d: &Value| text(self.read(d, "jet_lag", "date"));
        let local = |d: &Value| {
            let zone = text(self.read(d, "jet_lag", "zone"));
            self.local(&zone).unwrap_or_else(|| now.clone())
        };
        let Some(today) = list(data).iter().find(|d| date_of(d) == local(d)[..10]) else {
            self.set("jet_lag", Value::Null);
            return;
        };
        let date = date_of(today);
        let steps = self.steps(today);
        let timed = |s: &Map| !text(s.get("time")).is_empty();
        let begins: Vec<Option<String>> =
            steps.iter().map(|(_, s)| timed(s).then(|| self.begins(&date, s))).collect();
        let awake = self.awake(&date);
        let (mut current, mut next): (Option<Begun>, Option<Begun>) = (None, None);
        let mut out = Vec::with_capacity(steps.len());
        for ((_, mut s), begin) in steps.into_iter().zip(&begins) {
            let Some(begin) = begin else {
                s.set("state", Value::String("note".into()));
                out.push(Value::Map(s));
                continue;
            };
            let end = if text(s.get("until")).is_empty() {
                begins.iter().flatten().filter(|b| *b > begin).min().cloned()
            } else {
                Some(self.ending(&date, &s))
            };
            if ["bed", "sleep"].contains(&text(s.get("do")).as_str())
                && let Some((_, _, what)) = awake
                    .iter()
                    .find(|(b, e, _)| b >= begin || e.as_ref().is_some_and(|e| e > begin))
            {
                s.set("clash", Value::String(what.clone()));
            }
            let state = if *begin > now {
                "next"
            } else if end.as_ref().is_some_and(|e| *e <= now) {
                "past"
            } else {
                "now"
            };
            self.until
                .extend([Some(begin.clone()), end].into_iter().flatten().filter(|u| *u > now));
            s.set("state", Value::String(state.into()));
            let pick = match state {
                "now" => current.as_ref().is_none_or(|(b, _)| begin >= b).then_some(&mut current),
                "next" => next.as_ref().is_none_or(|(b, _)| begin < b).then_some(&mut next),
                _ => None,
            };
            if let Some(slot) = pick {
                *slot = Some((begin.clone(), s.clone()));
            }
            out.push(Value::Map(s));
        }
        let mut m = self.canon(today, "jet_lag");
        m.set("steps", Value::List(out));
        m.set("now", current.map_or(Value::Null, |(_, s)| Value::Map(s)));
        m.set("next", next.map_or(Value::Null, |(_, s)| Value::Map(s)));
        self.set("jet_lag", Value::Map(m));
    }

    /// Today's timed blocks on `date` that keep someone up, as when each begins, when it ends if
    /// it says, and its text; none when the timeline's day is another one.
    fn awake(&self, date: &str) -> Vec<(String, Option<String>, String)> {
        let day = self.scope.get("day");
        if text(day.and_then(|d| d.get("date"))) != date {
            return Vec::new();
        }
        let block = |b: &Value| {
            let b = b.as_map()?;
            let kind = text(b.get("type"));
            if text(b.get("time")).is_empty() || truthy(b.get("leg")) || RESTING.contains(&&*kind) {
                return None;
            }
            let end = (!text(b.get("until")).is_empty()).then(|| self.ending(date, b));
            Some((self.begins(date, b), end, text(b.get("text"))))
        };
        list(day.and_then(|d| d.get("blocks"))).iter().filter_map(block).collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::World;
    use crate::modules::tests::run as module;
    use crate::value::{Map, Value, show};
    use crate::yaml::parse;

    const PLAN: &str = r#"jet_lag:
  - date: 2026-10-01
    where: Home
    steps:
      - {text: Pack light}
      - {time: "07:00", text: Up, do: wake, alarm: true}
      - {time: "07:30", until: "09:00", text: Sun, do: light}
      - {time: "08:00", text: Coffee, do: cafe, for: [rita]}
      - {time: "18:00", text: Bed, do: bed, alarm: true}
  - date: 2026-10-02
    zone: Europe/Madrid
    steps:
      - {time: "22:00", until: "06:00", text: Sleep, do: sleep}
"#;

    /// The jet-lag module over `PLAN` at `now`, after a timeline day given as YAML.
    fn run(now: &str, holder: &str, zones: &str, day: &str) -> Map {
        let manifest = "keymap: {values: {do: {coffee: cafe}}}";
        let world = World {
            now: now.into(),
            holder: holder.into(),
            zones: parse(zones).unwrap().as_map().cloned().unwrap_or_default(),
            ..World::default()
        };
        let out = module("jet_lag", manifest, PLAN, &world, &format!("day: {day}"));
        let mut scope = out.scope;
        scope.set("until", Value::List(out.until.into_iter().map(Value::String).collect()));
        scope
    }

    fn step<'v>(v: &'v Value, key: &str) -> Option<&'v Value> {
        match (v, key.parse::<usize>()) {
            (Value::List(l), Ok(i)) => l.get(i),
            _ => v.get(key),
        }
    }

    /// What sits at a dotted path, a number indexing a list.
    fn at(m: &Map, path: &str) -> String {
        let mut keys = path.split('.');
        let first = m.get(keys.next().unwrap());
        show(keys.fold(first, |v, k| v.and_then(|v| step(v, k))))
    }

    fn states(m: &Map) -> String {
        let steps = m.get("jet_lag").and_then(|j| j.get("steps")).and_then(Value::as_list);
        let state = |s: &Value| show(s.get("state"));
        steps.unwrap_or_default().iter().map(state).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn no_entry_today_is_no_jet_lag() {
        assert_eq!(at(&run("2026-09-30T10:00", "", "{}", "null"), "jet_lag"), "null");
    }

    #[test]
    fn a_step_is_on_from_its_time_to_its_until_else_to_the_next_step() {
        let out = run("2026-10-01T07:45", "", "{}", "null");
        assert_eq!(states(&out), r#""note" "past" "now" "next" "next""#);
        assert_eq!(at(&out, "jet_lag.now.text"), r#""Sun""#);
        assert_eq!(at(&out, "jet_lag.next.text"), r#""Coffee""#);
        assert_eq!(at(&out, "jet_lag.where"), r#""Home""#);
        assert_eq!(at(&out, "jet_lag.steps.3.do"), r#""coffee""#);
        assert_eq!(
            at(&out, "until"),
            r#"["2026-10-01T09:00", "2026-10-01T08:00", "2026-10-01T18:00", "2026-10-01T18:00"]"#
        );
        // Two on at once: the one begun last is now. The last step lasts the rest of the day.
        let out = run("2026-10-01T08:30", "", "{}", "null");
        assert_eq!(states(&out), r#""note" "past" "now" "now" "next""#);
        assert_eq!(at(&out, "jet_lag.now.text"), r#""Coffee""#);
        let out = run("2026-10-01T20:00", "tomas", "{}", "null");
        assert_eq!(states(&out), r#""note" "past" "past" "now""#);
        assert_eq!(at(&out, "jet_lag.next"), "null");
    }

    #[test]
    fn a_day_in_another_zone_runs_on_that_clock_and_an_until_crosses_midnight() {
        let out = run("2026-10-02T15:00", "", "{Europe/Madrid: \"2026-10-02T23:00\"}", "null");
        assert_eq!(at(&out, "jet_lag.now.text"), r#""Sleep""#);
        assert_eq!(at(&out, "jet_lag.steps.0.zone"), r#""Europe/Madrid""#);
        assert_eq!(at(&out, "until"), r#"["2026-10-02T22:00"]"#);
        // Today is the date on the day's own clock, not the pack's.
        let over = run("2026-10-02T23:30", "", "{Europe/Madrid: \"2026-10-03T00:30\"}", "null");
        assert_eq!(at(&over, "jet_lag"), "null");
        let still = run("2026-10-03T00:30", "", "{Europe/Madrid: \"2026-10-02T23:30\"}", "null");
        assert_eq!(at(&still, "jet_lag.now.text"), r#""Sleep""#);
    }

    #[test]
    fn a_sleep_step_clashes_as_a_bed_time_does() {
        let zones = "{Europe/Madrid: \"2026-10-02T16:00\"}";
        let late = r#"{date: 2026-10-02, blocks: [{time: "23:00", text: Late show}]}"#;
        let out = run("2026-10-02T15:00", "", zones, late);
        assert_eq!(at(&out, "jet_lag.steps.0.clash"), r#""Late show""#);
    }

    #[test]
    fn a_bed_time_clashes_with_a_block_still_going_or_to_come_that_keeps_someone_up() {
        let day = |blocks: &str| format!("{{date: 2026-10-01, blocks: {blocks}}}");
        let clash = |blocks: &str| {
            at(&run("2026-10-01T07:00", "", "{}", &day(blocks)), "jet_lag.steps.4.clash")
        };
        assert_eq!(clash(r#"[[a], {time: "16:00", text: Pack, until: "20:00"}]"#), r#""Pack""#);
        assert_eq!(clash(r#"[{time: "19:00", text: Dinner}]"#), r#""Dinner""#);
        let resting = r#"[{time: "16:00", text: Pack, until: "17:00"}, {time: "17:00", text: Drive},
          {time: "19:00", text: Fly, type: flight}, {time: "19:00", text: L, leg: true}, {text: Note}]"#;
        assert_eq!(clash(resting), "nothing");
        let other = r#"{date: 2026-09-30, blocks: [{time: "19:00", text: Dinner}]}"#;
        assert_eq!(
            at(&run("2026-10-01T07:00", "", "{}", other), "jet_lag.steps.4.clash"),
            "nothing"
        );
    }
}
