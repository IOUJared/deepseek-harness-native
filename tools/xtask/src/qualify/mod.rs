//! Own-renderer component qualification shared by the native fixtures.
//!
//! Port of the shared half of `scripts/qualify-*.py`: a new private evidence
//! directory, the isolated child environment, the owned-process guard and the
//! per-mode component run. No Host, model, login or desktop setting is touched.
pub mod app;
pub mod composed;
pub mod composed_files;
pub mod control;
pub mod decisions;
pub mod layout;
pub mod node;
pub mod prompt;
pub mod scroll;
pub mod skill;
pub mod stage;
pub mod uncertainty;
pub mod upload;

use crate::compositor::{Facts, window_facts};
use crate::proc;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const DIALOG_MODES: [&str; 5] = [
    "approval",
    "question",
    "unsupported",
    "settings",
    "settings-confirm",
];
const CODEX_MODES: [&str; 4] = [
    "settings-codex-connect",
    "settings-codex-ready",
    "settings-codex-callback",
    "settings-codex-enable",
];
const DIALOG_CLASS: &str = "ai.deepseek.harness.native.dialog-fixture";
const COMPONENT_DEADLINE: Duration = Duration::from_secs(15);

/// A new private directory below `app/evidence`, with the child's XDG/HOME layout.
pub fn prepare(root: &Path, output: &Path) -> Result<PathBuf, String> {
    let allowed = root.join("app/evidence");
    let allowed = allowed.canonicalize().unwrap_or_else(|_| allowed.clone());
    let output = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(output))
            .unwrap_or_else(|_| output.to_path_buf())
    };
    let parent = output
        .parent()
        .ok_or("output must be a NEW directory below native app/evidence")?
        .canonicalize()
        .map_err(|_| "output must be a NEW directory below native app/evidence".to_string())?;
    if fs::symlink_metadata(&output).is_ok() || !parent.starts_with(&allowed) {
        return Err("output must be a NEW directory below native app/evidence".into());
    }
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&output)
        .map_err(|error| error.to_string())?;
    for name in [
        "harness",
        "workspace",
        "user",
        "config",
        "cache",
        "data",
        "state",
    ] {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(output.join(name))
            .map_err(|error| error.to_string())?;
    }
    Ok(output)
}

/// Preserve only native display/driver identity; no ambient model or account
/// credentials, and never the caller's real HOME or DISPLAY.
pub fn environment_from(
    output: &Path,
    inherited: &BTreeMap<OsString, OsString>,
) -> BTreeMap<OsString, OsString> {
    let names = [
        "PATH",
        "LANG",
        "LC_ALL",
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "HYPRLAND_INSTANCE_SIGNATURE",
        "AQ_DRM_DEVICES",
        "WGPU_BACKEND",
    ];
    let mut env = BTreeMap::new();
    for name in names {
        if let Some(value) = inherited.get(std::ffi::OsStr::new(name)) {
            env.insert(name.into(), value.clone());
        }
    }
    for (name, value) in [
        ("HOME", output.join("user")),
        ("XDG_CONFIG_HOME", output.join("config")),
        ("XDG_CACHE_HOME", output.join("cache")),
        ("XDG_DATA_HOME", output.join("data")),
        ("XDG_STATE_HOME", output.join("state")),
    ] {
        env.insert(name.into(), value.into_os_string());
    }
    env.insert("WINIT_UNIX_BACKEND".into(), "wayland".into());
    env.insert("ICED_BACKEND".into(), "wgpu".into());
    env
}

/// The isolated environment for this process's children.
pub fn environment(output: &Path) -> BTreeMap<OsString, OsString> {
    environment_from(output, &std::env::vars_os().collect())
}

/// A report file that is private and never replaces an existing one.
pub fn private_json(path: &Path, value: &Value) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(
        (serde_json::to_string_pretty(value).map_err(io::Error::other)? + "\n").as_bytes(),
    )
}

pub(super) fn pidfd_open(pid: i32) -> Option<i32> {
    // Safe: a plain pid argument; a negative answer is an absence, not an error to raise.
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) as i32 };
    (fd >= 0).then_some(fd)
}

pub(super) fn pidfd_send(fd: i32, signal: i32) {
    // Safe: the descriptor was opened for this pid and is closed by the caller.
    unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd,
            signal,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        );
    }
}

