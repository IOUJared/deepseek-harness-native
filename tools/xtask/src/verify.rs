//! Serial, headless native checks with retained outcomes and source-drift detection.
//!
//! Owns what `scripts/verify-native.py` owned: the exact command plan for each
//! suite, one-in-process guards (`binary-freshness`, `format-debt`), per-check
//! private logs, and a report whose keys match the Python runner's. Two checks
//! run in-process rather than as argv commands. `binary-freshness` compares each
//! *present* release artifact against the first-party sources that feed it: a
//! suite that passes while the launched binary predates the fix is how a report
//! misleads, so the guard fails instead. An absent artifact is not a failure. A
//! checkout that never built the release binary has nothing to launch.
use crate::fmtdebt;
use crate::snapshot::{Snapshot, source_snapshot};
use crate::tempdir::make_temp_dir;
use serde::Serialize;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

pub const SKILL: &str = ".agents/skills/impeccable-rust";
pub const SUITES: [&str; 6] = ["doctor", "clippy", "lint", "examples", "test", "all"];
/// First-party inputs every present release artifact must be newer than. The
/// comparison is modification time, not a rebuild claim: a comment-only edit
/// still requires the artifact to be rebuilt before it can be reported as verified.
pub const SOURCE_ROOTS: [&str; 5] = [
    "app/src",
    "app/examples",
    "app/fixtures",
    "core/src",
    "transport/src",
];
pub const SOURCE_FILES: [&str; 4] = [
    "app/Cargo.toml",
    "app/Cargo.lock",
    "core/Cargo.toml",
    "transport/Cargo.toml",
];
/// Only artifacts the documented flow builds and a report relies on: the launchable
/// deliverable (app/README.md) and the fixture that produces every rendered layout
/// capture. A debug build or a one-off release example is incidental.
pub const BUILT_ARTIFACTS: [&str; 2] = [
    "app/target/release/dsh-native-app",
    "app/target/release/examples/layout_smoke",
];
pub const FRESHNESS_ARGV: [&str; 2] = ["internal", "binary-freshness"];
pub const FMTDEBT_ARGV: [&str; 2] = ["internal", "format-debt"];
/// Clippy denies only unfinished macros and implicit unsafe operations; other
/// warnings stay visible by policy, so no suite here denies all baseline warnings.
const CLIPPY_DENIALS: [&str; 3] = [
    "clippy::todo",
    "clippy::unimplemented",
    "unsafe_op_in_unsafe_fn",
];
const MIN_TIMEOUT_SECONDS: u64 = 1;
const MAX_TIMEOUT_SECONDS: u64 = 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suite {
    Doctor,
    Clippy,
    Lint,
    Examples,
    Test,
    All,
}

impl Suite {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "doctor" => Some(Self::Doctor),
            "clippy" => Some(Self::Clippy),
            "lint" => Some(Self::Lint),
            "examples" => Some(Self::Examples),
            "test" => Some(Self::Test),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Doctor => "doctor",
            Self::Clippy => "clippy",
            Self::Lint => "lint",
            Self::Examples => "examples",
            Self::Test => "test",
            Self::All => "all",
        }
    }

    fn builds_artifacts(self) -> bool {
        matches!(self, Self::Test | Self::All)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub name: String,
    pub command: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Status {
    #[serde(rename = "passed")]
    Passed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "timed-out")]
    TimedOut,
    #[serde(rename = "unavailable")]
    Unavailable,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::TimedOut => "timed-out",
            Self::Unavailable => "unavailable",
        }
    }
}

/// One check outcome. Field names and the negative `exitCode` of a signalled child
/// match the Python runner's report, so evidence stays comparable.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub name: String,
    pub command: Vec<String>,
    pub status: Status,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub signal: Option<i32>,
    pub seconds: f64,
    pub log: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub present_artifacts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale_artifacts: Option<Vec<String>>,
}

impl CheckResult {
    fn new(name: &str, command: &[String], started: Instant, log: String) -> Self {
        Self {
            name: name.to_string(),
            command: command.to_vec(),
            status: Status::Failed,
            exit_code: None,
            timed_out: false,
            signal: None,
            seconds: millis(started),
            log,
            present_artifacts: None,
            stale_artifacts: None,
        }
    }
}

