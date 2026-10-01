//! What the family keeps as the trip goes (decision 0034): the car, where it was left, and notes.
//! All of it is facts, so another phone takes it with the rest.

use super::{Run, record};
use crate::value::{Map, Value, text};

/// The car as the log keeps it, each fact under `log.<name>`.
const CAR: [&str; 5] = ["plate", "model", "fuel", "km", "spot"];

impl Run<'_> {
    /// `log` is the car's `plate`, `model`, `fuel`, `km` and the `spot` it was left in, when it
    /// was `parked`, and the `notes`, newest first, each `{when, text}`. A note is the fact
    /// `note.<now.stamp>.<n>`, one each, so two phones' notes add up.
    pub(super) fn log(&mut self) {
        let store = &self.world.store;
        let fact = |k: &str| store.get(&format!("log.{k}")).cloned().unwrap_or(Value::Null);
        let mut log = Map(CAR.iter().map(|k| ((*k).to_owned(), fact(k))).collect());
        let when = |stamp: &str| Value::String(stamp.replacen('T', " ", 1));
        log.0.push(("parked".to_owned(), when(&text(store.get("log.parked")))));
        let mut notes: Vec<_> = store
            .iter()
            .filter_map(|(k, v)| {
                let rest = k.strip_prefix("note.")?;
                Some((rest.rsplit_once('.').unwrap_or((rest, "")), v))
            })
            .collect();
        // Newest first; within a minute by the count after it, a number.
        notes.sort_by(|((a, m), _), ((b, n), _)| (b, n.len(), n).cmp(&(a, m.len(), m)));
        let notes =
            notes.into_iter().map(|((at, _), v)| record([("when", when(at)), ("text", v.clone())]));
        log.0.push(("notes".to_owned(), Value::List(notes.collect())));
        self.set("log", Value::Map(log));
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::World;
    use crate::modules::tests::run;
    use crate::yaml::parse;

    #[test]
    fn the_log_is_the_cars_facts_and_the_notes_newest_first() {
        let store = parse(
            r#"{log.plate: 1234 ABC, log.fuel: 3/4, log.parked: "2026-10-03T10:15", car: {lat: 1},
              "note.2026-10-03T10:15.9": nine, "note.2026-10-03T10:15.10": ten,
              "note.2026-10-04T08:00.0": later, note.odd: odd, notes: x}"#,
        )
        .unwrap();
        let world = World {
            now: "2026-10-04T10:00".into(),
            store: store.as_map().cloned().unwrap(),
            ..World::default()
        };
        assert_eq!(
            run("log", "", "", &world, "").at("log"),
            concat!(
                r#"{"plate": "1234 ABC", "model": null, "fuel": "3/4", "km": null, "spot": null, "#,
                r#""parked": "2026-10-03 10:15", "notes": [{"when": "odd", "text": "odd"}, "#,
                r#"{"when": "2026-10-04 08:00", "text": "later"}, {"when": "2026-10-03 10:15", "text": "ten"}, "#,
                r#"{"when": "2026-10-03 10:15", "text": "nine"}]}"#
            )
        );
    }
}
