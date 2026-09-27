//! The definition: the part of pack.yaml that says how the app behaves. Read once at load into
//! parsed expressions, so a call to the engine never parses.

mod screens;

pub use screens::{COMPONENTS, Component, Does, Effect, Piece, Prop, Screen, fill};

use crate::expr::{Expr, Op, column_of, parse};
use crate::validate::Report;
use crate::validate::patterns::{is_id, is_real_date, is_stamp, is_time};
use crate::value::{Value, quote, text};

/// The modules in the order they run, the content root each reads unless told otherwise, and the
/// names it exposes. A module reads the names the ones before it exposed.
pub const MODULES: [(&str, &str, &[&str]); 7] = [
    ("timeline", "days", &["day", "block", "next"]),
    ("choices", "days", &["decision"]),
    ("people", "people", &["holder", "people"]),
    ("places", "places", &["place", "here"]),
    ("alerts", "alerts", &["alerts"]),
    ("documents", "documents", &["documents"]),
    ("climate", "climate", &["weather"]),
];

/// The names every expression can read.
pub const BASE: [&str; 4] = ["now", "content", "ui", "store"];

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub name: &'static str,
    /// The content root it reads, through the keymap.
    pub from: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    /// None on the last rule, which always holds.
    pub when: Option<Expr>,
    pub screen: String,
}

/// An instant some expression compares the clock with: `HH:MM` (today) or `YYYY-MM-DDTHH:MM`.
/// `later` means the answer changes the minute after it, as with `now.time > '11:15'`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clock {
    pub at: String,
    pub later: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Definition {
    pub modules: Vec<Module>,
    pub derive: Vec<(String, Expr)>,
    pub rules: Vec<Rule>,
    pub screens: Vec<(String, Screen)>,
    pub clocks: Vec<Clock>,
}

impl Definition {
    pub fn screen(&self, name: &str) -> Option<&Screen> {
        self.screens.iter().find(|(n, _)| n == name).map(|(_, s)| s)
    }
}

/// The definition of a manifest, and what is wrong with it. A manifest that is not a mapping
/// defines nothing and is the validator's to report.
pub fn define(manifest: &Value) -> (Definition, Report) {
    let mut d = Definer {
        def: Definition::default(),
        r: Report::default(),
        known: BASE.map(String::from).to_vec(),
    };
    if let Some(m) = manifest.as_map() {
        d.modules(m.get("modules"));
        d.derive(m.get("derive"));
        d.rules(m.get("rules"), m.get("screens"));
        d.screens(m.get("screens"));
    }
    (d.def, d.r)
}

struct Definer {
    def: Definition,
    r: Report,
    /// The names an expression may read at this point of the file.
    known: Vec<String>,
}

impl Definer {
    fn modules(&mut self, modules: Option<&Value>) {
        let Some(modules) = modules else {
            return;
        };
        let Some(modules) = modules.as_map() else {
            self.r.error("modules", "has to be a mapping of module name to its settings");
            return;
        };
        let names: Vec<&str> = MODULES.iter().map(|(n, ..)| *n).collect();
        for name in modules.keys().filter(|n| !names.contains(n)) {
            let message = format!("{} is not a module: {}", quote(name), names.join(", "));
            self.r.error(format!("modules.{name}"), message);
        }
        for (name, root, exposed) in MODULES {
            let Some(config) = modules.get(name) else {
                continue;
            };
            let at = format!("modules.{name}");
            let mut from = root.to_owned();
            match config {
                Value::Null => {}
                Value::Map(settings) => {
                    for (key, v) in settings.iter() {
                        match (key, v) {
                            ("from", Value::String(s)) if !s.is_empty() => from = s.clone(),
                            ("from", _) => self.r.error(
                                format!("{at}.from"),
                                format!("names the content root this module reads, like {root}"),
                            ),
                            ("sync", _) => self.r.warn(
                                format!("{at}.sync"),
                                "syncing lands in phase 7. Until then this module reads the pack",
                            ),
                            _ => self.r.error(
                                format!("{at}.{key}"),
                                format!("{} is not a module setting: from, sync", quote(key)),
                            ),
                        }
                    }
                }
                _ => self.r.error(&at, "has to be a mapping of settings, or nothing"),
            }
            self.known.extend(exposed.iter().map(|n| (*n).to_owned()));
            self.def.modules.push(Module { name, from });
        }
    }

