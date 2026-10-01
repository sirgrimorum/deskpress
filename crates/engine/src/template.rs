//! Templates: a definition bundled with the engine that a pack extends with `pack.extends`, so a
//! pack of a known kind brings only its content, its theme and what it changes.

use crate::value::{Value, show};
use crate::yaml;

/// Each template by name, and its manifest without a `pack` head.
const TEMPLATES: [(&str, &str); 1] =
    [("travel", include_str!("../../../templates/travel/pack.yaml"))];

/// The sections merged by key: the pack's entry replaces the template's where it stands, and a
/// new one is added at the end.
const BY_KEY: [&str; 5] = ["modules", "derive", "questions", "screens", "ui"];

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
    use crate::engine::{Engine, Nav, World};
    use crate::pack::Pack;
    use crate::tree::{Node, Tree};
    use crate::value::{Map, text};
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

    /// The travel template over a pack of one day.
    fn trip() -> Engine {
        load(TRIP)
    }

    fn load(content: &str) -> Engine {
        let manifest = over(&format!(
            "{HEAD}conventions: {{hidden_prefixes: [source]}}
"
        ))
        .unwrap();
        let content = parse(content).unwrap();
        Engine::load(Pack { manifest, content, theme: None }).unwrap().0
    }

    fn world(now: &str, holder: &str, store: &[(&str, &str)]) -> World {
        let store = store.iter().map(|(k, v)| ((*k).to_owned(), Value::String((*v).to_owned())));
        World {
            now: now.into(),
            holder: holder.into(),
            inside: vec![],
            located: false,
            store: Map(store.collect()),
            zones: Map::default(),
            can: vec![],
        }
    }

    /// The screen the travel template shows for a pack of one day, and the text of its nodes.
    fn shown(now: &str, holder: &str, store: &[(&str, &str)]) -> (String, Vec<String>) {
        let world = world(now, holder, store);
        let tree = trip().screen(&world, &mut Nav::default()).unwrap().tree;
        let texts = tree.nodes.iter().map(|n| {
            let prop = |k: &str| {
                n.props.iter().find(|(p, _)| p == k).map(|(_, v)| crate::value::text(Some(v)))
            };
            format!(
                "{} {}",
                n.kind,
                prop("text").or_else(|| prop("label")).or_else(|| prop("foot")).unwrap_or_default()
            )
        });
        (tree.screen, texts.collect())
    }

    const TRIP: &str = r#"people: [{id: ana, name: Ana}, {id: leo, name: Leo, adult: false}]
days:
  - date: 2026-04-11
    title: One
    blocks:
      - ["", "A note"]
      - ["10:00", "Park", {type: parking, place: lot}]
      - ["11:00", "Museum", {type: visit, place: museum, guide: leo, until: "12:00", leave: 20}]
      - ["13:00", "Drive", {type: driving, place: lot, duration: 40 min, tolls: none, until: "13:40", road: [{at: "13:10", name: Bridge, what: Toll}]}]
  - date: 2026-04-12
    title: Two
    sleeps_at: Home
    blocks: [["", "Rest"], ["19:00", "Dinner", {until: "19:45"}]]
  - date: 2026-04-13
    title: Three
    fixed: [["08:30", "Car"]]
    options:
      - {id: coast, name: Coast, recommended: true, why: Early arrival, cost: parking, blocks: [["09:30", "Go"]]}
      - {id: hill, name: Hill, blocks: [["09:00", "Train"]]}
    decision: {when: 2026-04-12, at: "20:00", question: Coast or hill?}
  - date: 2026-04-14
    title: Four
    blocks: [["10:00", "Surf", {for: [ana, leo]}]]
places:
  lot: {name: Lot, parking: {where: North side, price: "[to confirm]"}}
  museum:
    name: Museum
    safe: true
    at: {lat: 38.5, lon: -9.5}
    during:
      type: visit
      hours: 10 to 18
      ticket: doc_1
      points: [{name: Hall, for_kids: Count the lions}]
      source_web: x
    guide: {facts: [Look up, Count the doors], question: How old is it?, answer: Very old, challenge: Find the lion}
    plan:
      caption: The hall
      image: maps/museum.png
      points: [{name: Hall, x: 0.5, y: 0.25}]
documents:
  - {id: card, title: Card, for: ana, file: files/card.pdf, call: {Desk: "+1 555 0100"}, fields: {Number: A1}}
  - {id: pass, title: Pass, file: files/pass.pdf}