fn millis(started: Instant) -> f64 {
    (started.elapsed().as_secs_f64() * 1000.0).round() / 1000.0
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_string()).collect()
}

fn locked(offline: bool) -> Vec<String> {
    let mut flags = vec!["--locked".to_string()];
    if offline {
        flags.push("--offline".to_string());
    }
    flags
}

/// The explicit command plan for one suite. Never a shell expression and never a
/// live GUI, model or provider invocation.
pub fn commands(root: &Path, suite: Suite, offline: bool) -> Vec<Check> {
    match suite {
        Suite::Doctor => {
            let doctor = root
                .join(SKILL)
                .join("scripts/impeccable")
                .to_string_lossy()
                .into_owned();
            vec![Check {
                name: "tool-doctor".into(),
                command: argv(&["bash", &doctor, "doctor"]),
            }]
        }
        Suite::Examples => examples(offline),
        Suite::Clippy => clippy(offline),
        Suite::Lint => {
            let mut checks = vec![Check {
                name: "format-debt".into(),
                command: argv(&FMTDEBT_ARGV),
            }];
            checks.extend(clippy(offline));
            checks
        }
        Suite::Test => tests(root, offline),
        Suite::All => {
            let mut checks = vec![Check {
                name: "format-debt".into(),
                command: argv(&FMTDEBT_ARGV),
            }];
            checks.extend(clippy(offline));
            checks.extend(tests(root, offline));
            checks
        }
    }
}

fn examples(offline: bool) -> Vec<Check> {
    ["layout_smoke", "scroll_app_smoke"]
        .into_iter()
        .map(|name| {
            let mut command = argv(&["cargo", "test", "--manifest-path", "app/Cargo.toml"]);
            command.extend(locked(offline));
            command.extend(argv(&[
                "--features",
                "public-layout-fixture",
                "--example",
                name,
                "--",
                "--test-threads=2",
            ]));
            Check {
                name: name.to_string(),
                command,
            }
        })
        .collect()
}

fn clippy(offline: bool) -> Vec<Check> {
    // First-party crates only; the pinned `app/vendor/winit` tree is never claimed as ours.
    [
        ("core", "core/Cargo.toml"),
        ("transport", "transport/Cargo.toml"),
        ("app", "app/Cargo.toml"),
        ("xtask", "tools/xtask/Cargo.toml"),
    ]
    .into_iter()
    .map(|(name, manifest)| {
        let mut command = argv(&["cargo", "clippy", "--manifest-path", manifest]);
        command.extend(locked(offline));
        command.extend(argv(&["--all-targets", "--all-features", "--"]));
        for denial in CLIPPY_DENIALS {
            command.push("-D".to_string());
            command.push(denial.to_string());
        }
        Check {
            name: format!("{name}-clippy"),
            command,
        }
    })
    .collect()
}

