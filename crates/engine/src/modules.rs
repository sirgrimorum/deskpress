//! What each module puts in the scope for one world, and the instants and regions at which that
//! would change. Every module hands out canonical keys, whatever the pack calls them.

use std::cmp::Reverse;

use crate::clock::next_day;
use crate::engine::World;
use crate::validate::{Keymap, SEVERITIES};
use crate::value::{Map, Value, text, truthy};

/// One run of the modules over one world.
pub(crate) struct Run<'a> {
    keymap: Keymap<'a>,
    content: &'a Map,
    world: &'a World,
    date: &'a str,
    time: &'a str,
    pub scope: Map,
    /// Instants at which something exposed would change. Any of them may be past.
    pub until: Vec<String>,
    pub regions: Vec<String>,
}

fn list(data: Option<&Value>) -> &[Value] {
    data.and_then(Value::as_list).unwrap_or_default()
}

fn id(v: &Value) -> String {
    text(v.get("id"))
}

impl<'a> Run<'a> {
    pub fn new(keymap: Keymap<'a>, content: &'a Map, world: &'a World, scope: Map) -> Self {
        let (date, time) = (&world.now[..10], &world.now[11..]);
        Run { keymap, content, world, date, time, scope, until: Vec::new(), regions: Vec::new() }
    }

    pub fn module(&mut self, name: &str, from: &str) {
        let data = self.keymap.root(self.content, from);
        match name {
            "timeline" => self.timeline(data),
            "choices" => self.choices(data),
            "people" => self.people(data),
            "places" => self.places(data),
            "alerts" => self.alerts(data),
            "documents" => self.documents(data),
            _ => self.climate(data),
        }
    }

    fn set(&mut self, name: &str, value: Value) {
        self.scope.set(name, value);
    }

    fn today(&self, time: &str) -> String {
        format!("{}T{time}", self.date)
    }

    /// `obj` with canonical keys, as a mapping.
    fn canon(&self, obj: &Value, kind: &str) -> Map {
        match self.keymap.canon(obj, kind) {
            Value::Map(m) => m,
            _ => Map::default(),
        }
    }

