//! The binary itself: arguments in, exit code and streams out.

use std::process::Command;

fn deskpress(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_deskpress")).args(args).output().unwrap()
}

#[test]
fn a_bad_command_exits_two_with_nothing_on_stdout() {
    let output = deskpress(&["explode"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("usage:"));
}

#[test]
fn version_exits_zero() {
    let output = deskpress(&["--version"]);
    assert_eq!(output.status.code(), Some(0));
    let expected = format!("deskpress {}, tree 4\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
}

#[test]
fn validate_passes_the_example_pack() {
    let example = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/one-day");
    let output = deskpress(&["validate", example]);
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stdout));
    assert!(String::from_utf8_lossy(&output.stdout).ends_with(": loads. 0 warnings.\n"));
}

#[test]
fn preview_opens_an_answer_in_the_example_pack() {
    let example = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/one-day");
    let output = deskpress(&["preview", example, "--at", "2026-04-11T11:30", "ask", "show=now"]);
    let out = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "{out}");
    assert!(
        out.starts_with("# Ask about the trip")
            && out.contains("\n> What is happening now? · 11:15 "),
        "{out}"
    );
}
