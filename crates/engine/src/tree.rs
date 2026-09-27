//! The screen tree: what the engine hands a renderer. Plain data, the same on every platform.

use crate::TREE_VERSION;
use crate::define::{Component, Screen};
use crate::pack::Pack;
use crate::validate::Keymap;
use crate::value::{Map, Value, text, truthy};

#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    pub version: u32,
    pub screen: String,
    pub nodes: Vec<Node>,
}

/// One component and its props. A renderer knows the closed set of kinds and nothing about packs.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: String,
    pub props: Vec<(String, Value)>,
    /// Event to the action it runs, like `("tap", "confirm")`.
    pub on: Vec<(String, String)>,
}

impl Node {
    fn new(kind: &str, props: &[(&str, String)]) -> Self {
        let props =
            props.iter().map(|(k, v)| ((*k).to_owned(), Value::String(v.clone()))).collect();
        Node { kind: kind.to_owned(), props, on: Vec::new() }
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
    Tree { version: TREE_VERSION, screen: "outline".to_owned(), nodes }
}

/// The tree of `screen`, its props evaluated over `scope`. `item` is set while a component repeats
/// and gone afterwards.
pub fn build(name: &str, screen: &Screen, scope: &mut Map) -> Tree {
    let mut nodes = Vec::new();
    for c in &screen.layout {
        let Some(each) = &c.each else {
            nodes.extend(node(c, scope));
            continue;
        };
        let items = each.eval(scope).into_owned();
        for item in items.as_list().unwrap_or_default() {
            scope.set("item", item.clone());
            nodes.extend(node(c, scope));
        }
        scope.0.retain(|(k, _)| k != "item");
    }
    Tree { version: TREE_VERSION, screen: name.to_owned(), nodes }
}

fn node(c: &Component, scope: &Map) -> Option<Node> {
    if c.when.as_ref().is_some_and(|w| !truthy(Some(&w.eval(scope)))) {
        return None;
    }
    let props = c.props.iter().map(|(k, p)| (k.clone(), p.eval(scope))).collect();
    Some(Node { kind: c.kind.clone(), props, on: c.on.clone() })
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
        let tree = build("a", screen, &mut scope);
        let n = Node {
            kind: "Row".into(),
            props: vec![("text".into(), Value::Number(3.0))],
            on: vec![],
        };
        assert_eq!(tree.nodes, [chip("a"), chip("c"), n]);
        assert_eq!((tree.version, tree.screen.as_str()), (TREE_VERSION, "a"));
        // `item` is gone once the component is drawn.
        assert_eq!(scope.keys().collect::<Vec<_>>(), ["store"]);
        let tree = build(
            "a",
            screen,
            &mut store(
                "show: true
tags: x",
            ),
        );
        assert_eq!(tree.nodes[0], Node::new("Label", &[("text", "hi".to_owned())]));
        assert_eq!(tree.nodes.len(), 2);
    }
}