/// A pidfd becomes readable when its process exits.
pub(super) fn pidfd_readable(fd: i32) -> bool {
    let mut poll = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    // Safe: one pollfd for zero milliseconds.
    unsafe { libc::poll(&mut poll, 1, 0) > 0 }
}

/// The pass decision, separated so it can be reasoned about without a window.
pub fn component_passed(
    exit_code: i32,
    forced: bool,
    children: &[i32],
    fixture: &Value,
    facts: Option<&Facts>,
    expected_class: &str,
) -> bool {
    exit_code == 0
        && !forced
        && children.is_empty()
        && fixture.get("status").and_then(Value::as_str) == Some("passed")
        && facts.is_some_and(|facts| {
            facts.get("class").and_then(Value::as_str) == Some(expected_class)
                && facts.get("xwayland").and_then(Value::as_bool) == Some(false)
                && facts.get("mapped").and_then(Value::as_bool) == Some(true)
        })
}

#[derive(Clone, Copy, Debug)]
pub struct ComponentOptions {
    pub scale: f64,
    /// Compositor probe; tests without an owned window disable it.
    pub probe: bool,
}

/// Run one fixture mode and retain its report, children and window facts.
pub fn run_component(
    root: &Path,
    binary: &Path,
    output: &Path,
    mode: &str,
    options: &ComponentOptions,
) -> io::Result<Value> {
    let directory = output.join(mode);
    fs::DirBuilder::new().mode(0o700).create(&directory)?;
    let mut child = Command::new(binary)
        .arg("--output")
        .arg(&directory)
        .arg("--mode")
        .arg(mode)
        .arg("--scale")
        .arg(format!("{}", options.scale))
        .env_clear()
        .envs(environment(output))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;
    let pid = child.id() as i32;
    let Some(identity) = proc::stat(Path::new("/proc"), pid) else {
        let _ = child.wait();
        return Err(io::Error::other("owned process identity unavailable"));
    };
    let Some(fd) = pidfd_open(pid) else {
        let _ = child.wait();
        return Err(io::Error::other("owned process identity unavailable"));
    };
    let verified = proc::stat(Path::new("/proc"), pid);
    if verified.as_ref().map(|stat| stat.start_ticks) != Some(identity.start_ticks) {
        // Safe: this descriptor belongs to this run and is not shared.
        unsafe {
            libc::close(fd);
        }
        let _ = child.wait();
        return Err(io::Error::other("owned process identity unavailable"));
    }
    let mut facts: Option<Facts> = None;
    let mut observed: BTreeSet<i32> = BTreeSet::new();
    let mut forced = false;
    let deadline = Instant::now() + COMPONENT_DEADLINE;
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        for stat in proc::process_tree(Path::new("/proc"), pid, identity.start_ticks) {
            if stat.pid != pid {
                observed.insert(stat.pid);
            }
        }
        if facts.is_none()
            && options.probe
            && let Some(candidate) = window_facts(pid)
            && candidate.get("mapped").and_then(Value::as_bool) == Some(true)
        {
            facts = Some(candidate);
        }
        if Instant::now() >= deadline {
            forced = true;
            pidfd_send(fd, libc::SIGTERM);
            let hard = Instant::now() + Duration::from_secs(2);
            while child.try_wait()?.is_none() && Instant::now() < hard {
                std::thread::sleep(Duration::from_millis(20));
            }
            if child.try_wait()?.is_none() {
                pidfd_send(fd, libc::SIGKILL);
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let status = child.wait()?;
    if !forced && !pidfd_readable(fd) {
        // The process outlived its child handle; kill the captured identity, not a pid.
        pidfd_send(fd, libc::SIGKILL);
        let _ = child.wait();
    }
    // Safe: closed once, after all signal decisions for this run.
    unsafe {
        libc::close(fd);
    }
    let code = crate::exit_code(status);
    let report = directory.join("result.json");
    let fixture: Value = fs::read_to_string(&report)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({"status": "missing-fixture-report"}));
    let children: Vec<i32> = observed.into_iter().collect();
    let passed = component_passed(
        code,
        forced,
        &children,
        &fixture,
        facts.as_ref(),
        DIALOG_CLASS,
    );
    let frame = directory.join("own-window.png");
    Ok(json!({
        "mode": mode,
        "status": if passed { "passed" } else { "failed" },
        "exitCode": code,
        "forced": forced,
        "observedChildProcesses": children,
        "ownedWindow": facts,
        "fixture": fixture,
        "ownRendererFrame": relative_to_root(root, &frame),
        "paintInspected": false,
        "liveHostDecisionQualified": false,
        "nativeKeyboardPointerInputQualified": false,
    }))
}

fn relative_to_root(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

fn component_binary(root: &Path) -> Result<PathBuf, String> {
    let binary = root.join("app/target/release/examples/dialog_smoke");
    let binary = binary
        .canonicalize()
        .map_err(|_| "release component fixture not built".to_string())?;
    if !binary.is_file() {
        return Err("release component fixture not built".into());
    }
    Ok(binary)
}

fn run_modes(
    root: &Path,
    output: &Path,
    scale: f64,
    modes: &[&str],
    probe: bool,
) -> Result<Vec<Value>, String> {
    let binary = component_binary(root)?;
    let options = ComponentOptions { scale, probe };
    modes
        .iter()
        .map(|mode| {
            run_component(root, &binary, output, mode, &options).map_err(|error| error.to_string())
        })
        .collect()
}

fn dialog(root: &Path, output: &Path, scale: f64) -> i32 {
    let output = match prepare(root, output) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("dialog qualification refused: {message}");
            return 2;
        }
    };
    let results = match run_modes(root, &output, scale, &DIALOG_MODES, true) {
        Ok(results) => results,
        Err(message) => {
            eprintln!("dialog qualification failed: {message}");
            return 1;
        }
    };
    let passed = results.iter().all(|result| result["status"] == "passed");
    let binary = component_binary(root).unwrap_or_default();
    let report = json!({
        "status": if passed { "passed" } else { "failed" },
        "scope": "actual native component rendering/public local reducer fixtures, not composed Host/live interaction",
        "binarySha256": crate::sha256_file(&binary).ok(),
        "applicationScale": scale,
        "backendStarted": false,
        "realCredentialsUsed": false,
        "modelPrompts": 0,
        "replyRpcIssued": false,
        "fixtureResults": results,
        "desktopSettingsChanged": false,
    });
    if let Err(error) = private_json(&output.join("result.json"), &report) {
        eprintln!("cannot write the qualification report: {error}");
        return 1;
    }
    println!(
        "{}",
        json!({"status": report["status"], "report": output.join("result.json").to_string_lossy()})
    );
    if passed { 0 } else { 1 }
}

