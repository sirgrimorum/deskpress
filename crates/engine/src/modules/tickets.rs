//! The papers the phone shows outside the app (decision 0042): each timed block whose booking has
//! documents, from when the block before it begins, as the moment screen offers it, until it ends.

use super::{Run, list};
use crate::engine::{Paper, Ticket};
use crate::value::text;

impl Run<'_> {
    /// Every ticket not over yet, in the plan's order, from the content roots of the days, the
    /// places and the documents. A block's own `ticket` wins over its place's `during` one.
    pub fn tickets(&self, roots: [Option<&str>; 3]) -> Vec<Ticket> {
        let [days, places, documents] =
            roots.map(|r| r.and_then(|r| self.keymap.root(self.content, r)));
        let (days, documents) = (list(days), list(documents));
        let documents: Vec<_> = documents.iter().map(|d| self.canon(d, "document")).collect();
        let mut tickets = Vec::new();
        for day in days {
            let date = self.date_of(day);
            let timed = self.planned(days, day);
            for (n, b) in timed.iter().enumerate() {
                let place = places.and_then(|p| p.get(&text(b.get("place"))));
                let during = place.and_then(|p| self.read(p, "place", "during"));
                let during = during.map(|d| self.keymap.canon(d, "during"));
                let ticket = Some(text(b.get("ticket")))
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| text(during.as_ref().and_then(|d| d.get("ticket"))));
                let papers: Vec<Paper> = documents
                    .iter()
                    .filter(|d| !ticket.is_empty() && text(d.get("ticket")) == ticket)
                    .map(|d| Paper { title: text(d.get("title")), file: text(d.get("file")) })
                    .filter(|p| !p.file.is_empty())
                    .collect();
                let (until, until_zone) = self.end(&date, &timed, n);
                if papers.is_empty()
                    || self.stamp(&until[..10], &until[11..], &until_zone) <= self.world.now
                {
                    continue;
                }
                let time = text(b.get("time"));
                let before = timed[..n].iter().rev().find(|a| text(a.get("time")) < time);
                let (from, from_zone) = match before {
                    Some(a) => (format!("{date}T{}", text(a.get("time"))), text(a.get("zone"))),
                    None => (format!("{date}T00:00"), text(b.get("zone"))),
                };
                let title = super::calendar::title(&text(b.get("text")));
                let key = text(b.get("event"));
                tickets.push(Ticket { key, from, from_zone, until, until_zone, title, papers });
            }
        }
        tickets
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::{Ticket, World};
    use crate::modules::Run;
    use crate::validate::Keymap;
    use crate::value::{Map, Value};
    use crate::yaml::parse;

    const CONTENT: &str = r#"days:
  - date: 2026-04-11
    zone: Europe/Madrid
    blocks:
      - ["07:00", "Train. Coach 4.", {ticket: bookings.train, until: "09:30"}]
      - ["10:00", "Museum", {place: museum}]
      - ["13:00", "Lunch"]
      - ["13:00", "Picnic"]
      - ["16:00", "Show", {ticket: bookings.none}]
places:
  museum:
    name: Museum
    during: {ticket: bookings.museum}
documents:
  - {id: rita, title: Rita's seat, file: files/rita.pdf, ticket: bookings.train}
  - {id: tomas, title: Tomas's seat, file: files/tomas.pdf, ticket: bookings.train}
  - {id: entry, title: Entry, file: files/entry.pdf, ticket: bookings.museum}
  - {id: show, title: Show, ticket: bookings.none}
  - {id: passport, title: Passport, file: files/passport.pdf}
"#;

    fn tickets(now: &str) -> Vec<Ticket> {
        let content = parse(CONTENT).unwrap().as_map().cloned().unwrap();
        let world = World { now: now.into(), ..World::default() };
        let run = Run::new(Keymap::of(&Value::Null), &content, &world, Map::default());
        run.tickets([Some("days"), Some("places"), Some("documents")])
    }

    fn line(t: &Ticket) -> String {
        let papers: Vec<String> =
            t.papers.iter().map(|p| format!("{} {}", p.title, p.file)).collect();
        let (from, until) =
            (format!("{} {}", t.from, t.from_zone), format!("{} {}", t.until, t.until_zone));
        format!("{} | {from} | {until} | {} | {}", t.key, t.title, papers.join(", "))
    }

    #[test]
    fn a_booking_with_papers_shows_from_the_block_before_until_it_ends() {
        assert_eq!(
            tickets("2026-04-11T06:00").iter().map(line).collect::<Vec<_>>(),
            [
                "2026-04-11.blocks.0 | 2026-04-11T00:00 Europe/Madrid | 2026-04-11T09:30 Europe/Madrid \
                 | Train | Rita's seat files/rita.pdf, Tomas's seat files/tomas.pdf",
                "2026-04-11.blocks.1 | 2026-04-11T07:00 Europe/Madrid | 2026-04-11T13:00 Europe/Madrid \
                 | Museum | Entry files/entry.pdf",
            ]
        );
    }

    #[test]
    fn a_ticket_over_is_gone_and_with_no_roots_there_is_none() {
        let left: Vec<String> = tickets("2026-04-11T10:00").iter().map(|t| t.key.clone()).collect();
        assert_eq!(left, ["2026-04-11.blocks.1"]);
        let content = Map::default();
        let world = World { now: "2026-04-11T10:00".into(), ..World::default() };
        let run = Run::new(Keymap::of(&Value::Null), &content, &world, Map::default());
        assert!(run.tickets([None, None, None]).is_empty());
    }
}
