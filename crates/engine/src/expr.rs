//! Expressions: what `when`, `if`, `derive` and every prop are written in. Parsed at load, never
//! run as code. The grammar is in docs/pack-format.md.

use std::borrow::Cow;
use std::fmt;

use crate::value::{Map, Value, quote, show, text, truthy};

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    /// A name and the keys under it: `place.at.lat`.
    Path(Vec<String>),
    Call(Func, Box<Expr>),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Compare(Op, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    Count,
    First,
    Last,
    Empty,
}

const FUNCS: [(&str, Func); 4] =
    [("count", Func::Count), ("first", Func::First), ("last", Func::Last), ("empty", Func::Empty)];

/// A problem at a column of the expression, counted in characters from 1. The caller knows the key
/// path it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExprError {
    pub column: usize,
    pub message: String,
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "column {}: {}", self.column, self.message)
    }
}

impl std::error::Error for ExprError {}

type Parsed<T> = Result<T, ExprError>;

fn fail<T>(column: usize, message: impl Into<String>) -> Parsed<T> {
    Err(ExprError { column, message: message.into() })
}

// A tree is never deeper than its token count, so this bounds the parser, `eval` and the drop,
// each of which recurses. Real expressions run to a dozen tokens.
const MAX_TOKENS: usize = 128;

pub fn parse(src: &str) -> Parsed<Expr> {
    let toks = lex(src)?;
    // The last token is the end, which does not count.
    if toks.len() > MAX_TOKENS + 1 {
        let message = format!("longer than {MAX_TOKENS} tokens. Split it with derive");
        return fail(toks[MAX_TOKENS].1, message);
    }
    let mut p = Parser { toks, pos: 0 };
    let expr = p.or()?;
    match p.next() {
        (Tok::End, _) => Ok(expr),
        (tok, col) => fail(col, format!("expected the end, found {}", tok.shown())),
    }
}

impl Expr {
    /// The value of the expression over the names in `scope`. A name or key that is not there is
    /// null, so a missing fact reads as false rather than stopping the screen.
    pub fn eval<'a>(&'a self, scope: &'a Map) -> Cow<'a, Value> {
        match self {
            Expr::Literal(v) => Cow::Borrowed(v),
            Expr::Path(path) => {
                let found = path[1..].iter().fold(scope.get(&path[0]), |v, k| v?.get(k));
                self::found(found)
            }
            Expr::Call(f, arg) => match arg.eval(scope) {
                Cow::Borrowed(v) => f.apply(v),
                Cow::Owned(v) => Cow::Owned(f.apply(&v).into_owned()),
            },
            Expr::Not(e) => Cow::Owned(Value::Bool(!truthy(Some(&e.eval(scope))))),
            Expr::And(a, b) => {
                let left = a.eval(scope);
                if truthy(Some(&left)) { b.eval(scope) } else { left }
            }
            Expr::Or(a, b) => {
                let left = a.eval(scope);
                if truthy(Some(&left)) { left } else { b.eval(scope) }
            }
            Expr::Compare(op, a, b) => {
                Cow::Owned(Value::Bool(op.holds(&a.eval(scope), &b.eval(scope))))
            }
        }
    }

    /// Calls `f` on this expression and every one inside it, outside in.
    pub fn visit<'a>(&'a self, f: &mut impl FnMut(&'a Expr)) {
        f(self);
        match self {
            Expr::Literal(_) | Expr::Path(_) => {}
            Expr::Call(_, e) | Expr::Not(e) => e.visit(f),
            Expr::And(a, b) | Expr::Or(a, b) | Expr::Compare(_, a, b) => {
                a.visit(f);
                b.visit(f);
            }
        }
    }

    /// The first name of every path: what the expression needs the scope to have.
    pub fn roots(&self) -> Vec<&str> {
        let mut out = Vec::new();
        self.visit(&mut |e| {
            if let Expr::Path(path) = e {
                out.push(path[0].as_str());
            }
        });
        out
    }
}

/// The column, counted in characters from 1, where `name` first stands as a whole name in `src`.
pub fn column_of(src: &str, name: &str) -> usize {
    let part = |c: char| c.is_alphanumeric() || c == '_' || c == '$' || c == '.';
    let whole = src.match_indices(name).find(|(i, _)| {
        let before = src[..*i].chars().next_back();
        let after = src[i + name.len()..].chars().next();
        !before.is_some_and(part) && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
    });
    whole.map_or(1, |(i, _)| src[..i].chars().count() + 1)
}

