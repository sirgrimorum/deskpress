//! The engine a host holds: a pack that loaded, and its definition. For a world it answers which
//! screen shows and until when that answer holds (decision 0013).

use std::collections::{BTreeMap, BTreeSet, HashSet};

mod sync;

pub use sync::Request;

use crate::clock::{minutes, next_day, next_minute, shift};
use crate::define::{Definition, Does, Rule, define, fill};
use crate::expr::Expr;
use crate::modules::Run;
use crate::pack::Pack;
use crate::tree::{Conventions, Tree, build, outline};
use crate::validate::patterns::{is_real_date, is_stamp};
use crate::validate::{Finding, Keymap, validate};
use crate::value::{Map, Value, quote, text, truthy};

/// Everything outside the pack the answer depends on. The host passes it in; the engine reads no
/// clock and no sensor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct World {
    /// Local time in the pack's timezone, `YYYY-MM-DDTHH:MM`.
    pub now: String,
    /// The id of the person holding the phone.
    pub holder: String,
    /// The ids of the places whose region the device is inside, the smallest first.
    pub inside: Vec<String>,
    /// Whether the host knows where the device is. Without it `inside` says nothing.
    pub located: bool,
    /// The facts stored on the device, by key.
    pub store: Map,
    /// The local time now in each zone of `Engine::zones`, by zone name.
    pub zones: Map,
}

/// What would change the answer: the next instant, and the regions whose crossing matters.
#[derive(Debug, Clone, PartialEq)]
pub struct Watch {
    pub until: String,
    pub regions: Vec<Region>,
}

/// A place's circle on the map, for the host to watch.
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    pub radius_m: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub screen: String,
    /// The names the screen's expressions read.
    pub scope: Map,
    pub watch: Watch,
}

/// Where the person is inside the screen the rules picked. The host keeps it between calls and
/// never reads it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Nav {
    /// The screen the rules picked last. When they pick another, the stack starts over.
    pub rule: String,
    /// Never empty once the engine has seen it; the top is what shows.
    pub stack: Vec<Frame>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub screen: String,
    pub params: Map,
    pub state: Map,
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub tree: Tree,
    pub watch: Watch,
}

/// What an action did: the new view, the facts to store and the commands for the host.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub view: View,
    pub store: Map,
    pub commands: Vec<Command>,
}

/// A module action for the host to run, like `calendar.sync`, with the values it was given.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub name: String,
    pub args: Map,
}

/// A calendar event for a timed block, in the local time of its zones (decision 0021).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The block's `event`, stable while the block keeps its place in its list.
    pub id: String,
    pub start: String,
    pub end: String,
    /// The zone `start` and `reminder` are local to, and the one `end` is; empty for the pack's.
    pub zone: String,
    pub end_zone: String,
    pub title: String,
    pub location: String,
    pub notes: String,
    /// When to remind, from the `notify_from` of an alert at the block; empty for none.
    pub reminder: String,
    /// Changes whenever anything above does.
    pub fingerprint: String,
}

/// What a calendar sync would do to the events the host wrote before.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub add: Vec<Event>,
    pub change: Vec<Event>,
    /// Ids of events written before that the plan no longer has.
    pub remove: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Engine {
    pack: Pack,
    def: Definition,
}

/// The screen a pack with no rules shows.
pub const OUTLINE: &str = "outline";

impl Engine {
    /// The pack's content; empty when it is not a mapping.
    fn content(&self) -> &Map {
        static EMPTY: Map = Map(Vec::new());
        self.pack.content.as_map().unwrap_or(&EMPTY)
    }

    /// The root a module reads, as the pack names it.
    fn from(&self, module: &str) -> Option<&str> {
        self.def.modules.iter().find(|m| m.name == module).and_then(|m| m.from.as_deref())
    }

