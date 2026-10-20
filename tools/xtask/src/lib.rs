//! Development predicates for the native harness.
//!
//! These checks were owned by `scripts/verify-native.py` and
//! `scripts/format-debt.py`. They live here so the repository has one toolchain:
//! a checkout with Rust can run every predicate without a Python interpreter,
//! and the report shapes stay byte-compatible with the reports those scripts
//! wrote (see [`verify::CheckResult`]).
pub mod fmtdebt;
pub mod json;
pub mod snapshot;
pub mod tempdir;
pub mod verify;

use std::path::{Path, PathBuf};

/// Repository root, anchored the way `Path(__file__).parents[1]` was: the crate
/// lives at `<root>/tools/xtask`, so two parents up is the checkout.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under <root>/tools/xtask")
        .to_path_buf()
}
