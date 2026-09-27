//! The engine as the app sees it, through UniFFI. This crate only converts: every rule lives in
//! `deskpress-engine`, which never depends on UniFFI.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use deskpress_engine::engine::{self, Engine, Nav};
use deskpress_engine::value::{self as data, Map, text};
use deskpress_engine::{pack, tree, validate};

uniffi::setup_scaffolding!();

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Finding {
    pub at: String,
    pub message: String,
}

/// A value of the pack or of the device. `Items` and `Fields` are a list and a mapping: Kotlin
/// already has a `List` and a `Map`.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum Value {
    Null,
    Bool { value: bool },
    Number { value: f64 },
    Text { value: String },
    Items { items: Vec<Value> },
    Fields { fields: Vec<Field> },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Field {
    pub key: String,
    pub value: Value,
}

impl From<data::Value> for Value {
    fn from(v: data::Value) -> Self {
        match v {
            data::Value::Null => Value::Null,
            data::Value::Bool(value) => Value::Bool { value },
            data::Value::Number(value) => Value::Number { value },
            data::Value::String(value) => Value::Text { value },
            data::Value::List(items) => {
                Value::Items { items: items.into_iter().map(Value::from).collect() }
            }
            data::Value::Map(m) => {
                let fields = m.0.into_iter().map(|(key, v)| Field { key, value: v.into() });
                Value::Fields { fields: fields.collect() }
            }
        }
    }
}

impl From<Value> for data::Value {
    fn from(v: Value) -> Self {
        match v {
            Value::Null => data::Value::Null,
            Value::Bool { value } => data::Value::Bool(value),
            Value::Number { value } => data::Value::Number(value),
            Value::Text { value } => data::Value::String(value),
            Value::Items { items } => {
                data::Value::List(items.into_iter().map(Into::into).collect())
            }
            Value::Fields { fields } => {
                data::Value::Map(Map(fields.into_iter().map(|f| (f.key, f.value.into())).collect()))
            }
        }
    }
}

fn values(m: Map) -> HashMap<String, Value> {
    m.0.into_iter().map(|(k, v)| (k, v.into())).collect()
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Node {
    pub kind: String,
    pub props: HashMap<String, Value>,
    /// Event to the action to dispatch, like `tap` to `confirm`.
    pub on: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Tree {
    pub version: u32,
    pub screen: String,
    pub nodes: Vec<Node>,
}

impl From<tree::Tree> for Tree {
    fn from(t: tree::Tree) -> Self {
        let node = |n: tree::Node| Node {
            kind: n.kind,
            props: values(Map(n.props)),
            on: n.on.into_iter().collect(),
        };
        Tree {
            version: t.version,
            screen: t.screen,
            nodes: t.nodes.into_iter().map(node).collect(),
        }
    }
}

/// When to call `screen` again: at `until`, or when one of the `regions` is crossed.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Watch {
    pub until: String,
    pub regions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct View {
    pub tree: Tree,
    pub watch: Watch,
}

impl From<engine::View> for View {
    fn from(v: engine::View) -> Self {
        let watch = Watch { until: v.watch.until, regions: v.watch.regions };
        View { tree: v.tree.into(), watch }
    }
}

/// What an action did: the view to draw, the facts to store and the commands to run.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Outcome {
    pub view: View,
    pub store: HashMap<String, Value>,
    pub commands: Vec<String>,
}

/// Everything the answer depends on besides the pack. `now` is local to the pack's timezone,
/// `YYYY-MM-DDTHH:MM`; `inside` holds the ids of the places whose region the device is in.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct World {
    pub now: String,
    pub holder: String,
    pub inside: Vec<String>,
    pub store: HashMap<String, Value>,
}

impl From<World> for engine::World {
    fn from(w: World) -> Self {
        // Sorted, so the same facts make the same scope whatever order the map hands them in.
        let mut store: Vec<(String, data::Value)> =
            w.store.into_iter().map(|(k, v)| (k, v.into())).collect();
        store.sort_by(|a, b| a.0.cmp(&b.0));
        engine::World { now: w.now, holder: w.holder, inside: w.inside, store: Map(store) }
    }
}