fn tests(root: &Path, offline: bool) -> Vec<Check> {
    let flagged = |name: &str, manifest: &str, flags: &[&str]| {
        let mut command = argv(&["cargo", "test", "--manifest-path", manifest]);
        command.extend(locked(offline));
        command.extend(argv(flags));
        command.extend(argv(&["--", "--test-threads=2"]));
        Check {
            name: name.to_string(),
            command,
        }
    };
    let mut checks = vec![
        flagged("core", "core/Cargo.toml", &[]),
        flagged(
            "core-transport",
            "core/Cargo.toml",
            &["--features", "transport"],
        ),
        flagged("transport", "transport/Cargo.toml", &[]),
        flagged("app", "app/Cargo.toml", &["--bin", "dsh-native-app"]),
        flagged(
            "app-public",
            "app/Cargo.toml",
            &[
                "--features",
                "public-layout-fixture",
                "--bin",
                "dsh-native-app",
            ],
        ),
    ];
    let mut wayland_drops = argv(&["cargo", "test", "--manifest-path", "app/Cargo.toml"]);
    wayland_drops.extend(locked(offline));
    wayland_drops.extend(argv(&[
        "-p",
        "winit",
        "--lib",
        "data_device",
        "--",
        "--test-threads=2",
    ]));
    checks.push(Check {
        name: "wayland-drops".into(),
        command: wayland_drops,
    });
    checks.extend(examples(offline));
    // The runner's own guards are Rust tests now, so the suite that used to run
    // them through `python3 -m unittest` runs them through Cargo.
    let mut xtask_guards = argv(&["cargo", "test", "--manifest-path", "tools/xtask/Cargo.toml"]);
    xtask_guards.extend(locked(offline));
    xtask_guards.extend(argv(&["--", "--test-threads=2"]));
    checks.push(Check {
        name: "xtask-guards".into(),
        command: xtask_guards,
    });
    checks.push(Check {
        name: "bridge".into(),
        command: bridge(root),
    });
    // These three still discover the Python guards they have not replaced yet;
    // each owner moves to a `cargo test` command as its family is ported.
    // `benchmarks/test_runner.py` guards `benchmarks/run.py`, which the
    // qualification scripts still import as their metrics library; it moves to
    // `xtask-guards` when that family is ported. `app/` has no Python test left
    // (the history guards are `xtask history` tests), so it needs no discovery.
    for (name, directory, pattern) in [
        ("script-guards", "scripts", "test_*.py"),
        ("benchmark-guards", "benchmarks", "test_*.py"),
        ("release-guards", ".github/scripts", "test_*.py"),
    ] {
        checks.push(Check {
            name: name.into(),
            command: argv(&[
                "python3", "-B", "-m", "unittest", "discover", "-s", directory, "-p", pattern,
            ]),
        });
    }
    checks
}

fn bridge(root: &Path) -> Vec<String> {
    // Python sorted the globbed paths before handing them to Node.
    let mut paths: Vec<String> = fs::read_dir(root.join("core/runtime"))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    name.ends_with(".test.mjs")
                        .then(|| format!("core/runtime/{name}"))
                })
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    let mut command = argv(&["node", "--test"]);
    command.extend(paths);
    command
}

/// Newest modification time among the first-party inputs of the release artifacts.
pub fn newest_source_mtime(root: &Path) -> io::Result<SystemTime> {
    let mut newest = SystemTime::UNIX_EPOCH;
    for name in SOURCE_ROOTS {
        newest_of(&root.join(name), &mut newest)?;
    }
    for name in SOURCE_FILES {
        let path = root.join(name);
        if let Ok(metadata) = fs::metadata(&path) {
            newest = newest.max(metadata.modified()?);
        }
    }
    Ok(newest)
}

fn newest_of(path: &Path, newest: &mut SystemTime) -> io::Result<()> {
    let Ok(entries) = fs::read_dir(path) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            newest_of(&entry.path(), newest)?;
        } else if metadata.is_file() {
            *newest = (*newest).max(metadata.modified()?);
        }
    }
    Ok(())
}

/// Return the present artifacts and those older than their sources.
pub fn freshness_findings(root: &Path) -> io::Result<(Vec<String>, Vec<String>)> {
    let newest = newest_source_mtime(root)?;
    let mut built = Vec::new();
    let mut stale = Vec::new();
    for name in BUILT_ARTIFACTS {
        let path = root.join(name);
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        built.push(name.to_string());
        if metadata.modified()? < newest {
            stale.push(name.to_string());
        }
    }
    Ok((built, stale))
}

/// Fail when a present release artifact is older than the sources it embeds.
pub fn run_freshness_check(root: &Path, output: &Path) -> io::Result<CheckResult> {
    let started = Instant::now();
    let command = argv(&FRESHNESS_ARGV);
    let log = output.join("binary-freshness.log");
    let (built, stale) = freshness_findings(root)?;
    let mut lines = vec![format!(
        "present artifacts: {}",
        if built.is_empty() {
            "none".to_string()
        } else {
            built.join(", ")
        }
    )];
    let mut result = CheckResult::new(
        "binary-freshness",
        &command,
        started,
        log.to_string_lossy().into_owned(),
    );
    result.present_artifacts = Some(built.clone());
    result.stale_artifacts = Some(stale.clone());
    if stale.is_empty() {
        lines.push("every present artifact is at least as new as its sources".into());
        result.status = Status::Passed;
        result.exit_code = Some(0);
    } else {
        lines.push("older than their first-party sources:".into());
        lines.extend(stale.iter().map(|name| format!("  - {name}")));
        lines.push(
            "rebuild before reporting, or the report describes code you cannot launch:".into(),
        );
        lines.push(
            "  cargo build --release --manifest-path app/Cargo.toml --bin dsh-native-app".into(),
        );
        lines.push(
            "  cargo build --release --manifest-path app/Cargo.toml --example layout_smoke \
             --features public-layout-fixture"
                .into(),
        );
        lines.push(
            "a failing check here is not a test failure: nothing was rebuilt or rerun".into(),
        );
        result.status = Status::Failed;
        result.exit_code = Some(1);
    }
    write_private(&log, &(lines.join("\n") + "\n"))?;
    result.seconds = millis(started);
    Ok(result)
}

