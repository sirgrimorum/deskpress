//! A YAML reader for the subset a pack uses, with no dependencies.
//!
//! Supported: block mappings and sequences, flow sequences and mappings (which may wrap onto the
//! following lines), literal and folded block scalars with chomping, single and double quoted
//! scalars, comments, and the plain scalar coercions (null, booleans, numbers). Dates stay text.
//!
//! Refused with a line number instead of guessed: tabs for indentation, a second document,
//! duplicate keys and a `__proto__` key, which a JavaScript consumer would lose, a `\u` escape
//! that is half a surrogate pair, and nesting deeper than 100 levels.
//!
//! One simplification: in a folded block (`>`), a more indented line folds like the others
//! instead of keeping its line break.

use std::collections::HashSet;
use std::fmt;

use crate::value::{Map, Value, quote};

/// Why a file is not YAML this reader accepts, and on which line (counting from 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for YamlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for YamlError {}

type Result<T> = std::result::Result<T, YamlError>;

fn err<T>(message: impl Into<String>, line: usize) -> Result<T> {
    Err(YamlError { line, message: message.into() })
}

const TRUE: [&str; 8] = ["true", "True", "TRUE", "yes", "Yes", "YES", "on", "On"];
const FALSE: [&str; 8] = ["false", "False", "FALSE", "no", "No", "NO", "off", "Off"];
const NULL: [&str; 5] = ["", "~", "null", "Null", "NULL"];

// Each level of nesting is a call, so a hostile file could run the stack out. No pack comes close.
const MAX_DEPTH: usize = 100;
const TOO_DEEP: &str = "nested deeper than 100 levels";

/// Reads one document. An empty file is `Null`.
pub fn parse(text: &str) -> Result<Value> {
    let text =
        text.strip_prefix('\u{feff}').unwrap_or(text).replace("\r\n", "\n").replace('\r', "\n");
    let mut doc = Reader { lines: text.split('\n').map(str::to_owned).collect(), i: 0, depth: 0 };
    doc.open_document();
    doc.skip_blanks()?;
    if doc.eof() {
        return Ok(Value::Null);
    }
    // Every reader stops on a line that is not blank, so anything left is a line out of place.
    let value = doc.node(0)?;
    if !doc.eof() {
        return err("content left over after the document", doc.line_no());
    }
    Ok(value)
}

/// The entries of a mapping being read, refusing a key seen twice before its value is read.
#[derive(Default)]
struct Entries {
    list: Vec<(String, Value)>,
    seen: HashSet<String>,
}

impl Entries {
    fn claim(&mut self, key: &str, line: usize) -> Result<()> {
        if !self.seen.insert(key.to_owned()) {
            return err(format!("duplicate key {}", quote(key)), line);
        }
        Ok(())
    }

    fn push(&mut self, key: String, value: Value) {
        self.list.push((key, value));
    }

    fn done(self) -> Value {
        Value::Map(Map(self.list))
    }
}

struct Reader {
    lines: Vec<String>,
    i: usize,
    depth: usize,
}

impl Reader {
    fn eof(&self) -> bool {
        self.i >= self.lines.len()
    }

    fn line_no(&self) -> usize {
        self.i + 1
    }

