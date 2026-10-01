//! Blocks the person moved, resized, dropped or added on the device (decisions 0025, 0035). The
//! pack is a document, so none of it is written back: each edit is a stored fact, keyed by the
//! block's `event` id, and the timeline applies them every time it builds the day.

use super::travel::MOVING;
use super::{Run, list};
use crate::clock::{minutes, shift};
use crate::validate::patterns::{is_real_date, is_time};
use crate::value::{Map, Value, text, truthy};

/// The most minutes an edit moves or stretches one block, either way.
const LIMIT: i64 = 12 * 60;
/// The most blocks one day takes added to it.
const ADDED: usize = 32;
/// Minutes in a day: no edit takes a block out of its own.
const DAY: i64 = 24 * 60;
/// A drag's step, and the least a block is resized to.
const STEP: i64 = 15;

pub(crate) fn whole(v: Option<&Value>) -> i64 {
    match v {
        Some(Value::Number(n)) if n.fract() == 0.0 => (*n as i64).clamp(-LIMIT, LIMIT),
        _ => 0,
    }
}

/// Minutes from midnight to an `HH:MM`.
fn clock(time: &str) -> i64 {
    minutes(&format!("2000-01-01T{time}")) - minutes("2000-01-01T00:00")
}

/// Minutes from midnight as an `HH:MM`; the midnight that ends a day reads `00:00`.
fn hour(m: i64) -> String {
    shift("2000-01-01T00:00", m)[11..].to_owned()
}

