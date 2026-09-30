//! What each module puts in the scope for one world, and the instants and regions at which that
//! would change. Every module hands out canonical keys, whatever the pack calls them.

mod calendar;
mod chart;
pub(crate) mod plan;

use std::cmp::Reverse;

use crate::clock::{minutes, next_day, shift};
use crate::define::{Module, key};
use crate::engine::{Region, World};
use crate::validate::patterns::{is_real_date, is_stamp};
use crate::validate::{Keymap, SEVERITIES};
use crate::value::{Map, Value, text, truthy};

/// One run of the modules over one world.
pub(crate) struct Run<'a> {
    keymap: Keymap<'a>,
    content: &'a Map,
    world: &'a World,
    date: &'a str,
    pub scope: Map,
    /// Instants at which something exposed would change. Any of them may be past.
    pub until: Vec<String>,
    pub regions: Vec<Region>,
    /// The top level content keys the modules so far have read.
    used: Vec<String>,
}

fn list(data: Option<&Value>) -> &[Value] {
    data.and_then(Value::as_list).unwrap_or_default()
}

fn id(v: &Value) -> String {
    text(v.get("id"))
}

/// Whether a block is for the person holding the phone: its `for` names them, one id or a list,
/// or it has no `for`, or nobody holds the phone.
fn holds(block: &Map, holder: &str) -> bool {
    let Some(whom) = block.get("for") else {
        return true;
    };
    holder.is_empty()
        || match whom {
            Value::List(ids) => ids.iter().any(|p| text(Some(p)) == holder),
            one => text(Some(one)) == holder,
        }
}

/// The circle of a place's `at`, when it has both coordinates. The radius defaults to 100 metres.
fn region(id: &str, at: &Value) -> Option<Region> {
    let number = |k: &str| match at.get(k) {
        Some(Value::Number(n)) => Some(*n),
        _ => None,
    };
    let (lat, lon) = (number("lat")?, number("lon")?);
    Some(Region { id: id.to_owned(), lat, lon, radius_m: number("radius_m").unwrap_or(100.0) })
}

impl<'a> Run<'a> {
    pub fn new(keymap: Keymap<'a>, content: &'a Map, world: &'a World, scope: Map) -> Self {
        let date = &world.now[..10];
        let (until, regions, used) = (Vec::new(), Vec::new(), Vec::new());
        Run { keymap, content, world, date, scope, until, regions, used }
    }

    pub fn module(&mut self, m: &Module) {
        let data = m.from.as_deref().and_then(|from| self.keymap.root(self.content, from));
        if let Some(from) = m.from.as_deref() {
            self.used.push(self.keymap.root_key(from));
        }
        match m.name {
            "timeline" => self.timeline(data),
            "choices" => self.choices(data),
            "people" => self.people(data),
            "places" => self.places(data),
            "alerts" => self.alerts(data),
            "documents" => self.documents(data),
            "sheets" => self.sheets(data),
            _ => self.climate(data, m.sync.is_some()),
        }
    }

    fn set(&mut self, name: &str, value: Value) {
        self.scope.set(name, value);
    }

    /// The local time now in `zone`, when the host passed it; `None` for the pack's own zone.
    fn local(&self, zone: &str) -> Option<String> {
        Some(text(self.world.zones.get(zone))).filter(|l| is_stamp(l) && is_real_date(&l[..10]))
    }

    /// The pack's stamp for a local `date` and `time` in `zone`, moved by how far that zone's
    /// clock is from the pack's. A zone the host did not pass counts as the pack's.
    fn stamp(&self, date: &str, time: &str, zone: &str) -> String {
        let at = format!("{date}T{time}");
        match self.local(zone) {
            Some(local) => shift(&at, minutes(&self.world.now) - minutes(&local)),
            None => at,
        }
    }

    /// When a block of the day on `date` begins, as a stamp of the pack's zone.
    fn begins(&self, date: &str, block: &Map) -> String {
        self.stamp(date, &text(block.get("time")), &text(block.get("zone")))
    }

    /// When a block with an `until` ends: `{date}T{until}` and its zone, the next date when that
    /// reads before the block begins, as a flight landing the next day does.
    fn ends(&self, date: &str, block: &Map) -> (String, String) {
        let until = text(block.get("until"));
        let own = Some(text(block.get("until_zone"))).filter(|z| !z.is_empty());
        let zone = own.unwrap_or_else(|| text(block.get("zone")));
        let on = if self.stamp(date, &until, &zone) <= self.begins(date, block) {
            next_day(date)
        } else {
            date.to_owned()
        };
        (format!("{on}T{until}"), zone)
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
        // What the road passes, each stretch through the `road` context (decision 0026).
        if let Some(road) = m.get("road").and_then(Value::as_list) {
            let road: Vec<Value> = road.iter().map(|r| self.keymap.canon(r, "road")).collect();
            m.set("road", Value::List(road));
        }
        m
    }

