//! The screen tree: what the engine hands a renderer. Plain data, the same on every platform.

use crate::TREE_VERSION;
use crate::define::{Component, Screen};
use crate::pack::Pack;
use crate::validate::Keymap;
use crate::validate::patterns::dated_key;
use crate::value::{Map, Value, text, truthy};

#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    pub version: u32,
    pub screen: String,
    pub nodes: Vec<Node>,
    /// The theme of the person holding the phone, as the pack names it; empty for the default.
    pub theme: String,
    /// The person holding the phone is a child: kid type, borders and boxes.
    pub kid: bool,
}

/// One component and its props. A renderer knows the closed set of kinds and nothing about packs.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: String,
    pub props: Vec<(String, Value)>,
    /// Event to the action it runs, like `("tap", "confirm")`.
    pub on: Vec<(String, String)>,
    /// What a Group holds, drawn inside it. Empty for every other kind.
    pub children: Vec<Node>,
}

impl Node {
    fn new(kind: &str, props: &[(&str, String)]) -> Self {
        let props =
            props.iter().map(|(k, v)| ((*k).to_owned(), Value::String(v.clone()))).collect();
        Node { kind: kind.to_owned(), props, on: Vec::new(), children: Vec::new() }
    }
}

/// What a pack with no rules shows: its name, then one row per day.
pub fn outline(pack: &Pack) -> Tree {
    let keymap = Keymap::of(&pack.manifest);
    let days = pack.content.as_map().and_then(|c| keymap.root(c, "days"));
    let mut nodes = vec![Node::new("Title", &[("text", pack.name().unwrap_or_default())])];
    for day in days.and_then(Value::as_list).unwrap_or_default() {
        let read = |key| text(keymap.read(day, "day", key));
        nodes.push(Node::new("Row", &[("text", read("title")), ("caption", read("date"))]));
    }
    Tree {
        version: TREE_VERSION,
        screen: "outline".to_owned(),
        nodes,
        theme: String::new(),
        kid: false,
    }
}

/// What the tree reads from the manifest besides the screen: the prefixes of the unknown key rule.
#[derive(Debug, Clone, PartialEq)]
pub struct Conventions {
    pub hidden: Vec<String>,
    pub alerts: Vec<String>,
}

impl Conventions {
    /// The manifest's conventions; with none, no key is hidden and `warn` and `alert` warn.
    pub fn of(manifest: &Value) -> Self {
        let read = |key: &str, default: &[&str]| {
            let list = manifest.get("conventions").and_then(|c| c.get(key));
            let own = list.and_then(Value::as_list).map(|l| l.iter().map(|v| text(Some(v))));
            match own {
                Some(own) => own.collect(),
                None => default.iter().map(|s| (*s).to_owned()).collect(),
            }
        };
        let alerts = read("alert_prefixes", &["warn", "alert"]);
        Conventions { hidden: read("hidden_prefixes", &[]), alerts }
    }
}

/// The tree of `screen`, its props evaluated over `scope`.
pub fn build(name: &str, screen: &Screen, scope: &mut Map, conventions: &Conventions) -> Tree {
    let mut nodes = Vec::new();
    draw(&screen.layout, scope, conventions, &mut nodes);
    Tree { version: TREE_VERSION, screen: name.to_owned(), nodes, theme: String::new(), kid: false }
}

/// The nodes of `layout` over `scope`. While a component repeats its item is `item`, and a Group's
/// is `group` too; both are back to what they were afterwards.
fn draw(layout: &[Component], scope: &mut Map, conventions: &Conventions, nodes: &mut Vec<Node>) {
    for c in layout {
        let Some(each) = &c.each else {
            node(c, scope, conventions, nodes);
            continue;
        };
        let names: &[&str] = if c.kind == "Group" { &["item", "group"] } else { &["item"] };
        let before: Vec<Option<Value>> = names.iter().map(|n| scope.get(n).cloned()).collect();
        let items = each.eval(scope).into_owned();
        for item in items.as_list().unwrap_or_default() {
            for name in names {
                scope.set(name, item.clone());
            }
            node(c, scope, conventions, nodes);
        }
        for (name, before) in names.iter().zip(before) {
            match before {
                Some(v) => scope.set(name, v),
                None => scope.0.retain(|(k, _)| k != name),
            }
        }
    }
}