    // Every nested block is a sequence or a mapping. Each calls this first and steps back out
    // when it is done.
    fn enter(&mut self) -> Result<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return err(TOO_DEEP, self.line_no());
        }
        Ok(())
    }

    // A single leading "---" opens this document. Any later one starts a second, which is refused.
    fn open_document(&mut self) {
        while let Some(line) = self.lines.get(self.i) {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                self.i += 1;
                continue;
            }
            if trimmed == "---" {
                self.i += 1;
            }
            return;
        }
    }

    fn skip_blanks(&mut self) -> Result<()> {
        while let Some(line) = self.lines.get(self.i) {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                self.i += 1;
                continue;
            }
            if trimmed == "---" {
                return err(
                    "a second document starts here. A pack is one document per file, and merging two of them would lose keys quietly",
                    self.line_no(),
                );
            }
            if trimmed == "..." {
                let more = self.lines[self.i + 1..].iter().any(|l| !is_blank(l));
                if more {
                    return err("the document ends here, but the file keeps going", self.line_no());
                }
                self.i = self.lines.len();
                return Ok(());
            }
            if line.chars().take_while(|c| c.is_whitespace()).any(|c| c == '\t') {
                return err("tab used for indentation. YAML needs spaces", self.line_no());
            }
            return Ok(());
        }
        Ok(())
    }

    // The current line, split into its indentation and its content.
    fn head(&self) -> (usize, String) {
        let line = &self.lines[self.i];
        let indent = leading_spaces(line);
        (indent, line[indent..].trim_end().to_owned())
    }

    fn node(&mut self, min_indent: usize) -> Result<Value> {
        self.skip_blanks()?;
        if self.eof() {
            return Ok(Value::Null);
        }
        let (indent, text) = self.head();
        if indent < min_indent {
            return Ok(Value::Null);
        }
        if is_seq_entry(&text) { self.sequence(indent) } else { self.mapping(indent) }
    }

    fn sequence(&mut self, indent: usize) -> Result<Value> {
        self.enter()?;
        let mut out = Vec::new();
        loop {
            self.skip_blanks()?;
            if self.eof() {
                break;
            }
            let (at, text) = self.head();
            if at != indent || !is_seq_entry(&text) {
                break;
            }
            let rest = text[1..].trim_start();
            let rest_column = indent + text.chars().count() - rest.chars().count();
            if rest.is_empty() {
                self.i += 1;
                out.push(self.node(indent + 1)?);
            } else if is_mapping_start(rest) {
                // "- id: coast" opens a mapping whose other keys sit under the text, not under the
                // dash. Blanking the dash lets the mapping reader see a normal line.
                self.lines[self.i] = format!("{}{rest}", " ".repeat(rest_column));
                out.push(self.mapping(rest_column)?);
            } else if is_block_header(rest) {
                self.i += 1;
                out.push(Value::String(self.block_scalar(rest, indent)));
            } else {
                let line = self.line_no();
                self.i += 1;
                let joined = self.join_flow(rest, line)?;
                out.push(scalar(&joined, line)?);
            }
        }
        self.depth -= 1;
        Ok(Value::List(out))
    }

    fn mapping(&mut self, indent: usize) -> Result<Value> {
        self.enter()?;
        let mut out = Entries::default();
        loop {
            self.skip_blanks()?;
            if self.eof() {
                break;
            }
            let (at, text) = self.head();
            if at < indent {
                break;
            }
            let line = self.line_no();
            if at > indent {
                return err("unexpected indentation", line);
            }
            if is_seq_entry(&text) {
                break;
            }
            let Some((key, rest)) = split_key(&text) else {
                return err(format!("expected \"key: value\", found {}", quote(&text)), line);
            };
            let key = check_key(unquote(&key, line)?, line)?;
            out.claim(&key, line)?;
            self.i += 1;
            let value = if is_block_header(&rest) {
                Value::String(self.block_scalar(&rest, indent))
            } else if rest.is_empty() {
                self.child_of(indent)?
            } else {
                scalar(&self.join_flow(&rest, line)?, line)?
            };
            out.push(key, value);
        }
        self.depth -= 1;
        Ok(out.done())
    }

    // The value of a key with nothing after the colon: a nested block, a sequence written at the
    // key's own indentation (both styles are common), or nothing at all.
    fn child_of(&mut self, indent: usize) -> Result<Value> {
        self.skip_blanks()?;
        if self.eof() {
            return Ok(Value::Null);
        }
        let (at, text) = self.head();
        if at == indent && is_seq_entry(&text) {
            return self.sequence(indent);
        }
        if at > indent {
            return self.node(indent + 1);
        }
        Ok(Value::Null)
    }

    // A flow collection may wrap: keep taking lines until the brackets balance. Each check
    // rescans the text, which is fine for the few lines a collection wraps over.
    fn join_flow(&mut self, text: &str, line: usize) -> Result<String> {
        let mut joined = text.to_owned();
        if !text.starts_with(['[', '{']) {
            return Ok(joined);
        }
        while !flow_complete(&joined) {
            let Some(next) = self.lines.get(self.i) else {
                return err("a flow collection was never closed", line);
            };
            self.i += 1;
            if !is_blank(next) {
                joined.push(' ');
                joined.push_str(next.trim());
            }
        }
        Ok(joined)
    }

    fn block_scalar(&mut self, header: &str, indent: usize) -> String {
        let folded = header.starts_with('>');
        let strip = header.contains('-');
        let mut base = header.chars().find_map(|c| c.to_digit(10)).map(|d| indent + d as usize);
        let mut body: Vec<String> = Vec::new();
        while let Some(line) = self.lines.get(self.i) {
            if line.trim().is_empty() {
                body.push(String::new());
                self.i += 1;
                continue;
            }
            let at = leading_spaces(line);
            let from = *base.get_or_insert(at);
            if at <= indent || at < from {
                break;
            }
            body.push(line[from..].trim_end().to_owned());
            self.i += 1;
        }
        while body.last().is_some_and(String::is_empty) {
            body.pop();
        }
        let mut text = if folded { fold(&body) } else { body.join("\n") };
        if !strip {
            text.push('\n');
        }
        text
    }
}

