//! A pack on disk is a folder of YAML files. This reads one through a callback, so the engine never
//! touches a file system: the CLI hands it the disk, the app hands it its assets.

use std::collections::HashSet;

use crate::template;
use crate::value::{Map, Value, quote, text, truthy};
use crate::yaml;

/// The three parts of a pack, parsed and not yet validated.
#[derive(Debug, Clone, PartialEq)]
pub struct Pack {
    pub manifest: Value,
    pub content: Value,
    pub theme: Option<Value>,
}

impl Pack {
    /// What a person calls this pack: its name, else its id.
    pub fn name(&self) -> Option<String> {
        let head = self.manifest.get("pack");
        let pick = |key| head.and_then(|h| h.get(key)).filter(|v| truthy(Some(v)));
        pick("name").or_else(|| pick("id")).map(|v| text(Some(v)))
    }
}

/// Reads a pack whose manifest is `file`. `read` takes a path relative to the pack folder and
/// returns the file's text; every error comes back as one line naming the file.
pub fn load(
    file: &str,
    mut read: impl FnMut(&str) -> Result<String, String>,
) -> Result<Pack, String> {
    let manifest = parse(file, &mut read)?;
    let manifest = template::extend(manifest).map_err(|e| format!("{file}: {e}"))?;
    let head = manifest.get("pack");
    let field = |key| head.and_then(|h| h.get(key));

    // content is one file, a list of files, or a mapping of root key to file. The mapping roots
    // each file under the key that names it, the only form where two files cannot collide.
    let content = match field("content") {
        Some(Value::Map(roots)) => {
            let mut rooted = Vec::with_capacity(roots.0.len());
            for (root, file) in roots.iter() {
                let value = named(file, &format!("pack.content.{root}"), &mut read)?;
                rooted.push((root.to_owned(), value));
            }
            Value::Map(Map(rooted))
        }
        Some(Value::List(files)) => merge(files, &mut read)?,
        Some(file) if truthy(Some(file)) => merge(std::slice::from_ref(file), &mut read)?,
        _ => Value::Map(Map::default()),
    };
    let theme = match field("theme") {
        Some(file) if truthy(Some(file)) => Some(named(file, "pack.theme", &mut read)?),
        _ => None,
    };
    Ok(Pack { manifest, content, theme })
}

/// Content files side by side, each a mapping of root keys. A key in two files is refused: the
/// second would quietly win.
fn merge(
    files: &[Value],
    read: &mut impl FnMut(&str) -> Result<String, String>,
) -> Result<Value, String> {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();
    for file in files {
        let name = text(Some(file));
        let entries = match named(file, "pack.content", read)? {
            Value::Map(m) => m.0,
            Value::Null => Vec::new(),
            _ => {
                return Err(format!(
                    "{name}: a content file in a list is a mapping of root keys, like days: and places:"
                ));
            }
        };
        for (key, value) in entries {
            if !seen.insert(key.clone()) {
                return Err(format!(
                    "{name}: {} is already defined in an earlier content file, and the second one would quietly win. Split the pack by key, or name the root in the manifest: content: {{{key}: {name}}}",
                    quote(&key)
                ));
            }
            merged.push((key, value));
        }
    }
    Ok(Value::Map(Map(merged)))
}

/// A file the manifest names, at `at`. It has to stay inside the pack folder: the app loads packs
/// that came from somewhere else.
fn named(
    file: &Value,
    at: &str,
    read: &mut impl FnMut(&str) -> Result<String, String>,
) -> Result<Value, String> {
    let file = text(Some(file));
    let path = file.replace('\\', "/");
    let drive = path.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && path.as_bytes().get(1) == Some(&b':');
    if path.is_empty() {
        return Err(format!("{at}: names no file"));
    }
    if path.starts_with('/') || drive || path.split('/').any(|s| s == "..") {
        return Err(format!(
            "{at}: {} leaves the pack folder. Every file a pack names lives inside it",
            quote(&file)
        ));
    }
    parse(&file, read)
}