fn node(c: &Component, scope: &mut Map, conventions: &Conventions, nodes: &mut Vec<Node>) {
    if c.when.as_ref().is_some_and(|w| !truthy(Some(&w.eval(scope)))) {
        return;
    }
    let props: Vec<(String, Value)> =
        c.props.iter().map(|(k, p)| (k.clone(), p.eval(scope))).collect();
    if c.kind == "Group" {
        // A Group with nothing inside is left out, like a Card with nothing to say.
        let mut children = Vec::new();
        draw(&c.children, scope, conventions, &mut children);
        if !children.is_empty() {
            nodes.push(Node { kind: c.kind.clone(), props, on: c.on.clone(), children });
        }
        return;
    }
    let read = Reader { scope, conventions };
    if c.kind == "Auto" {
        let prop = |key: &str| props.iter().find(|(k, _)| k == key).map(|(_, v)| v);
        read.auto(prop("value"), prop("skip"), nodes);
        return;
    }
    read.push(Node { kind: c.kind.clone(), props, on: c.on.clone(), children: Vec::new() }, nodes);
}

/// What a node needs to be drawn as the pack format says: path references resolved, a value to
/// confirm drawn as `Missing`, a card with nothing to say left out.
struct Reader<'a> {
    scope: &'a Map,
    conventions: &'a Conventions,
}

impl Reader<'_> {
    fn push(&self, mut node: Node, nodes: &mut Vec<Node>) {
        let slot = node.props.iter().position(|(k, _)| k == "text");
        if let Some(i) = slot {
            let body = self.body(&node.props[i].1);
            if node.kind == "Card" && body.is_empty() {
                return;
            }
            let ui = self.scope.get("ui");
            let marker = text(ui.and_then(|u| u.get("to_confirm")));
            if !marker.is_empty() && body.contains(&format!("[{marker}]")) {
                node.kind = "Missing".to_owned();
            }
            if ["Card", "Missing", "Alert"].contains(&node.kind.as_str()) {
                node.props[i].1 = Value::String(body);
            }
        }
        nodes.push(node);
    }

    /// The unknown key rule over the entries of a mapping, leaving out the keys in `skip`. A list
    /// is a card per item, and any other value one card.
    fn auto(&self, value: Option<&Value>, skip: Option<&Value>, nodes: &mut Vec<Node>) {
        let card = |text: &Value| Node {
            kind: "Card".to_owned(),
            props: vec![("text".to_owned(), text.clone())],
            on: Vec::new(),
            children: Vec::new(),
        };
        match value {
            None | Some(Value::Null | Value::Map(_)) => {}
            Some(Value::List(items)) => {
                return items.iter().for_each(|i| self.push(card(i), nodes));
            }
            Some(one) => return self.push(card(one), nodes),
        }
        let skip = skip.and_then(Value::as_list).unwrap_or_default();
        let skipped = |k: &str| skip.iter().any(|s| text(Some(s)) == k);
        let today = text(self.scope.get("now").and_then(|n| n.get("date")));
        let entries = value.and_then(Value::as_map).map(Map::iter).into_iter().flatten();
        for (key, v) in entries.filter(|(k, _)| !skipped(k) && !self.hidden(k)) {
            let at = key.len().saturating_sub(12);
            let (stem, date) = match dated_key(key) {
                Some(_) => (&key[..at], Some(key[at + 2..].replace('_', "-"))),
                None => (key, None),
            };
            let alert = self.conventions.alerts.iter().any(|p| key.starts_with(p.as_str()));
            let kind = match date {
                Some(date) if date != today => continue,
                Some(_) => "Alert",
                None if alert => "Alert",
                None => "Card",
            };
            let props = vec![
                ("title".to_owned(), Value::String(stem.replace('_', " "))),
                ("text".to_owned(), v.clone()),
            ];
            self.push(
                Node { kind: kind.to_owned(), props, on: Vec::new(), children: Vec::new() },
                nodes,
            );
        }
    }

    fn hidden(&self, key: &str) -> bool {
        self.conventions.hidden.iter().any(|p| key.starts_with(p.as_str()))
    }

    /// A value as the text of a card: a path reference by what it points to, a list one line per
    /// item, a mapping one `key: value` line per key it shows.
    fn body(&self, v: &Value) -> String {
        let lines = |lines: Vec<String>| {
            let kept: Vec<String> = lines.into_iter().filter(|l| !l.is_empty()).collect();
            kept.join("\n")
        };
        match v {
            Value::String(s) => self.resolve(s).map_or_else(|| s.clone(), |found| self.body(found)),
            Value::List(items) => lines(items.iter().map(|i| self.body(i)).collect()),
            Value::Map(m) => {
                let shown = m.iter().filter(|(k, _)| !self.hidden(k));
                let line = |(k, v): (&str, &Value)| {
                    let body = self.body(v);
                    if body.is_empty() { body } else { format!("{}: {body}", k.replace('_', " ")) }
                };
                lines(shown.map(line).collect())
            }
            other => text(Some(other)),
        }
    }

    /// What a path reference like `bookings.azulejo` points to in the content: a mapping walks by
    /// key, a list by `id`. Text that is not one, or points nowhere, is nothing, and so is one that
    /// points at another reference: following those could loop.
    fn resolve(&self, path: &str) -> Option<&Value> {
        if !is_path(path) {
            return None;
        }
        let found = path.split('.').try_fold(self.scope.get("content")?, |node, step| match node {
            Value::List(items) => items.iter().find(|i| text(i.get("id")) == step),
            _ => node.get(step),
        });
        found.filter(|f| !matches!(f, Value::String(s) if is_path(s)))
    }
}

