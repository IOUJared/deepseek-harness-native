//! Draft interruption campaign: the checkpoint contract.
//!
//! Port of the validation half of `scripts/qualify-draft-interruption.py`. Nine
//! checkpoints exercise hold/recover/crash at each write barrier; a graceful
//! phase may not borrow the crash phase's external descendant cleanup, and a
//! crash observation must latch its gate until the owner kills it.
use super::composed::{Key, Owned, cleanup_owned, identity, observe_descendants, pin_root};
use super::drafts::{self, session_id};
use super::private;
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const OLD: &str = "PUBLIC committed draft\nΚαλημέρα · 日本語 · 🦀";
pub const NEW: &str =
    "PUBLIC committed draft\nΚαλημέρα · 日本語 · 🦀\nPUBLIC interrupted replacement · مرحبا";
pub const LATEST: &str = "PUBLIC committed draft\nΚαλημέρα · 日本語 · 🦀\nPUBLIC interrupted replacement · مرحبا\nPUBLIC newest text while actual save is unfinished · café";

pub const POINTS: [&str; 9] = [
    "before-write",
    "after-write",
    "before-temp-sync",
    "after-temp-sync",
    "before-rename",
    "after-rename",
    "before-dir-sync",
    "after-dir-sync",
    "before-ack",
];
const FORBIDDEN: [&str; 7] = [
    "step/start",
    "step/end",
    "request/header",
    "tool/call",
    "tool/result",
    "assistant/message",
    "user/message",
];

/// The document a checkpoint must leave behind: the new body only once the
/// replacement has been written.
pub fn expected(point: &str) -> &'static str {
    if POINTS[..5].contains(&point) {
        OLD
    } else {
        NEW
    }
}

pub fn valid_stop(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>()
        == [
            "exited",
            "graceful",
            "containmentUnknown",
            "observedDescendantsRemaining",
            "exitCode",
        ]
        .into_iter()
        .collect()
        && value["exited"].as_bool() == Some(true)
        && value["graceful"].as_bool() == Some(true)
        && value["containmentUnknown"].as_bool() == Some(false)
        && value["observedDescendantsRemaining"].as_i64() == Some(0)
        && value["exitCode"].as_i64() == Some(0)
}

/// A graceful phase may not borrow the crash phase's external descendant cleanup.
pub fn valid_cleanup(cleanup: &Value, mode: &str) -> bool {
    if !matches!(mode, "hold" | "recover" | "crash") {
        return false;
    }
    let Some(before) = cleanup["remainingBeforeExternalCleanup"].as_array() else {
        return false;
    };
    let Some(after) = cleanup["remainingAfterExternalCleanup"].as_array() else {
        return false;
    };
    ["rootPidfdPinned", "rootReaped", "observedCleanupComplete"]
        .iter()
        .all(|key| cleanup[*key].as_bool() == Some(true))
        && [
            "rootWasRunningAtCleanup",
            "ownedChildFallbackUsed",
            "cleanupTimedOut",
            "cleanupProbeOrSignalFailed",
        ]
        .iter()
        .all(|key| cleanup[*key].as_bool() == Some(false))
        && cleanup["killEscalated"].is_boolean()
        && after.is_empty()
        && (mode == "crash"
            || (before.is_empty() && cleanup["killEscalated"].as_bool() == Some(false)))
}

/// Shared checkpoint fields, exact pid correlation and quiet counters.
pub fn common(app: &Value, pid: u64, point: &str) -> bool {
    if !POINTS.contains(&point) {
        return false;
    }
    let Some(kinds) = app["recordKinds"].as_array() else {
        return false;
    };
    app["publicOnly"].as_bool() == Some(true)
        && app["version"].as_str() == Some("0.2.1-alpha.1")
        && pid > 0
        && pid < (1u64 << 31)
        && app["pid"].as_u64() == Some(pid)
        && session_id(&app["sessionId"])
        && app["checkpoint"].as_str() == Some(point)
        && [
            "promptSerial",
            "actualAppPromptCommands",
            "unexpectedWork",
            "manualBusinessMessagesForwarded",
        ]
        .iter()
        .all(|key| app[*key].as_i64() == Some(0))
        && app["catalogReads"].as_i64() == Some(1)
        && app["catalogSucceeded"].as_bool() == Some(true)
        && app["windowBackend"].as_str() == Some("wayland")
        && !kinds.is_empty()
        && kinds.iter().all(|kind| {
            kind.as_str()
                .is_some_and(|kind| !kind.contains("llm") && !FORBIDDEN.contains(&kind))
        })
        && [
            "kernelBlockedFsyncQualified",
            "powerLossQualified",
            "physicalInputQualified",
            "networkTraceQualified",
        ]
        .iter()
        .all(|key| app[*key].as_bool() == Some(false))
}

