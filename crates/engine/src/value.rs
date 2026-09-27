//! The data a pack is made of, as the YAML reader produces it.

/// One YAML value. Mappings keep the order of the file, because the order is what renders.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    List(Vec<Value>),
    Map(Map),
}

/// An ordered mapping. Pack mappings are small, so a lookup is a scan.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Map(pub Vec<(String, Value)>);

impl Map {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(k, _)| k.as_str())
    }

    /// Replaces the value of `key` where it stands, or adds it at the end.
    pub fn set(&mut self, key: &str, value: Value) {
        match self.0.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = value,
            None => self.0.push((key.to_owned(), value)),
        }
    }
}

impl Value {
    pub fn as_map(&self) -> Option<&Map> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(l) => Some(l),
            _ => None,
        }
    }

    /// A key of this mapping, or nothing when this is not a mapping.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_map().and_then(|m| m.get(key))
    }
}

/// Whether a value counts as present: not missing, null, false, zero or empty text.
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => *n != 0.0,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::List(_) | Value::Map(_)) => true,
    }
}

/// A value as text, with missing and null as the empty string.
pub fn text(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::List(items)) => {
            items.iter().map(|i| text(Some(i))).collect::<Vec<_>>().join(",")
        }
        Some(Value::Map(_)) => "[object]".to_owned(),
    }
}

/// A value as a message shows it: quoted text, and `nothing` when it is missing.
pub fn show(v: Option<&Value>) -> String {
    match v {
        None => "nothing".to_owned(),
        Some(Value::Null) => "null".to_owned(),
        Some(Value::String(s)) => quote(s),
        Some(Value::List(items)) => {
            let inner: Vec<String> = items.iter().map(|i| show(Some(i))).collect();
            format!("[{}]", inner.join(", "))
        }
        Some(Value::Map(m)) => {
            let inner: Vec<String> =
                m.iter().map(|(k, v)| format!("{}: {}", quote(k), show(Some(v)))).collect();
            format!("{{{}}}", inner.join(", "))
        }
        other => text(other),
    }
}

/// Text in double quotes, escaped the way JSON does.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(t: &str) -> Value {
        Value::String(t.to_owned())
    }

    #[test]
    fn a_lookup_on_anything_but_a_mapping_is_nothing() {
        assert_eq!(s("x").get("k"), None);
        assert_eq!(s("x").as_map(), None);
        assert_eq!(s("x").as_list(), None);
        let m = Value::Map(Map(vec![("k".into(), Value::Null)]));
        assert_eq!(m.get("k"), Some(&Value::Null));
        assert_eq!(m.get("other"), None);
        assert_eq!(m.as_map().unwrap().keys().collect::<Vec<_>>(), ["k"]);
        assert_eq!(Value::List(vec![]).as_list(), Some(&[][..]));
    }

    #[test]
    fn setting_a_key_keeps_its_place() {
        let mut m = Map(vec![("a".into(), s("1")), ("b".into(), s("2"))]);
        m.set("a", s("3"));
        m.set("c", s("4"));
        assert_eq!(m.keys().collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(m.get("a"), Some(&s("3")));
    }

    #[test]
    fn what_counts_as_absent() {
        for v in [None, Some(&Value::Null), Some(&Value::Bool(false)), Some(&Value::Number(0.0))] {
            assert!(!truthy(v));
        }
        assert!(!truthy(Some(&s(""))));
        for v in [Value::Bool(true), Value::Number(2.0), s("a"), Value::List(vec![])] {
            assert!(truthy(Some(&v)));
        }
        assert!(truthy(Some(&Value::Map(Map::default()))));
    }

    #[test]
    fn values_as_text() {
        assert_eq!(text(None), "");
        assert_eq!(text(Some(&Value::Null)), "");
        assert_eq!(text(Some(&Value::Bool(true))), "true");
        assert_eq!(text(Some(&Value::Number(7.0))), "7");
        assert_eq!(text(Some(&Value::Number(12.5))), "12.5");
        assert_eq!(text(Some(&Value::List(vec![s("a"), Value::Null, s("b")]))), "a,,b");
        assert_eq!(text(Some(&Value::Map(Map::default()))), "[object]");
    }

    #[test]
    fn values_as_a_message_shows_them() {
        assert_eq!(show(None), "nothing");
        assert_eq!(show(Some(&Value::Null)), "null");
        assert_eq!(show(Some(&Value::Number(1000.0))), "1000");
        assert_eq!(show(Some(&Value::List(vec![s("a"), Value::Bool(false)]))), r#"["a", false]"#);
        let m = Value::Map(Map(vec![("k".into(), s("v"))]));
        assert_eq!(show(Some(&m)), r#"{"k": "v"}"#);
    }

    #[test]
    fn quoting_escapes_what_json_escapes() {
        assert_eq!(quote("a\"b\\c\nd\te\rf\u{1}g é"), r#""a\"b\\c\nd\te\rf\u0001g é""#);
    }
}
