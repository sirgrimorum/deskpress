//! Templates: a definition bundled with the engine that a pack extends with `pack.extends`, so a
//! pack of a known kind brings only its content, its theme and what it changes.

use crate::value::{Value, show};
use crate::yaml;

/// Each template by name, and its manifest without a `pack` head.
const TEMPLATES: [(&str, &str); 1] =
    [("travel", include_str!("../../../templates/travel/pack.yaml"))];

/// The sections merged by key: the pack's entry replaces the template's where it stands, and a
/// new one is added at the end.
const BY_KEY: [&str; 4] = ["modules", "derive", "screens", "ui"];

/// `manifest` over the template it extends, or as it is when it extends none. A section of the
/// wrong shape is kept as the pack wrote it, for the validator to report.
pub fn extend(manifest: Value) -> Result<Value, String> {
    let Value::Map(pack) = manifest else {
        return Ok(manifest);
    };
    let Some(extends) = pack.get("pack").and_then(|h| h.get("extends")) else {
        return Ok(Value::Map(pack));
    };
    let named = |(n, _): &&(&str, &str)| matches!(extends, Value::String(s) if s == n);
    let Some((_, source)) = TEMPLATES.iter().find(named) else {
        let names: Vec<&str> = TEMPLATES.iter().map(|(n, _)| *n).collect();
        let message = format!(
            "pack.extends: {} is not a template: {}",
            show(Some(extends)),
            names.join(", ")
        );
        return Err(message);
    };
    // A bundled template parses to a mapping; a test holds every one to it.
    let base = yaml::parse(source).unwrap_or(Value::Null);
    let mut merged = base.as_map().cloned().unwrap_or_default();
    for (key, theirs) in pack.0 {
        let value = match (merged.get(&key), theirs) {
            (Some(Value::Map(base)), Value::Map(over)) if BY_KEY.contains(&key.as_str()) => {
                let mut base = base.clone();
                for (k, v) in over.0 {
                    base.set(&k, v);
                }
                Value::Map(base)
            }
            // The pack's rules are tried first; the template's close the table.
            (Some(Value::List(base)), Value::List(mut over)) if key == "rules" => {
                over.extend(base.iter().cloned());
                Value::List(over)
            }
            (_, theirs) => theirs,
        };
        merged.set(&key, value);
    }
    Ok(Value::Map(merged))
}