    /// A block of today with its `state` for the agenda: a note has no time, `now` is the current
    /// block, `past` one whose time has come, and one to come is `locked` or `next`.
    fn stated(&self, date: &str, block: &Value, current: &Value) -> Value {
        let mut m = block.as_map().cloned().unwrap_or_default();
        let time = text(m.get("time"));
        let state = if time.is_empty() {
            "note"
        } else if block == current {
            "now"
        } else if self.begins(date, &m) <= self.world.now {
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
    /// A timed block also carries `event`, its calendar id: `{date}.{list}.{n}`, where the list
    /// is `blocks`, `fixed` or `option-{id}` and `n` its place in that list. A block with no
    /// `zone` of its own takes the day's.
    fn day(&self, days: &[Value], day: &Value) -> (Map, Vec<Map>) {
        let (kept, chosen) = self.options(days, day, true);
        let date = self.date_of(day);
        let zone = self.read(day, "day", "zone").filter(|z| truthy(Some(z)));
        let holder = self.holder_id();
        let mut blocks: Vec<Map> = Vec::new();
        let mut add = |name: &str, from: Option<&Value>| {
            for (n, b) in list(from).iter().enumerate() {
                let mut b = self.block(b);
                if !text(b.get("time")).is_empty() {
                    b.set("event", Value::String(format!("{date}.{name}.{n}")));
                }
                if let Some(zone) = zone.filter(|_| b.get("zone").is_none()) {
                    b.set("zone", zone.clone());
                }
                if holds(&b, &holder) {
                    blocks.push(b);
                }
            }
        };
        for key in ["blocks", "fixed"] {
            add(key, self.read(day, "day", key));
        }
        if let Some(option) = chosen {
            add(&format!("option-{}", id(option)), self.read(option, "option", "blocks"));
            blocks.sort_by_key(|b| text(b.get("time")));
        }
        // What the person did to the day on the device: blocks added, moved, resized or dropped.
        let extra = plan::added(&self.world.store, &date);
        let touched = !extra.is_empty();
        blocks.extend(extra);
        if plan::replan(&self.world.store, &mut blocks) || touched {
            blocks.sort_by_key(|b| text(b.get("time")));
        }
        let mut m = self.canon(day, "day");
        m.set("blocks", Value::List(blocks.iter().cloned().map(Value::Map).collect()));
        if self.read(day, "day", "options").is_some() {
            m.set("options", self.option_list(&kept));
        }
        m.set("choice", chosen.map_or(Value::Null, |c| Value::String(id(c))));
        blocks.retain(|b| !text(b.get("time")).is_empty());
        (m, blocks)
    }

    /// `day` is today in its own `zone`, with `time` the local time there and `started` once a
    /// block's time has come; `block` the last one whose time has come, until its `until` (in its
    /// `until_zone`, else its `zone`); `next` the first one still to come. `days` is every day and
    /// `tomorrow` the day after this one, with its `first` timed block.
    fn timeline(&mut self, data: Option<&Value>) {
        let days = list(data);
        let now = self.world.now.clone();
        let (mut day, mut block, mut next) = (Value::Null, Value::Null, Value::Null);
        let local = |d: &Value| {
            let zone = text(self.read(d, "day", "zone"));
            self.local(&zone).unwrap_or_else(|| now.clone())
        };
        let today = days.iter().find(|d| self.date_of(d) == local(d)[..10]);
        let date = today.map_or_else(|| self.date.to_owned(), |t| self.date_of(t));
        let here = today.map(local).unwrap_or_default();
        // Another zone's midnight can make another day today.
        let midnights: Vec<String> = (self.world.zones.iter())
            .filter_map(|(z, _)| self.local(z))
            .map(|l| shift(&format!("{}T00:00", next_day(&l[..10])), minutes(&now) - minutes(&l)))
            .collect();
        self.until.extend(midnights);
        if let Some(today) = today {
            let (mut map, timed) = self.day(days, today);
            let started = timed.iter().any(|b| self.begins(&date, b) <= now);
            let mut current = None;
            for mut b in timed {
                let at = self.begins(&date, &b);
                if at <= now {
                    current = Some(b);
                } else if next == Value::Null {
                    self.until.push(at.clone());
                    // The set-off notice costs one timer, and its guard reads the pack's clock.
                    let off = plan::whole(b.get("leave"));
                    if off > 0 {
                        let set_off = shift(&at, -off);
                        self.until.push(set_off.clone());
                        let local = shift(&format!("{date}T{}", text(b.get("time"))), -off)[11..]
                            .to_owned();
                        b.set("leave_at", Value::String(local));
                        b.set("leaving", Value::Bool(set_off <= now));
                    }
                    next = Value::Map(b);
                }
            }
            if let Some(b) = current {
                let until = text(b.get("until"));
                let (end, zone) = self.ends(&date, &b);
                let ends = self.stamp(&end[..10], &until, &zone);
                if until.is_empty() || ends > now {
                    if !until.is_empty() {
                        self.until.push(ends);
                    }
                    block = Value::Map(b);
                }
            }
            let blocks = list(map.get("blocks")).iter().map(|b| self.stated(&date, b, &block));
            map.set("blocks", Value::List(blocks.collect()));
            map.set("started", Value::Bool(started));
            map.set("time", Value::String(here[11..].to_owned()));
            day = Value::Map(map);
        }
        let date = next_day(&date);
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
    /// `holder` by a relay. `people` is everyone. A stored `holder_until` stamp is watched, so
    /// the rules decide again when a handoff's extra time runs out.
    fn people(&mut self, data: Option<&Value>) {
        let until = text(self.world.store.get("holder_until"));
        if is_stamp(&until) && is_real_date(&until[..10]) {
            self.until.push(until);
        }
        let people: Vec<Map> = list(data).iter().map(|p| self.canon(p, "person")).collect();
        let id = self.holder_id();
        let holder = people.iter().find(|p| text(p.get("id")) == id);
        let holder = holder.cloned().map_or(Value::Null, Value::Map);
        self.set("holder", holder);
        self.set("people", Value::List(people.into_iter().map(Value::Map).collect()));
    }

    /// The id of the person holding the phone: the host's, else the one a relay stored.
    fn holder_id(&self) -> String {
        let stored = || text(self.world.store.get("holder"));
        Some(self.world.holder.clone()).filter(|h| !h.is_empty()).unwrap_or_else(stored)
    }

    /// `place` is where the current block happens, `here` the place the device is inside: the
    /// block's own when it is one of them, else the first. `away` is true when the device is
    /// located and out of the block's region. Every place with coordinates is a region to watch.
    fn places(&mut self, data: Option<&Value>) {
        let places = data.and_then(Value::as_map);
        let find = |id: &str| {
            let place = places?.get(id)?;
            let mut m = self.canon(place, "place");
            for kind in ["during", "parking", "plan"] {
                if let Some(v) = m.get(kind).map(|v| self.keymap.canon(v, kind)) {
                    m.set(kind, v);
                }
            }
            self.pointed(&mut m);
            // A terminal map is a plan of its own, with pins in the picture's coordinates.
            if let Some(plan) = m.get("plan").and_then(Value::as_map) {
                let mut plan = plan.clone();
                self.pointed(&mut plan);
                m.set("plan", Value::Map(plan));
            }
            m.set("id", Value::String(id.to_owned()));
            Some(Value::Map(m))
        };
        let regions: Vec<Region> = places
            .map(Map::iter)
            .into_iter()
            .flatten()
            .filter_map(|(id, p)| region(id, self.read(p, "place", "at")?))
            .collect();
        let at = text(self.scope.get("block").and_then(|b| b.get("place")));
        let inside = &self.world.inside;
        let own = inside.contains(&at).then_some(&at);
        let here = own.into_iter().chain(inside).find_map(|id| find(id)).unwrap_or(Value::Null);
        let fenced = regions.iter().any(|r| r.id == at);
        let away = self.world.located && fenced && !inside.contains(&at);
        let place = find(&at).unwrap_or(Value::Null);
        self.regions.extend(regions);
        self.set("place", place);
        self.set("here", here);
        self.set("away", Value::Bool(away));
        // The day's chart: the places its blocks name, in visit order, on plain paper.
        let blocks = self.scope.get("day").and_then(|d| d.get("blocks"));
        let stops: Vec<chart::Pin> = blocks
            .and_then(Value::as_list)
            .unwrap_or_default()
            .iter()
            .filter_map(|b| {
                let id = text(b.get("place"));
                let raw = places?.get(&id)?;
                let circle = region(&id, self.read(raw, "place", "at")?)?;
                let name = text(self.read(raw, "place", "name"));
                let state = if id == at {
                    "now"
                } else if inside.contains(&id) {
                    "here"
                } else {
                    ""
                };
                Some(chart::Pin { id, name, lat: circle.lat, lon: circle.lon, state })
            })
            .collect();
        self.set("chart", chart::chart(&stops));
    }

    /// The `points` of a mapping, each through the `point` context.
    fn pointed(&self, m: &mut Map) {
        if let Some(l) = m.get("points").and_then(Value::as_list) {
            let points: Vec<Value> = l.iter().map(|p| self.keymap.canon(p, "point")).collect();
            m.set("points", Value::List(points));
        }
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

    /// `sheets` is every entry of the sheets root as `{id, title, value}`; with no such root, every
    /// top level key of the content that no module read.
    fn sheets(&mut self, data: Option<&Value>) {
        let entries = match data {
            Some(own) => own.as_map().map(Map::iter).into_iter().flatten().collect::<Vec<_>>(),
            None => {
                self.content.iter().filter(|(k, _)| !self.used.iter().any(|r| r == k)).collect()
            }
        };
        let store = &self.world.store;
        // The `items` of a checked sheet, each with the `fact` that ticks it, named by its place.
        let ticks = |id: &str, value: &Value| {
            let tick = |(n, item): (usize, &Value)| {
                let key = format!("tick.{id}.{n}");
                record([
                    ("fact", Value::String(key.clone())),
                    ("text", Value::String(text(Some(item)))),
                    ("done", Value::Bool(truthy(store.get(&key)))),
                ])
            };
            let items = value.get("items").and_then(Value::as_list).unwrap_or_default();
            Value::List(items.iter().enumerate().map(tick).collect())
        };
        // `check` and `items` stay out of `value`, so the unknown key rule does not draw them.
        let sheet = |(key, value): (&str, &Value)| {
            let mut m = Map::default();
            m.set("id", Value::String(key.to_owned()));
            m.set("title", Value::String(key.replace('_', " ")));
            match value.as_map().filter(|_| value.get("check") == Some(&Value::Bool(true))) {
                Some(own) => {
                    let rest = own.0.iter().filter(|(k, _)| !TICKED.contains(&k.as_str()));
                    m.set("value", Value::Map(Map(rest.cloned().collect())));
                    m.set("ticks", ticks(key, value));
                }
                None => m.set("value", value.clone()),
            }
            Value::Map(m)
        };
        let sheets = Value::List(entries.into_iter().map(sheet).collect());
        self.set("sheets", sheets);
    }

    /// Every document, with `person` the name of the one it is `for` and `call` as
    /// `[{label, number}]`. None while a child holds the phone: they carry ids of real people.
    fn documents(&mut self, data: Option<&Value>) {
        if self.scope.get("holder").and_then(|h| h.get("adult")) == Some(&Value::Bool(false)) {
            self.set("documents", Value::List(Vec::new()));
            return;
        }
        // Everyone as the people module read them, wherever its `from` points.
        let people = list(self.scope.get("people"));
        let documents = list(data).iter().map(|d| {
            let mut m = self.canon(d, "document");
            let whose = text(m.get("for"));
            let person = people.iter().find(|p| text(p.get("id")) == whose);
            if let Some(name) = person.and_then(|p| p.get("name")) {
                m.set("person", name.clone());
            }
            if let Some(Value::Map(numbers)) = m.get("call") {
                let entry = |(label, number): (&str, &Value)| {
                    record([("label", Value::String(label.to_owned())), ("number", number.clone())])
                };
                let call = numbers.iter().map(entry).collect();
                m.set("call", Value::List(call));
            }
            Value::Map(m)
        });
        let documents = documents.collect();
        self.set("documents", Value::List(documents));
    }

    /// The weather for today at `place`, else `here`: the most specific entry wins, and a key it
    /// lacks falls through to the next one that matches. Synced rows come before the pack's, so
    /// they win where they cover and the pack fills the rest.
    fn climate(&mut self, data: Option<&Value>, syncs: bool) {
        let id_of = |name: &str| text(self.scope.get(name).and_then(|p| p.get("id")));
        let place = Some(id_of("place")).filter(|p| !p.is_empty()).unwrap_or_else(|| id_of("here"));
        let month: f64 = self.date[5..7].parse().unwrap_or_default();
        let synced = self.world.store.get(&key("climate"));
        let rows = list(synced.and_then(|s| s.get("rows")));
        let rows = rows.iter().map(|r| r.as_map().cloned().unwrap_or_default());
        let entries = list(data.and_then(|c| c.get("entries")));
        let entries = rows.chain(entries.iter().map(|e| self.canon(e, "climate")));
        let mut ranked: Vec<(u8, Map)> = entries
            .filter_map(|e| {
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
        // A module that syncs always has weather, so a screen can offer the sync.
        let weather = if ranked.is_empty() && !syncs {
            Value::Null
        } else {
            let pick = |key: &str| {
                let v = ranked.iter().find_map(|(_, e)| e.get(key)).cloned();
                match v {
                    // A reply may give a sunrise as a stamp; the day is today's anyway.
                    Some(Value::String(s)) if is_stamp(&s) => Value::String(s[11..].to_owned()),
                    v => v.unwrap_or(Value::Null),
                }
            };
            let keys = ["high", "low", "rain", "sunrise", "sunset", "summary"];
            let mut m = Map(keys.iter().map(|k| ((*k).to_owned(), pick(k))).collect());
            let units = data.and_then(|c| c.get("units")).cloned();
            m.set("units", units.unwrap_or_else(|| Value::String("metric".into())));
            let state = |key: &str| synced.and_then(|s| s.get(key)).cloned().unwrap_or(Value::Null);
            m.set("as_of", state("as_of"));
            m.set("failed", state("failed"));
            m.set("syncs", Value::Bool(syncs));
            Value::Map(m)
        };
        self.set("weather", weather);
    }
}

/// The keys a checked sheet keeps to itself: the opt-in and the list it ticks off.
const TICKED: [&str; 2] = ["check", "items"];

/// A mapping of these fields, in this order.
fn record<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Map(Map(fields.map(|(k, v)| (k.to_owned(), v)).to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::define::{Fetch, MODULES};
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
            located: true,
            store: Map(store.collect()),
            zones: Map::default(),
            can: vec![],
        }
    }

    struct Out {
        scope: Map,
        until: Vec<String>,
        regions: Vec<Region>,
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
        let (name, from, _) = *MODULES.iter().find(|(n, ..)| *n == module).unwrap();
        let content = content.as_map().cloned().unwrap_or_default();
        let mut r = Run::new(Keymap::of(&manifest), &content, world, scope);
        r.module(&Module { name, from: Some(from.to_owned()), sync: None });
        Out { scope: r.scope, until: r.until, regions: r.regions }
    }

    fn timeline(now: &str, store: &[(&str, &str)]) -> Out {
        run("timeline", "", DAYS, &world(now, store), "")
    }

    fn choices(now: &str, store: &[(&str, &str)]) -> Out {
        run("choices", "", DAYS, &world(now, store), "")
    }

    /// A world whose stored facts are whole values, as a timeline edit writes them.
    fn planned(now: &str, store: &str) -> World {
        World {
            store: parse(store).unwrap().as_map().cloned().unwrap_or_default(),
            ..world(now, &[])
        }
    }

    /// One block you have to set off for.
    const LEAVE: &str = "days: [{date: 2026-04-11, blocks: [[\"10:30\", Ferry, {leave: 20}]]}]\n";

    /// One you set off for before its own midnight.
    const REDEYE: &str = "days: [{date: 2026-04-11, blocks: [[\"00:10\", Flight, {leave: 20}]]}]\n";

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

    /// The sheets after running `modules` over `content`, as `id=title` pairs.
    fn sheets(manifest: &str, content: &str, modules: &[&str]) -> Out {
        let (manifest, content) = (parse(manifest).unwrap(), parse(content).unwrap());
        let content = content.as_map().cloned().unwrap_or_default();
        let w = world("2026-04-11T10:00", &[("tick.packing.0", "yes")]);
        let mut r = Run::new(Keymap::of(&manifest), &content, &w, Map::default());
        for module in modules {
            let (name, from, _) = *MODULES.iter().find(|(n, ..)| n == module).unwrap();
            r.module(&Module { name, from: Some(from.to_owned()), sync: None });
        }
        Out { scope: r.scope, until: r.until, regions: r.regions }
    }

    #[test]
    fn with_no_sheets_root_every_key_no_module_read_is_a_sheet() {
        let content = "trip: {days: [], notes: x}\nplaces: {}\nphrase_book: [a, b]\nbudget: 3\n";
        let keymap = "keymap: {root: {days: trip.days}}";
        let out = sheets(keymap, content, &["timeline", "places", "sheets"]);
        assert_eq!(out.each("sheets", "id"), "phrase_book budget");
        assert_eq!(out.each("sheets", "title"), "phrase book budget");
        let none = sheets("", "days: []\n", &["timeline", "sheets"]);
        assert_eq!(none.at("sheets"), "[]");
    }

    #[test]
    fn a_sheets_root_lists_only_its_own_entries() {
        let content = "sheets: {bookings: [a], phrases: {hi: hola}}\nbudget: 3\n";
        let out = sheets("", content, &["sheets"]);
        assert_eq!(out.each("sheets", "id"), "bookings phrases");
        let odd = sheets("", "sheets: [x]\n", &["sheets"]);
        assert_eq!(odd.at("sheets"), "[]");
    }

    #[test]
    fn a_day_carries_the_blocks_the_person_added_moved_and_dropped() {
        let added = "added.2026-04-11: [{time: \"09:00\", text: Coffee}]";
        let one = run("timeline", "", DAYS, &planned("2026-04-11T08:00", added), "");
        assert_eq!(one.each("day.blocks", "text"), "A note Coffee Walk Museum Ferry");
        let store = format!(
            "{added}\nplan.2026-04-11.blocks.1: {{shift: 60}}\nplan.2026-04-11.blocks.3: {{off: true}}"
        );
        let out = run("timeline", "", DAYS, &planned("2026-04-11T08:00", &store), "");
        assert_eq!(out.each("day.blocks", "text"), "A note Coffee Museum Walk");
    }

    #[test]
    fn a_block_you_set_off_for_says_when_and_the_watch_waits_for_that_minute() {
        // Without `leave` a block says nothing about setting off.
        assert_eq!(timeline("2026-04-11T09:00", &[]).at("next.leave_at"), "nothing");
        let early = run("timeline", "", LEAVE, &world("2026-04-11T09:00", &[]), "");
        assert_eq!([early.at("next.leave_at"), early.at("next.leaving")], ["\"10:10\"", "false"]);
        assert!(early.until.contains(&"2026-04-11T10:10".to_owned()), "{:?}", early.until);
        let now = run("timeline", "", LEAVE, &world("2026-04-11T10:15", &[]), "");
        assert_eq!(now.at("next.leaving"), "true");
        // The hour to go crosses back over midnight rather than stopping at it.
        let red = run("timeline", "", REDEYE, &world("2026-04-11T00:00", &[]), "");
        assert_eq!([red.at("next.leave_at"), red.at("next.leaving")], ["\"23:50\"", "true"]);
    }

    #[test]
    fn a_sheet_asking_to_be_checked_lists_its_items_with_what_is_done() {
        let content = "sheets: {packing: {check: true, items: [Hat, Bottle], note: Two}, phrases: {hi: hola}}";
        let rows = concat!(
            r#"[{"id": "packing", "title": "packing", "value": {"note": "Two"}, "ticks": ["#,
            r#"{"fact": "tick.packing.0", "text": "Hat", "done": true}, "#,
            r#"{"fact": "tick.packing.1", "text": "Bottle", "done": false}]}, "#,
            // A sheet that did not ask has no list of its own, and keeps every key it wrote.
            r#"{"id": "phrases", "title": "phrases", "value": {"hi": "hola"}}]"#
        );
        assert_eq!(sheets("", content, &["sheets"]).at("sheets"), rows);
        // Asking with nothing to tick off is an empty list, not a card of leftover keys.
        let bare = sheets("", "sheets: {packing: {check: true}}", &["sheets"]);
        assert_eq!(
            bare.at("sheets"),
            r#"[{"id": "packing", "title": "packing", "value": {}, "ticks": []}]"#
        );
    }

    #[test]
    fn a_block_for_somebody_else_is_left_out_of_the_day() {
        let content = r#"days:
  - date: 2026-04-11
    blocks:
      - ["09:00", "Talk", {for: [tomas]}]
      - ["09:30", "Swim", {for: tomas}]
      - ["10:00", "Walk", {for: [rita, tomas]}]
      - ["11:00", "Lunch"]
"#;
        let mut w = world("2026-04-11T09:45", &[]);
        let rita = run("timeline", "", content, &w, "");
        assert_eq!(rita.each("day.blocks", "text"), "Walk Lunch");
        assert_eq!(rita.at("block"), "null");
        w.holder = String::new();
        let anyone = run("timeline", "", content, &w, "");
        assert_eq!(anyone.each("day.blocks", "text"), "Talk Swim Walk Lunch");
        assert_eq!(anyone.at("block.text"), "\"Swim\"");
    }

    #[test]
    fn a_day_in_another_zone_runs_on_that_zones_clock() {
        let content = r#"days:
  - date: 2026-10-02
    zone: Asia/Tokyo
    title: Home
  - date: 2026-10-03
    zone: Asia/Tokyo
    blocks:
      - ["01:00", "Land", {until: "01:30", until_zone: Europe/Lisbon}]
      - ["03:00", "Train", {zone: Bad/Zone}]
"#;
        let mut w = world("2026-10-02T19:00", &[]);
        let zones = [("Asia/Tokyo", "2026-10-03T02:00"), ("Europe/Lisbon", "2026-10-02T18:00")];
        let zones = zones.iter().map(|(z, t)| ((*z).to_owned(), Value::String((*t).to_owned())));
        w.zones =
            Map(zones.chain([("Bad/Zone".to_owned(), Value::String("later".into()))]).collect());
        let out = run("timeline", "", content, &w, "");
        assert_eq!(out.at("day.date"), "\"2026-10-03\"");
        assert_eq!(pair(out.at("day.time"), out.at("day.started")), strs("\"02:00\"", "true"));
        assert_eq!(
            pair(out.at("block.text"), out.at("block.zone")),
            strs("\"Land\"", "\"Asia/Tokyo\"")
        );
        assert_eq!(
            pair(out.at("next.text"), out.at("next.zone")),
            strs("\"Train\"", "\"Bad/Zone\"")
        );
        // Tokyo's and Lisbon's next midnights come first, then the next block and the end of this one.
        let until =
            ["2026-10-03T17:00", "2026-10-03T01:00", "2026-10-03T03:00", "2026-10-03T02:30"];
        assert_eq!(out.until, until);
        assert_eq!(out.each("day.blocks", "state"), "now next");
        assert_eq!(out.at("tomorrow"), "null");
    }

    #[test]
    fn a_block_whose_until_reads_before_it_ends_the_next_day() {
        let content = r#"days:
  - date: 2026-10-02
    blocks:
      - ["22:00", "Fly", {until: "01:00", until_zone: Europe/Lisbon}]
"#;
        let out = run("timeline", "", content, &world("2026-10-02T23:30", &[]), "");
        assert_eq!(out.at("block.text"), "\"Fly\"");
        assert_eq!(out.until, ["2026-10-03T01:00"]);
    }

    #[test]
    fn the_block_is_the_last_one_begun_until_it_ends() {
        let out = timeline("2026-04-11T11:30", &[]);
        assert_eq!(
            out.at("block"),
            r#"{"time": "11:15", "text": "Museum", "type": "visit", "place": "azulejo", "until": "13:00", "event": "2026-04-11.blocks.2"}"#
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
        assert_eq!(
            out.at("block"),
            r#"{"time": "07:30", "text": "x", "event": "2026-04-15.blocks.1"}"#
        );
        assert!(out.at("day.blocks").contains(
            r#"{"time": "07:00", "text": null, "event": "2026-04-15.blocks.0", "state": "past"}"#
        ));
    }

    #[test]
    fn keys_come_out_canonical_through_the_keymap() {
        let manifest = "keymap:\n  root: {days: dias}\n  day: {date: fecha, title: titulo}\n  block: {place: lugar}\n  road: {name: nombre}\n";
        let content = "dias:
  - fecha: 2026-04-11
    titulo: Hola
    blocks: [['10:00', x, {lugar: p, road: [{nombre: El puente, what: Mira}]}]]
";
        let out = run("timeline", manifest, content, &world("2026-04-11T10:00", &[]), "");
        assert_eq!(
            pair(out.at("day.date"), out.at("day.title")),
            strs(r#""2026-04-11""#, r#""Hola""#)
        );
        assert_eq!(out.at("block.place"), r#""p""#);
        assert_eq!(out.at("block.road"), r#"[{"name": "El puente", "what": "Mira"}]"#);
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
        assert!(out.until.is_empty());
    }

    #[test]
    fn a_stored_holder_until_is_watched_when_it_is_a_real_stamp() {
        let until = |stamp: &str| {
            let w = world("2026-04-11T10:00", &[("holder_until", stamp)]);
            run("people", "", "people: []", &w, "").until
        };
        assert_eq!(until("2026-04-11T10:15"), ["2026-04-11T10:15"]);
        assert!(until("2026-02-30T10:15").is_empty());
        assert!(until("10:15").is_empty());
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
        let museum = Region { id: "azulejo".into(), lat: 1.0, lon: 1.0, radius_m: 100.0 };
        assert_eq!(out.regions, [museum]);
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
        assert!(out.regions.is_empty());
        let out = run("places", "", content, &w, "block: {place: odd}");
        assert_eq!(out.at("place"), r#"{"id": "odd"}"#);
        // A pack with no places at all: nothing to be at, and no chart of the day either.
        let out = run("places", "", "", &w, "day: {blocks: [{place: gone}]}");
        assert_eq!(pair(out.at("place"), out.at("here")), strs("null", "null"));
        assert!(out.regions.is_empty());
        assert_eq!(out.at("chart"), "null");
    }

    #[test]
    fn the_day_is_charted_from_the_places_its_blocks_name() {
        let keymap = "keymap: {plan: {image: imagen}, point: {name: nombre}}";
        let content = "places:
  azulejo: {name: Museum, at: {lat: 38.7, lon: -9.1}}
  cais: {name: Ferry, at: {lat: 38.71, lon: -9.14}, plan: {imagen: hall.png, points: [{nombre: Gate, x: 0.5, y: 0.5}]}}
  odd: {name: Nowhere}
  half: {name: Half, at: {lat: 1}}
";
        let scope = "block: {place: azulejo}
day: {blocks: [{place: azulejo}, {place: odd}, {place: half}, {place: cais}, {place: azulejo}]}";
        let out = run("places", keymap, content, &world("2026-04-11T10:00", &[]), scope);
        assert_eq!(out.each("chart.points", "name"), "Museum Ferry");
        assert_eq!(out.each("chart.points", "state"), "now here");
        // Neither a place without coordinates nor one with half of them is a pin, and the place
        // visited twice is one pin and two stops.
        assert_eq!(out.at("chart.path").matches("\"x\"").count(), 3);
        // The device is inside cais, so that is `here`, and its own map comes out canonical.
        assert_eq!(
            out.at("here.plan"),
            r#"{"image": "hall.png", "points": [{"name": "Gate", "x": 0.5, "y": 0.5}]}"#
        );
    }

    #[test]
    fn here_prefers_the_block_place_and_away_needs_a_fix_outside_its_region() {
        let content = "places:
  a: {at: {lat: 1, lon: 2, radius_m: 300}}
  b: {at: {lat: 1, lon: 3}}
  c: {name: no circle}
  d: {at: {lat: 1}}
";
        let away = |inside: &[&str], located: bool, block: &str| {
            let inside = inside.iter().map(|i| (*i).to_owned()).collect();
            let w = World { inside, located, ..world("2026-04-11T10:00", &[]) };
            let out = run("places", "", content, &w, &format!("block: {{place: {block}}}"));
            (text(out.scope.get("here").and_then(|h| h.get("id"))), out.at("away"))
        };
        assert_eq!(away(&["a", "b"], true, "b"), strs("b", "false"));
        assert_eq!(away(&["a"], true, "b"), strs("a", "true"));
        assert_eq!(away(&[], false, "b"), strs("", "false"));
        assert_eq!(away(&[], true, "c"), strs("", "false"));
        let w = world("2026-04-11T10:00", &[]);
        let out = run("places", "", content, &w, "");
        assert_eq!(out.regions[0].radius_m, 300.0);
        assert_eq!(out.regions.len(), 2);
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
    fn a_child_holding_the_phone_gets_no_documents() {
        let content = "documents: [{id: a, title: T}]";
        let w = world("2026-04-11T10:00", &[]);
        let out = run("documents", "", content, &w, "holder: {id: tomas, adult: false}");
        assert_eq!(out.at("documents"), "[]");
        let out = run("documents", "", content, &w, "holder: {id: rita, adult: true}");
        assert_eq!(out.at("documents"), r#"[{"id": "a", "title": "T"}]"#);
    }

    #[test]
    fn a_document_names_its_person_and_lists_its_numbers() {
        // The people come as the people module read them, not from the pack's `people` root.
        let content = "people: [{id: rita, name: Wrong}]
documents:
  - {id: a, for: rita, call: {Desk: '+1 555', Help: '112'}}
  - {id: b, for: tomas}
  - {id: c, for: nobody, call: '112'}";
        let w = world("2026-04-11T10:00", &[]);
        let people = "people: [{id: rita, name: Rita}, {id: tomas}]";
        let out = run("documents", "", content, &w, people);
        assert_eq!(
            out.at("documents"),
            r#"[{"id": "a", "for": "rita", "call": [{"label": "Desk", "number": "+1 555"}, {"label": "Help", "number": "112"}], "person": "Rita"}, {"id": "b", "for": "tomas"}, {"id": "c", "for": "nobody", "call": "112"}]"#
        );
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
            r#"{"high": 23, "low": 12, "rain": 35, "sunrise": "06:55", "sunset": null, "summary": "Spring", "units": "metric", "as_of": null, "failed": null, "syncs": false}"#
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

    #[test]
    fn synced_weather_wins_where_it_covers_and_says_how_old_it_is() {
        let content = "climate: {entries: [{date: 2026-04-11, high: 20, low: 10, summary: Pack}]}";
        let content = parse(content).unwrap().as_map().cloned().unwrap();
        let synced = r#"sync.climate:
  as_of: 2026-04-11T08:00
  failed: the server answered 500
  rows:
    - {place: azulejo, date: 2026-04-11, high: 25, sunrise: 2026-04-11T06:55}
    - {place: cais, date: 2026-04-11, high: 30}
    - x
"#;
        let store = parse(synced).unwrap().as_map().cloned().unwrap();
        let w = World { store, ..world("2026-04-11T10:00", &[]) };
        let scope = parse("place: {id: azulejo}").unwrap().as_map().cloned().unwrap();
        let sync = Fetch {
            every: None,
            url: "https://a.org".into(),
            query: vec![],
            secret: None,
            tag: vec![],
            read: vec![],
        };
        let module = Module { name: "climate", from: None, sync: Some(sync) };
        let mut r = Run::new(Keymap::of(&Value::Null), &content, &w, scope.clone());
        r.module(&module);
        assert_eq!(
            show(r.scope.get("weather")),
            r#"{"high": 25, "low": null, "rain": null, "sunrise": "06:55", "sunset": null, "summary": null, "units": "metric", "as_of": "2026-04-11T08:00", "failed": "the server answered 500", "syncs": true}"#
        );
        let both = Module { from: Some("climate".into()), ..module.clone() };
        let mut r = Run::new(Keymap::of(&Value::Null), &content, &w, scope);
        r.module(&both);
        let got = show(r.scope.get("weather"));
        assert!(got.starts_with(r#"{"high": 25, "low": 10, "rain": null, "sunrise": "06:55", "sunset": null, "summary": "Pack""#), "{got}");
        let fresh = World { store: Map::default(), ..w };
        let mut r = Run::new(Keymap::of(&Value::Null), &content, &fresh, Map::default());
        r.module(&module);
        assert!(
            show(r.scope.get("weather"))
                .ends_with(r#""as_of": null, "failed": null, "syncs": true}"#)
        );
    }
}
