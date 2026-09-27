use std::process::ExitCode;

// Returns instead of calling `process::exit`, so buffers flush and coverage is written.
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(deskpress_cli::run(&args, &mut std::io::stdout(), &mut std::io::stderr()))
}