/// Retain failed and timed-out attempts; never rerun or silently skip a command.
pub fn run_check(
    root: &Path,
    name: &str,
    command: &[String],
    output: &Path,
    env: &BTreeMap<OsString, OsString>,
    timeout: Duration,
) -> io::Result<CheckResult> {
    let started = Instant::now();
    let log = output.join(format!("{name}.log"));
    let mut result = CheckResult::new(name, command, started, log.to_string_lossy().into_owned());
    let mut stream = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&log)?;
    let (program, arguments) = command
        .split_first()
        .ok_or_else(|| io::Error::other("empty command"))?;
    let spawned = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .env_clear()
        .envs(env)
        .stdout(Stdio::from(stream.try_clone()?))
        .stderr(Stdio::from(stream.try_clone()?))
        // Own process group: a timeout kills the whole group, as `start_new_session` did.
        .process_group(0)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            writeln!(stream, "Cannot execute command: {error}")?;
            result.status = Status::Unavailable;
            result.seconds = millis(started);
            return Ok(result);
        }
    };
    let deadline = started + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            // Not reaped yet, so its pid is still this group's id.
            kill_group(child.id());
            result.timed_out = true;
            break child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    // Python's `wait()` reports a signalled child as a negative code, e.g. -9.
    let signal = status.signal();
    result.signal = signal;
    result.exit_code = Some(
        status
            .code()
            .or_else(|| signal.map(|signal| -signal))
            .unwrap_or(0),
    );
    result.status = match signal {
        _ if result.timed_out => Status::TimedOut,
        Some(_) => Status::Failed,
        None if result.exit_code == Some(0) => Status::Passed,
        None => Status::Failed,
    };
    result.seconds = millis(started);
    Ok(result)
}

fn kill_group(pid: u32) {
    // Safe: SIGKILL to a process group this run created and has not reaped.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}

pub fn write_private(path: &Path, contents: &str) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents.as_bytes())
}

/// The environment every child check sees: no ambient credentials, Cargo serialized
/// at two jobs, no automatic toolchain install, offline unless explicitly allowed.
pub fn check_environment(cargo_home: Option<&Path>, offline: bool) -> BTreeMap<OsString, OsString> {
    let mut env: BTreeMap<OsString, OsString> = std::env::vars_os()
        .filter(|(name, _)| {
            let uppercase = name.to_string_lossy().to_uppercase();
            !["KEY", "SECRET", "TOKEN", "PASSWORD"]
                .iter()
                .any(|word| uppercase.contains(word))
        })
        .collect();
    env.insert("CARGO_BUILD_JOBS".into(), "2".into());
    env.insert("RUSTUP_AUTO_INSTALL".into(), "0".into());
    if let Some(home) = cargo_home {
        env.insert("CARGO_HOME".into(), home.as_os_str().to_owned());
    }
    if offline {
        env.insert("CARGO_NET_OFFLINE".into(), "true".into());
    }
    env
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    suite: Suite,
    plan: bool,
    online: bool,
    cargo_home: Option<PathBuf>,
    timeout: u64,
}