    fn read<'v>(&self, obj: &'v Value, kind: &str, key: &str) -> Option<&'v Value> {
        self.keymap.read(obj, kind, key)
    }

    fn date_of(&self, day: &Value) -> String {
        text(self.read(day, "day", "date"))
    }

    fn stored_choice(&self, date: &str) -> String {
        text(self.world.store.get(&format!("choice.{date}")))
    }

    /// The options a day keeps once `requires` is applied, and the one in force: the stored
    /// choice, else the recommended one, else the first.
    fn options<'v>(
        &self,
        days: &'v [Value],
        day: &'v Value,
        required: bool,
    ) -> (Vec<&'v Value>, Option<&'v Value>) {
        let all = list(self.read(day, "day", "options"));
        let kept: Vec<&Value> =
            all.iter().filter(|o| !required || self.required(days, o)).collect();
        let stored = self.stored_choice(&self.date_of(day));
        let recommended =
            |o: &&&Value| self.read(o, "option", "recommended") == Some(&Value::Bool(true));
        let chosen = kept.iter().find(|o| id(o) == stored);
        let chosen = chosen.or_else(|| kept.iter().find(recommended)).or(kept.first()).copied();
        (kept, chosen)
    }

    /// Whether an option's `requires` holds: the day it names has that option in force. One
    /// level only; the required day's own `requires` are not applied.
    fn required(&self, days: &[Value], option: &Value) -> bool {
        let Some(requires) = option.get("requires") else {
            return true;
        };
        let date = text(requires.get("date"));
        let day = days.iter().find(|d| self.date_of(d) == date);
        let chosen = day.and_then(|d| self.options(days, d, false).1);
        chosen.is_some_and(|c| id(c) == text(requires.get("option")))
    }

    fn option_list(&self, kept: &[&Value]) -> Value {
        Value::List(kept.iter().map(|o| Value::Map(self.canon(o, "option"))).collect())
    }

    /// `[time, text, meta]` as `{time, text, ...meta}`, its type in the canonical word.
    fn block(&self, block: &Value) -> Map {
        let parts = list(Some(block));
        let mut m = Map(vec![
            ("time".into(), Value::String(text(parts.first()))),
            ("text".into(), parts.get(1).cloned().unwrap_or(Value::Null)),
        ]);
        if let Some(meta) = parts.get(2) {
            m.0.extend(self.canon(meta, "block").0);
        }
        if let Some(kind) = m.get("type") {
            let kind = self.keymap.value("type", Some(kind));
            m.set("type", Value::String(kind));
        }
        m
    }

    /// A block of today with its `state` for the agenda: a note has no time, `now` is the current
    /// block, `past` one whose time has come, and one to come is `locked` or `next`.
    fn stated(&self, block: &Value, current: &Value) -> Value {
        let mut m = block.as_map().cloned().unwrap_or_default();
        let time = text(m.get("time"));
        let state = if time.is_empty() {
            "note"
        } else if block == current {
            "now"
        } else if time.as_str() <= self.time {
            "past"
        } else if truthy(m.get("locked")) {
            "locked"
        } else {
            "next"
        };
        m.set("state", Value::String(state.to_owned()));
        Value::Map(m)
    }

    /// A day with canonical keys, its blocks in force as mappings, its options and its choice.
    fn day(&self, days: &[Value], day: &Value) -> (Map, Vec<Map>) {
        let (kept, chosen) = self.options(days, day, true);
        let mut blocks: Vec<&Value> = Vec::new();
        for key in ["blocks", "fixed"] {
            blocks.extend(list(self.read(day, "day", key)));
        }
        let mut blocks: Vec<Map> = match chosen {
            Some(option) => {
                blocks.extend(list(self.read(option, "option", "blocks")));
                let mut blocks: Vec<Map> = blocks.iter().map(|b| self.block(b)).collect();
                blocks.sort_by_key(|b| text(b.get("time")));
                blocks
            }
            None => blocks.iter().map(|b| self.block(b)).collect(),
        };
        let mut m = self.canon(day, "day");
        m.set("blocks", Value::List(blocks.iter().cloned().map(Value::Map).collect()));
        if self.read(day, "day", "options").is_some() {
            m.set("options", self.option_list(&kept));
        }
        m.set("choice", chosen.map_or(Value::Null, |c| Value::String(id(c))));
        blocks.retain(|b| !text(b.get("time")).is_empty());
        (m, blocks)
    }

    /// `day` is today, `started` once a block's time has come; `block` the last one whose time
    /// has come, until its `until`; `next` the first one still to come. `days` is every day and
    /// `tomorrow` the day after this one, with its `first` timed block.
    fn timeline(&mut self, data: Option<&Value>) {
        let days = list(data);
        let (mut day, mut block, mut next) = (Value::Null, Value::Null, Value::Null);
        if let Some(today) = days.iter().find(|d| self.date_of(d) == self.date) {
            let (mut map, timed) = self.day(days, today);
            let started = timed.iter().any(|b| text(b.get("time")).as_str() <= self.time);
            let mut current = None;
            for b in timed {
                if text(b.get("time")).as_str() <= self.time {
                    current = Some(b);
                } else if next == Value::Null {
                    self.until.push(self.today(&text(b.get("time"))));
                    next = Value::Map(b);
                }
            }
            if let Some(b) = current {
                let until = text(b.get("until"));
                if until.is_empty() || until.as_str() > self.time {
                    if !until.is_empty() {
                        self.until.push(self.today(&until));
                    }
                    block = Value::Map(b);
                }
            }
            let blocks = list(map.get("blocks")).iter().map(|b| self.stated(b, &block)).collect();
            map.set("blocks", Value::List(blocks));
            map.set("started", Value::Bool(started));
            day = Value::Map(map);
        }
        let date = next_day(self.date);
        let tomorrow = days.iter().find(|d| self.date_of(d) == date);
        let tomorrow = tomorrow.map_or(Value::Null, |t| {
            let (mut map, timed) = self.day(days, t);
            map.set("first", timed.into_iter().next().map_or(Value::Null, Value::Map));
            Value::Map(map)
        });
        let all = days.iter().map(|d| Value::Map(self.canon(d, "day"))).collect();
        self.set("day", day);
        self.set("block", block);
        self.set("next", next);
        self.set("days", Value::List(all));
        self.set("tomorrow", tomorrow);
    }

    /// The decision to show: the first one due and unanswered, else the first one due, else the
    /// next one coming, with `due` false.
    fn choices(&mut self, data: Option<&Value>) {
        let days = list(data);
        let now = &self.world.now;
        let mut best: Option<(u8, String, &Value, &Value)> = None;
        for day in days {
            let Some(decision) = self.read(day, "day", "decision").filter(|d| d.as_map().is_some())
            else {
                continue;
            };
            let when = text(self.read(decision, "decision", "when"));
            let at = Some(text(self.read(decision, "decision", "at"))).filter(|t| !t.is_empty());
            let ask = format!("{when}T{}", at.as_deref().unwrap_or("00:00"));
            let (kept, _) = self.options(days, day, true);
            let stored = self.stored_choice(&self.date_of(day));
            let answered = kept.iter().any(|o| id(o) == stored);
            let rank = if ask.as_str() > now.as_str() {
                self.until.push(ask.clone());
                2
            } else if self.date <= self.date_of(day).as_str() {
                u8::from(answered)
            } else {
                continue;
            };
            // Among the due ones the first in the file wins; among the coming ones, the soonest.
            let better =
                best.as_ref().is_none_or(|(r, a, ..)| rank < *r || (rank == 2 && ask < *a));
            if better {
                best = Some((rank, ask, day, decision));
            }
        }
        let decision = best.map_or(Value::Null, |(rank, _, day, decision)| {
            let (kept, chosen) = self.options(days, day, true);
            let recommended = kept
                .iter()
                .find(|o| self.read(o, "option", "recommended") == Some(&Value::Bool(true)));
            let date = self.date_of(day);
            let mut m = self.canon(decision, "decision");
            m.set("date", Value::String(date.clone()));
            m.set("title", self.read(day, "day", "title").cloned().unwrap_or(Value::Null));
            m.set("options", self.option_list(&kept));
            m.set("recommended", recommended.map_or(Value::Null, |o| Value::String(id(o))));
            m.set("choice", chosen.map_or(Value::Null, |c| Value::String(id(c))));
            m.set("due", Value::Bool(rank < 2));
            m.set("answered", Value::Bool(rank == 1));
            Value::Map(m)
        });
        self.set("decision", decision);
    }

    /// `holder` is the person holding the phone: the one the host says, else the one stored as
    /// `holder` by a relay. `people` is everyone.
    fn people(&mut self, data: Option<&Value>) {
        let people: Vec<Map> = list(data).iter().map(|p| self.canon(p, "person")).collect();
        let stored = text(self.world.store.get("holder"));
        let id = if self.world.holder.is_empty() { &stored } else { &self.world.holder };
        let holder = people.iter().find(|p| text(p.get("id")) == *id);
        let holder = holder.cloned().map_or(Value::Null, Value::Map);
        self.set("holder", holder);
        self.set("people", Value::List(people.into_iter().map(Value::Map).collect()));
    }

    /// `place` is where the current block happens, `here` the first place the device is inside.
    /// Every place with coordinates is a region to watch.
    fn places(&mut self, data: Option<&Value>) {
        let places = data.and_then(Value::as_map);
        let find = |id: &str| {
            let place = places?.get(id)?;
            let mut m = self.canon(place, "place");
            for kind in ["during", "parking"] {
                if let Some(v) = m.get(kind).map(|v| self.keymap.canon(v, kind)) {
                    m.set(kind, v);
                }
            }
            let points = m.get("points").and_then(Value::as_list);
            let points: Option<Vec<Value>> =
                points.map(|l| l.iter().map(|p| self.keymap.canon(p, "point")).collect());
            if let Some(points) = points {
                m.set("points", Value::List(points));
            }
            m.set("id", Value::String(id.to_owned()));
            Some(Value::Map(m))
        };
        let at = text(self.scope.get("block").and_then(|b| b.get("place")));
        let place = find(&at).unwrap_or(Value::Null);
        let here = self.world.inside.iter().find_map(|id| find(id)).unwrap_or(Value::Null);
        let geofenced = places.map(Map::iter).into_iter().flatten();
        let regions: Vec<String> = geofenced
            .filter(|(_, p)| self.read(p, "place", "at").is_some())
            .map(|(id, _)| id.to_owned())
            .collect();
        self.regions.extend(regions);
        self.set("place", place);
        self.set("here", here);
    }

    /// The alerts showing now, most severe first. One shows from `notify_from`, else from the
    /// start of its day, to the end of its day.
    fn alerts(&mut self, data: Option<&Value>) {
        let now = self.world.now.as_str();
        let mut active: Vec<(usize, Map)> = Vec::new();
        for alert in list(data) {
            let at = text(self.read(alert, "alert", "at"));
            let from = text(self.read(alert, "alert", "notify_from"));
            let day = at.get(..10);
            let start = match (from.as_str(), day) {
                ("", Some(day)) => format!("{day}T00:00"),
                ("", None) => String::new(),
                (from, _) => crate::clock::start(from),
            };
            if start.as_str() > now {
                self.until.push(start);
                continue;
            }
            if day.is_some_and(|d| d < self.date) {
                continue;
            }
            let severity = self.keymap.value("severity", self.read(alert, "alert", "severity"));
            let rank = SEVERITIES.iter().position(|s| *s == severity).unwrap_or(SEVERITIES.len());
            let mut m = self.canon(alert, "alert");
            m.set("severity", Value::String(severity));
            active.push((rank, m));
        }
        active.sort_by_key(|(rank, _)| *rank);
        self.set("alerts", Value::List(active.into_iter().map(|(_, m)| Value::Map(m)).collect()));
    }

    fn documents(&mut self, data: Option<&Value>) {
        let documents = list(data).iter().map(|d| Value::Map(self.canon(d, "document")));
        self.set("documents", Value::List(documents.collect()));
    }

    /// The weather for today at `place`, else `here`: the most specific entry wins, and a key it
    /// lacks falls through to the next one that matches.
    fn climate(&mut self, data: Option<&Value>) {
        let id_of = |name: &str| text(self.scope.get(name).and_then(|p| p.get("id")));
        let place = Some(id_of("place")).filter(|p| !p.is_empty()).unwrap_or_else(|| id_of("here"));
        let month: f64 = self.date[5..7].parse().unwrap_or_default();
        let entries = list(data.and_then(|c| c.get("entries")));
        let mut ranked: Vec<(u8, Map)> = entries
            .iter()
            .filter_map(|e| {
                let e = self.canon(e, "climate");
                let own = text(e.get("place"));
                if !own.is_empty() && own != place {
                    return None;
                }
                let when = match (e.get("date"), e.get("month")) {
                    (Some(d), _) if text(Some(d)) == self.date => 2,
                    (Some(_), _) => return None,
                    (None, Some(Value::Number(m))) if *m == month => 1,
                    (None, Some(_)) => return None,
                    (None, None) => 0,
                };
                Some((when * 2 + u8::from(!own.is_empty()), e))
            })
            .collect();
        ranked.sort_by_key(|(rank, _)| Reverse(*rank));
        let weather = if ranked.is_empty() {
            Value::Null
        } else {
            let pick = |key: &str| {
                ranked.iter().find_map(|(_, e)| e.get(key)).cloned().unwrap_or(Value::Null)
            };
            let keys = ["high", "low", "rain", "sunrise", "sunset", "summary"];
            let mut m = Map(keys.iter().map(|k| ((*k).to_owned(), pick(k))).collect());
            let units = data.and_then(|c| c.get("units")).cloned();
            m.set("units", units.unwrap_or_else(|| Value::String("metric".into())));
            m.set("as_of", Value::Null);
            Value::Map(m)
        };
        self.set("weather", weather);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::define::MODULES;
    use crate::value::show;
    use crate::yaml::parse;

    const DAYS: &str = r#"days:
  - date: 2026-04-11
    title: Arrive
    blocks:
      - ["", "A note"]
      - ["10:30", "Walk", {type: walking}]
      - ["11:15", "Museum", {type: visit, place: azulejo, until: "13:00"}]
      - ["18:40", "Ferry", {place: cais, locked: true}]
  - date: 2026-04-12
    title: Coast or hill
    fixed: [["08:30", "Car"], ["20:00", "Dinner"]]
    options:
      - {id: coast, name: Coast, recommended: true, blocks: [["09:30", "Leave"]]}
      - {id: hill, name: Hill, blocks: [["09:00", "Train"]]}
    decision: {when: 2026-04-11, at: "21:00", question: Which?}
  - date: 2026-04-13
    title: After
    options:
      - {id: beach, name: Beach, requires: {date: 2026-04-12, option: coast}, blocks: []}
      - {id: town, name: Town, requires: {date: 2026-04-12, option: hill}, blocks: []}
      - {id: rest, name: Rest, blocks: []}
    decision: {when: 2026-04-12}
  - date: 2026-04-14
    title: Early question
    options: [{id: a, name: A, blocks: []}, {id: b, name: B, blocks: []}]
    decision: {when: 2026-04-09, at: "12:00"}
  - date: 2026-04-15
    title: Odd blocks
    blocks: [["07:00"], ["07:30", "x", "oops"]]
    decision: x
"#;

    fn world(now: &str, store: &[(&str, &str)]) -> World {
        let store = store.iter().map(|(k, v)| ((*k).to_owned(), Value::String((*v).to_owned())));
        World {
            now: now.into(),
            holder: "rita".into(),
            inside: vec!["nowhere".into(), "cais".into()],
            store: Map(store.collect()),
        }
    }

    struct Out {
        scope: Map,
        until: Vec<String>,
        regions: Vec<String>,
    }

    impl Out {
        /// What sits at a dotted path of the scope, as a message would show it.
        fn at(&self, path: &str) -> String {
            let mut keys = path.split('.');
            let first = self.scope.get(keys.next().unwrap_or_default());
            show(keys.fold(first, |v, k| v.and_then(|v| v.get(k))))
        }

        /// The values of `key` in the list at `path`, in order.
        fn each(&self, path: &str, key: &str) -> String {
            let marker = format!("\"{key}\": \"");
            let found = self.at(path);
            let values =
                found.split(&marker).skip(1).map(|s| s.split('"').next().unwrap_or_default());
            values.collect::<Vec<_>>().join(" ")
        }
    }

    fn run(module: &str, manifest: &str, content: &str, world: &World, scope: &str) -> Out {
        let (manifest, content) = (parse(manifest).unwrap(), parse(content).unwrap());
        let scope = parse(scope).unwrap().as_map().cloned().unwrap_or_default();
        let from = MODULES.iter().find(|(n, ..)| *n == module).map(|(_, r, _)| *r).unwrap();
        let content = content.as_map().cloned().unwrap_or_default();
        let mut r = Run::new(Keymap::of(&manifest), &content, world, scope);
        r.module(module, from);
        Out { scope: r.scope, until: r.until, regions: r.regions }
    }

    fn timeline(now: &str, store: &[(&str, &str)]) -> Out {
        run("timeline", "", DAYS, &world(now, store), "")
    }

    fn choices(now: &str, store: &[(&str, &str)]) -> Out {
        run("choices", "", DAYS, &world(now, store), "")
    }

    fn pair(a: String, b: String) -> (String, String) {
        (a, b)
    }

    fn strs(a: &str, b: &str) -> (String, String) {
        (a.to_owned(), b.to_owned())
    }

    #[test]
    fn with_no_day_today_the_timeline_is_empty() {
        let none = run("timeline", "", "", &world("2026-04-11T10:00", &[]), "");
        for out in [timeline("2026-04-20T10:00", &[]), none] {
            assert_eq!([out.at("day"), out.at("block"), out.at("next")], ["null", "null", "null"]);
            assert!(out.until.is_empty());
        }
    }

    #[test]
    fn the_block_is_the_last_one_begun_until_it_ends() {
        let out = timeline("2026-04-11T11:30", &[]);
        assert_eq!(
            out.at("block"),
            r#"{"time": "11:15", "text": "Museum", "type": "visit", "place": "azulejo", "until": "13:00"}"#
        );
        assert_eq!(out.at("next.text"), r#""Ferry""#);
        assert_eq!(out.until, ["2026-04-11T18:40", "2026-04-11T13:00"]);
        assert_eq!(out.at("day.title"), r#""Arrive""#);
        assert_eq!(out.at("day.choice"), "null");
        assert_eq!(out.at("day.options"), "nothing");
        // The note stays in the day, and never becomes the block.
        assert_eq!(out.each("day.blocks", "time"), " 10:30 11:15 18:40");

        let out = timeline("2026-04-11T13:00", &[]);
        assert_eq!(pair(out.at("block"), out.at("next.time")), strs("null", r#""18:40""#));
        let out = timeline("2026-04-11T09:00", &[]);
        assert_eq!(pair(out.at("block"), out.at("next.time")), strs("null", r#""10:30""#));
        let out = timeline("2026-04-11T19:00", &[]);
        assert_eq!(pair(out.at("block.time"), out.at("next")), strs(r#""18:40""#, "null"));
        assert!(out.until.is_empty());
    }

    #[test]
    fn a_day_with_options_runs_the_fixed_blocks_and_the_choice_in_time_order() {
        let out = timeline("2026-04-12T09:10", &[]);
        assert_eq!(out.each("day.blocks", "time"), "08:30 09:30 20:00");
        assert_eq!(
            pair(out.at("day.choice"), out.at("block.text")),
            strs(r#""coast""#, r#""Car""#)
        );
        assert_eq!(out.each("day.options", "id"), "coast hill");
        let out = timeline("2026-04-12T09:10", &[("choice.2026-04-12", "hill")]);
        assert_eq!(out.each("day.blocks", "time"), "08:30 09:00 20:00");
        assert_eq!(out.at("block.text"), r#""Train""#);
        let out = timeline("2026-04-12T09:10", &[("choice.2026-04-12", "gone")]);
        assert_eq!(out.at("day.choice"), r#""coast""#);
    }

    #[test]
    fn requires_keeps_the_options_the_answer_allows() {
        let choice = |store: &[(&str, &str)]| timeline("2026-04-13T09:00", store).at("day.choice");
        assert_eq!(choice(&[]), r#""beach""#);
        assert_eq!(choice(&[("choice.2026-04-12", "hill")]), r#""town""#);
        let both = [("choice.2026-04-12", "hill"), ("choice.2026-04-13", "beach")];
        assert_eq!(choice(&both), r#""town""#);
        assert_eq!(choice(&[("choice.2026-04-13", "rest")]), r#""rest""#);
        let content = "days:\n  - date: 2026-04-13\n    options: [{id: a, requires: {date: 2026-01-01, option: x}}]\n";
        let out = run("timeline", "", content, &world("2026-04-13T09:00", &[]), "");
        assert_eq!(pair(out.at("day.choice"), out.at("day.options")), strs("null", "[]"));
    }

    #[test]
    fn a_block_is_a_mapping_whatever_shape_it_came_in() {
        let out = timeline("2026-04-15T08:00", &[]);
        assert_eq!(out.at("block"), r#"{"time": "07:30", "text": "x"}"#);
        assert!(
            out.at("day.blocks").contains(r#"{"time": "07:00", "text": null, "state": "past"}"#)
        );
    }

    #[test]
    fn keys_come_out_canonical_through_the_keymap() {
        let manifest = "keymap:\n  root: {days: dias}\n  day: {date: fecha, title: titulo}\n  block: {place: lugar}\n";
        let content =
            "dias:\n  - {fecha: 2026-04-11, titulo: Hola, blocks: [['10:00', x, {lugar: p}]]}\n";
        let out = run("timeline", manifest, content, &world("2026-04-11T10:00", &[]), "");
        assert_eq!(
            pair(out.at("day.date"), out.at("day.title")),
            strs(r#""2026-04-11""#, r#""Hola""#)
        );
        assert_eq!(out.at("block.place"), r#""p""#);
    }

    #[test]
    fn today_has_its_blocks_stated_and_tomorrow_its_first_hour() {
        let out = timeline("2026-04-11T11:30", &[]);
        assert_eq!(out.each("day.blocks", "state"), "note past now locked");
        assert_eq!(out.at("day.started"), "true");
        assert_eq!(out.at("tomorrow.first.text"), r#""Car""#);
        let days = out.scope.get("days").and_then(Value::as_list).unwrap();
        let dates: Vec<String> = days.iter().map(|d| text(d.get("date"))).collect();
        assert_eq!(dates, ["2026-04-11", "2026-04-12", "2026-04-13", "2026-04-14", "2026-04-15"]);
        let out = timeline("2026-04-11T09:00", &[]);
        assert_eq!(out.each("day.blocks", "state"), "note next next locked");
        assert_eq!(out.at("day.started"), "false");
        let out = timeline("2026-04-13T09:00", &[]);
        assert_eq!(
            pair(out.at("tomorrow.first"), out.at("tomorrow.title")),
            strs("null", r#""Early question""#)
        );
        assert_eq!(timeline("2026-04-15T09:00", &[]).at("tomorrow"), "null");
    }

    #[test]
    fn a_block_type_comes_out_as_the_canonical_word() {
        let manifest = "keymap: {values: {type: {visit: visita}}}";
        let content = "days: [{date: 2026-04-11, blocks: [['10:00', x, {type: visita}], ['11:00', y, {type: meal}]]}]";
        let out = run("timeline", manifest, content, &world("2026-04-11T10:00", &[]), "");
        assert_eq!(out.each("day.blocks", "type"), "visit meal");
    }

    #[test]
    fn with_nothing_due_the_decision_is_the_next_one_coming() {
        let out = choices("2026-04-09T08:00", &[]);
        assert_eq!(
            pair(out.at("decision.date"), out.at("decision.due")),
            strs(r#""2026-04-14""#, "false")
        );
        assert_eq!(out.until, ["2026-04-11T21:00", "2026-04-12T00:00", "2026-04-09T12:00"]);
        // Once asked, a question stays due until its day is over.
        let out = choices("2026-04-10T08:00", &[]);
        assert_eq!(
            pair(out.at("decision.date"), out.at("decision.due")),
            strs(r#""2026-04-14""#, "true")
        );
        assert_eq!(choices("2026-04-16T08:00", &[]).at("decision"), "null");
        let none = run("choices", "", "", &world("2026-04-11T10:00", &[]), "");
        assert_eq!(none.at("decision"), "null");
    }

    #[test]
    fn the_first_due_and_unanswered_decision_wins() {
        let out = choices("2026-04-11T21:30", &[]);
        let fields = ["date", "title", "question", "recommended", "choice", "due", "answered"];
        let got: Vec<String> = fields.iter().map(|f| out.at(&format!("decision.{f}"))).collect();
        let want = [
            r#""2026-04-12""#,
            r#""Coast or hill""#,
            r#""Which?""#,
            r#""coast""#,
            r#""coast""#,
            "true",
            "false",
        ];
        assert_eq!(got, want);
        assert_eq!(out.each("decision.options", "id"), "coast hill");

        let out = choices("2026-04-12T08:00", &[("choice.2026-04-12", "hill")]);
        assert_eq!(out.at("decision.date"), r#""2026-04-13""#);
        assert_eq!(
            pair(out.at("decision.recommended"), out.at("decision.choice")),
            strs("null", r#""town""#)
        );

        let store = [
            ("choice.2026-04-12", "hill"),
            ("choice.2026-04-13", "rest"),
            ("choice.2026-04-14", "a"),
        ];
        let out = choices("2026-04-12T08:00", &store);
        assert_eq!(
            pair(out.at("decision.date"), out.at("decision.answered")),
            strs(r#""2026-04-12""#, "true")
        );
    }

    #[test]
    fn the_holder_is_the_person_holding_the_phone() {
        let content = "people: [{id: rita, nombre: Rita}, {id: tomas}, x]";
        let keymap = "keymap: {person: {name: nombre}}";
        let out = run("people", keymap, content, &world("2026-04-11T10:00", &[]), "");
        assert_eq!(out.at("holder"), r#"{"id": "rita", "name": "Rita"}"#);
        assert_eq!(out.at("people"), r#"[{"id": "rita", "name": "Rita"}, {"id": "tomas"}, {}]"#);
        let mut nobody = world("2026-04-11T10:00", &[]);
        nobody.holder = "ana".into();
        assert_eq!(run("people", "", content, &nobody, "").at("holder"), "null");
        // With no holder from the host, the one a relay stored holds the phone.
        let mut relayed = world("2026-04-11T10:00", &[("holder", "tomas")]);
        relayed.holder = String::new();
        assert_eq!(run("people", "", content, &relayed, "").at("holder"), r#"{"id": "tomas"}"#);
        // The host's holder wins over a stored one.
        let both = world("2026-04-11T10:00", &[("holder", "tomas")]);
        assert_eq!(run("people", keymap, content, &both, "").at("holder"), out.at("holder"));
    }

    #[test]
    fn the_place_comes_from_the_block_and_here_from_the_device() {
        let content = "places:\n  azulejo: {name: Museum, at: {lat: 1, lon: 1}}\n  cais: {name: Ferry}\n  odd: x\n";
        let w = world("2026-04-11T10:00", &[]);
        let out = run("places", "", content, &w, "block: {place: azulejo}");
        assert_eq!(
            out.at("place"),
            r#"{"name": "Museum", "at": {"lat": 1, "lon": 1}, "id": "azulejo"}"#
        );
        assert_eq!(out.at("here"), r#"{"name": "Ferry", "id": "cais"}"#);
        assert_eq!(out.regions, ["azulejo"]);
        let keymap =
            "keymap: {during: {hours: horario}, parking: {price: precio}, point: {name: nombre}}";
        let nested = "places:
  p: {during: {horario: x}, parking: {precio: y}, points: [{nombre: z}], at: 1}
";
        let out = run("places", keymap, nested, &w, "block: {place: p}");
        assert_eq!(
            out.at("place"),
            r#"{"during": {"hours": "x"}, "parking": {"price": "y"}, "points": [{"name": "z"}], "at": 1, "id": "p"}"#
        );
        let out = run("places", "", content, &w, "block: {place: odd}");
        assert_eq!(out.at("place"), r#"{"id": "odd"}"#);
        let out = run("places", "", "", &w, "");
        assert_eq!(pair(out.at("place"), out.at("here")), strs("null", "null"));
        assert!(out.regions.is_empty());
    }

    #[test]
    fn alerts_show_from_their_start_to_the_end_of_their_day_most_severe_first() {
        let content = "alerts:
  - {id: low_one, severity: baja, at: 2026-04-11}
  - {id: ferry, severity: high, at: 2026-04-11T18:40, notify_from: 2026-04-11T17:30}
  - {id: always, severity: odd}
  - {id: soon, severity: critical, notify_from: 2026-04-11}
  - {id: gone, severity: critical, at: 2026-04-10}
";
        let keymap = "keymap: {values: {severity: {low: baja}}}";
        let alerts = |now: &str| run("alerts", keymap, content, &world(now, &[]), "");
        let out = alerts("2026-04-11T18:00");
        assert_eq!(out.each("alerts", "id"), "soon ferry low_one always");
        assert_eq!(out.each("alerts", "severity"), "critical high low odd");
        assert!(out.until.is_empty());
        let out = alerts("2026-04-11T12:00");
        assert_eq!(out.each("alerts", "id"), "soon low_one always");
        assert_eq!(out.until, ["2026-04-11T17:30"]);
        let out = alerts("2026-04-10T12:00");
        assert_eq!(out.each("alerts", "id"), "gone always");
        assert_eq!(out.until, ["2026-04-11T00:00", "2026-04-11T17:30", "2026-04-11T00:00"]);
    }

    #[test]
    fn documents_come_out_canonical() {
        let content = "documents: [{id: a, titulo: T}]";
        let w = world("2026-04-11T10:00", &[]);
        let out = run("documents", "keymap: {document: {title: titulo}}", content, &w, "");
        assert_eq!(out.at("documents"), r#"[{"id": "a", "title": "T"}]"#);
    }

    #[test]
    fn the_most_specific_weather_wins_and_the_rest_falls_through() {
        let content = r#"climate:
  entries:
    - {month: 4, high: 20, low: 12, summary: Spring}
    - {place: azulejo, month: 4, rain: 35}
    - {date: 2026-04-11, high: 23}
    - {place: cais, date: 2026-04-11, summary: Windy}
    - {month: 5, high: 30}
    - {date: 2026-04-12, high: 1}
    - {place: other, high: 99}
    - {low: 5, sunrise: "06:55"}
"#;
        let w = world("2026-04-11T10:00", &[]);
        let out = run("climate", "", content, &w, "place: {id: azulejo}");
        assert_eq!(
            out.at("weather"),
            r#"{"high": 23, "low": 12, "rain": 35, "sunrise": "06:55", "sunset": null, "summary": "Spring", "units": "metric", "as_of": null}"#
        );
        let out = run("climate", "", content, &w, "place: null\nhere: {id: cais}");
        assert_eq!(
            pair(out.at("weather.summary"), out.at("weather.rain")),
            strs(r#""Windy""#, "null")
        );
        let imperial = "climate: {units: imperial, entries: [{high: 70}]}";
        assert_eq!(run("climate", "", imperial, &w, "").at("weather.units"), r#""imperial""#);
        let may = "climate: {entries: [{month: 5}]}";
        assert_eq!(run("climate", "", may, &w, "").at("weather"), "null");
        assert_eq!(run("climate", "", "", &w, "").at("weather"), "null");
    }
}