fn codex(root: &Path, output: &Path, scale: f64) -> i32 {
    let output = match prepare(root, output) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("codex qualification refused: {message}");
            return 2;
        }
    };
    let results = match run_modes(root, &output, scale, &CODEX_MODES, true) {
        Ok(results) => results,
        Err(message) => {
            eprintln!("codex qualification failed: {message}");
            return 1;
        }
    };
    let passed = results.iter().all(|result| {
        result["status"] == "passed"
            && result["fixture"]["fixturePassed"].as_bool() == Some(true)
            && result["fixture"]["browserOpened"].as_bool() == Some(false)
            && result["fixture"]["credentialWriteIssued"].as_bool() == Some(false)
    });
    let binary = component_binary(root).unwrap_or_default();
    let report = json!({
        "status": if passed { "passed" } else { "failed" },
        "scope": "public native Codex settings rendering/local reducer only",
        "binarySha256": crate::sha256_file(&binary).ok(),
        "applicationScale": scale,
        "backendStarted": false,
        "browserOpened": false,
        "realCredentialsUsed": false,
        "modelPrompts": 0,
        "rpcIssued": false,
        "desktopSettingsChanged": false,
        "fixtureResults": results,
        "realOpenAiLoginQualified": false,
        "nativeKeyboardPointerInputQualified": false,
    });
    if let Err(error) = private_json(&output.join("result.json"), &report) {
        eprintln!("cannot write the qualification report: {error}");
        return 1;
    }
    println!(
        "{}: {}",
        report["status"],
        output.join("result.json").display()
    );
    if passed { 0 } else { 1 }
}

fn usage() -> &'static str {
    "usage: xtask qualify <app|dialog|codex> --output NEW_DIR [--scale 1.0|1.25] \
     [--seconds 8-30] [--screenshot]"
}

fn parse_scale(arguments: &mut std::slice::Iter<'_, String>) -> Result<f64, String> {
    let value = arguments.next().ok_or("--scale needs a value")?;
    let scale: f64 = value.parse().map_err(|_| "scale is not a number")?;
    if !(scale == 1.0 || scale == 1.25) {
        return Err("scale must be 1.0 or 1.25".into());
    }
    Ok(scale)
}