pub fn valid_finished(app: &Value, pid: u64, mode: &str, point: &str) -> bool {
    if !matches!(mode, "hold" | "recover") || !common(app, pid, point) {
        return false;
    }
    if app["status"].as_str() != Some("passed")
        || app["mode"].as_str() != Some(mode)
        || app.get("errorCode").is_some()
        || !valid_stop(&app["stop"])
        || [
            "draftIoJoined",
            "draftsCleanAtExit",
            "expectedSnapshotDurable",
        ]
        .iter()
        .any(|key| app[*key].as_bool() != Some(true))
    {
        return false;
    }
    if mode == "hold" {
        return app["saveUnfinished"].as_bool() == Some(true)
            && app["stoppedWhileSaveUnfinished"].as_bool() == Some(true)
            && app["exitAnywayRefused"].as_bool() == Some(true)
            && app["savedOutcomes"].as_i64() == Some(3)
            && app["guiHeartbeatsWhileBlocked"]
                .as_i64()
                .is_some_and(|beats| beats >= 3);
    }
    app["recoveredNeedsReview"].as_bool() == Some(true) && app["savedOutcomes"].as_i64() == Some(0)
}

pub fn valid_observation(app: &Value, pid: u64, point: &str) -> bool {
    common(app, pid, point)
        && app["status"].as_str() == Some("checkpoint-observed")
        && app.get("errorCode").is_none()
        && app["mode"].as_str() == Some("crash")
        && app["saveUnfinished"].as_bool() == Some(true)
        && app["crashGateLatchedUntilOwnedKill"].as_bool() == Some(true)
        && app["savedOutcomes"].as_i64() == Some(1)
        && session_id(&app["sessionId"])
}

// ---- campaign orchestration ----