    fn derive(&mut self, derive: Option<&Value>) {
        let Some(derive) = derive else {
            return;
        };
        let Some(derive) = derive.as_map() else {
            self.r.error("derive", "has to be a mapping of name to expression");
            return;
        };
        for (name, src) in derive.iter() {
            let at = format!("derive.{name}");
            if !is_id(name) {
                let message = format!(
                    "{} is not a name: lowercase letters, digits and underscores",
                    quote(name)
                );
                self.r.error(at, message);
                continue;
            }
            if self.known.iter().any(|k| k == name) {
                let message =
                    format!("{} is already a name here, and this would hide it", quote(name));
                self.r.error(at, message);
                continue;
            }
            if let Some(e) = self.expr(&at, src) {
                self.def.derive.push((name.to_owned(), e));
            }
            // Known even when broken, so its uses are not a second error.
            self.known.push(name.to_owned());
        }
    }

    fn rules(&mut self, rules: Option<&Value>, screens: Option<&Value>) {
        let Some(rules) = rules else {
            return;
        };
        let Some(list) = rules.as_list().filter(|l| !l.is_empty()) else {
            self.r.error("rules", "has to be a list of {when, screen}, the last one with no when");
            return;
        };
        let screens = screens.and_then(Value::as_map);
        let last = list.len() - 1;
        for (i, rule) in list.iter().enumerate() {
            let at = format!("rules[{i}]");
            let Some(rule) = rule.as_map() else {
                self.r.error(&at, "has to be a mapping: {when: expression, screen: name}");
                continue;
            };
            for key in rule.keys().filter(|k| !["when", "screen"].contains(k)) {
                let message = format!("{} is not part of a rule: when, screen", quote(key));
                self.r.error(format!("{at}.{key}"), message);
            }
            let screen = text(rule.get("screen"));
            if screen.is_empty() {
                self.r.error(&at, "a rule needs a screen");
            } else if screens.and_then(|s| s.get(&screen)).is_none() {
                let message = format!("{} is not one of the screens", quote(&screen));
                self.r.error(format!("{at}.screen"), message);
            }
            let when = match (rule.get("when"), i == last) {
                (None, true) => None,
                (None, false) => {
                    self.r.error(
                        &at,
                        "only the last rule goes without a when: no rule after this one could ever be picked",
                    );
                    None
                }
                (Some(_), true) => {
                    self.r.error(
                        format!("{at}.when"),
                        "the last rule has no when, so there is always a screen to show",
                    );
                    None
                }
                (Some(w), false) => self.expr(&format!("{at}.when"), w),
            };
            self.def.rules.push(Rule { when, screen });
        }
    }

    /// An expression at `at`, parsed and reading only known names.
    fn expr(&mut self, at: &str, src: &Value) -> Option<Expr> {
        match src {
            Value::String(s) => self.source(at, s),
            Value::Bool(_) | Value::Number(_) => self.source(at, &text(Some(src))),
            _ => {
                self.r.error(
                    at,
                    "has to be an expression, like block.locked and not decision.answered",
                );
                None
            }
        }
    }

    fn source(&mut self, at: &str, src: &str) -> Option<Expr> {
        let e = parse(src).map_err(|e| self.r.error(at, e.to_string())).ok()?;
        let mut unknown: Vec<&str> = Vec::new();
        for root in e.roots() {
            if !unknown.contains(&root) && !self.known.iter().any(|k| k == root) {
                unknown.push(root);
            }
        }
        for root in &unknown {
            let message = format!(
                "column {}: {} is not a name here. Known: {}",
                column_of(src, root),
                quote(root),
                self.known.join(", ")
            );
            self.r.error(at, message);
        }
        self.clocks(&e);
        unknown.is_empty().then_some(e)
    }

    /// The instants at which a comparison of the clock with a literal changes its answer.
    fn clocks(&mut self, e: &Expr) {
        let clocks = &mut self.def.clocks;
        e.visit(&mut |e| {
            let Expr::Compare(op, a, b) = e else {
                return;
            };
            let (op, path, at) = match (&**a, &**b) {
                (Expr::Path(p), Expr::Literal(Value::String(s))) => (*op, p, s),
                (Expr::Literal(Value::String(s)), Expr::Path(p)) => (flip(*op), p, s),
                _ => return,
            };
            let clock = match path.iter().map(String::as_str).collect::<Vec<_>>()[..] {
                ["now", "time"] => is_time(at),
                ["now", "stamp"] => is_stamp(at) && is_real_date(&at[..10]),
                _ => false,
            };
            if !clock {
                return;
            }
            let mut push = |later| clocks.push(Clock { at: at.clone(), later });
            match op {
                Op::Eq | Op::Ne => {
                    push(false);
                    push(true);
                }
                Op::Lt | Op::Ge => push(false),
                Op::Le | Op::Gt => push(true),
                Op::In => {}
            }
        });
    }
}