// Lines become one paragraph joined by spaces; a blank line is a line break.
fn fold(body: &[String]) -> String {
    let mut out = String::new();
    for line in body {
        if line.is_empty() {
            out.push('\n');
        } else {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push(' ');
            }
            out.push_str(line);
        }
    }
    out
}

fn is_blank(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_seq_entry(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next() == Some('-') && chars.next().is_none_or(char::is_whitespace)
}

fn is_mapping_start(text: &str) -> bool {
    !text.starts_with(['[', '{']) && split_key(text).is_some()
}

// `|` or `>`, an optional chomping sign, an optional indentation digit.
fn is_block_header(text: &str) -> bool {
    let Some(rest) = text.strip_prefix(['|', '>']) else {
        return false;
    };
    rest.strip_prefix(['-', '+']).unwrap_or(rest).bytes().all(|b| b.is_ascii_digit())
}

// An apostrophe inside a word is a letter, not a quote: L'Hospitalet is one plain scalar.
fn is_quote_start(s: &[char], i: usize) -> bool {
    matches!(s[i], '"' | '\'') && (i == 0 || s[i - 1].is_whitespace() || ",[{:".contains(s[i - 1]))
}

/// Walks the text outside quotes, tracking bracket depth. Calls `stop(i, depth)` on every other
/// character and returns the first index where it says yes; otherwise, whether a quote was left
/// open and the final depth.
fn scan(
    s: &[char],
    mut stop: impl FnMut(usize, i32) -> bool,
) -> std::result::Result<usize, (bool, i32)> {
    let (mut quote, mut depth, mut i) = (None, 0, 0);
    while i < s.len() {
        let c = s[i];
        match quote {
            // Inside single quotes, two apostrophes are one.
            Some('\'') if c == '\'' && s.get(i + 1) == Some(&'\'') => i += 1,
            Some(q) if c == q => quote = None,
            Some('"') if c == '\\' => i += 1,
            Some(_) => {}
            None if is_quote_start(s, i) => quote = Some(c),
            None if c == '[' || c == '{' => depth += 1,
            None if c == ']' || c == '}' => depth -= 1,
            None if stop(i, depth) => return Ok(i),
            None => {}
        }
        i += 1;
    }
    Err((quote.is_some(), depth))
}

fn chars(text: &str) -> Vec<char> {
    text.chars().collect()
}

// True when every bracket and quote opened in the text is closed again.
fn flow_complete(text: &str) -> bool {
    scan(&chars(text), |_, _| false) == Err((false, 0))
}

// Finds the colon that ends a key: the first ": " or trailing ":" outside quotes and brackets.
// A comment before it means the line has no key.
fn split_key(text: &str) -> Option<(String, String)> {
    let s = chars(text);
    let colon = |i: usize| s[i] == ':' && s.get(i + 1).is_none_or(|&n| n == ' ');
    let comment = |i: usize| s[i] == '#' && i > 0 && s[i - 1].is_whitespace();
    let i = scan(&s, |i, depth| depth == 0 && (colon(i) || comment(i))).ok()?;
    let key: String = s[..i].iter().collect();
    let key = key.trim();
    if s[i] == '#' || key.is_empty() {
        return None;
    }
    let rest: String = s[i + 1..].iter().collect();
    Some((key.to_owned(), strip_comment(rest.trim())))
}

fn strip_comment(text: &str) -> String {
    let s = chars(text);
    let comment =
        |i: usize, depth| depth == 0 && s[i] == '#' && (i == 0 || s[i - 1].is_whitespace());
    match scan(&s, comment) {
        Ok(i) => s[..i].iter().collect::<String>().trim_end().to_owned(),
        Err(_) => text.to_owned(),
    }
}

// JavaScript assigns "__proto__" through the prototype setter, so the key would vanish in a JS
// consumer instead of rendering as a card like every other unknown key.
fn check_key(key: String, line: usize) -> Result<String> {
    if key == "__proto__" {
        return err("\"__proto__\" cannot be a key: it would disappear instead of rendering", line);
    }
    Ok(key)
}

fn unquote(text: &str, line: usize) -> Result<String> {
    let inner = || &text[1..text.len() - 1];
    if text.len() > 1 && text.starts_with('"') && text.ends_with('"') {
        return json_string(inner()).ok_or_else(|| YamlError {
            line,
            message: format!(
                "{text} has an escape this reader does not know. Inside double quotes a backslash starts \\n, \\t, \\\" or \\\\"
            ),
        });
    }
    if text.len() > 1 && text.starts_with('\'') && text.ends_with('\'') {
        return Ok(inner().replace("''", "'"));
    }
    Ok(text.to_owned())
}

// The inside of a double quoted scalar, read the way JSON reads a string.
fn json_string(inner: &str) -> Option<String> {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push(escape(&mut chars)?),
            '"' => return None,
            c if c < ' ' => return None,
            c => out.push(c),
        }
    }
    Some(out)
}

