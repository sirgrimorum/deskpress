//! Screens: each one a machine of local state, actions and a layout, read at load like the rules.

use super::Definer;
use crate::expr::Expr;
use crate::validate::patterns::is_id;
use crate::value::{Map, Value, quote, text};

/// The components a layout can use. The set grows only with a shell release (decision 0005).
pub const COMPONENTS: [&str; 12] = [
    "BigValue",
    "Label",
    "Card",
    "Row",
    "PhraseRow",
    "Alert",
    "Chip",
    "Button",
    "Segmented",
    "Missing",
    "Auto",
    "Screen",
];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Screen {
    /// The local values and what each starts as.
    pub state: Map,
    pub actions: Vec<(String, Vec<Effect>)>,
    pub layout: Vec<Component>,
}

impl Screen {
    pub fn action(&self, name: &str) -> Option<&[Effect]> {
        self.actions.iter().find(|(n, _)| n == name).map(|(_, e)| e.as_slice())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub kind: String,
    /// Repeats the component once per item of this list, read as `item`.
    pub each: Option<Expr>,
    /// Hides the component when false.
    pub when: Option<Expr>,
    pub props: Vec<(String, Prop)>,
    /// Event to action: `on_tap: pick` is `("tap", "pick")`.
    pub on: Vec<(String, String)>,
}

/// A prop: an expression, or a text whose `{expr}` pieces are filled in.
#[derive(Debug, Clone, PartialEq)]
pub enum Prop {
    Expr(Expr),
    Template(Vec<Piece>),
}

impl Prop {
    pub fn eval(&self, scope: &Map) -> Value {
        match self {
            Prop::Expr(e) => e.eval(scope).into_owned(),
            Prop::Template(pieces) => Value::String(fill(pieces, scope)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    Text(String),
    Expr(Expr),
}

/// A template's text, with every expression replaced by its value.
pub fn fill(pieces: &[Piece], scope: &Map) -> String {
    let piece = |p: &Piece| match p {
        Piece::Text(t) => t.clone(),
        Piece::Expr(e) => text(Some(&e.eval(scope))),
    };
    pieces.iter().map(piece).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    /// Skipped when false.
    pub when: Option<Expr>,
    pub does: Does,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Does {
    Set(String, Expr),
    /// The key is a template, like `choice.{day.date}`.
    Store(Vec<Piece>, Expr),
    Open(String, Vec<(String, Expr)>),
    Back,
    Home,
    /// A module's action, like `calendar.sync`, for the host to run.
    Command(String),
}

const EFFECT: &str = "has to be an effect: back, home, a module action like calendar.sync, or {set, to}, {store, value}, {open, with} or {do}";

impl Definer {
    pub(super) fn screens(&mut self, screens: Option<&Value>) {
        let Some(screens) = screens else {
            return;
        };
        let Some(screens) = screens.as_map() else {
            self.r.error(
                "screens",
                "has to be a mapping of screen name to its state, actions and layout",
            );
            return;
        };
        let names: Vec<&str> = screens.keys().collect();
        for (name, screen) in screens.iter() {
            let at = format!("screens.{name}");
            if !is_id(name) {
                let message = format!(
                    "{} is not a name: lowercase letters, digits and underscores",
                    quote(name)
                );
                self.r.error(at, message);
                continue;
            }
            let screen = self.screen(&at, screen, &names);
            self.def.screens.push((name.to_owned(), screen));
        }
    }

    fn screen(&mut self, at: &str, screen: &Value, names: &[&str]) -> Screen {
        let mut s = Screen::default();
        let empty = Map::default();
        let m = match screen {
            Value::Null => &empty,
            Value::Map(m) => m,
            _ => {
                self.r.error(at, "has to be a mapping: state, actions, layout");
                return s;
            }
        };
        for key in m.keys().filter(|k| !["state", "actions", "layout"].contains(k)) {
            let message = format!("{} is not part of a screen: state, actions, layout", quote(key));
            self.r.error(format!("{at}.{key}"), message);
        }
        let known = self.known.len();
        self.known.push("params".to_owned());
        s.state = self.state(at, m.get("state"));
        s.actions = self.actions(at, m.get("actions"), &s.state, names);
        s.layout = self.layout(at, m.get("layout"), &s);
        self.known.truncate(known);
        s
    }

    fn state(&mut self, at: &str, state: Option<&Value>) -> Map {
        let mut out = Map::default();
        let Some(state) = state.filter(|v| **v != Value::Null) else {
            return out;
        };
        let Some(state) = state.as_map() else {
            self.r
                .error(format!("{at}.state"), "has to be a mapping of name to its starting value");
            return out;
        };
        for (name, start) in state.iter() {
            if let Some(name) = self.name(&format!("{at}.state.{name}"), name) {
                out.set(&name, start.clone());
                self.known.push(name);
            }
        }
        out
    }

    /// `name` if it is an id that hides no other name, else an error at `at`.
    fn name(&mut self, at: &str, name: &str) -> Option<String> {
        if !is_id(name) {
            let message =
                format!("{} is not a name: lowercase letters, digits and underscores", quote(name));
            self.r.error(at, message);
            return None;
        }
        if self.known.iter().any(|k| k == name) {
            let message = format!("{} is already a name here, and this would hide it", quote(name));
            self.r.error(at, message);
            return None;
        }
        Some(name.to_owned())
    }

    fn actions(
        &mut self,
        at: &str,
        actions: Option<&Value>,
        state: &Map,
        names: &[&str],
    ) -> Vec<(String, Vec<Effect>)> {
        let mut out = Vec::new();
        let Some(actions) = actions.filter(|v| **v != Value::Null) else {
            return out;
        };
        let Some(actions) = actions.as_map() else {
            self.r.error(
                format!("{at}.actions"),
                "has to be a mapping of action name to its effects",
            );
            return out;
        };
        self.known.push("$arg".to_owned());
        for (name, effects) in actions.iter() {
            let here = format!("{at}.actions.{name}");
            let Some(list) = effects.as_list() else {
                self.r.error(&here, "has to be a list of effects, run in order");
                continue;
            };
            let effects = list.iter().enumerate();
            let effects =
                effects.filter_map(|(i, e)| self.effect(&format!("{here}[{i}]"), e, state, names));
            out.push((name.to_owned(), effects.collect()));
        }
        self.known.pop();
        out
    }

    fn effect(&mut self, at: &str, effect: &Value, state: &Map, names: &[&str]) -> Option<Effect> {
        let m = match effect {
            Value::String(s) => return self.does(at, s).map(|does| Effect { when: None, does }),
            Value::Map(m) => m,
            _ => {
                self.r.error(at, EFFECT);
                return None;
            }
        };
        let main: Vec<&str> =
            m.keys().filter(|k| ["set", "store", "open", "do"].contains(k)).collect();
        let [main] = main[..] else {
            self.r.error(at, EFFECT);
            return None;
        };
        let with = match main {
            "set" => "to",
            "store" => "value",
            "open" => "with",
            _ => "do",
        };
        for key in m.keys().filter(|k| ![main, with, "if"].contains(k)) {
            let message = format!("{} is not part of this effect: {main}, {with}, if", quote(key));
            self.r.error(format!("{at}.{key}"), message);
        }
        let when = m.get("if").map(|w| self.expr(&format!("{at}.if"), w));
        let value = || m.get(with).unwrap_or(&Value::Null);
        let does = match main {
            "set" => {
                let name = text(m.get("set"));
                let found = state.get(&name).is_some();
                if !found {
                    let known: Vec<&str> = state.keys().collect();
                    let message = format!(
                        "{} is not a state value of this screen: {}",
                        quote(&name),
                        known.join(", ")
                    );
                    self.r.error(format!("{at}.set"), message);
                }
                let to = self.expr(&format!("{at}.to"), value());
                to.filter(|_| found).map(|to| Does::Set(name, to))
            }
            "store" => {
                let key = self.template(&format!("{at}.store"), &text(m.get("store")));
                let v = self.expr(&format!("{at}.value"), value());
                key.zip(v).map(|(k, v)| Does::Store(k, v))
            }
            "open" => {
                let screen = text(m.get("open"));
                let found = names.contains(&screen.as_str());
                if !found {
                    let message = format!("{} is not one of the screens", quote(&screen));
                    self.r.error(format!("{at}.open"), message);
                }
                let params = self.params(&format!("{at}.with"), m.get("with"));
                params.filter(|_| found).map(|p| Does::Open(screen, p))
            }
            _ => self.does(&format!("{at}.do"), &text(m.get("do"))),
        };
        if when.as_ref().is_some_and(Option::is_none) {
            return None;
        }
        does.map(|does| Effect { when: when.flatten(), does })
    }

    /// An effect named by a word: back, home, or a module action.
    fn does(&mut self, at: &str, word: &str) -> Option<Does> {
        match word.split_once('.') {
            None if word == "back" => Some(Does::Back),
            None if word == "home" => Some(Does::Home),
            Some((module, action)) if is_id(module) && is_id(action) => {
                Some(Does::Command(word.to_owned()))
            }
            _ => {
                let message = format!(
                    "{} is not an effect: back, home, or a module action like calendar.sync",
                    quote(word)
                );
                self.r.error(at, message);
                None
            }
        }
    }

    fn params(&mut self, at: &str, with: Option<&Value>) -> Option<Vec<(String, Expr)>> {
        let Some(with) = with else {
            return Some(Vec::new());
        };
        let Some(with) = with.as_map() else {
            self.r.error(at, "has to be a mapping of name to expression, read as params.name");
            return None;
        };
        let mut out = Some(Vec::new());
        for (name, e) in with.iter() {
            let e = self.expr(&format!("{at}.{name}"), e);
            if !is_id(name) {
                let message = format!(
                    "{} is not a name: lowercase letters, digits and underscores",
                    quote(name)
                );
                self.r.error(format!("{at}.{name}"), message);
                out = None;
            }
            out = out.zip(e).map(|(mut out, e)| {
                out.push((name.to_owned(), e));
                out
            });
        }
        out
    }

    /// A text whose `{expr}` pieces are expressions.
    fn template(&mut self, at: &str, src: &str) -> Option<Vec<Piece>> {
        let mut pieces = Vec::new();
        let mut ok = true;
        let mut rest = src;
        while let Some(open) = rest.find('{') {
            if open > 0 {
                pieces.push(Piece::Text(rest[..open].to_owned()));
            }
            let Some(close) = rest[open..].find('}') else {
                self.r.error(at, format!("{}: a {{ with no }} to close it", quote(src)));
                return None;
            };
            match self.source(at, &rest[open + 1..open + close]) {
                Some(e) => pieces.push(Piece::Expr(e)),
                None => ok = false,
            }
            rest = &rest[open + close + 1..];
        }
        if !rest.is_empty() {
            pieces.push(Piece::Text(rest.to_owned()));
        }
        ok.then_some(pieces)
    }

    fn layout(&mut self, at: &str, layout: Option<&Value>, screen: &Screen) -> Vec<Component> {
        let Some(layout) = layout.filter(|v| **v != Value::Null) else {
            return Vec::new();
        };
        let Some(list) = layout.as_list() else {
            self.r.error(
                format!("{at}.layout"),
                "has to be a list of components, like - BigValue: {text: block.text}",
            );
            return Vec::new();
        };
        let components = list.iter().enumerate();
        let components =
            components.filter_map(|(i, c)| self.component(&format!("{at}.layout[{i}]"), c, screen));
        components.collect()
    }

    fn component(&mut self, at: &str, c: &Value, screen: &Screen) -> Option<Component> {
        let Some([(kind, props)]) = c.as_map().map(|m| &m.0[..]) else {
            self.r.error(
                at,
                "has to be one component and its props, like BigValue: {text: block.text}",
            );
            return None;
        };
        let at = format!("{at}.{kind}");
        if !COMPONENTS.contains(&kind.as_str()) {
            let message = format!("{} is not a component: {}", quote(kind), COMPONENTS.join(", "));
            self.r.error(at, message);
            return None;
        }
        let empty = Map::default();
        let props = match props {
            Value::Null => &empty,
            Value::Map(m) => m,
            _ => {
                self.r.error(at, "has to be a mapping of prop to expression");
                return None;
            }
        };
        let mut ok = true;
        let mut out = Component {
            kind: kind.clone(),
            each: None,
            when: None,
            props: Vec::new(),
            on: Vec::new(),
        };
        let each = props.get("each");
        if let Some(each) = each {
            out.each = self.expr(&format!("{at}.each"), each);
            ok &= out.each.is_some();
            self.known.push("item".to_owned());
        }
        for (key, v) in props.iter().filter(|(k, _)| *k != "each") {
            let here = format!("{at}.{key}");
            if key == "if" {
                out.when = self.expr(&here, v);
                ok &= out.when.is_some();
            } else if let Some(event) = key.strip_prefix("on_") {
                let action = text(Some(v));
                if screen.action(&action).is_none() {
                    let known: Vec<&str> = screen.actions.iter().map(|(n, _)| n.as_str()).collect();
                    let message = format!(
                        "{} is not an action of this screen: {}",
                        quote(&action),
                        known.join(", ")
                    );
                    self.r.error(here, message);
                    ok = false;
                } else {
                    out.on.push((event.to_owned(), action));
                }
            } else {
                let prop = match v {
                    Value::String(s) if s.contains('{') => {
                        self.template(&here, s).map(Prop::Template)
                    }
                    // A list is taken as it is written, like `skip: [time, text]`.
                    Value::List(_) => Some(Prop::Expr(Expr::Literal(v.clone()))),
                    _ => self.expr(&here, v).map(Prop::Expr),
                };
                ok &= prop.is_some();
                out.props.extend(prop.map(|p| (key.to_owned(), p)));
            }
        }
        if each.is_some() {
            self.known.pop();
        }
        ok.then_some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Definition, define};
    use super::*;
    use crate::expr::parse;
    use crate::yaml::parse as yaml;

    fn said(manifest: &str) -> String {
        let (_, r) = define(&yaml(manifest).unwrap());
        r.errors.iter().map(|f| format!("{}: {}", f.at, f.message)).collect::<Vec<_>>().join("\n")
    }

    fn def(manifest: &str) -> Definition {
        let (d, r) = define(&yaml(manifest).unwrap());
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        d
    }

    fn e(src: &str) -> Expr {
        parse(src).unwrap()
    }

    /// What is wrong with a screen `a` whose actions are `go: [effect]` and whose state is `n: 0`.
    fn effect(effect: &str) -> String {
        said(&format!("screens:\n  a: {{state: {{n: 0}}, actions: {{go: [{effect}]}}}}\n  b:\n"))
    }

    fn layout(layout: &str) -> String {
        said(&format!("screens:\n  a: {{actions: {{go: [back]}}, layout: {layout}}}\n"))
    }

    #[test]
    fn screens_are_a_mapping_of_names_to_mappings_of_known_keys() {
        assert_eq!(
            said("screens: [a]"),
            "screens: has to be a mapping of screen name to its state, actions and layout"
        );
        assert_eq!(
            said("screens: {Moment: \"\", b: 3, c: {look: x}}"),
            "screens.Moment: \"Moment\" is not a name: lowercase letters, digits and underscores\n\
             screens.b: has to be a mapping: state, actions, layout\n\
             screens.c.look: \"look\" is not part of a screen: state, actions, layout"
        );
        let d = def("screens:\n  a:\n  b:\n    state:\n    actions:\n    layout:\n");
        let empty = Screen::default();
        assert_eq!(d.screens, [("a".to_owned(), empty.clone()), ("b".to_owned(), empty.clone())]);
        assert_eq!(d.screen("b"), Some(&empty));
        assert_eq!(d.screen("c"), None);
    }

    #[test]
    fn state_names_are_new_ids_with_a_starting_value() {
        assert_eq!(
            said("screens: {a: {state: [x]}}"),
            "screens.a.state: has to be a mapping of name to its starting value"
        );
        assert_eq!(
            said("modules:\n  timeline:\nscreens: {a: {state: {Picked: 1, day: 2, params: 3}}}"),
            "screens.a.state.Picked: \"Picked\" is not a name: lowercase letters, digits and underscores\n\
             screens.a.state.day: \"day\" is already a name here, and this would hide it\n\
             screens.a.state.params: \"params\" is already a name here, and this would hide it"
        );
        let d = def("screens: {a: {state: {picked: null, n: 1}}, b: {state: {picked: x}}}");
        let state = yaml("picked: null\nn: 1").unwrap();
        assert_eq!(Some(&d.screens[0].1.state), state.as_map());
        // Each screen has its own names.
        assert_eq!(d.screens[1].1.state.get("picked"), Some(&Value::String("x".into())));
    }

    #[test]
    fn actions_are_lists_of_effects() {
        assert_eq!(
            said("screens: {a: {actions: [x]}}"),
            "screens.a.actions: has to be a mapping of action name to its effects"
        );
        assert_eq!(
            said("screens: {a: {actions: {go: back}}}"),
            "screens.a.actions.go: has to be a list of effects, run in order"
        );
        let d = def(
            "screens: {a: {actions: {go: [back, home, calendar.sync, {do: map.open, if: $arg}]}}}",
        );
        let effects = d.screens[0].1.action("go").unwrap();
        let does: Vec<&Does> = effects.iter().map(|e| &e.does).collect();
        let sync = Does::Command("calendar.sync".into());
        let map = Does::Command("map.open".into());
        assert_eq!(does, [&Does::Back, &Does::Home, &sync, &map]);
        assert_eq!(effects[3].when, Some(e("$arg")));
        assert_eq!(d.screens[0].1.action("stay"), None);
    }

    #[test]
    fn an_effect_is_a_word_or_a_mapping_that_does_one_thing() {
        let shape = "screens.a.actions.go[0]: has to be an effect: back, home, a module action like \
                     calendar.sync, or {set, to}, {store, value}, {open, with} or {do}";
        for e in ["3", "{if: true}", "{do: back, open: b}"] {
            assert_eq!(effect(e), shape);
        }
        let word = |w: &str| {
            format!("\"{w}\" is not an effect: back, home, or a module action like calendar.sync")
        };
        for w in ["jump", "Map.open", "map.", "map.open.now"] {
            assert_eq!(effect(w), format!("screens.a.actions.go[0]: {}", word(w)));
        }
        assert_eq!(
            effect("{do: forward}"),
            format!("screens.a.actions.go[0].do: {}", word("forward"))
        );
        assert_eq!(
            effect("{do: back, to: 1}"),
            "screens.a.actions.go[0].to: \"to\" is not part of this effect: do, do, if"
        );
        assert_eq!(
            effect("{set: n, to: 1, value: 2}"),
            "screens.a.actions.go[0].value: \"value\" is not part of this effect: set, to, if"
        );
        assert_eq!(
            effect("{do: back, if: nope}"),
            "screens.a.actions.go[0].if: column 1: \"nope\" is not a name here. \
             Known: now, content, ui, store, params, n, $arg"
        );
    }

    #[test]
    fn set_changes_a_state_value_of_its_own_screen() {
        assert_eq!(
            effect("{set: m, to: 1}"),
            "screens.a.actions.go[0].set: \"m\" is not a state value of this screen: n"
        );
        assert_eq!(
            effect("{set: n}"),
            "screens.a.actions.go[0].to: has to be an expression, like block.locked and not decision.answered"
        );
        let d =
            def("screens: {a: {state: {n: 0}, actions: {go: [{set: n, to: $arg, if: not n}]}}}");
        let set = Effect { when: Some(e("not n")), does: Does::Set("n".into(), e("$arg")) };
        assert_eq!(d.screens[0].1.actions[0].1, [set]);
    }

    #[test]
    fn store_keys_are_templates() {
        assert_eq!(
            effect("{store: \"choice.{day\", value: 1}"),
            "screens.a.actions.go[0].store: \"choice.{day\": a { with no } to close it"
        );
        assert_eq!(
            effect("{store: \"choice.{day.date}\", value: 1}"),
            "screens.a.actions.go[0].store: column 1: \"day\" is not a name here. \
             Known: now, content, ui, store, params, n, $arg"
        );
        let d = def(
            "screens: {a: {actions: {go: [{store: \"{now.date}.{now.time}!\", value: $arg}]}}}",
        );
        let key = vec![
            Piece::Expr(e("now.date")),
            Piece::Text(".".into()),
            Piece::Expr(e("now.time")),
            Piece::Text("!".into()),
        ];
        assert_eq!(d.screens[0].1.actions[0].1[0].does, Does::Store(key, e("$arg")));
    }

    #[test]
    fn open_pushes_a_known_screen_with_params() {
        assert_eq!(
            effect("{open: c}"),
            "screens.a.actions.go[0].open: \"c\" is not one of the screens"
        );
        assert_eq!(
            effect("{open: b, with: [x]}"),
            "screens.a.actions.go[0].with: has to be a mapping of name to expression, read as params.name"
        );
        assert_eq!(
            effect("{open: b, with: {Day: now.date, x: nope}}"),
            "screens.a.actions.go[0].with.Day: \"Day\" is not a name: lowercase letters, digits and underscores\n\
             screens.a.actions.go[0].with.x: column 1: \"nope\" is not a name here. \
             Known: now, content, ui, store, params, n, $arg"
        );
        let d = def(
            "screens:\n  a: {actions: {go: [{open: b, with: {day: now.date}}, {open: a}]}}\n  b:\n",
        );
        let does: Vec<&Does> = d.screens[0].1.actions[0].1.iter().map(|e| &e.does).collect();
        let with = vec![("day".to_owned(), e("now.date"))];
        assert_eq!(does, [&Does::Open("b".into(), with), &Does::Open("a".into(), vec![])]);
    }

    #[test]
    fn a_layout_is_a_list_of_known_components() {
        assert_eq!(
            layout("{BigValue: x}"),
            "screens.a.layout: has to be a list of components, like - BigValue: {text: block.text}"
        );
        let shape = "screens.a.layout[0]: has to be one component and its props, like BigValue: {text: block.text}";
        assert_eq!(layout("[x]"), shape);
        assert_eq!(layout("[{Label: \"\", Chip: \"\"}]"), shape);
        assert_eq!(
            layout("[{Title: \"\"}]"),
            format!(
                "screens.a.layout[0].Title: \"Title\" is not a component: {}",
                COMPONENTS.join(", ")
            )
        );
        assert_eq!(
            layout("[{Label: x}]"),
            "screens.a.layout[0].Label: has to be a mapping of prop to expression"
        );
        let d = def("screens:\n  a:\n    layout:\n      - Label:\n");
        let label =
            Component { kind: "Label".into(), each: None, when: None, props: vec![], on: vec![] };
        assert_eq!(d.screens[0].1.layout, [label]);
    }

    #[test]
    fn props_are_expressions_or_templates_and_events_name_actions() {
        let known = "Known: now, content, ui, store, params";
        assert_eq!(
            layout("[{Button: {on_tap: stay}}]"),
            "screens.a.layout[0].Button.on_tap: \"stay\" is not an action of this screen: go"
        );
        assert_eq!(
            layout("[{Button: {label: item, if: item}}]"),
            format!(
                "screens.a.layout[0].Button.label: column 1: \"item\" is not a name here. {known}\n\
                 screens.a.layout[0].Button.if: column 1: \"item\" is not a name here. {known}"
            )
        );
        assert_eq!(
            layout("[{Button: {each: item, label: \"{nope}\"}}]"),
            format!(
                "screens.a.layout[0].Button.each: column 1: \"item\" is not a name here. {known}\n\
                 screens.a.layout[0].Button.label: column 1: \"nope\" is not a name here. {known}, item"
            )
        );
        // `item` is only known inside the component that repeats.
        let after = layout("[{Label: {each: content}}, {Label: {text: item}}]");
        assert_eq!(
            after,
            format!(
                "screens.a.layout[1].Label.text: column 1: \"item\" is not a name here. {known}"
            )
        );
        let d = def(
            "screens: {a: {actions: {go: [back]}, layout: [{Button: {each: content, if: item, label: \"{item}!\", kid: true, on_tap: go}}]}}",
        );
        let label = Prop::Template(vec![Piece::Expr(e("item")), Piece::Text("!".into())]);
        let button = Component {
            kind: "Button".into(),
            each: Some(e("content")),
            when: Some(e("item")),
            props: vec![("label".into(), label), ("kid".into(), Prop::Expr(e("true")))],
            on: vec![("tap".into(), "go".into())],
        };
        assert_eq!(d.screens[0].1.layout, [button]);
    }

    #[test]
    fn props_evaluate_and_templates_fill_in_their_text() {
        let scope = yaml("n: 2\nname: Rita\nlist: [a, b]").unwrap();
        let scope = scope.as_map().unwrap();
        assert_eq!(Prop::Expr(e("n")).eval(scope), Value::Number(2.0));
        let pieces = vec![
            Piece::Text("Hi ".into()),
            Piece::Expr(e("name")),
            Piece::Expr(e("list")),
            Piece::Expr(e("x")),
        ];
        assert_eq!(Prop::Template(pieces).eval(scope), Value::String("Hi Ritaa,b".into()));
    }
}
