//! The engine as the app sees it, through UniFFI. This crate only converts: every rule lives in
//! `deskpress-engine`, which never depends on UniFFI.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use deskpress_engine::{pack, tree, validate};

uniffi::setup_scaffolding!();

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Finding {
    pub at: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Node {
    pub kind: String,
    pub props: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Tree {
    pub version: u32,
    pub screen: String,
    pub nodes: Vec<Node>,
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

/// A pack that passed validation. It may still have warnings.
#[derive(uniffi::Object)]
pub struct LoadedPack {
    pack: pack::Pack,
    warnings: Vec<Finding>,
}

#[uniffi::export]
impl LoadedPack {
    pub fn warnings(&self) -> Vec<Finding> {
        self.warnings.clone()
    }

    pub fn screen(&self) -> Tree {
        let tree = tree::outline(&self.pack);
        let nodes = tree
            .nodes
            .into_iter()
            .map(|n| Node { kind: n.kind, props: n.props.into_iter().collect() })
            .collect();
        Tree { version: tree.version, screen: tree.screen, nodes }
    }
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
    let report = validate::validate(&pack.manifest, &pack.content, pack.theme.as_ref());
    let findings = |list: Vec<validate::Finding>| {
        list.into_iter().map(|f| Finding { at: f.at, message: f.message }).collect()
    };
    if !report.ok() {
        return Err(LoadError::Invalid { errors: findings(report.errors) });
    }
    Ok(Arc::new(LoadedPack { pack, warnings: findings(report.warnings) }))
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

    fn files(content: &str) -> HashMap<String, String> {
        HashMap::from([
            ("pack.yaml".to_owned(), MANIFEST.to_owned()),
            ("content.yaml".to_owned(), content.to_owned()),
        ])
    }

    fn load_err(files: HashMap<String, String>) -> LoadError {
        load("pack.yaml".into(), files).err().unwrap()
    }

    #[test]
    fn a_missing_file_is_unreadable() {
        let e = load_err(HashMap::new());
        assert_eq!(e, LoadError::Unreadable { detail: "pack.yaml: no such file".into() });
        assert_eq!(e.to_string(), "pack.yaml: no such file");
    }

    #[test]
    fn a_pack_with_errors_is_invalid_and_lists_them() {
        let e = load_err(files("days: x\n"));
        let at = "days".to_owned();
        let message = "has to be a list, one entry per day".to_owned();
        assert_eq!(e, LoadError::Invalid { errors: vec![Finding { at, message }] });
        assert_eq!(e.to_string(), "1 errors");
    }

    #[test]
    fn a_valid_pack_loads_with_its_warnings_and_draws_its_outline() {
        let day = "days:\n  - {date: 2026-04-11, title: Arrive}\n";
        let pack = load("pack.yaml".into(), files(day)).unwrap();
        assert_eq!(pack.warnings().len(), 1);
        let tree = pack.screen();
        assert_eq!((tree.version, tree.screen.as_str()), (tree_version(), "outline"));
        let props = |pairs: &[(&str, &str)]| {
            pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
        };
        let expected = vec![
            Node { kind: "Title".into(), props: props(&[("text", "Test")]) },
            Node {
                kind: "Row".into(),
                props: props(&[("text", "Arrive"), ("caption", "2026-04-11")]),
            },
        ];
        assert_eq!(tree.nodes, expected);
    }
}
