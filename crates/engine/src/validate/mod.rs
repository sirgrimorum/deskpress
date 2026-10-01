//! The validator. It never stops at the first problem: the person fixing a pack wants the whole
//! list, once. Errors block the load, because the app will not pretend; warnings load and show a
//! line.

mod blocks;
mod keymap;
pub(crate) mod patterns;
mod theme;

use std::collections::{HashMap, HashSet};

pub use keymap::Keymap;
use patterns::{
    is_date, is_id, is_language, is_real_date, is_slug, is_stamp, is_time, is_timezone,
};
pub use theme::TOKENS as COLOR_TOKENS;

use crate::engine::Region;
use crate::modules::chart::apart;
use crate::modules::jet_lag::DOES;
use crate::modules::region;
use crate::modules::tasks::STATUSES;
use crate::modules::travel::TRAVEL;
use crate::value::{Map, Value, quote, show, text, truthy};

/// How far around a place its offline map is kept at the least, in metres, as `Maps.kt` keeps it.
const KEPT_M: f64 = 1000.0;

/// A number from `lo` to `hi`, both included.
fn within(v: Option<&Value>, lo: f64, hi: f64) -> bool {
    matches!(v, Some(Value::Number(n)) if (lo..=hi).contains(n))
}

/// One problem, at a path into the pack (`days.2026-04-11.blocks[2]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub at: String,
    pub message: String,
}

/// Everything wrong with a pack, in the order it was found.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub errors: Vec<Finding>,
    pub warnings: Vec<Finding>,
}

impl Report {
    /// Whether the pack loads: warnings do not stop it.
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub(crate) fn error(&mut self, at: impl Into<String>, message: impl Into<String>) {
        self.errors.push(Finding { at: at.into(), message: message.into() });
    }

    pub(crate) fn warn(&mut self, at: impl Into<String>, message: impl Into<String>) {
        self.warnings.push(Finding { at: at.into(), message: message.into() });
    }

    /// The id of the entry at `at`, when it has one in the shape of an id.
    fn id(&mut self, at: &str, id: Option<&Value>, needs: &str) -> Option<String> {
        let found = text(id);
        if !truthy(id) {
            self.error(at, needs);
        } else if !is_id(&found) {
            self.error(format!("{at}.id"), format!("{} {NOT_AN_ID}", show(id)));
        } else {
            return Some(found);
        }
        None
    }
}

pub(crate) const SEVERITIES: [&str; 4] = ["critical", "high", "medium", "low"];
/// A key of the manifest head, the test its value has to pass, and what to say when it does not.
type Shape = (&'static str, fn(&str) -> bool, &'static str);

const NOT_AN_ID: &str = "is not an id: lowercase letters, digits and underscores";

/// Checks a manifest, its content and its theme (when it has one) against each other.
pub fn validate(manifest: &Value, content: &Value, theme: Option<&Value>) -> Report {
    let mut c = Checker {
        r: Report::default(),
        keymap: Keymap::of(manifest),
        hidden: Vec::new(),
        places: HashSet::new(),
        shown: HashSet::new(),
        people: HashSet::new(),
        kids: false,
    };
    c.pack(manifest, content, theme);
    let (_, defined) = crate::define::define(manifest);
    c.r.errors.extend(defined.errors);
    c.r.warnings.extend(defined.warnings);
    c.r
}

/// What the checks learn on the way: people and places first, because days point at them.
struct Checker<'a> {
    r: Report,
    keymap: Keymap<'a>,
    hidden: Vec<String>,
    places: HashSet<String>,
    /// The places that show: some block names them, or `here` finds them by their `at`.
    shown: HashSet<String>,
    people: HashSet<String>,
    kids: bool,
}

