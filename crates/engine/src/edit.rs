//! Changes one scalar of a YAML text in place. Every other byte stays: comments, order, blank
//! lines, and how the rest of the file quotes its values. This is how a theme edit in the app is
//! written back to the file it came from (decision 0006).

use crate::value::{Value, quote};
use crate::yaml::parse;

/// `text` with the scalar at `path`, a key of a block mapping per level from the root, set to
/// `value`. A quoted value stays quoted. A plain one stays plain while the new text reads back as
/// the same kind of value, a number as a number; otherwise it is quoted, so a text never turns into
/// a number or a boolean by accident.
pub fn set(text: &str, path: &[&str], value: &str) -> Result<String, String> {
    if path.is_empty() {
        return Err("an edit needs a path".to_owned());
    }
    let shown = path.join(".");
    // A byte order mark and each line's `\r` are kept aside and put back as they were.
    let (bom, text) = text.strip_prefix('\u{feff}').map_or(("", text), |t| ("\u{feff}", t));
    let raw: Vec<&str> = text.split('\n').collect();
    let lines: Vec<&str> = raw.iter().map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    let (mut start, mut end, mut at) = (0, lines.len(), 0);
    for (depth, key) in path.iter().enumerate() {
        let Some(i) = find(&lines[start..end], key) else {
            return Err(format!("{shown}: no {} in a block mapping", quote(key)));
        };
        at = start + i;
        if depth + 1 < path.len() {
            if !value_of(after_key(lines[at]).unwrap_or_default()).is_empty() {
                return Err(format!("{shown}: {key} is not a block mapping"));
            }
            let indent = indent(lines[at]);
            start = at + 1;
            end = (start..lines.len())
                .find(|&i| content(lines[i]) && self::indent(lines[i]) <= indent)
                .unwrap_or(lines.len());
        }
    }
    let line = lines[at];
    let rest = after_key(line).unwrap_or_default();
    let old = value_of(rest);
    if old.is_empty() || old.starts_with(['{', '[', '|', '>', '&', '*']) {
        return Err(format!("{shown} is not a single value"));
    }
    let from = line.len() - rest.trim_start().len();
    let new = scalar(old, value);
    let end = &raw[at][from + old.len()..];
    let edited = format!("{}{new}{end}", &line[..from]);
    let mut out = raw;
    out[at] = &edited;
    let out = out.join("\n");
    parse(&out).map_err(|e| format!("{shown}: {e}"))?;
    Ok(format!("{bom}{out}"))
}

/// The new value as YAML, in the style of the old one.
fn scalar(old: &str, value: &str) -> String {
    let read = |s: &str| parse(&format!("k: {s}")).ok().and_then(|v| v.get("k").cloned());
    let plain = !old.starts_with(['"', '\'']) && !value.contains(" #") && !value.contains(": ");
    let reads = read(value);
    // A text must read back as itself: plain YAML drops a trailing space, for one.
    let same = match &reads {
        Some(Value::String(s)) => s == value,
        _ => true,
    };
    if plain && same && read(old).as_ref().map(kind_of) == reads.as_ref().map(kind_of) {
        value.to_owned()
    } else {
        quote(value)
    }
}

fn kind_of(v: &Value) -> u8 {
    match v {
        Value::Number(_) => 1,
        Value::String(_) => 2,
        _ => 0,
    }
}

/// The line of `lines` that holds `key` at the level's indent: the first content line sets it.
fn find(lines: &[&str], key: &str) -> Option<usize> {
    let level = lines.iter().find(|l| content(l)).map(|l| indent(l))?;
    lines.iter().position(|l| content(l) && indent(l) == level && key_of(l) == Some(key))
}

