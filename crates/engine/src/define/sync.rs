//! A module's `sync`: a source the host fetches besides the pack, and when (decision 0022).

use super::Definer;
use crate::expr::Expr;
use crate::validate::patterns::is_id;
use crate::value::{Map, Value, quote, text};

/// The keys of a synced climate row, as `read` and `tag` name them.
pub const ROW: [&str; 9] =
    ["place", "date", "month", "high", "low", "rain", "sunrise", "sunset", "summary"];

/// The fewest minutes between two automatic syncs, so a pack cannot flood a server.
pub const FLOOR: u32 = 15;

/// The stored fact that holds a module's last sync.
pub(crate) fn key(module: &str) -> String {
    format!("sync.{module}")
}

/// A module's `sync`: what it fetches, and how often.
#[derive(Debug, Clone, PartialEq)]
pub struct Fetch {
    /// Minutes between automatic syncs; None when only an action syncs.
    pub every: Option<u32>,
    /// An `https://` address, before its query.
    pub url: String,
    pub query: Vec<(String, Expr)>,
    /// The secret's name, and the query parameter its value goes in.
    pub secret: Option<(String, String)>,
    /// Keys set on every row read, from the scope when the request is made.
    pub tag: Vec<(String, Expr)>,
    /// Each row key, and the path in the reply it comes from.
    pub read: Vec<(String, Vec<String>)>,
}

impl Fetch {
    /// The host part of `url`, the one the person approves.
    pub fn host(&self) -> &str {
        host(&self.url)
    }
}

fn host(url: &str) -> &str {
    let rest = url.strip_prefix("https://").unwrap_or_default();
    rest.split(['/', '?', '#']).next().unwrap_or_default()
}

/// `6h` as 360: a whole number of minutes, hours or days.
fn period(every: &str) -> Option<u32> {
    let unit = every.chars().last()?;
    let n = &every[..every.len() - unit.len_utf8()];
    let per = match unit {
        'm' => 1,
        'h' => 60,
        'd' => 24 * 60,
        _ => return None,
    };
    n.parse::<u32>().ok()?.checked_mul(per)
}

impl Definer {
    /// The `sync` at `at`, or None when it has an error, which the report then holds.
    pub(super) fn sync(&mut self, at: &str, v: &Value) -> Option<Fetch> {
        let before = self.r.errors.len();
        let Some(m) = v.as_map() else {
            self.r.error(at, "has to be a mapping: trigger, every, request, tag, read");
            return None;
        };
        self.only(at, m, "a sync", &["trigger", "every", "request", "tag", "read"]);
        let every = match (text(m.get("trigger")).as_str(), m.get("every")) {
            ("button", None) => None,
            ("button", Some(_)) => {
                self.r.error(format!("{at}.every"), "only an auto sync has an every");
                None
            }
            ("auto", every) => {
                let minutes = every.and_then(|e| period(&text(Some(e)))).filter(|m| *m >= FLOOR);
                if minutes.is_none() {
                    let message =
                        format!("an auto sync needs every: at least {FLOOR}m, like 30m, 6h or 1d");
                    self.r.error(format!("{at}.every"), message);
                }
                minutes
            }
            _ => {
                self.r.error(format!("{at}.trigger"), "has to be button or auto");
                None
            }
        };
        let empty = Map::default();
        let request = m.get("request").and_then(Value::as_map).unwrap_or_else(|| {
            let message = "has to be a mapping: url, and optionally query and secret";
            self.r.error(format!("{at}.request"), message);
            &empty
        });
        let r = format!("{at}.request");
        self.only(&r, request, "a request", &["url", "query", "secret"]);
        let url = text(request.get("url"));
        // A plain host, so the one the person approves is the one reached: `a.org@b.io` goes to b.io.
        let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':');
        if host(&url).is_empty() || !host(&url).chars().all(plain) {
            self.r.error(format!("{r}.url"), "has to be an https:// address with a host");
        }
        let query = self.exprs(&format!("{r}.query"), request.get("query"), None);
        let secret = request.get("secret").and_then(|s| {
            let (name, param) = (text(s.get("name")), text(s.get("param")));
            if s.as_map().is_some_and(|m| m.0.len() == 2) && is_id(&name) && !param.is_empty() {
                return Some((name, param));
            }
            let message = "has to be {name, param}: the secret's name, and the query parameter its value goes in";
            self.r.error(format!("{r}.secret"), message);
            None
        });
        let tag = self.exprs(&format!("{at}.tag"), m.get("tag"), Some(&ROW));
        let read = self.read(&format!("{at}.read"), m.get("read"));
        (self.r.errors.len() == before).then_some(Fetch { every, url, query, secret, tag, read })
    }

    /// A mapping of name to expression, the names among `keys` when given.
    fn exprs(&mut self, at: &str, v: Option<&Value>, keys: Option<&[&str]>) -> Vec<(String, Expr)> {
        let Some(v) = v else {
            return Vec::new();
        };
        let Some(m) = v.as_map() else {
            self.r.error(at, "has to be a mapping of name to expression");
            return Vec::new();
        };
        if let Some(keys) = keys {
            self.only(at, m, "a row", keys);
        }
        let mut out = Vec::new();
        for (k, src) in m.iter() {
            if let Some(e) = self.expr(&format!("{at}.{k}"), src) {
                out.push((k.to_owned(), e));
            }
        }
        out
    }

    fn read(&mut self, at: &str, v: Option<&Value>) -> Vec<(String, Vec<String>)> {
        let Some(m) = v.and_then(Value::as_map).filter(|m| !m.0.is_empty()) else {
            self.r.error(at, "has to map row keys to paths in the reply, like high: daily.max");
            return Vec::new();
        };
        self.only(at, m, "a row", &ROW);
        let mut read = Vec::new();
        for (key, path) in m.iter() {
            let path = text(Some(path));
            let keys: Vec<String> = path.split('.').map(str::to_owned).collect();
            if keys.iter().any(String::is_empty) {
                let message = format!("{} is not a path, like daily.max", quote(&path));
                self.r.error(format!("{at}.{key}"), message);
            }
            read.push((key.to_owned(), keys));
        }
        read
    }
}

