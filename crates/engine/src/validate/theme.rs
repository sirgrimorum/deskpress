//! The theme: every token present, every color a hex color, every text pair readable at 4.5:1.

use super::Report;
use super::patterns::is_slug;
use crate::value::{Value, show, text, truthy};

pub(super) const TOKENS: [&str; 23] = [
    "paper",
    "ink",
    "ink-muted",
    "card",
    "card-line",
    "line",
    "rule",
    "soft",
    "alert",
    "action-bg",
    "action-ink",
    "highlight-bg",
    "highlight-ink",
    "highlight-line",
    "highlight-text",
    "chip-bg",
    "chip-ink",
    "bar-bg",
    "bar-ink",
    "bar-muted",
    "missing-fill",
    "missing-border",
    "missing-text",
];

/// Text on a background: every pair a component draws.
const PAIRS: [(&str, &str); 13] = [
    ("ink", "paper"),
    ("ink-muted", "paper"),
    ("ink", "card"),
    ("ink-muted", "card"),
    ("action-ink", "action-bg"),
    ("highlight-ink", "highlight-bg"),
    ("highlight-text", "highlight-bg"),
    ("chip-ink", "chip-bg"),
    ("bar-ink", "bar-bg"),
    ("bar-muted", "bar-bg"),
    ("alert", "paper"),
    ("alert", "card"),
    ("missing-text", "missing-fill"),
];

pub(super) fn check(r: &mut Report, theme: &Value) {
    let Some(themes) = theme.get("themes").and_then(Value::as_map) else {
        r.error("theme", "a theme file needs a \"themes:\" mapping of theme id to theme");
        return;
    };
    let ids: Vec<&str> = themes.keys().collect();
    let default = theme.get("default");
    if truthy(default) && !ids.contains(&text(default).as_str()) {
        let message = format!("{} is not one of the themes: {}", show(default), ids.join(", "));
        r.error("theme.default", message);
    }
    for (id, t) in themes.iter() {
        let at = format!("theme.themes.{id}");
        if !is_slug(id) {
            r.error(&at, format!("{} is not a slug", show(Some(&Value::String(id.into())))));
        }
        let Some(t) = t.as_map() else {
            r.error(&at, "has to be a mapping");
            continue;
        };
        if !truthy(t.get("name")) {
            r.error(&at, "a theme needs a name: it is what a person sees on the handoff screen");
        }
        if !matches!(t.get("mode"), Some(Value::String(m)) if m == "light" || m == "dark") {
            r.error(format!("{at}.mode"), "has to be light or dark");
        }
        let Some(colors) = t.get("colors").and_then(Value::as_map) else {
            r.error(format!("{at}.colors"), "a theme needs all 23 color tokens");
            continue;
        };
        let missing: Vec<&str> = TOKENS.into_iter().filter(|k| !truthy(colors.get(k))).collect();
        if !missing.is_empty() {
            let message = format!(
                "missing {} of the 23 tokens: {}. A component that needed one of these would have rendered invisible",
                missing.len(),
                missing.join(", ")
            );
            r.error(format!("{at}.colors"), message);
        }
        let extra: Vec<&str> = colors.keys().filter(|k| !TOKENS.contains(k)).collect();
        if !extra.is_empty() {
            let message = format!("not a token the shell knows: {}", extra.join(", "));
            r.warn(format!("{at}.colors"), message);
        }
        let mut rgb: Vec<(&str, Rgba)> = Vec::new();
        for (k, v) in colors.iter() {
            match hex(&text(Some(v))) {
                Some(c) => rgb.push((k, c)),
                None => r.error(
                    format!("{at}.colors.{k}"),
                    format!("{} is not a hex color", show(Some(v))),
                ),
            }
        }
        let get = |k: &str| rgb.iter().find(|(name, _)| *name == k).map(|(_, c)| *c);
        let paper = get("paper");
        for (fg, bg) in PAIRS {
            let (Some(f), Some(b)) = (get(fg), get(bg)) else {
                continue;
            };
            let ratio = contrast(over(f, Some(b), paper), over(b, paper, paper));
            if ratio < 4.5 {
                let message = format!(
                    "{fg} on {bg} is {ratio:.2}:1, under 4.5:1. This gets read outdoors in the sun, with one hand"
                );
                r.error(format!("{at}.colors"), message);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Rgba {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

const WHITE: Rgba = Rgba { r: 255.0, g: 255.0, b: 255.0, a: 1.0 };

/// `#rrggbb` or `#rrggbbaa`.
pub(super) fn hex(value: &str) -> Option<Rgba> {
    let digits = value.trim().strip_prefix('#')?;
    if !matches!(digits.len(), 6 | 8) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| f64::from(u8::from_str_radix(&digits[i..i + 2], 16).unwrap_or_default());
    let a = if digits.len() == 8 { byte(6) / 255.0 } else { 1.0 };
    Some(Rgba { r: byte(0), g: byte(2), b: byte(4), a })
}

// A token with alpha is measured over what is actually behind it.
fn over(color: Rgba, behind: Option<Rgba>, paper: Option<Rgba>) -> Rgba {
    if color.a == 1.0 {
        return color;
    }
    let base = behind.filter(|b| b.a == 1.0).or(paper).unwrap_or(WHITE);
    let mix = |c: f64, under: f64| c * color.a + under * (1.0 - color.a);
    Rgba { r: mix(color.r, base.r), g: mix(color.g, base.g), b: mix(color.b, base.b), a: 1.0 }
}

fn luminance(c: Rgba) -> f64 {
    let f = |v: f64| {
        let c = v / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
}

/// The WCAG contrast ratio, 1 to 21.
fn contrast(a: Rgba, b: Rgba) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(r: f64, g: f64, b: f64, a: f64) -> Rgba {
        Rgba { r, g, b, a }
    }

    #[test]
    fn only_hex_colors_are_colors() {
        for bad in ["black", "#fff", "#12345", "#1234567", "#gggggg", "123456", ""] {
            assert_eq!(hex(bad), None, "{bad}");
        }
        assert_eq!(hex(" #FF8000 "), Some(rgb(255.0, 128.0, 0.0, 1.0)));
        assert_eq!(hex("#00000080"), Some(rgb(0.0, 0.0, 0.0, 128.0 / 255.0)));
    }

    #[test]
    fn contrast_runs_from_one_to_twenty_one() {
        let black = rgb(0.0, 0.0, 0.0, 1.0);
        assert!((contrast(black, WHITE) - 21.0).abs() < 1e-9);
        assert!((contrast(WHITE, WHITE) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_translucent_color_is_measured_over_what_is_behind_it() {
        let half_black = rgb(0.0, 0.0, 0.0, 0.5);
        let red = rgb(255.0, 0.0, 0.0, 1.0);
        assert_eq!(over(red, None, None), red);
        assert_eq!(over(half_black, Some(red), None), rgb(127.5, 0.0, 0.0, 1.0));
        let see_through = rgb(0.0, 0.0, 255.0, 0.5);
        assert_eq!(over(half_black, Some(see_through), Some(red)), rgb(127.5, 0.0, 0.0, 1.0));
        assert_eq!(over(half_black, None, None), rgb(127.5, 127.5, 127.5, 1.0));
    }
}