/// Not blank and not only a comment.
fn content(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && !t.starts_with('#')
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// The key of a `key: value` line, unquoted.
fn key_of(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let rest = after_key(line)?;
    let key = t[..t.len() - rest.len() - 1].trim_end();
    Some(key.strip_prefix('"').and_then(|k| k.strip_suffix('"')).unwrap_or(key))
}

/// What follows the colon of a `key:` line.
fn after_key(line: &str) -> Option<&str> {
    let t = line.trim_start();
    // A quoted key may hold a colon: the search starts after its closing quote.
    let search = t.strip_prefix('"').map_or(0, |k| k.find('"').map_or(t.len(), |i| i + 2));
    let colon = t[search..]
        .char_indices()
        .find(|&(i, c)| c == ':' && t[search + i + 1..].chars().next().is_none_or(|n| n == ' '))
        .map(|(i, _)| search + i)?;
    Some(&t[colon + 1..])
}

/// The value part of what follows a key, without its comment.
fn value_of(rest: &str) -> &str {
    let t = rest.trim_start();
    if let Some(q @ ('"' | '\'')) = t.chars().next() {
        // A quote closes at the next unescaped quote; '' is a quote inside single quotes.
        let bytes = t.as_bytes();
        let mut i = 1;
        while i < bytes.len() {
            let b = bytes[i];
            if q == '"' && b == b'\\' {
                i += 2;
                continue;
            }
            if b == q as u8 {
                if q == '\'' && bytes.get(i + 1) == Some(&b'\'') {
                    i += 2;
                    continue;
                }
                return &t[..=i];
            }
            i += 1;
        }
        return t;
    }
    if t.starts_with('#') {
        return "";
    }
    let end = t.find(" #").unwrap_or(t.len());
    t[..end].trim_end()
}

#[cfg(test)]
mod tests {
    use super::*;

    const THEME: &str = "# the theme\ndefault: a\n\nthemes:\n  a:\n    name: A # shown\n    colors:\n      ink: \"#111111\"\n      paper: '#FFFFFF'   # the page\n  b:\n    colors:\n      ink: \"#222222\"\ntype:\n  hero: {size: 64px}\n  body:\n    size: 18\n    tracking: -0.02em\n";

    fn edit(path: &[&str], value: &str) -> Result<String, String> {
        set(THEME, path, value)
    }

    #[test]
    fn a_path_that_is_not_there_is_refused() {
        assert_eq!(edit(&[], "x").unwrap_err(), "an edit needs a path");
        assert_eq!(edit(&["nope"], "x").unwrap_err(), "nope: no \"nope\" in a block mapping");
        assert_eq!(
            edit(&["themes", "c", "colors", "ink"], "x").unwrap_err(),
            "themes.c.colors.ink: no \"c\" in a block mapping"
        );
        // A key of the second theme is not found under the first.
        assert!(edit(&["themes", "a", "colors", "missing"], "x").is_err());
        assert_eq!(edit(&["", "x"], "x").unwrap_err(), ".x: no \"\" in a block mapping");
        assert_eq!(set("", &["a"], "x").unwrap_err(), "a: no \"a\" in a block mapping");
    }

    #[test]
    fn only_a_single_value_in_a_block_mapping_is_edited() {
        assert_eq!(
            edit(&["default", "x"], "y").unwrap_err(),
            "default.x: default is not a block mapping"
        );
        assert_eq!(edit(&["themes"], "y").unwrap_err(), "themes is not a single value");
        assert_eq!(edit(&["type", "hero"], "y").unwrap_err(), "type.hero is not a single value");
        // A flow mapping is left alone: changing one would mean writing YAML, not editing a value.
        assert_eq!(
            edit(&["type", "hero", "size"], "y").unwrap_err(),
            "type.hero.size: hero is not a block mapping"
        );
    }

    #[test]
    fn an_edit_that_would_break_the_file_is_refused() {
        let e = set("k: 1\nk: 2\n", &["k"], "3").unwrap_err();
        assert!(e.starts_with("k: line 2: duplicate key"), "{e}");
    }

    #[test]
    fn the_rest_of_the_file_stays_as_it_was() {
        let out = edit(&["themes", "a", "colors", "ink"], "#333333").unwrap();
        assert_eq!(out, THEME.replace("ink: \"#111111\"", "ink: \"#333333\""));
        let out = edit(&["themes", "b", "colors", "ink"], "#444444").unwrap();
        assert_eq!(out, THEME.replace("ink: \"#222222\"", "ink: \"#444444\""));
    }

    #[test]
    fn a_quoted_value_stays_quoted_and_keeps_its_comment() {
        let out = edit(&["themes", "a", "colors", "paper"], "#FAFAFA").unwrap();
        assert!(out.contains("      paper: \"#FAFAFA\"   # the page\n"), "{out}");
        let out = edit(&["themes", "a", "name"], "Ana's").unwrap();
        assert!(out.contains("    name: Ana's # shown\n"), "{out}");
        assert_eq!(set("k: 'it''s'\n", &["k"], "x").unwrap(), "k: \"x\"\n");
        assert_eq!(set("k: \"a\\\"b\" # c\n", &["k"], "x").unwrap(), "k: \"x\" # c\n");
        assert_eq!(set("\"k\": v\n", &["k"], "w").unwrap(), "\"k\": w\n");
        assert_eq!(set("k: 1\n", &["k"], "\"").unwrap(), "k: \"\\\"\"\n");
        // An unclosed quote runs to the end of the line.
        assert_eq!(set("k: 'open\n", &["k"], "w").unwrap(), "k: \"w\"\n");
    }

    #[test]
    fn a_plain_value_stays_plain_only_as_the_same_kind() {
        let at = |v| edit(&["type", "body", "size"], v).unwrap();
        assert!(at("20").contains("    size: 20\n"));
        assert!(at("20px").contains("    size: \"20px\"\n"));
        let at = |v| edit(&["type", "body", "tracking"], v).unwrap();
        assert!(at("0.01em").contains("    tracking: 0.01em\n"));
        assert!(at("true").contains("    tracking: \"true\"\n"));
        assert!(at("12").contains("    tracking: \"12\"\n"));
        assert!(at("a #b").contains("    tracking: \"a #b\"\n"));
        assert!(at("a: b").contains("    tracking: \"a: b\"\n"));
        assert!(at("[x").contains("    tracking: \"[x\"\n"));
        assert!(at("b ").contains("    tracking: \"b \"\n"));
    }

    #[test]
    fn line_ends_a_byte_order_mark_and_a_comment_after_a_key_stay() {
        let crlf = "k: 1\r\ns:\r\n  x: a # c\r\n";
        assert_eq!(set(crlf, &["s", "x"], "b").unwrap(), crlf.replace("a #", "b #"));
        let bom = "\u{feff}k: v\n";
        assert_eq!(set(bom, &["k"], "w").unwrap(), "\u{feff}k: w\n");
        let commented = "s: # the section\n  x: a\n";
        assert_eq!(set(commented, &["s", "x"], "b").unwrap(), "s: # the section\n  x: b\n");
    }

    #[test]
    fn a_line_that_is_not_a_key_is_passed_over() {
        let text = "list:\n  - a\nurl: http://x\nk: v\n";
        assert_eq!(set(text, &["k"], "w").unwrap(), "list:\n  - a\nurl: http://x\nk: w\n");
        assert_eq!(set("- a\n", &["k"], "w").unwrap_err(), "k: no \"k\" in a block mapping");
    }
}
