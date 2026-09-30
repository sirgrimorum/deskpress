//! The engine as the app sees it, through UniFFI. This crate only converts: every rule lives in
//! `deskpress-engine`, which never depends on UniFFI.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use deskpress_engine::engine::{self, Engine, Nav};
use deskpress_engine::value::{self as data, Map, text};
use deskpress_engine::{edit, facts, pack, tree, validate, yaml};

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

/// Sorted, so the same values make the same answer whatever order the map hands them in.
fn sorted(m: HashMap<String, Value>) -> Map {
    let mut list: Vec<(String, data::Value)> = m.into_iter().map(|(k, v)| (k, v.into())).collect();
    list.sort_by(|a, b| a.0.cmp(&b.0));
    Map(list)
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Node {
    /// What tells this node from its siblings across a redraw, for the renderer to key a list by.
    pub key: String,
    pub kind: String,
    pub props: HashMap<String, Value>,
    /// Event to the action to dispatch, like `tap` to `confirm`.
    pub on: HashMap<String, String>,
    /// What a Group holds, drawn inside it.
    pub children: Vec<Node>,
}

impl From<tree::Node> for Node {
    fn from(n: tree::Node) -> Self {
        Node {
            key: n.key,
            kind: n.kind,
            props: values(Map(n.props)),
            on: n.on.into_iter().collect(),
            children: n.children.into_iter().map(Node::from).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Tree {
    pub version: u32,
    pub screen: String,
    pub nodes: Vec<Node>,
    /// The holder's theme id, as the pack names it; empty for the pack's default.
    pub theme: String,
    /// A child holds the phone.
    pub kid: bool,
}

impl From<tree::Tree> for Tree {
    fn from(t: tree::Tree) -> Self {
        Tree {
            version: t.version,
            screen: t.screen,
            nodes: t.nodes.into_iter().map(Node::from).collect(),
            theme: t.theme,
            kid: t.kid,
        }
    }
}

/// When to call `screen` again: at `until`, or when one of the `regions` is crossed.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Watch {
    pub until: String,
    pub regions: Vec<Region>,
}

/// A place's circle, `radius_m` metres around `lat`, `lon`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Region {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    pub radius_m: f64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct View {
    pub tree: Tree,
    pub watch: Watch,
    /// The questions to offer the launcher and the phone's assistant, by the id that opens one.
    pub shortcuts: Vec<Shortcut>,
}

/// A question the phone may offer outside the app. The words only: the answer stays in the app.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Shortcut {
    pub id: String,
    pub ask: String,
}

impl From<engine::View> for View {
    fn from(v: engine::View) -> Self {
        let regions = v.watch.regions.into_iter().map(|r| Region {
            id: r.id,
            lat: r.lat,
            lon: r.lon,
            radius_m: r.radius_m,
        });
        let watch = Watch { until: v.watch.until, regions: regions.collect() };
        let shortcuts = v.shortcuts.into_iter().map(|s| Shortcut { id: s.id, ask: s.ask });
        View { tree: v.tree.into(), watch, shortcuts: shortcuts.collect() }
    }
}

/// What an action did: the view to draw, the facts to store and the commands to run.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Outcome {
    pub view: View,
    pub store: HashMap<String, Value>,
    pub commands: Vec<Command>,
}

/// A module action for the host to run, like `device.unlock`, with the values it was given.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Command {
    pub name: String,
    pub args: HashMap<String, Value>,
}

impl From<engine::Command> for Command {
    fn from(c: engine::Command) -> Self {
        Command { name: c.name, args: values(c.args) }
    }
}

/// A calendar event. `start` and `reminder` are local to `zone` and `end` to `end_zone`, where
/// empty is the pack's timezone. `reminder` is empty for none.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Event {
    pub id: String,
    pub start: String,
    pub end: String,
    pub zone: String,
    pub end_zone: String,
    pub title: String,
    pub location: String,
    pub notes: String,
    pub reminder: String,
    pub fingerprint: String,
}

impl From<engine::Event> for Event {
    fn from(e: engine::Event) -> Self {
        let engine::Event {
            id,
            start,
            end,
            zone,
            end_zone,
            title,
            location,
            notes,
            reminder,
            fingerprint,
        } = e;
        Event { id, start, end, zone, end_zone, title, location, notes, reminder, fingerprint }
    }
}