#[cfg(test)]
mod tests {
    use crate::define::define;
    use crate::yaml::parse;

    fn said(sync: &str) -> String {
        let manifest = format!("modules:\n  places:\n  climate:\n    sync: {sync}\n");
        let (_, r) = define(&parse(&manifest).unwrap());
        let found = r.errors.iter().map(|f| format!("{}: {}", f.at, f.message));
        found.collect::<Vec<_>>().join("\n")
    }

    const REQUEST: &str = "request: {url: 'https://api.example.org/f'}, read: {high: d.max}";

    #[test]
    fn a_sync_says_when_where_and_what_to_read() {
        let at = "modules.climate.sync";
        assert_eq!(
            said("x"),
            format!("{at}: has to be a mapping: trigger, every, request, tag, read")
        );
        let all = said("{when: now}");
        assert!(all.contains(&format!("{at}.when: \"when\" is not part of a sync")), "{all}");
        assert!(all.contains(&format!("{at}.trigger: has to be button or auto")), "{all}");
        assert!(all.contains(&format!("{at}.request: has to be a mapping")), "{all}");
        assert!(all.contains(&format!("{at}.read: has to map row keys")), "{all}");
        let every =
            format!("{at}.every: an auto sync needs every: at least 15m, like 30m, 6h or 1d");
        for bad in [
            "",
            ", every: ''",
            ", every: 10m",
            ", every: 6",
            ", every: xh",
            ", every: h",
            ", every: 9999999999d",
            ", every: 6é",
        ] {
            assert_eq!(said(&format!("{{trigger: auto{bad}, {REQUEST}}}")), every, "{bad}");
        }
        assert_eq!(
            said(&format!("{{trigger: button, every: 6h, {REQUEST}}}")),
            format!("{at}.every: only an auto sync has an every")
        );
        for ok in ["trigger: auto, every: 15m", "trigger: auto, every: 1d", "trigger: button"] {
            assert_eq!(said(&format!("{{{ok}, {REQUEST}}}")), "", "{ok}");
        }
    }

    #[test]
    fn a_request_is_an_https_address_with_expressions_and_a_named_secret() {
        let r = "modules.climate.sync.request";
        let with = |request: &str| {
            said(&format!("{{trigger: button, request: {request}, read: {{high: d.max}}}}"))
        };
        assert_eq!(
            with("{url: 'http://a.org'}"),
            format!("{r}.url: has to be an https:// address with a host")
        );
        assert_eq!(
            with("{url: 'https://'}"),
            format!("{r}.url: has to be an https:// address with a host")
        );
        assert_eq!(
            with("{url: 'https://a.org@b.io/f'}"),
            format!("{r}.url: has to be an https:// address with a host")
        );
        assert_eq!(
            with("{url: 'https://a.org', port: 1}"),
            format!("{r}.port: \"port\" is not part of a request: url, query, secret")
        );
        assert_eq!(
            with("{url: 'https://a.org', query: x}"),
            format!("{r}.query: has to be a mapping of name to expression")
        );
        let unknown = with("{url: 'https://a.org', query: {lat: nowhere.lat}}");
        assert!(
            unknown
                .starts_with(&format!("{r}.query.lat: column 1: \"nowhere\" is not a name here")),
            "{unknown}"
        );
        let secret = format!(
            "{r}.secret: has to be {{name, param}}: the secret's name, and the query parameter its value goes in"
        );
        for bad in [
            "key",
            "{name: key}",
            "{name: Key, param: k}",
            "{name: key, param: ''}",
            "{name: key, param: k, x: 1}",
        ] {
            assert_eq!(with(&format!("{{url: 'https://a.org', secret: {bad}}}")), secret, "{bad}");
        }
        assert_eq!(
            with("{url: 'https://a.org', query: {lat: place.lat}, secret: {name: key, param: k}}"),
            ""
        );
    }

    #[test]
    fn tag_and_read_name_row_keys() {
        let at = "modules.climate.sync";
        let with = |rest: &str| {
            said(&format!("{{trigger: button, request: {{url: 'https://a.org'}}, {rest}}}"))
        };
        assert_eq!(
            with("read: {}"),
            format!("{at}.read: has to map row keys to paths in the reply, like high: daily.max")
        );
        assert_eq!(
            with("read: {wind: d.w}"),
            format!(
                "{at}.read.wind: \"wind\" is not part of a row: place, date, month, high, low, rain, sunrise, sunset, summary"
            )
        );
        assert_eq!(
            with("read: {high: 'd..max'}"),
            format!("{at}.read.high: \"d..max\" is not a path, like daily.max")
        );
        assert_eq!(
            with("read: {high: d.max}, tag: {wind: place.id}"),
            format!(
                "{at}.tag.wind: \"wind\" is not part of a row: place, date, month, high, low, rain, sunrise, sunset, summary"
            )
        );
        assert_eq!(with("read: {high: d.max}, tag: {place: place.id}"), "");
    }
}