impl Options {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let mut options = Self {
            suite: Suite::Doctor,
            plan: false,
            online: false,
            cargo_home: None,
            timeout: 900,
        };
        let mut arguments = arguments.iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--suite" => {
                    let value = arguments.next().ok_or("--suite needs a value")?;
                    options.suite =
                        Suite::parse(value).ok_or_else(|| format!("unknown suite: {value}"))?;
                }
                "--plan" => options.plan = true,
                "--online" => options.online = true,
                "--cargo-home" => {
                    let value = arguments.next().ok_or("--cargo-home needs a value")?;
                    let path = PathBuf::from(value);
                    if !path.is_absolute() || !path.is_dir() {
                        return Err("--cargo-home must be an existing absolute directory".into());
                    }
                    options.cargo_home = Some(path);
                }
                "--timeout" => {
                    let value = arguments.next().ok_or("--timeout needs a value")?;
                    let seconds: u64 = value
                        .parse()
                        .map_err(|_| format!("--timeout is not a number: {value}"))?;
                    if !(MIN_TIMEOUT_SECONDS..=MAX_TIMEOUT_SECONDS).contains(&seconds) {
                        return Err("--timeout must be 1-3600".into());
                    }
                    options.timeout = seconds;
                }
                other => return Err(format!("unexpected argument: {other}")),
            }
        }
        Ok(options)
    }
}

fn usage() -> &'static str {
    "usage: xtask verify [--suite doctor|clippy|lint|examples|test|all] [--plan] \
     [--online] [--cargo-home ABS_DIR] [--timeout 1-3600]"
}

/// Run one suite from argv; returns the process exit code (0 passed, 1 failed or
/// drifted, 2 rejected arguments).
pub fn run(arguments: &[String]) -> i32 {
    let options = match Options::parse(arguments) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{}\n{message}", usage());
            return 2;
        }
    };
    let root = crate::root();
    let offline = !options.online;
    let plan = commands(&root, options.suite, offline);
    if options.plan {
        let mut checks = plan.clone();
        if options.suite.builds_artifacts() {
            checks.insert(
                0,
                Check {
                    name: "binary-freshness".into(),
                    command: argv(&FRESHNESS_ARGV),
                },
            );
        }
        let planned = Planned {
            suite: options.suite.name(),
            offline,
            checks,
        };
        match serde_json::to_string_pretty(&planned) {
            Ok(rendered) => println!("{rendered}"),
            Err(error) => {
                eprintln!("cannot render the plan: {error}");
                return 1;
            }
        }
        return 0;
    }

    let evidence = root.join("app/evidence");
    // Private, like the `mkdir(mode=0o700, exist_ok=True)` this replaced.
    match DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&evidence)
    {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            eprintln!("cannot create the evidence directory: {error}");
            return 1;
        }
    }
    let output = match make_temp_dir(&evidence, "verification-") {
        Ok(output) => output,
        Err(error) => {
            eprintln!("cannot create a private evidence directory: {error}");
            return 1;
        }
    };
    let env = check_environment(options.cargo_home.as_deref(), offline);
    let before = match source_snapshot(&root) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("cannot snapshot the sources: {error}");
            return 1;
        }
    };
    let mut toolchain = Vec::new();
    for (name, command) in [
        ("rustc-version", argv(&["rustc", "-vV"])),
        ("cargo-version", argv(&["cargo", "--version"])),
        ("node-version", argv(&["node", "--version"])),
        ("git-revision", argv(&["git", "rev-parse", "HEAD"])),
    ] {
        match run_check(
            &root,
            name,
            &command,
            &output,
            &env,
            Duration::from_secs(30),
        ) {
            Ok(result) => toolchain.push(result),
            Err(error) => {
                eprintln!("cannot run {name}: {error}");
                return 1;
            }
        }
    }

    let mut results = Vec::new();
    if options.suite.builds_artifacts() {
        match run_freshness_check(&root, &output) {
            Ok(result) => {
                println!("{}: {}", result.name, result.status.name());
                results.push(result);
            }
            Err(error) => {
                eprintln!("cannot run binary-freshness: {error}");
                return 1;
            }
        }
    }
    for check in &plan {
        println!("Checking {}…", check.name);
        let result = if check.command.iter().map(String::as_str).eq(FMTDEBT_ARGV) {
            run_format_debt(&root, &output)
        } else {
            run_check(
                &root,
                &check.name,
                &check.command,
                &output,
                &env,
                Duration::from_secs(options.timeout),
            )
        };
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                eprintln!("cannot run {}: {error}", check.name);
                return 1;
            }
        };
        println!("{}: {}", result.name, result.status.name());
        results.push(result);
    }

    let after = match source_snapshot(&root) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("cannot snapshot the sources after the run: {error}");
            return 1;
        }
    };
    let source_changed = before.sha256 != after.sha256;
    let all_passed = toolchain
        .iter()
        .chain(results.iter())
        .all(|result| result.status == Status::Passed);
    let status = if source_changed {
        "source-changed"
    } else if all_passed {
        "passed"
    } else {
        "failed"
    };
    let report = Report {
        status,
        suite: options.suite.name(),
        offline,
        cargo_jobs: 2,
        toolchain,
        checks: results,
        source_before: before,
        source_after: after,
        source_changed,
        scope: "Headless checks only; not GUI/physical input/provider/formal proof or complete compiled closure",
    };
    let path = output.join("report.json");
    let rendered = match serde_json::to_string_pretty(&report) {
        Ok(rendered) => rendered,
        Err(error) => {
            eprintln!("cannot render the report: {error}");
            return 1;
        }
    };
    if let Err(error) = write_private(&path, &(rendered + "\n")) {
        eprintln!("cannot write the report: {error}");
        return 1;
    }
    let shown = path.strip_prefix(&root).unwrap_or(&path);
    println!("Result: {status}; evidence: {}", shown.display());
    if status == "passed" { 0 } else { 1 }
}

