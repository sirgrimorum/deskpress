//! The engine on a desk. `run` takes its output streams, so every path is a unit test.

use std::io::Write;
use std::path::Path;

use deskpress_engine::engine::{Engine, Nav, Region, View, World};
use deskpress_engine::tree::Tree;
use deskpress_engine::value::{Map, Value, show};
use deskpress_engine::{pack, validate, yaml};

pub const USAGE: &str = "usage: deskpress <command>

commands:
  validate <pack>                            check a pack folder, or its pack.yaml, and list every problem
  screen <pack> --at <time> [world]          print the screen for that moment and its watch, as JSON
  act <pack> --at <time> [world] <action>... run actions in turn and print where they leave the screen
  version                                    print the screen tree version

<time> is local to the pack, YYYY-MM-DDTHH:MM. [world] is any of --holder <person id>,
--inside <place id>, --store <key>=<value> and --can <name>, the last three once per value. Any
--inside means the device is located; --inside '' says it is inside none. An action is a name,
or name=<value> to send it a value. A value is YAML, like 3, true, b or [a, b].";

/// Runs one command and returns the exit code.
pub fn run(args: &[String], out: &mut impl Write, err: &mut impl Write) -> u8 {
    let (code, stdout, stderr) = match args {
        [command] if command == "version" => {
            (0, format!("tree {}\n", deskpress_engine::TREE_VERSION), String::new())
        }
        [command, target] if command == "validate" => validate(target),
        [command, target, rest @ ..] if command == "screen" || command == "act" => {
            screen(target, rest, command == "act")
        }
        _ => (2, String::new(), format!("{USAGE}\n")),
    };
    // A closed pipe is not worth a panic.
    out.write_all(stdout.as_bytes())
        .and_then(|()| err.write_all(stderr.as_bytes()))
        .map_or(1, |()| code)
}

/// The pack at `target`, a folder or its manifest, or the exit code and message that say why not.
fn open(target: &str) -> Result<pack::Pack, (u8, String, String)> {
    let Ok(path) = Path::new(target).canonicalize() else {
        let message =
            format!("no pack at {target}. Point me at a pack folder or at its pack.yaml\n");
        return Err((2, String::new(), message));
    };
    let (dir, manifest) = if path.is_dir() {
        (path.as_path(), "pack.yaml".into())
    } else {
        (path.parent().unwrap_or(&path), path.file_name().unwrap_or_default().to_string_lossy())
    };
    // The engine keeps every name inside the folder; a link on disk could still lead out of it.
    let read = |file: &str| {
        let real = dir.join(file).canonicalize().map_err(|e| e.to_string())?;
        if !real.starts_with(dir) {
            return Err("is a link that leaves the pack folder".to_owned());
        }
        std::fs::read_to_string(real).map_err(|e| e.to_string())
    };
    pack::load(&manifest, read).map_err(|e| (1, String::new(), format!("{e}\n")))
}

/// `deskpress validate`: exit 0 when the pack loads, 1 when it does not, 2 when there is no pack.
fn validate(target: &str) -> (u8, String, String) {
    let pack = match open(target) {
        Ok(pack) => pack,
        Err(e) => return e,
    };
    let report = validate::validate(&pack.manifest, &pack.content, pack.theme.as_ref());
    let mut out = String::new();
    for e in &report.errors {
        out += &format!("error  {}: {}\n", e.at, e.message);
    }
    for w in &report.warnings {
        out += &format!("warn   {}: {}\n", w.at, w.message);
    }
    let name = pack.name().unwrap_or_else(|| target.to_owned());
    let warnings = count(report.warnings.len(), "warning");
    if report.ok() {
        out += &format!("\n{name}: loads. {warnings}.\n");
        return (0, out, String::new());
    }
    let errors = count(report.errors.len(), "error");
    out += &format!("\n{name}: does not load. {errors}, {warnings}.\n");
    (1, out, String::new())
}

