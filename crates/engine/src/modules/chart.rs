//! The day's places as pins on plain paper (decision 0026). Equirectangular with a `cos`
//! correction at the middle latitude: right for a day you can cross, and honest about being
//! nothing more. Nothing here is fetched; the numbers are the pack's own.

use crate::value::{Map, Value};

/// Metres in one degree of latitude.
const DEGREE_M: f64 = 111_320.0;
/// What the drawing leaves empty around the pins, on each side.
const MARGIN: f64 = 0.1;

/// One place the day stops at, before it is projected.
pub(crate) struct Pin {
    pub id: String,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    /// `now` for the current block's place, `here` for where the device is, else empty.
    pub state: &'static str,
}

/// The lowest and highest of a run of numbers, which is never empty here.
fn span(values: impl Iterator<Item = f64>) -> (f64, f64) {
    values.fold((f64::MAX, f64::MIN), |(low, high), v| (low.min(v), high.max(v)))
}

/// Four decimals: the host draws in whole pixels, and a stable number compares in a test.
fn round(v: f64) -> Value {
    Value::Number((v * 10_000.0).round() / 10_000.0)
}

/// The chart of these stops, in the order the day visits them: `points`, one pin per place;
/// `path`, one `{x, y}` per stop, so a place visited twice is one pin and two stops; and
/// `span_m`, how wide the drawing is on the ground, for a scale line. `Null` with no stops.
pub(crate) fn chart(stops: &[Pin]) -> Value {
    if stops.is_empty() {
        return Value::Null;
    }
    let (low, high) = span(stops.iter().map(|p| p.lat));
    let wide = ((low + high) / 2.0).to_radians().cos();
    // East and south, in degrees still: the drawing's own axes, `y` down as a screen counts.
    let uv: Vec<(f64, f64)> = stops.iter().map(|p| (p.lon * wide, -p.lat)).collect();
    let ((u0, u1), (v0, v1)) = (span(uv.iter().map(|p| p.0)), span(uv.iter().map(|p| p.1)));
    let reach = (u1 - u0).max(v1 - v0);
    // One place, or several at one point: the drawing is the middle of the paper.
    let scale = if reach > 0.0 { (1.0 - 2.0 * MARGIN) / reach } else { 0.0 };
    let paper = |(u, v): &(f64, f64)| {
        (0.5 + (u - (u0 + u1) / 2.0) * scale, 0.5 + (v - (v0 + v1) / 2.0) * scale)
    };
    let mut points: Vec<Value> = Vec::new();
    let mut path: Vec<Value> = Vec::new();
    for (stop, at) in stops.iter().zip(&uv) {
        let (x, y) = paper(at);
        let mut step = Map::default();
        step.set("x", round(x));
        step.set("y", round(y));
        path.push(Value::Map(step.clone()));
        if points.iter().any(|p| p.get("id") == Some(&Value::String(stop.id.clone()))) {
            continue;
        }
        step.set("id", Value::String(stop.id.clone()));
        step.set("name", Value::String(stop.name.clone()));
        step.set("state", Value::String(stop.state.to_owned()));
        points.push(Value::Map(step));
    }
    let across = (u1 - u0).hypot(v1 - v0) * DEGREE_M;
    let mut m = Map::default();
    m.set("points", Value::List(points));
    m.set("path", Value::List(path));
    m.set("span_m", Value::Number(across.round()));
    Value::Map(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::show;

    fn pin(id: &str, lat: f64, lon: f64, state: &'static str) -> Pin {
        Pin { id: id.to_owned(), name: id.to_uppercase(), lat, lon, state }
    }

    #[test]
    fn a_day_that_stops_nowhere_has_no_chart() {
        assert_eq!(chart(&[]), Value::Null);
    }

    #[test]
    fn one_place_sits_in_the_middle_of_the_paper_and_spans_nothing() {
        let out = chart(&[pin("a", 38.7, -9.1, "now")]);
        assert_eq!(
            show(out.get("points")),
            r#"[{"x": 0.5, "y": 0.5, "id": "a", "name": "A", "state": "now"}]"#
        );
        assert_eq!(out.get("span_m"), Some(&Value::Number(0.0)));
    }

    #[test]
    fn the_widest_axis_fills_the_paper_and_the_other_stays_centred() {
        // Two degrees of latitude apart, none of longitude: the drawing is a vertical line.
        let out = chart(&[pin("n", 39.0, -9.0, ""), pin("s", 37.0, -9.0, "here")]);
        let points = out.get("points").unwrap().as_list().unwrap();
        assert_eq!(
            show(Some(&points[0])),
            r#"{"x": 0.5, "y": 0.1, "id": "n", "name": "N", "state": ""}"#
        );
        assert_eq!(
            show(Some(&points[1])),
            r#"{"x": 0.5, "y": 0.9, "id": "s", "name": "S", "state": "here"}"#
        );
        // Two degrees of latitude, and the diagonal of a line is the line.
        assert_eq!(out.get("span_m"), Some(&Value::Number(222640.0)));
    }

    #[test]
    fn a_place_visited_twice_is_one_pin_and_two_stops_on_the_path() {
        let out =
            chart(&[pin("a", 38.0, -9.0, ""), pin("b", 38.1, -9.0, ""), pin("a", 38.0, -9.0, "")]);
        assert_eq!(out.get("points").unwrap().as_list().unwrap().len(), 2);
        assert_eq!(
            show(out.get("path")),
            r#"[{"x": 0.5, "y": 0.9}, {"x": 0.5, "y": 0.1}, {"x": 0.5, "y": 0.9}]"#
        );
    }

    #[test]
    fn longitude_is_narrowed_at_the_latitude_the_day_is_at() {
        // The same degrees each way, far north: the east west side draws shorter than the other.
        let out = chart(&[pin("a", 60.0, 0.0, ""), pin("b", 61.0, 1.0, "")]);
        let points = out.get("points").unwrap().as_list().unwrap();
        assert_eq!(
            show(Some(&points[0])),
            r#"{"x": 0.303, "y": 0.9, "id": "a", "name": "A", "state": ""}"#
        );
        assert_eq!(
            show(Some(&points[1])),
            r#"{"x": 0.697, "y": 0.1, "id": "b", "name": "B", "state": ""}"#
        );
    }
}