impl Checker<'_> {
    fn pack(&mut self, manifest: &Value, content: &Value, theme: Option<&Value>) {
        let Some(manifest) = manifest.as_map() else {
            self.r.error("pack.yaml", "the manifest is missing or is not a mapping");
            return;
        };
        let Some(head) = manifest.get("pack").and_then(Value::as_map) else {
            self.r.error(
                "pack.yaml",
                "the manifest needs a \"pack:\" block with id, name, language, timezone and content",
            );
            return;
        };
        self.head(head);
        let conventions = manifest.get("conventions").and_then(Value::as_map);
        let convention = |key: &str| conventions.and_then(|c| c.get(key));
        for key in ["alert_prefixes", "hidden_prefixes"] {
            if convention(key).is_some_and(|v| v.as_list().is_none()) {
                self.r.error(format!("conventions.{key}"), "has to be a list of prefixes");
            }
        }
        let hidden = convention("hidden_prefixes").and_then(Value::as_list).unwrap_or_default();
        self.hidden = hidden.iter().map(|p| text(Some(p))).collect();

        let Some(content) = content.as_map() else {
            self.r.error("content", "the content file is missing or is not a mapping");
            return;
        };
        let keymap = self.keymap;
        let root = |name: &str| keymap.root(content, name);
        let days = match root("days") {
            Some(Value::List(days)) => days,
            Some(_) => {
                self.r.error("days", "has to be a list, one entry per day");
                return;
            }
            None => {
                self.r.error(
                    "days",
                    "no days in the content. If this pack calls them something else, map it: keymap.root.days",
                );
                return;
            }
        };
        let themes = theme.and_then(|t| t.get("themes")).and_then(Value::as_map);
        if let Some(people) = root("people") {
            self.people_list(people, themes);
        }
        if let Some(places) = root("places") {
            self.places_map(places);
        }
        self.days(days);
        let places = root("places").and_then(Value::as_map).map(Map::keys).into_iter().flatten();
        for id in places.filter(|id| !self.shown.contains(*id)) {
            let message = format!(
                "no block names this place, so what it says never shows: add place: {id} to the blocks that happen there"
            );
            self.r.warn(format!("places.{id}"), message);
        }
        if let Some(alerts) = root("alerts") {
            self.alerts(alerts);
        }
        if let Some(plan) = root("jet_lag") {
            self.jet_lag(plan);
        }
        if let Some(tasks) = root("tasks") {
            self.tasks(tasks);
        }
        if let Some(documents) = root("documents") {
            self.documents(documents);
        }
        if let Some(climate) = root("climate") {
            self.climate(climate);
        }
        if let Some(theme) = theme.filter(|t| **t != Value::Null) {
            theme::check(&mut self.r, theme);
        }
    }

    fn head(&mut self, head: &Map) {
        // A value of the wrong type is there, and gets its own message below.
        let missing = |v: Option<&Value>| {
            matches!(v, None | Some(Value::Null))
                || matches!(v, Some(Value::String(s)) if s.is_empty())
        };
        for key in ["id", "name", "language", "timezone", "content"] {
            if missing(head.get(key)) {
                self.r.error(format!("pack.{key}"), "required in the manifest");
            }
        }
        let shapes: [Shape; 4] = [
            (
                "id",
                is_slug,
                "is not a slug. Calendar event ids derive from it, so it has to be stable and plain: lowercase, digits and single dashes",
            ),
            ("timezone", is_timezone, "is not an IANA timezone name, like Europe/Madrid"),
            ("language", is_language, "is not a language tag, like es or pt-BR"),
            ("updated", |s| is_date(s) || is_stamp(s), "is not a date, or a date and time"),
        ];
        for (key, ok, message) in shapes {
            let v = head.get(key);
            if !missing(v) && !ok(&text(v)) {
                self.r.error(format!("pack.{key}"), format!("{} {message}", show(v)));
            }
        }
    }

    fn people_list(&mut self, people: &Value, themes: Option<&Map>) {
        let Some(people) = people.as_list() else {
            self.r.error("people", "has to be a list");
            return;
        };
        let keymap = self.keymap;
        for (i, p) in people.iter().enumerate() {
            let at = format!("people[{i}]");
            let Some(p) = p.as_map() else {
                self.r.error(&at, "has to be a mapping");
                continue;
            };
            let read = |key: &str| keymap.field(p, "person", key);
            let id = p.get("id");
            if let Some(found) = self.r.id(&at, id, "a person needs an id")
                && !self.people.insert(found)
            {
                self.r.error(format!("{at}.id"), format!("{} is used twice", show(id)));
            }
            if !truthy(read("name")) {
                self.r.warn(&at, "a person with no name shows up as their id");
            }
            if read("adult") == Some(&Value::Bool(false)) && truthy(id) {
                self.kids = true;
            }
            let language = text(read("language"));
            if !language.is_empty() && !is_language(&language) {
                let message =
                    format!("{} is not a language tag, like es or pt-BR", quote(&language));
                self.r.error(format!("{at}.language"), message);
            }
            let family = text(read("theme"));
            let dashed = format!("{family}-");
            if !family.is_empty()
                && let Some(themes) = themes
                && !themes.keys().any(|t| t == family || t.starts_with(&dashed))
            {
                let message = format!("{} is not a theme in the theme file", quote(&family));
                self.r.error(format!("{at}.theme"), message);
            }
        }
    }

    fn places_map(&mut self, places: &Value) {
        let Some(places) = places.as_map() else {
            self.r.error("places", "has to be a mapping of place id to place");
            return;
        };
        let keymap = self.keymap;
        for (id, place) in places.iter() {
            let at = format!("places.{id}");
            if !is_id(id) {
                self.r.error(&at, format!("{} {NOT_AN_ID}", quote(id)));
            }
            self.places.insert(id.to_owned());
            let Some(place) = place.as_map() else {
                self.r.error(&at, "has to be a mapping");
                continue;
            };
            let read = |key: &str| keymap.field(place, "place", key);
            if !truthy(read("name")) {
                self.r.warn(
                    &at,
                    "no name, so the screen shows the id with its underscores turned into spaces",
                );
            }
            match read("at") {
                Some(Value::Map(coords)) => {
                    self.shown.insert(id.to_owned());
                    self.coordinates(&at, coords);
                    let radius = coords.get("radius_m");
                    if radius.is_some() && !within(radius, 25.0, 20000.0) {
                        let message =
                            format!("{} is not a radius in metres between 25 and 20000", show(radius));
                        self.r.error(format!("{at}.at.radius_m"), message);
                    }
                }
                Some(_) => {
                    self.r.error(format!("{at}.at"), "has to be a mapping with lat, lon and radius_m");
                }
                None => self.r.warn(
                    &at,
                    "no coordinates, so this place cannot become a geofence and the app has to trust the plan about where you are",
                ),
            }
            let verified = read("verified");
            if verified.is_some() && !is_date(&text(verified)) {
                let message = format!("{} is not a date, YYYY-MM-DD", show(verified));
                self.r.error(format!("{at}.verified"), message);
            }
            let during = read("during");
            let during_map = during.and_then(Value::as_map);
            if during.is_some() && during_map.is_none() {
                self.r.error(format!("{at}.during"), "has to be a mapping");
            }
            if let Some(kind) = during_map.and_then(|d| keymap.field(d, "block", "type")) {
                self.block_type(&format!("{at}.during.type"), kind);
            }
            if read("parking").is_some_and(|p| p.as_map().is_none()) {
                self.r.error(format!("{at}.parking"), "has to be a mapping");
            }
            // Points sit on the place, or inside during, where many packs already keep them.
            let points = read("points")
                .filter(|p| **p != Value::Null)
                .or_else(|| during_map.and_then(|d| keymap.field(d, "place", "points")));
            if let Some(points) = points {
                let around = read("at").and_then(|c| region(id, c));
                self.points(&at, points, around.as_ref());
            }
            if read("in_order").is_some_and(|o| !matches!(o, Value::Bool(_))) {
                self.r.error(format!("{at}.in_order"), "is true or false");
            }
            if let Some(plan) = read("plan") {
                self.plan(&at, plan);
            }
            if let Some(legs) = read("legs") {
                self.legs(&at, legs, places);
            }
            let known = [
                "name", "kind", "at", "safe", "during", "parking", "points", "in_order", "plan",
                "verified", "legs",
            ];
            self.free_keys(&at, place, &known, "place");
        }
    }

    /// `legs`: the minutes from this place to others of the pack, for the travel between blocks.
    fn legs(&mut self, at: &str, legs: &Value, places: &Map) {
        let Some(legs) = legs.as_map() else {
            self.r.error(format!("{at}.legs"), "has to be a mapping of place id to minutes");
            return;
        };
        for (to, m) in legs.iter() {
            let here = format!("{at}.legs.{to}");
            if !within(Some(m), 1.0, 720.0) || !matches!(m, Value::Number(n) if n.fract() == 0.0) {
                let message = format!("{} is not whole minutes from 1 to 720", show(Some(m)));
                self.r.error(&here, message);
            }
            if places.get(to).is_none() {
                self.r.warn(&here, "is not a place of this pack, so no leg uses it");
            }
        }
    }

    /// `lat` and `lon`, and whether both are on the globe.
    fn coordinates(&mut self, at: &str, coords: &Map) -> bool {
        let lat = coords.get("lat");
        let on_lat = within(lat, -90.0, 90.0);
        if !on_lat {
            self.r.error(format!("{at}.at.lat"), format!("{} is not a latitude", show(lat)));
        }
        let lon = coords.get("lon");
        let on_lon = within(lon, -180.0, 180.0);
        if !on_lon {
            self.r.error(format!("{at}.at.lon"), format!("{} is not a longitude", show(lon)));
        }
        on_lat && on_lon
    }

    /// A place's own map: pins in the picture's coordinates, and the picture when there is one
    /// (decision 0026).
    fn plan(&mut self, at: &str, plan: &Value) {
        let keymap = self.keymap;
        let at = format!("{at}.plan");
        let Some(plan) = plan.as_map() else {
            self.r.error(&at, "has to be a mapping: the pins, and an image to put them on");
            return;
        };
        let image = keymap.field(plan, "plan", "image");
        if image.is_some() && patterns::leaves(&text(image)) {
            let message = format!("{} leaves the pack folder", show(image));
            self.r.error(format!("{at}.image"), message);
        }
        let Some(pins) = keymap.field(plan, "plan", "points") else {
            self.r.warn(&at, "no points, so the map of this place has nothing on it");
            return;
        };
        let Some(pins) = pins.as_list() else {
            self.r.error(format!("{at}.points"), "has to be a list of pins");
            return;
        };
        let fraction =
            |v: Option<&Value>| matches!(v, Some(Value::Number(n)) if (0.0..=1.0).contains(n));
        for (i, pin) in pins.iter().enumerate() {
            let at = format!("{at}.points[{i}]");
            let Some(pin) = pin.as_map() else {
                self.r.error(&at, "has to be a mapping with a name and where it sits");
                continue;
            };
            if !truthy(keymap.field(pin, "point", "name")) {
                self.r.error(&at, "a pin needs a name: it is what the person reads on the map");
            }
            for axis in ["x", "y"] {
                let v = keymap.field(pin, "point", axis);
                if !fraction(v) {
                    let message = format!("{} is not a fraction of the picture, 0 to 1", show(v));
                    self.r.error(format!("{at}.{axis}"), message);
                }
            }
        }
    }

    /// A place's points; `around` is the place's own circle, which its offline map keeps.
    fn points(&mut self, at: &str, points: &Value, around: Option<&Region>) {
        let Some(points) = points.as_list() else {
            self.r.error(format!("{at}.points"), "has to be a list of points");
            return;
        };
        let keymap = self.keymap;
        let mut seen = HashSet::new();
        let mut pinned = false;
        for (i, pt) in points.iter().enumerate() {
            let pat = format!("{at}.points[{i}]");
            let Some(pt) = pt.as_map() else {
                self.r.error(&pat, "has to be a mapping");
                continue;
            };
            let id = pt.get("id");
            if !truthy(id) {
                self.r.warn(&pat, "a point with no id cannot be linked to");
            } else if !is_id(&text(id)) {
                self.r.error(format!("{pat}.id"), format!("{} {NOT_AN_ID}", show(id)));
            } else if !seen.insert(text(id)) {
                let message = format!("{} is used twice in this place", show(id));
                self.r.error(format!("{pat}.id"), message);
            }
            if !truthy(keymap.field(pt, "point", "name")) {
                self.r.error(&pat, "a point needs a name");
            }
            if let Some(spot) = keymap.field(pt, "point", "at") {
                pinned = true;
                self.point_at(&pat, spot, around);
            }
            if self.kids && !truthy(keymap.field(pt, "point", "for_kids")) {
                self.r.warn(
                    &pat,
                    "nothing written for a kid here, so this point is left out of the kid's list",
                );
            }
        }
        if pinned && around.is_none() {
            self.r
                .warn(at, "its points have a position but it has none, so no map is kept for them");
        }
    }

    /// A point's own position, and whether the map kept offline around its place reaches it.
    fn point_at(&mut self, at: &str, spot: &Value, around: Option<&Region>) {
        let Some(coords) = spot.as_map() else {
            self.r.error(format!("{at}.at"), "has to be a mapping with lat and lon");
            return;
        };
        if !self.coordinates(at, coords) {
            return;
        }
        if coords.get("radius_m").is_some() {
            self.r.warn(format!("{at}.at.radius_m"), "a point has no radius, only where it is");
        }
        let (Some(c), Some(p)) = (around, region("", spot)) else { return };
        let (far, reach) = (apart(c.lat, c.lon, p.lat, p.lon), c.radius_m.max(KEPT_M));
        if far > reach {
            let message = format!(
                "is {far:.0} m from its place, and the map kept offline reaches {reach:.0} m around it"
            );
            self.r.warn(format!("{at}.at"), message);
        }
    }

    fn days(&mut self, days: &[Value]) {
        let keymap = self.keymap;
        let mut dates = HashSet::new();
        let mut options_by_date: HashMap<String, Vec<String>> = HashMap::new();
        for (i, day) in days.iter().enumerate() {
            let label = format!("days[{i}]");
            let Some(day) = day.as_map() else {
                self.r.error(&label, "has to be a mapping");
                continue;
            };
            let read = |key: &str| keymap.field(day, "day", key);
            let date = text(read("date"));
            let at = if date.is_empty() { label.clone() } else { format!("days.{date}") };
            if date.is_empty() {
                self.r.error(&label, "a day needs a date");
            } else if !is_real_date(&date) {
                let message = format!("{} is not a real date, YYYY-MM-DD", quote(&date));
                self.r.error(format!("{at}.date"), message);
            } else if !dates.insert(date.clone()) {
                let message = format!(
                    "{date} appears twice. Two entries for one day means the reader has to guess"
                );
                self.r.error(format!("{at}.date"), message);
            }
            if !truthy(read("title")) {
                self.r.error(&at, "a day needs a title: it is what the agenda shows");
            }
            if let Some(Value::List(who)) = read("who") {
                for id in who.iter().filter(|id| !self.people.contains(&text(Some(id)))) {
                    let message =
                        format!("{} is not one of the people in this pack", show(Some(id)));
                    self.r.warn(format!("{at}.who"), message);
                }
            }
            let (blocks, fixed, options) = (read("blocks"), read("fixed"), read("options"));
            if blocks.is_none() && fixed.is_none() && options.is_none() {
                self.r.warn(&at, "no blocks and no options, so this day has nothing to show");
            }
            if let Some(blocks) = blocks {
                self.blocks(&format!("{at}.blocks"), blocks);
            }
            if let Some(fixed) = fixed {
                self.blocks(&format!("{at}.fixed"), fixed);
            }
            if let Some(options) = options {
                if let Some(ids) = self.options(&at, options)
                    && !date.is_empty()
                {
                    options_by_date.insert(date.clone(), ids);
                }
                self.decision(&at, &date, read("decision"));
            }
            self.word(&at, "travel", &TRAVEL, read("travel"));
            let known = [
                "date", "title", "who", "zone", "travel", "blocks", "fixed", "options", "decision",
            ];
            self.free_keys(&at, day, &known, "day");
        }

        // Second pass: references across days, now that every day is known.
        for day in days.iter().filter_map(Value::as_map) {
            let read = |key: &str| keymap.field(day, "day", key);
            let date = Some(text(read("date"))).filter(|d| !d.is_empty()).unwrap_or("?".into());
            let decides = read("decision").and_then(|d| keymap.read(d, "decision", "decides"));
            for other in decides.and_then(Value::as_list).unwrap_or_default() {
                let other = text(Some(other));
                if !dates.contains(&other) {
                    let message = format!("{} is not a day in this pack", quote(&other));
                    self.r.error(format!("days.{date}.decision.decides"), message);
                }
            }
            let options = read("options").and_then(Value::as_list).unwrap_or_default();
            for (j, requires) in options.iter().enumerate() {
                let Some(requires) = requires.get("requires") else {
                    continue;
                };
                let rat = format!("days.{date}.options[{j}].requires");
                let Some(requires) = requires.as_map() else {
                    self.r.error(&rat, "has to be a mapping with date and option");
                    continue;
                };
                let rdate = text(requires.get("date"));
                if !dates.contains(&rdate) {
                    self.r.error(&rat, format!("{} is not a day in this pack", quote(&rdate)));
                    continue;
                }
                let option = text(requires.get("option"));
                match options_by_date.get(&rdate) {
                    None => self.r.error(
                        &rat,
                        format!("{rdate} has no options, so nothing on it can be required"),
                    ),
                    Some(ids) if !ids.contains(&option) => self.r.error(
                        &rat,
                        format!(
                            "{} is not an option on {rdate}. It has {}",
                            quote(&option),
                            ids.join(", ")
                        ),
                    ),
                    Some(_) => {}
                }
            }
        }
    }

    /// The ids of the options, when they are a list.
    fn options(&mut self, at: &str, options: &Value) -> Option<Vec<String>> {
        let Some(options) = options.as_list() else {
            self.r.error(format!("{at}.options"), "has to be a list of closed alternatives");
            return None;
        };
        if options.len() < 2 {
            self.r.error(
                format!("{at}.options"),
                "one option is not an option. Either two closed plans, or plain blocks",
            );
        }
        let keymap = self.keymap;
        let mut ids: Vec<String> = Vec::new();
        let mut recommended = 0;
        for (j, opt) in options.iter().enumerate() {
            let oat = format!("{at}.options[{j}]");
            let Some(opt) = opt.as_map() else {
                self.r.error(&oat, "has to be a mapping");
                continue;
            };
            let read = |key: &str| keymap.field(opt, "option", key);
            let id = opt.get("id");
            let needs = "an option needs an id: the choice is stored on the device under it";
            if let Some(found) = self.r.id(&oat, id, needs) {
                if ids.contains(&found) {
                    let message = format!("{} is used twice on this day", show(id));
                    self.r.error(format!("{oat}.id"), message);
                } else {
                    ids.push(found);
                }
            }
            if !truthy(read("name")) {
                self.r.error(
                    &oat,
                    "an option needs a name: it is what the suggestion screen puts on the button",
                );
            }
            if read("recommended") == Some(&Value::Bool(true)) {
                recommended += 1;
            }
            match read("blocks") {
                Some(blocks) => self.blocks(&format!("{oat}.blocks"), blocks),
                None => self.r.error(&oat, "an option with no blocks is an idea, not an option"),
            }
        }
        if recommended == 0 {
            self.r.warn(
                format!("{at}.options"),
                "no option is recommended, so until somebody chooses the app has nothing to behave as",
            );
        }
        if recommended > 1 {
            let message = format!("{recommended} options are recommended. Only one can be");
            self.r.error(format!("{at}.options"), message);
        }
        Some(ids)
    }

    fn decision(&mut self, at: &str, date: &str, decision: Option<&Value>) {
        let Some(decision) = decision.and_then(Value::as_map) else {
            self.r.error(
                at,
                "a day with options needs a decision block: when the app asks, and what that answer decides",
            );
            return;
        };
        let keymap = self.keymap;
        let read = |key: &str| keymap.field(decision, "decision", key);
        let when = text(read("when"));
        let wat = format!("{at}.decision.when");
        if when.is_empty() {
            self.r.error(&wat, "required: the day the app asks, once");
        } else if !is_real_date(&when) {
            self.r.error(&wat, format!("{} is not a real date", quote(&when)));
        } else if !date.is_empty() && when.as_str() > date {
            let message = format!(
                "{when} is after the day it decides ({date}). The question has to be asked before it stops being a question"
            );
            self.r.error(&wat, message);
        }
        let time = text(read("at"));
        if !time.is_empty() && !is_time(&time) {
            let message = format!("{} is not a time, HH:MM on a 24 hour clock", quote(&time));
            self.r.error(format!("{at}.decision.at"), message);
        }
        if read("decides").is_some_and(|d| d.as_list().is_none()) {
            self.r.error(format!("{at}.decision.decides"), "has to be a list of dates");
        }
    }

    fn alerts(&mut self, alerts: &Value) {
        let Some(alerts) = alerts.as_list() else {
            self.r.error("alerts", "has to be a list");
            return;
        };
        let keymap = self.keymap;
        let mut ids = HashSet::new();
        let mut critical = 0;
        for (i, a) in alerts.iter().enumerate() {
            let at = format!("alerts[{i}]");
            let Some(a) = a.as_map() else {
                self.r.error(&at, "has to be a mapping");
                continue;
            };
            let read = |key: &str| keymap.field(a, "alert", key);
            let id = a.get("id");
            if !truthy(id) {
                self.r.error(&at, "an alert needs an id");
            } else if !ids.insert(text(id)) {
                self.r.error(format!("{at}.id"), format!("{} is used twice", show(id)));
            }
            if !truthy(read("title")) {
                self.r.error(
                    &at,
                    "an alert needs a title: at critical severity it is the line above the answer",
                );
            }
            let severity = read("severity");
            let canon = keymap.value("severity", severity);
            let list = SEVERITIES.join(", ");
            if severity.is_none() {
                self.r.error(&at, format!("an alert needs a severity: {list}"));
            } else if !SEVERITIES.contains(&canon.as_str()) {
                let message = format!(
                    "{} is not a severity. Use {list}, or map your own words in keymap.values.severity",
                    show(severity)
                );
                self.r.error(format!("{at}.severity"), message);
            }
            if canon == "critical" {
                critical += 1;
            }
            for key in ["at", "notify_from"] {
                let v = text(read(key));
                if !v.is_empty() && !is_stamp(&v) && !is_date(&v) {
                    let message = format!(
                        "{} is not a date or a timestamp, YYYY-MM-DD or YYYY-MM-DDTHH:MM",
                        quote(&v)
                    );
                    self.r.error(format!("{at}.{key}"), message);
                }
            }
            let time = text(read("time"));
            if !time.is_empty() && !is_time(&time) {
                self.r
                    .error(format!("{at}.time"), format!("{} is not a time, HH:MM", quote(&time)));
            }
            self.word(&at, "status", &STATUSES, read("status"));
            let day = text(read("at")).get(..10).map(str::to_owned);
            if let Some(repeat) = read("repeat") {
                self.repeat(&format!("{at}.repeat"), repeat, day.as_deref());
            }
            let (from, when) = (text(read("notify_from")), text(read("at")));
            let rings = is_stamp(&from) || is_stamp(&when) || (is_date(&when) && is_time(&time));
            match read("alarm") {
                Some(Value::Bool(true)) if !rings => self.r.error(
                    format!("{at}.alarm"),
                    "an alarm needs a time to ring at: a notify_from or an at with its hour",
                ),
                Some(a) if !matches!(a, Value::Bool(_)) => {
                    self.r.error(format!("{at}.alarm"), "is true or false")
                }
                _ => {}
            }
        }
        if critical > 6 {
            let message = format!(
                "{critical} alerts are critical. Severity is a budget: when everything is critical, the real one gets swiped past too"
            );
            self.r.warn("alerts", message);
        }
    }

    /// A word the shell knows for `key`, in the canonical form or the pack's own.
    fn word(&mut self, at: &str, key: &str, words: &[&str], v: Option<&Value>) {
        if v.is_some() && !words.contains(&self.keymap.value(key, v).as_str()) {
            let message = format!(
                "{} is not one of {}, or map your own words in keymap.values.{key}",
                show(v),
                words.join(", ")
            );
            self.r.error(format!("{at}.{key}"), message);
        }
    }

    /// An alert's `repeat`: every so many whole days, to a date not before the alert's own.
    fn repeat(&mut self, at: &str, repeat: &Value, day: Option<&str>) {
        let Some(repeat) = repeat.as_map() else {
            self.r.error(at, "has to be a mapping: every, a number of days, and until, a date");
            return;
        };
        let keymap = self.keymap;
        let read = |k: &str| keymap.field(repeat, "repeat", k);
        let (every, until) = (read("every"), text(read("until")));
        if !matches!(every, Some(Value::Number(n)) if n.fract() == 0.0 && *n >= 1.0) {
            self.r.error(
                format!("{at}.every"),
                format!("{} is not a whole number of days", show(every)),
            );
        }
        let Some(day) = day else {
            self.r.error(at, "a repeat needs the alert's at, the first time it falls");
            return;
        };
        if !is_real_date(&until) || until.as_str() < day {
            let message = format!("{} is not a real date on or after {day}", quote(&until));
            self.r.error(format!("{at}.until"), message);
        }
    }

    fn jet_lag(&mut self, plan: &Value) {
        let Some(days) = plan.as_list() else {
            self.r.error("jet_lag", "has to be a list, one entry per day");
            return;
        };
        let keymap = self.keymap;
        let mut dates = HashSet::new();
        for (i, day) in days.iter().enumerate() {
            let label = format!("jet_lag[{i}]");
            let Some(day) = day.as_map() else {
                self.r.error(&label, "has to be a mapping");
                continue;
            };
            let read = |key: &str| keymap.field(day, "jet_lag", key);
            let date = text(read("date"));
            let at = if is_real_date(&date) { format!("jet_lag.{date}") } else { label };
            if !is_real_date(&date) {
                let message = format!("{} is not a real date, YYYY-MM-DD", show(read("date")));
                self.r.error(&at, message);
            } else if !dates.insert(date.clone()) {
                self.r.error(format!("{at}.date"), format!("{date} appears twice"));
            }
            let Some(steps) = read("steps").and_then(Value::as_list) else {
                self.r.error(&at, "a jet-lag day needs steps, a list");
                continue;
            };
            for (n, step) in steps.iter().enumerate() {
                self.step(&format!("{at}.steps[{n}]"), step);
            }
        }
    }

    fn step(&mut self, at: &str, step: &Value) {
        let Some(step) = step.as_map() else {
            self.r.error(at, "has to be a mapping: a time, a text and what it does");
            return;
        };
        let keymap = self.keymap;
        let read = |key: &str| keymap.field(step, "step", key);
        if !truthy(read("text")) {
            self.r.error(at, "a step needs a text: what to do");
        }
        for key in ["time", "until"] {
            let v = text(read(key));
            if !v.is_empty() && !is_time(&v) {
                self.r.error(format!("{at}.{key}"), format!("{} is not a time, HH:MM", quote(&v)));
            }
        }
        self.word(at, "do", &DOES, read("do"));
        self.whom(&format!("{at}.for"), read("for"));
        match read("alarm") {
            Some(Value::Bool(true)) if text(read("time")).is_empty() => {
                self.r.error(format!("{at}.alarm"), "an alarm needs the step's time to ring at")
            }
            Some(a) if !matches!(a, Value::Bool(_)) => {
                self.r.error(format!("{at}.alarm"), "is true or false")
            }
            _ => {}
        }
    }

    fn tasks(&mut self, tasks: &Value) {
        let Some(tasks) = tasks.as_list() else {
            self.r.error("tasks", "has to be a list");
            return;
        };
        let keymap = self.keymap;
        let mut ids = HashSet::new();
        for (i, t) in tasks.iter().enumerate() {
            let at = format!("tasks[{i}]");
            let Some(t) = t.as_map() else {
                self.r.error(&at, "has to be a mapping");
                continue;
            };
            let read = |key: &str| keymap.field(t, "task", key);
            let id = t.get("id");
            if let Some(found) = self.r.id(&at, id, "a task needs an id: it names the tick")
                && !ids.insert(found)
            {
                self.r.error(format!("{at}.id"), format!("{} is used twice", show(id)));
            }
            if !truthy(read("title")) {
                self.r.error(&at, "a task needs a title");
            }
            let deadline = read("deadline");
            if deadline.is_some() && !is_real_date(&text(deadline)) {
                let message = format!("{} is not a real date, YYYY-MM-DD", show(deadline));
                self.r.error(format!("{at}.deadline"), message);
            }
            self.word(&at, "status", &STATUSES, read("status"));
        }
    }

    fn documents(&mut self, documents: &Value) {
        let Some(documents) = documents.as_list() else {
            self.r.error("documents", "has to be a list");
            return;
        };
        let keymap = self.keymap;
        let mut ids = HashSet::new();
        for (i, d) in documents.iter().enumerate() {
            let at = format!("documents[{i}]");
            let Some(d) = d.as_map() else {
                self.r.error(&at, "has to be a mapping");
                continue;
            };
            let read = |key: &str| keymap.field(d, "document", key);
            let id = d.get("id");
            if let Some(found) = self.r.id(&at, id, "a document needs an id")
                && !ids.insert(found)
            {
                self.r.error(format!("{at}.id"), format!("{} is used twice", show(id)));
            }
            if !truthy(read("title")) {
                self.r.error(&at, "a document needs a title");
            }
            let file = text(read("file"));
            if file.is_empty() {
                self.r.error(
                    &at,
                    "a document needs a file: it is the thing somebody at a counter is asking for",
                );
            } else if patterns::leaves(&file) {
                self.r.error(
                    format!("{at}.file"),
                    format!("{} leaves the pack folder", show(read("file"))),
                );
            }
            match read("call") {
                Some(Value::Map(numbers)) => {
                    for (label, number) in numbers.iter() {
                        if !patterns::is_phone(&text(Some(number))) {
                            let at = format!("{at}.call.{label}");
                            self.r.error(
                                at,
                                format!("{} is not a number to dial", show(Some(number))),
                            );
                        }
                    }
                }
                Some(_) => {
                    self.r.error(format!("{at}.call"), "has to be a mapping of label to number")
                }
                None => {}
            }
            self.person(&format!("{at}.for"), &text(read("for")));
            if read("fields").is_some_and(|f| f.as_map().is_none()) {
                self.r.error(format!("{at}.fields"), "has to be a mapping of label to value");
            }
        }
    }

    fn climate(&mut self, climate: &Value) {
        let Some(climate) = climate.as_map() else {
            self.r.error("climate", "has to be a mapping with units and entries");
            return;
        };
        let units = climate.get("units");
        if units.is_some_and(|u| !["metric", "imperial"].contains(&text(Some(u)).as_str())) {
            self.r.error("climate.units", format!("{} is not metric or imperial", show(units)));
        }
        let Some(entries) = climate.get("entries") else {
            return;
        };
        let Some(entries) = entries.as_list() else {
            self.r.error("climate.entries", "has to be a list");
            return;
        };
        for (i, e) in entries.iter().enumerate() {
            let at = format!("climate.entries[{i}]");
            let Some(e) = e.as_map() else {
                self.r.error(&at, "has to be a mapping");
                continue;
            };
            let read = |key: &str| self.keymap.field(e, "climate", key);
            let place = text(read("place"));
            if !place.is_empty() && !self.places.contains(&place) {
                let message = format!("{} is not a place in this pack", quote(&place));
                self.r.error(format!("{at}.place"), message);
            }
            let (month, date) = (read("month"), read("date"));
            if month.is_some() && date.is_some() {
                self.r.error(&at, "a month or a date, not both: which one would it be?");
            }
            let a_month = matches!(month, Some(Value::Number(m)) if m.fract() == 0.0 && (1.0..=12.0).contains(m));
            if month.is_some() && !a_month {
                self.r.error(
                    format!("{at}.month"),
                    format!("{} is not a month, 1 to 12", show(month)),
                );
            }
            if date.is_some() && !is_real_date(&text(date)) {
                let message = format!("{} is not a real date, YYYY-MM-DD", show(date));
                self.r.error(format!("{at}.date"), message);
            }
            for key in ["high", "low", "rain"] {
                let v = read(key);
                if v.is_some_and(|v| !matches!(v, Value::Number(_))) {
                    self.r.error(format!("{at}.{key}"), format!("{} is not a number", show(v)));
                }
            }
            for key in ["sunrise", "sunset"] {
                let v = read(key);
                if v.is_some() && !is_time(&text(v)) {
                    let message = format!("{} is not a time, HH:MM", show(v));
                    self.r.error(format!("{at}.{key}"), message);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml::parse;

    const MANIFEST: &str = "pack:
  id: test-pack
  name: Test pack
  language: en
  timezone: Europe/Madrid
  content: content.yaml
";

    const DAY: &str = "  - date: 2026-04-11
    title: A day
    blocks: [['10:00', 'Something happens.']]
";

    fn yaml(text: &str) -> Value {
        parse(text).unwrap()
    }

    fn with(manifest: &str, content: &str, theme: Option<&str>) -> Report {
        let theme = theme.map(yaml);
        validate(&yaml(&format!("{MANIFEST}{manifest}")), &yaml(content), theme.as_ref())
    }

    fn said(r: &Report) -> String {
        let all = r.errors.iter().chain(&r.warnings);
        all.map(|f| format!("{}: {}", f.at, f.message)).collect::<Vec<_>>().join("\n")
    }

    /// The content, with one day ahead of whatever else it holds.
    fn day_and(rest: &str) -> String {
        format!("days:\n{DAY}{rest}")
    }

    fn says(content: &str, needle: &str) {
        let s = said(&with("", content, None));
        assert!(s.contains(needle), "wanted {needle:?} in:\n{s}");
    }

    fn clean(manifest: &str, content: &str, theme: Option<&str>) {
        assert_eq!(said(&with(manifest, content, theme)), "");
    }

    // The 23 tokens as a theme that passes every pair. `over` replaces or adds lines.
    fn palette(over: &str) -> String {
        let fg = ["ink", "ink-muted", "action-ink", "highlight-ink", "highlight-text", "chip-ink"];
        let fg2 = ["bar-ink", "bar-muted", "alert", "missing-text"];
        let bg =
            ["paper", "card", "action-bg", "highlight-bg", "chip-bg", "bar-bg", "missing-fill"];
        let rest = ["card-line", "line", "rule", "soft", "highlight-line", "missing-border"];
        let mut colors: Vec<(String, String)> = Vec::new();
        for k in fg.iter().chain(&fg2) {
            colors.push((k.to_string(), "'#000000'".into()));
        }
        for k in bg {
            colors.push((k.into(), "'#ffffff'".into()));
        }
        for k in rest {
            colors.push((k.into(), "'#888888'".into()));
        }
        for line in over.lines() {
            let (k, v) = line.split_once(": ").unwrap();
            match colors.iter_mut().find(|(name, _)| name == k) {
                Some(slot) if v == "DELETE" => slot.1 = "DELETE".into(),
                Some(slot) => slot.1 = v.into(),
                None => colors.push((k.into(), v.into())),
            }
        }
        let body: String = colors
            .iter()
            .filter(|(_, v)| v != "DELETE")
            .map(|(k, v)| format!("      {k}: {v}\n"))
            .collect();
        format!("themes:\n  plain:\n    name: Plain\n    mode: light\n    colors:\n{body}")
    }

    #[test]
    fn a_manifest_that_is_not_one_stops_everything() {
        let r = validate(&Value::Null, &Value::Null, None);
        assert_eq!(said(&r), "pack.yaml: the manifest is missing or is not a mapping");
        assert!(!r.ok());
        let r = validate(&yaml("nope: 1"), &yaml("days: []"), None);
        assert!(said(&r).contains("needs a \"pack:\" block"));
    }

    #[test]
    fn the_manifest_head_needs_its_keys_in_shape() {
        let r = validate(&yaml("pack:\n  id: ''\n  name: ~\n"), &yaml(&day_and("")), None);
        let s = said(&r);
        for key in ["id", "name", "language", "timezone", "content"] {
            assert!(s.contains(&format!("pack.{key}: required in the manifest")), "{s}");
        }
        let bad = "pack:
  id: Not A Slug
  name: x
  language: Spanish
  timezone: Madrid
  content: c.yaml
  updated: soon
";
        let s = said(&validate(&yaml(bad), &yaml(&day_and("")), None));
        assert!(s.contains("pack.id: \"Not A Slug\" is not a slug"), "{s}");
        assert!(s.contains("pack.updated: \"soon\" is not a date, or a date and time"), "{s}");
        assert!(s.contains("pack.timezone: \"Madrid\" is not an IANA timezone name"), "{s}");
        assert!(s.contains("pack.language: \"Spanish\" is not a language tag"), "{s}");

        // A value of the wrong type is in the wrong shape, not missing.
        let typed = "pack: {id: [], name: x, language: false, timezone: 3, content: c.yaml}\n";
        let s = said(&validate(&yaml(typed), &yaml(&day_and("")), None));
        assert!(!s.contains("required"), "{s}");
        assert!(s.contains("pack.id: [] is not a slug"), "{s}");
        assert!(s.contains("pack.language: false is not a language tag"), "{s}");
        assert!(s.contains("pack.timezone: 3 is not an IANA timezone name"), "{s}");
    }

    #[test]
    fn conventions_are_lists_of_prefixes() {
        let r = with(
            "conventions: {alert_prefixes: warn, hidden_prefixes: source}\n",
            &day_and(""),
            None,
        );
        let s = said(&r);
        assert!(s.contains("conventions.alert_prefixes: has to be a list of prefixes"), "{s}");
        assert!(s.contains("conventions.hidden_prefixes: has to be a list of prefixes"), "{s}");
    }

    #[test]
    fn content_needs_days_as_a_list() {
        let r = validate(&yaml(MANIFEST), &Value::Null, None);
        assert!(said(&r).contains("content file is missing"));
        says("people: []", "keymap.root.days");
        says("days: {a: 1}", "days: has to be a list, one entry per day");
    }

    #[test]
    fn the_smallest_pack_that_says_something_loads() {
        let r = with("", &day_and(""), None);
        assert!(r.ok());
        assert_eq!(said(&r), "");
    }

    #[test]
    fn the_example_pack_loads_with_no_errors_and_no_warnings() {
        let manifest = yaml(include_str!("../../../../examples/one-day/pack.yaml"));
        let manifest = crate::template::extend(manifest).unwrap();
        let content = yaml(include_str!("../../../../examples/one-day/content.yaml"));
        let theme = yaml(include_str!("../../../../examples/one-day/theme.yaml"));
        assert_eq!(said(&validate(&manifest, &content, Some(&theme))), "");
    }

    #[test]
    fn people_need_ids_that_are_ids_and_themes_that_exist() {
        let content = day_and(
            "people:
  - just text
  - {name: Nobody}
  - {id: Rita, name: Rita}
  - {id: tomas, name: Tomas, adult: false, theme: tomas}
  - {id: tomas}
  - {id: leo, name: Leo, language: PT}
  - {id: ana, name: Ana, theme: ana, language: pt-BR}
",
        );
        let theme = palette("").replace("  plain:", "  ana-light:");
        let s = said(&with("", &content, Some(&theme)));
        assert!(s.contains("people[0]: has to be a mapping"), "{s}");
        assert!(s.contains("people[1]: a person needs an id"), "{s}");
        assert!(s.contains("people[2].id: \"Rita\" is not an id"), "{s}");
        assert!(s.contains("people[3].theme: \"tomas\" is not a theme in the theme file"), "{s}");
        assert!(s.contains("people[4].id: \"tomas\" is used twice"), "{s}");
        assert!(s.contains("people[4]: a person with no name shows up as their id"), "{s}");
        assert!(s.contains("people[5].language: \"PT\" is not a language tag"), "{s}");
        assert!(!s.contains("people[6]"), "{s}");
        says("days: []\npeople: {a: 1}", "people: has to be a list");
    }

    #[test]
    fn places_are_a_mapping_of_ids_with_coordinates() {
        let content = day_and(
            "places:
  Bad Id: {name: x, at: {lat: 1, lon: 1}}
  text: just text
  nameless: {at: [1, 2]}
  far: {name: Far, at: {lat: 91, lon: '2', radius_m: 10}}
  none: {name: None, at: {}}
  fine: {name: Fine, at: {lat: -90, lon: 180, radius_m: 20000}, verified: 2026-04-01}
  old: {name: Old, verified: April, during: x, parking: x, points: x}
",
        );
        let s = said(&with("", &content, None));
        for needle in [
            "places.Bad Id: \"Bad Id\" is not an id",
            "places.text: has to be a mapping",
            "places.nameless: no name, so the screen shows the id",
            "places.nameless.at: has to be a mapping with lat, lon and radius_m",
            "places.far.at.lat: 91 is not a latitude",
            "places.far.at.lon: \"2\" is not a longitude",
            "places.far.at.radius_m: 10 is not a radius in metres between 25 and 20000",
            "places.none.at.lat: nothing is not a latitude",
            "places.old: no coordinates, so this place cannot become a geofence",
            "places.old.verified: \"April\" is not a date, YYYY-MM-DD",
            "places.old.during: has to be a mapping",
            "places.old.parking: has to be a mapping",
            "places.old.points: has to be a list of points",
        ] {
            assert!(s.contains(needle), "wanted {needle:?} in:\n{s}");
        }
        // No block names fine or old, but fine has an `at`, so only old never shows.
        let unnamed = "places.old: no block names this place, so what it says never shows: add place: old to the blocks that happen there";
        assert!(s.contains(unnamed) && !s.contains("places.fine"), "{s}");
        says("days: []\nplaces: [a]", "places: has to be a mapping of place id to place");

        // No coordinates is a warning: the pack still loads.
        let r = with("", &day_and("places:\n  home: {name: Home}\n"), None);
        let s = said(&r);
        assert!(r.ok(), "{s}");
        assert_eq!(r.warnings.len(), 2);
        assert!(s.starts_with("places.home: no coordinates"), "{s}");
        assert!(s.contains("places.home: no block names this place"), "{s}");
    }

    #[test]
    fn points_on_a_place_or_inside_during() {
        let content = day_and(
            "people:
  - {id: tomas, name: Tomas, adult: false}
places:
  museum:
    name: Museum
    at: {lat: 1, lon: 1}
    points:
      - text
      - {name: Hall}
      - {id: hall, name: Hall, for_kids: The big one}
      - {id: hall}
      - {id: Hall, name: Big}
  park:
    name: Park
    at: {lat: 1, lon: 1}
    points: ~
    during: {type: walk, points: [{id: gate, name: Gate, for_kids: Run}]}
",
        );
        let s = said(&with("", &content, None));
        for needle in [
            "places.museum.points[0]: has to be a mapping",
            "places.museum.points[1]: a point with no id cannot be linked to",
            "places.museum.points[1]: nothing written for a kid here",
            "places.museum.points[3].id: \"hall\" is used twice in this place",
            "places.museum.points[3]: a point needs a name",
            "places.museum.points[4].id: \"Hall\" is not an id",
            "places.park.during.type: \"walk\" is not one of the thirteen types. Did you mean walking?",
        ] {
            assert!(s.contains(needle), "wanted {needle:?} in:\n{s}");
        }
        assert!(!s.contains("points[2]"), "{s}");
        assert!(!s.contains("places.park.points"), "{s}");
    }

    #[test]
    fn a_point_may_carry_its_own_position_and_the_place_its_order() {
        let content = day_and(
            "places:
  castle:
    name: Castle
    at: {lat: 38.7, lon: -9.1, radius_m: 300}
    in_order: 'yes'
    points:
      - {id: gate, name: Gate, at: {lat: 38.701, lon: -9.1}}
      - {id: moat, name: Moat, at: {lat: 91, lon: -9.1}}
      - {id: far, name: Far, at: {lat: 38.72, lon: -9.1}}
      - {id: odd, name: Odd, at: here}
      - {id: well, name: Well, at: {lat: 38.7, lon: -9.1, radius_m: 30}}
  town:
    name: Town
    at: {lat: 38.7, lon: -9.1, radius_m: 5000}
    points: [{id: square, name: Square, at: {lat: 38.718, lon: -9.1}}]
  loose:
    name: Loose
    in_order: true
    points: [{id: a, name: A, at: {lat: 1, lon: 1}}]
",
        );
        let s = said(&with("", &content, None));
        for needle in [
            "places.castle.in_order: is true or false",
            "places.castle.points[1].at.lat: 91 is not a latitude",
            "places.castle.points[2].at: is 2226 m from its place, and the map kept offline reaches 1000 m around it",
            "places.castle.points[3].at: has to be a mapping with lat and lon",
            "places.castle.points[4].at.radius_m: a point has no radius, only where it is",
        ] {
            assert!(s.contains(needle), "wanted {needle:?} in:\n{s}");
        }
        assert!(!s.contains("points[0]") && !s.contains("places.loose.points"), "{s}");
        // Off the globe is one error, not a distance too; a wide place keeps its own circle.
        assert!(!s.contains("points[1].at:") && !s.contains("places.town"), "{s}");
        assert!(!s.contains("places.loose.in_order"), "{s}");
        // A place with no circle of its own has no offline map at all.
        let kept =
            "places.loose: its points have a position but it has none, so no map is kept for them";
        assert!(s.contains(kept), "{s}");
    }

    #[test]
    fn a_leg_is_whole_minutes_to_a_place_and_a_day_travels_one_of_three_ways() {
        let content = "days:
  - {date: 2026-04-11, title: A, travel: walking, blocks: [['10:00', 'Go.']]}
  - {date: 2026-04-12, title: B, travel: a pie, blocks: [['10:00', 'Go.']]}
  - {date: 2026-04-13, title: C, travel: swimming, blocks: [['10:00', 'Go.']]}
places:
  castle: {name: Castle, legs: {town: 20, moat: 0, gate: 7.5, nowhere: 10}}
  town: {name: Town, legs: soon}
  moat: {name: Moat}
  gate: {name: Gate}
";
        let manifest = "keymap:\n  values: {travel: {walking: a pie}}\n";
        let s = said(&with(manifest, content, None));
        for needle in [
            "places.castle.legs.moat: 0 is not whole minutes from 1 to 720",
            "places.castle.legs.gate: 7.5 is not whole minutes from 1 to 720",
            "places.castle.legs.nowhere: is not a place of this pack, so no leg uses it",
            "places.town.legs: has to be a mapping of place id to minutes",
            "days.2026-04-13.travel: \"swimming\" is not one of walking, driving, transit",
        ] {
            assert!(s.contains(needle), "wanted {needle:?} in:\n{s}");
        }
        assert!(!s.contains("legs.town") && !s.contains("2026-04-11.travel"), "{s}");
        assert!(!s.contains("2026-04-12.travel"), "{s}");
    }

    #[test]
    fn a_places_own_map_is_pins_on_a_picture_of_it() {
        let content = day_and(
            "places:
  gare:
    name: Gare
    at: {lat: 1, lon: 1}
    plan:
      image: ../../private/gare.png
      points:
        - text
        - {name: Platform 1, x: 0.2, y: 0.8}
        - {x: 1.4, y: -1}
  dock:
    name: Dock
    at: {lat: 1, lon: 1}
    plan: {image: maps/dock.png}
  slip:
    name: Slip
    at: {lat: 1, lon: 1}
    plan: {points: 3}
  pier:
    name: Pier
    at: {lat: 1, lon: 1}
    plan: nothing
",
        );
        let s = said(&with("", &content, None));
        for needle in [
            "places.gare.plan.image: \"../../private/gare.png\" leaves the pack folder",
            "places.gare.plan.points[0]: has to be a mapping with a name and where it sits",
            "places.gare.plan.points[2]: a pin needs a name",
            "places.gare.plan.points[2].x: 1.4 is not a fraction of the picture, 0 to 1",
            "places.gare.plan.points[2].y: -1 is not a fraction of the picture, 0 to 1",
            "places.dock.plan: no points, so the map of this place has nothing on it",
            "places.slip.plan.points: has to be a list of pins",
            "places.pier.plan: has to be a mapping: the pins, and an image to put them on",
        ] {
            assert!(s.contains(needle), "wanted {needle:?} in:\n{s}");
        }
        assert!(!s.contains("gare.plan.points[1]"), "{s}");
    }

    #[test]
    fn the_road_a_block_takes_is_named_stretches_in_order() {
        let content = "days:
  - date: 2026-04-11
    title: x
    blocks:
      - ['10:00', 'x', {type: driving, duration: 30 min, road: [{at: '10:20', name: The bridge}]}]
      - ['11:00', 'x', {type: driving, duration: 30 min, road: [text, {what: A castle}, {name: A pass, at: noon}]}]
      - ['12:00', 'x', {type: driving, duration: 30 min, road: 3}]
";
        let s = said(&with("", content, None));
        let at = "days.2026-04-11.blocks[1].road";
        for needle in [
            format!("{at}[0]: has to be a mapping with a name and what to look for"),
            format!("{at}[1]: a stretch of road needs a name"),
            format!("{at}[2].at: \"noon\" is not a time, HH:MM"),
            "blocks[2].road: has to be a list: what the road passes, in the order it passes it"
                .into(),
        ] {
            assert!(s.contains(&needle), "wanted {needle:?} in:\n{s}");
        }
        assert!(!s.contains("blocks[0]"), "{s}");
    }

    #[test]
    fn a_day_needs_a_real_date_once_and_a_title() {
        says("days:\n  - {blocks: [['10:00', 'x']]}", "days[0]: a day needs a date");
        says(
            "days:\n  - {date: 2026-04-11, blocks: [['10:00', 'x']]}",
            "days.2026-04-11: a day needs a title",
        );
        says(&format!("days:\n{DAY}{DAY}"), "days.2026-04-11.date: 2026-04-11 appears twice");
        says("days:\n  - {date: 2026-02-30, title: x}", "\"2026-02-30\" is not a real date");
        says("days:\n  - text", "days[0]: has to be a mapping");
        says("days:\n  - {date: 2026-04-11, title: x}", "no blocks and no options");
    }

    #[test]
    fn who_names_the_people_of_the_pack() {
        let content = "people: [{id: rita, name: Rita}]
days:
  - {date: 2026-04-11, title: x, who: [rita, nobody], fixed: [['10:00', 'x']]}
";
        let s = said(&with("", content, None));
        assert_eq!(s, "days.2026-04-11.who: \"nobody\" is not one of the people in this pack");
    }

    #[test]
    fn a_block_is_a_list_with_a_time_a_text_and_maybe_a_map() {
        let content = "days:
  - date: 2026-04-11
    title: x
    blocks:
      - {time: '10:00'}
      - ['10:00']
      - ['10:00', 'a', {}, 'd']
      - ['10:00', 'x', visit]
      - [1000, 'x']
      - ['10:00', '  ']
      - ['12:00', 'noon']
      - ['11:00', 'price [to confirm]']
      - ['', 'a note about the day']
";
        let s = said(&with("", content, None));
        let at = "days.2026-04-11.blocks";
        for needle in [
            format!("{at}[0]: a block is a list"),
            format!("{at}[1]: a block needs a time and a text"),
            format!("{at}[2]: 4 elements. A block is a time, a text, and at most one map"),
            format!("{at}[3]: the third element has to be a map"),
            format!("{at}[4]: 1000 is not a time. Quote it"),
            format!("{at}[5]: the second element is the text, and it cannot be empty"),
            format!("{at}[7]: 11:00 comes after 12:00 in the list but earlier in the day"),
            format!("{at}[7]: holds something to confirm, and will render as a hole"),
        ] {
            assert!(s.contains(&needle), "wanted {needle:?} in:\n{s}");
        }
        assert!(!s.contains("[8]"), "{s}");
        says("days:\n  - {date: 2026-04-11, title: x, blocks: text}", "has to be a list of blocks");
    }

    #[test]
    fn a_block_map_points_at_things_that_exist() {
        let content = "people: [{id: rita, name: Rita}]
places: {azulejo: {name: Museu, at: {lat: 38.72, lon: -9.11, radius_m: 120}}}
days:
  - date: 2026-04-11
    title: x
    blocks:
      - ['10:00', 'x', {type: visit, place: azulejo, guide: rita, for: [rita], until: '11:00', locked: true}]
      - ['12:00', 'x', {type: zzz, place: nowhere, guide: nobody, for: nobody, until: '11:00', locked: 'yes', leave: soon}]
      - ['13:00', 'x', {type: meal, until: noon, for: [rita, ~]}]
      - ['23:00', 'x', {until: '01:00', until_zone: Asia/Tokyo, leave: 20}]
";
        let s = said(&with("", content, None));
        let at = "days.2026-04-11.blocks[1]";
        for needle in [
            format!("{at}.type: \"zzz\" is not one of the thirteen types: visit, train, driving"),
            format!("{at}.place: \"nowhere\" is not a place in this pack"),
            format!("{at}.guide: \"nobody\" is not one of the people in this pack"),
            format!("{at}.for: \"nobody\" is not one of the people in this pack"),
            format!("{at}.until: 11:00 is not after the block's own time, 12:00"),
            format!("{at}.locked: is true or false"),
            format!("{at}.leave: is a whole number of minutes"),
            "blocks[2].until: \"noon\" is not a time, HH:MM".into(),
            "blocks[2].for: \"\" is not one of the people".into(),
        ] {
            assert!(s.contains(&needle), "wanted {needle:?} in:\n{s}");
        }
        assert!(!s.contains("blocks[0]") && !s.contains("blocks[3]"), "{s}");
        says(
            "days:\n  - {date: 2026-04-11, title: x, blocks: [['10:00', 'x', {place: azulejo}]]}",
            "this pack has no places, so \"azulejo\" points at nothing",
        );
    }

    #[test]
    fn a_typed_block_without_its_answer_is_warned() {
        let content = "days:
  - date: 2026-04-11
    title: x
    blocks:
      - ['09:00', 'x', {type: driving}]
      - ['10:00', 'x', {type: flight, boarding: ''}]
      - ['11:00', 'x', {type: walking, duration: 20 min}]
      - ['12:00', 'x', {type: visit}]
";
        let s = said(&with("", content, None));
        let at = "days.2026-04-11.blocks";
        for needle in [
            format!("{at}[0].duration: a driving block with no duration shows its start time"),
            format!("{at}[1].boarding: a flight block with no boarding"),
        ] {
            assert!(s.contains(&needle), "wanted {needle:?} in:\n{s}");
        }
        assert!(!s.contains("[2]") && !s.contains("[3]"), "{s}");
    }

    #[test]
    fn an_option_day_needs_two_options_one_recommended_and_a_decision() {
        let two =
            "      - {id: coast, name: The coast, recommended: true, blocks: [['10:00', 'Coast.']]}
      - {id: hill, name: The hill, blocks: [['10:00', 'Hill.']]}
";
        let day = |options: &str, decision: &str| {
            format!("days:\n  - date: 2026-04-11\n    title: x\n    options:\n{options}{decision}")
        };
        let asked = "    decision: {when: 2026-04-01, at: '21:00', decides: [2026-04-11]}\n";
        clean("", &day(two, asked), None);
        // A place only an option's block or a fixed one names is named all the same.
        let places = "places:\n  cove: {name: Cove}\n  pier: {name: Pier}\n";
        let tagged = two.replace("'Coast.'", "'Coast.', {place: cove}");
        let fixed = format!("{asked}    fixed: [['08:00', 'Out.', {{place: pier}}]]\n{places}");
        let s = said(&with("", &day(&tagged, &fixed), None));
        assert!(s.contains("places.pier: no coordinates") && !s.contains("no block names"), "{s}");

        let one = two.lines().next().unwrap().to_owned() + "\n";
        says(&day(&one, asked), "one option is not an option");
        says(&day(two, ""), "needs a decision block");
        says(&day(two, "    decision: {when: 2026-04-12}\n"), "before it stops being a question");
        says(
            &day(two, "    decision: {at: '9pm', decides: x}\n"),
            "required: the day the app asks",
        );
        says(&day(two, "    decision: {at: '9pm'}\n"), "decision.at: \"9pm\" is not a time");
        says(
            &day(two, "    decision: {when: 2026-04-01, decides: x}\n"),
            "has to be a list of dates",
        );
        says(&day(two, "    decision: {when: 2026-02-30}\n"), "\"2026-02-30\" is not a real date");
        let both = two.replace("{id: hill,", "{id: hill, recommended: true,");
        says(&day(&both, asked), "2 options are recommended. Only one can be");
        let neither = two.replace("recommended: true, ", "");
        says(&day(&neither, asked), "no option is recommended");
        let broken = "      - text\n      - {name: x, blocks: []}\n      - {id: Hill}\n      - {id: hill_2, name: x, blocks: []}\n      - {id: hill_2, name: x, blocks: []}\n";
        let s = said(&with("", &day(broken, asked), None));
        for needle in [
            "options[0]: has to be a mapping",
            "options[1]: an option needs an id",
            "options[2].id: \"Hill\" is not an id",
            "options[2]: an option needs a name",
            "options[2]: an option with no blocks is an idea",
            "options[4].id: \"hill_2\" is used twice on this day",
        ] {
            assert!(s.contains(needle), "wanted {needle:?} in:\n{s}");
        }
        says(
            &day("", "    decision: {when: 2026-04-01}\n").replace("options:\n", "options: x\n"),
            "has to be a list of closed alternatives",
        );
    }

    #[test]
    fn decides_and_requires_name_days_and_options_that_exist() {
        let content = "days:
  - date: 2026-04-11
    title: The fork
    options:
      - {id: coast, name: Coast, recommended: true, blocks: [['10:00', 'a']]}
      - {id: hill, name: Hill, blocks: [['10:00', 'b']]}
    decision: {when: 2026-04-01, decides: [2026-04-30]}
  - date: 2026-04-12
    title: The second fork
    options:
      - {id: near, name: Near, recommended: true, requires: {date: 2026-04-11, option: nope}, blocks: [['11:00', 'c']]}
      - {id: far, name: Far, requires: {date: 2026-04-11, option: coast}, blocks: [['11:00', 'd']]}
      - {id: mid, name: Mid, requires: x, blocks: [['11:00', 'e']]}
      - {id: odd, name: Odd, requires: {date: 2026-04-13}, blocks: [['11:00', 'f']]}
      - {id: gone, name: Gone, requires: {date: 2026-05-01}, blocks: [['11:00', 'g']]}
      - text
    decision: {when: 2026-04-10}
  - {date: 2026-04-13, title: Plain, blocks: [['10:00', 'x']]}
  - title: No date
    options: []
    decision: {decides: [2026-04-11, 2026-06-01]}
";
        let s = said(&with("", content, None));
        for needle in [
            "days.2026-04-11.decision.decides: \"2026-04-30\" is not a day in this pack",
            "days.2026-04-12.options[0].requires: \"nope\" is not an option on 2026-04-11. It has coast, hill",
            "days.2026-04-12.options[2].requires: has to be a mapping with date and option",
            "days.2026-04-12.options[3].requires: 2026-04-13 has no options, so nothing on it can be required",
            "days.2026-04-12.options[4].requires: \"2026-05-01\" is not a day in this pack",
            "days.?.decision.decides: \"2026-06-01\" is not a day in this pack",
        ] {
            assert!(s.contains(needle), "wanted {needle:?} in:\n{s}");
        }
        assert!(!s.contains("options[1].requires"), "{s}");
    }

    #[test]
    fn a_keymap_moves_the_root_and_the_keys_inside_a_day() {
        let keymap = "keymap:
  root: {days: itinerario.dias}
  day: {date: fecha, title: titulo, blocks: bloques}
";
        let content = "itinerario:
  dias:
    - {fecha: 2026-04-11, titulo: Un dia, bloques: [['10:00', 'Algo pasa.']]}
";
        clean(keymap, content, None);
    }

    #[test]
    fn an_alert_needs_an_id_a_title_and_a_severity_the_shell_knows() {
        let alert = "{id: ferry, title: The last ferry, severity: high, at: 2026-04-11T18:40, notify_from: 2026-04-10, time: '18:00'}";
        clean("", &day_and(&format!("alerts:\n  - {alert}\n")), None);
        let content = day_and(&format!(
            "alerts:
  - {alert}
  - {alert}
  - text
  - {{severity: urgent, at: soon, time: '6pm'}}
  - {{id: x, title: x}}
"
        ));
        let s = said(&with("", &content, None));
        for needle in [
            "alerts[1].id: \"ferry\" is used twice",
            "alerts[2]: has to be a mapping",
            "alerts[3]: an alert needs an id",
            "alerts[3]: an alert needs a title",
            "alerts[3].severity: \"urgent\" is not a severity. Use critical, high, medium, low",
            "alerts[3].at: \"soon\" is not a date or a timestamp",
            "alerts[3].time: \"6pm\" is not a time, HH:MM",
            "alerts[4]: an alert needs a severity: critical, high, medium, low",
        ] {
            assert!(s.contains(needle), "wanted {needle:?} in:\n{s}");
        }
        says(&day_and("alerts: x\n"), "alerts: has to be a list");
    }

    #[test]
    fn severity_is_a_budget() {
        let one = "  - {id: a, title: x, severity: critical}\n";
        let seven: String = (0..7).map(|i| one.replace("id: a", &format!("id: a{i}"))).collect();
        says(&day_and(&format!("alerts:\n{seven}")), "alerts: 7 alerts are critical");
        let six: String = seven.lines().skip(1).map(|l| format!("{l}\n")).collect();
        clean("", &day_and(&format!("alerts:\n{six}")), None);
    }

    #[test]
    fn a_severity_in_another_language_passes_once_mapped() {
        let content = day_and("alerts: [{id: ferry, title: El ferry, severity: alta}]\n");
        clean("keymap:\n  values:\n    severity: {high: alta}\n", &content, None);
        says(&content, "\"alta\" is not a severity");
    }

    #[test]
    fn an_alert_may_be_done_repeat_and_ask_for_an_alarm() {
        let ok = "{id: tide, title: Tide, severity: low, at: 2026-04-11, time: '09:00', status: open, alarm: true, repeat: {every: 1, until: 2026-04-13}}";
        clean("", &day_and(&format!("alerts:\n  - {ok}\n")), None);
        let content = day_and(
            "alerts:
  - {id: a, title: x, severity: low, status: shut, alarm: true}
  - {id: b, title: x, severity: low, at: 2026-04-11, alarm: 'yes', repeat: {every: 1.5, until: 2026-04-10}}
  - {id: c, title: x, severity: low, repeat: {every: 2, until: 2026-04-12}}
  - {id: d, title: x, severity: low, at: 2026-04-11, repeat: weekly}
  - {id: e, title: x, severity: low, notify_from: 2026-04-11T08:00, alarm: true}
",
        );
        let s = said(&with("", &content, None));
        for needle in [
            "alerts[0].status: \"shut\" is not one of open, partial, done",
            "alerts[0].alarm: an alarm needs a time to ring at",
            "alerts[1].alarm: is true or false",
            "alerts[1].repeat.every: 1.5 is not a whole number of days",
            "alerts[1].repeat.until: \"2026-04-10\" is not a real date on or after 2026-04-11",
            "alerts[2].repeat: a repeat needs the alert's at",
            "alerts[3].repeat: has to be a mapping",
        ] {
            assert!(s.contains(needle), "wanted {needle:?} in:\n{s}");
        }
        assert!(!s.contains("alerts[4]"), "{s}");
        let content = day_and("alerts: [{id: a, title: x, severity: low, status: hecho}]\n");
        clean("keymap:\n  values:\n    status: {done: hecho}\n", &content, None);
    }

    #[test]
    fn a_jet_lag_day_has_a_date_and_steps_that_say_what_to_do() {
        let people = "people: [{id: rita, name: Rita}]\n";
        let ok = "{date: 2026-04-11, zone: Europe/Madrid, steps: [{time: '07:00', until: '09:00', text: Up, do: wake, for: rita, alarm: true}, {text: Water}]}";
        clean("", &day_and(&format!("{people}jet_lag:\n  - {ok}\n")), None);
        let content = day_and(&format!(
            "{people}jet_lag:
  - {ok}
  - {ok}
  - text
  - {{date: 2026-02-30, steps: x}}
  - {{date: 2026-04-12, steps: [x, {{time: 7am, until: '9', do: nap, for: [nobody], alarm: 'yes'}}, {{text: y, alarm: true}}]}}
"
        ));
        let s = said(&with("", &content, None));
        for needle in [
            "jet_lag.2026-04-11.date: 2026-04-11 appears twice",
            "jet_lag[2]: has to be a mapping",
            "jet_lag[3]: \"2026-02-30\" is not a real date",
            "jet_lag[3]: a jet-lag day needs steps, a list",
            "jet_lag.2026-04-12.steps[0]: has to be a mapping",
            "jet_lag.2026-04-12.steps[1]: a step needs a text",
            "jet_lag.2026-04-12.steps[1].time: \"7am\" is not a time, HH:MM",
            "jet_lag.2026-04-12.steps[1].until: \"9\" is not a time, HH:MM",
            "jet_lag.2026-04-12.steps[1].do: \"nap\" is not one of wake, bed, sleep",
            "jet_lag.2026-04-12.steps[1].for: \"nobody\" is not one of the people",
            "jet_lag.2026-04-12.steps[1].alarm: is true or false",
            "jet_lag.2026-04-12.steps[2].alarm: an alarm needs the step's time",
        ] {
            assert!(s.contains(needle), "wanted {needle:?} in:\n{s}");
        }
        says(&day_and("jet_lag: x\n"), "jet_lag: has to be a list, one entry per day");
        let mapped = day_and("jet_lag: [{date: 2026-04-11, steps: [{text: x, do: cafe}]}]\n");
        clean("keymap:\n  values:\n    do: {coffee: cafe}\n", &mapped, None);
    }

    #[test]
    fn a_task_has_an_id_a_title_and_maybe_a_deadline_and_a_status() {
        let ok = "{id: tickets, title: Tickets, deadline: 2026-10-01, status: partial, who: Rita}";
        clean("", &day_and(&format!("tasks:\n  - {ok}\n  - {{id: bags, title: Bags}}\n")), None);
        let content = day_and(&format!(
            "tasks:
  - {ok}
  - {ok}
  - text
  - {{deadline: soon, status: later}}
  - {{id: 'not an id', title: x}}
"
        ));
        let s = said(&with("", &content, None));
        for needle in [
            "tasks[1].id: \"tickets\" is used twice",
            "tasks[2]: has to be a mapping",
            "tasks[3]: a task needs an id",
            "tasks[3]: a task needs a title",
            "tasks[3].deadline: \"soon\" is not a real date",
            "tasks[3].status: \"later\" is not one of open, partial, done",
            "tasks[4].id: \"not an id\"",
        ] {
            assert!(s.contains(needle), "wanted {needle:?} in:\n{s}");
        }
        says(&day_and("tasks: x\n"), "tasks: has to be a list");
    }

    #[test]
    fn a_document_needs_an_id_a_title_and_a_file() {
        let doc = "{id: passport_rita, title: Passport, file: files/passport.pdf, for: rita, fields: {number: X1}, call: {Help: '+1 (555) 010-0100', Emergency: '112'}}";
        let people = "people: [{id: rita, name: Rita}]\n";
        clean("", &day_and(&format!("{people}documents:\n  - {doc}\n")), None);
        let content = day_and(&format!(
            "{people}documents:
  - {doc}
  - {doc}
  - text
  - {{id: Bad, for: nobody, fields: x}}
  - {{}}
  - {{id: out, title: T, file: ../x.pdf, call: {{Desk: '12', Home: 'ask', Plus: '+', Deep: [1]}}}}
  - {{id: flat, title: T, file: x.pdf, call: '112'}}
"
        ));
        let s = said(&with("", &content, None));
        for needle in [
            "documents[1].id: \"passport_rita\" is used twice",
            "documents[2]: has to be a mapping",
            "documents[3].id: \"Bad\" is not an id",
            "documents[3]: a document needs a title",
            "documents[3]: a document needs a file",
            "documents[3].for: \"nobody\" is not one of the people in this pack",
            "documents[3].fields: has to be a mapping of label to value",
            "documents[4]: a document needs an id",
            "documents[5].file: \"../x.pdf\" leaves the pack folder",
            "documents[5].call.Desk: \"12\" is not a number to dial",
            "documents[5].call.Home: \"ask\" is not a number to dial",
            "documents[5].call.Plus: \"+\" is not a number to dial",
            "documents[5].call.Deep: [1] is not a number to dial",
            "documents[6].call: has to be a mapping of label to number",
        ] {
            assert!(s.contains(needle), "wanted {needle:?} in:\n{s}");
        }
        says(&day_and("documents: x\n"), "documents: has to be a list");
    }

    #[test]
    fn the_theme_has_every_token_readable() {
        clean("", &day_and(""), Some(&palette("")));
        let r = validate(&yaml(MANIFEST), &yaml(&day_and("")), Some(&Value::Null));
        assert_eq!(said(&r), "");
        let broken = [
            ("ink: DELETE", "missing 1 of the 23 tokens: ink."),
            ("ink: '#dddddd'", "ink on paper is 1.36:1, under 4.5:1"),
            ("ink: black", "theme.themes.plain.colors.ink: \"black\" is not a hex color"),
            ("sparkle: '#ff00ff'", "not a token the shell knows: sparkle"),
            ("ink: '#00000010'", "ink on paper is 1.15:1"),
        ];
        for (over, needle) in broken {
            let s = said(&with("", &day_and(""), Some(&palette(over))));
            assert!(s.contains(needle), "{over}: wanted {needle:?} in:\n{s}");
        }
        let cases = [
            ("x: 1", "a theme file needs a \"themes:\" mapping"),
            (
                "default: nope\nthemes: {Plain: x}",
                "theme.default: \"nope\" is not one of the themes: Plain",
            ),
            ("themes: {Plain: x}", "theme.themes.Plain: \"Plain\" is not a slug"),
            ("themes: {plain: x}", "theme.themes.plain: has to be a mapping"),
            ("themes: {plain: {}}", "a theme needs a name"),
            ("themes: {plain: {mode: sepia}}", "theme.themes.plain.mode: has to be light or dark"),
            ("themes: {plain: {name: x, mode: dark}}", "a theme needs all 23 color tokens"),
        ];
        for (theme, needle) in cases {
            let s = said(&with("", &day_and(""), Some(theme)));
            assert!(s.contains(needle), "{theme}: wanted {needle:?} in:\n{s}");
        }
        let s = said(&with("", &day_and(""), Some(&format!("default: plain\n{}", palette("")))));
        assert_eq!(s, "");
    }

    #[test]
    fn unknown_keys_are_the_feature_with_two_exceptions() {
        clean(
            "",
            "days:\n  - {date: 2026-04-11, title: x, fixed: [], what_we_learned: Two.}",
            None,
        );
        says(
            "days:\n  - {date: 2026-04-11, title: x, fixed: [], note: 'price [to confirm]'}",
            "renders as a hole on purpose",
        );
        says(
            "days:\n  - {date: 2026-04-11, title: x, fixed: [], closed__2026_02_30: never}",
            "ends in a date that does not exist",
        );
        let hidden = "conventions: {hidden_prefixes: [source]}\nkeymap: {day: {title: titulo}}\n";
        clean(
            hidden,
            "days:\n  - {date: 2026-04-11, titulo: x, fixed: [], source_note: '[to confirm]', id: d}",
            None,
        );
    }

    #[test]
    fn climate_entries_are_checked_against_their_shape_and_the_places() {
        let places = "places:\n  lisbon: {name: Lisbon, at: {lat: 38.7, lon: -9.1}}\n";
        let cases = [
            ("climate: [x]", "climate: has to be a mapping with units and entries"),
            ("climate: {units: kelvin}", "climate.units: \"kelvin\" is not metric or imperial"),
            ("climate: {entries: x}", "climate.entries: has to be a list"),
            ("climate: {entries: [x]}", "climate.entries[0]: has to be a mapping"),
            (
                "climate: {entries: [{place: porto}]}",
                "climate.entries[0].place: \"porto\" is not a place in this pack",
            ),
            (
                "climate: {entries: [{month: 4, date: 2026-04-11}]}",
                "climate.entries[0]: a month or a date, not both",
            ),
            (
                "climate: {entries: [{month: 13}]}",
                "climate.entries[0].month: 13 is not a month, 1 to 12",
            ),
            (
                "climate: {entries: [{month: 4.5}]}",
                "climate.entries[0].month: 4.5 is not a month, 1 to 12",
            ),
            (
                "climate: {entries: [{month: april}]}",
                "climate.entries[0].month: \"april\" is not a month, 1 to 12",
            ),
            (
                "climate: {entries: [{date: 2026-02-30}]}",
                "climate.entries[0].date: \"2026-02-30\" is not a real date",
            ),
            (
                "climate: {entries: [{high: warm}]}",
                "climate.entries[0].high: \"warm\" is not a number",
            ),
            (
                "climate: {entries: [{sunset: '25:00'}]}",
                "climate.entries[0].sunset: \"25:00\" is not a time, HH:MM",
            ),
        ];
        for (climate, needle) in cases {
            says(&day_and(&format!("{places}{climate}")), needle);
        }
        let good = "climate:\n  units: imperial\n  entries:\n    - {place: lisbon, month: 4, high: 68, low: 54, rain: 35, sunrise: '06:55', sunset: '20:10', summary: Spring}\n    - {date: 2026-04-11, high: 73}\n";
        clean("", &day_and(&format!("{places}{good}")), None);
        clean("", &day_and("climate: {}"), None);
    }

    #[test]
    fn the_definition_is_part_of_the_report() {
        let r = with("modules: {weather: {}}\n", &day_and(""), None);
        assert_eq!(r.errors[0].at, "modules.weather");
    }
}
