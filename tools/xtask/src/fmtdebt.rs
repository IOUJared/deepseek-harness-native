//! First-party Rust formatting-debt ratchet.
//!
//! Parts of this repository are not rustfmt-clean, and `AGENTS.md` forbids
//! rewriting all shared Rust files to resolve local formatting debt. This check
//! keeps the debt visible and measurable without silently waiving it:
//!
//! - It runs the same scoped `cargo fmt -- --check` as the verification runner,
//!   so the pinned third-party tree `app/vendor/winit` is never claimed as ours.
//! - It fails when a file that was clean becomes unformatted, or when a new file
//!   is added unformatted.
//! - Files already recorded in the baseline stay listed, with exact hunk counts,
//!   so the debt can only shrink deliberately.
//!
//! Update the baseline only for an intentional, reviewed change:
//! `xtask format-debt --update`.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const BASELINE: &str = "scripts/format-debt-baseline.json";
/// Scoped per-package checks; `--all` would pull the pinned vendor tree in.
pub const MANIFESTS: [&str; 4] = [
    "core/Cargo.toml",
    "transport/Cargo.toml",
    "app/Cargo.toml",
    "tools/xtask/Cargo.toml",
];

#[derive(Debug)]
pub struct Outcome {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug)]
pub struct Failure {
    pub manifest: String,
    pub code: i32,
    pub lines: Vec<String>,
}

pub fn baseline_path(root: &Path) -> PathBuf {
    root.join(BASELINE)
}

/// Map normalized first-party source paths to their rustfmt diff-hunk count.
pub fn parse_diff_paths(text: &str, root: &Path) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for line in text.lines() {
        let Some((path, _line)) = diff_header(line.trim()) else {
            continue;
        };
        let normalized = normalize(path);
        let name = match Path::new(&normalized).strip_prefix(root) {
            Ok(relative) => relative.to_string_lossy().into_owned(),
            Err(_) => normalized,
        };
        *counts.entry(name).or_insert(0) += 1;
    }
    counts
}

/// One `Diff in <path>:<line>:` header. The line number is not the count: a file
/// with two hunks produces two headers, which is what the ratchet records.
fn diff_header(line: &str) -> Option<(&str, usize)> {
    let body = line.strip_prefix("Diff in ")?.strip_suffix(':')?;
    let (path, number) = body.rsplit_once(':')?;
    let line: usize = number.parse().ok()?;
    Some((path, line))
}

/// POSIX `os.path.normpath`: collapse `.` and `//`, resolve `..` lexically, keep a
/// leading `/`, and answer `.` for an empty relative path.
pub fn normalize(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => match parts.last() {
                Some(previous) if *previous != ".." => {
                    parts.pop();
                }
                _ if absolute => {}
                _ => parts.push(".."),
            },
            part => parts.push(part),
        }
    }
    let joined = parts.join("/");
    match (absolute, joined.is_empty()) {
        (true, _) => format!("/{joined}"),
        (false, true) => ".".to_string(),
        (false, false) => joined,
    }
}

/// Run the scoped check per first-party package and collect path -> hunk counts.
pub fn collect(
    root: &Path,
    manifests: &[&str],
) -> io::Result<(BTreeMap<String, usize>, Vec<Failure>)> {
    let mut counts = BTreeMap::new();
    let mut failures = Vec::new();
    for manifest in manifests {
        let output = Command::new("cargo")
            .args(["fmt", "--manifest-path", manifest, "--", "--check"])
            .current_dir(root)
            .output()?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let found = parse_diff_paths(&text, root);
        let code = output.status.code().unwrap_or(-1);
        if !matches!(code, 0 | 1) && found.is_empty() {
            let lines = text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .take(3)
                .map(str::to_string)
                .collect();
            failures.push(Failure {
                manifest: (*manifest).to_string(),
                code,
                lines,
            });
        }
        counts.extend(found);
    }
    Ok((counts, failures))
}

#[derive(Deserialize, Serialize)]
struct BaselineFile {
    note: String,
    files: Vec<String>,
}

pub fn load_baseline(path: &Path) -> io::Result<Vec<String>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let parsed: BaselineFile = serde_json::from_str(&fs::read_to_string(path)?)
        .map_err(|error| io::Error::other(format!("invalid formatting-debt baseline: {error}")))?;
    Ok(parsed.files)
}