    /// The engine for a pack and its warnings, or every error that stops it loading.
    pub fn load(pack: Pack) -> Result<(Engine, Vec<Finding>), Vec<Finding>> {
        let report = validate(&pack.manifest, &pack.content, pack.theme.as_ref());
        if !report.ok() {
            return Err(report.errors);
        }
        let (def, _) = define(&pack.manifest);
        Ok((Engine { pack, def }, report.warnings))
    }

    pub fn pack(&self) -> &Pack {
        &self.pack
    }

    /// The zones the plan names besides the pack's own: every `zone` and `until_zone` under the
    /// days, once each, sorted. The host passes the local time in each as `World::zones`.
    pub fn zones(&self) -> Vec<String> {
        fn walk(v: &Value, into: &mut BTreeSet<String>) {
            match v {
                Value::Map(m) => m.iter().for_each(|(k, v)| match (k, v) {
                    ("zone" | "until_zone", Value::String(z)) => _ = into.insert(z.clone()),
                    _ => walk(v, into),
                }),
                Value::List(items) => items.iter().for_each(|v| walk(v, into)),
                _ => {}
            }
        }
        let content = self.content();
        let days =
            self.from("timeline").and_then(|d| Keymap::of(&self.pack.manifest).root(content, d));
        let mut zones = BTreeSet::new();
        days.into_iter().for_each(|d| walk(d, &mut zones));
        zones.into_iter().filter(|z| !z.is_empty()).collect()
    }

    /// Runs the modules, the derived names and the rules over `world`.
    pub fn decide(&self, world: &World) -> Result<Decision, String> {
        check(&world.now)?;
        Ok(self.run(world))
    }

    /// The screen on top for `world`, and when to ask again.
    pub fn screen(&self, world: &World, nav: &mut Nav) -> Result<View, String> {
        check(&world.now)?;
        let d = self.follow(world, nav);
        Ok(self.view(d, nav))
    }

    /// Runs `action` of the screen on top, with `arg` as `$arg`, then shows the screen again over
    /// the stored facts it changed.
    pub fn dispatch(
        &self,
        world: &World,
        nav: &mut Nav,
        action: &str,
        arg: Value,
    ) -> Result<Outcome, String> {
        check(&world.now)?;
        let d = self.follow(world, nav);
        let at = nav.stack.len() - 1;
        let name = nav.stack[at].screen.clone();
        let effects = self.def.screen(&name).and_then(|s| s.action(action)).ok_or_else(|| {
            format!("{} is not an action of the screen {}", quote(action), quote(&name))
        })?;
        let mut scope = d.scope;
        enter(&mut scope, &nav.stack[at]);
        scope.set("$arg", arg);
        let (mut store, mut patch, mut commands) =
            (world.store.clone(), Map::default(), Vec::new());
        for effect in effects {
            if effect.when.as_ref().is_some_and(|w| !truthy(Some(&w.eval(&scope)))) {
                continue;
            }
            match &effect.does {
                Does::Set(name, to) => {
                    let v = to.eval(&scope).into_owned();
                    if let Some(frame) = nav.stack.get_mut(at) {
                        frame.state.set(name, v.clone());
                    }
                    scope.set(name, v);
                }
                Does::Store(key, value) => {
                    let (key, v) = (fill(key, &scope), value.eval(&scope).into_owned());
                    store.set(&key, v.clone());
                    patch.set(&key, v);
                    scope.set("store", Value::Map(store.clone()));
                }
                Does::Open(screen, with) => {
                    let params = values(with, &scope);
                    let top = nav.stack.len() - 1;
                    // Leaving the top resets a screen's local state.
                    nav.stack[top] =
                        self.frame(&nav.stack[top].screen, nav.stack[top].params.clone());
                    nav.stack.push(self.frame(screen, params));
                }
                Does::Back => {
                    if nav.stack.len() > 1 {
                        nav.stack.pop();
                    }
                }
                Does::Home => nav.stack.truncate(1),
                Does::Command(name, with) => {
                    commands.push(Command { name: name.clone(), args: values(with, &scope) });
                }
            }
        }
        let world = World { store, ..world.clone() };
        let d = self.follow(&world, nav);
        Ok(Outcome { view: self.view(d, nav), store: patch, commands })
    }