/// The operator that says the same with its sides swapped: `'10:00' < now.time` is `now.time > '10:00'`.
fn flip(op: Op) -> Op {
    match op {
        Op::Lt => Op::Gt,
        Op::Gt => Op::Lt,
        Op::Le => Op::Ge,
        Op::Ge => Op::Le,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml::parse as yaml;

    fn said(manifest: &str) -> String {
        let (_, r) = define(&yaml(manifest).unwrap());
        let all =
            r.errors.iter().map(|f| ("error", f)).chain(r.warnings.iter().map(|f| ("warning", f)));
        all.map(|(kind, f)| format!("{kind} {}: {}", f.at, f.message))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn def(manifest: &str) -> Definition {
        let (d, r) = define(&yaml(manifest).unwrap());
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        d
    }

    const SCREENS: &str = "screens: {moment: {}, blocker: {}}\n";

    #[test]
    fn a_manifest_that_is_not_a_mapping_defines_nothing() {
        let (d, r) = define(&Value::Null);
        assert_eq!((d, r), (Definition::default(), Report::default()));
    }

    #[test]
    fn modules_are_known_ones_with_known_settings() {
        assert_eq!(
            said("modules: [timeline]"),
            "error modules: has to be a mapping of module name to its settings"
        );
        assert_eq!(
            said("modules: {weather: {}}"),
            "error modules.weather: \"weather\" is not a module: timeline, choices, people, places, alerts, documents, climate"
        );
        assert_eq!(
            said("modules: {timeline: 3}"),
            "error modules.timeline: has to be a mapping of settings, or nothing"
        );
        assert_eq!(
            said("modules: {places: {geofence: at}}"),
            "error modules.places.geofence: \"geofence\" is not a module setting: from, sync"
        );
        assert_eq!(
            said("modules: {places: {from: ''}}"),
            "error modules.places.from: names the content root this module reads, like places"
        );
        assert_eq!(
            said("modules: {places: {from: 3}}"),
            "error modules.places.from: names the content root this module reads, like places"
        );
        assert_eq!(
            said("modules: {climate: {sync: {trigger: button}}}"),
            "warning modules.climate.sync: syncing lands in phase 7. Until then this module reads the pack"
        );
    }

    #[test]
    fn modules_run_in_their_own_order_from_their_own_root_unless_told() {
        let d = def("modules:
  climate: {from: tiempo}
  timeline:
  people: {}
");
        let got: Vec<(&str, &str)> = d.modules.iter().map(|m| (m.name, m.from.as_str())).collect();
        assert_eq!(got, [("timeline", "days"), ("people", "people"), ("climate", "tiempo")]);
    }

    #[test]
    fn derive_names_are_new_ids_over_known_names() {
        assert_eq!(said("derive: [a]"), "error derive: has to be a mapping of name to expression");
        assert_eq!(
            said("derive: {Kid: 'true'}"),
            "error derive.Kid: \"Kid\" is not a name: lowercase letters, digits and underscores"
        );
        assert_eq!(
            said("derive: {now: 'true'}"),
            "error derive.now: \"now\" is already a name here, and this would hide it"
        );
        assert_eq!(
            said("modules:\n  people:\nderive: {holder: 'true'}"),
            "error derive.holder: \"holder\" is already a name here, and this would hide it"
        );
        assert_eq!(
            said("derive: {a: [1]}"),
            "error derive.a: has to be an expression, like block.locked and not decision.answered"
        );
        assert_eq!(
            said("derive: {a: ~}"),
            "error derive.a: has to be an expression, like block.locked and not decision.answered"
        );
        assert_eq!(
            said("derive: {a: 'x =='}"),
            "error derive.a: column 5: expected a value, found the end"
        );
        assert_eq!(
            said("derive: {a: 'holder.adult or day or holder'}"),
            "error derive.a: column 1: \"holder\" is not a name here. Known: now, content, ui, store\nerror derive.a: column 17: \"day\" is not a name here. Known: now, content, ui, store"
        );
        // A broken name is still known, so its uses are not a second error.
        assert_eq!(said("derive: {a: 'x', b: 'a'}").lines().count(), 1);
    }

    #[test]
    fn derive_reads_modules_and_what_came_before_it_in_order() {
        let d = def(
            "modules:\n  people:\nderive: {kid: not holder.adult, flag: true, n: 2, two: kid and flag}",
        );
        let names: Vec<&str> = d.derive.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["kid", "flag", "n", "two"]);
        assert_eq!(d.derive[2].1, Expr::Literal(Value::Number(2.0)));
    }

    #[test]
    fn rules_are_a_list_whose_last_one_alone_has_no_when() {
        let rules_error =
            "error rules: has to be a list of {when, screen}, the last one with no when";
        assert_eq!(said("rules: []"), rules_error);
        assert_eq!(said("rules: {screen: moment}"), rules_error);
        let s = said(&format!("{SCREENS}rules: [x, {{screen: moment, why: no}}]"));
        assert_eq!(
            s,
            "error rules[0]: has to be a mapping: {when: expression, screen: name}\nerror rules[1].why: \"why\" is not part of a rule: when, screen"
        );
        let s = said(&format!(
            "{SCREENS}rules: [{{screen: moment}}, {{when: 'true', screen: blocker}}]"
        ));
        assert_eq!(
            s,
            "error rules[0]: only the last rule goes without a when: no rule after this one could ever be picked\nerror rules[1].when: the last rule has no when, so there is always a screen to show"
        );
        assert_eq!(
            said(&format!("{SCREENS}rules: [{{when: nope, screen: moment}}, {{screen: moment}}]"))
                .lines()
                .count(),
            1
        );
    }

    #[test]
    fn a_rule_shows_one_of_the_screens() {
        assert_eq!(said("rules: [{}]"), "error rules[0]: a rule needs a screen");
        assert_eq!(
            said("rules: [{screen: moment}]"),
            "error rules[0].screen: \"moment\" is not one of the screens"
        );
        assert_eq!(
            said("screens: [moment]\nrules: [{screen: moment}]"),
            "error rules[0].screen: \"moment\" is not one of the screens\n\
             error screens: has to be a mapping of screen name to its state, actions and layout"
        );
    }

    #[test]
    fn rules_are_kept_in_order() {
        let d = def(&format!(
            "{SCREENS}modules:\n  timeline:\nrules: [{{when: block.locked, screen: blocker}}, {{screen: moment}}]"
        ));
        assert_eq!(d.rules.len(), 2);
        assert_eq!(d.rules[0].when, Some(Expr::Path(vec!["block".into(), "locked".into()])));
        assert_eq!(d.rules[1], Rule { when: None, screen: "moment".into() });
    }

    #[test]
    fn a_clock_compared_with_a_literal_is_an_instant_to_watch() {
        let clocks = |e: &str| {
            let d = def(&format!("derive: {{x: \"{e}\"}}"));
            d.clocks
                .iter()
                .map(|c| format!("{}{}", c.at, if c.later { "+" } else { "" }))
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(clocks("now.time == '10:00'"), "10:00 10:00+");
        assert_eq!(clocks("now.time != '10:00'"), "10:00 10:00+");
        assert_eq!(clocks("now.time < '10:00'"), "10:00");
        assert_eq!(clocks("now.time >= '10:00'"), "10:00");
        assert_eq!(clocks("now.time <= '10:00'"), "10:00+");
        assert_eq!(clocks("now.time > '10:00'"), "10:00+");
        // The literal on the left is the same test turned around.
        assert_eq!(clocks("'10:00' < now.time"), "10:00+");
        assert_eq!(clocks("'10:00' >= now.time"), "10:00+");
        assert_eq!(clocks("'10:00' == now.time"), "10:00 10:00+");
        assert_eq!(
            clocks("now.stamp >= '2026-04-11T17:30' and not (now.time < '09:00')"),
            "2026-04-11T17:30 09:00"
        );
    }

    #[test]
    fn only_a_real_clock_and_a_real_instant_count() {
        let clocks = |e: &str| def(&format!("derive: {{x: \"{e}\"}}")).clocks.len();
        for e in [
            "now.time in '10:00'",
            "now.time < '25:00'",
            "now.stamp < '10:00'",
            "now.stamp < '2026-02-30T10:00'",
            "now.time < 10",
            "now.date < '2026-04-11'",
            "content.time < '10:00'",
            "now < '10:00'",
            "now.time < now.stamp",
        ] {
            assert_eq!(clocks(e), 0, "{e}");
        }
    }

    #[test]
    fn flipping_leaves_the_symmetric_operators_alone() {
        assert_eq!(flip(Op::Le), Op::Ge);
        assert_eq!(flip(Op::Gt), Op::Lt);
        assert_eq!(flip(Op::In), Op::In);
    }
}