pub fn write_baseline(files: &BTreeMap<String, usize>, path: &Path) -> io::Result<()> {
    let baseline = BaselineFile {
        note: "First-party Rust files that are not rustfmt-clean. Recorded debt, \
               not approval: new files must not be added here without review."
            .to_string(),
        files: files.keys().cloned().collect(),
    };
    let rendered = serde_json::to_string_pretty(&baseline)
        .map_err(|error| io::Error::other(error.to_string()))?;
    fs::write(path, rendered + "\n")
}

/// Return (new offenders, resolved baseline entries) as sorted lists.
pub fn compare(
    current: &BTreeMap<String, usize>,
    baseline: &[String],
) -> (Vec<String>, Vec<String>) {
    let mut new: Vec<String> = current
        .keys()
        .filter(|name| !baseline.contains(name))
        .cloned()
        .collect();
    let mut resolved: Vec<String> = baseline
        .iter()
        .filter(|name| !current.contains_key(*name))
        .cloned()
        .collect();
    new.sort();
    resolved.sort();
    (new, resolved)
}

/// Evaluate the ratchet. `update` records the current state instead of grading it.
pub fn evaluate(root: &Path, update: bool, baseline: &Path) -> io::Result<Outcome> {
    let (current, failures) = collect(root, &MANIFESTS)?;
    let mut stderr = String::new();
    for failure in &failures {
        stderr.push_str(&format!(
            "cargo fmt could not run for {} (exit {}): {}\n",
            failure.manifest,
            failure.code,
            if failure.lines.is_empty() {
                String::new()
            } else {
                failure.lines.join(" / ")
            }
        ));
    }
    if !failures.is_empty() {
        return Ok(Outcome {
            code: 2,
            stdout: String::new(),
            stderr,
        });
    }
    if update {
        write_baseline(&current, baseline)?;
        return Ok(Outcome {
            code: 0,
            stdout: format!(
                "Recorded {} first-party file(s) in formatting-debt baseline.\n",
                current.len()
            ),
            stderr,
        });
    }

    let (new, resolved) = compare(&current, &load_baseline(baseline)?);
    let hunks: usize = current.values().sum();
    let mut stdout = format!(
        "First-party formatting debt: {} file(s), {hunks} rustfmt diff hunk(s).\n",
        current.len()
    );
    if !resolved.is_empty() {
        stdout.push_str(&format!(
            "Resolved since the baseline (remove with --update): {} file(s).\n",
            resolved.len()
        ));
    }
    if !new.is_empty() {
        stdout.push_str("New unformatted file(s) — run cargo fmt for the owning package:\n");
        for name in &new {
            stdout.push_str(&format!("  {name}\n"));
        }
        return Ok(Outcome {
            code: 1,
            stdout,
            stderr,
        });
    }
    stdout.push_str("No new formatting debt.\n");
    Ok(Outcome {
        code: 0,
        stdout,
        stderr,
    })
}

/// CLI entry: `xtask format-debt [--update] [--baseline PATH] [--root PATH]`.
pub fn run(arguments: &[String]) -> i32 {
    let mut update = false;
    let mut baseline: Option<PathBuf> = None;
    let mut root: Option<PathBuf> = None;
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--update" => update = true,
            "--baseline" => match arguments.next() {
                Some(value) => baseline = Some(PathBuf::from(value)),
                None => return rejected("--baseline needs a value"),
            },
            "--root" => match arguments.next() {
                Some(value) => root = Some(PathBuf::from(value)),
                None => return rejected("--root needs a value"),
            },
            other => return rejected(&format!("unexpected argument: {other}")),
        }
    }
    let root = root.unwrap_or_else(crate::root);
    let baseline = baseline.unwrap_or_else(|| baseline_path(&root));
    match evaluate(&root, update, &baseline) {
        Ok(outcome) => {
            print!("{}", outcome.stdout);
            eprint!("{}", outcome.stderr);
            outcome.code
        }
        Err(error) => {
            eprintln!("format-debt could not evaluate the tree: {error}");
            2
        }
    }
}