/// `xtask qualify <fixture> --output NEW_DIR [--scale S]`.
pub fn run(arguments: &[String]) -> i32 {
    let Some(command) = arguments.first().map(String::as_str) else {
        eprintln!("{}", usage());
        return 2;
    };
    let mut output: Option<PathBuf> = None;
    let mut scale = 1.0_f64;
    let mut seconds: u32 = 10;
    let mut screenshot = false;
    let mut sdk = false;
    let mut fail_at = String::from("none");
    let mut viewport_minimum = false;
    let mut arguments = arguments[1..].iter();
    while let Some(argument) = arguments.next() {
        let parsed: Result<(), String> = (|| match argument.as_str() {
            "--output" => {
                output = Some(PathBuf::from(
                    arguments.next().ok_or("--output needs a value")?,
                ));
                Ok(())
            }
            "--scale" => {
                scale = parse_scale(&mut arguments)?;
                Ok(())
            }
            "--seconds" => {
                seconds = arguments
                    .next()
                    .ok_or("--seconds needs a value")?
                    .parse()
                    .map_err(|_| "seconds is not a number")?;
                if !(8..=30).contains(&seconds) {
                    return Err("seconds must be 8-30".into());
                }
                Ok(())
            }
            "--screenshot" => {
                screenshot = true;
                Ok(())
            }
            "--sdk" => {
                sdk = true;
                Ok(())
            }
            "--viewport" => {
                match arguments.next().ok_or("--viewport needs a value")?.as_str() {
                    "full" => viewport_minimum = false,
                    "minimum" => viewport_minimum = true,
                    other => {
                        return Err(format!("--viewport must be full or minimum, not {other}"));
                    }
                }
                Ok(())
            }
            "--fail-at" => {
                fail_at = arguments.next().ok_or("--fail-at needs a value")?.clone();
                if !matches!(fail_at.as_str(), "none" | "ack-held" | "store-held") {
                    return Err("--fail-at must be none, ack-held or store-held".into());
                }
                Ok(())
            }
            other => Err(format!("unexpected argument: {other}")),
        })();
        if let Err(message) = parsed {
            eprintln!("{}\n{message}", usage());
            return 2;
        }
    }
    let Some(output) = output else {
        eprintln!("{}\n--output is required", usage());
        return 2;
    };
    let root = crate::root();
    match command {
        "app" => app::run(
            &root,
            &output,
            &app::AppOptions {
                scale,
                seconds,
                screenshot,
                probe: true,
            },
        ),
        "details" => layout::run(&root.join("app")),
        "scroll" => scroll::run(&root.join("app")),
        "agent-discovery" => node::agent_discovery(&root, &output, sdk),
        "client-projections" => node::client_projections(&root, &output),
        "turn-outline" => node::turn_outline(&root, &output),
        "file-upload" => upload::run(&root, &output),
        "native-file-stage" => stage::run(&root, &output),
        "native-file-prompt" => prompt::run(&root, &output),
        "impeccable-skill" => skill::run(&root, &output),
        "upload-uncertainty" => uncertainty::run(&root, &output, &fail_at),
        "composed-settings" => composed::run(&root, &output, scale),
        "composed-files" => composed_files::run(&root, &output, scale),
        "composed-decisions" => {
            let viewport = if viewport_minimum { "minimum" } else { "full" };
            decisions::run(&root, &output, scale, viewport)
        }
        "lazy-account" => composed::lazy_account(&root, &output),
        "control" => control::run(&root.join("app")),
        "submenus" => layout::submenus(&root.join("app")),
        "dialog" => dialog(&root, &output, scale),
        "codex" => codex(&root, &output, scale),
        other => {
            eprintln!("{}\nunknown qualification: {other}", usage());
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempdir::make_temp_dir;
    use std::os::unix::fs::PermissionsExt;

    fn inherited(pairs: &[(&str, &str)]) -> BTreeMap<OsString, OsString> {
        pairs
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value)))
            .collect()
    }

    #[test]
    fn a_report_is_private_and_is_never_overwritten() {
        let base = make_temp_dir(&std::env::temp_dir(), "qualify-private-").unwrap();
        let path = base.join("result.json");
        private_json(&path, &json!({"scope": "public local fixture"})).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(private_json(&path, &json!({"replaced": true})).is_err());
        let kept: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(kept, json!({"scope": "public local fixture"}));
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn the_fixture_environment_keeps_only_the_owned_home_and_native_display() {
        let ambient = inherited(&[
            ("DEEPSEEK_API_KEY", "PUBLIC_SENTINEL"),
            ("HOME", "/real-user-home"),
            ("DSH_HOME", "/real-harness"),
            ("NODE_OPTIONS", "PUBLIC_SENTINEL"),
            ("DISPLAY", ":9"),
            ("WAYLAND_DISPLAY", "wayland-public-test"),
            ("XDG_RUNTIME_DIR", "/public-test-runtime"),
        ]);
        let env = environment_from(Path::new("/owned-fixture"), &ambient);
        let get = |key: &str| {
            env.get(std::ffi::OsStr::new(key))
                .map(|value| value.to_string_lossy().into_owned())
        };
        for forbidden in ["DEEPSEEK_API_KEY", "DSH_HOME", "NODE_OPTIONS", "DISPLAY"] {
            assert_eq!(get(forbidden), None, "{forbidden} leaked");
        }
        assert_eq!(get("HOME").as_deref(), Some("/owned-fixture/user"));
        assert_eq!(get("WINIT_UNIX_BACKEND").as_deref(), Some("wayland"));
        assert_eq!(get("ICED_BACKEND").as_deref(), Some("wgpu"));
        assert_eq!(
            get("WAYLAND_DISPLAY").as_deref(),
            Some("wayland-public-test")
        );
    }

    #[test]
    fn prepare_refuses_a_reused_or_unowned_output() {
        let base = make_temp_dir(&std::env::temp_dir(), "qualify-prepare-").unwrap();
        fs::create_dir_all(base.join("app/evidence")).unwrap();
        let first = prepare(&base, &base.join("app/evidence/owned")).unwrap();
        assert_eq!(
            first.metadata().unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(first.join("user"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert!(prepare(&base, &first).is_err(), "a used output is refused");
        assert!(
            prepare(&base, &base.join("unrelated")).is_err(),
            "an output outside app/evidence is refused"
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_component_decision_requires_a_matching_native_window() {
        let fixture = json!({"status": "passed"});
        let mut facts = Facts::new();
        facts.insert("class".into(), json!(DIALOG_CLASS));
        facts.insert("xwayland".into(), json!(false));
        facts.insert("mapped".into(), json!(true));
        assert!(component_passed(
            0,
            false,
            &[],
            &fixture,
            Some(&facts),
            DIALOG_CLASS
        ));
        // A forced exit, an observed child, a stray class or XWayland all fail.
        assert!(!component_passed(
            1,
            false,
            &[],
            &fixture,
            Some(&facts),
            DIALOG_CLASS
        ));
        assert!(!component_passed(
            0,
            true,
            &[],
            &fixture,
            Some(&facts),
            DIALOG_CLASS
        ));
        assert!(!component_passed(
            0,
            false,
            &[7],
            &fixture,
            Some(&facts),
            DIALOG_CLASS
        ));
        assert!(!component_passed(
            0,
            false,
            &[],
            &fixture,
            Some(&facts),
            "another-class"
        ));
        assert!(!component_passed(
            0,
            false,
            &[],
            &fixture,
            None,
            DIALOG_CLASS
        ));
    }

    #[test]
    fn a_run_without_a_window_still_records_its_fixture_report() {
        let base = make_temp_dir(&std::env::temp_dir(), "qualify-component-").unwrap();
        let output = base.join("run");
        fs::create_dir(&output).unwrap();
        let binary = base.join("fake-dialog");
        fs::write(
            &binary,
            "#!/bin/sh\nwhile [ $# -gt 0 ]; do case \"$1\" in --output) out=\"$2\"; shift 2;; *) shift;; esac; done\nprintf '{\"status\":\"passed\",\"fixturePassed\":true}\\n' > \"$out/result.json\"\n",
        )
        .unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        let case = run_component(
            &base,
            &binary,
            &output,
            "approval",
            &ComponentOptions {
                scale: 1.0,
                probe: false,
            },
        )
        .unwrap();
        assert_eq!(case["exitCode"].as_i64(), Some(0));
        assert_eq!(case["fixture"]["status"].as_str(), Some("passed"));
        assert_eq!(case["ownedWindow"], Value::Null);
        assert_eq!(
            case["status"].as_str(),
            Some("failed"),
            "no owned window was proven"
        );
        fs::remove_dir_all(base).unwrap();
    }
}