/// What a calendar sync would do: the events to add and to change, and the ids to remove.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Plan {
    pub add: Vec<Event>,
    pub change: Vec<Event>,
    pub remove: Vec<String>,
}

/// A fetch for the host to make: `url` with its query, plus `&param=` the secret named `secret`
/// when there is one. `tag` goes back untouched with the reply.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Request {
    pub module: String,
    pub url: String,
    pub secret: String,
    pub param: String,
    pub tag: HashMap<String, Value>,
}

impl From<engine::Request> for Request {
    fn from(r: engine::Request) -> Self {
        let engine::Request { module, url, secret, param, tag } = r;
        Request { module, url, secret, param, tag: values(tag) }
    }
}

impl From<Request> for engine::Request {
    fn from(r: Request) -> Self {
        let Request { module, url, secret, param, tag } = r;
        engine::Request { module, url, secret, param, tag: sorted(tag) }
    }
}

/// Everything the answer depends on besides the pack. `now` is local to the pack's timezone,
/// `YYYY-MM-DDTHH:MM`; `inside` holds the ids of the places whose region the device is in, the
/// smallest first, and `located` whether the device knows where it is. `zones` holds the local
/// time now in each zone of `LoadedPack::zones`, in the same shape as `now`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct World {
    pub now: String,
    pub holder: String,
    pub inside: Vec<String>,
    pub located: bool,
    pub store: HashMap<String, Value>,
    pub zones: HashMap<String, String>,
    /// What this host can do beyond the shell itself, read as `can.assistant`.
    pub can: Vec<String>,
}

