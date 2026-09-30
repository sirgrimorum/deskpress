//! Blocks the person moved, resized, dropped or added on the device (decision 0025). The pack is a
//! document, so none of it is written back: each edit is a stored fact, keyed by the block's
//! `event` id, and the timeline applies them every time it builds the day.

use crate::clock::{minutes, shift};
use crate::validate::patterns::{is_real_date, is_time};
use crate::value::{Map, Value, text, truthy};

/// The most an edit moves or stretches one block, either way.
const LIMIT: i64 = 12 * 60;
/// The most blocks one day takes added to it.
const ADDED: usize = 32;

pub(crate) fn whole(v: Option<&Value>) -> i64 {
    match v {
        Some(Value::Number(n)) if n.fract() == 0.0 => (*n as i64).clamp(-LIMIT, LIMIT),
        _ => 0,
    }
}

/// An `HH:MM` moved by minutes, kept inside its day: an edit never pushes a block past midnight.
pub(crate) fn moved(time: &str, by: i64) -> String {
    let day = minutes("2000-01-01T00:00");
    let at = (minutes(&format!("2000-01-01T{time}")) + by).clamp(day, day + 1439);
    shift("2000-01-01T00:00", at - day)[11..].to_owned()
}

/// The blocks added to the day on `date`, as the pack writes one: time, text and its own map.
/// Their `event` ids continue the pack's scheme, so they move, drop and sync like any other.
pub(crate) fn added(store: &Map, date: &str) -> Vec<Map> {
    let rows = store.get(&format!("added.{date}")).and_then(Value::as_list).unwrap_or_default();
    let block = |(n, row): (usize, &Value)| {
        let (time, what) = (text(row.get("time")), text(row.get("text")));
        if !is_time(&time) || what.is_empty() {
            return None;
        }
        let mut m = Map::default();
        m.set("time", Value::String(time));
        m.set("text", Value::String(what));
        m.set("event", Value::String(format!("{date}.added.{n}")));
        Some(m)
    };
    rows.iter().take(ADDED).enumerate().filter_map(block).collect()
}

/// Applies the edits to a day's blocks, in time order, and says whether any landed. A resize moves
/// what follows it; a `locked` hour never moves and ends that push, so a booked train stays put and
/// the day after it keeps the hours the pack gave it.
pub(crate) fn replan(store: &Map, blocks: &mut Vec<Map>) -> bool {
    let fact = |b: &Map| store.get(&format!("plan.{}", text(b.get("event")))).cloned();
    let mut edited: Vec<(Option<Value>, Map)> = blocks.drain(..).map(|b| (fact(&b), b)).collect();
    if edited.iter().all(|(e, _)| e.is_none()) {
        blocks.extend(edited.into_iter().map(|(_, b)| b));
        return false;
    }
    edited.retain(|(e, _)| !truthy(e.as_ref().and_then(|e| e.get("off"))));
    edited.sort_by_key(|(_, b)| text(b.get("time")));
    let mut carry = 0;
    for (e, b) in &mut edited {
        let time = text(b.get("time"));
        if time.is_empty() {
            continue;
        }
        if truthy(b.get("locked")) {
            carry = 0;
            continue;
        }
        let (start, grow) = (
            carry + whole(e.as_ref().and_then(|e| e.get("shift"))),
            whole(e.as_ref().and_then(|e| e.get("grow"))),
        );
        if start != 0 {
            b.set("time", Value::String(moved(&time, start)));
        }
        let until = text(b.get("until"));
        if !until.is_empty() && start + grow != 0 {
            b.set("until", Value::String(moved(&until, start + grow)));
        }
        carry += grow;
    }
    blocks.extend(edited.into_iter().map(|(_, b)| b));
    true
}