    /// The calendar sync for `scope`: the whole trip when empty, a day by its date, or one event
    /// by its id. `known` is what the host wrote before, each event id with its fingerprint.
    pub fn calendar(
        &self,
        world: &World,
        scope: &str,
        known: &BTreeMap<String, String>,
    ) -> Result<Plan, String> {
        check(&world.now)?;
        let content = self.content();
        let run = Run::new(Keymap::of(&self.pack.manifest), content, world, Map::default());
        let within = |id: &str| {
            scope.is_empty()
                || id == scope
                || id.strip_prefix(scope).is_some_and(|r| r.starts_with('.'))
        };
        let mut events =
            run.events(self.from("timeline"), self.from("places"), self.from("alerts"));
        events.retain(|e| within(&e.id));
        let ids: HashSet<&str> = events.iter().map(|e| e.id.as_str()).collect();
        let remove = known.keys().filter(|k| within(k) && !ids.contains(k.as_str())).cloned();
        let mut plan = Plan { remove: remove.collect(), ..Plan::default() };
        for e in events {
            match known.get(&e.id) {
                None => plan.add.push(e),
                Some(f) if *f != e.fingerprint => plan.change.push(e),
                Some(_) => {}
            }
        }
        Ok(plan)
    }

    /// Decides, and starts the stack over when the rules picked another screen.
    fn follow(&self, world: &World, nav: &mut Nav) -> Decision {
        let d = self.run(world);
        if nav.rule != d.screen || nav.stack.is_empty() {
            let frame = self.frame(&d.screen, Map::default());
            *nav = Nav { rule: d.screen.clone(), stack: vec![frame] };
        }
        d
    }

    fn frame(&self, screen: &str, params: Map) -> Frame {
        let state = self.def.screen(screen).map(|s| s.state.clone()).unwrap_or_default();
        Frame { screen: screen.to_owned(), params, state }
    }

    fn view(&self, d: Decision, nav: &Nav) -> View {
        let top = &nav.stack[nav.stack.len() - 1];
        let holder = d.scope.get("holder").cloned().unwrap_or(Value::Null);
        let mut tree = match self.def.screen(&top.screen) {
            None => outline(&self.pack),
            Some(screen) => {
                let mut scope = d.scope;
                enter(&mut scope, top);
                build(&top.screen, screen, &mut scope, &Conventions::of(&self.pack.manifest))
            }
        };
        tree.theme = text(holder.get("theme"));
        tree.kid = holder.get("adult") == Some(&Value::Bool(false));
        View { tree, watch: d.watch }
    }