phrases: {hi: hola}
packing: {check: true, items: [Hat, Bottle]}
"#;

    /// One prop of a node.
    fn at<'a>(n: &'a Node, key: &str) -> Option<&'a Value> {
        n.props.iter().find(|(p, _)| p == key).map(|(_, v)| v)
    }

    /// Every node as `kind title text`, for the screens whose answer is not the node's text.
    fn said(tree: &Tree) -> Vec<String> {
        let say = |n: &Node, k: &str| text(at(n, k));
        let line = |n: &Node| format!("{} {} {}", n.kind, say(n, "title"), say(n, "text"));
        tree.nodes.iter().map(line).collect()
    }

    /// The `value` a tapping node carries, found by what it says.
    fn carried(tree: &Tree, says: &str) -> Value {
        let (lines, mine) = (said(tree), |v: &Value| text(Some(v)) == says);
        let node = tree.nodes.iter().find(|n| at(n, "text").is_some_and(mine));
        assert!(node.is_some(), "no node says {says:?} in {lines:?}");
        at(node.unwrap(), "value").cloned().unwrap()
    }

    /// The hours the Day node draws, as `time text`.
    fn hours(tree: &Tree) -> Vec<String> {
        let day = tree.nodes.iter().find(|n| n.kind == "Day").expect("a Day node");
        let blocks = at(day, "blocks").and_then(Value::as_list).unwrap_or_default();
        blocks.iter().map(|b| format!("{} {}", text(b.get("time")), text(b.get("text")))).collect()
    }

    /// The `event` of the hour of the Day node that says `says`.
    fn hour(tree: &Tree, says: &str) -> Value {
        let day = tree.nodes.iter().find(|n| n.kind == "Day").expect("a Day node");
        let blocks = at(day, "blocks").and_then(Value::as_list).unwrap_or_default();
        let b = blocks.iter().find(|b| text(b.get("text")) == says);
        assert!(b.is_some(), "no hour says {says:?}");
        b.unwrap().get("event").cloned().unwrap()
    }

    /// The day the pack shows after these facts were stored.
    fn after(e: &Engine, now: &str, facts: &Map) -> Tree {
        let w = World { store: facts.clone(), ..world(now, "", &[]) };
        e.screen(&w, &mut Nav::default()).unwrap().tree
    }

    #[test]
    fn a_block_you_set_off_for_says_when_and_then_says_to_go() {
        let e = trip();
        let lines =
            |now: &str| said(&e.screen(&world(now, "", &[]), &mut Nav::default()).unwrap().tree);
        assert!(lines("2026-04-11T10:30").contains(&"Label  Set off at 10:40".to_owned()));
        assert!(lines("2026-04-11T10:45").contains(&"Alert Set off now Museum".to_owned()));
        // Once the museum has started there is nothing to set off for.
        assert!(!lines("2026-04-11T11:30").iter().any(|l| l.contains("Set off")));
    }

    #[test]
    fn a_packing_list_is_ticked_off_row_by_row_and_remembered() {
        let (e, mut nav) = (trip(), Nav::default());
        let w = world("2026-04-11T12:30", "", &[]);
        let agenda = e.screen(&w, &mut nav).unwrap().tree;
        let out = e.dispatch(&w, &mut nav, "sheet", carried(&agenda, "packing")).unwrap();
        assert_eq!(said(&out.view.tree)[1..], ["Check  Hat", "Check  Bottle"]);
        let hat = carried(&out.view.tree, "Hat");
        let out = e.dispatch(&w, &mut nav, "tick", hat.clone()).unwrap();
        assert_eq!(out.store, Map(vec![("tick.packing.0".to_owned(), Value::Bool(true))]));
        // Tapping the same row again takes the tick off.
        let w = World { store: out.store, ..w };
        let out = e.dispatch(&w, &mut nav, "tick", hat).unwrap();
        assert_eq!(out.store, Map(vec![("tick.packing.0".to_owned(), Value::Bool(false))]));
    }

    #[test]
    fn the_night_screen_moves_tomorrow_about_and_the_day_comes_back_changed() {
        let (e, mut nav) = (trip(), Nav::default());
        let night = world("2026-04-11T21:30", "", &[]);
        let out = e.dispatch(&night, &mut nav, "adjust", Value::Null).unwrap();
        let dinner = hour(&out.view.tree, "Dinner");
        e.dispatch(&night, &mut nav, "pick", dinner.clone()).unwrap();
        let out = e.dispatch(&night, &mut nav, "later", Value::Null).unwrap();
        // The pack is untouched: the new hour comes from the fact, every time the day is built.
        assert_eq!(said(&after(&e, "2026-04-11T21:30", &out.store))[1], "BigValue  19:15");
        let out = e.dispatch(&night, &mut nav, "drop", Value::Null).unwrap();
        let gone = said(&after(&e, "2026-04-11T21:30", &out.store));
        assert!(!gone.iter().any(|l| l.contains("Dinner")), "{gone:?}");
    }

    #[test]
    fn an_hour_dragged_down_the_day_trades_places_with_the_ones_it_passes() {
        let (e, mut nav) = (trip(), Nav::default());
        let noon = world("2026-04-11T14:00", "", &[]);
        let out = e.dispatch(&noon, &mut nav, "adjust", Value::Null).unwrap();
        let park = hour(&out.view.tree, "Park");
        let arg = Map(vec![("block".into(), park), ("to".into(), Value::Number(1.0))]);
        let out = e.dispatch(&noon, &mut nav, "reorder", Value::Map(arg)).unwrap();
        assert_eq!(hours(&out.view.tree), [" A note", "10:00 Museum", "12:00 Park", "13:00 Drive"]);
    }

    #[test]
    fn a_block_added_at_night_joins_tomorrow_in_time_order() {
        let (e, mut nav) = (trip(), Nav::default());
        let night = world("2026-04-11T21:30", "", &[]);
        e.dispatch(&night, &mut nav, "adjust", Value::Null).unwrap();
        e.dispatch(&night, &mut nav, "add", Value::Null).unwrap();
        e.dispatch(&night, &mut nav, "set_time", Value::String("08:00".into())).unwrap();
        e.dispatch(&night, &mut nav, "set_what", Value::String("Bread".into())).unwrap();
        let out = e.dispatch(&night, &mut nav, "keep", Value::Null).unwrap();
        // Eight is before the dinner, so it becomes tomorrow's first hour.
        assert_eq!(said(&after(&e, "2026-04-11T21:30", &out.store))[1], "BigValue  08:00");
    }

    #[test]
    fn the_travel_template_picks_a_screen_for_every_moment_of_a_day() {
        let screen = |now, holder| shown(now, holder, &[]).0;
        assert_eq!(screen("2026-04-20T10:00", ""), "days");
        assert_eq!(screen("2026-04-11T09:00", ""), "morning");
        assert_eq!(screen("2026-04-11T10:30", ""), "moment");
        assert_eq!(screen("2026-04-11T11:30", "leo"), "suggestion");
        assert_eq!(screen("2026-04-11T12:30", ""), "agenda");
        assert_eq!(screen("2026-04-11T21:30", ""), "night");
        // A block whose `for` lists the holder is theirs too.
        assert_eq!(screen("2026-04-14T10:30", "ana"), "suggestion");
        // A relay stores who holds the phone when the host does not say.
        assert_eq!(shown("2026-04-11T11:30", "", &[("holder", "leo")]).0, "suggestion");
    }

    #[test]
    fn a_child_holds_the_phone_only_where_it_is_safe_or_for_the_time_given() {
        let kid = |now, store: &[(&str, &str)]| shown(now, "leo", store);
        // Seated in the car: the kid screen, with the places' points for kids and no way out.
        let (screen, texts) = kid("2026-04-11T13:30", &[]);
        assert_eq!(
            (screen.as_str(), texts.last().map(String::as_str)),
            ("kid", Some("Button Give the phone back"))
        );
        // A parking lot, a free hour and no day at all are not safe.
        assert_eq!(kid("2026-04-11T10:30", &[]).0, "complete");
        assert_eq!(kid("2026-04-11T12:30", &[]).0, "complete");
        assert_eq!(kid("2026-04-20T10:00", &[]).0, "complete");
        // Extra time a parent gave holds until it runs out.
        let given = [("holder_until", "2026-04-11T10:45")];
        assert_eq!(kid("2026-04-11T10:30", &given).0, "kid");
        assert_eq!(kid("2026-04-11T10:45", &given).0, "complete");
        let (_, hand_back) = kid("2026-04-11T10:30", &[]);
        assert_eq!(hand_back[2..], ["Button Give the phone back", "Button 15 more minutes"]);
    }

    #[test]
    fn the_handoff_offers_a_child_where_it_is_safe_and_a_parent_takes_it_back() {
        let e = trip();
        let offered = |now| {
            let mut nav = Nav::default();
            let out =
                e.dispatch(&world(now, "", &[("holder", "ana")]), &mut nav, "relay", Value::Null);
            out.unwrap().view.tree.nodes.len() - 1
        };
        assert_eq!((offered("2026-04-11T13:30"), offered("2026-04-11T10:30")), (2, 1));
        // Handing over keeps who to hand back to.
        let mut nav = Nav::default();
        let w = world("2026-04-11T13:30", "", &[("holder", "ana")]);
        e.dispatch(&w, &mut nav, "relay", Value::Null).unwrap();
        let out = e.dispatch(&w, &mut nav, "hold", Value::String("leo".into())).unwrap();
        assert_eq!(out.store.get("returns_to"), Some(&Value::String("ana".into())));
        // The hand-back asks the host for a parent, and names what to run after.
        let w = world("2026-04-11T10:30", "", &[("holder", "leo"), ("returns_to", "ana")]);
        let mut nav = Nav::default();
        let out = e.dispatch(&w, &mut nav, "more_time", Value::Null).unwrap();
        assert_eq!(out.commands[0].name, "device.unlock");
        assert_eq!(out.commands[0].args.get("then"), Some(&Value::String("extend".into())));
        let out = e.dispatch(&w, &mut nav, "extend", Value::Null).unwrap();
        let until = Value::String("2026-04-11T10:45".into());
        assert_eq!(
            (out.store.get("holder_until"), out.view.tree.screen.as_str()),
            (Some(&until), "kid")
        );
        let out = e.dispatch(&w, &mut Nav::default(), "returned", Value::Null).unwrap();
        assert_eq!((out.view.tree.screen.as_str(), out.view.tree.kid), ("moment", false));
    }

    #[test]
    fn an_adult_opens_a_document_from_the_agenda_and_calls_before_the_file() {
        let e = trip();
        let w = world("2026-04-11T12:30", "ana", &[]);
        let (_, agenda) = shown("2026-04-11T12:30", "ana", &[]);
        assert!(agenda.contains(&"Button Documents".to_owned()), "{agenda:?}");
        let mut nav = Nav::default();
        let list = e.dispatch(&w, &mut nav, "documents", Value::Null).unwrap().view.tree;
        // One group per person with documents, then the ones that are nobody's.
        let kinds: Vec<&str> = list.nodes.iter().map(|n| n.kind.as_str()).collect();
        assert_eq!(kinds, ["Screen", "Group", "Row"]);
        let text = |n: &Node, key: &str| {
            crate::value::text(n.props.iter().find(|(k, _)| k == key).map(|(_, v)| v))
        };
        assert_eq!(
            (text(&list.nodes[1], "title"), text(&list.nodes[2], "text")),
            ("Ana".into(), "Pass".into())
        );
        let row = &list.nodes[1].children[0];
        let doc = row.props.iter().find(|(k, _)| k == "value").map(|(_, v)| v.clone()).unwrap();
        assert_eq!(crate::value::text(doc.get("person")), "Ana");
        let one = e.dispatch(&w, &mut nav, "open", doc).unwrap().view.tree;
        let kinds: Vec<&str> = one.nodes.iter().map(|n| n.kind.as_str()).collect();
        assert_eq!(kinds, ["Screen", "Label", "Button", "Card", "Button"]);
        let number = Value::String("+1 555 0100".into());
        let call = e.dispatch(&w, &mut nav, "call", number.clone()).unwrap().commands;
        assert_eq!(
            (call[0].name.as_str(), call[0].args.get("number")),
            ("phone.call", Some(&number))
        );
        let file = e.dispatch(&w, &mut nav, "file", Value::Null).unwrap().commands;
        let path = Value::String("files/card.pdf".into());
        assert_eq!(
            (file[0].name.as_str(), file[0].args.get("file")),
            ("document.open", Some(&path))
        );
    }

    #[test]
    fn the_moment_shows_the_answer_of_its_type_and_what_the_pack_added() {
        // The hero is the first big answer on the screen, whatever stands above it.
        let hero = |t: &[String]| {
            t.iter()
                .find(|l| l.starts_with("BigValue") || l.starts_with("Missing"))
                .unwrap()
                .clone()
        };
        let (_, parking) = shown("2026-04-11T10:30", "", &[]);
        assert_eq!(hero(&parking), "Missing [to confirm]");
        assert!(parking.contains(&"Card North side".to_owned()), "{parking:?}");
        let (_, driving) = shown("2026-04-11T13:30", "", &[]);
        assert_eq!(hero(&driving), "BigValue 40 min");
        let parking = "Missing where: North side\nprice: [to confirm]";
        assert!(driving.contains(&parking.to_owned()), "{driving:?}");
        assert!(driving.contains(&"Card none".to_owned()), "{driving:?}");
        assert_eq!(driving[0], "Screen Nothing more today");
        let (_, visit) = shown("2026-04-11T11:30", "ana", &[]);
        assert!(visit.contains(&"Card 10 to 18".to_owned()), "{visit:?}");
        assert!(!visit.iter().any(|t| t.ends_with(" x")), "{visit:?}");
        assert!(!visit.iter().any(|t| t.contains("doc_1")), "{visit:?}");
        assert!(visit.contains(&"Button Point by point".to_owned()));
        assert!(visit.contains(&"Button The ticket".to_owned()));
        let (_, night) = shown("2026-04-11T21:30", "", &[]);
        assert_eq!(night[..2], ["Screen Two", "BigValue 19:00"]);
        assert!(night.contains(&"Card Home".to_owned()), "{night:?}");
    }

    #[test]
    fn a_free_lot_says_so_one_with_no_price_says_its_time_and_types_speak_the_pack_s_words() {
        let manifest = over(&format!("{HEAD}ui: {{free: Gratis, types: {{parking: Parqueo}}}}\n"));
        let content = parse(
            r#"days: [{date: 2026-04-11, title: One, blocks: [["10:00", "A", {type: parking, place: free}], ["11:00", "B", {type: parking, place: open}]]}]
places: {free: {name: Free, parking: {price: 0}}, open: {name: Open, parking: {where: West}}}
"#,
        );
        let pack = Pack { manifest: manifest.unwrap(), content: content.unwrap(), theme: None };
        let e = Engine::load(pack).unwrap().0;
        let big = |now: &str| {
            let tree = e.screen(&world(now, "", &[]), &mut Nav::default()).unwrap().tree;
            let lines = said(&tree);
            (lines.iter().find(|l| l.starts_with("BigValue")).cloned().unwrap(), lines)
        };
        let (free, lines) = big("2026-04-11T10:30");
        assert_eq!(free, "BigValue  Gratis");
        assert!(lines.contains(&"Label  Now · Parqueo".to_owned()), "{lines:?}");
        assert_eq!(big("2026-04-11T11:30").0, "BigValue  11:00");
    }

    #[test]
    fn a_child_leaving_a_safe_place_ends_the_turn_while_the_device_knows_where_it_is() {
        let screen = |inside: &[&str], located| {
            let inside = inside.iter().map(|i| (*i).to_owned()).collect();
            let w = World { inside, located, ..world("2026-04-11T11:30", "leo", &[]) };
            trip().screen(&w, &mut Nav::default()).unwrap().tree.screen
        };
        assert_eq!(screen(&[], false), "suggestion");
        assert_eq!(screen(&["museum"], true), "suggestion");
        assert_eq!(screen(&[], true), "complete");
    }

    #[test]
    fn the_map_opens_the_place_and_the_car_is_saved_where_it_was_left() {
        let e = trip();
        let (_, visit) = shown("2026-04-11T11:30", "ana", &[]);
        assert!(visit.contains(&"Button Open in the map".to_owned()), "{visit:?}");
        let w = world("2026-04-11T11:30", "ana", &[]);
        let map = e.dispatch(&w, &mut Nav::default(), "map", Value::Null).unwrap().commands;
        assert_eq!(
            (map[0].name.as_str(), map[0].args.get("lat"), map[0].args.get("label")),
            ("map.open", Some(&Value::Number(38.5)), Some(&Value::String("Museum".into())))
        );
        let (_, parking) = shown("2026-04-11T10:30", "ana", &[]);
        assert!(parking.contains(&"Button Save where the car is".to_owned()), "{parking:?}");
        let w = world("2026-04-11T10:30", "ana", &[]);
        let park = e.dispatch(&w, &mut Nav::default(), "park", Value::Null).unwrap().commands;
        let then = Value::String("parked".into());
        assert_eq!(
            (park[0].name.as_str(), park[0].args.get("then")),
            ("location.get", Some(&then))
        );
        let spot = crate::yaml::parse("v: {lat: 1, lon: 2}").unwrap().get("v").cloned().unwrap();
        let out = e.dispatch(&w, &mut Nav::default(), "parked", spot.clone()).unwrap();
        assert_eq!(out.store.get("car"), Some(&spot));
        let when = (out.store.get("log.parked"), out.store.get("log.spot"));
        assert_eq!(when, (Some(&Value::String("2026-04-11T10:30".into())), Some(&Value::Null)));
        // The agenda finds the car from then on.
        let mut w = world("2026-04-11T12:30", "ana", &[]);
        w.store.set("car", spot);
        let mut nav = Nav::default();
        let agenda = e.screen(&w, &mut nav).unwrap().tree;
        let car = Value::String("Find the car".into());
        assert!(
            agenda.nodes.iter().any(|n| n.props.iter().any(|(k, v)| k == "label" && *v == car))
        );
        let car = e.dispatch(&w, &mut nav, "car", Value::Null).unwrap().commands;
        assert_eq!(
            (car[0].name.as_str(), car[0].args.get("lon")),
            ("map.open", Some(&Value::Number(2.0)))
        );
    }

    #[test]
    fn a_task_ticked_twice_is_open_again_and_a_bed_time_the_plan_runs_past_warns_until_it_passes() {
        let extra = r#"tasks: [{id: bags, title: Bags}]
jet_lag: [{date: 2026-04-11, steps: [{time: "12:45", text: Nap, do: bed}, {time: "13:15", text: Up}]}]
"#;
        let e = load(&format!("{TRIP}{extra}"));
        let (w, mut nav) = (world("2026-04-11T12:30", "", &[]), Nav::default());
        let agenda = said(&e.screen(&w, &mut nav).unwrap().tree);
        let clash = "Alert The plan runs past bed time 12:45 · Drive".to_owned();
        assert!(agenda.contains(&clash), "{agenda:?}");
        let later = world("2026-04-11T13:30", "", &[]);
        let past = e.dispatch(&later, &mut Nav::default(), "agenda", Value::Null).unwrap();
        assert!(!said(&past.view.tree).contains(&clash));
        let tasks = e.dispatch(&w, &mut nav, "tasks", Value::Null).unwrap().view.tree;
        let bags = carried(&tasks, "Bags");
        let once = e.dispatch(&w, &mut nav, "tick", bags.clone()).unwrap().store;
        let w = World { store: once.clone(), ..w };
        let twice = e.dispatch(&w, &mut nav, "tick", bags).unwrap().store;
        let tick = |done| Map(vec![("task.bags".to_owned(), Value::Bool(done))]);
        assert_eq!((once, twice), (tick(true), tick(false)));
    }

    #[test]
    fn the_log_keeps_only_what_changed_of_the_car_and_each_note_on_its_own() {
        let (e, mut nav) = (trip(), Nav::default());
        let w = world("2026-04-11T12:30", "", &[("log.plate", "1234 ABC")]);
        e.dispatch(&w, &mut nav, "log", Value::Null).unwrap();
        e.dispatch(&w, &mut nav, "set_plate", Value::String("1234 ABC".into())).unwrap();
        e.dispatch(&w, &mut nav, "set_fuel", Value::String("3/4".into())).unwrap();
        let out = e.dispatch(&w, &mut nav, "keep", Value::Null).unwrap();
        assert_eq!(out.store, Map(vec![("log.fuel".to_owned(), Value::String("3/4".into()))]));
        assert_eq!(out.view.tree.screen, "agenda");
        e.dispatch(&w, &mut nav, "log", Value::Null).unwrap();
        e.dispatch(&w, &mut nav, "note", Value::Null).unwrap();
        e.dispatch(&w, &mut nav, "write", Value::String("Filled up".into())).unwrap();
        let out = e.dispatch(&w, &mut nav, "keep", Value::Null).unwrap();
        let note = ("note.2026-04-11T12:30.0".to_owned(), Value::String("Filled up".into()));
        assert_eq!(out.store, Map(vec![note]));
        let w = World { store: out.store, ..w };
        let log = said(&e.dispatch(&w, &mut Nav::default(), "log", Value::Null).unwrap().view.tree);
        assert_eq!(log.last().unwrap(), "Row  Filled up");
    }

    #[test]
    fn a_place_draws_its_own_pins_the_road_says_what_it_passes_and_the_chart_opens() {
        let e = trip();
        let prop = |t: &Tree, kind: &str, key: &str| {
            let node = t.nodes.iter().find(|n| n.kind == kind).expect(kind);
            node.props.iter().find(|(p, _)| p == key).map(|(_, v)| v.clone()).expect(key)
        };
        // A place with a plan draws its pins over the picture the pack ships.
        let w = world("2026-04-11T11:30", "ana", &[]);
        let visit = e.screen(&w, &mut Nav::default()).unwrap().tree;
        assert_eq!(prop(&visit, "Map", "image"), Value::String("maps/museum.png".into()));
        assert_eq!(prop(&visit, "Map", "caption"), Value::String("The hall".into()));
        // A point with an `at` pins the moment's map and the kid's; the kid's list keeps for_kids.
        let gate = load(&TRIP.replace(
            "points: [{name: Hall, for_kids",
            "points: [{name: Gate, at: {lat: 38.51, lon: -9.51}}, {name: Hall, for_kids",
        ));
        let area = |t: &Tree| {
            let map = t.nodes.iter().find(|n| n.kind == "Map" && at(n, "image").is_none());
            let points = map.and_then(|m| at(m, "points")).and_then(Value::as_list).unwrap();
            points.iter().map(|p| text(p.get("name"))).collect::<Vec<_>>()
        };
        assert_eq!(area(&gate.screen(&w, &mut Nav::default()).unwrap().tree), ["Gate"]);
        let leo = world("2026-04-11T11:30", "leo", &[]);
        let kid = gate.dispatch(&leo, &mut Nav::default(), "go", Value::Null).unwrap().view.tree;
        assert_eq!((kid.screen.as_str(), area(&kid)), ("kid", vec!["Gate".to_owned()]));
        let rows: Vec<_> = said(&kid).into_iter().filter(|l| l.starts_with("Row")).collect();
        assert_eq!(rows, ["Row  Hall"]);
        // A moving block lists what it passes, in the order it passes it.
        let (_, drive) = shown("2026-04-11T13:20", "ana", &[]);
        assert!(drive.contains(&"Row Bridge".to_owned()), "{drive:?}");
        // The chart is the day's own places, and nothing is fetched to draw them.
        let chart = e.dispatch(&w, &mut Nav::default(), "chart", Value::Null).unwrap().view.tree;
        assert_eq!(chart.screen, "chart");
        assert_eq!(prop(&chart, "Map", "points").as_list().map(<[Value]>::len), Some(1));
        // One stop is no route; a second place with an `at` makes the day one, in visit order.
        let route = Value::String("The whole day in the map app".into());
        let routes = |t: &Tree| t.nodes.iter().any(|n| at(n, "label") == Some(&route));
        assert!(!routes(&chart));
        let two =
            load(&TRIP.replace("lot: {name: Lot,", "lot: {name: Lot, at: {lat: 38.6, lon: -9.6},"));
        let mut nav = Nav::default();
        assert!(routes(&two.dispatch(&w, &mut nav, "chart", Value::Null).unwrap().view.tree));
        let sent = two.dispatch(&w, &mut nav, "route", Value::Null).unwrap().commands;
        let stops = sent[0].args.get("stops").and_then(Value::as_list).unwrap();
        let names: Vec<_> = stops.iter().map(|s| text(s.get("name"))).collect();
        assert_eq!(
            (sent[0].name.as_str(), names),
            ("map.route", ["Lot", "Museum", "Lot"].map(String::from).to_vec())
        );
    }

    #[test]
    fn a_question_opens_its_answer_in_a_dialog_that_closes() {
        let (e, w, mut nav) = (trip(), world("2026-04-11T11:30", "ana", &[]), Nav::default());
        let dialogs =
            |t: &Tree| said(t).into_iter().filter(|l| l.starts_with("Dialog")).collect::<Vec<_>>();
        let ask = e.dispatch(&w, &mut nav, "ask", Value::Null).unwrap().view.tree;
        assert_eq!((ask.screen.as_str(), dialogs(&ask).len()), ("ask", 0));
        let now = carried(&ask, "What is happening now?");
        let open = e.dispatch(&w, &mut nav, "show", now).unwrap().view.tree;
        assert_eq!(dialogs(&open), ["Dialog What is happening now? 11:00 Museum"]);
        let dialog = open.nodes.iter().find(|n| n.kind == "Dialog").unwrap();
        assert_eq!(at(dialog, "close"), Some(&Value::String("Close".into())));
        let closed = e.dispatch(&w, &mut nav, "close", Value::Null).unwrap().view.tree;
        assert!(dialogs(&closed).is_empty());
    }

    #[test]
    fn an_adult_puts_the_event_the_day_or_the_trip_in_the_calendar() {
        let e = trip();
        let (_, visit) = shown("2026-04-11T11:30", "ana", &[]);
        assert!(visit.contains(&"Button Put this in the calendar".to_owned()), "{visit:?}");
        assert!(!visit.iter().any(|t| t.contains("2026-04-11.blocks")), "{visit:?}");
        let (_, agenda) = shown("2026-04-11T12:30", "ana", &[]);
        for label in ["Button Put this day in the calendar", "Button Put the trip in the calendar"]
        {
            assert!(agenda.contains(&label.to_owned()), "{agenda:?}");
        }
        let scope = |now: &str, action: &str| {
            let w = world(now, "ana", &[]);
            let out = e.dispatch(&w, &mut Nav::default(), action, Value::Null).unwrap();
            let cmd = &out.commands[0];
            (cmd.name.clone(), crate::value::text(cmd.args.get("scope")))
        };
        let sync = |s: &str| ("calendar.sync".to_owned(), s.to_owned());
        assert_eq!(scope("2026-04-11T11:30", "calendar"), sync("2026-04-11.blocks.2"));
        assert_eq!(scope("2026-04-11T12:30", "calendar_day"), sync("2026-04-11"));
        assert_eq!(scope("2026-04-11T12:30", "calendar_trip"), sync(""));
    }

    #[test]
    fn a_climate_that_syncs_is_updated_from_the_agenda_which_says_when_it_failed() {
        let sync = "modules:
  climate: {sync: {trigger: button, request: {url: 'https://a.org'}, read: {high: max}}}
";
        let manifest = over(&format!("{HEAD}{sync}")).unwrap();
        let content = parse(TRIP).unwrap();
        let e = Engine::load(Pack { manifest, content, theme: None }).unwrap().0;
        let mut w = world("2026-04-11T12:30", "ana", &[]);
        let failed = "{as_of: null, tried: '2026-04-11T12:00', failed: no answer, rows: []}";
        w.store.set("sync.climate", parse(failed).unwrap());
        let tree = e.screen(&w, &mut Nav::default()).unwrap().tree;
        let said: Vec<String> = tree
            .nodes
            .iter()
            .flat_map(|n| n.props.iter().filter(|(k, _)| k == "text" || k == "label"))
            .map(|(_, v)| crate::value::text(Some(v)))
            .collect();
        for line in ["Update the weather", "The weather did not update: no answer"] {
            assert!(said.contains(&line.to_owned()), "{said:?}");
        }
        let out = e.dispatch(&w, &mut Nav::default(), "weather", Value::Null).unwrap();
        assert_eq!(out.commands[0].name, "climate.sync");
    }

    #[test]
    fn a_due_decision_asks_an_adult_and_keeping_a_plan_writes_the_day() {
        // Not before its hour, and never to a child.
        assert_eq!(shown("2026-04-12T19:50", "ana", &[]).0, "night");
        assert_ne!(shown("2026-04-12T20:30", "leo", &[]).0, "choose");
        let (screen, texts) = shown("2026-04-12T20:30", "ana", &[]);
        assert_eq!(screen, "choose");
        // Today leads out without answering.
        let e = trip();
        let w = world("2026-04-12T20:30", "ana", &[]);
        let out = e.dispatch(&w, &mut Nav::default(), "agenda", Value::Null).unwrap();
        assert_eq!(out.view.tree.screen, "agenda");
        assert_eq!(
            texts,
            [
                "Screen ",
                "BigValue Coast or hill?",
                "Label Until somebody chooses, the day follows the recommended plan.",
                "Card Early arrival",
                "Card parking",
                "Button See the other options",
                "Button Keep this plan"
            ]
        );
        let mut nav = Nav::default();
        let kinds = |out: crate::engine::Outcome| -> Vec<String> {
            out.view.tree.nodes.iter().map(|n| n.kind.clone()).collect()
        };
        let others = kinds(e.dispatch(&w, &mut nav, "others", Value::Null).unwrap());
        assert!(others.contains(&"Segmented".to_owned()), "{others:?}");
        // Keeping it as it is stores the recommended plan.
        let out = e.dispatch(&w, &mut nav, "confirm", Value::Null).unwrap();
        let coast = Value::String("coast".into());
        assert_eq!(out.store.get("choice.2026-04-13"), Some(&coast));
        assert_eq!(
            (out.commands[0].name.as_str(), crate::value::text(out.commands[0].args.get("scope"))),
            ("calendar.sync", "2026-04-13".to_owned())
        );
        // A picked option wins, and once answered the day moves on.
        let hill = parse("{id: hill, name: Hill}").unwrap();
        let mut nav = Nav::default();
        e.dispatch(&w, &mut nav, "pick", hill).unwrap();
        let out = e.dispatch(&w, &mut nav, "confirm", Value::Null).unwrap();
        assert_eq!(out.store.get("choice.2026-04-13"), Some(&Value::String("hill".into())));
        assert_eq!(shown("2026-04-12T20:30", "ana", &[("choice.2026-04-13", "hill")]).0, "night");
        // Once kept, the agenda opens it again, and keeping a plan goes home.
        let kept = world("2026-04-12T20:30", "ana", &[("choice.2026-04-13", "hill")]);
        let mut nav = Nav::default();
        let labels = |out: crate::engine::Outcome| -> Vec<String> {
            let label = |n: &crate::tree::Node| n.props.iter().find(|(k, _)| k == "label").cloned();
            out.view
                .tree
                .nodes
                .iter()
                .filter_map(label)
                .map(|(_, v)| crate::value::text(Some(&v)))
                .collect()
        };
        let agenda = labels(e.dispatch(&kept, &mut nav, "agenda", Value::Null).unwrap());
        assert!(agenda.contains(&"Change the plan".to_owned()), "{agenda:?}");
        let out = e.dispatch(&kept, &mut nav, "choose", Value::Null).unwrap();
        assert_eq!(out.view.tree.screen, "choose");
        assert_eq!(
            e.dispatch(&kept, &mut nav, "confirm", Value::Null).unwrap().view.tree.screen,
            "night"
        );
    }

    #[test]
    fn the_agenda_lists_the_sheets_and_opens_one_whole() {
        let (_, agenda) = shown("2026-04-11T12:30", "ana", &[]);
        assert!(agenda.contains(&"Row phrases".to_owned()), "{agenda:?}");
        let e = trip();
        let w = world("2026-04-11T12:30", "ana", &[]);
        let sheet = parse("{id: phrases, title: phrases, value: {hi: hola}}").unwrap();
        let tree = e.dispatch(&w, &mut Nav::default(), "sheet", sheet).unwrap().view.tree;
        // The whole value, laid out by kind.
        assert_eq!(tree.screen, "sheet");
        assert!(format!("{:?}", tree.nodes[1]).contains("hola"), "{:?}", tree.nodes);
    }

    #[test]
    fn a_child_the_block_names_its_guide_gets_the_places_script() {
        let e = trip();
        let w = world("2026-04-11T11:30", "leo", &[]);
        let mut nav = Nav::default();
        let texts = |out: crate::engine::Outcome| -> Vec<String> {
            let text = |n: &Node| {
                let prop = |k: &str| n.props.iter().find(|(p, _)| p == k).map(|(_, v)| v);
                crate::value::text(prop("text").or_else(|| prop("label")))
            };
            out.view.tree.nodes.iter().map(|n| format!("{} {}", n.kind, text(n))).collect()
        };
        let kid = texts(e.dispatch(&w, &mut nav, "go", Value::Null).unwrap());
        for line in [
            "Label You could be the guide",
            "Group ",
            "Card How old is it?",
            "Button Show the answer",
            "Card Find the lion",
        ] {
            assert!(kid.contains(&line.to_owned()), "{kid:?}");
        }
        assert!(!kid.contains(&"Card Very old".to_owned()), "{kid:?}");
        let answered = texts(e.dispatch(&w, &mut nav, "answer", Value::Null).unwrap());
        assert!(answered.contains(&"Card Very old".to_owned()), "{answered:?}");
        assert!(!answered.contains(&"Button Show the answer".to_owned()), "{answered:?}");
        // Seated in the car nobody guides: no script.
        let (_, car) = shown("2026-04-11T13:30", "leo", &[]);
        assert!(!car.contains(&"Label You could be the guide".to_owned()), "{car:?}");
    }

    #[test]
    fn the_handoff_stars_the_guide_the_block_suggests() {
        let e = trip();
        let w = world("2026-04-11T11:30", "", &[("holder", "ana")]);
        let tree = e.dispatch(&w, &mut Nav::default(), "relay", Value::Null).unwrap().view.tree;
        let chips: Vec<String> = tree.nodes[1..]
            .iter()
            .map(|n| crate::value::text(n.props.iter().find(|(k, _)| k == "text").map(|(_, v)| v)))
            .collect();
        assert_eq!(chips, ["★ Leo", "Ana"]);
    }

    #[test]
    fn the_night_starts_at_seven_after_the_last_block_and_no_day_says_so() {
        assert_ne!(shown("2026-04-11T18:30", "", &[]).0, "night");
        assert_eq!(shown("2026-04-11T19:00", "", &[]).0, "night");
        assert_eq!(shown("2026-04-11T19:30", "", &[]).0, "night");
        // A last block still running keeps its moment until its `until`.
        assert_eq!(shown("2026-04-12T19:30", "", &[]).0, "moment");
        let (screen, texts) = shown("2026-04-20T10:00", "", &[]);
        assert_eq!(
            (screen.as_str(), &texts[..2]),
            (
                "days",
                &["Screen ".to_owned(), "Label The plan does not cover today.".to_owned()][..]
            )
        );
    }
}
