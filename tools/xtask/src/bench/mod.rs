//! Prototype benchmark measurement: run owned targets or summarize finished runs.
pub mod analyze;
pub mod runner;

const USAGE: &str = "\
usage: xtask bench <command>

commands:
  run       Measure owned benchmark trees: --app LABEL=PATH --scale S --output NEW_DIR
  analyze   Summarize finished results: analyze RESULTS.json [--markdown]
";

pub fn run(arguments: &[String]) -> i32 {
    match arguments.first().map(String::as_str) {
        Some("run") => runner::run(&arguments[1..]),
        Some("analyze") => analyze::run(&arguments[1..]),
        Some("help" | "--help" | "-h") | None => {
            print!("{USAGE}");
            0
        }
        Some(other) => {
            eprintln!("unknown bench command: {other}\n\n{USAGE}");
            2
        }
    }
}