/// An `HH:MM` moved by minutes, kept inside its day: an edit never pushes a block past midnight.
pub(crate) fn moved(time: &str, by: i64) -> String {
    hour((clock(time) + by).clamp(0, DAY - 1))
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

/// Applies the stored edits to a day's blocks and says whether any landed. A dropped block goes;
/// an edited one takes its `at` and `until`, or an older `shift` and `grow` on itself alone. A
/// `locked` one never moves.
pub(crate) fn replan(store: &Map, blocks: &mut Vec<Map>) -> bool {
    let mut landed = false;
    blocks.retain_mut(|b| {
        let Some(fact) = b.get("event").and_then(|e| store.get(&format!("plan.{}", text(Some(e)))))
        else {
            return true;
        };
        landed = true;
        if truthy(fact.get("off")) {
            return false;
        }
        if truthy(b.get("locked")) {
            return true;
        }
        let time = text(b.get("time"));
        if !is_time(&time) {
            return true;
        }
        let at = Some(text(fact.get("at"))).filter(|t| is_time(t));
        let by = at.map_or_else(|| whole(fact.get("shift")), |at| clock(&at) - clock(&time));
        b.set("time", Value::String(moved(&time, by)));
        let until = Some(text(fact.get("until"))).filter(|t| is_time(t));
        let old = Some(text(b.get("until"))).filter(|u| !u.is_empty());
        let until = until.or_else(|| old.map(|u| moved(&u, by + whole(fact.get("grow")))));
        if let Some(until) = until {
            b.set("until", Value::String(until));
        }
        true
    });
    landed
}

/// The facts a `timeline` drop, restore or add writes, or `None` for an action not the
/// timeline's. They change stored facts and touch nothing outside, so no command reaches the host
/// (decision 0025). A move or a resize needs the day, so `Run::reshape` writes those.
pub(crate) fn edit(store: &Map, action: &str, args: &Map) -> Option<Map> {
    let event = text(args.get("block"));
    let mut facts = Map::default();
    // An action of ours with arguments that make no sense writes nothing. It never falls through
    // to the host: `timeline` is the engine's, so a typo in a pack must not reach a device tool.
    match action {
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
        a if a.starts_with("timeline.") => {}
        _ => return None,
    }
    Some(facts)
}

/// A timed block as a reshape sees it, in minutes from the midnight of the block moved; as a row
/// of a settle, `at` is where it wants to start and `gap` what it needs after the row before.
#[derive(Clone, Copy)]
struct Slot {
    at: i64,
    len: i64,
    least: i64,
    /// Never moves: locked, begun, on another clock or past midnight.
    fixed: bool,
    free: bool,
    gap: i64,
}

/// Starts for rows kept in their order, each after the one before and its gap; the first stays.
/// A fixed row a push reaches, or one past `wall`, takes the room from the tight run before it:
/// `free` rows first, then the nearest, none below its `least`. `None` when that is not enough.
fn settle(rows: &mut [Slot], wall: i64) -> Option<Vec<i64>> {
    loop {
        let mut at = vec![rows[0].at];
        let mut short = None;
        for r in 1..rows.len() {
            let (row, after) = (rows[r], at[r - 1] + rows[r - 1].len + rows[r].gap);
            let start = if row.fixed { row.at } else { after.max(row.at) };
            at.push(start);
            short = if row.fixed {
                (after > start).then(|| (r - 1, after - start))
            } else {
                (start + row.len > wall).then(|| (r, start + row.len - wall))
            };
            if short.is_some() {
                break;
            }
        }
        let Some((y, mut need)) = short else {
            return Some(at);
        };
        // Back from the wall while each row was pushed by the one before it.
        let mut run = Vec::new();
        let mut k = y;
        while k > 0 {
            run.push(k);
            if at[k] <= rows[k].at {
                break;
            }
            k -= 1;
        }
        run.sort_by_key(|&k| !rows[k].free);
        for k in run {
            let take = (rows[k].len - rows[k].least).clamp(0, need);
            rows[k].len -= take;
            need -= take;
        }
        if need > 0 {
            return None;
        }
    }
}

impl Run<'_> {
    /// The facts `timeline.move {block, by}` or `timeline.resize {block, edge, by}` stores: that
    /// block and each one it moves or squeezes, as its own hours (decision 0035). Empty when the
    /// change does not fit.
    pub(crate) fn reshape(&self, data: Option<&Value>, action: &str, args: &Map) -> Option<Map> {
        let (days, moving) = (list(data), action == "timeline.move");
        let (event, edge) = (text(args.get("block")), text(args.get("edge")));
        let by = whole(args.get("by"));
        let date = event.get(..10).filter(|d| is_real_date(d))?;
        let day = days.iter().find(|d| self.date_of(d) == date)?;
        let chosen = self.options(days, day, true).1;
        let travel = self.keymap.value("travel", self.read(day, "day", "travel"));
        let timed = |edited| {
            let page = self.page(date, day, &self.in_force(day, chosen, edited));
            let timed = |b: &Map| !text(b.get("time")).is_empty() && !truthy(b.get("leg"));
            page.into_iter().filter(timed).collect::<Vec<Map>>()
        };
        let mut blocks = timed(true);
        blocks.sort_by_cached_key(|b| minutes(&self.begins(date, b)));
        let pack = timed(false);
        let i = blocks.iter().position(|b| text(b.get("event")) == event)?;
        let zone = text(blocks[i].get("zone"));
        let base = minutes(&self.stamp(date, "00:00", &zone));
        let now = minutes(&self.world.now) - base;
        let floor = now.max(0);
        let other = |b: &Map| minutes(&self.stamp(date, "00:00", &text(b.get("zone")))) != base;
        let lasts = |b: &Map| text(b.get("lasts")).parse::<i64>().unwrap_or(0);
        let slot = |b: &Map| {
            let (at, len, kind) =
                (minutes(&self.begins(date, b)) - base, lasts(b), text(b.get("type")));
            let fixed = truthy(b.get("locked"))
                || at <= now
                || other(b)
                || b.get("until_zone").is_some()
                || at + len > DAY;
            let given = pack.iter().find(|p| p.get("event") == b.get("event")).map_or(len, lasts);
            let least = if fixed || MOVING.contains(&kind.as_str()) {
                len
            } else {
                STEP.max((given + 1) / 2)
            };
            Slot { at, len, least, fixed, free: kind == "free", gap: 0 }
        };
        let slots: Vec<Slot> = blocks.iter().map(slot).collect();

        // The block itself: never one locked, over or ending on another clock, and of the one in
        // progress only its end.
        let (s, e) = (slots[i].at, slots[i].at + slots[i].len);
        let end = !moving && edge == "end";
        let wall = truthy(blocks[i].get("locked")) || blocks[i].get("until_zone").is_some();
        if wall || e <= now || (s <= now && !end) {
            return None;
        }
        let (ts, te) = match (moving, edge.as_str()) {
            (true, _) => (s + by, e + by),
            (false, "start") => (s + by, e),
            (false, "end") => (s, e + by),
            _ => return None,
        };
        let len = te - ts;
        let rides = MOVING.contains(&text(blocks[i].get("type")).as_str());
        if by == 0 || (len < e - s && (len < STEP || rides)) {
            return None;
        }

        // Past a neighbour's middle the two trade places; a fixed block is never passed.
        let n = slots.len();
        let mid = |k: usize| 2 * slots[k].at + slots[k].len;
        let (m0, m1) = (s + e, ts + te);
        let passes = |k: &usize, later: bool| {
            let m = mid(*k);
            let past = if later { m0 < m && m <= m1 } else { m1 <= m && m < m0 };
            moving && !slots[*k].fixed && past
        };
        let ahead = (i + 1..n).take_while(|k| passes(k, true)).count();
        let behind = (0..i).rev().take_while(|k| passes(k, false)).count();
        let mut order: Vec<usize> = (0..n).collect();
        order[i..=i + ahead].rotate_left(1);
        order[i - behind..=i].rotate_right(1);
        let at = i + ahead - behind;
        let crossed = ahead + behind > 0;

        let leg = |a: usize, b: usize| self.way(&blocks[a], &blocks[b], &travel).map_or(0, |w| w.0);
        let slack = |a: usize, b: usize| slots[b].at - slots[a].at - slots[a].len - leg(a, b);
        // What a pair needs between them: the leg, less an overlap the pack already had there.
        let gap = |p: usize, k: usize| {
            let overlap = if p + 1 == k { (-slack(p, k)).max(0) } else { 0 };
            leg(p, k) - overlap
        };

        // What touched it after it follows it earlier; a crossing closes the hole it left.
        let pull = if crossed && i + 1 < n {
            let from = |k: usize| i.checked_sub(1).map_or(0, |p| leg(p, k));
            slots[i + 1].at - (s - from(i) + from(i + 1))
        } else {
            e - te
        };
        let mut want: Vec<i64> = slots.iter().map(|s| s.at).collect();
        let mut k = i + 1;
        while pull > 0 && k < n && !slots[k].fixed && slack(k - 1, k) <= 0 {
            want[k] -= pull;
            k += 1;
        }
        let mut ts = ts;
        if let Some(p) = at.checked_sub(1).filter(|_| crossed).map(|r| order[r]) {
            ts = ts.max(want[p] + slots[p].len + gap(p, i));
        }
        let te = ts + len;
        if te <= floor || te > DAY || (ts < floor && ts != s) {
            return None;
        }

        // Settle each side of it: later ones as they are, earlier ones mirrored.
        let anchor = Slot { at: ts, len, ..slots[i] };
        let mut later = vec![anchor];
        later.extend(order[at..].windows(2).map(|w| Slot {
            at: want[w[1]],
            gap: gap(w[0], w[1]),
            ..slots[w[1]]
        }));
        let mut earlier = vec![Slot { at: -te, ..anchor }];
        earlier.extend(order[..=at].windows(2).rev().map(|w| Slot {
            at: -(want[w[0]] + slots[w[0]].len),
            gap: gap(w[0], w[1]),
            ..slots[w[0]]
        }));
        let (ends, starts) = (settle(&mut later, DAY)?, settle(&mut earlier, -floor)?);
        let mut fin: Vec<(i64, i64)> = slots.iter().map(|s| (s.at, s.at + s.len)).collect();
        for (r, &k) in order[at..].iter().enumerate() {
            fin[k] = (ends[r], ends[r] + later[r].len);
        }
        for (r, &k) in order[..=at].iter().rev().enumerate() {
            fin[k] = (-(starts[r] + earlier[r].len), -starts[r]);
        }

        // A block that keeps its hours stores nothing, unless the next one moving would stretch it.
        let mut facts = Map::default();
        for (r, &k) in order.iter().enumerate() {
            let (b, (s, e)) = (&blocks[k], fin[k]);
            let reads = order.get(r + 1).map_or(s + 60, |&n| (fin[n].0 - leg(k, n)).max(s));
            let has = !text(b.get("until")).is_empty();
            let same = (s, e) == (slots[k].at, slots[k].at + slots[k].len);
            if truthy(b.get("locked")) || other(b) || (same && (has || e == reads)) {
                continue;
            }
            if s >= DAY {
                return None;
            }
            let mut fact = Map(vec![("at".to_owned(), Value::String(hour(s)))]);
            if has || e != reads {
                fact.set("until", Value::String(hour(e)));
            }
            facts.set(&format!("plan.{}", text(b.get("event"))), Value::Map(fact));
        }
        Some(facts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::World;
    use crate::validate::Keymap;
    use crate::value::show;
    use crate::yaml::parse;

    /// The mapping this YAML writes.
    fn map(src: &str) -> Map {
        parse(src).unwrap().as_map().cloned().unwrap_or_default()
    }

    /// The facts an edit writes, as a message shows them.
    fn edited(store: &Map, action: &str, args: &str) -> String {
        let facts = edit(store, action, &map(args));
        show(facts.as_ref().map(|f| Value::Map(f.clone())).as_ref())
    }

    /// A morning to the train and a day near midnight; the walk is free time.
    const DAYS: &str = r#"days:
  - date: 2026-04-11
    blocks:
      - ["09:00", Breakfast, {until: "10:00"}]
      - ["10:00", Museum, {until: "12:00"}]
      - ["12:00", Lunch, {until: "13:00"}]
      - ["13:00", Walk, {type: free, until: "14:00"}]
      - ["14:00", Train, {type: train, until: "15:00", locked: true}]
      - ["15:00", Beach]
  - date: 2026-04-12
    blocks:
      - ["21:00", Dinner, {until: "23:00"}]
      - ["23:00", Night walk, {until: "23:45"}]
"#;

    /// The facts a move or resize stores at `now`, as `block at-until` for each.
    fn reshape(now: &str, content: &str, action: &str, args: &str) -> String {
        reshape_on("{}", now, content, action, args)
    }

    /// `reshape` over the edits already in `store`.
    fn reshape_on(store: &str, now: &str, content: &str, action: &str, args: &str) -> String {
        let content = map(content);
        // The pack's own clock, and Paris an hour ahead of it.
        let zones = [("Europe/Madrid", now.to_owned()), ("Europe/Paris", shift(now, 60))];
        let zones = Map(zones.map(|(z, t)| (z.to_owned(), Value::String(t))).to_vec());
        let world = World { now: now.into(), zones, store: map(store), ..World::default() };
        let mut run = Run::new(Keymap::of(&Value::Null), &content, &world, Map::default());
        run.places = content.get("places");
        let facts = run.reshape(content.get("days"), action, &map(args)).unwrap_or_default();
        let line = |(k, v): &(String, Value)| {
            let k = k.rsplit_once(".blocks.").map_or(k.as_str(), |(_, n)| n);
            format!("{k} {}-{}", text(v.get("at")), text(v.get("until")))
        };
        facts.0.iter().map(line).collect::<Vec<_>>().join(", ")
    }

    fn mv(now: &str, block: usize, by: i64) -> String {
        let args = format!("block: 2026-04-11.blocks.{block}\nby: {by}");
        reshape(&format!("2026-04-11T{now}"), DAYS, "timeline.move", &args)
    }

    fn resize(now: &str, block: usize, edge: &str, by: i64) -> String {
        let args = format!("block: 2026-04-11.blocks.{block}\nedge: {edge}\nby: {by}");
        reshape(&format!("2026-04-11T{now}"), DAYS, "timeline.resize", &args)
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
        assert_eq!(hour(DAY), "00:00");
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
        let mut blocks = vec![map("time: \"09:00\"\nevent: d.blocks.0"), map("text: A note")];
        assert!(!replan(&map("plan.other: {at: \"10:00\"}\nplan.: {off: true}"), &mut blocks));
        assert_eq!(text(blocks[0].get("time")), "09:00");
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn a_block_with_no_hour_takes_none_from_an_edit() {
        let mut blocks = vec![map("text: A note\nevent: d.blocks.0")];
        assert!(replan(&map("plan.d.blocks.0: {at: \"10:00\", shift: 30}"), &mut blocks));
        assert_eq!(text(blocks[0].get("time")), "");
    }

    #[test]
    fn stored_hours_move_a_block_and_an_older_shift_or_grow_only_its_own() {
        let store = map(r#"plan.d.blocks.1: {at: "09:30"}
plan.d.blocks.2: {at: "12:30", until: "13:15"}
plan.d.blocks.3: {at: "08:00"}
plan.d.blocks.4: {off: true}
plan.d.blocks.5: {shift: 30, grow: 15}
plan.d.blocks.6: {at: never, until: "25:00", grow: 60}
"#);
        let mut blocks = vec![
            map("time: \"09:00\"\ntext: Museum\nuntil: \"11:00\"\nevent: d.blocks.1"),
            map("time: \"11:00\"\ntext: Walk\nevent: d.blocks.2"),
            map("time: \"12:00\"\ntext: Train\nevent: d.blocks.3\nlocked: true"),
            map("time: \"14:00\"\ntext: Lunch\nevent: d.blocks.4"),
            map("time: \"15:00\"\ntext: Cafe\nuntil: \"16:00\"\nevent: d.blocks.5"),
            map("time: \"17:00\"\ntext: Market\nuntil: \"18:00\"\nevent: d.blocks.6"),
        ];
        assert!(replan(&store, &mut blocks));
        let hours: Vec<String> = blocks
            .iter()
            .map(|b| format!("{} {}", text(b.get("time")), text(b.get("until"))))
            .collect();
        // The train never moves and the lunch is gone; nothing carries to the next block.
        assert_eq!(hours, ["09:30 11:30", "12:30 13:15", "12:00 ", "15:30 16:45", "17:00 19:00"]);
    }

    #[test]
    fn an_action_that_is_not_the_timelines_is_left_for_the_host() {
        for other in ["", "timeline", "timelines.drop", "places.open"] {
            assert_eq!(edited(&Map::default(), other, ""), "nothing", "{other}");
        }
    }

    #[test]
    fn an_edit_the_arguments_do_not_make_sense_for_writes_nothing() {
        // Still ours, so a typo in a pack stops here and never reaches a tool on the device.
        for (action, args) in [
            ("timeline.drop", "by: 30"),
            ("timeline.restore", "by: 30"),
            ("timeline.add", "date: 2026-04-11\ntime: \"09:00\""),
            ("timeline.add", "date: 2026-04-11\ntime: nine\ntext: Coffee"),
            ("timeline.add", "date: 2026-02-30\ntime: \"09:00\"\ntext: Coffee"),
            ("timeline.grow", "block: d.blocks.1\nby: 30"),
        ] {
            assert_eq!(edited(&Map::default(), action, args), "{}", "{action} {args}");
        }
    }

    #[test]
    fn dropping_a_block_marks_it_off_and_restoring_forgets_every_edit() {
        let args = "block: d.blocks.1";
        let drop = edited(&Map::default(), "timeline.drop", args);
        assert_eq!(drop, r#"{"plan.d.blocks.1": {"off": true}}"#);
        let back = edited(&Map::default(), "timeline.restore", args);
        assert_eq!(back, r#"{"plan.d.blocks.1": null}"#);
    }

    #[test]
    fn a_day_takes_only_so_many_blocks_added_to_it() {
        let row = "{time: \"09:00\", text: Coffee}";
        let full = map(&format!("added.2026-04-11: [{}]", vec![row; ADDED].join(", ")));
        let args = "date: 2026-04-11\ntime: \"18:00\"\ntext: Market";
        assert_eq!(edited(&full, "timeline.add", args), "{}");
        let one = edited(&Map::default(), "timeline.add", args);
        assert_eq!(one, r#"{"added.2026-04-11": [{"time": "18:00", "text": "Market"}]}"#);
    }

    #[test]
    fn a_stretch_pushes_what_touches_it_and_squeezes_free_time_at_a_locked_block() {
        assert_eq!(mv("08:00", 1, 15), "1 10:15-12:15, 2 12:15-13:15, 3 13:15-14:00");
        assert_eq!(resize("08:00", 1, "end", 15), "1 10:00-12:15, 2 12:15-13:15, 3 13:15-14:00");
        // Free time first, then the nearest, none below half its length; then no fit.
        assert_eq!(resize("08:00", 1, "end", 60), "1 10:00-13:00, 2 13:00-13:30, 3 13:30-14:00");
        assert_eq!(resize("08:00", 1, "end", 90), "");
    }

    #[test]
    fn free_time_gives_first_and_none_below_half_the_packs_length() {
        let content = r#"days:
  - date: 2026-04-11
    blocks:
      - ["10:00", A, {until: "11:00"}]
      - ["11:00", Free, {type: free, until: "12:00"}]
      - ["12:00", B, {until: "13:00"}]
      - ["13:00", C, {until: "14:00", locked: true}]
"#;
        let args = "block: 2026-04-11.blocks.0\nedge: end\nby: 30";
        let free = reshape("2026-04-11T08:00", content, "timeline.resize", args);
        assert_eq!(free, "0 10:00-11:30, 1 11:30-12:00");
        // B was stretched to two hours; it shrinks to half of the hour the pack gave it.
        let content = r#"days:
  - date: 2026-04-11
    blocks:
      - ["10:00", A, {until: "11:00"}]
      - ["11:00", B, {until: "12:00"}]
      - ["13:00", C, {until: "14:00", locked: true}]
"#;
        let store = r#"plan.2026-04-11.blocks.1: {at: "11:00", until: "13:00"}"#;
        let squeeze = |by: i64| {
            let args = format!("block: 2026-04-11.blocks.0\nedge: end\nby: {by}");
            reshape_on(store, "2026-04-11T08:00", content, "timeline.resize", &args)
        };
        assert_eq!(squeeze(75), "0 10:00-12:15, 1 12:15-13:00");
        assert_eq!(squeeze(105), "");
    }

    #[test]
    fn a_block_that_ends_or_moves_earlier_pulls_what_touched_it_after_it() {
        assert_eq!(resize("08:00", 1, "end", -15), "1 10:00-11:45, 2 11:45-12:45, 3 12:45-13:45");
        assert_eq!(
            mv("08:00", 1, -15),
            "0 08:45-09:45, 1 09:45-11:45, 2 11:45-12:45, 3 12:45-13:45"
        );
        // A start moved later leaves its hole, and an earlier one pushes the ones before.
        assert_eq!(resize("08:00", 1, "start", 15), "1 10:15-12:00");
        assert_eq!(resize("08:00", 1, "start", -15), "0 08:45-09:45, 1 09:45-12:00");
        // Down to a quarter of an hour; the pull stops at a gap.
        assert_eq!(resize("08:00", 1, "end", -105), "1 10:00-10:15, 2 10:15-11:15, 3 11:15-12:15");
        let gap = "days: [{date: 2026-04-11, blocks: [[\"10:00\", A, {until: \"11:00\"}], [\"11:00\", B, {until: \"12:00\"}], [\"12:30\", C, {until: \"13:30\"}]]}]";
        let args = "block: 2026-04-11.blocks.0\nedge: end\nby: -15";
        let pulled = reshape("2026-04-11T08:00", gap, "timeline.resize", args);
        assert_eq!(pulled, "0 10:00-10:45, 1 10:45-11:45");
    }

    #[test]
    fn nothing_lands_before_now_and_what_is_pushed_there_shrinks() {
        assert_eq!(resize("08:00", 1, "start", -90), "0 08:00-08:30, 1 08:30-12:00");
        assert_eq!(resize("08:00", 1, "start", -150), "");
        // Breakfast is under way: a wall, and nothing goes before now.
        assert_eq!(mv("09:30", 1, -15), "");
        assert_eq!(mv("09:30", 1, -45), "");
        assert_eq!(mv("09:30", 2, -150), "2 10:00-11:00, 1 11:00-13:00");
    }

    #[test]
    fn of_the_block_in_progress_only_the_end_moves_and_never_before_now() {
        assert_eq!(
            resize("09:30", 0, "end", -15),
            "0 09:00-09:45, 1 09:45-11:45, 2 11:45-12:45, 3 12:45-13:45"
        );
        assert_eq!(resize("09:30", 0, "end", -45), "");
        for refused in [mv("09:30", 0, 15), resize("09:30", 0, "start", 15), mv("10:30", 0, 15)] {
            assert_eq!(refused, "");
        }
    }

    #[test]
    fn past_a_neighbours_middle_the_two_trade_places() {
        // Not yet past the lunch's middle: it is pushed.
        assert_eq!(mv("08:00", 1, 60), "1 11:00-13:00, 2 13:00-13:30, 3 13:30-14:00");
        assert_eq!(mv("08:00", 1, 90), "2 10:00-11:00, 1 11:30-13:30, 3 13:30-14:00");
        // Up past the museum it lands after the breakfast at the least.
        assert_eq!(mv("08:00", 2, -120), "2 10:00-11:00, 1 11:00-13:00");
        assert_eq!(mv("08:00", 2, -90), "2 10:30-11:30, 1 11:30-13:30, 3 13:30-14:00");
    }

    #[test]
    fn a_locked_block_never_moves_or_is_passed_and_midnight_ends_the_day() {
        assert_eq!(mv("08:00", 4, 15), "");
        assert_eq!(mv("08:00", 3, 120), "");
        let late = |block: usize, action: &str, by: i64| {
            let args = format!("block: 2026-04-12.blocks.{block}\nedge: end\nby: {by}");
            reshape("2026-04-11T08:00", DAYS, action, &args)
        };
        assert_eq!(late(0, "timeline.move", 30), "0 21:30-23:30, 1 23:30-00:00");
        assert_eq!(late(0, "timeline.move", 60), "");
        assert_eq!(late(1, "timeline.resize", 30), "");
    }

    #[test]
    fn a_block_with_no_end_of_its_own_keeps_its_length_when_the_next_moves() {
        assert_eq!(mv("08:00", 5, 15), "5 15:15-");
        let content = "days: [{date: 2026-04-11, blocks: [[\"10:00\", A], [\"11:00\", B, {until: \"12:00\"}]]}]";
        let args = "block: 2026-04-11.blocks.1\nby: 30";
        let moved = reshape("2026-04-11T08:00", content, "timeline.move", args);
        assert_eq!(moved, "0 10:00-11:00, 1 11:30-12:30");
    }

    #[test]
    fn a_leg_takes_its_minutes_and_another_clock_is_a_wall() {
        let content = r#"places: {old: {at: {lat: 40.0, lon: 0.0}, legs: {new: 25}}, new: {at: {lat: 40.01, lon: 0.0}}}
days:
  - date: 2026-04-11
    blocks:
      - ["09:00", Old, {place: old, until: "10:00"}]
      - ["10:30", New, {place: new}]
      - ["12:00", Paris, {zone: Europe/Paris, until: "13:00"}]
"#;
        let mv = |block: usize, by: i64| {
            let args = format!("block: 2026-04-11.blocks.{block}\nby: {by}");
            reshape("2026-04-11T08:00", content, "timeline.move", &args)
        };
        // The five minutes to spare go first, then the leg pushes.
        assert_eq!(mv(0, 15), "0 09:15-10:15, 1 10:40-");
        assert_eq!(mv(1, 60), "");
        assert_eq!(mv(1, -15), "0 08:50-09:50, 1 10:15-10:45");
        // The pack's own zone, written out, is the same clock.
        let own = "days: [{date: 2026-04-11, blocks: [[\"10:00\", A, {zone: Europe/Madrid, until: \"11:00\"}], [\"11:00\", B, {until: \"12:00\"}]]}]";
        let args = "block: 2026-04-11.blocks.0\nby: 30";
        let moved = reshape("2026-04-11T08:00", own, "timeline.move", args);
        assert_eq!(moved, "0 10:30-11:30, 1 11:30-12:30");
    }

    #[test]
    fn a_block_left_with_a_longer_leg_to_a_locked_one_shrinks_to_reach_it() {
        let content = r#"places: {bakery: {legs: {station: 80, shop: 5}}, shop: {legs: {station: 5}}, station: {}}
days:
  - date: 2026-04-11
    blocks:
      - ["10:00", Bakery, {place: bakery, until: "11:00"}]
      - ["11:05", Shop, {place: shop, until: "11:55"}]
      - ["12:00", Train, {place: station, until: "13:00", locked: true}]
"#;
        let mv = |by: i64| {
            let args = format!("block: 2026-04-11.blocks.1\nby: {by}");
            reshape("2026-04-11T08:00", content, "timeline.move", &args)
        };
        // The shop moved first, the bakery keeps its start and goes straight to the train.
        assert_eq!(mv(-155), "1 08:30-09:20, 0 10:00-10:40");
    }

    #[test]
    fn a_ride_moves_and_grows_but_never_shrinks() {
        let content = r#"days:
  - date: 2026-04-11
    blocks:
      - ["10:00", Bus, {type: transfer, until: "11:00"}]
      - ["11:00", Museum, {until: "12:00"}]
"#;
        let resize = |edge: &str, by: i64| {
            let args = format!("block: 2026-04-11.blocks.0\nedge: {edge}\nby: {by}");
            reshape("2026-04-11T08:00", content, "timeline.resize", &args)
        };
        assert_eq!(resize("end", -15), "");
        assert_eq!(resize("start", 15), "");
        assert_eq!(resize("start", -15), "0 09:45-11:00");
    }

    #[test]
    fn a_block_ending_on_another_clock_or_pushed_to_midnight_stays() {
        let content = r#"days:
  - date: 2026-04-11
    blocks:
      - ["08:30", Flight, {until: "13:00", until_zone: Europe/Paris}]
      - ["22:00", Dinner, {until: "23:00"}]
      - ["23:00", Drinks]
      - ["23:00", Show, {until: "23:30"}]
"#;
        let mv = |block: usize, by: i64| {
            let args = format!("block: 2026-04-11.blocks.{block}\nby: {by}");
            reshape("2026-04-11T08:00", content, "timeline.move", &args)
        };
        assert_eq!(mv(0, 15), "");
        assert_eq!(mv(2, 60), "");
    }

    #[test]
    fn a_reshape_with_nothing_to_go_on_stores_nothing() {
        for args in [
            "block: x\nby: 15",
            "block: 2026-04-30.blocks.0\nby: 15",
            "block: 2026-04-11.blocks.9\nby: 15",
            "block: 2026-04-11.blocks.1\nby: 0",
            "block: 2026-04-11.blocks.1\nby: soon",
        ] {
            assert_eq!(reshape("2026-04-11T08:00", DAYS, "timeline.move", args), "", "{args}");
        }
        let odd = "block: 2026-04-11.blocks.1\nedge: middle\nby: 15";
        assert_eq!(reshape("2026-04-11T08:00", DAYS, "timeline.resize", odd), "");
        assert_eq!(resize("08:00", 1, "end", -110), "");
        // Under a quarter of an hour a block only grows.
        let short = "days: [{date: 2026-04-11, blocks: [[\"10:00\", A, {until: \"10:05\"}]]}]";
        let edge = |edge: &str, by: i64| {
            let args = format!("block: 2026-04-11.blocks.0\nedge: {edge}\nby: {by}");
            reshape("2026-04-11T08:00", short, "timeline.resize", &args)
        };
        assert_eq!(edge("end", 15), "0 10:00-10:20");
        assert_eq!(edge("start", 15), "");
    }
}