impl From<World> for engine::World {
    fn from(w: World) -> Self {
        let (now, holder, inside, located) = (w.now, w.holder, w.inside, w.located);
        let mut zones: Vec<_> =
            w.zones.into_iter().map(|(z, t)| (z, data::Value::String(t))).collect();
        zones.sort_by(|a, b| a.0.cmp(&b.0));
        let zones = Map(zones);
        let store = sorted(w.store);
        engine::World { now, holder, inside, located, store, zones, can: w.can }
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

    /// The pack's id, which names where the host keeps its facts.
    pub fn id(&self) -> String {
        text(self.engine.pack().manifest.get("pack").and_then(|p| p.get("id")))
    }

    /// The pack's theme file as it was written, or null when it has none. The shell reads its
    /// tokens; the validator already checked them.
    pub fn theme(&self) -> Value {
        self.engine.pack().theme.clone().unwrap_or(data::Value::Null).into()
    }

    /// The IANA timezone the host turns its clock into before it passes `now`.
    pub fn timezone(&self) -> String {
        text(self.engine.pack().manifest.get("pack").and_then(|p| p.get("timezone")))
    }

    /// The other IANA zones the plan names; the host passes the local time in each.
    pub fn zones(&self) -> Vec<String> {
        self.engine.zones()
    }

    pub fn screen(&self, world: World) -> Result<View, CallError> {
        let mut nav = self.nav.lock().unwrap_or_else(PoisonError::into_inner);
        let view = self.engine.screen(&world.into(), &mut nav).map_err(refused)?;
        Ok(view.into())
    }

    /// The calendar sync for `scope`: empty for the whole trip, a date, or one event id. `known`
    /// is every event the host wrote before, by id, with its fingerprint.
    pub fn calendar(
        &self,
        world: World,
        scope: String,
        known: HashMap<String, String>,
    ) -> Result<Plan, CallError> {
        let known = known.into_iter().collect();
        let plan = self.engine.calendar(&world.into(), &scope, &known).map_err(refused)?;
        let events = |l: Vec<engine::Event>| l.into_iter().map(Event::from).collect();
        Ok(Plan { add: events(plan.add), change: events(plan.change), remove: plan.remove })
    }

    /// Every host the pack fetches from, sorted, for the person to approve before the first sync.
    pub fn hosts(&self) -> Vec<String> {
        self.engine.hosts()
    }

    /// The fetches `module` syncs with now; an empty `module` asks for the automatic syncs due.
    pub fn requests(&self, world: World, module: String) -> Result<Vec<Request>, CallError> {
        let requests = self.engine.requests(&world.into(), &module).map_err(refused)?;
        Ok(requests.into_iter().map(Request::from).collect())
    }

    /// The facts to store after fetching `request`: `status` is 0 when nothing answered, and
    /// `body` then says why.
    pub fn received(
        &self,
        world: World,
        request: Request,
        status: u16,
        body: String,
    ) -> Result<HashMap<String, Value>, CallError> {
        let facts = self.engine.received(&world.into(), &request.into(), status, &body);
        Ok(values(facts.map_err(refused)?))
    }

    pub fn dispatch(&self, world: World, action: String, arg: Value) -> Result<Outcome, CallError> {
        let mut nav = self.nav.lock().unwrap_or_else(PoisonError::into_inner);
        let out = self.engine.dispatch(&world.into(), &mut nav, &action, arg.into());
        let out = out.map_err(refused)?;
        let commands = out.commands.into_iter().map(Command::from).collect();
        Ok(Outcome { view: out.view.into(), store: values(out.store), commands })
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

/// The stored facts as text for a file on the device, in key order so the same facts make the
/// same file.
#[uniffi::export]
pub fn encode_facts(store: HashMap<String, Value>) -> String {
    facts::encode(&sorted(store))
}

/// The facts a file holds. A damaged file holds none.
#[uniffi::export]
pub fn decode_facts(text: String) -> HashMap<String, Value> {
    values(facts::decode(&text))
}

/// A theme file after an edit: which file of the pack it is, and its whole new text.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Edit {
    pub file: String,
    pub text: String,
}

/// Sets one value of the pack's theme file, at `path` from its root, and checks the whole pack
/// again. The new text comes back for the host to write where the file came from; an edit that
/// leaves an error, a text pair under 4.5:1 among them, is refused with it (decision 0006).
#[uniffi::export]
pub fn edit_theme(
    manifest: String,
    mut files: HashMap<String, String>,
    path: Vec<String>,
    value: String,
) -> Result<Edit, LoadError> {
    let unreadable = |detail: String| LoadError::Unreadable { detail };
    let head =
        files.get(&manifest).ok_or_else(|| unreadable(format!("{manifest}: no such file")))?;
    let head = yaml::parse(head).map_err(|e| unreadable(format!("{manifest}: {e}")))?;
    let file = text(head.get("pack").and_then(|p| p.get("theme")));
    let old = files.get(&file).ok_or_else(|| unreadable("the pack has no theme file".into()))?;
    let keys: Vec<&str> = path.iter().map(String::as_str).collect();
    let new = edit::set(old, &keys, &value).map_err(|e| unreadable(format!("{file}: {e}")))?;
    files.insert(file.clone(), new.clone());
    load(manifest, files)?;
    Ok(Edit { file, text: new })
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
      go: [{set: n, to: $arg}, {store: seen, value: $arg}, {do: map.open, with: {to: \"'here'\"}}]
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
        let (holder, inside) = (String::new(), vec![]);
        let (store, zones) = (HashMap::new(), HashMap::new());
        World { now: now.into(), holder, inside, located: false, store, zones, can: vec![] }
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
        let ask =
            "questions:\n  first: {ask: What is first?, answer: \"'Arrive'\", shortcut: true}\n";
        let pack = load("pack.yaml".into(), files(ask, day)).unwrap();
        assert_eq!(pack.warnings().len(), 1);
        assert_eq!(pack.timezone(), "UTC");
        assert_eq!(pack.id(), "t");
        assert_eq!(pack.theme(), Value::Null);
        let view = pack.screen(world("2026-04-11T10:00")).unwrap();
        assert_eq!((view.tree.version, view.tree.screen.as_str()), (tree_version(), "outline"));
        let node = |key: &str, kind: &str, props: &[(&str, &str)]| Node {
            key: key.into(),
            kind: kind.into(),
            props: props.iter().map(|(k, v)| ((*k).to_owned(), text(v))).collect(),
            on: HashMap::new(),
            children: vec![],
        };
        let expected = vec![
            node("name", "Title", &[("text", "Test")]),
            node("2026-04-11", "Row", &[("text", "Arrive"), ("caption", "2026-04-11")]),
        ];
        assert_eq!(view.tree.nodes, expected);
        assert_eq!(view.watch, Watch { until: "2026-04-12T00:00".into(), regions: vec![] });
        let first = Shortcut { id: "first".into(), ask: "What is first?".into() };
        assert_eq!(view.shortcuts, vec![first]);
        assert_eq!((view.tree.theme.as_str(), view.tree.kid), ("", false));
    }

    #[test]
    fn a_pack_hands_over_its_theme_and_the_holders() {
        let head =
            "pack: {id: t, name: T, language: en, timezone: UTC, content: c.yaml, theme: t.yaml}";
        let people = "days: []\npeople: [{id: tomas, adult: false}]\n";
        let places = "places:\n  home: {at: {lat: 1, lon: 2, radius_m: 50}}\n";
        let files = HashMap::from([
            ("pack.yaml".to_owned(), format!("{head}\nmodules: {{people: , places: }}\n")),
            ("c.yaml".to_owned(), format!("{people}{places}")),
            ("t.yaml".to_owned(), "themes: {}\n".to_owned()),
        ]);
        let pack = load("pack.yaml".into(), files).unwrap();
        let fields = vec![Field { key: "themes".into(), value: Value::Fields { fields: vec![] } }];
        assert_eq!(pack.theme(), Value::Fields { fields });
        let mut w = world("2026-04-11T10:00");
        w.holder = "tomas".into();
        let view = pack.screen(w).unwrap();
        assert_eq!((view.tree.theme.as_str(), view.tree.kid), ("", true));
        let home = Region { id: "home".into(), lat: 1.0, lon: 2.0, radius_m: 50.0 };
        assert_eq!(view.watch.regions, [home]);
    }

    /// A pack with a theme whose every text pair is black on white.
    fn themed() -> HashMap<String, String> {
        let head =
            "pack: {id: t, name: T, language: en, timezone: UTC, content: c.yaml, theme: t.yaml}\n";
        let ink = ["ink", "ink-muted", "alert", "action-ink", "highlight-ink", "highlight-text"];
        let ink = [&ink[..], &["chip-ink", "bar-ink", "bar-muted", "missing-text"]].concat();
        let colors: String = validate::COLOR_TOKENS
            .iter()
            .map(|t| {
                let c = if ink.contains(t) { "#000000" } else { "#FFFFFF" };
                format!("      {t}: \"{c}\"\n")
            })
            .collect();
        let theme = format!(
            "# kept\ndefault: a\nthemes:\n  a:\n    name: A\n    mode: light\n    colors:\n{colors}"
        );
        HashMap::from([
            ("pack.yaml".to_owned(), head.to_owned()),
            ("c.yaml".to_owned(), "days: []\n".to_owned()),
            ("t.yaml".to_owned(), theme),
        ])
    }

    fn edit(files: HashMap<String, String>, path: &[&str], value: &str) -> Result<Edit, LoadError> {
        let path = path.iter().map(|k| (*k).to_owned()).collect();
        edit_theme("pack.yaml".into(), files, path, value.into())
    }

    fn unreadable(detail: &str) -> Result<Edit, LoadError> {
        Err(LoadError::Unreadable { detail: detail.into() })
    }

    #[test]
    fn an_edit_needs_a_pack_with_a_theme_file_and_a_value_in_it() {
        let ink = ["themes", "a", "colors", "ink"];
        assert_eq!(edit(HashMap::new(), &ink, "#111111"), unreadable("pack.yaml: no such file"));
        let mut broken = themed();
        broken.insert("pack.yaml".into(), "pack: [\n".into());
        let e = edit(broken, &ink, "#111111").unwrap_err().to_string();
        assert!(e.starts_with("pack.yaml: line"), "{e}");
        let bare = files("", "days: []\n");
        assert_eq!(edit(bare, &ink, "#111111"), unreadable("the pack has no theme file"));
        let e = edit(themed(), &["themes", "b", "mode"], "dark");
        assert_eq!(e, unreadable("t.yaml: themes.b.mode: no \"b\" in a block mapping"));
    }

    #[test]
    fn an_edit_that_makes_text_unreadable_is_refused_with_the_pair() {
        let e = edit(themed(), &["themes", "a", "colors", "ink"], "#EEEEEE").unwrap_err();
        let pair = |f: &Finding| f.message.starts_with("ink on paper is 1.");
        assert!(matches!(&e, LoadError::Invalid { errors } if errors.iter().any(pair)), "{e:?}");
    }

    #[test]
    fn an_edit_comes_back_as_the_whole_file_with_the_rest_kept() {
        let before = themed()["t.yaml"].clone();
        let edited = edit(themed(), &["themes", "a", "colors", "ink"], "#111111").unwrap();
        assert_eq!(edited.file, "t.yaml");
        let ink = |c: &str| format!("\n      ink: \"{c}\"");
        assert_eq!(edited.text, before.replace(&ink("#000000"), &ink("#111111")));
        assert!(edited.text.starts_with("# kept\n"));
    }

    #[test]
    fn facts_go_to_text_in_key_order_and_come_back() {
        let store = HashMap::from([("b".to_owned(), text("2")), ("a".to_owned(), Value::Null)]);
        let saved = encode_facts(store.clone());
        assert_eq!(saved, "\"a\": null\n\"b\": \"2\"\n");
        assert_eq!(decode_facts(saved), store);
        assert!(decode_facts("not facts".into()).is_empty());
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
        let args = HashMap::from([("to".to_owned(), text("here"))]);
        assert_eq!(out.commands, [Command { name: "map.open".into(), args }]);
        let node = &out.view.tree.nodes[0];
        assert_eq!(node.props["text"], every);
        assert_eq!(node.on, HashMap::from([("tap".to_owned(), "go".to_owned())]));
        // The pack keeps where the person is between calls.
        assert_eq!(pack.screen(w).unwrap().tree.nodes[0].props["text"], every);
    }

    #[test]
    fn a_calendar_plan_hands_over_its_events_and_what_to_remove() {
        let definition = "modules: {timeline: }\nscreens: {a: {}}\nrules: [{screen: a}]\n";
        let content = "days: [{date: 2026-04-11, title: A, zone: Asia/Tokyo, blocks: [[\"10:00\", \"Walk. Slowly\"]]}]\n";
        let pack = load("pack.yaml".into(), files(definition, content)).unwrap();
        assert_eq!(pack.zones(), ["Asia/Tokyo"]);
        let e = pack.calendar(world("today"), String::new(), HashMap::new()).unwrap_err();
        assert!(matches!(e, CallError::Refused { .. }));
        let known = HashMap::from([("2026-04-11.blocks.5".to_owned(), "x".to_owned())]);
        let mut w = world("2026-04-10T10:00");
        let zones = [("Asia/Tokyo", "2026-04-10T19:00"), ("UTC", "2026-04-10T10:00")];
        w.zones = zones.iter().map(|(z, t)| ((*z).to_owned(), (*t).to_owned())).collect();
        let plan = pack.calendar(w, "2026-04-11".into(), known).unwrap();
        assert_eq!(plan.remove, ["2026-04-11.blocks.5"]);
        assert!(plan.change.is_empty());
        let walk = &plan.add[0];
        assert_eq!(
            [&walk.id, &walk.title, &walk.start, &walk.end],
            ["2026-04-11.blocks.0", "Walk", "2026-04-11T10:00", "2026-04-11T11:00"]
        );
        assert_eq!([&walk.location, &walk.notes, &walk.reminder], ["", "Walk. Slowly", ""]);
        assert_eq!([&walk.zone, &walk.end_zone], ["Asia/Tokyo", "Asia/Tokyo"]);
        assert_eq!(walk.fingerprint.len(), 16);
    }

    #[test]
    fn a_sync_hands_over_its_hosts_its_requests_and_the_facts_a_reply_makes() {
        let definition = "modules:
  climate:
    sync:
      trigger: button
      request: {url: 'https://api.example.org/f', query: {day: now.date}}
      tag: {place: \"'lisbon'\"}
      read: {high: max}
screens: {a: {}}
rules: [{screen: a}]
";
        let day = "days: [{date: 2026-04-11, title: A}]\n";
        let pack = load("pack.yaml".into(), files(definition, day)).unwrap();
        assert_eq!(pack.hosts(), ["api.example.org"]);
        assert!(pack.requests(world("today"), "climate".into()).is_err());
        let now = world("2026-04-11T10:00");
        let request = pack.requests(now.clone(), "climate".into()).unwrap().remove(0);
        assert_eq!(request.url, "https://api.example.org/f?day=2026-04-11");
        assert_eq!((request.secret.as_str(), request.param.as_str()), ("", ""));
        assert_eq!(request.tag, HashMap::from([("place".to_owned(), text("lisbon"))]));
        let facts =
            pack.received(now.clone(), request.clone(), 200, "{\"max\": 21}".into()).unwrap();
        let sync = data::Value::from(facts["sync.climate"].clone());
        let rows = data::show(sync.get("rows"));
        assert_eq!(rows, r#"[{"high": 21, "place": "lisbon"}]"#);
        let elsewhere = Request { module: "alerts".into(), ..request };
        assert!(pack.received(now, elsewhere, 200, String::new()).is_err());
    }
}