fn run_format_debt(root: &Path, output: &Path) -> io::Result<CheckResult> {
    let started = Instant::now();
    let log = output.join("format-debt.log");
    let mut result = CheckResult::new(
        "format-debt",
        &argv(&FMTDEBT_ARGV),
        started,
        log.to_string_lossy().into_owned(),
    );
    let outcome = fmtdebt::evaluate(root, false, &fmtdebt::baseline_path(root))?;
    let combined = format!("{}{}", outcome.stdout, outcome.stderr);
    write_private(&log, &combined)?;
    result.exit_code = Some(outcome.code);
    result.status = if outcome.code == 0 {
        Status::Passed
    } else {
        Status::Failed
    };
    result.seconds = millis(started);
    Ok(result)
}

#[derive(Serialize)]
struct Planned {
    suite: &'static str,
    offline: bool,
    checks: Vec<Check>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    status: &'static str,
    suite: &'static str,
    offline: bool,
    cargo_jobs: u32,
    toolchain: Vec<CheckResult>,
    checks: Vec<CheckResult>,
    source_before: Snapshot,
    source_after: Snapshot,
    source_changed: bool,
    scope: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempdir::make_temp_dir;

    fn root() -> PathBuf {
        crate::root()
    }

    fn names(checks: &[Check]) -> Vec<&str> {
        checks.iter().map(|check| check.name.as_str()).collect()
    }

    #[test]
    fn plan_for_examples_executes_existing_examples_not_the_gui_main_entry() {
        let checks = commands(&root(), Suite::Examples, true);
        assert_eq!(names(&checks), ["layout_smoke", "scroll_app_smoke"]);
        for check in checks {
            assert_eq!(check.command.first().map(String::as_str), Some("cargo"));
            assert_eq!(check.command.get(1).map(String::as_str), Some("test"));
            assert!(check.command.iter().any(|argument| argument == "--locked"));
            assert!(check.command.iter().any(|argument| argument == "--offline"));
            assert!(check.command.iter().any(|argument| argument == "--example"));
            assert!(!check.command.iter().any(|argument| argument == "run"));
        }
    }

    #[test]
    fn clippy_denies_unfinished_macros_without_denying_baseline_warnings() {
        let checks = clippy(true);
        assert_eq!(
            names(&checks),
            [
                "core-clippy",
                "transport-clippy",
                "app-clippy",
                "xtask-clippy"
            ]
        );
        for check in checks {
            for denial in CLIPPY_DENIALS {
                assert!(
                    check.command.iter().any(|argument| argument == denial),
                    "{}",
                    check.name
                );
            }
            assert!(!check.command.iter().any(|argument| argument == "warnings"));
            assert!(!check.command.iter().any(|argument| argument == "-A"));
        }
    }

