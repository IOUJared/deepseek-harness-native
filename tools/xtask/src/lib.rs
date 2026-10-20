//! Development predicates for the native harness.
//!
//! These checks were owned by Python scripts in `scripts/` and `benchmarks/`.
//! They live here so the repository has one toolchain: a checkout with Rust can
//! run every predicate without a Python interpreter, and the report shapes stay
//! compatible with the reports those scripts wrote (see [`verify::CheckResult`]).
pub mod archive;
pub mod bench;
pub mod fmtdebt;
pub mod idle;
pub mod json;
pub mod proc;
pub mod snapshot;
pub mod tempdir;
pub mod verify;

use sha2::{Digest, Sha256};
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

/// Repository root, anchored the way `Path(__file__).parents[1]` was: the crate
/// lives at `<root>/tools/xtask`, so two parents up is the checkout.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under <root>/tools/xtask")
        .to_path_buf()
}

/// SHA-256 of a file's body, as the Python report fields named it.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// A signalled child reports a negative code, matching Python's `returncode`.
pub fn exit_code(status: ExitStatus) -> i32 {
    status
        .code()
        .or_else(|| status.signal().map(|signal| -signal))
        .unwrap_or(0)
}