/// The facts a `timeline` action writes, or `None` when the engine does not handle it. These are
/// the only module actions the engine answers itself: they change stored facts and touch nothing
/// outside, so no command reaches the host (decision 0025).
pub(crate) fn edit(store: &Map, scope: &Map, action: &str, args: &Map) -> Option<Map> {
    let event = text(args.get("block"));
    let by = whole(args.get("by"));
    let mut facts = Map::default();
    let mut bump = |event: &str, key: &str, by: i64| {
        let was = store.get(&format!("plan.{event}"));
        let mut m = was.and_then(Value::as_map).cloned().unwrap_or_default();
        m.set(
            key,
            Value::Number((whole(was.and_then(|w| w.get(key))) + by).clamp(-LIMIT, LIMIT) as f64),
        );
        facts.set(&format!("plan.{event}"), Value::Map(m));
    };
    // An action of ours with arguments that make no sense writes nothing. It never falls through
    // to the host: `timeline` is the engine's, so a typo in a pack must not reach a device tool.
    match action {
        "timeline.move" | "timeline.grow" => {
            if !event.is_empty() && by != 0 {
                bump(&event, if action == "timeline.move" { "shift" } else { "grow" }, by);
            }
        }
        "timeline.swap" => {
            if let Some((a, b, other)) = neighbours(scope, &event, &text(args.get("side"))) {
                let at = |t: &str| minutes(&format!("2000-01-01T{t}"));
                bump(&event, "shift", at(&b) - at(&a));
                bump(&other, "shift", at(&a) - at(&b));
            }
        }
        "timeline.drop" => {
            if !event.is_empty() {
                let off = Map(vec![("off".to_owned(), Value::Bool(true))]);
                facts.set(&format!("plan.{event}"), Value::Map(off));
            }
        }
        "timeline.restore" => {
            if !event.is_empty() {
                facts.set(&format!("plan.{event}"), Value::Null);
            }
        }
        "timeline.add" => {
            let (date, time, what) =
                (text(args.get("date")), text(args.get("time")), text(args.get("text")));
            let key = format!("added.{date}");
            let mut rows = store.get(&key).and_then(Value::as_list).unwrap_or_default().to_vec();
            if is_time(&time) && !what.is_empty() && is_real_date(&date) && rows.len() < ADDED {
                let mut row = Map::default();
                row.set("time", Value::String(time));
                row.set("text", Value::String(what));
                rows.push(Value::Map(row));
                facts.set(&key, Value::List(rows));
            }
        }
        _ => return None,
    }
    Some(facts)
}