    #[test]
    fn the_all_suite_is_every_lint_followed_by_every_test() {
        let all = commands(&root(), Suite::All, true);
        let mut expected = commands(&root(), Suite::Lint, true);
        expected.extend(commands(&root(), Suite::Test, true));
        assert_eq!(names(&all), names(&expected));
        for (actual, wanted) in all.iter().zip(expected.iter()) {
            assert_eq!(actual.command, wanted.command, "{}", actual.name);
        }
        assert_eq!(
            names(&commands(&root(), Suite::Lint, true))[0],
            "format-debt"
        );
        let tests = names(&commands(&root(), Suite::Test, true)).join(" ");
        for owner in [
            "core",
            "core-transport",
            "transport",
            "app",
            "app-public",
            "wayland-drops",
            "layout_smoke",
            "scroll_app_smoke",
            "xtask-guards",
            "bridge",
        ] {
            assert!(tests.split(' ').any(|name| name == owner), "{owner}");
        }
    }

    #[test]
    fn the_wayland_drop_owner_is_executed_locally_and_in_ci() {
        let checks = commands(&root(), Suite::Test, true);
        let wayland = checks
            .iter()
            .find(|check| check.name == "wayland-drops")
            .expect("wayland-drops check");
        let index = wayland
            .command
            .iter()
            .position(|argument| argument == "-p")
            .unwrap();
        assert_eq!(wayland.command[index + 1], "winit");
        assert!(
            wayland
                .command
                .iter()
                .any(|argument| argument == "data_device")
        );
        // CI must run this same owner, not a look-alike command.
        let workflow = fs::read_to_string(root().join(".github/workflows/native.yml")).unwrap();
        assert!(
            workflow.contains("-p winit --lib data_device -- --test-threads=2"),
            "the workflow no longer runs the wayland drop owner"
        );
        assert!(workflow.contains("verify --suite examples --online"));
        assert!(workflow.contains("verify --suite clippy --online"));
        assert!(workflow.contains("rustup component add clippy"));
    }

    #[test]
    fn online_only_removes_the_offline_flag() {
        for suite in [Suite::Clippy, Suite::Examples, Suite::Test, Suite::All] {
            let offline = commands(&root(), suite, true);
            let online = commands(&root(), suite, false);
            assert_eq!(offline.len(), online.len(), "{}", suite.name());
            for (offline, online) in offline.iter().zip(online.iter()) {
                let stripped: Vec<&String> = offline
                    .command
                    .iter()
                    .filter(|argument| argument.as_str() != "--offline")
                    .collect();
                let wanted: Vec<&String> = online.command.iter().collect();
                assert_eq!(stripped, wanted, "{} / {}", suite.name(), offline.name);
            }
        }
    }

    #[test]
    fn secrets_are_removed_and_two_cargo_jobs_are_forced() {
        let env = check_environment(None, true);
        let value = |key: &str| {
            env.get(std::ffi::OsStr::new(key))
                .map(|value| value.to_string_lossy().into_owned())
        };
        assert_eq!(value("CARGO_BUILD_JOBS").as_deref(), Some("2"));
        assert_eq!(value("RUSTUP_AUTO_INSTALL").as_deref(), Some("0"));
        assert_eq!(value("CARGO_NET_OFFLINE").as_deref(), Some("true"));
        assert!(!env.keys().any(|name| {
            let name = name.to_string_lossy().to_uppercase();
            ["KEY", "SECRET", "TOKEN", "PASSWORD"]
                .iter()
                .any(|word| name.contains(word))
        }));
    }

    #[test]
    fn a_stale_release_artifact_fails_the_guard_and_a_rebuilt_one_passes() {
        let root = make_temp_dir(&std::env::temp_dir(), "freshness-").unwrap();
        let source = root.join("app/src/ui.rs");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, "PUBLIC first-party source\n").unwrap();
        let artifact = root.join("app/target/release/dsh-native-app");
        fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        fs::write(&artifact, "PUBLIC binary\n").unwrap();
        let logs = root.join("logs");
        fs::create_dir(&logs).unwrap();
        set_mtime(&artifact, 1_000_000);
        set_mtime(&source, 2_000_000);