fn rejected(message: &str) -> i32 {
    eprintln!("usage: xtask format-debt [--update] [--baseline PATH] [--root PATH]\n{message}");
    2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempdir::make_temp_dir;

    const ROOT: &str = "/repo";

    fn counts(entries: &[(&str, usize)]) -> BTreeMap<String, usize> {
        entries
            .iter()
            .map(|(name, hunks)| ((*name).to_string(), *hunks))
            .collect()
    }

    #[test]
    fn a_rustfmt_diff_hunk_is_counted_per_file() {
        let text = "Diff in /repo/app/src/ui.rs:12:\n+let x = 1;\n\
                    Diff in /repo/app/src/chat.rs:40:\n+let y = 2;\n";
        let found = parse_diff_paths(text, Path::new(ROOT));
        assert_eq!(
            found,
            counts(&[("app/src/ui.rs", 1), ("app/src/chat.rs", 1)])
        );
    }

    #[test]
    fn a_repeated_hunk_in_one_file_counts_twice() {
        let text = "Diff in /repo/app/src/ui.rs:12:\nDiff in /repo/app/src/ui.rs:90:\n";
        assert_eq!(
            parse_diff_paths(text, Path::new(ROOT)),
            counts(&[("app/src/ui.rs", 2)])
        );
    }

    #[test]
    fn an_absolute_diff_path_becomes_a_repository_relative_name() {
        let found = parse_diff_paths("Diff in /repo/core/src/lib.rs:3:\n", Path::new(ROOT));
        assert_eq!(found.keys().next().unwrap(), "core/src/lib.rs");
    }

    #[test]
    fn a_path_outside_the_repository_keeps_its_absolute_form() {
        let found = parse_diff_paths(
            "Diff in /elsewhere/vendor/winit/src/lib.rs:3:\n",
            Path::new(ROOT),
        );
        assert_eq!(
            found.keys().next().unwrap(),
            "/elsewhere/vendor/winit/src/lib.rs"
        );
    }

    #[test]
    fn non_diff_output_is_ignored() {
        let text = "Checking formatting...\nwarning: unused\nDiff in bad-line\n";
        assert!(parse_diff_paths(text, Path::new(ROOT)).is_empty());
    }

    #[test]
    fn a_normalized_path_collapses_dot_segments_and_keeps_the_root() {
        assert_eq!(
            normalize("/repo/./app/../app/src/ui.rs"),
            "/repo/app/src/ui.rs"
        );
        assert_eq!(normalize("app//src/./ui.rs"), "app/src/ui.rs");
        assert_eq!(normalize("./app"), "app");
        assert_eq!(normalize(""), ".");
        assert_eq!(normalize("/"), "/");
        assert_eq!(normalize("../sibling"), "../sibling");
    }

    #[test]
    fn an_entry_missing_from_the_baseline_is_a_new_offender() {
        let current = counts(&[("app/src/ui.rs", 2), ("core/src/lib.rs", 1)]);
        let baseline = vec!["app/src/ui.rs".to_string()];
        let (new, resolved) = compare(&current, &baseline);
        assert_eq!(new, ["core/src/lib.rs"]);
        assert!(resolved.is_empty());
    }

    #[test]
    fn a_baseline_entry_that_became_clean_is_reported_as_resolved() {
        let current = counts(&[("app/src/ui.rs", 1)]);
        let baseline = vec!["app/src/ui.rs".to_string(), "app/src/chat.rs".to_string()];
        let (new, resolved) = compare(&current, &baseline);
        assert!(new.is_empty());
        assert_eq!(resolved, ["app/src/chat.rs"]);
    }

    #[test]
    fn an_update_records_only_the_current_files_in_sorted_order() {
        let directory = make_temp_dir(&std::env::temp_dir(), "baseline-").unwrap();
        let path = directory.join("baseline.json");
        write_baseline(&counts(&[("b.rs", 1), ("a.rs", 1)]), &path).unwrap();
        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["files"], serde_json::json!(["a.rs", "b.rs"]));
        assert!(written["note"].as_str().unwrap().contains("Recorded debt"));
        assert_eq!(load_baseline(&path).unwrap(), ["a.rs", "b.rs"]);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn an_absent_baseline_reads_as_an_empty_allowance() {
        let directory = make_temp_dir(&std::env::temp_dir(), "baseline-absent-").unwrap();
        assert!(
            load_baseline(&directory.join("missing.json"))
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