    fn run(&self, world: &World) -> Decision {
        let now = world.now.as_str();
        let (date, time) = (&now[..10], &now[11..]);
        let manifest = &self.pack.manifest;
        let string = |s: &str| Value::String(s.to_owned());
        let scope = Map(vec![
            (
                "now".into(),
                Value::Map(Map(vec![
                    ("date".into(), string(date)),
                    ("time".into(), string(time)),
                    ("stamp".into(), string(now)),
                ])),
            ),
            ("content".into(), self.pack.content.clone()),
            ("ui".into(), manifest.get("ui").cloned().unwrap_or(Value::Null)),
            ("store".into(), Value::Map(world.store.clone())),
        ]);
        let content = self.content();
        let mut run = Run::new(Keymap::of(manifest), content, world, scope);
        for m in &self.def.modules {
            run.module(m);
        }
        for (name, e) in &self.def.derive {
            let value = e.eval(&run.scope).into_owned();
            run.scope.set(name, value);
        }
        let holds = |r: &&Rule| r.when.as_ref().is_none_or(|w| truthy(Some(&w.eval(&run.scope))));
        let rule = self.def.rules.iter().find(holds);
        let screen = rule.map_or(OUTLINE.to_owned(), |r| r.screen.clone());

        // The earliest instant still to come; the next midnight always is one, since the day changes.
        // A `day.time` clock is on the day's own clock: moved by how far that is from the pack's.
        let day = run.scope.get("day");
        let field = |k: &str| text(day.and_then(|d| d.get(k)));
        let (on, here) = (field("date"), field("time"));
        let off = if here.is_empty() { 0 } else { minutes(now) - minutes(&format!("{on}T{here}")) };
        let clocks = self.def.clocks.iter().filter(|c| !c.day || !here.is_empty()).map(|c| {
            let at = match (c.at.len(), c.day) {
                (5, true) => shift(&format!("{on}T{}", c.at), off),
                (5, false) => format!("{date}T{}", c.at),
                _ => c.at.clone(),
            };
            if c.later { next_minute(&at) } else { at }
        });
        let midnight = format!("{}T00:00", next_day(date));
        // When an automatic sync falls due, the host is to be asked again, and it fetches then.
        let due = self.def.modules.iter().filter_map(|m| {
            let every = m.sync.as_ref()?.every?;
            Some(Self::due(world, m.name, every))
        });
        let instants = run.until.into_iter().chain(clocks).chain([midnight]).chain(due);
        let until = instants.filter(|u| u.as_str() > now).min().unwrap_or_default();
        let watch = Watch { until, regions: run.regions };
        Decision { screen, scope: run.scope, watch }
    }
}

fn check(now: &str) -> Result<(), String> {
    if is_stamp(now) && is_real_date(&now[..10]) {
        return Ok(());
    }
    Err(format!("{} is not a time: YYYY-MM-DDTHH:MM, in the pack's timezone", quote(now)))
}

/// Each name with the value of its expression.
fn values(with: &[(String, Expr)], scope: &Map) -> Map {
    Map(with.iter().map(|(k, e)| (k.clone(), e.eval(scope).into_owned())).collect())
}

