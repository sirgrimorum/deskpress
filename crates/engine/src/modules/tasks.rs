//! What has to be done around the trip (decision 0033): by deadline, the ones the pack marks done
//! left out, and a tick on the phone kept as a fact, so a task can be ticked and unticked.

use super::{Run, id, list, record};
use crate::value::{Map, Value, text, truthy};

/// What a task's `status` may say; a pack in another language maps its words in `keymap.values.status`.
pub(crate) const STATUSES: [&str; 3] = ["open", "partial", "done"];

impl Run<'_> {
    /// `tasks` is `{open, items}`: every task the pack has not marked done, by deadline and the
    /// undated last, each with the `fact` that ticks it, `done` once ticked, `late` past its
    /// deadline and not done, and `about`, its deadline and who. `open` counts the ones not ticked.
    pub(super) fn tasks(&mut self, data: Option<&Value>) {
        let task = |t: &Value| {
            let status = self.keymap.value("status", self.read(t, "task", "status"));
            if status == "done" {
                return None;
            }
            let mut m = self.canon(t, "task");
            let fact = format!("task.{}", id(t));
            let done = truthy(self.world.store.get(&fact));
            let deadline = text(m.get("deadline"));
            let late = !done && !deadline.is_empty() && deadline.as_str() < self.date;
            let status = if status.is_empty() { "open".to_owned() } else { status };
            let who = text(m.get("who"));
            let about = [deadline.as_str(), who.as_str()].into_iter().filter(|s| !s.is_empty());
            m.set("about", Value::String(about.collect::<Vec<_>>().join(" · ")));
            m.set("status", Value::String(status));
            m.set("fact", Value::String(fact));
            m.set("done", Value::Bool(done));
            m.set("late", Value::Bool(late));
            Some((deadline, m))
        };
        let mut items: Vec<(String, Map)> = list(data).iter().filter_map(task).collect();
        items.sort_by(|(a, _), (b, _)| (a.is_empty(), a).cmp(&(b.is_empty(), b)));
        let open = items.iter().filter(|(_, m)| !truthy(m.get("done"))).count();
        let items = items.into_iter().map(|(_, m)| Value::Map(m)).collect();
        let tasks = record([("open", Value::Number(open as f64)), ("items", Value::List(items))]);
        self.set("tasks", tasks);
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::World;
    use crate::modules::tests::run;
    use crate::value::{Map, Value, show};

    const TASKS: &str = r#"tasks:
  - {id: forms, title: Forms, deadline: 2026-09-28, status: hecho}
  - {id: tickets, titulo: Tickets, who: Rita, deadline: 2026-10-01, status: partial}
  - {id: bags, title: Bags}
  - {id: car, title: Car, deadline: 2026-09-29}
  - {id: now, title: Now, deadline: 2026-09-30}
  - {id: photos, title: Photos, deadline: 2026-10-05}
"#;

    fn tasks(store: &[(&str, bool)]) -> Value {
        let manifest = "keymap: {task: {title: titulo}, values: {status: {done: hecho}}}";
        let store = store.iter().map(|(k, v)| ((*k).to_owned(), Value::Bool(*v)));
        let world = World {
            now: "2026-09-30T10:00".into(),
            store: Map(store.collect()),
            ..World::default()
        };
        run("tasks", manifest, TASKS, &world, "").scope.get("tasks").cloned().unwrap()
    }

    fn each(tasks: &Value, key: &str) -> String {
        let items = tasks.get("items").and_then(Value::as_list).unwrap_or_default();
        items.iter().map(|t| show(t.get(key))).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn tasks_come_by_deadline_the_done_ones_left_out_and_a_tick_kept() {
        let all = tasks(&[]);
        assert_eq!(each(&all, "id"), r#""car" "now" "tickets" "photos" "bags""#);
        assert_eq!(each(&all, "title"), r#""Car" "Now" "Tickets" "Photos" "Bags""#);
        assert_eq!(each(&all, "status"), r#""open" "open" "partial" "open" "open""#);
        // Due today is not late yet.
        assert_eq!(each(&all, "late"), "true false false false false");
        let about = r#""2026-09-29" "2026-09-30" "2026-10-01 · Rita" "2026-10-05" """#;
        assert_eq!(each(&all, "about"), about);
        let facts = r#""task.car" "task.now" "task.tickets" "task.photos" "task.bags""#;
        assert_eq!(each(&all, "fact"), facts);
        assert_eq!(show(all.get("open")), "5");
        let ticked = tasks(&[("task.car", true), ("task.bags", false)]);
        assert_eq!(each(&ticked, "done"), "true false false false false");
        assert_eq!(each(&ticked, "late"), "false false false false false");
        assert_eq!(show(ticked.get("open")), "4");
    }
}