fn phase(
    output: &Path,
    binary: &Path,
    runtime: &Path,
    mode: &str,
    point: &str,
    seconds: u64,
) -> Value {
    let mut report = json!({
        "status": "failed", "mode": mode, "checkpoint": point,
        "pid": Value::Null, "identityPinFailures": 0, "timedOut": false,
        "intendedSignal": Value::Null, "signal": Value::Null,
    });
    let logs = (
        output.join(format!("{mode}-stdout.log")),
        output.join(format!("{mode}-stderr.log")),
    );
    let (Ok(stdout), Ok(stderr)) = (
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&logs.0),
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&logs.1),
    ) else {
        report["errorType"] = json!("log-open-failed");
        return report;
    };
    let mut env = super::environment(output);
    env.insert("PATH".into(), "/usr/bin:/bin".into());
    let spawned = Command::new(binary)
        .arg("--campaign-mode")
        .arg(mode)
        .arg("--checkpoint")
        .arg(point)
        .arg("--runtime")
        .arg(runtime)
        .arg("--expected-version")
        .arg("0.2.1-alpha.1")
        .arg("--home")
        .arg(output.join("harness"))
        .arg("--user-home")
        .arg(output.join("user"))
        .arg("--cwd")
        .arg(output.join("workspace"))
        .arg("--scale")
        .arg("1.0")
        .current_dir(output.join("workspace"))
        .env_clear()
        .envs(&env)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .process_group(0)
        .spawn();
    let Ok(child) = spawned else {
        report["errorType"] = json!("spawn-failed");
        return report;
    };
    let pid = child.id() as u64;
    report["pid"] = json!(pid);
    let mut owned = Owned {
        child,
        status: None,
    };
    let owner = std::process::id() as i32;
    let mut pidfds: std::collections::BTreeMap<Key, i32> = std::collections::BTreeMap::new();
    let mut observed: std::collections::BTreeSet<Key> = std::collections::BTreeSet::new();
    let mut pin_failures = 0u64;
    let mut timed_out = false;
    let mut observation: Option<Value> = None;
    let mut intended_signal: Option<i64> = None;
    let mut pinned = false;
    let root = match pin_root(&mut owned, owner) {
        Ok((root, fd)) => {
            pinned = true;
            pidfds.insert(identity(&root), fd);
            observed.insert(identity(&root));
            Some(root)
        }
        Err(()) => None,
    };
    if let Some(root) = root.as_ref() {
        let marker = output.join("crash-observed.json");
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            if owned.poll().ok().flatten().is_some() {
                break;
            }
            pin_failures +=
                observe_descendants(&mut owned, root, &mut pidfds, &mut observed, owner);
            if mode == "crash" && intended_signal.is_none() && marker.symlink_metadata().is_ok() {
                match private::json(&marker, 65536) {
                    Ok(app) if valid_observation(&app, pid, point) => {
                        observation = Some(app);
                        intended_signal = Some(9);
                        if let Some(fd) = pidfds.get(&identity(root)) {
                            super::pidfd_send(*fd, libc::SIGKILL);
                        }
                    }
                    _ => {
                        report["errorType"] = json!("checkpoint-observation-invalid");
                        timed_out = true;
                        break;
                    }
                }
            }
            if super::composed::INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst)
                || Instant::now() >= deadline
            {
                timed_out = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let cleanup = cleanup_owned(&mut owned, &pidfds, pinned);
    for fd in pidfds.values() {
        // Safe: each descriptor was opened for one identity this run and closed once.
        unsafe {
            libc::close(*fd);
        }
    }
    let code = owned.code();
    let signal = owned.status.and_then(|status| status.signal());
    let safe = !super::composed::INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst)
        && !timed_out
        && pin_failures == 0
        && valid_cleanup(&json!(&cleanup), mode);
    let passed = if mode == "crash" {
        let ok = safe && intended_signal == Some(9) && signal == Some(libc::SIGKILL) && code == -9;
        if ok {
            let termination = json!({
                "version": 1, "pid": pid, "signal": 9,
                "nativeReaped": cleanup.root_reaped,
                "ownedCleanupComplete": cleanup.observed_cleanup_complete,
            });
            let _ = super::private_json(&output.join("termination.json"), &termination);
        }
        ok
    } else {
        match private::json(&output.join(format!("{mode}-app.json")), 65536) {
            Ok(app) if safe && code == 0 && valid_finished(&app, pid, mode, point) => {
                report["app"] = app;
                true
            }
            _ => false,
        }
    };
    report["identityPinFailures"] = json!(pin_failures);
    report["timedOut"] = json!(timed_out);
    report["intendedSignal"] = json!(intended_signal);
    report["signal"] = json!(signal);
    report["exitCode"] = json!(code);
    report["cleanup"] = json!(&cleanup);
    report["observedIdentities"] = json!(observed.len());
    if let Some(observation) = observation {
        report["observation"] = observation;
    }
    report["status"] = json!(if passed { "passed" } else { "failed" });
    let _ = super::private_json(&output.join(format!("{mode}-owned.json")), &report);
    report
}

fn case_root(root: &Path, base: &Path, suffix: &str) -> Result<PathBuf, String> {
    let requested = base.join(format!("PUBLIC-draft-interruption-{suffix}"));
    super::prepare(root, &requested)
}