/// Adds what a screen's expressions read besides the decision: its `params` and its state.
fn enter(scope: &mut Map, frame: &Frame) {
    scope.set("params", Value::Map(frame.params.clone()));
    for (name, v) in frame.state.iter() {
        scope.set(name, v.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::show;
    use crate::yaml::parse;

    const HEAD: &str =
        "pack: {id: test, name: Test, language: en, timezone: Europe/Lisbon, content: c.yaml}\n";

    const CONTENT: &str = r#"days:
  - date: 2026-04-11
    title: Arrive
    blocks:
      - ["10:30", "Walk"]
      - ["11:15", "Museum", {place: azulejo, until: "13:00"}]
      - ["18:40", "Ferry"]
places:
  azulejo: {name: Museum, at: {lat: 1, lon: 1}}
"#;

    const DEFINITION: &str = "ui: {today: Today}
modules:
  timeline:
  places:
derive:
  late: \"now.time >= '18:00'\"
  after_noon: \"now.time > '12:00'\"
  before_eight: \"now.stamp < '2026-04-11T20:00'\"
  busy: block and not late
screens: {moment: {}, evening: {}, idle: {}}
rules:
  - {when: late, screen: evening}
  - {when: busy, screen: moment}
  - {screen: idle}
";

    fn engine(definition: &str) -> Engine {
        let pack = Pack {
            manifest: parse(&format!("{HEAD}{definition}")).unwrap(),
            content: parse(CONTENT).unwrap(),
            theme: None,
        };
        Engine::load(pack).unwrap().0
    }

    fn at(now: &str) -> World {
        World { now: now.into(), ..World::default() }
    }

    #[test]
    fn a_calendar_sync_adds_changes_and_removes_only_within_its_scope() {
        let e = engine(DEFINITION);
        let none = BTreeMap::new();
        assert!(e.calendar(&World::default(), "", &none).is_err());
        let w = World { now: "2026-04-10T09:00".into(), ..World::default() };
        let first = e.calendar(&w, "", &none).unwrap();
        let ids = |l: &[Event]| l.iter().map(|e| e.id.clone()).collect::<Vec<_>>();
        let day = ["2026-04-11.blocks.0", "2026-04-11.blocks.1", "2026-04-11.blocks.2"];
        assert_eq!(
            (ids(&first.add), first.change.len(), first.remove.len()),
            (day.map(String::from).to_vec(), 0, 0)
        );
        let mut known: BTreeMap<String, String> =
            first.add.iter().map(|e| (e.id.clone(), e.fingerprint.clone())).collect();
        for (id, print) in [
            (day[0], "x"),
            ("2026-04-11.blocks.9", "y"),
            ("2026-04-11x", "z"),
            ("2026-04-12.fixed.0", "z"),
        ] {
            known.insert(id.into(), print.into());
        }
        let plan = e.calendar(&w, "2026-04-11", &known).unwrap();
        assert_eq!(
            (plan.add.len(), ids(&plan.change), plan.remove),
            (0, vec![day[0].to_owned()], vec!["2026-04-11.blocks.9".to_owned()])
        );
        let one = e.calendar(&w, "2026-04-11.blocks.9", &known).unwrap();
        assert_eq!(
            (one.add.len(), one.change.len(), one.remove),
            (0, 0, vec!["2026-04-11.blocks.9".to_owned()])
        );
        let trip = e.calendar(&w, "", &known).unwrap();
        assert_eq!(trip.remove, ["2026-04-11.blocks.9", "2026-04-11x", "2026-04-12.fixed.0"]);
        let bare = engine("screens: {idle: {}}\nrules: [{screen: idle}]\n");
        assert_eq!(bare.calendar(&w, "", &none).unwrap(), Plan::default());
    }

    #[test]
    fn the_zones_are_the_ones_the_days_name_once_each() {
        assert!(engine(DEFINITION).zones().is_empty());
        assert!(engine("screens: {idle: {}}\nrules: [{screen: idle}]\n").zones().is_empty());
        let content = r#"days:
  - date: 2026-04-11
    title: Arrive
    zone: Asia/Tokyo
    blocks:
      - ["10:30", "Walk", {zone: Asia/Seoul, until: "11:00", until_zone: Asia/Tokyo}]
      - ["11:15", "Museum", {zone: ""}]
zone: Europe/Paris
"#;
        let pack = Pack {
            manifest: parse(&format!("{HEAD}{DEFINITION}")).unwrap(),
            content: parse(content).unwrap(),
            theme: None,
        };
        assert_eq!(Engine::load(pack).unwrap().0.zones(), ["Asia/Seoul", "Asia/Tokyo"]);
    }

    #[test]
    fn a_day_clock_is_watched_on_the_days_own_clock() {
        let content = "days: [{date: 2026-04-11, title: A, zone: Asia/Tokyo}]\n";
        let definition = "modules: {timeline: }\nderive: {night: \"day.time >= '19:00'\"}\nscreens: {a: {}}\nrules: [{screen: a}]\n";
        let pack = Pack {
            manifest: parse(&format!("{HEAD}{definition}")).unwrap(),
            content: parse(content).unwrap(),
            theme: None,
        };
        let e = Engine::load(pack).unwrap().0;
        // Tokyo is nine hours on, so its 19:00 is 10:00 here, before its midnight at 15:00.
        let mut w = at("2026-04-11T08:00");
        w.zones = Map(vec![("Asia/Tokyo".into(), Value::String("2026-04-11T17:00".into()))]);
        assert_eq!(e.decide(&w).unwrap().watch.until, "2026-04-11T10:00");
        // With no day today there is no day clock, and the day changes at midnight.
        assert_eq!(e.decide(&at("2026-04-10T16:00")).unwrap().watch.until, "2026-04-11T00:00");
    }

    #[test]
    fn a_pack_with_errors_does_not_load() {
        let pack = Pack {
            manifest: parse(&format!("{HEAD}modules: {{weather: {{}}}}")).unwrap(),
            content: parse(CONTENT).unwrap(),
            theme: None,
        };
        let errors = Engine::load(pack).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].at, "modules.weather");
    }

    #[test]
    fn a_pack_that_loads_keeps_its_warnings() {
        let pack = Pack {
            manifest: parse(HEAD).unwrap(),
            content: parse(&format!("{CONTENT}people: [{{id: rita}}]\n")).unwrap(),
            theme: None,
        };
        let (engine, warnings) = Engine::load(pack).unwrap();
        assert_eq!(warnings[0].message, "a person with no name shows up as their id");
        assert_eq!(engine.pack().name(), Some("Test".into()));
    }

    #[test]
    fn the_tree_carries_the_holders_theme_and_whether_a_child_holds_it() {
        let people =
            "people: [{id: rita, adult: true, theme: rita-light}, {id: tomas, adult: false}]";
        let pack = Pack {
            manifest: parse(&format!("{HEAD}modules: {{people: }}")).unwrap(),
            content: parse(&format!("{CONTENT}{people}\n")).unwrap(),
            theme: None,
        };
        let e = Engine::load(pack).unwrap().0;
        let view = |holder: &str| {
            let world = World { holder: holder.into(), ..at("2026-04-11T10:00") };
            let tree = e.screen(&world, &mut Nav::default()).unwrap().tree;
            (tree.theme, tree.kid)
        };
        assert_eq!(view(""), (String::new(), false));
        assert_eq!(view("rita"), ("rita-light".into(), false));
        assert_eq!(view("tomas"), (String::new(), true));
    }

    #[test]
    fn the_clock_has_to_be_a_real_local_time() {
        let e = engine("");
        for now in ["", "2026-04-11", "2026-04-11 10:00", "2026-02-30T10:00", "2026-04-11T24:00"] {
            let message = e.decide(&at(now)).unwrap_err();
            assert_eq!(
                message,
                format!("{} is not a time: YYYY-MM-DDTHH:MM, in the pack's timezone", quote(now))
            );
        }
    }

    #[test]
    fn with_no_rules_the_screen_is_the_outline_until_midnight() {
        let d = engine("").decide(&at("2026-04-11T10:00")).unwrap();
        assert_eq!(d.screen, OUTLINE);
        assert_eq!(d.watch, Watch { until: "2026-04-12T00:00".into(), regions: vec![] });
        let keys: Vec<&str> = d.scope.keys().collect();
        assert_eq!(keys, ["now", "content", "ui", "store"]);
        assert_eq!(
            show(d.scope.get("now")),
            r#"{"date": "2026-04-11", "time": "10:00", "stamp": "2026-04-11T10:00"}"#
        );
        assert_eq!(show(d.scope.get("ui")), "null");
    }

    #[test]
    fn the_first_rule_that_holds_picks_the_screen() {
        let e = engine(DEFINITION);
        let mut world = at("2026-04-11T11:30");
        world.store = Map(vec![("seen".into(), Value::Bool(true))]);
        let d = e.decide(&world).unwrap();
        assert_eq!(d.screen, "moment");
        assert_eq!(show(d.scope.get("store")), r#"{"seen": true}"#);
        assert_eq!(show(d.scope.get("ui")), r#"{"today": "Today"}"#);
        assert_eq!(show(d.scope.get("busy")), "true");
        assert_eq!(d.watch.regions.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["azulejo"]);
        assert_eq!(e.decide(&at("2026-04-11T13:05")).unwrap().screen, "idle");
        assert_eq!(e.decide(&at("2026-04-11T18:00")).unwrap().screen, "evening");
    }

    #[test]
    fn the_watch_is_the_earliest_instant_still_to_come() {
        let e = engine(DEFINITION);
        let until = |now: &str| e.decide(&at(now)).unwrap().watch.until;
        // The minute after noon, since the test is "later than 12:00".
        assert_eq!(until("2026-04-11T11:30"), "2026-04-11T12:01");
        // The block ends at 13:00.
        assert_eq!(until("2026-04-11T12:30"), "2026-04-11T13:00");
        assert_eq!(until("2026-04-11T13:05"), "2026-04-11T18:00");
        assert_eq!(until("2026-04-11T18:10"), "2026-04-11T18:40");
        assert_eq!(until("2026-04-11T19:00"), "2026-04-11T20:00");
        assert_eq!(until("2026-04-11T21:00"), "2026-04-12T00:00");
        assert_eq!(until("2026-12-31T23:59"), "2027-01-01T00:00");
    }

    #[test]
    fn the_same_world_gives_the_same_answer() {
        let e = engine(DEFINITION);
        let world = at("2026-04-11T11:30");
        assert_eq!(e.decide(&world), e.decide(&world));
    }

    const MACHINE: &str = "modules:
  timeline:
screens:
  moment:
    state: {picked: null}
    actions:
      pick: [{set: picked, to: $arg}]
      confirm:
        - {if: picked, store: \"choice.{now.date}\", value: picked}
        - {if: store.choice, set: picked, to: \"'never'\"}
        - {do: calendar.sync, with: {day: now.date}}
      detail: [{set: picked, to: \"'gone'\"}, {open: info, with: {what: block.text}}]
      finish: [{store: done, value: true}]
      stay: [back, home]
    layout:
      - BigValue: {text: block.text}
      - Label: {text: \"picked {picked}\", on_tap: pick}
  info:
    state: {seen: false}
    actions:
      deeper: [{open: info, with: {what: \"'more'\"}}]
      up: [back]
      top: [home]
      leave: [back, {set: seen, to: true}]
      finish: [{store: done, value: true}]
    layout:
      - Label: {text: params.what}
  done:
    layout:
      - Label: {text: \"'done'\"}
rules:
  - {when: store.done, screen: done}
  - {screen: moment}
";

    fn texts(view: &View) -> Vec<String> {
        view.tree.nodes.iter().map(|n| text(Some(&n.props[0].1))).collect()
    }

    fn act(e: &Engine, nav: &mut Nav, action: &str, arg: Value) -> Outcome {
        e.dispatch(&at("2026-04-11T11:30"), nav, action, arg).unwrap()
    }

    #[test]
    fn a_world_with_no_real_time_is_refused_by_every_call() {
        let (e, mut nav) = (engine(MACHINE), Nav::default());
        let message = "\"\" is not a time: YYYY-MM-DDTHH:MM, in the pack's timezone";
        assert_eq!(e.screen(&at(""), &mut nav).unwrap_err(), message);
        assert_eq!(e.dispatch(&at(""), &mut nav, "pick", Value::Null).unwrap_err(), message);
        assert_eq!(nav, Nav::default());
    }

    #[test]
    fn with_no_rules_the_screen_is_the_outline() {
        let mut nav = Nav::default();
        let view = engine("").screen(&at("2026-04-11T10:00"), &mut nav).unwrap();
        assert_eq!(view.tree.screen, OUTLINE);
        assert_eq!(texts(&view), ["Test", "Arrive"]);
        assert_eq!(view.watch.until, "2026-04-12T00:00");
        assert_eq!(nav.rule, OUTLINE);
        let refused = engine("").dispatch(&at("2026-04-11T10:00"), &mut nav, "pick", Value::Null);
        assert_eq!(refused.unwrap_err(), "\"pick\" is not an action of the screen \"outline\"");
    }

    #[test]
    fn the_screen_the_rules_pick_is_drawn_over_its_state() {
        let (e, mut nav) = (engine(MACHINE), Nav::default());
        let view = e.screen(&at("2026-04-11T11:30"), &mut nav).unwrap();
        assert_eq!(view.tree.screen, "moment");
        assert_eq!(texts(&view), ["Museum", "picked "]);
        assert_eq!(view.tree.nodes[1].on, [("tap".to_owned(), "pick".to_owned())]);
        assert_eq!(view.watch, Watch { until: "2026-04-11T13:00".into(), regions: vec![] });
        assert_eq!(nav.stack.len(), 1);
        let refused = e.dispatch(&at("2026-04-11T11:30"), &mut nav, "up", Value::Null);
        assert_eq!(refused.unwrap_err(), "\"up\" is not an action of the screen \"moment\"");
    }

    #[test]
    fn set_keeps_a_value_on_the_screen_until_it_leaves_the_top() {
        let (e, mut nav) = (engine(MACHINE), Nav::default());
        let out = act(&e, &mut nav, "pick", Value::String("b".into()));
        assert_eq!(texts(&out.view), ["Museum", "picked b"]);
        assert_eq!((out.store, out.commands), (Map::default(), vec![]));
        // The state lives in the nav, so the next call still has it.
        let view = e.screen(&at("2026-04-11T12:00"), &mut nav).unwrap();
        assert_eq!(texts(&view), ["Museum", "picked b"]);
        // Opening a screen over it resets it, even what the same action set just before.
        let out = act(&e, &mut nav, "detail", Value::Null);
        assert_eq!(
            (out.view.tree.screen.as_str(), texts(&out.view)),
            ("info", vec!["Museum".to_owned()])
        );
        assert_eq!(nav.stack[0].state.get("picked"), Some(&Value::Null));
        let out = act(&e, &mut nav, "deeper", Value::Null);
        assert_eq!(texts(&out.view), ["more"]);
        assert_eq!(nav.stack.len(), 3);
        act(&e, &mut nav, "up", Value::Null);
        assert_eq!(nav.stack.len(), 2);
        act(&e, &mut nav, "deeper", Value::Null);
        let out = act(&e, &mut nav, "top", Value::Null);
        assert_eq!(texts(&out.view), ["Museum", "picked "]);
        // Back and home on the first screen stay on it.
        act(&e, &mut nav, "stay", Value::Null);
        assert_eq!(nav.stack.len(), 1);
    }

    #[test]
    fn a_set_after_its_screen_is_gone_changes_nothing() {
        let (e, mut nav) = (engine(MACHINE), Nav::default());
        act(&e, &mut nav, "detail", Value::Null);
        let out = act(&e, &mut nav, "leave", Value::Null);
        assert_eq!(out.view.tree.screen, "moment");
        assert_eq!(nav.stack.len(), 1);
        assert_eq!(nav.stack[0].state.get("seen"), None);
    }

    #[test]
    fn store_and_commands_go_to_the_host_and_the_effects_after_them_see_the_facts() {
        let (e, mut nav) = (engine(MACHINE), Nav::default());
        let out = act(&e, &mut nav, "confirm", Value::Null);
        let day = Map(vec![("day".to_owned(), Value::String("2026-04-11".into()))]);
        let sync = Command { name: "calendar.sync".into(), args: day };
        assert_eq!((out.store, out.commands), (Map::default(), vec![sync]));
        act(&e, &mut nav, "pick", Value::String("b".into()));
        let out = act(&e, &mut nav, "confirm", Value::Null);
        let stored = vec![("choice.2026-04-11".to_owned(), Value::String("b".into()))];
        assert_eq!(out.store, Map(stored));
        // `store.choice` is not the key just stored, so the second effect is skipped.
        assert_eq!(texts(&out.view), ["Museum", "picked b"]);
    }

    #[test]
    fn a_stored_fact_can_move_the_rules_and_the_stack_starts_over() {
        let (e, mut nav) = (engine(MACHINE), Nav::default());
        act(&e, &mut nav, "detail", Value::Null);
        let out = act(&e, &mut nav, "finish", Value::Null);
        assert_eq!(out.store, Map(vec![("done".to_owned(), Value::Bool(true))]));
        assert_eq!(
            (out.view.tree.screen.as_str(), texts(&out.view)),
            ("done", vec!["done".to_owned()])
        );
        assert_eq!((nav.rule.as_str(), nav.stack.len()), ("done", 1));
    }
}
