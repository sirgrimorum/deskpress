//! The engine a host holds: a pack that loaded, and its definition. For a world it answers which
//! screen shows and until when that answer holds (decision 0013).

use crate::clock::{next_day, next_minute};
use crate::define::{Definition, Does, Rule, define, fill};
use crate::modules::Run;
use crate::pack::Pack;
use crate::tree::{Conventions, Tree, build, outline};
use crate::validate::patterns::{is_real_date, is_stamp};
use crate::validate::{Finding, Keymap, validate};
use crate::value::{Map, Value, quote, truthy};

/// Everything outside the pack the answer depends on. The host passes it in; the engine reads no
/// clock and no sensor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct World {
    /// Local time in the pack's timezone, `YYYY-MM-DDTHH:MM`.
    pub now: String,
    /// The id of the person holding the phone.
    pub holder: String,
    /// The ids of the places whose region the device is inside.
    pub inside: Vec<String>,
    /// The facts stored on the device, by key.
    pub store: Map,
}

/// What would change the answer: the next instant, and the regions whose crossing matters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watch {
    pub until: String,
    pub regions: Vec<String>,
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
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Engine {
    pack: Pack,
    def: Definition,
}

/// The screen a pack with no rules shows.
pub const OUTLINE: &str = "outline";

impl Engine {
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
                    let params = with.iter().map(|(k, e)| (k.clone(), e.eval(&scope).into_owned()));
                    let params = Map(params.collect());
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
                Does::Command(c) => commands.push(c.clone()),
            }
        }
        let world = World { store, ..world.clone() };
        let d = self.follow(&world, nav);
        Ok(Outcome { view: self.view(d, nav), store: patch, commands })
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
        let tree = match self.def.screen(&top.screen) {
            None => outline(&self.pack),
            Some(screen) => {
                let mut scope = d.scope;
                enter(&mut scope, top);
                build(&top.screen, screen, &mut scope, &Conventions::of(&self.pack.manifest))
            }
        };
        View { tree, watch: d.watch }
    }

    fn run(&self, world: &World) -> Decision {
        let now = world.now.as_str();
        let (date, time) = (&now[..10], &now[11..]);
        let manifest = &self.pack.manifest;
        let text = |s: &str| Value::String(s.to_owned());
        let scope = Map(vec![
            (
                "now".into(),
                Value::Map(Map(vec![
                    ("date".into(), text(date)),
                    ("time".into(), text(time)),
                    ("stamp".into(), text(now)),
                ])),
            ),
            ("content".into(), self.pack.content.clone()),
            ("ui".into(), manifest.get("ui").cloned().unwrap_or(Value::Null)),
            ("store".into(), Value::Map(world.store.clone())),
        ]);
        let empty = Map::default();
        let content = self.pack.content.as_map().unwrap_or(&empty);
        let mut run = Run::new(Keymap::of(manifest), content, world, scope);
        for m in &self.def.modules {
            run.module(m.name, &m.from);
        }
        for (name, e) in &self.def.derive {
            let value = e.eval(&run.scope).into_owned();
            run.scope.set(name, value);
        }
        let holds = |r: &&Rule| r.when.as_ref().is_none_or(|w| truthy(Some(&w.eval(&run.scope))));
        let rule = self.def.rules.iter().find(holds);
        let screen = rule.map_or(OUTLINE.to_owned(), |r| r.screen.clone());

        // The earliest instant still to come; the next midnight always is one, since the day changes.
        let clocks = self.def.clocks.iter().map(|c| {
            let at = if c.at.len() == 5 { format!("{date}T{}", c.at) } else { c.at.clone() };
            if c.later { next_minute(&at) } else { at }
        });
        let midnight = format!("{}T00:00", next_day(date));
        let instants = run.until.into_iter().chain(clocks).chain([midnight]);
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
            manifest: parse(&format!("{HEAD}modules:\n  climate: {{sync: {{}}}}\n")).unwrap(),
            content: parse(CONTENT).unwrap(),
            theme: None,
        };
        let (engine, warnings) = Engine::load(pack).unwrap();
        assert_eq!(warnings[0].at, "modules.climate.sync");
        assert_eq!(engine.pack().name(), Some("Test".into()));
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
        assert_eq!(d.watch.regions, ["azulejo"]);
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
        - calendar.sync
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
        view.tree.nodes.iter().map(|n| crate::value::text(Some(&n.props[0].1))).collect()
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
        assert_eq!((out.store, out.commands), (Map::default(), vec!["calendar.sync".to_owned()]));
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