/// Why a pack did not load: a file could not be read or parsed, or the pack has errors.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum LoadError {
    Unreadable { detail: String },
    Invalid { errors: Vec<Finding> },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Unreadable { detail } => f.write_str(detail),
            LoadError::Invalid { errors } => write!(f, "{} errors", errors.len()),
        }
    }
}

impl std::error::Error for LoadError {}

/// A call the engine cannot answer: a time that is not one, or an action the screen lacks.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum CallError {
    Refused { detail: String },
}

impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let CallError::Refused { detail } = self;
        f.write_str(detail)
    }
}

impl std::error::Error for CallError {}

fn refused(detail: String) -> CallError {
    CallError::Refused { detail }
}

/// A pack that passed validation, and where the person is in it. It may still have warnings.
#[derive(uniffi::Object)]
pub struct LoadedPack {
    engine: Engine,
    nav: Mutex<Nav>,
    warnings: Vec<Finding>,
}

#[uniffi::export]
impl LoadedPack {
    pub fn warnings(&self) -> Vec<Finding> {
        self.warnings.clone()
    }

    /// The IANA timezone the host turns its clock into before it passes `now`.
    pub fn timezone(&self) -> String {
        text(self.engine.pack().manifest.get("pack").and_then(|p| p.get("timezone")))
    }

    pub fn screen(&self, world: World) -> Result<View, CallError> {
        let mut nav = self.nav.lock().unwrap_or_else(PoisonError::into_inner);
        let view = self.engine.screen(&world.into(), &mut nav).map_err(refused)?;
        Ok(view.into())
    }

    pub fn dispatch(&self, world: World, action: String, arg: Value) -> Result<Outcome, CallError> {
        let mut nav = self.nav.lock().unwrap_or_else(PoisonError::into_inner);
        let out = self.engine.dispatch(&world.into(), &mut nav, &action, arg.into());
        let out = out.map_err(refused)?;
        Ok(Outcome { view: out.view.into(), store: values(out.store), commands: out.commands })
    }
}

fn findings(list: Vec<validate::Finding>) -> Vec<Finding> {
    list.into_iter().map(|f| Finding { at: f.at, message: f.message }).collect()
}

/// Loads the pack whose manifest is `manifest`. `files` holds every file of the pack by its path
/// inside the pack folder, which is how an app that reads its own assets has them.
#[uniffi::export]
pub fn load(
    manifest: String,
    files: HashMap<String, String>,
) -> Result<Arc<LoadedPack>, LoadError> {
    let read = |name: &str| files.get(name).cloned().ok_or_else(|| "no such file".to_owned());
    let pack = pack::load(&manifest, read).map_err(|detail| LoadError::Unreadable { detail })?;
    let (engine, warnings) =
        Engine::load(pack).map_err(|errors| LoadError::Invalid { errors: findings(errors) })?;
    let nav = Mutex::new(Nav::default());
    Ok(Arc::new(LoadedPack { engine, nav, warnings: findings(warnings) }))
}