/// `xtask qualify draft-interruption --output PUBLIC-draft-interruption-... [--binary]`.
pub fn run(root: &Path, requested: &Path, binary_arg: &Path) -> i32 {
    unsafe {
        libc::umask(0o077);
    }
    let requested = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(requested))
            .unwrap_or_else(|_| requested.to_path_buf())
    };
    if !requested
        .file_name()
        .map(|name| {
            name.to_string_lossy()
                .starts_with("PUBLIC-draft-interruption-")
        })
        .unwrap_or(false)
    {
        eprintln!("draft-interruption failed: explicit PUBLIC interruption root required");
        return 1;
    }
    let output = match super::prepare(root, &requested)
        .and_then(|path| path.canonicalize().map_err(|error| error.to_string()))
    {
        Ok(output) => output,
        Err(message) => {
            eprintln!("draft-interruption failed: {message}");
            return 1;
        }
    };
    let source = match binary_arg.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            eprintln!("draft-interruption failed: fixture binary is not ready");
            return 1;
        }
    };
    let binary = output.join("draft-interruption-snapshot");
    let copied = fs::copy(&source, &binary)
        .and_then(|_| fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)));
    if let Err(error) = copied {
        eprintln!("draft-interruption failed: cannot deploy the fixture binary: {error}");
        return 1;
    }
    let artifact = drafts::digest(&binary).unwrap_or_default();
    let mut before = drafts::sources(root);
    for path in [
        root.join("tools/xtask/src/qualify/interruption.rs"),
        root.join("app/examples/draft_interruption_smoke.rs"),
        root.join("app/src/draft_store.rs"),
    ] {
        if let Some(value) = drafts::digest(&path) {
            before.insert(path.to_string_lossy().into_owned(), value);
        }
    }
    let mut summary = json!({
        "status": "failed",
        "scope": "Draft interruption across write barriers; no power-loss or kernel-blocked fsync proof.",
        "cases": [],
        "artifactSha256": artifact,
        "sourceClosureComplete": false,
        "powerLossQualified": false,
        "kernelBlockedFsyncQualified": false,
        "physicalInputQualified": false,
        "networkTraceQualified": false,
    });
    let runtime = root
        .parent()
        .map(|parent| parent.join("deepseek-harness-linux/apps/cli"))
        .unwrap_or_default();
    let mut failure: Option<String> = None;
    match case_root(root, &output, "hold") {
        Ok(hold) => {
            let result = phase(&hold, &binary, &runtime, "hold", "before-temp-sync", 70);
            let passed = result["status"].as_str() == Some("passed");
            if let Some(cases) = summary["cases"].as_array_mut() {
                cases.push(json!({"checkpoint": "unfinished-save-close", "phases": [result], "status": if passed { "passed" } else { "failed" }}));
            }
            if !passed {
                failure = Some("unfinished-save-close-failed".into());
            }
        }
        Err(error) => failure = Some(error),
    }
    if failure.is_none() {
        for point in POINTS {
            if super::composed::INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst) {
                failure = Some("cancelled".into());
                break;
            }
            let case = match case_root(root, &output, point) {
                Ok(case) => case,
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            };
            let first = phase(&case, &binary, &runtime, "crash", point, 70);
            let mut item = json!({
                "checkpoint": point,
                "status": "failed",
                "phases": [&first],
                "expected": if POINTS[..5].contains(&point) { "old" } else { "new" },
            });
            if first["status"].as_str() != Some("passed") {
                if let Some(cases) = summary["cases"].as_array_mut() {
                    cases.push(item);
                }
                failure = Some("owned-crash-not-qualified".into());
                break;
            }
            let doc = private::json(&case.join("harness/native-drafts.v1.json"), 2 * 1024 * 1024);
            let subject = first["observation"]["sessionId"]
                .as_str()
                .unwrap_or_default();
            let doc_ok = doc.is_ok_and(|doc| {
                doc.as_object().is_some_and(|object| {
                    object
                        .keys()
                        .map(String::as_str)
                        .collect::<std::collections::BTreeSet<_>>()
                        == ["version", "drafts"].into_iter().collect()
                }) && doc["version"].as_i64() == Some(1)
                    && doc["drafts"] == json!({ subject: expected(point) })
            });
            if !doc_ok {
                if let Some(cases) = summary["cases"].as_array_mut() {
                    cases.push(item);
                }
                failure = Some("post-crash-document-mismatch".into());
                break;
            }
            let second = phase(&case, &binary, &runtime, "recover", point, 70);
            let second_ok = second["status"].as_str() == Some("passed")
                && first["pid"] != second["pid"]
                && second["app"]["sessionId"] == first["observation"]["sessionId"];
            if !second_ok {
                if let Some(phases) = item["phases"].as_array_mut() {
                    phases.push(second);
                }
                if let Some(cases) = summary["cases"].as_array_mut() {
                    cases.push(item);
                }
                failure = Some("cold-crash-recovery-not-qualified".into());
                break;
            }
            item["status"] = json!("passed");
            if let Some(phases) = item["phases"].as_array_mut() {
                phases.push(second);
            }
            if let Some(cases) = summary["cases"].as_array_mut() {
                cases.push(item);
            }
        }
    }
    let mut after = drafts::sources(root);
    for path in [
        root.join("tools/xtask/src/qualify/interruption.rs"),
        root.join("app/examples/draft_interruption_smoke.rs"),
        root.join("app/src/draft_store.rs"),
    ] {
        if let Some(value) = drafts::digest(&path) {
            after.insert(path.to_string_lossy().into_owned(), value);
        }
    }
    let source_unchanged = before == after;
    let artifact_unchanged = drafts::digest(&binary).as_deref() == Some(artifact.as_str());
    if let Some(failure) = &failure {
        summary["errorType"] = json!(failure);
    }
    summary["sourceSha256Before"] = json!(&before);
    summary["sourceSha256After"] = json!(&after);
    summary["sourceUnchanged"] = json!(source_unchanged);
    summary["artifactUnchanged"] = json!(artifact_unchanged);
    if failure.is_none() && source_unchanged && artifact_unchanged {
        summary["status"] = json!("passed");
    }
    if let Err(error) = super::private_json(
        &output.join("draft-interruption-qualification.json"),
        &summary,
    ) {
        eprintln!("cannot write the interruption report: {error}");
        return 1;
    }
    println!(
        "{}",
        json!({"status": summary["status"], "report": output.join("draft-interruption-qualification.json").to_string_lossy()})
    );
    if summary["status"].as_str() == Some("passed") {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(mode: &str, point: &str) -> Value {
        let mut value = json!({
            "publicOnly": true, "version": "0.2.1-alpha.1", "pid": 5100,
            "sessionId": "session-PUBLIC-1", "checkpoint": point,
            "promptSerial": 0, "actualAppPromptCommands": 0, "unexpectedWork": 0,
            "manualBusinessMessagesForwarded": 0, "catalogReads": 1, "catalogSucceeded": true,
            "windowBackend": "wayland", "recordKinds": ["draft/save"],
            "kernelBlockedFsyncQualified": false, "powerLossQualified": false,
            "physicalInputQualified": false, "networkTraceQualified": false,
            "stop": {"exited": true, "graceful": true, "containmentUnknown": false,
                     "observedDescendantsRemaining": 0, "exitCode": 0},
            "draftIoJoined": true, "draftsCleanAtExit": true, "expectedSnapshotDurable": true,
        });
        if mode == "hold" {
            value["status"] = json!("passed");
            value["mode"] = json!("hold");
            value["saveUnfinished"] = json!(true);
            value["stoppedWhileSaveUnfinished"] = json!(true);
            value["exitAnywayRefused"] = json!(true);
            value["savedOutcomes"] = json!(3);
            value["guiHeartbeatsWhileBlocked"] = json!(4);
        } else if mode == "recover" {
            value["status"] = json!("passed");
            value["mode"] = json!("recover");
            value["recoveredNeedsReview"] = json!(true);
            value["savedOutcomes"] = json!(0);
        } else {
            value["status"] = json!("checkpoint-observed");
            value["mode"] = json!("crash");
            value["saveUnfinished"] = json!(true);
            value["crashGateLatchedUntilOwnedKill"] = json!(true);
            value["savedOutcomes"] = json!(1);
        }
        value
    }

    #[test]
    fn the_document_only_changes_after_the_replacement_is_written() {
        assert_eq!(expected("before-write"), OLD);
        assert_eq!(expected("before-rename"), OLD);
        assert_eq!(expected("after-rename"), NEW);
        assert_eq!(expected("before-ack"), NEW);
        assert_eq!(
            LATEST,
            NEW.to_string() + "\nPUBLIC newest text while actual save is unfinished · café"
        );
    }

    #[test]
    fn a_finished_hold_must_refuse_exit_and_keep_heartbeats() {
        assert!(valid_finished(
            &report("hold", "after-write"),
            5100,
            "hold",
            "after-write"
        ));
        let mut refused = report("hold", "after-write");
        refused["exitAnywayRefused"] = json!(false);
        assert!(!valid_finished(&refused, 5100, "hold", "after-write"));
        let mut few = report("hold", "after-write");
        few["guiHeartbeatsWhileBlocked"] = json!(2);
        assert!(!valid_finished(&few, 5100, "hold", "after-write"));
        let mut saved = report("hold", "after-write");
        saved["savedOutcomes"] = json!(2);
        assert!(!valid_finished(&saved, 5100, "hold", "after-write"));
    }

    #[test]
    fn a_recover_and_a_crash_observation_need_their_own_shapes() {
        assert!(valid_finished(
            &report("recover", "after-dir-sync"),
            5100,
            "recover",
            "after-dir-sync"
        ));
        let mut recovered = report("recover", "after-dir-sync");
        recovered["recoveredNeedsReview"] = json!(false);
        assert!(!valid_finished(
            &recovered,
            5100,
            "recover",
            "after-dir-sync"
        ));
        assert!(valid_observation(
            &report("crash", "before-ack"),
            5100,
            "before-ack"
        ));
        let mut latched = report("crash", "before-ack");
        latched["crashGateLatchedUntilOwnedKill"] = json!(false);
        assert!(!valid_observation(&latched, 5100, "before-ack"));
        // A finished report cannot be a crash observation and vice versa.
        assert!(!valid_observation(
            &report("hold", "before-ack"),
            5100,
            "before-ack"
        ));
        assert!(!valid_finished(
            &report("crash", "before-ack"),
            5100,
            "hold",
            "before-ack"
        ));
    }

    #[test]
    fn a_checkpoint_must_match_its_pid_and_forbid_model_records() {
        let mut moved = report("hold", "after-write");
        moved["pid"] = json!(5101);
        assert!(!valid_finished(&moved, 5100, "hold", "after-write"));
        let mut point = report("hold", "after-write");
        point["checkpoint"] = json!("before-write");
        assert!(!valid_finished(&point, 5100, "hold", "after-write"));
        let mut model = report("hold", "after-write");
        model["recordKinds"] = json!(["llm/response"]);
        assert!(!valid_finished(&model, 5100, "hold", "after-write"));
        let mut step = report("hold", "after-write");
        step["recordKinds"] = json!(["step/start"]);
        assert!(!valid_finished(&step, 5100, "hold", "after-write"));
        assert!(!valid_finished(&json!({}), 5100, "hold", "after-write"));
    }

    #[test]
    fn a_graceful_phase_cannot_borrow_the_crash_phases_cleanup() {
        let graceful = json!({
            "rootPidfdPinned": true, "rootReaped": true, "observedCleanupComplete": true,
            "rootWasRunningAtCleanup": false, "ownedChildFallbackUsed": false,
            "cleanupTimedOut": false, "cleanupProbeOrSignalFailed": false,
            "killEscalated": false, "remainingBeforeExternalCleanup": [],
            "remainingAfterExternalCleanup": [],
        });
        assert!(valid_cleanup(&graceful, "hold"));
        let mut borrowed = graceful.clone();
        borrowed["remainingBeforeExternalCleanup"] = json!([[42, 7]]);
        assert!(!valid_cleanup(&borrowed, "hold"));
        assert!(
            valid_cleanup(&borrowed, "crash"),
            "the crash phase may retain descendants"
        );
        let mut escalated = graceful.clone();
        escalated["killEscalated"] = json!(true);
        assert!(!valid_cleanup(&escalated, "recover"));
        assert!(!valid_cleanup(&graceful, "other"));
    }

    #[test]
    fn a_stop_must_have_exact_types_and_values() {
        let stop = json!({"exited": true, "graceful": true, "containmentUnknown": false,
                          "observedDescendantsRemaining": 0, "exitCode": 0});
        assert!(valid_stop(&stop));
        let mut extra = stop.clone();
        extra["extra"] = json!(0);
        assert!(!valid_stop(&extra));
        let mut boolean_code = stop.clone();
        boolean_code["exitCode"] = json!(true);
        assert!(!valid_stop(&boolean_code));
        let mut not_graceful = stop;
        not_graceful["graceful"] = json!(false);
        assert!(!valid_stop(&not_graceful));
    }
}