fn escape(chars: &mut std::str::Chars<'_>) -> Option<char> {
    Some(match chars.next()? {
        '"' => '"',
        '\\' => '\\',
        '/' => '/',
        'b' => '\u{8}',
        'f' => '\u{c}',
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        'u' => {
            let unit = hex4(&chars.take(4).collect::<String>())?;
            if !(0xD800..0xDC00).contains(&unit) {
                return char::from_u32(unit);
            }
            // A character outside the basic plane is written as two escapes, high then low.
            let pair: String = chars.take(6).collect();
            let low = pair.strip_prefix("\\u").and_then(hex4)?;
            let low = low.checked_sub(0xDC00).filter(|l| *l < 0x400)?;
            return char::from_u32(0x10000 + ((unit - 0xD800) << 10) + low);
        }
        _ => return None,
    })
}

fn hex4(digits: &str) -> Option<u32> {
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(digits, 16).ok()
}

fn scalar(text: &str, line: usize) -> Result<Value> {
    let t = strip_comment(text);
    let t = t.trim();
    if t.starts_with(['[', '{']) {
        return flow(t, line);
    }
    if t.starts_with(['"', '\'']) {
        if t.len() < 2 || !t.ends_with(&t[..1]) {
            return err("unterminated quoted string", line);
        }
        return unquote(t, line).map(Value::String);
    }
    Ok(if NULL.contains(&t) {
        Value::Null
    } else if TRUE.contains(&t) {
        Value::Bool(true)
    } else if FALSE.contains(&t) {
        Value::Bool(false)
    } else if is_number(t)
        && let Ok(n) = t.parse()
    {
        Value::Number(n)
    } else {
        Value::String(t.to_owned())
    })
}

// `[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?`. A leading zero is still a number, as in YAML 1.1.
fn is_number(t: &str) -> bool {
    let b = t.strip_prefix(['-', '+']).unwrap_or(t).as_bytes();
    let digits =
        |from: usize| b[from.min(b.len())..].iter().take_while(|c| c.is_ascii_digit()).count();
    let int = digits(0);
    let mut i = int;
    let mut frac = 0;
    if b.get(i) == Some(&b'.') {
        frac = digits(i + 1);
        i += 1 + frac;
    }
    if int + frac == 0 {
        return false;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1 + usize::from(matches!(b.get(i + 1), Some(b'-' | b'+')));
        let exp = digits(i);
        if exp == 0 {
            return false;
        }
        i += exp;
    }
    i == b.len()
}

// A flow collection: ["09:20", "text", {type: visit}]
fn flow(text: &str, line: usize) -> Result<Value> {
    let mut c = Flow { s: chars(text), i: 0, line, depth: 0 };
    let value = c.value()?;
    c.skip_space();
    match c.peek() {
        Some(ch) => err(format!("unexpected {} after a flow collection", found(Some(ch))), line),
        None => Ok(value),
    }
}

fn found(ch: Option<char>) -> String {
    quote(&ch.map_or("end of line".to_owned(), String::from))
}

struct Flow {
    s: Vec<char>,
    i: usize,
    line: usize,
    depth: usize,
}

