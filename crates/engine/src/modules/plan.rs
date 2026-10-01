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
    // A move some block cannot take whole is refused, never half applied.
    let fits = |moves: &Vec<(String, i64)>| {
        let shift = |e: &str| whole(store.get(&format!("plan.{e}")).and_then(|w| w.get("shift")));
        moves.iter().all(|(e, by)| (shift(e) + by).abs() <= LIMIT)
    };
    // An action of ours with arguments that make no sense writes nothing. It never falls through
    // to the host: `timeline` is the engine's, so a typo in a pack must not reach a device tool.
    match action {
        "timeline.move" | "timeline.grow" => {
            if !event.is_empty() && by != 0 {
                bump(&event, if action == "timeline.move" { "shift" } else { "grow" }, by);
            }
        }
        "timeline.swap" | "timeline.reorder" => {
            let at = |t: &str| minutes(&format!("2000-01-01T{t}"));
            let moves = if action == "timeline.swap" {
                let side = neighbours(scope, &event, &text(args.get("side")));
                side.map(|(a, b, other)| {
                    vec![(event.clone(), at(&b) - at(&a)), (other, at(&a) - at(&b))]
                })
            } else {
                let to =
                    args.get("to").filter(|t| matches!(t, Value::Number(n) if n.fract() == 0.0));
                to.and_then(|to| reorder(scope, &event, whole(Some(to))))
            };
            for (e, by) in moves.filter(fits).unwrap_or_default() {
                bump(&e, "shift", by);
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

/// The timed blocks of the day the timeline exposed that holds `event`, legs left out, and where
/// `event` is among them.
fn timed<'s>(scope: &'s Map, event: &str) -> Option<(Vec<&'s Value>, usize)> {
    ["day", "tomorrow"].into_iter().find_map(|d| {
        let blocks: Vec<&Value> = (scope.get(d)?.get("blocks")?.as_list()?.iter())
            .filter(|b| !text(b.get("time")).is_empty() && !truthy(b.get("leg")))
            .collect();
        let at = blocks.iter().position(|b| text(b.get("event")) == event)?;
        Some((blocks, at))
    })
}

/// The times of the block `event` and of the timed neighbour on `side` (`up` or `down`) of it, with
/// that neighbour's id, in the day the timeline exposed. `None` when either end is missing or
/// locked: a locked hour is fixed, so nothing swaps with it.
fn neighbours(scope: &Map, event: &str, side: &str) -> Option<(String, String, String)> {
    let (day, at) = timed(scope, event)?;
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

/// The shifts that put block `event` at place `to` of its day, the blocks between taking its
/// place in turn; each keeps its slot, from its start to the next one's. None across a locked hour.
fn reorder(scope: &Map, event: &str, to: i64) -> Option<Vec<(String, i64)>> {
    let (day, i) = timed(scope, event)?;
    let j = usize::try_from(to).ok().filter(|j| *j < day.len() && *j != i)?;
    if day[i.min(j)..=i.max(j)].iter().any(|b| truthy(b.get("locked"))) {
        return None;
    }
    let at = |t: &str| minutes(&format!("2000-01-01T{t}"));
    let start: Vec<i64> = day.iter().map(|b| at(&text(b.get("time")))).collect();
    let slot = |k: usize| match start.get(k + 1) {
        Some(next) => next - start[k],
        None => Some(text(day[k].get("until")))
            .filter(|u| !u.is_empty())
            .map(|u| at(&u) - start[k])
            .filter(|m| *m > 0)
            .unwrap_or(60),
    };
    let id = |k: usize| text(day[k].get("event"));
    let (s, mut shifts) = (slot(i), Vec::with_capacity(i.abs_diff(j) + 1));
    if i < j {
        shifts.extend((i + 1..=j).map(|k| (id(k), -s)));
        shifts.push((id(i), start[j] + slot(j) - s - start[i]));
    } else {
        shifts.extend((j..i).map(|k| (id(k), s)));
        shifts.push((id(i), start[j] - start[i]));
    }
    Some(shifts)
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

    /// A day to reorder, with a leg the page put in and a locked hour at its end.
    const ROW: &str = r#"day:
  blocks:
    - {time: "09:00", text: Museum, event: d.blocks.0}
    - {time: "10:45", text: → Market, leg: true}
    - {time: "11:00", text: Market, event: d.blocks.1}
    - {time: "12:00", text: Lunch, event: d.blocks.2}
    - {time: "14:00", text: Train, event: d.blocks.3, locked: true}
tomorrow:
  blocks:
    - {time: "08:00", text: Bus, event: e.blocks.0}
    - {time: "09:00", text: Beach, event: e.blocks.1, until: "12:00"}
"#;

    #[test]
    fn a_swap_passes_over_the_leg_between_two_blocks() {
        let both = edited(&Map::default(), ROW, "timeline.swap", "block: d.blocks.1\nside: up");
        assert_eq!(
            both,
            r#"{"plan.d.blocks.1": {"shift": -120}, "plan.d.blocks.0": {"shift": 120}}"#
        );
    }

    #[test]
    fn a_block_moved_down_the_day_takes_the_slot_after_the_ones_it_passes() {
        let down = edited(&Map::default(), ROW, "timeline.reorder", "block: d.blocks.0\nto: 2");
        assert_eq!(
            down,
            r#"{"plan.d.blocks.1": {"shift": -120}, "plan.d.blocks.2": {"shift": -120}, "plan.d.blocks.0": {"shift": 180}}"#
        );
        let up = edited(&Map::default(), ROW, "timeline.reorder", "block: d.blocks.2\nto: 0");
        assert_eq!(
            up,
            r#"{"plan.d.blocks.0": {"shift": 120}, "plan.d.blocks.1": {"shift": 120}, "plan.d.blocks.2": {"shift": -180}}"#
        );
    }

    #[test]
    fn the_last_block_of_a_day_lasts_to_its_until_or_else_an_hour() {
        let args = "block: e.blocks.0\nto: 1";
        let beach = edited(&Map::default(), ROW, "timeline.reorder", args);
        assert_eq!(
            beach,
            r#"{"plan.e.blocks.1": {"shift": -60}, "plan.e.blocks.0": {"shift": 180}}"#
        );
        let odd = "day:\n  blocks:\n    - {time: \"08:00\", event: a}\n    - {time: \"09:00\", event: b, until: \"08:30\"}";
        let hour = edited(&Map::default(), odd, "timeline.reorder", "block: a\nto: 1");
        assert_eq!(hour, r#"{"plan.b": {"shift": -60}, "plan.a": {"shift": 60}}"#);
    }

    #[test]
    fn nothing_is_reordered_across_a_locked_hour_or_to_nowhere() {
        for args in [
            "block: d.blocks.9\nto: 1",
            "block: d.blocks.0\nto: 0",
            "block: d.blocks.0\nto: soon",
            "block: d.blocks.0\nto: 1.5",
            "block: d.blocks.0",
            "block: d.blocks.0\nto: -1",
            "block: d.blocks.0\nto: 4",
            "block: d.blocks.0\nto: 3",
            "block: d.blocks.3\nto: 0",
        ] {
            assert_eq!(edited(&Map::default(), ROW, "timeline.reorder", args), "{}", "{args}");
        }
    }

    #[test]
    fn a_move_one_block_cannot_take_whole_is_refused_for_all_of_them() {
        let near = map("plan.d.blocks.0: {shift: 600}\nplan.d.blocks.1: {shift: -700}");
        let args = "block: d.blocks.0\nto: 2";
        assert_eq!(edited(&near, ROW, "timeline.reorder", args), "{}");
        let swap = "block: d.blocks.1\nside: up";
        assert_eq!(edited(&near, ROW, "timeline.swap", swap), "{}");
        let fine = map("plan.d.blocks.0: {shift: 540}");
        assert!(edited(&fine, ROW, "timeline.reorder", args).contains(r#"{"shift": 720}"#));
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