/// `^[a-z_][a-z0-9_]*(\.[a-z0-9_]+)+$`, the shape of a path reference.
fn is_path(s: &str) -> bool {
    let word = |w: &str| {
        !w.is_empty()
            && w.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    };
    let first = s.split('.').next().unwrap_or_default();
    s.contains('.') && s.split('.').all(word) && !first.starts_with(|c: char| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::define::define;
    use crate::yaml::parse;

    fn pack(manifest: &str, content: &str) -> Pack {
        Pack { manifest: parse(manifest).unwrap(), content: parse(content).unwrap(), theme: None }
    }

    fn row(text: &str, caption: &str) -> Node {
        Node::new("Row", &[("text", text.to_owned()), ("caption", caption.to_owned())])
    }

    #[test]
    fn a_pack_with_nothing_readable_is_a_title_alone() {
        let empty =
            Pack { manifest: Value::Null, content: Value::Map(Map::default()), theme: None };
        for p in [empty, pack("pack: {}", "days: x"), pack("pack: {}", "- 1")] {
            let tree = outline(&p);
            assert_eq!(tree.nodes, [Node::new("Title", &[("text", String::new())])]);
            assert_eq!((tree.version, tree.screen.as_str()), (TREE_VERSION, "outline"));
        }
    }

    #[test]
    fn the_outline_is_the_name_then_a_row_per_day_through_the_keymap() {
        let p = pack(
            "pack: {name: Trip}\nkeymap:\n  root: {days: dias}\n  day: {date: fecha, title: titulo}\n",
            "dias:\n  - {fecha: 2026-04-11, titulo: Arrive}\n  - {fecha: 2026-04-12}\n  - x\n",
        );
        let expected = vec![
            Node::new("Title", &[("text", "Trip".to_owned())]),
            row("Arrive", "2026-04-11"),
            row("", "2026-04-12"),
            row("", ""),
        ];
        assert_eq!(outline(&p).nodes, expected);
    }

    #[test]
    fn a_screen_is_its_components_in_order_each_repeated_and_hidden_by_their_if() {
        let manifest = parse(
            "screens:
  a:
    actions: {go: [back]}
    layout:
      - Label: {text: \"'hi'\", if: store.show}
      - Chip: {each: store.tags, if: \"item != 'b'\", text: \"#{item}\", on_tap: go}
      - Chip: {each: store.none, text: item}
      - Row: {text: store.n}
",
        )
        .unwrap();
        let (def, r) = define(&manifest);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let screen = def.screen("a").unwrap();
        let chip = |t: &str| Node {
            kind: "Chip".into(),
            props: vec![("text".into(), Value::String(format!("#{t}")))],
            on: vec![("tap".into(), "go".into())],
            children: vec![],
        };
        let store = |yaml: &str| {
            let store = parse(yaml).unwrap();
            let mut scope = Map::default();
            scope.set("store", store);
            scope
        };
        let mut scope = store(
            "tags: [a, b, c]
n: 3",
        );
        let tree = build("a", screen, &mut scope, &Conventions::of(&Value::Null));
        let n = Node {
            kind: "Row".into(),
            props: vec![("text".into(), Value::Number(3.0))],
            on: vec![],
            children: vec![],
        };
        assert_eq!(tree.nodes, [chip("a"), chip("c"), n]);
        assert_eq!((tree.version, tree.screen.as_str()), (TREE_VERSION, "a"));
        // `item` is gone once the component is drawn.
        assert_eq!(scope.keys().collect::<Vec<_>>(), ["store"]);
        let mut scope = store(
            "show: true
tags: x",
        );
        let tree = build("a", screen, &mut scope, &Conventions::of(&Value::Null));
        assert_eq!(tree.nodes[0], Node::new("Label", &[("text", "hi".to_owned())]));
        assert_eq!(tree.nodes.len(), 2);
    }

    #[test]
    fn a_group_holds_its_nodes_and_is_left_out_with_none() {
        let manifest = parse(
            "screens:
  a:
    layout:
      - Group:
          each: store.people
          title: item.name
          layout:
            - Row: {each: store.docs, if: \"item.for == group.id\", text: item.title}
            - Label: {text: item.name}
      - Group: {title: \"'none'\", layout: [{Row: {each: store.none, text: item}}]}
",
        )
        .unwrap();
        let (def, r) = define(&manifest);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let store = "store: {people: [{id: a, name: Ann}, {id: b, name: Ben}], docs: [{for: a, title: Card}]}";
        let mut scope = parse(store).unwrap().as_map().cloned().unwrap();
        let tree = build("a", def.screen("a").unwrap(), &mut scope, &Conventions::of(&Value::Null));
        let group = |title: &str, children: Vec<Node>| Node {
            children,
            ..Node::new("Group", &[("title", title.to_owned())])
        };
        let label = |t: &str| Node::new("Label", &[("text", t.to_owned())]);
        let card = Node::new("Row", &[("text", "Card".to_owned())]);
        // Inside, `item` is the group's again once a child's own `each` is done.
        let want = [group("Ann", vec![card, label("Ann")]), group("Ben", vec![label("Ben")])];
        assert_eq!(tree.nodes, want);
        assert_eq!(scope.keys().collect::<Vec<_>>(), ["store"]);
    }

    /// The kind and text of each node `layout` draws over `scope`.
    fn drawn(manifest: &str, layout: &str, scope: &str) -> Vec<String> {
        let (def, r) = define(&parse(&format!("screens:\n  a:\n    layout:\n{layout}")).unwrap());
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let mut scope = parse(scope).unwrap().as_map().cloned().unwrap_or_default();
        let conventions = Conventions::of(&parse(manifest).unwrap());
        let tree = build("a", def.screen("a").unwrap(), &mut scope, &conventions);
        let prop = |n: &Node, k: &str| text(n.props.iter().find(|(p, _)| p == k).map(|(_, v)| v));
        let node = |n: &Node| format!("{} {}|{}", n.kind, prop(n, "title"), prop(n, "text"));
        tree.nodes.iter().map(node).collect()
    }

    const FACTS: &str = r#"now: {date: 2026-04-11}
ui: {to_confirm: to confirm}
content:
  bookings: [{id: museum, code: M-1, source: web, gate: ""}, {id: loop, next: bookings.museum}]
  hops: bookings.museum
store:
  hours: 10 to 18
  source_page: x
  warn_bags: Lockers
  alert: Closed at noon
  note__2026_04_11: Today only
  note__2026_04_12: Tomorrow only
  warn_gate__2026_04_12: Tomorrow too
  note__2026_4_11x: Odd date
  ticket: bookings.museum
  loop: bookings.loop.next
  hop: hops.x
  gone: bookings.nowhere
  not_a_path: Bookings.museum
  numbers: [1, "", [2, 3]]
  price: "[to confirm]"
  empty: ""
  skipped: yes
"#;

    #[test]
    fn auto_follows_the_unknown_key_rule() {
        let layout = "      - Auto: {value: store, skip: [skipped]}\n";
        let own = "conventions: {hidden_prefixes: [source], alert_prefixes: [warn]}";
        let got = drawn(own, layout, FACTS);
        let want = [
            "Card hours|10 to 18",
            "Alert warn bags|Lockers",
            "Card alert|Closed at noon",
            "Alert note|Today only",
            "Card note  2026 4 11x|Odd date",
            "Card ticket|id: museum\ncode: M-1",
            "Card loop|bookings.loop.next",
            "Card hop|hops.x",
            "Card gone|bookings.nowhere",
            "Card not a path|Bookings.museum",
            "Card numbers|1\n2\n3",
            "Missing price|[to confirm]",
        ];
        assert_eq!(got, want);
        // With no conventions nothing is hidden, and warn and alert both warn.
        let got = drawn("pack: {}", layout, FACTS);
        assert_eq!(
            got[1..4],
            ["Card source page|x", "Alert warn bags|Lockers", "Alert alert|Closed at noon"]
        );
        // A single value is one card, a list a card per item, and nothing is nothing.
        let auto = |value: &str| drawn("", &format!("      - Auto: {{value: {value}}}\n"), FACTS);
        assert_eq!(auto("store.hours"), ["Card |10 to 18"]);
        assert_eq!(auto("store.numbers"), ["Card |1", "Card |2\n3"]);
        assert!(auto("store.nothing").is_empty());
    }

    #[test]
    fn a_value_to_confirm_is_missing_only_when_the_pack_says_how_it_writes_one() {
        let layout = "      - Card: {text: store.price}\n      - Row: {text: store.price}\n      - Card: {text: store.empty}\n      - Label: {text: store.ticket}\n";
        assert_eq!(
            drawn("", layout, FACTS),
            ["Missing |[to confirm]", "Missing |[to confirm]", "Label |bookings.museum"]
        );
        let scope = FACTS.replace("ui: {to_confirm: to confirm}", "ui: {}");
        assert_eq!(drawn("", layout, &scope)[..2], ["Card |[to confirm]", "Row |[to confirm]"]);
        // Without content in scope a reference is only text.
        let scope = "store: {ticket: bookings.museum}";
        assert_eq!(
            drawn("", "      - Card: {text: store.ticket}\n", scope),
            ["Card |bookings.museum"]
        );
    }
}
