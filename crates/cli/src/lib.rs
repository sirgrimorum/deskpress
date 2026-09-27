//! The engine on a desk. `run` takes its output streams, so every path is a unit test.

use std::io::Write;
use std::path::Path;

use deskpress_engine::{pack, validate};

pub const USAGE: &str = "usage: deskpress <command>

commands:
  validate <pack>   check a pack folder, or its pack.yaml, and list every problem
  version           print the screen tree version";

/// Runs one command and returns the exit code.
pub fn run(args: &[String], out: &mut impl Write, err: &mut impl Write) -> u8 {
    let (code, stdout, stderr) = match args {
        [command] if command == "version" => {
            (0, format!("tree {}\n", deskpress_engine::TREE_VERSION), String::new())
        }
        [command, target] if command == "validate" => validate(target),
        _ => (2, String::new(), format!("{USAGE}\n")),
    };
    // A closed pipe is not worth a panic.
    out.write_all(stdout.as_bytes())
        .and_then(|()| err.write_all(stderr.as_bytes()))
        .map_or(1, |()| code)
}

/// `deskpress validate`: exit 0 when the pack loads, 1 when it does not, 2 when there is no pack.
fn validate(target: &str) -> (u8, String, String) {
    let Ok(path) = Path::new(target).canonicalize() else {
        let message =
            format!("no pack at {target}. Point me at a pack folder or at its pack.yaml\n");
        return (2, String::new(), message);
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
    let pack = match pack::load(&manifest, read) {
        Ok(pack) => pack,
        Err(e) => return (1, String::new(), format!("{e}\n")),
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
        assert_eq!(call(&["version"]), (0, "tree 1\n".to_owned(), String::new()));
    }
}
