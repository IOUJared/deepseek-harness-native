//! The launcher an extracted development package runs as `./launch`.
//!
//! The command-line surface is `launcher::main`, shared with `xtask launch`; this
//! binary exists so the archive ships a launcher that needs no subcommand and no
//! Python. It resolves the package root from its own location, which is the payload
//! root beside `development-manifest.json` and `bin/dsh-native-app`.
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(dsh_xtask::launcher::main(&arguments, None).clamp(0, 255) as u8)
}
