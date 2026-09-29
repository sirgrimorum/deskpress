//! Stored facts as text, for a host to keep on the device between runs. The engine owns the
//! format, so a host never has to agree with it on one.

use crate::value::{Map, Value, quote, show};
use crate::yaml;

/// One `"key": value` line per fact, in the order they were stored.
pub fn encode(store: &Map) -> String {
    store.iter().map(|(k, v)| format!("{}: {}\n", quote(k), show(Some(v)))).collect()
}

/// The facts `text` holds. Text that is not a mapping holds none: a damaged file costs the facts,
/// never the pack.
pub fn decode(text: &str) -> Map {
    let value = yaml::parse(text).unwrap_or(Value::Null);
    value.as_map().cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml::parse;

    #[test]
    fn text_that_is_not_facts_holds_none() {
        for text in ["", "just words", "- a\n- b", "a: [", "\"a\": {"] {
            assert_eq!(decode(text), Map::default(), "{text:?}");
        }
    }

    #[test]
    fn every_kind_of_value_comes_back_as_it_went() {
        let store = parse(
            r#"s:
  holder: tomas
  choice.2026-04-11: coast
  "odd \"key\"": "line one\nline \\ two, ñ"
  looks like true: "true"
  number: 1.5
  whole: 3
  yes: true
  none: null
  list: [a, 2, [b]]
  map: {id: tomas, seen: [x]}
"#,
        )
        .unwrap();
        let store = store.get("s").and_then(Value::as_map).unwrap().clone();
        let text = encode(&store);
        assert!(text.starts_with("\"holder\": \"tomas\"\n"), "{text}");
        assert_eq!(decode(&text), store);
        assert_eq!(encode(&Map::default()), "");
    }
}