        let (built, stale) = freshness_findings(&root).unwrap();
        assert_eq!(built, ["app/target/release/dsh-native-app"]);
        assert_eq!(stale, ["app/target/release/dsh-native-app"]);
        let result = run_freshness_check(&root, &logs).unwrap();
        assert_eq!(result.status, Status::Failed);
        assert_eq!(result.exit_code, Some(1));
        let text = fs::read_to_string(&result.log).unwrap();
        assert!(
            text.contains("older than their first-party sources"),
            "{text}"
        );
        assert!(text.contains("cargo build --release"), "{text}");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&result.log).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // A rebuilt artifact of equal or newer age passes.
        set_mtime(&artifact, 2_000_000);
        assert_eq!(freshness_findings(&root).unwrap(), (built, Vec::new()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_absent_release_artifact_is_not_a_failure() {
        let root = make_temp_dir(&std::env::temp_dir(), "freshness-absent-").unwrap();
        let source = root.join("app/src/ui.rs");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, "PUBLIC first-party source\n").unwrap();
        let logs = root.join("logs");
        fs::create_dir(&logs).unwrap();
        assert_eq!(freshness_findings(&root).unwrap(), (Vec::new(), Vec::new()));
        let result = run_freshness_check(&root, &logs).unwrap();
        assert_eq!(result.status, Status::Passed);
        assert_eq!(result.present_artifacts, Some(Vec::new()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_failed_attempt_retains_its_exit_and_a_private_log() {
        let output = make_temp_dir(&std::env::temp_dir(), "check-").unwrap();
        let result = run_check(
            &output,
            "failure",
            &argv(&["/bin/sh", "-c", "echo counterexample; exit 7"]),
            &output,
            &BTreeMap::new(),
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(result.status, Status::Failed);
        assert_eq!(result.exit_code, Some(7));
        let text = fs::read_to_string(&result.log).unwrap();
        assert!(text.contains("counterexample"), "{text}");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&result.log).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(output).unwrap();
    }

    #[test]
    fn a_timeout_is_not_reported_as_a_successful_exit() {
        let output = make_temp_dir(&std::env::temp_dir(), "check-timeout-").unwrap();
        let result = run_check(
            &output,
            "timeout",
            &argv(&["/bin/sh", "-c", "sleep 30"]),
            &output,
            &BTreeMap::new(),
            Duration::from_millis(50),
        )
        .unwrap();
        assert_eq!(result.status, Status::TimedOut);
        assert!(result.timed_out);
        assert_eq!(result.signal, Some(9));
        assert_eq!(result.exit_code, Some(-9));
        fs::remove_dir_all(output).unwrap();
    }

    #[test]
    fn a_missing_command_is_unavailable_rather_than_skipped() {
        let output = make_temp_dir(&std::env::temp_dir(), "check-missing-").unwrap();
        let result = run_check(
            &output,
            "missing",
            &argv(&["/definitely-absent-impeccable-command"]),
            &output,
            &BTreeMap::new(),
            Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(result.status, Status::Unavailable);
        assert_eq!(result.exit_code, None);
        assert!(!result.timed_out);
        assert!(
            fs::read_to_string(&result.log)
                .unwrap()
                .contains("Cannot execute command"),
            "the absent command is recorded, not silently skipped"
        );
        fs::remove_dir_all(output).unwrap();
    }

    #[test]
    fn arguments_outside_the_bounds_are_rejected() {
        for arguments in [
            vec!["--timeout".to_string(), "0".to_string()],
            vec!["--timeout".to_string(), "3601".to_string()],
            vec!["--cargo-home".to_string(), "relative-cache".to_string()],
            vec!["--suite".to_string(), "live-model".to_string()],
            vec!["--unknown".to_string()],
        ] {
            assert!(Options::parse(&arguments).is_err(), "{arguments:?}");
        }
        let accepted =
            Options::parse(&["--suite".into(), "examples".into(), "--plan".into()]).unwrap();
        assert_eq!(accepted.suite, Suite::Examples);
        assert!(accepted.plan);
    }

    fn set_mtime(path: &Path, seconds: u64) {
        let status = Command::new("touch")
            .args(["-d", &format!("@{seconds}"), path.to_str().unwrap()])
            .status()
            .unwrap();
        assert!(status.success());
    }
}