/// `deskpress screen` and `deskpress act`: exit 0 with the JSON, 1 when the pack does not load or
/// an action is not on the screen, 2 when the arguments are wrong.
fn screen(target: &str, args: &[String], act: bool) -> (u8, String, String) {
    let usage = |message: String| (2, String::new(), format!("{message}\n\n{USAGE}\n"));
    let (mut world, actions) = match world(args) {
        Ok(parsed) => parsed,
        Err(message) => return usage(message),
    };
    if actions.is_empty() == act {
        let message = if act { "act needs an action" } else { "screen takes no actions: use act" };
        return usage(message.to_owned());
    }
    let pack = match open(target) {
        Ok(pack) => pack,
        Err(e) => return e,
    };
    let Ok((engine, _)) = Engine::load(pack) else {
        return (
            1,
            String::new(),
            format!("the pack does not load: deskpress validate {target}\n"),
        );
    };
    let mut nav = Nav::default();
    let mut view = match engine.screen(&world, &mut nav) {
        Ok(view) => view,
        Err(e) => return (1, String::new(), format!("{e}\n")),
    };
    let (mut stored, mut commands) = (Map::default(), Vec::new());
    for action in actions {
        let (name, arg) = match action.split_once('=') {
            Some((name, arg)) => (name, value(arg)),
            None => (action.as_str(), Value::Null),
        };
        let out = match engine.dispatch(&world, &mut nav, name, arg) {
            Ok(out) => out,
            Err(e) => return (1, String::new(), format!("{e}\n")),
        };
        // The host would persist these, so the next action sees them.
        for (k, v) in out.store.iter() {
            world.store.set(k, v.clone());
            stored.set(k, v.clone());
        }
        commands.extend(out.commands.into_iter().map(|c| {
            Value::Map(Map(vec![
                ("name".to_owned(), Value::String(c.name)),
                ("args".to_owned(), Value::Map(c.args)),
            ]))
        }));
        view = out.view;
    }
    let effects = act.then(|| {
        Map(vec![
            ("store".to_owned(), Value::Map(stored)),
            ("commands".to_owned(), Value::List(commands)),
        ])
    });
    (0, format!("{}\n", show(Some(&json(view, effects)))), String::new())
}

/// The world the flags describe, and the arguments left over.
fn world(args: &[String]) -> Result<(World, Vec<String>), String> {
    let mut world = World::default();
    let mut rest = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if !arg.starts_with("--") {
            rest.push(arg.clone());
            continue;
        }
        let Some(v) = args.next() else {
            return Err(format!("{arg} needs a value"));
        };
        match arg.as_str() {
            "--at" => world.now.clone_from(v),
            "--holder" => world.holder.clone_from(v),
            "--inside" => {
                world.located = true;
                world.inside.extend(Some(v.clone()).filter(|v| !v.is_empty()));
            }
            "--can" => world.can.push(v.clone()),
            "--store" => {
                let Some((key, v)) = v.split_once('=') else {
                    return Err(format!("--store takes key=value, not {v}"));
                };
                world.store.set(key, value(v));
            }
            _ => return Err(format!("{arg} is not an option")),
        }
    }
    if world.now.is_empty() {
        return Err("--at <time> is needed: the moment to show, YYYY-MM-DDTHH:MM".to_owned());
    }
    Ok((world, rest))
}

/// A value from the command line, read as YAML; what YAML cannot read is taken as text.
fn value(src: &str) -> Value {
    let doc = yaml::parse(&format!("v: {src}")).ok();
    doc.and_then(|d| d.get("v").cloned()).unwrap_or_else(|| Value::String(src.to_owned()))
}

/// A node as JSON, with the nodes it holds.
fn node(n: deskpress_engine::tree::Node) -> Value {
    let on = n.on.into_iter().map(|(k, v)| (k, Value::String(v)));
    Value::Map(Map(vec![
        ("key".to_owned(), Value::String(n.key)),
        ("kind".to_owned(), Value::String(n.kind)),
        ("props".to_owned(), Value::Map(Map(n.props))),
        ("on".to_owned(), Value::Map(Map(on.collect()))),
        ("children".to_owned(), Value::List(n.children.into_iter().map(node).collect())),
    ]))
}

/// A region as JSON.
fn region(r: Region) -> Value {
    let number = |k: &str, n: f64| (k.to_owned(), Value::Number(n));
    Value::Map(Map(vec![
        ("id".to_owned(), Value::String(r.id)),
        number("lat", r.lat),
        number("lon", r.lon),
        number("radius_m", r.radius_m),
    ]))
}

/// The view as the JSON a renderer would get, with what the actions did after it.
fn json(view: View, effects: Option<Map>) -> Value {
    let Tree { version, screen, nodes, theme, kid } = view.tree;
    let text = |s: String| Value::String(s);
    let watch = Map(vec![
        ("until".to_owned(), text(view.watch.until)),
        ("regions".to_owned(), Value::List(view.watch.regions.into_iter().map(region).collect())),
    ]);
    let mut out = Map(vec![
        ("version".to_owned(), Value::Number(f64::from(version))),
        ("screen".to_owned(), text(screen)),
        ("nodes".to_owned(), Value::List(nodes.into_iter().map(node).collect())),
        ("theme".to_owned(), text(theme)),
        ("kid".to_owned(), Value::Bool(kid)),
        ("watch".to_owned(), Value::Map(watch)),
    ]);
    out.0.extend(effects.map(|e| e.0).unwrap_or_default());
    Value::Map(out)
}

