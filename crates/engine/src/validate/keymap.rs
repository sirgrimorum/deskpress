//! A pack whose keys are in another language declares a keymap instead of being rewritten. This
//! reads a pack through it: canonical names in, the pack's own keys and values out.

use crate::value::{Map, Value, text, truthy};

/// The `keymap` of a manifest: a pack's own names for the canonical keys, roots and values.
#[derive(Clone, Copy)]
pub struct Keymap<'a> {
    map: Option<&'a Map>,
}

impl<'a> Keymap<'a> {
    /// The keymap of a manifest; none is an empty keymap.
    pub fn of(manifest: &'a Value) -> Self {
        Keymap { map: manifest.get("keymap").and_then(Value::as_map) }
    }

    fn section(&self, name: &str) -> Option<&'a Map> {
        self.map?.get(name)?.as_map()
    }

    /// A field of `obj` by its canonical name, or by the name the keymap gives it for this kind.
    pub fn read<'v>(&self, obj: &'v Value, kind: &str, canonical: &str) -> Option<&'v Value> {
        self.field(obj.as_map()?, kind, canonical)
    }

    /// `read`, on a mapping already in hand.
    pub fn field<'v>(&self, obj: &'v Map, kind: &str, canonical: &str) -> Option<&'v Value> {
        if let Some(v) = obj.get(canonical) {
            return Some(v);
        }
        let renamed = self.section(kind)?.get(canonical).filter(|r| truthy(Some(r)))?;
        obj.get(&text(Some(renamed)))
    }

    /// The pack's own names for the fields of this kind, which are known keys and not free ones.
    pub fn renames(&self, kind: &str) -> Vec<String> {
        let values = self.section(kind).map(Map::iter).into_iter().flatten();
        values
            .filter_map(|(_, v)| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }

    /// `canonical` and every name any section gives it, for a walk that does not know the kind.
    pub fn names(&self, canonical: &str) -> Vec<String> {
        let sections = self.map.map(Map::iter).into_iter().flatten();
        let renamed = sections.map(|(_, s)| text(s.get(canonical))).filter(|r| !r.is_empty());
        std::iter::once(canonical.to_owned()).chain(renamed).collect()
    }

    /// A root collection of the content, by its canonical name or the dotted path the keymap
    /// gives it (`keymap.root.days: itinerario.dias`).
    pub fn root<'v>(&self, content: &'v Map, canonical: &str) -> Option<&'v Value> {
        if let Some(v) = content.get(canonical) {
            return Some(v);
        }
        let path = self.section("root")?.get(canonical).filter(|p| truthy(Some(p)))?;
        let path = text(Some(path));
        let mut steps = path.split('.');
        let first = content.get(steps.next().unwrap_or_default())?;
        steps.try_fold(first, |node, step| node.get(step))
    }

    /// The top level key of the content that holds a root: the first step of its keymap path.
    pub fn root_key(&self, canonical: &str) -> String {
        let path = self.section("root").and_then(|r| r.get(canonical)).map(|p| text(Some(p)));
        let path = path.filter(|p| !p.is_empty()).unwrap_or_else(|| canonical.to_owned());
        path.split('.').next().unwrap_or_default().to_owned()
    }

    /// `obj` with the pack's own key names turned back into canonical ones, so a module hands
    /// every pack the same shape. Anything that is not a mapping comes back as it is.
    pub fn canon(&self, obj: &Value, kind: &str) -> Value {
        let Some(map) = obj.as_map() else {
            return obj.clone();
        };
        let section = self.section(kind);
        let named = |key: &str| {
            let found = section?.iter().find(|(_, r)| matches!(r, Value::String(s) if s == key));
            found.map(|(canonical, _)| canonical.to_owned())
        };
        let keys = map.iter().map(|(k, v)| (named(k).unwrap_or_else(|| k.to_owned()), v.clone()));
        Value::Map(Map(keys.collect()))
    }

    /// The canonical word for a value of an enumeration, through `keymap.values.<name>`.
    pub fn value(&self, name: &str, value: Option<&Value>) -> String {
        let theirs = text(value);
        let table = self.section("values").and_then(|v| v.get(name)).and_then(Value::as_map);
        table
            .and_then(|t| t.iter().find(|(_, v)| text(Some(v)) == theirs))
            .map_or(theirs.clone(), |(canonical, _)| canonical.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml::parse;

    fn manifest(keymap: &str) -> Value {
        parse(&format!("keymap:\n{keymap}")).unwrap()
    }

    #[test]
    fn no_keymap_reads_canonical_keys_only() {
        let m = parse("pack: {}").unwrap();
        let k = Keymap::of(&m);
        let obj = parse("fecha: x\ndate: y").unwrap();
        assert_eq!(k.read(&obj, "day", "date"), Some(&Value::String("y".into())));
        assert_eq!(k.read(&obj, "day", "title"), None);
        assert_eq!(k.read(&Value::Null, "day", "date"), None);
        assert!(k.renames("day").is_empty());
        assert_eq!(k.value("severity", Some(&Value::String("alta".into()))), "alta");
    }

    #[test]
    fn a_renamed_key_is_read_under_its_own_name() {
        let m = manifest("  day: {date: fecha, title: '', blocks: 3}\n");
        let k = Keymap::of(&m);
        let obj = parse("fecha: x\n'3': y").unwrap();
        assert_eq!(k.read(&obj, "day", "date"), Some(&Value::String("x".into())));
        assert_eq!(k.read(&obj, "day", "title"), None);
        assert_eq!(k.read(&obj, "day", "blocks"), Some(&Value::String("y".into())));
        assert_eq!(k.read(&obj, "day", "who"), None);
        assert_eq!(k.read(&obj, "place", "date"), None);
        assert_eq!(k.renames("day"), ["fecha", ""]);
    }

    #[test]
    fn a_root_can_live_at_a_path() {
        let m = manifest(
            "  root: {days: itinerario.dias, places: '', people: nada.x, alerts: nope, documents: nope.x}\n",
        );
        let k = Keymap::of(&m);
        let content = parse("itinerario:\n  dias: [1]\nnada: 2\nalerts: []").unwrap();
        let content = content.as_map().unwrap();
        assert_eq!(k.root(content, "alerts"), Some(&Value::List(vec![])));
        assert_eq!(k.root(content, "days"), Some(&Value::List(vec![Value::Number(1.0)])));
        assert_eq!(k.root(content, "places"), None);
        assert_eq!(k.root(content, "people"), None);
        assert_eq!(k.root(content, "documents"), None);
    }

    #[test]
    fn a_value_is_translated_to_its_canonical_word() {
        let m = manifest("  values:\n    severity: {high: alta, low: baja}\n");
        let k = Keymap::of(&m);
        let alta = Value::String("alta".into());
        assert_eq!(k.value("severity", Some(&alta)), "high");
        assert_eq!(k.value("severity", Some(&Value::String("media".into()))), "media");
        assert_eq!(k.value("type", Some(&alta)), "alta");
    }
}
