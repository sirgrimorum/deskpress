//! The definition: the part of pack.yaml that says how the app behaves. Read once at load into
//! parsed expressions, so a call to the engine never parses.

mod screens;
mod sync;

pub use screens::{COMPONENTS, Component, Does, Effect, Piece, Prop, Screen, fill};
pub(crate) use sync::key;
pub use sync::{FLOOR, Fetch};

use crate::expr::{Expr, Op, column_of, parse};
use crate::validate::Report;
use crate::validate::patterns::{is_id, is_language, is_real_date, is_stamp, is_time};
use crate::value::{Map, Value, quote, text};

/// The modules in the order they run, the content root each reads unless told otherwise, and the
/// names it exposes. A module reads the names the ones before it exposed.
/// `sheets` comes last: it hands out what the modules before it did not read.
pub const MODULES: [(&str, &str, &[&str]); 8] = [
    ("timeline", "days", &["day", "block", "next", "days", "tomorrow"]),
    ("choices", "days", &["decision"]),
    ("people", "people", &["holder", "people"]),
    ("places", "places", &["place", "here", "away", "chart"]),
    ("alerts", "alerts", &["alerts"]),
    ("documents", "documents", &["documents"]),
    ("climate", "climate", &["weather"]),
    ("sheets", "sheets", &["sheets"]),
];

/// The names every expression can read.
pub const BASE: [&str; 5] = ["now", "content", "ui", "store", "can"];

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub name: &'static str,
    /// The content root it reads, through the keymap; None for a module fed by its sync alone.
    pub from: Option<String>,
    pub sync: Option<Fetch>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    /// None on the last rule, which always holds.
    pub when: Option<Expr>,
    pub screen: String,
}

/// An instant some expression compares the clock with: `HH:MM` (today) or `YYYY-MM-DDTHH:MM`.
/// `later` means the answer changes the minute after it, as with `now.time > '11:15'`. `day`
/// means the time is on today's day's own clock, `day.time`, rather than the pack's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clock {
    pub at: String,
    pub later: bool,
    pub day: bool,
}

/// A question the pack can answer, and the answer as an expression (decision 0027).
#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    pub id: String,
    /// The words to offer, by language tag. An empty tag is the text a pack with one language
    /// wrote, used when nothing matches.
    pub ask: Vec<(String, String)>,
    pub answer: Prop,
    /// Offered only while this holds. None means always.
    pub when: Option<Expr>,
    /// Offer it to the phone's launcher and assistant too.
    pub shortcut: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Definition {
    pub modules: Vec<Module>,
    pub derive: Vec<(String, Expr)>,
    pub questions: Vec<Question>,
    pub rules: Vec<Rule>,
    pub screens: Vec<(String, Screen)>,
    pub clocks: Vec<Clock>,
}