impl Flow {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Result<Value> {
        self.skip_space();
        let open = self.peek();
        if !matches!(open, Some('[' | '{')) {
            return scalar(&self.plain(false), self.line);
        }
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return err(TOO_DEEP, self.line);
        }
        let value = if open == Some('[') { self.seq() } else { self.map() };
        self.depth -= 1;
        value
    }

    // Only reached on text whose brackets balance, so running off the end shows up as a wrong
    // character ("end of line") rather than needing its own message.
    fn seq(&mut self) -> Result<Value> {
        self.i += 1;
        let mut out = Vec::new();
        loop {
            self.skip_space();
            if self.peek() == Some(']') {
                self.i += 1;
                return Ok(Value::List(out));
            }
            out.push(self.value()?);
            self.skip_space();
            match self.peek() {
                Some(',') => self.i += 1,
                Some(']') => {
                    self.i += 1;
                    return Ok(Value::List(out));
                }
                other => {
                    let message = format!(
                        "expected \",\" or \"]\" in a flow sequence, found {}",
                        found(other)
                    );
                    return err(message, self.line);
                }
            }
        }
    }

    fn map(&mut self) -> Result<Value> {
        self.i += 1;
        let mut out = Entries::default();
        loop {
            self.skip_space();
            if self.peek() == Some('}') {
                self.i += 1;
                return Ok(out.done());
            }
            let key = check_key(unquote(self.plain(true).trim(), self.line)?, self.line)?;
            out.claim(&key, self.line)?;
            self.skip_space();
            let value = match self.peek() {
                // YAML allows a key with no value. It is almost always an unquoted value with a
                // comma in it, so it stays visible as a null for the validator to report.
                Some(',' | '}') => Value::Null,
                Some(':') => {
                    self.i += 1;
                    self.value()?
                }
                _ => {
                    let message =
                        format!("expected \":\" after the key {} in a flow mapping", quote(&key));
                    return err(message, self.line);
                }
            };
            out.push(key, value);
            self.skip_space();
            match self.peek() {
                Some(',') => self.i += 1,
                Some('}') => {
                    self.i += 1;
                    return Ok(out.done());
                }
                other => {
                    let message = format!(
                        "expected \",\" or \"}}\" in a flow mapping, found {}",
                        found(other)
                    );
                    return err(message, self.line);
                }
            }
        }
    }

    // A quoted or plain scalar, up to the next separator. An unclosed quote runs to the end and
    // the scalar reader refuses it.
    fn plain(&mut self, is_key: bool) -> String {
        self.skip_space();
        let start = self.i;
        match self.peek() {
            Some(q @ ('"' | '\'')) => {
                self.i += 1;
                while let Some(c) = self.peek() {
                    self.i += 1;
                    if c == q && q == '\'' && self.peek() == Some('\'') {
                        self.i += 1;
                    } else if c == q {
                        break;
                    }
                    if c == '\\' && q == '"' {
                        self.i += 1;
                    }
                }
            }
            _ => {
                while self.peek().is_some_and(|c| !",]}".contains(c) && !(is_key && c == ':')) {
                    self.i += 1;
                }
            }
        }
        self.s[start..self.i.min(self.s.len())].iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused(text: &str) -> YamlError {
        parse(text).unwrap_err()
    }

    fn read(text: &str) -> Value {
        parse(text).unwrap()
    }

    fn s(text: &str) -> Value {
        Value::String(text.to_owned())
    }

    fn num(n: f64) -> Value {
        Value::Number(n)
    }

    fn key<'a>(v: &'a Value, path: &[&str]) -> &'a Value {
        path.iter().fold(v, |v, k| v.get(k).unwrap())
    }

    fn assert_refused(text: &str, line: usize, message: &str) {
        let e = refused(text);
        assert_eq!(e.line, line, "{text:?}: {e}");
        assert!(e.message.contains(message), "{text:?}: {e}");
    }

    #[test]
    fn an_error_names_its_line() {
        let e = refused("a: 1\n\tb: 2\n");
        assert_eq!(e.to_string(), "line 2: tab used for indentation. YAML needs spaces");
        let boxed: Box<dyn std::error::Error> = Box::new(e);
        assert!(boxed.to_string().starts_with("line 2"));
    }

    #[test]
    fn tabs_for_indentation_are_refused() {
        assert_refused("a:\n\tb: 1\n", 2, "tab used");
        assert_refused("a:\n  \tb: 1\n", 2, "tab used");
    }

    #[test]
    fn a_second_document_is_refused() {
        assert_refused("a: 1\n---\na: 2\n", 2, "a second document starts here");
        assert_refused("---\na: 1\n---\n", 3, "second document");
    }

    #[test]
    fn content_after_the_end_of_the_document_is_refused() {
        assert_refused("a: 1\n...\nb: 2\n", 2, "the document ends here, but the file keeps going");
    }

    #[test]
    fn nesting_past_the_limit_is_refused_not_a_stack_overflow() {
        let indented = |line: &str, levels: usize| -> String {
            (0..levels).map(|i| format!("{}{line}\n", " ".repeat(i))).collect()
        };
        let flow = |open: &str, close: &str, levels: usize| {
            format!("k: {}{}", open.repeat(levels), close.repeat(levels))
        };
        for line in ["a:", "-"] {
            assert!(parse(&indented(line, 100)).is_ok());
            assert_refused(&indented(line, 101), 101, "nested deeper than 100 levels");
        }
        for (open, close) in [("[", "]"), ("{a: ", "}")] {
            assert!(parse(&flow(open, close, 100)).is_ok());
            assert_refused(&flow(open, close, 101), 1, "nested deeper than 100 levels");
        }
    }

    #[test]
    fn a_bad_escape_is_refused_with_its_line() {
        for text in ["k: \"a\\q\"\n", "\"a\\q\": 1\n", "k: {\"a\\q\": 1}\n"] {
            assert_refused(text, 1, "has an escape this reader does not know");
        }
    }

    #[test]
    fn a_double_quoted_scalar_reads_like_a_json_string() {
        let bad = [
            r#""a"b""#,
            r#""a\""#,
            r#""\x""#,
            r#""\u12""#,
            r#""\u12G4""#,
            r#""\uD800""#,
            r#""\uD800\n0000""#,
            r#""\uD800\u0041""#,
            r#""\uD800\uE000""#,
            r#""\uDC00""#,
            "\"a\u{1}b\"",
            "\"a\tb\"",
        ];
        for text in bad {
            assert_refused(&format!("k: {text}\n"), 1, "has an escape");
        }
        let v = read(r#"k: "\" \\ \/ \b \f \n \r \t \u00e9 \uD83D\uDE00""#);
        assert_eq!(v.get("k"), Some(&s("\" \\ / \u{8} \u{c} \n \r \t é 😀")));
    }

    #[test]
    fn an_error_deep_inside_is_the_error_of_the_file() {
        assert_refused("\tk: 1\n", 1, "tab used");
        assert_refused("-\n\tx\n", 2, "tab used");
        assert_refused("- a\n\tb\n", 2, "tab used");
        assert_refused("- a: 1\n  a: 2\n", 2, "duplicate key");
        assert_refused("- [a\n", 1, "never closed");
        assert_refused(r#"k: ["\q"]"#, 1, "has an escape");
        assert_refused(r#"k: {a: "\q"}"#, 1, "has an escape");
    }

    #[test]
    fn a_proto_key_is_refused_instead_of_disappearing() {
        for text in ["__proto__:\n  polluted: true\n", "k: {__proto__: {polluted: true}}\n"] {
            assert_refused(text, 1, "\"__proto__\" cannot be a key");
        }
    }

    #[test]
    fn a_duplicate_key_is_refused() {
        assert_refused("a: 1\nb: 2\na: 3\n", 3, "duplicate key \"a\"");
        assert_refused("k: {a: 1, a: 2}\n", 1, "duplicate key \"a\"");
        // The duplicate is the problem, not whatever its value holds.
        assert_refused("a: 1\na:\n  - [\n", 2, "duplicate key \"a\"");
        assert_refused("k: {a: 1, a: {b c}}\n", 1, "duplicate key \"a\"");
    }

    #[test]
    fn a_line_that_is_not_a_key_is_refused() {
        assert_refused("a: 1\n  b: 2\n", 2, "unexpected indentation");
        assert_refused("a: 1\nplain\n", 2, "expected \"key: value\", found \"plain\"");
        assert_refused(": x\n", 1, "expected \"key: value\"");
        assert_refused("a #b: c\n", 1, "expected \"key: value\"");
        assert_refused("a: 1\n- b\n", 2, "content left over after the document");
        assert_refused("k: |\n    x\n  y\n", 3, "unexpected indentation");
    }

    #[test]
    fn a_broken_quote_or_flow_collection_is_refused() {
        assert_refused("k: \"abc\n", 1, "unterminated quoted string");
        assert_refused("k: \"\n", 1, "unterminated quoted string");
        assert_refused("k: [a,\n  b\n", 1, "a flow collection was never closed");
        assert_refused("k: [a{]}\n", 1, "unexpected \"}\" after a flow collection");
        assert_refused("- [a]: b\n", 1, "unexpected \":\" after a flow collection");
        assert_refused(
            "k: [[a] b]\n",
            1,
            "expected \",\" or \"]\" in a flow sequence, found \"b\"",
        );
        assert_refused("k: {a]\n", 1, "expected \":\" after the key \"a\" in a flow mapping");
        assert_refused(
            "k: {a: [b] c}\n",
            1,
            "expected \",\" or \"}\" in a flow mapping, found \"c\"",
        );
    }

    #[test]
    fn nothing_is_null() {
        assert_eq!(read(""), Value::Null);
        assert_eq!(read("# only a comment\n\n"), Value::Null);
        assert_eq!(read("---\n"), Value::Null);
    }

    #[test]
    fn maps_nested_maps_and_a_sequence_of_maps() {
        let v = read(
            "people:
  - {id: rita, name: Rita, adult: true}
  - id: tomas
    name: Tomas
    adult: false
places:
  azulejo:
    name: Museu Nacional do Azulejo
    at: {lat: 38.7248, lon: -9.1139, radius_m: 120}
",
        );
        let people = key(&v, &["people"]).as_list().unwrap();
        assert_eq!(people.len(), 2);
        assert_eq!(people[1].get("name"), Some(&s("Tomas")));
        assert_eq!(people[1].get("adult"), Some(&Value::Bool(false)));
        assert_eq!(key(&v, &["places", "azulejo", "at", "lat"]), &num(38.7248));
        assert_eq!(key(&v, &["places", "azulejo", "at", "lon"]), &num(-9.1139));
    }

    #[test]
    fn a_block_is_a_list_of_a_time_a_text_and_an_optional_map() {
        let v = read(
            "blocks:
  - [\"\", \"Nothing before 10:00.\"]
  - [\"11:15\", \"The museum, top floor and come down.\",
     {type: visit, place: azulejo, until: \"13:00\"}]
",
        );
        let blocks = key(&v, &["blocks"]).as_list().unwrap();
        assert_eq!(blocks[0], Value::List(vec![s(""), s("Nothing before 10:00.")]));
        assert_eq!(blocks[1].as_list().unwrap().len(), 3);
        assert_eq!(blocks[1].as_list().unwrap()[2].get("until"), Some(&s("13:00")));
    }

    #[test]
    fn a_flow_list_wrapped_over_lines_is_one_list() {
        let v = read(
            "fixed: [[\"08:30\", \"Pick up the car.\"],

  # the evening
  [\"20:00\", \"Dinner, all four.\"]]
",
        );
        let fixed = key(&v, &["fixed"]).as_list().unwrap();
        assert_eq!(fixed.len(), 2);
        assert_eq!(fixed[1].as_list().unwrap()[0], s("20:00"));
    }

    #[test]
    fn plain_scalars_become_null_booleans_numbers_or_text() {
        let v = read(
            "date: 2026-04-11
time: 08:30
radius_m: 120
code: 07
price: 12.50
exp: -1.5e3
half: .5
plus: +3
dot_end: 1.
big: 2E+2
not_exp: 1e
not_exp_sign: 1e+
dot: .
dash: -
t: yes
f: Off
n: ~
empty:
word: null
",
        );
        let expect = [
            ("date", s("2026-04-11")),
            ("time", s("08:30")),
            ("radius_m", num(120.0)),
            ("code", num(7.0)),
            ("price", num(12.5)),
            ("exp", num(-1500.0)),
            ("half", num(0.5)),
            ("plus", num(3.0)),
            ("dot_end", num(1.0)),
            ("big", num(200.0)),
            ("not_exp", s("1e")),
            ("not_exp_sign", s("1e+")),
            ("dot", s(".")),
            ("dash", s("-")),
            ("t", Value::Bool(true)),
            ("f", Value::Bool(false)),
            ("n", Value::Null),
            ("empty", Value::Null),
            ("word", Value::Null),
        ];
        for (k, want) in expect {
            assert_eq!(v.get(k), Some(&want), "{k}");
        }
    }

    #[test]
    fn quoted_scalars_and_keys() {
        let v = read(
            "\"a b\": 'it''s'
'c': \"x # not a comment\"
name: L'Hospitalet
flow: [\"a\\\"b\", 'c''d', L'Hospitalet, 'it''s]']
",
        );
        assert_eq!(v.get("a b"), Some(&s("it's")));
        assert_eq!(v.get("c"), Some(&s("x # not a comment")));
        assert_eq!(v.get("name"), Some(&s("L'Hospitalet")));
        assert_eq!(
            v.get("flow"),
            Some(&Value::List(vec![s("a\"b"), s("c'd"), s("L'Hospitalet"), s("it's]")]))
        );
    }

    #[test]
    fn comments_after_values() {
        let v = read(
            "id: coast   # the slug\nlist: [a, b] # two\n- x # not this\n"
                .replace("- x # not this\n", "")
                .as_str(),
        );
        assert_eq!(v.get("id"), Some(&s("coast")));
        assert_eq!(v.get("list"), Some(&Value::List(vec![s("a"), s("b")])));
        let v = read("- x # a comment\n- # only a comment\n");
        assert_eq!(v, Value::List(vec![s("x"), Value::Null]));
    }

    #[test]
    fn flow_collections() {
        let v = read("k: {a, b: 1, c: [], d: {}, e: {f: [1, {g: h}]}}\n");
        assert_eq!(key(&v, &["k", "a"]), &Value::Null);
        assert_eq!(key(&v, &["k", "b"]), &num(1.0));
        assert_eq!(key(&v, &["k", "c"]), &Value::List(vec![]));
        assert_eq!(key(&v, &["k", "d"]), &Value::Map(Map::default()));
        assert_eq!(key(&v, &["k", "e", "f"]).as_list().unwrap()[1].get("g"), Some(&s("h")));
        assert_eq!(read("k: {a: 1}\n").get("k").unwrap().as_map().unwrap().keys().count(), 1);
    }

    #[test]
    fn keys_with_nothing_after_the_colon() {
        let v = read("detail:\nnested:\n  a: 1\nlist:\n- a\n- b\nlast:");
        assert_eq!(v.get("detail"), Some(&Value::Null));
        assert_eq!(key(&v, &["nested", "a"]), &num(1.0));
        assert_eq!(v.get("list"), Some(&Value::List(vec![s("a"), s("b")])));
        assert_eq!(v.get("last"), Some(&Value::Null));
    }

    #[test]
    fn sequences() {
        let v = read(
            "- date: 2026-04-11
  title: The tile museum
-
  title: The ferry back
-
- last
",
        );
        let items = v.as_list().unwrap();
        assert_eq!(items.len(), 4);
        assert_eq!(items[1].get("title"), Some(&s("The ferry back")));
        assert_eq!(items[2], Value::Null);
        assert_eq!(items[3], s("last"));
        assert_eq!(read("k:\n  - a\n  -\n"), read("k:\n  - a\n  - ~\n"));
    }

    #[test]
    fn literal_and_folded_blocks() {
        let v = read(
            "why: |
  three driving days follow
    and one indented line

  and the car goes back
folded: >-
  a
  b

  c
kept: |+
  x

forced: |2
     y
empty: |
list:
  - |
    in a list
end: 1
",
        );
        assert_eq!(
            v.get("why"),
            Some(&s(
                "three driving days follow\n  and one indented line\n\nand the car goes back\n"
            ))
        );
        assert_eq!(v.get("folded"), Some(&s("a b\nc")));
        assert_eq!(v.get("kept"), Some(&s("x\n")));
        assert_eq!(v.get("forced"), Some(&s("   y\n")));
        assert_eq!(v.get("empty"), Some(&s("\n")));
        assert_eq!(v.get("list"), Some(&Value::List(vec![s("in a list\n")])));
        assert_eq!(v.get("end"), Some(&num(1.0)));
    }

    #[test]
    fn document_markers_bom_and_line_endings() {
        assert_eq!(read("---\na: 1\n").get("a"), Some(&num(1.0)));
        assert_eq!(read("# c\n\n---\na: 1\n...\n# done\n\n").get("a"), Some(&num(1.0)));
        assert_eq!(read("\u{feff}a: 1\r\nb: 2\rc: 3").get("c"), Some(&num(3.0)));
    }
}
