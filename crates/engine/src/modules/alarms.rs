//! The alarms the phone rings with the app closed (decision 0033): jet-lag steps and alerts that
//! ask for one, and the set-off notice of every block with a `leave`.

use super::{Run, id, list, plan};
use crate::clock::shift;
use crate::engine::Alarm;
use crate::validate::patterns::is_stamp;
use crate::value::{text, truthy};

impl Run<'_> {
    /// Every alarm still to come, by time, from the content roots of the days, the jet lag and the
    /// alerts. `set_off` is the words a set-off notice carries under the block's text.
    pub fn alarms(&self, roots: [Option<&str>; 3], set_off: &str) -> Vec<Alarm> {
        let [days, jet_lag, alerts] =
            roots.map(|r| list(r.and_then(|r| self.keymap.root(self.content, r))));
        let now = &self.world.now;
        let mut alarms = Vec::new();
        for day in jet_lag {
            let date = text(self.read(day, "jet_lag", "date"));
            for (n, s) in self.steps(day) {
                let (time, zone) = (text(s.get("time")), text(s.get("zone")));
                if truthy(s.get("alarm"))
                    && !time.is_empty()
                    && self.stamp(&date, &time, &zone) > *now
                {
                    let (key, at) = (format!("jet_lag.{date}.{n}"), format!("{date}T{time}"));
                    let title = text(s.get("text"));
                    alarms.push(Alarm { key, at, zone, title, text: String::new() });
                }
            }
        }
        for day in days {
            let date = self.date_of(day);
            for b in self.planned(days, day) {
                let off = plan::whole(b.get("leave"));
                if off <= 0 {
                    continue;
                }
                let at = shift(&format!("{date}T{}", text(b.get("time"))), -off);
                let zone = text(b.get("zone"));
                if self.stamp(&at[..10], &at[11..], &zone) > *now {
                    let key = format!("leave.{}", text(b.get("event")));
                    let title = super::calendar::title(&text(b.get("text")));
                    alarms.push(Alarm { key, at, zone, title, text: set_off.to_owned() });
                }
            }
        }
        for alert in alerts.iter().filter(|a| truthy(self.read(a, "alert", "alarm"))) {
            let field = |k: &str| text(self.read(alert, "alert", k));
            let (time, says) = (field("time"), [field("action"), field("detail")]);
            for (at, from) in self.falls(alert) {
                // It rings when it says to notify, else at its hour.
                let hour = format!("{at}T{time}");
                let ring = [from, at, hour].into_iter().find(|r| is_stamp(r));
                let Some(ring) = ring.filter(|r| r > now) else {
                    continue;
                };
                let key = format!("alert.{}.{ring}", id(alert));
                let words = says.iter().find(|s| !s.is_empty()).cloned().unwrap_or_default();
                let title = field("title");
                alarms.push(Alarm { key, at: ring, zone: String::new(), title, text: words });
            }
        }
        alarms.sort_by(|a, b| a.at.cmp(&b.at));
        alarms
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::{Alarm, World};
    use crate::modules::Run;
    use crate::validate::Keymap;
    use crate::value::{Map, Value};
    use crate::yaml::parse;

    const CONTENT: &str = r#"days:
  - date: 2026-04-11
    zone: Europe/Madrid
    blocks:
      - ["09:00", "Walk"]
      - ["10:40", "Gone", {leave: 30}]
      - ["11:30", "Ferry. Gate 2.", {leave: 20}]
  - date: 2026-04-12
    blocks: [["08:00", "Car", {leave: 15}]]
    options:
      - {id: a, name: A, blocks: [["09:00", "Boat", {leave: 10}]]}
      - {id: b, name: B, blocks: []}
jet_lag:
  - date: 2026-04-11
    steps:
      - {time: "10:15", text: Up, do: wake, alarm: true}
      - {time: "10:20", text: Light, alarm: false}
      - {text: All day, alarm: true}
      - {time: "09:00", text: Early, alarm: true}
alerts:
  - {id: tide, title: Tide, at: 2026-04-11, time: "12:00", alarm: true, detail: Low, action: Move the car}
  - {id: call, title: Call, at: "2026-04-11T15:00", notify_from: "2026-04-11T14:00", alarm: true, detail: Ring}
  - {id: daily, title: Daily, at: 2026-04-11, time: "09:00", alarm: true, repeat: {every: 1, until: 2026-04-13}}
  - {id: quiet, title: Quiet, at: "2026-04-11T16:00"}
  - {id: undated, title: Undated, at: 2026-04-11, alarm: true}
  - {id: over, title: Over, at: "2026-04-11T16:00", alarm: true, status: done}
"#;

    fn alarms(store: &[(&str, &str)]) -> Vec<Alarm> {
        let content = parse(CONTENT).unwrap().as_map().cloned().unwrap();
        let store = store.iter().map(|(k, v)| ((*k).to_owned(), Value::String((*v).to_owned())));
        let world = World {
            now: "2026-04-11T10:00".into(),
            store: Map(store.collect()),
            zones: parse("{Europe/Madrid: \"2026-04-11T11:00\"}")
                .unwrap()
                .as_map()
                .cloned()
                .unwrap(),
            ..World::default()
        };
        let run = Run::new(Keymap::of(&Value::Null), &content, &world, Map::default());
        run.alarms([Some("days"), Some("jet_lag"), Some("alerts")], "Set off now")
    }

    fn line(a: &Alarm) -> String {
        format!("{} {} {} | {} | {}", a.key, a.at, a.zone, a.title, a.text)
    }

    #[test]
    fn what_asks_for_an_alarm_rings_once_to_come_and_a_set_off_notice_for_each_leave() {
        let all: Vec<String> = alarms(&[]).iter().map(line).collect();
        assert_eq!(
            all,
            [
                "jet_lag.2026-04-11.0 2026-04-11T10:15  | Up | ",
                "leave.2026-04-11.blocks.2 2026-04-11T11:10 Europe/Madrid | Ferry | Set off now",
                "alert.tide.2026-04-11T12:00 2026-04-11T12:00  | Tide | Move the car",
                "alert.call.2026-04-11T14:00 2026-04-11T14:00  | Call | Ring",
                "leave.2026-04-12.blocks.0 2026-04-12T07:45  | Car | Set off now",
                "alert.daily.2026-04-12T09:00 2026-04-12T09:00  | Daily | ",
                "alert.daily.2026-04-13T09:00 2026-04-13T09:00  | Daily | ",
            ]
        );
        let chosen = alarms(&[("choice.2026-04-12", "a")]);
        assert_eq!(chosen[5].key, "leave.2026-04-12.option-a.0");
        assert_eq!(chosen[5].at, "2026-04-12T08:50");
    }

    #[test]
    fn a_set_off_notice_follows_its_block_moved_and_is_gone_once_it_is_dropped() {
        let leave = |plan: &str| {
            let content = parse(CONTENT).unwrap().as_map().cloned().unwrap();
            let store = parse(&format!("plan.2026-04-11.blocks.2: {plan}")).unwrap();
            let world = World {
                now: "2026-04-11T10:00".into(),
                store: store.as_map().cloned().unwrap(),
                ..World::default()
            };
            let run = Run::new(Keymap::of(&Value::Null), &content, &world, Map::default());
            let all = run.alarms([Some("days"), None, None], "");
            all.iter().find(|a| a.key == "leave.2026-04-11.blocks.2").map(|a| a.at.clone())
        };
        assert_eq!(leave("{shift: 30}").as_deref(), Some("2026-04-11T11:40"));
        assert_eq!(leave("{off: true}"), None);
    }

    #[test]
    fn an_alert_repeating_for_ever_rings_a_year_of_times_at_most() {
        let content = "alerts: [{id: f, at: 2026-04-11, time: '09:00', alarm: true, repeat: {every: 1, until: 2099-01-01}}]";
        let content = parse(content).unwrap().as_map().cloned().unwrap();
        let world = World { now: "2026-04-11T08:00".into(), ..World::default() };
        let run = Run::new(Keymap::of(&Value::Null), &content, &world, Map::default());
        let all = run.alarms([None, None, Some("alerts")], "");
        assert_eq!((all.len(), all[365].at.as_str()), (366, "2027-04-11T09:00"));
    }

    #[test]
    fn with_no_roots_there_is_nothing_to_ring() {
        let content = Map::default();
        let world = World { now: "2026-04-11T10:00".into(), ..World::default() };
        let run = Run::new(Keymap::of(&Value::Null), &content, &world, Map::default());
        assert!(run.alarms([None, None, None], "").is_empty());
    }
}