/// The times of the block `event` and of the timed neighbour on `side` (`up` or `down`) of it, with
/// that neighbour's id, in the day the timeline exposed. `None` when either end is missing or
/// locked: a locked hour is fixed, so nothing swaps with it.
fn neighbours(scope: &Map, event: &str, side: &str) -> Option<(String, String, String)> {
    let (day, at) = ["day", "tomorrow"].into_iter().find_map(|d| {
        let blocks: Vec<&Value> = scope
            .get(d)?
            .get("blocks")?
            .as_list()?
            .iter()
            .filter(|b| !text(b.get("time")).is_empty())
            .collect();
        let at = blocks.iter().position(|b| text(b.get("event")) == event)?;
        Some((blocks, at))
    })?;
    let other = match side {
        "up" => at.checked_sub(1)?,
        "down" => at + 1,
        _ => return None,
    };
    let other = day.get(other)?;
    let both = [day[at], other];
    (!both.iter().any(|b| truthy(b.get("locked"))))
        .then(|| (text(day[at].get("time")), text(other.get("time")), text(other.get("event"))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::show;
    use crate::yaml::parse;

    /// The mapping this YAML writes.
    fn map(src: &str) -> Map {
        parse(src).unwrap().as_map().cloned().unwrap_or_default()
    }

    /// The day a swap reads, with an untimed block, a locked one and a neighbour in tomorrow.
    const DAY: &str = r#"day:
  blocks:
    - {text: A note, event: d.blocks.0}
    - {time: "09:00", text: Museum, event: d.blocks.1}
    - {time: "11:00", text: Walk, event: d.blocks.2}
    - {time: "12:00", text: Train, event: d.blocks.3, locked: true}
tomorrow:
  blocks:
    - {time: "08:00", text: Bus, event: e.blocks.0}
    - {time: "09:00", text: Beach, event: e.blocks.1}
"#;

    /// The facts an edit writes, as a message shows them.
    fn edited(store: &Map, scope: &str, action: &str, args: &str) -> String {
        let facts = edit(store, &map(scope), action, &map(args));
        show(facts.as_ref().map(|f| Value::Map(f.clone())).as_ref())
    }

    #[test]
    fn a_distance_is_a_round_number_and_never_past_the_limit() {
        for bad in [Value::Null, Value::Bool(true), Value::String("30".into()), Value::Number(0.5)]
        {
            assert_eq!(whole(Some(&bad)), 0, "{bad:?}");
        }
        assert_eq!(whole(None), 0);
        assert_eq!(whole(Some(&Value::Number(-30.0))), -30);
        assert_eq!(whole(Some(&Value::Number(5000.0))), LIMIT);
        assert_eq!(whole(Some(&Value::Number(-5000.0))), -LIMIT);
    }

    #[test]
    fn moving_a_time_never_pushes_it_out_of_its_own_day() {
        assert_eq!(moved("00:10", -60), "00:00");
        assert_eq!(moved("23:30", 120), "23:59");
        assert_eq!(moved("10:30", 0), "10:30");
        assert_eq!(moved("10:30", 45), "11:15");
        assert_eq!(moved("10:30", -45), "09:45");
    }

    #[test]
    fn a_block_added_needs_an_hour_and_something_to_say() {
        let store = map(r#"added.2026-04-11:
  - {time: "25:00", text: Never}
  - {time: "09:30", text: ""}
  - {time: "10:00"}
  - {text: Coffee}
  - {time: "11:00", text: Market}
"#);
        assert!(added(&Map::default(), "2026-04-11").is_empty());
        assert!(added(&store, "2026-04-12").is_empty());
        let kept = added(&store, "2026-04-11");
        assert_eq!(kept.len(), 1);
        // The place in the stored list is the id, so dropping one never renames another.
        assert_eq!(
            show(Some(&Value::Map(kept[0].clone()))),
            r#"{"time": "11:00", "text": "Market", "event": "2026-04-11.added.4"}"#
        );
    }

    #[test]
    fn with_no_edit_stored_a_day_keeps_the_hours_the_pack_gave_it() {
        let mut blocks = vec![map("time: \"09:00\"\nevent: d.blocks.0")];
        assert!(!replan(&map("plan.other: {shift: 30}"), &mut blocks));
        assert_eq!(text(blocks[0].get("time")), "09:00");
    }

    #[test]
    fn a_resize_moves_what_follows_until_a_locked_hour_holds_the_rest() {
        let store = map(r#"plan.d.blocks.1: {grow: 30}
plan.d.blocks.3: {shift: 60}
plan.d.blocks.4: {off: true}
"#);
        let mut blocks = vec![
            map("time: \"12:00\"\ntext: Lunch\nevent: d.blocks.4"),
            map("time: \"14:00\"\ntext: Lunch\nevent: d.blocks.5"),
            map("time: \"12:00\"\ntext: Train\nevent: d.blocks.3\nlocked: true"),
            map("text: A note\nevent: d.blocks.0"),
            map("time: \"09:00\"\ntext: Museum\nuntil: \"11:00\"\nevent: d.blocks.1"),
            map("time: \"11:00\"\ntext: Walk\nevent: d.blocks.2"),
        ];
        assert!(replan(&store, &mut blocks));
        let hours: Vec<String> = blocks
            .iter()
            .map(|b| format!("{} {}", text(b.get("time")), text(b.get("until"))))
            .collect();
        // The note has no hour, the dropped lunch is gone, and the train never moves, edit or not.
        assert_eq!(hours, [" ", "09:00 11:30", "11:30 ", "12:00 ", "14:00 "]);
    }

    #[test]
    fn an_action_that_is_not_the_timelines_is_left_for_the_host() {
        for other in ["", "timeline", "timeline.nudge", "places.open"] {
            assert_eq!(edited(&Map::default(), "", other, ""), "nothing", "{other}");
        }
    }

    #[test]
    fn an_edit_the_arguments_do_not_make_sense_for_writes_nothing() {
        // Still ours, so a typo in a pack stops here and never reaches a tool on the device.
        for (action, args) in [
            ("timeline.move", "by: 30"),
            ("timeline.move", "block: d.blocks.1\nby: 0"),
            ("timeline.grow", "block: d.blocks.1\nby: soon"),
            ("timeline.drop", "by: 30"),
            ("timeline.restore", "by: 30"),
            ("timeline.add", "date: 2026-04-11\ntime: \"09:00\""),
            ("timeline.add", "date: 2026-04-11\ntime: nine\ntext: Coffee"),
            ("timeline.add", "date: 2026-02-30\ntime: \"09:00\"\ntext: Coffee"),
        ] {
            assert_eq!(edited(&Map::default(), "", action, args), "{}", "{action} {args}");
        }
    }

    #[test]
    fn nothing_swaps_with_a_block_that_is_missing_locked_or_at_the_end() {
        for (scope, args) in [
            ("", "block: d.blocks.1\nside: up"),
            ("day: {}", "block: d.blocks.1\nside: up"),
            ("day: {blocks: here}", "block: d.blocks.1\nside: up"),
            (DAY, "block: d.blocks.9\nside: up"),
            (DAY, "block: d.blocks.0\nside: down"),
            (DAY, "block: d.blocks.1\nside: sideways"),
            (DAY, "block: d.blocks.1\nside: up"),
            (DAY, "block: e.blocks.1\nside: down"),
            (DAY, "block: d.blocks.2\nside: down"),
        ] {
            assert_eq!(edited(&Map::default(), scope, "timeline.swap", args), "{}", "{args}");
        }
    }

    #[test]
    fn a_swap_gives_each_block_the_others_hour() {
        let both = edited(&Map::default(), DAY, "timeline.swap", "block: d.blocks.2\nside: up");
        assert_eq!(
            both,
            r#"{"plan.d.blocks.2": {"shift": -120}, "plan.d.blocks.1": {"shift": 120}}"#
        );
    }

    #[test]
    fn moving_and_growing_add_up_on_the_block_they_name() {
        let mut store = Map::default();
        let mut apply = |action: &str, by: i64| {
            let args = format!("block: d.blocks.1\nby: {by}");
            for (key, v) in edit(&store, &Map::default(), action, &map(&args)).unwrap().iter() {
                store.set(key, v.clone());
            }
        };
        apply("timeline.move", 30);
        apply("timeline.move", -10);
        apply("timeline.grow", 15);
        apply("timeline.grow", 5000);
        assert_eq!(show(store.get("plan.d.blocks.1")), r#"{"shift": 20, "grow": 720}"#);
    }

    #[test]
    fn dropping_a_block_marks_it_off_and_restoring_forgets_every_edit() {
        let args = "block: d.blocks.1";
        let drop = edited(&Map::default(), "", "timeline.drop", args);
        assert_eq!(drop, r#"{"plan.d.blocks.1": {"off": true}}"#);
        let back = edited(&Map::default(), "", "timeline.restore", args);
        assert_eq!(back, r#"{"plan.d.blocks.1": null}"#);
    }

    #[test]
    fn a_day_takes_only_so_many_blocks_added_to_it() {
        let row = "{time: \"09:00\", text: Coffee}";
        let full = map(&format!("added.2026-04-11: [{}]", vec![row; ADDED].join(", ")));
        let args = "date: 2026-04-11\ntime: \"18:00\"\ntext: Market";
        assert_eq!(edited(&full, "", "timeline.add", args), "{}");
        let one = edited(&Map::default(), "", "timeline.add", args);
        assert_eq!(one, r#"{"added.2026-04-11": [{"time": "18:00", "text": "Market"}]}"#);
    }
}
