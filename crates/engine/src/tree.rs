//! The screen tree: what the engine hands a renderer. Plain data, the same on every platform.

use crate::TREE_VERSION;
use crate::pack::Pack;
use crate::validate::Keymap;
use crate::value::{Value, text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    pub version: u32,
    pub screen: String,
    pub nodes: Vec<Node>,
}

/// One component and its props. A renderer knows the closed set of kinds and nothing about packs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub kind: String,
    pub props: Vec<(String, String)>,
}

impl Node {
    fn new(kind: &str, props: &[(&str, String)]) -> Self {
        let props = props.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect();
        Node { kind: kind.to_owned(), props }
    }
}

/// The phase 2 screen: the pack's name, then one row per day. Phase 3 replaces it with the screen
/// the pack's rules pick.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Map;
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
}