#[uniffi::export]
pub fn tree_version() -> u32 {
    deskpress_engine::TREE_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str =
        "pack: {id: t, name: Test, language: en, timezone: UTC, content: content.yaml}\n";

    const MACHINE: &str = "screens:
  a:
    state: {n: null}
    actions:
      go: [{set: n, to: $arg}, {store: seen, value: $arg}, map.open]
    layout:
      - Label: {text: n, on_tap: go}
rules:
  - {screen: a}
";

    fn files(definition: &str, content: &str) -> HashMap<String, String> {
        HashMap::from([
            ("pack.yaml".to_owned(), format!("{MANIFEST}{definition}")),
            ("content.yaml".to_owned(), content.to_owned()),
        ])
    }

    fn load_err(files: HashMap<String, String>) -> LoadError {
        load("pack.yaml".into(), files).err().unwrap()
    }

    fn world(now: &str) -> World {
        World { now: now.into(), holder: String::new(), inside: vec![], store: HashMap::new() }
    }

    fn text(value: &str) -> Value {
        Value::Text { value: value.into() }
    }

    #[test]
    fn a_missing_file_is_unreadable() {
        let e = load_err(HashMap::new());
        assert_eq!(e, LoadError::Unreadable { detail: "pack.yaml: no such file".into() });
        assert_eq!(e.to_string(), "pack.yaml: no such file");
    }

    #[test]
    fn a_pack_with_errors_is_invalid_and_lists_them() {
        let e = load_err(files("", "days: x\n"));
        let at = "days".to_owned();
        let message = "has to be a list, one entry per day".to_owned();
        assert_eq!(e, LoadError::Invalid { errors: vec![Finding { at, message }] });
        assert_eq!(e.to_string(), "1 errors");
    }

    #[test]
    fn a_valid_pack_loads_with_its_warnings_and_draws_its_outline() {
        let day = "days:\n  - {date: 2026-04-11, title: Arrive}\n";
        let pack = load("pack.yaml".into(), files("", day)).unwrap();
        assert_eq!(pack.warnings().len(), 1);
        assert_eq!(pack.timezone(), "UTC");
        let view = pack.screen(world("2026-04-11T10:00")).unwrap();
        assert_eq!((view.tree.version, view.tree.screen.as_str()), (tree_version(), "outline"));
        let node = |kind: &str, props: &[(&str, &str)]| Node {
            kind: kind.into(),
            props: props.iter().map(|(k, v)| ((*k).to_owned(), text(v))).collect(),
            on: HashMap::new(),
        };
        let expected = vec![
            node("Title", &[("text", "Test")]),
            node("Row", &[("text", "Arrive"), ("caption", "2026-04-11")]),
        ];
        assert_eq!(view.tree.nodes, expected);
        assert_eq!(view.watch, Watch { until: "2026-04-12T00:00".into(), regions: vec![] });
    }

    #[test]
    fn a_call_the_engine_cannot_answer_is_refused() {
        let pack = load(
            "pack.yaml".into(),
            files(
                MACHINE,
                "days: []
",
            ),
        )
        .unwrap();
        let e = pack.screen(world("today")).unwrap_err();
        let message = "\"today\" is not a time: YYYY-MM-DDTHH:MM, in the pack's timezone";
        assert_eq!(e.to_string(), message);
        let e = pack.dispatch(world("2026-04-11T10:00"), "stop".into(), Value::Null).unwrap_err();
        let detail = "\"stop\" is not an action of the screen \"a\"".to_owned();
        assert_eq!(e, CallError::Refused { detail });
    }

    #[test]
    fn an_action_changes_the_screen_it_keeps_and_hands_back_facts_and_commands() {
        let pack = load(
            "pack.yaml".into(),
            files(
                MACHINE,
                "days: []
",
            ),
        )
        .unwrap();
        let fields = vec![Field { key: "k".into(), value: text("v") }];
        let items = vec![
            Value::Null,
            Value::Bool { value: true },
            Value::Number { value: 1.5 },
            Value::Fields { fields },
        ];
        let every = Value::Items { items };
        let mut w = world("2026-04-11T10:00");
        w.store = HashMap::from([("b".to_owned(), text("2")), ("a".to_owned(), text("1"))]);
        let out = pack.dispatch(w.clone(), "go".into(), every.clone()).unwrap();
        assert_eq!(out.store, HashMap::from([("seen".to_owned(), every.clone())]));
        assert_eq!(out.commands, ["map.open"]);
        let node = &out.view.tree.nodes[0];
        assert_eq!(node.props["text"], every);
        assert_eq!(node.on, HashMap::from([("tap".to_owned(), "go".to_owned())]));
        // The pack keeps where the person is between calls.
        assert_eq!(pack.screen(w).unwrap().tree.nodes[0].props["text"], every);
    }
}
