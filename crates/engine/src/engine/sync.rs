//! Data sync (decision 0022). The engine says what to fetch and reads what came back; the host
//! only makes the HTTPS GET, with the secret it keeps, and stores the facts it is handed.

use super::{Engine, World, check};
use crate::clock::later;
use std::fmt::Write;

use crate::define::{FLOOR, Fetch, Module, key};
use crate::value::{Map, Value, quote, text};
use crate::yaml;

/// A fetch for the host: an HTTPS GET of `url`. When `secret` is not empty, the host adds the
/// value it keeps under that name as the query parameter `param`.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub module: String,
    pub url: String,
    pub secret: String,
    pub param: String,
    /// Set on every row read from the reply; the host hands it back untouched.
    pub tag: Map,
}

/// A query value, percent encoded: everything but letters, digits and `-._~`.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

/// What `path` finds in `v`. On a list it goes on into every item, so `days.max` over a list of
/// days is the list of their maxima.
fn at(v: &Value, path: &[String]) -> Value {
    let Some((first, rest)) = path.split_first() else {
        return v.clone();
    };
    match v {
        Value::Map(m) => m.get(first).map_or(Value::Null, |v| at(v, rest)),
        Value::List(items) => Value::List(items.iter().map(|i| at(i, path)).collect()),
        _ => Value::Null,
    }
}

/// The rows of a reply. A path that finds a list gives one value per row, in order, and one that
/// finds anything else gives it to every row; a row with nothing read is dropped.
fn rows(sync: &Fetch, body: &str, tag: &Map) -> Result<Vec<Value>, String> {
    let reply = yaml::parse(body).map_err(|_| "the reply is not JSON".to_owned())?;
    let columns: Vec<(&str, Value)> =
        sync.read.iter().map(|(k, path)| (k.as_str(), at(&reply, path))).collect();
    let n = columns.iter().map(|(_, v)| v.as_list().map_or(1, <[Value]>::len)).max();
    let mut out = Vec::new();
    for i in 0..n.unwrap_or_default() {
        let mut row = Map::default();
        for (k, v) in &columns {
            let v = v.as_list().map_or(Some(v), |l| l.get(i));
            if let Some(v) = v.filter(|v| **v != Value::Null) {
                row.set(k, v.clone());
            }
        }
        if row.0.is_empty() {
            continue;
        }
        for (k, v) in tag.iter() {
            row.set(k, v.clone());
        }
        out.push(Value::Map(row));
    }
    if out.is_empty() {
        return Err("the reply had nothing to read".into());
    }
    Ok(out)
}