fn count(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Deref;
    use std::path::PathBuf;

    fn call(args: &[&str]) -> (u8, String, String) {
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&args, &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    /// A folder of its own under the temp dir, removed when the test is done with it.
    struct Folder(PathBuf);

    impl Deref for Folder {
        type Target = Path;

        fn deref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("deskpress-cli-{name}-{}", std::process::id()))
    }

    fn folder(name: &str, files: &[(&str, &str)]) -> Folder {
        let dir = Folder(temp(name));
        let _ = std::fs::remove_dir_all(&dir.0);
        for (file, text) in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        dir
    }

    fn validate_at(path: &Path) -> (u8, String, String) {
        call(&["validate", path.to_str().unwrap()])
    }

    const DAY: &str = "days:\n  - {date: 2026-04-11, title: A day, blocks: [['10:00', 'x']]}\n";

    fn manifest(head: &str) -> String {
        format!(
            "pack:\n  id: test\n  language: en\n  timezone: UTC\n  content: content.yaml\n{head}"
        )
    }

    #[test]
    fn no_command_is_refused_with_the_usage() {
        assert_eq!(call(&[]), (2, String::new(), format!("{USAGE}\n")));
    }

    #[test]
    fn an_unknown_command_is_refused() {
        let (code, out, _) = call(&["explode"]);
        assert_eq!((code, out.as_str()), (2, ""));
    }

    #[test]
    fn a_known_command_with_extra_arguments_is_refused() {
        for args in [&["version", "extra"][..], &["validate"], &["validate", "a", "b"]] {
            let (code, out, _) = call(args);
            assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
        }
    }

    #[test]
    fn a_closed_stream_is_exit_code_one_not_a_panic() {
        // A zero length slice refuses every write, like a closed pipe.
        let (mut out, mut err): (&mut [u8], &mut [u8]) = (&mut [], &mut []);
        assert_eq!(run(&["version".to_owned()], &mut out, &mut err), 1);
        assert_eq!(run(&[], &mut out, &mut err), 1);
    }

    #[test]
    fn no_pack_at_the_path_exits_two() {
        let missing = temp("nothing-here");
        let (code, out, err) = validate_at(&missing);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.starts_with("no pack at "), "{err}");
        assert!(err.ends_with("Point me at a pack folder or at its pack.yaml\n"), "{err}");
    }

    #[test]
    fn a_pack_that_cannot_be_read_exits_one_on_stderr() {
        let dir = folder("unreadable", &[("pack.yaml", &manifest(""))]);
        let (code, out, err) = validate_at(&dir);
        assert_eq!((code, out.as_str()), (1, ""));
        assert!(err.starts_with("content.yaml: "), "{err}");

        // There, and inside the folder, but not a file.
        std::fs::create_dir(dir.join("content.yaml")).unwrap();
        let (code, _, err) = validate_at(&dir);
        assert_eq!(code, 1);
        assert!(err.starts_with("content.yaml: "), "{err}");
    }

    #[test]
    fn a_link_that_leaves_the_pack_folder_is_refused() {
        let outside = folder("outside", &[("content.yaml", DAY)]);
        let dir = folder("linked", &[("pack.yaml", &manifest(""))]);
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.join("content.yaml"), dir.join("content.yaml")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(outside.join("content.yaml"), dir.join("content.yaml"))
            .unwrap();
        let (code, out, err) = validate_at(&dir);
        assert_eq!((code, out.as_str()), (1, ""));
        assert_eq!(err, "content.yaml: is a link that leaves the pack folder\n");
    }

    #[test]
    fn a_pack_in_several_files_loads_and_a_clash_or_an_escape_does_not() {
        let rooted = "  name: Rooted
  content: {days: d/days.yaml, places: places.yaml}
";
        let dir = folder(
            "several",
            &[
                (
                    "pack.yaml",
                    &manifest("").replace(
                        "  content: content.yaml
",
                        rooted,
                    ),
                ),
                (
                    "d/days.yaml",
                    "- {date: 2026-04-11, title: A day, blocks: [['10:00', 'x']]}
",
                ),
                (
                    "places.yaml",
                    "home: {name: Home, at: {lat: 41.4, lon: 2.2}}
",
                ),
            ],
        );
        let (code, out, _) = validate_at(&dir);
        assert_eq!(
            (code, out.as_str()),
            (
                0,
                "
Rooted: loads. 0 warnings.
"
            )
        );

        let listed = "  content: [a.yaml, b.yaml]
";
        let dir = folder(
            "clash",
            &[
                (
                    "pack.yaml",
                    &manifest("").replace(
                        "  content: content.yaml
",
                        listed,
                    ),
                ),
                ("a.yaml", DAY),
                ("b.yaml", DAY),
            ],
        );
        let (code, _, err) = validate_at(&dir);
        assert_eq!(code, 1);
        assert!(err.starts_with("b.yaml: \"days\" is already defined"), "{err}");

        let escape = "  content: ../content.yaml
";
        let dir = folder(
            "escape",
            &[(
                "pack.yaml",
                &manifest("").replace(
                    "  content: content.yaml
",
                    escape,
                ),
            )],
        );
        let (code, _, err) = validate_at(&dir);
        assert_eq!(code, 1);
        assert!(
            err.starts_with("pack.content: \"../content.yaml\" leaves the pack folder"),
            "{err}"
        );
    }

    #[test]
    fn a_pack_with_errors_does_not_load() {
        let dir = folder(
            "broken",
            &[("pack.yaml", &manifest("  name: Broken\n")), ("content.yaml", "days: x\n")],
        );
        let (code, out, err) = validate_at(&dir);
        assert_eq!((code, err.as_str()), (1, ""));
        assert_eq!(
            out,
            "error  days: has to be a list, one entry per day\n\nBroken: does not load. 1 error, 0 warnings.\n"
        );
    }

    #[test]
    fn warnings_load_and_are_counted() {
        let content = "days:\n  - {date: 2026-04-11, title: x}\n  - {date: 2026-04-12, title: y}\n";
        let dir = folder(
            "warned",
            &[
                (
                    "pack.yaml",
                    &manifest(
                        "  name: Warned
",
                    ),
                ),
                ("content.yaml", content),
            ],
        );
        let (code, out, _) = validate_at(&dir);
        assert_eq!(code, 0);
        assert!(out.starts_with("warn   days.2026-04-11: no blocks"), "{out}");
        assert!(out.ends_with("\n\nWarned: loads. 2 warnings.\n"), "{out}");

        let bad = "days:\n  - {date: 2026-04-11}\n  - {date: 2026-04-12}\n";
        let dir =
            folder("warned-bad", &[("pack.yaml", &manifest("  name: ~\n")), ("content.yaml", bad)]);
        let (code, out, _) = validate_at(&dir);
        assert_eq!(code, 1);
        assert!(out.ends_with("\n\ntest: does not load. 3 errors, 2 warnings.\n"), "{out}");
    }

    #[test]
    fn a_clean_pack_loads_from_its_folder_or_its_manifest() {
        let dir =
            folder("clean", &[("pack.yaml", &manifest("  name: Clean\n")), ("content.yaml", DAY)]);
        let expected = (0, "\nClean: loads. 0 warnings.\n".to_owned(), String::new());
        assert_eq!(validate_at(&dir), expected);
        assert_eq!(validate_at(&dir.join("pack.yaml")), expected);
    }

    #[test]
    fn a_pack_with_no_name_or_id_goes_by_its_path() {
        let dir = folder("nameless", &[("m.yaml", "pack: {}\n")]);
        let (code, out, _) = validate_at(&dir.join("m.yaml"));
        assert_eq!(code, 1);
        let tail =
            format!("\n\n{}: does not load. 6 errors, 0 warnings.\n", dir.join("m.yaml").display());
        assert!(out.ends_with(&tail), "{out}");
    }

    #[test]
    fn one_warning_is_singular() {
        assert_eq!(count(1, "warning"), "1 warning");
        assert_eq!(count(0, "warning"), "0 warnings");
    }

    #[test]
    fn version_prints_the_screen_tree_version() {
        assert_eq!(call(&["version"]), (0, "tree 4\n".to_owned(), String::new()));
    }

    const MACHINE: &str = "modules:
  timeline:
