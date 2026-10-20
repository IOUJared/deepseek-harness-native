//! `xtask` — the repository's development predicates, one toolchain.
use std::process::ExitCode;

const USAGE: &str = "\
usage: xtask <command>

commands:
  verify        Serial headless checks: doctor, clippy, lint, examples, test, all
  format-debt   First-party Rust formatting-debt ratchet
  help          This message
";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let code = match arguments.first().map(String::as_str) {
        Some("verify") => dsh_xtask::verify::run(&arguments[1..]),
        Some("format-debt") => dsh_xtask::fmtdebt::run(&arguments[1..]),
        Some("help" | "--help" | "-h") | None => {
            print!("{USAGE}");
            0
        }
        Some(other) => {
            eprintln!("unknown command: {other}\n\n{USAGE}");
            2
        }
    };
    ExitCode::from(code.clamp(0, 255) as u8)
}