impl Engine {
    /// Every host the pack's syncs reach, once each, for the person to approve.
    pub fn hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> = self.syncs().map(|(_, s)| s.host().to_owned()).collect();
        hosts.sort();
        hosts.dedup();
        hosts
    }

    fn syncs(&self) -> impl Iterator<Item = (&Module, &Fetch)> {
        self.def.modules.iter().filter_map(|m| Some((m, m.sync.as_ref()?)))
    }

    /// When an automatic sync of `module` is next due: `every` after the last good one, and never
    /// sooner than [`FLOOR`] minutes after the last try. Empty when it never ran, which is due now.
    pub(super) fn due(world: &World, module: &str, every: u32) -> String {
        let state = world.store.get(&key(module));
        let stamp = |k: &str| text(state.and_then(|s| s.get(k)));
        let tried = stamp("tried");
        if tried.len() != 16 {
            return String::new();
        }
        let next = later(&tried, FLOOR);
        let as_of = stamp("as_of");
        if as_of.len() != 16 {
            return next;
        }
        next.max(later(&as_of, every))
    }

    /// What to fetch now: `module` alone when named, as an action asks; else every automatic
    /// sync that is due. A request whose query has a value missing, say with no place, is left out.
    pub fn requests(&self, world: &World, module: &str) -> Result<Vec<Request>, String> {
        check(&world.now)?;
        let named = self.syncs().any(|(m, _)| m.name == module);
        if !module.is_empty() && !named {
            return Err(format!("{} does not sync", quote(module)));
        }
        let wanted = |m: &Module, s: &Fetch| {
            if module.is_empty() {
                s.every.is_some_and(|every| Self::due(world, m.name, every) <= world.now)
            } else {
                m.name == module
            }
        };
        let wanted: Vec<(&Module, &Fetch)> = self.syncs().filter(|(m, s)| wanted(m, s)).collect();
        if wanted.is_empty() {
            return Ok(Vec::new());
        }
        let scope = self.run(world).scope;
        let mut out = Vec::new();
        'each: for (m, s) in wanted {
            let mut url = s.url.clone();
            for (name, e) in &s.query {
                let v = e.eval(&scope);
                if matches!(*v, Value::Null | Value::List(_) | Value::Map(_)) {
                    continue 'each;
                }
                let sep = if url.contains('?') { '&' } else { '?' };
                let _ = write!(url, "{sep}{}={}", encode(name), encode(&text(Some(&v))));
            }
            let (secret, param) = s.secret.clone().unwrap_or_default();
            let tag = super::values(&s.tag, &scope);
            out.push(Request { module: m.name.to_owned(), url, secret, param, tag });
        }
        Ok(out)
    }

    /// The facts to store after a fetch. `status` is the HTTP status, or 0 when there was no
    /// answer, with `body` then saying why. A failure keeps the last good rows and says why.
    pub fn received(
        &self,
        world: &World,
        request: &Request,
        status: u16,
        body: &str,
    ) -> Result<Map, String> {
        check(&world.now)?;
        let sync = self.syncs().find(|(m, _)| m.name == request.module).map(|(_, s)| s);
        let sync = sync.ok_or_else(|| format!("{} does not sync", quote(&request.module)))?;
        let read = match status {
            0 => Err(if body.is_empty() { "no answer".to_owned() } else { body.to_owned() }),
            200..=299 => rows(sync, body, &request.tag),
            _ => Err(format!("the server answered {status}")),
        };
        let key = key(&request.module);
        let old = |k: &str| world.store.get(&key).and_then(|s| s.get(k)).cloned();
        let now = Value::String(world.now.clone());
        let (as_of, rows, failed) = match read {
            Ok(rows) => (now.clone(), Value::List(rows), Value::Null),
            Err(why) => (
                old("as_of").unwrap_or(Value::Null),
                old("rows").unwrap_or(Value::List(Vec::new())),
                Value::String(why),
            ),
        };
        let state = Map(vec![
            ("as_of".into(), as_of),
            ("tried".into(), now),
            ("failed".into(), failed),
            ("rows".into(), rows),
        ]);
        Ok(Map(vec![(key, Value::Map(state))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::Pack;
    use crate::yaml::parse;

    const MANIFEST: &str = r#"pack: {id: t, name: T, language: en, timezone: UTC, content: c.yaml}
modules:
  timeline:
  places:
  climate:
    from: climate
    sync:
      trigger: auto
      every: 6h
      request:
        url: "https://api.example.org/forecast?units=metric"
        query: {lat: place.at.lat, lon: place.at.lon, day: now.date, name: place.name}
        secret: {name: forecast_key, param: key}
      tag: {place: place.id}
      read: {date: daily.time, high: daily.max, summary: note}
screens: {a: {}}
rules: [{screen: a}]
"#;

    const CONTENT: &str = r#"days:
  - date: 2026-04-11
    title: T
    blocks: [["10:00", "Museum", {place: m}], ["12:00", "Nowhere", {place: n}]]
places:
  m: {name: "Museu & co", at: {lat: 38.5, lon: -9.25}}
  n: {name: Nowhere}
"#;

    fn engine(manifest: &str) -> Engine {
        let pack = Pack {
            manifest: parse(manifest).unwrap(),
            content: parse(CONTENT).unwrap(),
            theme: None,
        };
        Engine::load(pack).unwrap().0
    }

    fn world(now: &str, store: &str) -> World {
        let store = parse(store).unwrap().as_map().cloned().unwrap_or_default();
        World { now: now.into(), store, ..World::default() }
    }

    #[test]
    fn a_bad_clock_or_a_module_that_does_not_sync_is_refused() {
        let e = engine(MANIFEST);
        let w = world("2026-04-11T10:00", "");
        assert!(e.requests(&world("x", ""), "").is_err());
        assert_eq!(e.requests(&w, "alerts").unwrap_err(), "\"alerts\" does not sync");
        let r = &e.requests(&w, "climate").unwrap()[0];
        let stranger = Request { module: "alerts".into(), ..r.clone() };
        assert_eq!(e.received(&w, &stranger, 200, "{}").unwrap_err(), "\"alerts\" does not sync");
        assert!(e.received(&world("x", ""), r, 200, "{}").is_err());
    }

    #[test]
    fn a_query_value_is_percent_encoded() {
        assert_eq!(encode("Museu & co/é~"), "Museu%20%26%20co%2F%C3%A9~");
    }

    #[test]
    fn the_hosts_are_listed_once_and_a_pack_with_no_sync_has_none() {
        assert_eq!(engine(MANIFEST).hosts(), ["api.example.org"]);
        let none = MANIFEST.split("    sync:").next().unwrap().to_owned()
            + "screens: {a: {}}\nrules: [{screen: a}]\n";
        assert!(engine(&none).hosts().is_empty());
    }

    #[test]
    fn an_auto_sync_is_due_until_it_runs_then_every_after_the_last_good_one() {
        let e = engine(MANIFEST);
        let asks = |now: &str, store: &str| e.requests(&world(now, store), "").unwrap().len();
        assert_eq!(asks("2026-04-11T10:00", ""), 1);
        let good = "sync.climate: {as_of: 2026-04-11T05:00, tried: 2026-04-11T05:00}";
        assert_eq!(asks("2026-04-11T10:59", good), 0);
        assert_eq!(asks("2026-04-11T11:00", good), 1);
        // The screen is asked for again when the sync falls due, so the host fetches then.
        let until = e.decide(&world("2026-04-11T10:30", good)).unwrap().watch.until;
        assert_eq!(until, "2026-04-11T11:00");
        let failed = "sync.climate: {as_of: null, tried: 2026-04-11T10:00}";
        assert_eq!(asks("2026-04-11T10:14", failed), 0);
        assert_eq!(asks("2026-04-11T10:15", failed), 1);
        let button = MANIFEST.replace("trigger: auto\n      every: 6h", "trigger: button");
        assert_eq!(engine(&button).requests(&world("2026-04-11T10:00", ""), "").unwrap(), []);
        let until = engine(&button).decide(&world("2026-04-11T10:30", good)).unwrap().watch.until;
        assert_ne!(until, "2026-04-11T11:00", "a button sync is never due");
    }

    #[test]
    fn a_request_carries_its_query_its_secret_and_its_tag() {
        let e = engine(MANIFEST);
        let good = "sync.climate: {as_of: 2026-04-11T05:00, tried: 2026-04-11T05:00}";
        let r = e.requests(&world("2026-04-11T10:30", good), "climate").unwrap();
        let tag = Map(vec![("place".into(), Value::String("m".into()))]);
        let expected = Request {
            module: "climate".into(),
            url: "https://api.example.org/forecast?units=metric&lat=38.5&lon=-9.25&day=2026-04-11&name=Museu%20%26%20co".into(),
            secret: "forecast_key".into(),
            param: "key".into(),
            tag,
        };
        assert_eq!(r, [expected]);
        // At a place with no position there is nothing to ask for.
        assert_eq!(e.requests(&world("2026-04-11T12:30", ""), "climate").unwrap(), []);
        let bare = MANIFEST
            .replace("?units=metric", "")
            .replace("        secret: {name: forecast_key, param: key}\n", "");
        let r = &engine(&bare).requests(&world("2026-04-11T10:30", ""), "").unwrap()[0];
        assert!(r.url.starts_with("https://api.example.org/forecast?lat=38.5&"), "{}", r.url);
        assert_eq!((r.secret.as_str(), r.param.as_str()), ("", ""));
    }

    #[test]
    fn a_reply_is_read_into_rows_by_column_or_by_item() {
        let e = engine(MANIFEST);
        let w = world("2026-04-11T10:30", "");
        let r = &e.requests(&w, "climate").unwrap()[0];
        let columns = r#"{"daily": {"time": ["2026-04-11", "2026-04-12", "2026-04-13"], "max": [21.5, null]}, "note": "Sunny"}"#;
        let facts = e.received(&w, r, 200, columns).unwrap();
        assert_eq!(
            crate::value::show(facts.get("sync.climate")),
            r#"{"as_of": "2026-04-11T10:30", "tried": "2026-04-11T10:30", "failed": null, "rows": [{"date": "2026-04-11", "high": 21.5, "summary": "Sunny", "place": "m"}, {"date": "2026-04-12", "summary": "Sunny", "place": "m"}, {"date": "2026-04-13", "summary": "Sunny", "place": "m"}]}"#
        );
        let items = r#"{"daily": [{"time": "2026-04-11", "max": 20}, {"max": 19}]}"#;
        let facts = e.received(&w, r, 200, items).unwrap();
        let rows = facts.get("sync.climate").and_then(|s| s.get("rows")).and_then(Value::as_list);
        assert_eq!(rows.map(<[Value]>::len), Some(2));
    }

    #[test]
    fn a_failed_sync_keeps_the_last_good_rows_and_says_why() {
        let e = engine(MANIFEST);
        let before =
            "sync.climate: {as_of: 2026-04-11T04:00, tried: 2026-04-11T04:00, rows: [{high: 1}]}";
        let w = world("2026-04-11T10:30", before);
        let r = &e.requests(&w, "climate").unwrap()[0];
        let failed = |status: u16, body: &str| {
            let facts = e.received(&w, r, status, body).unwrap();
            let state = facts.get("sync.climate").cloned().unwrap();
            assert_eq!(state.get("as_of"), Some(&Value::String("2026-04-11T04:00".into())));
            assert_eq!(state.get("tried"), Some(&Value::String("2026-04-11T10:30".into())));
            assert_eq!(crate::value::show(state.get("rows")), r#"[{"high": 1}]"#);
            text(state.get("failed"))
        };
        assert_eq!(failed(0, ""), "no answer");
        assert_eq!(failed(0, "offline"), "offline");
        assert_eq!(failed(500, "{}"), "the server answered 500");
        assert_eq!(failed(200, "{"), "the reply is not JSON");
        assert_eq!(failed(200, r#"{"daily": {"max": [null]}}"#), "the reply had nothing to read");
        assert_eq!(failed(200, r#"{"daily": 3}"#), "the reply had nothing to read");
        let first = world("2026-04-11T10:30", "");
        let facts = e.received(&first, r, 404, "").unwrap();
        assert_eq!(
            crate::value::show(facts.get("sync.climate")),
            r#"{"as_of": null, "tried": "2026-04-11T10:30", "failed": "the server answered 404", "rows": []}"#
        );
    }
}
