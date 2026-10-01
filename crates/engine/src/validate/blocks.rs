//! Blocks, block types and the free keys every entry may carry.

use super::Checker;
use super::patterns::{dated_key, is_time, to_confirm};
use crate::value::{Map, Value, quote, show, text, truthy};

pub const TYPES: [&str; 13] = [
    "visit", "train", "driving", "walking", "meal", "event", "flight", "parking", "lodging",
    "night", "morning", "transfer", "free",
];

/// The key a type's answer comes from, the big value at the top of a moment. Parking is not here:
/// its answer is the place's price, or free.
const ANSWERS: [(&str, &str); 5] = [
    ("driving", "duration"),
    ("walking", "duration"),
    ("flight", "boarding"),
    ("lodging", "check_in"),
    ("free", "until"),
];

impl Checker<'_> {
    pub(super) fn blocks(&mut self, at: &str, blocks: &Value) {
        let Some(blocks) = blocks.as_list() else {
            self.r.error(
                at,
                "has to be a list of blocks, each one a list: a time, a text, and optionally a map",
            );
            return;
        };
        let mut last = String::new();
        for (i, b) in blocks.iter().enumerate() {
            let bat = format!("{at}[{i}]");
            let Some(b) = b.as_list() else {
                self.r
                    .error(&bat, "a block is a list: [\"09:00\", \"what happens\", {type: visit}]");
                continue;
            };
            if b.len() < 2 {
                self.r.error(&bat, "a block needs a time and a text");
                continue;
            }
            if b.len() > 3 {
                let message =
                    format!("{} elements. A block is a time, a text, and at most one map", b.len());
                self.r.error(&bat, message);
                continue;
            }
            let t = text(Some(&b[0]));
            let timed = is_time(&t);
            if !t.is_empty() && !timed {
                let message = format!(
                    "{} is not a time. Quote it, \"09:00\", on a 24 hour clock. An empty time is a note about the whole day",
                    show(Some(&b[0]))
                );
                self.r.error(&bat, message);
            }
            if !matches!(&b[1], Value::String(s) if !s.trim().is_empty()) {
                self.r.error(&bat, "the second element is the text, and it cannot be empty");
            }
            if timed && !last.is_empty() && t < last {
                let message = format!(
                    "{t} comes after {last} in the list but earlier in the day. The reader takes the last block whose time has passed, so order matters"
                );
                self.r.warn(&bat, message);
            }
            if matches!(&b[1], Value::String(s) if to_confirm(s)) {
                self.r.warn(
                    &bat,
                    "holds something to confirm, and will render as a hole until it is confirmed",
                );
            }
            if timed {
                last = t.clone();
            }
            match b.get(2) {
                None => {}
                Some(Value::Map(meta)) => self.meta(&bat, meta, &t),
                Some(_) => self.r.error(
                    &bat,
                    "the third element has to be a map: {type: visit, place: azulejo}. This is the single most common mistake in a pack",
                ),
            }
        }
    }

    fn meta(&mut self, bat: &str, meta: &Map, t: &str) {
        let keymap = self.keymap;
        let read = |key: &str| keymap.field(meta, "block", key);
        if let Some(kind) = read("type") {
            self.block_type(&format!("{bat}.type"), kind);
            let canon = self.keymap.value("type", Some(kind));
            if let Some((_, key)) = ANSWERS.iter().find(|(t, _)| *t == canon)
                && !truthy(read(key))
            {
                let message = format!(
                    "a {canon} block with no {key} shows its start time where the {key} would be"
                );
                self.r.warn(format!("{bat}.{key}"), message);
            }
        }
        let place = text(read("place"));
        self.shown.insert(place.clone());
        if !place.is_empty() && self.places.is_empty() {
            let message = format!(
                "this pack has no places, so {} points at nothing and the screen keeps only the block's own text",
                quote(&place)
            );
            self.r.warn(format!("{bat}.place"), message);
        } else if !place.is_empty() && !self.places.contains(&place) {
            self.r.error(
                format!("{bat}.place"),
                format!("{} is not a place in this pack", quote(&place)),
            );
        }
        self.whom(&format!("{bat}.for"), read("for"));
        self.person(&format!("{bat}.guide"), &text(read("guide")));
        let until = text(read("until"));
        if !until.is_empty() && !is_time(&until) {
            let message = format!("{} is not a time, HH:MM", quote(&until));
            self.r.error(format!("{bat}.until"), message);
        }
        for key in ["zone", "until_zone"] {
            self.zone(&format!("{bat}.{key}"), read(key));
        }
        // An `until` in another zone can read earlier than the block and still be after it.
        if !until.is_empty() && !t.is_empty() && until.as_str() <= t && read("until_zone").is_none()
        {
            let message = format!("{until} is not after the block's own time, {t}");
            self.r.error(format!("{bat}.until"), message);
        }
        if read("locked").is_some_and(|l| !matches!(l, Value::Bool(_))) {
            self.r.error(
                format!("{bat}.locked"),
                "is true or false: an hour that cannot move, or one that can",
            );
        }
        let minutes = |l: &Value| matches!(l, Value::Number(n) if n.fract() == 0.0 && *n > 0.0);
        if read("leave").is_some_and(|l| !minutes(l)) {
            self.r.error(
                format!("{bat}.leave"),
                "is a whole number of minutes: how long before its time you have to set off",
            );
        }
        if let Some(road) = read("road") {
            self.road(bat, road);
        }
    }

    /// What a moving block passes on the way, in the order it passes it (decision 0026).
    fn road(&mut self, bat: &str, road: &Value) {
        let keymap = self.keymap;
        let at = format!("{bat}.road");
        let Some(steps) = road.as_list() else {
            self.r.error(&at, "has to be a list: what the road passes, in the order it passes it");
            return;
        };
        for (i, step) in steps.iter().enumerate() {
            let at = format!("{at}[{i}]");
            let Some(step) = step.as_map() else {
                self.r.error(&at, "has to be a mapping with a name and what to look for");
                continue;
            };
            if !truthy(keymap.field(step, "road", "name")) {
                self.r.error(&at, "a stretch of road needs a name");
            }
            let when = text(keymap.field(step, "road", "at"));
            if !when.is_empty() && !is_time(&when) {
                let message = format!("{} is not a time, HH:MM", quote(&when));
                self.r.error(format!("{at}.at"), message);
            }
        }
    }

    /// A reference to a person, checked only when the pack lists people.
    pub(super) fn person(&mut self, at: &str, id: &str) {
        if !id.is_empty() && !self.people.is_empty() && !self.people.contains(id) {
            self.r.error(at, format!("{} is not one of the people in this pack", quote(id)));
        }
    }

    /// A `for`: one id or a list of them, each one of the people when the pack lists any.
    pub(super) fn whom(&mut self, at: &str, ids: Option<&Value>) {
        let ids = match ids {
            Some(Value::List(ids)) => ids.as_slice(),
            Some(one) => std::slice::from_ref(one),
            None => &[],
        };
        for id in ids.iter().map(|id| text(Some(id))) {
            if !self.people.is_empty() && !self.people.contains(&id) {
                let message = format!("{} is not one of the people in this pack", quote(&id));
                self.r.error(at, message);
            }
        }
    }

    pub(super) fn block_type(&mut self, at: &str, value: &Value) {
        let canon = self.keymap.value("type", Some(value));
        if TYPES.contains(&canon.as_str()) {
            self.types.insert(canon);
            return;
        }
        let stem: String = canon.chars().take(3).collect();
        let near = TYPES.iter().find(|t| !stem.is_empty() && t.starts_with(&stem));
        let hint =
            near.map_or(format!(": {}", TYPES.join(", ")), |n| format!(". Did you mean {n}?"));
        self.r.error(at, format!("{} is not one of the thirteen types{hint}", show(Some(value))));
    }

    /// Unknown keys are the point, so this reports only the two cases worth a word: a date suffix
    /// that no calendar has, and a value still to confirm.
    pub(super) fn free_keys(&mut self, at: &str, obj: &Map, known: &[&str], kind: &str) {
        let renames = self.keymap.renames(kind);
        for (key, value) in obj.iter() {
            if known.contains(&key) || renames.iter().any(|r| r == key) || key == "id" {
                continue;
            }
            if self.hidden.iter().any(|p| key.starts_with(p.as_str())) {
                continue;
            }
            if dated_key(key) == Some(false) {
                self.r.error(
                    format!("{at}.{key}"),
                    "ends in a date that does not exist, so it would never show",
                );
            }
            if matches!(value, Value::String(s) if to_confirm(s)) {
                self.r.warn(
                    format!("{at}.{key}"),
                    "holds something to confirm, and renders as a hole on purpose",
                );
            }
        }
    }
}