impl Op {
    fn holds(self, a: &Value, b: &Value) -> bool {
        let order = match (a, b) {
            (Value::Number(x), Value::Number(y)) => x.partial_cmp(y),
            // Dates and times are text in a pack, and in their shape text order is time order.
            (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
            _ => None,
        };
        match self {
            Op::Eq => a == b,
            Op::Ne => a != b,
            Op::Lt => order.is_some_and(|o| o.is_lt()),
            Op::Le => order.is_some_and(|o| o.is_le()),
            Op::Gt => order.is_some_and(|o| o.is_gt()),
            Op::Ge => order.is_some_and(|o| o.is_ge()),
            Op::In => match b {
                Value::List(items) => items.contains(a),
                Value::Map(m) => m.get(&text(Some(a))).is_some(),
                Value::String(s) => matches!(a, Value::String(x) if s.contains(x.as_str())),
                _ => false,
            },
        }
    }
}

impl Func {
    fn apply(self, v: &Value) -> Cow<'_, Value> {
        let len = match v {
            Value::List(items) => items.len(),
            Value::Map(m) => m.0.len(),
            Value::String(s) => s.chars().count(),
            _ => 0,
        };
        let list = v.as_list().unwrap_or_default();
        match self {
            Func::Count => Cow::Owned(Value::Number(len as f64)),
            Func::First => found(list.first()),
            Func::Last => found(list.last()),
            Func::Empty => Cow::Owned(Value::Bool(len == 0)),
        }
    }
}

/// A value that is there, borrowed, or null.
fn found(v: Option<&Value>) -> Cow<'_, Value> {
    v.map_or(Cow::Owned(Value::Null), Cow::Borrowed)
}

/// A piece of a text template: `"{block.time} · {place.name}"` is text, an expression, text, and
/// an expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text(String),
    Expr(Expr),
}

pub fn template(src: &str) -> Parsed<Vec<Part>> {
    let chars: Vec<char> = src.chars().collect();
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '{' {
            text.push(chars[i]);
            i += 1;
            continue;
        }
        // The closing brace is the first one outside a quoted text.
        let mut quoted = false;
        let close = chars[i + 1..].iter().position(|&c| {
            quoted ^= c == '\'';
            c == '}' && !quoted
        });
        let Some(close) = close else {
            // An unclosed quote hides the brace, and is the real problem.
            let rest: String = chars[i + 1..].iter().collect();
            lex(&rest).map_err(|e| ExprError { column: e.column + i + 1, ..e })?;
            return fail(i + 1, "a \"{\" that is never closed. A template reads \"{name}\"");
        };
        let inner: String = chars[i + 1..i + 1 + close].iter().collect();
        let expr = parse(&inner).map_err(|e| ExprError { column: e.column + i + 1, ..e })?;
        if !text.is_empty() {
            parts.push(Part::Text(std::mem::take(&mut text)));
        }
        parts.push(Part::Expr(expr));
        i += close + 2;
    }
    if !text.is_empty() {
        parts.push(Part::Text(text));
    }
    Ok(parts)
}