impl Question {
    /// The words for the first of `langs` this question speaks, matching the whole tag before its
    /// primary subtag, else the text a pack with one language wrote.
    pub fn words(&self, langs: &[&str]) -> &str {
        fn primary(t: &str) -> &str {
            t.split('-').next().unwrap_or(t)
        }
        for want in langs.iter().filter(|l| !l.is_empty()) {
            let exact = self.ask.iter().find(|(l, _)| l == want);
            let hit = exact.or_else(|| self.ask.iter().find(|(l, _)| primary(l) == primary(want)));
            if let Some((_, words)) = hit {
                return words;
            }
        }
        let plain = self.ask.iter().find(|(l, _)| l.is_empty());
        plain.unwrap_or(&self.ask[0]).1.as_str()
    }
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
        d.questions(m.get("questions"));
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
            let (mut from, mut sync, mut own) = (root.to_owned(), None, false);
            match config {
                Value::Null => {}
                Value::Map(settings) => {
                    for (key, v) in settings.iter() {
                        match (key, v) {
                            ("from", Value::String(s)) if !s.is_empty() => {
                                from = s.clone();
                                own = true;
                            }
                            ("from", _) => self.r.error(
                                format!("{at}.from"),
                                format!("names the content root this module reads, like {root}"),
                            ),
                            ("sync", _) if name != "climate" => {
                                self.r.error(format!("{at}.sync"), "only climate syncs so far")
                            }
                            ("sync", v) => sync = self.sync(&format!("{at}.sync"), v),
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
            // With a sync and no from, the pack's data is not read at all.
            let reads = own || config.get("sync").is_none();
            self.def.modules.push(Module { name, from: reads.then_some(from), sync });
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

    /// The questions the pack can answer (decision 0027). `questions` becomes a known name after
    /// they are read, whether or not any were, so a rule or a screen reads it and a question
    /// cannot read itself.
    fn questions(&mut self, section: Option<&Value>) {
        match section.map(Value::as_map) {
            None => {}
            Some(Some(questions)) => {
                for (id, q) in questions.iter() {
                    self.question(id, q);
                }
            }
            Some(None) => {
                self.r.error("questions", "has to be a mapping of question id to {ask, answer}");
            }
        }
        self.known.push("questions".to_owned());
    }

    fn question(&mut self, id: &str, q: &Value) {
        let at = format!("questions.{id}");
        if !is_id(id) {
            let message =
                format!("{} is not a name: lowercase letters, digits and underscores", quote(id));
            self.r.error(at, message);
            return;
        }
        let Some(q) = q.as_map() else {
            self.r.error(&at, "has to be a mapping: {ask, answer, when, shortcut}");
            return;
        };
        self.only(&at, q, "a question", &["ask", "answer", "when", "shortcut"]);
        let ask = self.ask(&at, q.get("ask"));
        let answer = match q.get("answer") {
            Some(a) => self.prop(&format!("{at}.answer"), a),
            None => {
                self.r.error(&at, "a question needs an answer: the expression that answers it");
                None
            }
        };
        let when = q.get("when").map(|w| self.expr(&format!("{at}.when"), w));
        if when.as_ref().is_some_and(Option::is_none) {
            return;
        }
        let (Some(ask), Some(answer)) = (ask, answer) else {
            return;
        };
        let shortcut = q.get("shortcut") == Some(&Value::Bool(true));
        let id = id.to_owned();
        self.def.questions.push(Question { id, ask, answer, when: when.flatten(), shortcut });
    }

    /// The words a question is offered in: one text, or one per language tag.
    fn ask(&mut self, at: &str, ask: Option<&Value>) -> Option<Vec<(String, String)>> {
        let at = format!("{at}.ask");
        match ask {
            Some(Value::String(s)) if !s.is_empty() => Some(vec![(String::new(), s.clone())]),
            Some(Value::Map(m)) if !m.0.is_empty() => {
                let mut out = Vec::new();
                for (lang, words) in m.iter() {
                    if is_language(lang) {
                        out.push((lang.to_owned(), text(Some(words))));
                        continue;
                    }
                    let message =
                        format!("{} is not a language tag, like es or pt-BR", quote(lang));
                    self.r.error(format!("{at}.{lang}"), message);
                }
                (!out.is_empty()).then_some(out)
            }
            _ => {
                self.r
                    .error(at, "a question needs an ask: the words to offer, or one per language");
                None
            }
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
            self.only(&at, rule, "a rule", &["when", "screen"]);
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

    /// Reports each key of `m` that is not one of `keys`.
    fn only(&mut self, at: &str, m: &Map, what: &str, keys: &[&str]) {
        for key in m.keys().filter(|k| !keys.contains(k)) {
            let message = format!("{} is not part of {what}: {}", quote(key), keys.join(", "));
            self.r.error(format!("{at}.{key}"), message);
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
                ["now" | "day", "time"] => is_time(at),
                ["now", "stamp"] => is_stamp(at) && is_real_date(&at[..10]),
                _ => false,
            };
            if !clock {
                return;
            }
            let day = path[0] == "day";
            let mut push = |later| clocks.push(Clock { at: at.clone(), later, day });
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
        assert!(r.warnings.is_empty(), "a definition only has errors");
        r.errors
            .iter()
            .map(|f| format!("error {}: {}", f.at, f.message))
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
            "error modules.weather: \"weather\" is not a module: timeline, choices, people, places, alerts, documents, climate, sheets"
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
            said("modules: {alerts: {sync: {trigger: button}}}"),
            "error modules.alerts.sync: only climate syncs so far"
        );
    }

    #[test]
    fn modules_run_in_their_own_order_from_their_own_root_unless_told() {
        let d = def("modules:
  climate: {from: tiempo}
  timeline:
  people: {}
");
        let got: Vec<(&str, Option<&str>)> =
            d.modules.iter().map(|m| (m.name, m.from.as_deref())).collect();
        let expected =
            [("timeline", Some("days")), ("people", Some("people")), ("climate", Some("tiempo"))];
        assert_eq!(got, expected);
    }

    #[test]
    fn a_module_with_a_sync_and_no_from_reads_no_pack_data() {
        let sync =
            "{trigger: auto, every: 6h, request: {url: 'https://a.org/f'}, read: {high: d.max}}";
        let only = def(&format!("modules: {{climate: {{sync: {sync}}}}}"));
        let both = def(&format!("modules: {{climate: {{from: climate, sync: {sync}}}}}"));
        assert_eq!(
            (only.modules[0].from.as_deref(), both.modules[0].from.as_deref()),
            (None, Some("climate"))
        );
        let s = only.modules[0].sync.as_ref().unwrap();
        assert_eq!((s.every, s.host(), s.url.as_str()), (Some(360), "a.org", "https://a.org/f"));
        assert_eq!(s.read, [("high".to_owned(), vec!["d".to_owned(), "max".to_owned()])]);
        let broken = define(&yaml("modules: {climate: {sync: {trigger: auto}}}").unwrap()).0;
        assert_eq!(
            (broken.modules[0].from.as_deref(), broken.modules[0].sync.is_none()),
            (None, true)
        );
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
            "error derive.a: column 1: \"holder\" is not a name here. Known: now, content, ui, store, can\nerror derive.a: column 17: \"day\" is not a name here. Known: now, content, ui, store, can"
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
    fn a_question_needs_an_ask_an_answer_and_a_real_language() {
        assert_eq!(
            said("questions: [a]"),
            "error questions: has to be a mapping of question id to {ask, answer}"
        );
        assert_eq!(
            said("questions: {Loo: {}}"),
            "error questions.Loo: \"Loo\" is not a name: lowercase letters, digits and underscores"
        );
        assert_eq!(
            said("questions: {loo: [a]}"),
            "error questions.loo: has to be a mapping: {ask, answer, when, shortcut}"
        );
        let no_ask = concat!(
            "error questions.loo.ask: ",
            "a question needs an ask: the words to offer, or one per language"
        );
        assert_eq!(said("questions: {loo: {answer: 'true'}}").lines().last(), Some(no_ask));
        assert_eq!(said("questions: {loo: {ask: '', answer: 'true'}}"), no_ask);
        assert_eq!(said("questions: {loo: {ask: {}, answer: 'true'}}"), no_ask);
        // A tag nobody speaks is reported; a question left with no words at all is not asked.
        assert_eq!(
            said("questions: {loo: {ask: {ES: Donde}, answer: 'true'}}"),
            "error questions.loo.ask.ES: \"ES\" is not a language tag, like es or pt-BR"
        );
        assert_eq!(
            said("questions: {loo: {ask: Where, why: no}}"),
            concat!(
                "error questions.loo.why: \"why\" is not part of a question: ",
                "ask, answer, when, shortcut\n",
                "error questions.loo: a question needs an answer: the expression that answers it"
            )
        );
        assert_eq!(
            said("questions: {loo: {ask: Where, answer: 'true', when: nope}}"),
            concat!(
                "error questions.loo.when: column 1: \"nope\" is not a name here. ",
                "Known: now, content, ui, store, can"
            )
        );
    }

    #[test]
    fn a_question_is_answered_by_an_expression_and_offered_while_its_when_holds() {
        let d = def(&format!(
            "{SCREENS}modules:\n  people:\nquestions:\n  loo: {{ask: {{es: Donde, en: Where}}, answer: holder.name, shortcut: true}}\n  bed: {{ask: Where do we sleep, answer: 'the {{holder.name}} bed', when: holder}}\nrules: [{{when: questions, screen: moment}}, {{screen: blocker}}]"
        ));
        assert_eq!(d.questions.len(), 2);
        let (loo, bed) = (&d.questions[0], &d.questions[1]);
        assert_eq!(loo.ask, [("es".to_owned(), "Donde".to_owned()), ("en".into(), "Where".into())]);
        assert_eq!((loo.shortcut, loo.when.is_some()), (true, false));
        assert_eq!(bed.ask, [(String::new(), "Where do we sleep".to_owned())]);
        assert_eq!((bed.shortcut, bed.when.is_some()), (false, true));
        // An answer with {} in it fills in; a plain one is read as an expression.
        let filled = |p: &Prop| matches!(p, Prop::Template(_));
        assert_eq!((filled(&loo.answer), filled(&bed.answer)), (false, true));
    }

    #[test]
    fn the_words_are_the_first_language_the_question_speaks() {
        let q = |ask: &[(&str, &str)]| Question {
            id: "q".into(),
            ask: ask.iter().map(|(l, w)| ((*l).to_owned(), (*w).to_owned())).collect(),
            answer: Prop::Expr(Expr::Literal(Value::Null)),
            when: None,
            shortcut: false,
        };
        let many = q(&[("es", "Donde"), ("pt", "Onde"), ("", "Where")]);
        // A person with no language of their own falls through to the pack's.
        assert_eq!(many.words(&["", "es"]), "Donde");
        // pt-BR is close enough to pt, and closer than the words with no language.
        assert_eq!(many.words(&["pt-BR", "es"]), "Onde");
        assert_eq!(many.words(&["de", "fr"]), "Where");
        // Nothing matches and nothing is plain: the first words the pack wrote.
        assert_eq!(q(&[("es", "Donde")]).words(&["de"]), "Donde");
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
        let d = def("modules: {timeline: }\nderive: {x: \"day.time >= '19:00'\"}");
        assert_eq!((d.clocks[0].at.as_str(), d.clocks[0].day), ("19:00", true));
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