fn parse(
    file: &str,
    read: &mut impl FnMut(&str) -> Result<String, String>,
) -> Result<Value, String> {
    let source = read(file).map_err(|e| format!("{file}: {e}"))?;
    yaml::parse(&source).map_err(|e| format!("{file}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pack folder in memory: a file that is not listed does not exist.
    fn folder(files: &[(&str, &str)]) -> impl FnMut(&str) -> Result<String, String> {
        let files: Vec<(String, String)> =
            files.iter().map(|(n, t)| ((*n).to_owned(), (*t).to_owned())).collect();
        move |name| {
            files
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, t)| t.clone())
                .ok_or_else(|| "no such file".to_owned())
        }
    }

    fn keys(v: &Value) -> Vec<&str> {
        v.as_map().unwrap().keys().collect()
    }

    #[test]
    fn a_missing_or_broken_manifest_is_one_line_naming_it() {
        assert_eq!(load("pack.yaml", folder(&[])), Err("pack.yaml: no such file".into()));
        let broken = folder(&[("pack.yaml", "a: 1\na: 2")]);
        let e = load("pack.yaml", broken).unwrap_err();
        assert!(e.starts_with("pack.yaml: line 2: "), "{e}");
    }

    #[test]
    fn a_manifest_is_read_over_the_template_it_extends() {
        let unknown = folder(&[("pack.yaml", "pack: {extends: blog}")]);
        let e = load("pack.yaml", unknown).unwrap_err();
        assert_eq!(e, "pack.yaml: pack.extends: \"blog\" is not a template: travel");
        let travel = folder(&[("pack.yaml", "pack: {extends: travel}")]);
        let pack = load("pack.yaml", travel).unwrap();
        assert!(pack.manifest.get("screens").is_some());
    }

    #[test]
    fn a_file_that_leaves_the_pack_folder_is_refused() {
        for bad in [
            "/etc/x.yaml",
            "\\\\server\\x.yaml",
            "C:/x.yaml",
            "c:x.yaml",
            "../x.yaml",
            "a/../../x.yaml",
            "a\\..\\x.yaml",
        ] {
            let manifest = format!("pack:\n  content: {}\n", quote(bad));
            let e = load("pack.yaml", folder(&[("pack.yaml", &manifest)])).unwrap_err();
            assert_eq!(
                e,
                format!(
                    "pack.content: {} leaves the pack folder. Every file a pack names lives inside it",
                    quote(bad)
                )
            );
        }
        let manifest = "pack:\n  content: {days: ../days.yaml}\n";
        let e = load("pack.yaml", folder(&[("pack.yaml", manifest)])).unwrap_err();
        assert!(e.starts_with("pack.content.days: \"../days.yaml\" leaves"), "{e}");
        let manifest = "pack:\n  content: {days: ''}\n";
        let e = load("pack.yaml", folder(&[("pack.yaml", manifest)])).unwrap_err();
        assert_eq!(e, "pack.content.days: names no file");
        // A colon second is a drive only after a letter.
        let manifest = "pack:\n  content: '1:x.yaml'\n";
        let e = load("pack.yaml", folder(&[("pack.yaml", manifest)])).unwrap_err();
        assert_eq!(e, "1:x.yaml: no such file");
        let manifest = "pack:\n  theme: /theme.yaml\n";
        let e = load("pack.yaml", folder(&[("pack.yaml", manifest)])).unwrap_err();
        assert!(e.starts_with("pack.theme: \"/theme.yaml\" leaves"), "{e}");
    }

    #[test]
    fn a_missing_or_broken_content_file_is_refused() {
        let manifest = "pack:\n  content: content.yaml\n";
        let e = load("pack.yaml", folder(&[("pack.yaml", manifest)])).unwrap_err();
        assert_eq!(e, "content.yaml: no such file");
        let e = load("pack.yaml", folder(&[("pack.yaml", manifest), ("content.yaml", "\tx: 1")]))
            .unwrap_err();
        assert!(e.starts_with("content.yaml: line 1: "), "{e}");
        let manifest = "pack:\n  content: {days: days.yaml}\n";
        let e = load("pack.yaml", folder(&[("pack.yaml", manifest)])).unwrap_err();
        assert_eq!(e, "days.yaml: no such file");
        let manifest = "pack:\n  theme: theme.yaml\n";
        let e = load("pack.yaml", folder(&[("pack.yaml", manifest)])).unwrap_err();
        assert_eq!(e, "theme.yaml: no such file");
    }

    #[test]
    fn two_content_files_that_define_one_key_are_refused() {
        let manifest = "pack:\n  content: [a.yaml, b.yaml]\n";
        let files = [("pack.yaml", manifest), ("a.yaml", "days: []"), ("b.yaml", "days: []")];
        let e = load("pack.yaml", folder(&files)).unwrap_err();
        assert_eq!(
            e,
            "b.yaml: \"days\" is already defined in an earlier content file, and the second one would quietly win. Split the pack by key, or name the root in the manifest: content: {days: b.yaml}"
        );
    }

    #[test]
    fn a_content_file_in_a_list_is_a_mapping() {
        let manifest = "pack:\n  content: [a.yaml]\n";
        let e =
            load("pack.yaml", folder(&[("pack.yaml", manifest), ("a.yaml", "- 1")])).unwrap_err();
        assert!(e.starts_with("a.yaml: a content file in a list is a mapping of root keys"), "{e}");
    }

    #[test]
    fn content_can_be_one_file_a_list_or_a_mapping_of_roots() {
        let one = folder(&[
            ("pack.yaml", "pack:\n  content: c.yaml\n"),
            ("c.yaml", "days: []\nplaces: {}"),
        ]);
        assert_eq!(keys(&load("pack.yaml", one).unwrap().content), ["days", "places"]);

        let list = folder(&[
            ("pack.yaml", "pack:\n  content: [a.yaml, empty.yaml, b.yaml]\n"),
            ("a.yaml", "days: []"),
            ("empty.yaml", ""),
            ("b.yaml", "people: []\nalerts: []"),
        ]);
        assert_eq!(keys(&load("pack.yaml", list).unwrap().content), ["days", "people", "alerts"]);

        let rooted = folder(&[
            ("pack.yaml", "pack:\n  content: {days: d/days.yaml, people: people.yaml}\n"),
            ("d/days.yaml", "- {date: 2026-04-11}"),
            ("people.yaml", "- {id: rita}"),
        ]);
        let pack = load("pack.yaml", rooted).unwrap();
        assert_eq!(keys(&pack.content), ["days", "people"]);
        assert_eq!(pack.content.get("days").and_then(Value::as_list).map(<[Value]>::len), Some(1));
    }

    #[test]
    fn no_content_and_no_theme_load_as_nothing_for_the_validator_to_judge() {
        for manifest in ["pack: {}", "pack:\n  content: ''\n  theme: ''\n", "just: text"] {
            let pack = load("pack.yaml", folder(&[("pack.yaml", manifest)])).unwrap();
            assert_eq!(pack.content, Value::Map(Map::default()), "{manifest}");
            assert_eq!(pack.theme, None, "{manifest}");
        }
    }

    #[test]
    fn a_theme_is_read_when_the_manifest_names_one() {
        let files = [("pack.yaml", "pack:\n  theme: theme.yaml\n"), ("theme.yaml", "themes: {}")];
        let pack = load("pack.yaml", folder(&files)).unwrap();
        assert!(pack.theme.is_some_and(|t| t.get("themes").is_some()));
    }

    #[test]
    fn the_name_is_the_name_then_the_id() {
        let name = |manifest: &str| load("m.yaml", folder(&[("m.yaml", manifest)])).unwrap().name();
        assert_eq!(name("pack: {id: one-day, name: One day}"), Some("One day".into()));
        assert_eq!(name("pack: {id: one-day, name: ''}"), Some("one-day".into()));
        assert_eq!(name("pack: {}"), None);
        assert_eq!(name("x: 1"), None);
    }
}