/// A template's manifest by name, for the tests and the docs.
#[cfg(test)]
fn source(name: &str) -> Value {
    let (_, source) = TEMPLATES.iter().find(|(n, _)| *n == name).unwrap();
    yaml::parse(source).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::define::define;
    use crate::yaml::parse;

    fn over(pack: &str) -> Result<Value, String> {
        extend(parse(pack).unwrap())
    }

    const HEAD: &str =
        "pack: {id: t, name: T, language: en, timezone: UTC, content: c.yaml, extends: travel}\n";

    #[test]
    fn an_unknown_template_is_refused_and_named() {
        let e = over("pack: {extends: blog}").unwrap_err();
        assert_eq!(e, "pack.extends: \"blog\" is not a template: travel");
        let e = over("pack: {extends: [travel]}").unwrap_err();
        assert!(e.starts_with("pack.extends: [\"travel\"] is not"), "{e}");
    }

    #[test]
    fn a_manifest_that_extends_nothing_comes_back_as_it_is() {
        for m in ["pack: {id: t}", "- 1", "pack: x"] {
            let v = parse(m).unwrap();
            assert_eq!(extend(v.clone()).unwrap(), v);
        }
    }

    #[test]
    fn a_section_of_the_wrong_shape_is_kept_for_the_validator() {
        let m = over(&format!("{HEAD}rules: x\nui: [a]\n")).unwrap();
        assert_eq!(m.get("rules"), Some(&Value::String("x".into())));
        assert_eq!(m.get("ui"), parse("ui: [a]").unwrap().get("ui"));
    }

    #[test]
    fn the_pack_wins_by_key_where_the_template_had_it() {
        let m = over(&format!(
            "{HEAD}ui: {{today: Hoy, extra: x}}\nconventions: {{alert_prefixes: [ojo]}}\n"
        ))
        .unwrap();
        let ui = m.get("ui").and_then(Value::as_map).unwrap();
        let base = source("travel");
        let keys: Vec<&str> = ui.keys().collect();
        let mut expected: Vec<&str> =
            base.get("ui").and_then(Value::as_map).unwrap().keys().collect();
        expected.push("extra");
        assert_eq!(keys, expected);
        assert_eq!(ui.get("today"), Some(&Value::String("Hoy".into())));
        let conventions = parse("c: {alert_prefixes: [ojo]}").unwrap();
        assert_eq!(m.get("conventions"), conventions.get("c"));
        assert_eq!(m.get("pack").and_then(|h| h.get("id")), Some(&Value::String("t".into())));
    }

    #[test]
    fn the_pack_rules_come_before_the_template_rules() {
        let m = over(&format!("{HEAD}rules: [{{when: 'now.time > \"23:00\"', screen: days}}]\n"))
            .unwrap();
        let rules = m.get("rules").and_then(Value::as_list).unwrap();
        let base = source("travel");
        assert_eq!(rules.len(), base.get("rules").and_then(Value::as_list).unwrap().len() + 1);
        assert_eq!(rules[0].get("screen"), Some(&Value::String("days".into())));
    }

    #[test]
    fn every_template_defines_clean() {
        for (name, _) in TEMPLATES {
            let m = over(&HEAD.replace("travel", name)).unwrap();
            let (_, r) = define(&m);
            assert!(r.errors.is_empty() && r.warnings.is_empty(), "{name}: {r:?}");
        }
    }

    /// The screen the travel template shows for a pack of one day, and the text of its nodes.
    fn shown(now: &str, holder: &str, store: &[(&str, &str)]) -> (String, Vec<String>) {
        use crate::engine::{Engine, Nav, World};
        use crate::pack::Pack;
        let manifest = over(&format!(
            "{HEAD}conventions: {{hidden_prefixes: [source]}}
"
        ))
        .unwrap();
        let content = parse(TRIP).unwrap();
        let (engine, _) = Engine::load(Pack { manifest, content, theme: None }).unwrap();
        let store = store.iter().map(|(k, v)| ((*k).to_owned(), Value::String((*v).to_owned())));
        let world = World {
            now: now.into(),
            holder: holder.into(),
            inside: vec![],
            store: crate::value::Map(store.collect()),
        };
        let tree = engine.screen(&world, &mut Nav::default()).unwrap().tree;
        let texts = tree.nodes.iter().map(|n| {
            let prop = |k: &str| {
                n.props.iter().find(|(p, _)| p == k).map(|(_, v)| crate::value::text(Some(v)))
            };
            format!("{} {}", n.kind, prop("text").or_else(|| prop("label")).unwrap_or_default())
        });
        (tree.screen, texts.collect())
    }

    const TRIP: &str = r#"people: [{id: ana, name: Ana, adult: true}, {id: leo, name: Leo, adult: false}]
days:
  - date: 2026-04-11
    title: One
    blocks:
      - ["", "A note"]
      - ["10:00", "Park", {type: parking, place: lot}]
      - ["11:00", "Museum", {type: visit, place: museum, guide: leo, until: "12:00"}]
      - ["13:00", "Drive", {type: driving, place: lot, duration: 40 min, tolls: none}]
  - date: 2026-04-12
    title: Two
    sleeps_at: Home
    blocks: [["", "Rest"], ["09:00", "Leave"]]
places:
  lot: {name: Lot, parking: {where: North side, price: "[to confirm]"}}
  museum:
    name: Museum
    during:
      type: visit
      hours: 10 to 18
      ticket: doc_1
      points: [{name: Hall, for_kids: Count the lions}]
      source_web: x
"#;

    #[test]
    fn the_travel_template_picks_a_screen_for_every_moment_of_a_day() {
        let screen = |now, holder| shown(now, holder, &[]).0;
        assert_eq!(screen("2026-04-20T10:00", ""), "days");
        assert_eq!(screen("2026-04-11T09:00", ""), "morning");
        assert_eq!(screen("2026-04-11T10:30", ""), "moment");
        assert_eq!(screen("2026-04-11T11:30", "leo"), "suggestion");
        assert_eq!(screen("2026-04-11T10:30", "leo"), "complete");
        assert_eq!(screen("2026-04-11T12:30", ""), "agenda");
        assert_eq!(screen("2026-04-11T21:30", ""), "night");
        // A relay stores who holds the phone when the host does not say.
        assert_eq!(shown("2026-04-11T11:30", "", &[("holder", "leo")]).0, "suggestion");
    }

    #[test]
    fn the_moment_shows_the_answer_of_its_type_and_what_the_pack_added() {
        let (_, parking) = shown("2026-04-11T10:30", "", &[]);
        assert_eq!(parking[3], "Missing [to confirm]");
        assert!(parking.contains(&"Card North side".to_owned()), "{parking:?}");
        let (_, driving) = shown("2026-04-11T13:30", "", &[]);
        assert_eq!(driving[3], "BigValue 40 min");
        let parking = "Missing where: North side\nprice: [to confirm]";
        assert!(driving.contains(&parking.to_owned()), "{driving:?}");
        assert!(driving.contains(&"Card none".to_owned()), "{driving:?}");
        assert_eq!(driving.last().unwrap(), "Label Nothing more today");
        let (_, visit) = shown("2026-04-11T11:30", "ana", &[]);
        assert!(visit.contains(&"Card 10 to 18".to_owned()), "{visit:?}");
        assert!(!visit.iter().any(|t| t.ends_with(" x")), "{visit:?}");
        assert!(!visit.iter().any(|t| t.contains("doc_1")), "{visit:?}");
        assert!(visit.contains(&"Button Point by point".to_owned()));
        assert!(visit.contains(&"Button The ticket".to_owned()));
        let (_, night) = shown("2026-04-11T21:30", "", &[]);
        assert_eq!(night[1..3], ["BigValue 09:00", "Label Tomorrow: Two"]);
        assert!(night.contains(&"Card Home".to_owned()), "{night:?}");
    }
}