screens:
  a:
    state: {n: null}
    actions:
      go: [{set: n, to: $arg}, {store: \"seen.{now.date}\", value: $arg}, map.open]
    layout:
      - Label: {text: \"{block.text} {n} {store.x}\", on_tap: go}
rules:
  - {screen: a}
";

    fn machine(name: &str) -> Folder {
        let pack = manifest(&format!("  name: M\n{MACHINE}"));
        folder(name, &[("pack.yaml", &pack), ("content.yaml", DAY)])
    }

    fn at(dir: &Path, command: &str, args: &[&str]) -> (u8, String, String) {
        let mut all = vec![command, dir.to_str().unwrap()];
        all.extend(args);
        call(&all)
    }

    #[test]
    fn screen_and_act_refuse_arguments_that_say_no_world() {
        let dir = machine("args");
        let cases: [(&str, &[&str], &str); 7] = [
            ("screen", &[], "--at <time> is needed: the moment to show, YYYY-MM-DDTHH:MM"),
            ("screen", &["--at"], "--at needs a value"),
            ("screen", &["--at", "x", "--store", "k"], "--store takes key=value, not k"),
            ("screen", &["--at", "x", "--when", "y"], "--when is not an option"),
            ("screen", &["--at", "x", "go"], "screen takes no actions: use act"),
            ("act", &["--at", "x"], "act needs an action"),
            ("act", &["go"], "--at <time> is needed: the moment to show, YYYY-MM-DDTHH:MM"),
        ];
        for (command, args, message) in cases {
            let expected = (2, String::new(), format!("{message}\n\n{USAGE}\n"));
            assert_eq!(at(&dir, command, args), expected, "{args:?}");
        }
    }

    #[test]
    fn screen_needs_a_pack_that_loads_and_a_real_time() {
        let (code, _, err) = call(&["screen", &temp("none").to_string_lossy(), "--at", "x"]);
        assert!(code == 2 && err.starts_with("no pack at"), "{err}");
        let pack = manifest("  name: U\nrules: 3\n");
        let dir = folder("unloadable", &[("pack.yaml", &pack), ("content.yaml", DAY)]);
        let message = format!("the pack does not load: deskpress validate {}\n", dir.display());
        assert_eq!(at(&dir, "screen", &["--at", "x"]), (1, String::new(), message));
        let message = "\"x\" is not a time: YYYY-MM-DDTHH:MM, in the pack's timezone\n".to_owned();
        assert_eq!(at(&machine("time"), "act", &["--at", "x", "go"]), (1, String::new(), message));
    }

    #[test]
    fn screen_prints_the_tree_and_its_watch_as_json() {
        let world = [
            "--at",
            "2026-04-11T10:30",
            "--store",
            "x=1",
            "--holder",
            "rita",
            "--inside",
            "",
            "--can",
            "assistant",
        ];
        let content = format!("{DAY}places:\n  home: {{at: {{lat: 1, lon: 2}}}}\n");
        let modules = MACHINE.replace("  timeline:\n", "  timeline:\n  places:\n");
        let pack = manifest(&format!("  name: M\n{modules}"));
        let dir = folder("screen", &[("pack.yaml", &pack), ("content.yaml", &content)]);
        let (code, out, err) = at(&dir, "screen", &world);
        let expected = r#"{"version": 4, "screen": "a", "nodes": [{"key": "0", "kind": "Label", "props": {"text": "x  1"}, "on": {"tap": "go"}, "children": []}], "theme": "", "kid": false, "watch": {"until": "2026-04-12T00:00", "regions": [{"id": "home", "lat": 1, "lon": 2, "radius_m": 100}]}}"#;
        assert_eq!((code, out.as_str(), err.as_str()), (0, format!("{expected}\n").as_str(), ""));
    }

    #[test]
    fn act_runs_actions_in_turn_and_prints_what_they_stored_and_asked_for() {
        let dir = machine("act");
        let (code, out, _) =
            at(&dir, "act", &["--at", "2026-04-11T10:30", "go=[a, b]", "go=hello"]);
        assert_eq!(code, 0);
        let tail = r#""props": {"text": "x hello "}, "on": {"tap": "go"}, "children": []}], "theme": "", "kid": false, "watch": {"until": "2026-04-12T00:00", "regions": []}, "store": {"seen.2026-04-11": "hello"}, "commands": [{"name": "map.open", "args": {}}, {"name": "map.open", "args": {}}]}"#;
        assert!(out.ends_with(&format!("{tail}\n")), "{out}");
        // What YAML cannot read goes as text.
        let (_, out, _) = at(&dir, "act", &["--at", "2026-04-11T10:30", "go={"]);
        assert!(out.contains(r#""text": "x { ""#), "{out}");
        let refused = at(&dir, "act", &["--at", "2026-04-11T10:30", "go", "stop"]);
        let message = "\"stop\" is not an action of the screen \"a\"\n".to_owned();
        assert_eq!(refused, (1, String::new(), message));
    }
}