pub fn render(parts: &[Part], scope: &Map) -> String {
    parts
        .iter()
        .map(|p| match p {
            Part::Text(t) => t.clone(),
            Part::Expr(e) => text(Some(&e.eval(scope))),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Name(String),
    Sym(&'static str),
    End,
}

impl Tok {
    fn shown(&self) -> String {
        match self {
            Tok::Num(n) => show(Some(&Value::Number(*n))),
            Tok::Str(s) => format!("'{s}'"),
            Tok::Name(n) => quote(n),
            Tok::Sym(s) => quote(s),
            Tok::End => "the end".into(),
        }
    }
}

const SYMBOLS: [&str; 10] = ["==", "!=", "<=", ">=", "<", ">", "(", ")", ",", "."];

/// Tokens with the column each starts at.
fn lex(src: &str) -> Parsed<Vec<(Tok, usize)>> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (c, col) = (chars[i], i + 1);
        let rest: String = chars[i..chars.len().min(i + 2)].iter().collect();
        let word = |from: usize, ok: fn(char) -> bool| {
            from + chars[from..].iter().take_while(|&&c| ok(c)).count()
        };
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit()
            || (c == '-' && rest[1..].starts_with(|d: char| d.is_ascii_digit()))
        {
            let end = word(i + 1, |c| c.is_ascii_digit() || c == '.');
            let digits: String = chars[i..end].iter().collect();
            let Ok(n) = digits.parse() else {
                return fail(col, format!("{} is not a number", quote(&digits)));
            };
            toks.push((Tok::Num(n), col));
            i = end;
        } else if c == '\'' {
            // Inside a text, '' is a quote, as in YAML.
            let mut s = String::new();
            let mut j = i + 1;
            loop {
                match (chars.get(j), chars.get(j + 1)) {
                    (None, _) => return fail(col, "a text opened here is never closed"),
                    (Some('\''), Some('\'')) => {
                        s.push('\'');
                        j += 2;
                    }
                    (Some('\''), _) => break,
                    (Some(&c), _) => {
                        s.push(c);
                        j += 1;
                    }
                }
            }
            toks.push((Tok::Str(s), col));
            i = j + 1;
        } else if c.is_alphabetic() || c == '_' || c == '$' {
            let end = word(i + 1, |c| c.is_alphanumeric() || c == '_');
            toks.push((Tok::Name(chars[i..end].iter().collect()), col));
            i = end;
        } else if let Some(sym) = SYMBOLS.into_iter().find(|s| rest.starts_with(s)) {
            toks.push((Tok::Sym(sym), col));
            i += sym.len();
        } else {
            return fail(col, format!("{} is not part of an expression", quote(&c.to_string())));
        }
    }
    toks.push((Tok::End, chars.len() + 1));
    Ok(toks)
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    pos: usize,
}

impl Parser {
    fn next(&mut self) -> (Tok, usize) {
        let tok = self.toks[self.pos].clone();
        self.pos = (self.pos + 1).min(self.toks.len() - 1);
        tok
    }

    fn peek(&self) -> &Tok {
        &self.toks[self.pos].0
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        let found = self.peek() == tok;
        if found {
            self.next();
        }
        found
    }

    fn keyword(&mut self, word: &str) -> bool {
        self.eat(&Tok::Name(word.into()))
    }

    fn or(&mut self) -> Parsed<Expr> {
        let mut left = self.and()?;
        while self.keyword("or") {
            left = Expr::Or(Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }

    fn and(&mut self) -> Parsed<Expr> {
        let mut left = self.not()?;
        while self.keyword("and") {
            left = Expr::And(Box::new(left), Box::new(self.not()?));
        }
        Ok(left)
    }

    fn not(&mut self) -> Parsed<Expr> {
        if self.keyword("not") { Ok(Expr::Not(Box::new(self.not()?))) } else { self.compare() }
    }

    fn compare(&mut self) -> Parsed<Expr> {
        let left = self.value()?;
        let op = match self.peek() {
            Tok::Sym("==") => Op::Eq,
            Tok::Sym("!=") => Op::Ne,
            Tok::Sym("<") => Op::Lt,
            Tok::Sym("<=") => Op::Le,
            Tok::Sym(">") => Op::Gt,
            Tok::Sym(">=") => Op::Ge,
            Tok::Name(n) if n == "in" => Op::In,
            _ => return Ok(left),
        };
        self.next();
        Ok(Expr::Compare(op, Box::new(left), Box::new(self.value()?)))
    }

    fn value(&mut self) -> Parsed<Expr> {
        let (tok, col) = self.next();
        let name = match tok {
            Tok::Num(n) => return Ok(Expr::Literal(Value::Number(n))),
            Tok::Str(s) => return Ok(Expr::Literal(Value::String(s))),
            Tok::Sym("(") => {
                let inner = self.or()?;
                return self.close(inner);
            }
            Tok::Name(name) if !["and", "or", "not", "in"].contains(&name.as_str()) => name,
            other => return fail(col, format!("expected a value, found {}", other.shown())),
        };
        match name.as_str() {
            "true" => return Ok(Expr::Literal(Value::Bool(true))),
            "false" => return Ok(Expr::Literal(Value::Bool(false))),
            "null" => return Ok(Expr::Literal(Value::Null)),
            _ => {}
        }
        if self.eat(&Tok::Sym("(")) {
            let Some((_, f)) = FUNCS.into_iter().find(|(n, _)| *n == name) else {
                let names: Vec<&str> = FUNCS.iter().map(|(n, _)| *n).collect();
                return fail(
                    col,
                    format!("{} is not a function: {}", quote(&name), names.join(", ")),
                );
            };
            let arg = self.or()?;
            return self.close(Expr::Call(f, Box::new(arg)));
        }
        let mut path = vec![name];
        while self.eat(&Tok::Sym(".")) {
            match self.next() {
                (Tok::Name(key), _) => path.push(key),
                (other, col) => {
                    return fail(
                        col,
                        format!("expected a name after \".\", found {}", other.shown()),
                    );
                }
            }
        }
        Ok(Expr::Path(path))
    }

    fn close(&mut self, inner: Expr) -> Parsed<Expr> {
        match self.next() {
            (Tok::Sym(")"), _) => Ok(inner),
            (other, col) => fail(col, format!("expected \")\", found {}", other.shown())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml;

    fn err(src: &str) -> String {
        parse(src).unwrap_err().to_string()
    }

    fn scope() -> Map {
        let v = yaml::parse(
            "block: {time: '10:00', locked: true, tags: [a, b]}
place: {name: Museu, at: {lat: 38.7}}
days: [{date: 2026-04-11}, {date: 2026-04-12}]
n: 3
empty_list: []
nothing: ~",
        );
        v.unwrap().as_map().unwrap().clone()
    }

    fn eval(src: &str) -> Value {
        parse(src).unwrap().eval(&scope()).into_owned()
    }

    #[test]
    fn a_character_outside_the_grammar_is_refused_at_its_column() {
        assert_eq!(err("a = b"), "column 3: \"=\" is not part of an expression");
        assert_eq!(err("a ! b"), "column 3: \"!\" is not part of an expression");
        assert_eq!(err("#"), "column 1: \"#\" is not part of an expression");
        assert_eq!(err("1.2.3"), "column 1: \"1.2.3\" is not a number");
        assert_eq!(err("x == 'open"), "column 6: a text opened here is never closed");
    }

    #[test]
    fn a_misplaced_token_is_refused_at_its_column() {
        assert_eq!(err(""), "column 1: expected a value, found the end");
        assert_eq!(err("a and"), "column 6: expected a value, found the end");
        assert_eq!(err("not"), "column 4: expected a value, found the end");
        assert_eq!(err("a =="), "column 5: expected a value, found the end");
        assert_eq!(err("and"), "column 1: expected a value, found \"and\"");
        assert_eq!(err("a b"), "column 3: expected the end, found \"b\"");
        assert_eq!(err("a == b == c"), "column 8: expected the end, found \"==\"");
        assert_eq!(err("(a"), "column 3: expected \")\", found the end");
        assert_eq!(err("count(a, b)"), "column 8: expected \")\", found \",\"");
        assert_eq!(err("count()"), "column 7: expected a value, found \")\"");
        assert_eq!(err("a.1"), "column 3: expected a name after \".\", found 1");
        assert_eq!(err("a.'b'"), "column 3: expected a name after \".\", found 'b'");
        assert_eq!(err("a ,"), "column 3: expected the end, found \",\"");
        assert_eq!(
            err("explode(a)"),
            "column 1: \"explode\" is not a function: count, first, last, empty"
        );
        assert_eq!(err("(a or b or (c"), "column 14: expected \")\", found the end");
    }

    #[test]
    fn an_expression_past_the_token_limit_is_refused() {
        // 64 names and 63 "or"s is 127 tokens; one more pair is 129.
        let chain = |n: usize| vec!["a"; n].join(" or ");
        assert!(parse(&chain(64)).is_ok());
        assert!(parse(&format!("{}a{}", "(".repeat(63), ")".repeat(63))).is_ok());
        assert_eq!(err(&chain(65)), "column 321: longer than 128 tokens. Split it with derive");
        assert!(err(&"not ".repeat(500)).ends_with("longer than 128 tokens. Split it with derive"));
    }

    #[test]
    fn the_grammar_nests_or_over_and_over_not_over_compare() {
        let p = |s: &str| Box::new(Expr::Path(vec![s.into()]));
        assert_eq!(
            parse("not a and b or c").unwrap(),
            Expr::Or(Box::new(Expr::And(Box::new(Expr::Not(p("a"))), p("b"))), p("c"))
        );
        assert_eq!(
            parse("a and (b or c)").unwrap(),
            Expr::And(p("a"), Box::new(Expr::Or(p("b"), p("c"))))
        );
        assert_eq!(parse("not not a").unwrap(), Expr::Not(Box::new(Expr::Not(p("a")))));
        assert_eq!(parse(" $arg.x_1 ").unwrap(), Expr::Path(vec!["$arg".into(), "x_1".into()]));
        // A pack keeps its own language, keys included.
        assert_eq!(parse("día.año").unwrap(), Expr::Path(vec!["día".into(), "año".into()]));
    }

    #[test]
    fn literals() {
        assert_eq!(eval("true"), Value::Bool(true));
        assert_eq!(eval("false"), Value::Bool(false));
        assert_eq!(eval("null"), Value::Null);
        assert_eq!(eval("-2.5"), Value::Number(-2.5));
        assert_eq!(eval("'it''s'"), Value::String("it's".into()));
        assert_eq!(eval("''"), Value::String(String::new()));
    }

    #[test]
    fn a_path_that_is_not_there_is_null() {
        assert_eq!(eval("place.at.lat"), Value::Number(38.7));
        assert_eq!(eval("place.at.lon"), Value::Null);
        assert_eq!(eval("nowhere.at"), Value::Null);
        assert_eq!(eval("n.x"), Value::Null);
    }

    #[test]
    fn and_or_and_not_follow_truthiness() {
        assert_eq!(eval("nothing or 'fallback'"), Value::String("fallback".into()));
        assert_eq!(eval("place.name or 'fallback'"), Value::String("Museu".into()));
        assert_eq!(eval("n and place.name"), Value::String("Museu".into()));
        assert_eq!(eval("nothing and place.name"), Value::Null);
        assert_eq!(eval("not nothing"), Value::Bool(true));
        assert_eq!(eval("not block.locked"), Value::Bool(false));
    }

    #[test]
    fn comparisons_order_numbers_and_text_and_nothing_else() {
        let yes = |s: &str| assert_eq!(eval(s), Value::Bool(true), "{s}");
        let no = |s: &str| assert_eq!(eval(s), Value::Bool(false), "{s}");
        for s in ["n == 3", "n != 4", "n < 4", "n <= 3", "n > 2", "n >= 3", "block.time < '10:30'"]
        {
            yes(s);
        }
        for s in
            ["n == '3'", "n != 3", "n < 3", "n <= 2", "n > 3", "n >= 4", "n < '4'", "nothing < 1"]
        {
            no(s);
        }
    }

    #[test]
    fn in_looks_inside_a_list_a_map_or_a_text() {
        assert_eq!(eval("'a' in block.tags"), Value::Bool(true));
        assert_eq!(eval("'c' in block.tags"), Value::Bool(false));
        assert_eq!(eval("'name' in place"), Value::Bool(true));
        assert_eq!(eval("'zip' in place"), Value::Bool(false));
        assert_eq!(eval("'use' in 'Museu'"), Value::Bool(true));
        assert_eq!(eval("1 in 'Museu'"), Value::Bool(false));
        assert_eq!(eval("1 in n"), Value::Bool(false));
    }

    #[test]
    fn the_four_functions() {
        assert_eq!(eval("count(days)"), Value::Number(2.0));
        assert_eq!(eval("count(place)"), Value::Number(2.0));
        assert_eq!(eval("count('Museu')"), Value::Number(5.0));
        assert_eq!(eval("count(n)"), Value::Number(0.0));
        assert_eq!(eval("count(first(days))"), Value::Number(1.0));
        assert_eq!(eval("first(block.tags)"), Value::String("a".into()));
        assert_eq!(eval("last(block.tags)"), Value::String("b".into()));
        assert_eq!(eval("last(empty_list)"), Value::Null);
        assert_eq!(eval("first(n)"), Value::Null);
        assert_eq!(eval("empty(empty_list)"), Value::Bool(true));
        assert_eq!(eval("empty(nothing)"), Value::Bool(true));
        assert_eq!(eval("empty(days)"), Value::Bool(false));
        // A function of a computed value.
        assert_eq!(eval("count(count(days))"), Value::Number(0.0));
    }

    #[test]
    fn the_roots_are_the_first_name_of_every_path() {
        let e = parse("not count(a.b) and (c or 'x' < d.e) or true").unwrap();
        assert_eq!(e.roots(), ["a", "c", "d"]);
        assert!(parse("1 == 2").unwrap().roots().is_empty());
    }

    #[test]
    fn a_name_is_found_where_it_stands_whole() {
        assert_eq!(column_of("place.day or day", "day"), 14);
        assert_eq!(column_of("days or day", "day"), 9);
        assert_eq!(column_of("día or x", "x"), 8);
        assert_eq!(column_of("$day or day", "day"), 9);
        assert_eq!(column_of("a", "b"), 1);
    }

    #[test]
    fn a_template_is_text_with_expressions_in_braces() {
        assert_eq!(
            template("{place.name").unwrap_err().to_string(),
            "column 1: a \"{\" that is never closed. A template reads \"{name}\""
        );
        assert_eq!(
            template("a {x == 'open}").unwrap_err().to_string(),
            "column 9: a text opened here is never closed"
        );
        assert_eq!(
            template("at {a b}").unwrap_err().to_string(),
            "column 7: expected the end, found \"b\""
        );
        let parts = template("{block.time} · {place.name or '}'}!").unwrap();
        assert_eq!(parts.len(), 4);
        assert_eq!(render(&parts, &scope()), "10:00 · Museu!");
        assert_eq!(render(&template("{nothing}").unwrap(), &scope()), "");
        assert_eq!(render(&template("} plain").unwrap(), &scope()), "} plain");
        assert_eq!(template("").unwrap(), []);
    }
}
