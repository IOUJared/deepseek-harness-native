//! Two owned native App processes, joined save/shutdown and cold UNSENT recovery.
//!
//! Port of the validation half of `scripts/qualify-draft-restart.py`. The two
//! reports must correlate with the explicit PUBLIC manifest/document while
//! differing pids and an identical session prove a real restart.
use serde_json::{Value, json};

/// The exact PUBLIC unsent draft both processes must agree on.
pub const LATEST: &str = "PUBLIC unsent draft\nΚαλημέρα · 日本語 · 🦀\nPUBLIC newer edit while joined save outcome is held\nمرحبا · café";

fn natural(value: &Value) -> Option<u64> {
    value.as_u64()
}

fn natural_between(value: &Value, minimum: u64, maximum: u64) -> bool {
    natural(value).is_some_and(|number| (minimum..=maximum).contains(&number))
}

/// A bounded, non-control, non-surrogate session id.
pub fn session_id(value: &Value) -> bool {
    value.as_str().is_some_and(|text| {
        !text.is_empty()
            && text.len() <= 128
            && text.chars().all(|c| {
                !c.is_whitespace()
                    && (c as u32) >= 0x20
                    && c != '\u{7f}'
                    && !(0xD800..=0xDFFF).contains(&(c as u32))
            })
    })
}

fn stopped(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
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
    }) && value["exited"].as_bool() == Some(true)
        && value["graceful"].as_bool() == Some(true)
        && value["containmentUnknown"].as_bool() == Some(false)
        && value["observedDescendantsRemaining"].as_i64() == Some(0)
        && value["exitCode"].as_i64() == Some(0)
}

const COMMON_TRUE: [&str; 13] = [
    "publicOnly",
    "actualAppExercised",
    "actualDraftTasks",
    "wholeProfileUnchanged",
    "joinedOutcomeNotBlockedSyscall",
    "draftRecoveryLoaded",
    "draftIoJoined",
    "draftsCleanAtExit",
    "latestSnapshotDurable",
    "modelCatalogSucceeded",
    "inspectionObserved",
    "completePUBLICSnapshotObserved",
    "gracefulStopped",
];
const FALSE_FLAGS: [&str; 5] = [
    "backendMetricsAvailable",
    "networkTraceQualified",
    "physicalInputQualified",
    "crashDurabilityQualified",
    "driverPromptResumeModelSigninAllowed",
];
const FORBIDDEN_RECORDS: [&str; 7] = [
    "step/start",
    "step/end",
    "request/header",
    "tool/call",
    "tool/result",
    "assistant/message",
    "user/message",
];

/// Validate one phase's exact native report types and record kinds.
pub fn valid(app: &Value, mode: &str, pid: u64) -> bool {
    if !matches!(mode, "write" | "recover") || pid == 0 {
        return false;
    }
    let Some(kinds) = app["observedPUBLICRecordKinds"].as_array() else {
        return false;
    };
    let records_ok = !kinds.is_empty()
        && kinds.iter().all(|kind| {
            kind.as_str()
                .is_some_and(|kind| !kind.contains("llm") && !FORBIDDEN_RECORDS.contains(&kind))
        });
    let counters_ok = [
        "actualAppPromptCommands",
        "promptSerial",
        "unexpectedWork",
        "manualBusinessMessagesForwarded",
    ]
    .iter()
    .all(|key| natural(&app[key]) == Some(0))
        && natural(&app["modelCatalogReads"]) == Some(1);
    if app["status"].as_str() != Some("passed")
        || app["mode"].as_str() != Some(mode)
        || natural(&app["pid"]) != Some(pid)
        || !session_id(&app["sessionId"])
        || app.get("errorCode").is_some()
        || app["version"].as_str() != Some("0.2.1-alpha.1")
        || app["windowBackend"].as_str() != Some("wayland")
        || COMMON_TRUE
            .iter()
            .any(|key| app[*key].as_bool() != Some(true))
        || FALSE_FLAGS
            .iter()
            .any(|key| app[*key].as_bool() != Some(false))
        || !counters_ok
        || !stopped(&app["stop"])
        || !records_ok
    {
        return false;
    }
    if mode == "write" {
        return [
            "stoppedWhileBusy",
            "exitAnywayBlockedWhileBusy",
            "heldOutcomeReleased",
        ]
        .iter()
        .all(|key| app[*key].as_bool() == Some(true))
            && natural(&app["savedOutcomes"]) == Some(2)
            && natural_between(&app["heldRevision"], 1, u64::MAX)
            && natural_between(&app["latestSavedRevision"], 1, u64::MAX)
            && app["latestSavedRevision"].as_u64() > app["heldRevision"].as_u64();
    }
    app["recoveredNeedsReview"].as_bool() == Some(true)
        && app["coldExplicitSelectOnly"].as_bool() == Some(true)
        && app["coldRosterContainedSubject"].as_bool() == Some(true)
        && app["sendAllowed"].as_bool() == Some(false)
        && natural(&app["savedOutcomes"]) == Some(0)
}

/// Correlate two validated reports with only the explicitly owned PUBLIC document.
pub fn valid_restart(first: &Value, second: &Value, manifest: &Value, document: &Value) -> bool {
    let (Some(first_pid), Some(second_pid)) = (natural(&first["pid"]), natural(&second["pid"]))
    else {
        return false;
    };
    valid(first, "write", first_pid)
        && valid(second, "recover", second_pid)
        && first_pid != second_pid
        && natural_between(&first["pid"], 1, (1u64 << 31) - 1)
        && natural(&second["pid"]).is_some_and(|pid| (1..=(1u64 << 31) - 1).contains(&pid))
        && first["sessionId"] == second["sessionId"]
        && natural(&manifest["version"]) == Some(1)
        && manifest["publicOnly"].as_bool() == Some(true)
        && natural(&manifest["firstPid"]) == Some(first_pid)
        && natural_between(&manifest["firstPid"], 1, (1u64 << 31) - 1)
        && manifest["sessionId"] == first["sessionId"]
        && document.as_object().is_some_and(|object| {
            object
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>()
                == ["version", "drafts"].into_iter().collect()
        })
        && natural(&document["version"]) == Some(1)
        && document["drafts"] == {
            let mut drafts = serde_json::Map::new();
            drafts.insert(
                first["sessionId"].as_str().unwrap_or_default().to_string(),
                json!(LATEST),
            );
            Value::Object(drafts)
        }
}

use super::composed::{Owned, guard_with};
use super::private;
use std::fs;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn digest(path: &Path) -> Option<String> {
    crate::sha256_file(path).ok()
}

fn walk(directory: &Path, extension: &str, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            walk(&path, extension, paths);
        } else if path.extension().is_some_and(|ext| ext == extension) {
            paths.push(path);
        }
    }
}

/// Every source whose digest must stay unchanged across both processes.
fn sources(root: &Path) -> std::collections::BTreeMap<String, String> {
    let mut paths: Vec<PathBuf> = vec![
        root.join("tools/xtask/src/qualify/drafts.rs"),
        root.join("tools/xtask/src/qualify/composed.rs"),
        root.join("app/examples/draft_composed_smoke.rs"),
    ];
    for (directory, extension) in [
        ("app/src", "rs"),
        ("core/src", "rs"),
        ("transport/src", "rs"),
        ("core/runtime", "mjs"),
    ] {
        walk(&root.join(directory), extension, &mut paths);
    }
    for crate_name in ["app", "core", "transport"] {
        for name in ["Cargo.toml", "Cargo.lock"] {
            let path = root.join(crate_name).join(name);
            if path.is_file() {
                paths.push(path);
            }
        }
    }
    if let Some(runtime) = root
        .parent()
        .map(|parent| parent.join("deepseek-harness-linux/apps/cli"))
    {
        if runtime.join("package.json").is_file() {
            paths.push(runtime.join("package.json"));
        }
        walk(&runtime.join("lib"), "js", &mut paths);
    }
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let digest = digest(&path)?;
            Some((path.to_string_lossy().into_owned(), digest))
        })
        .collect()
}

fn private_log(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

fn phase(
    output: &Path,
    binary: &Path,
    runtime: &Path,
    mode: &str,
    scale: f64,
    seconds: u64,
) -> Value {
    let mut report = json!({
        "mode": mode,
        "pid": Value::Null,
        "timedOut": false,
        "identityPinFailures": Value::Null,
        "observedIdentities": [],
        "exitCode": Value::Null,
        "cleanup": Value::Null,
        "status": "failed",
    });
    let folder = output;
    let logs = (
        folder.join(format!("{mode}-stdout.log")),
        folder.join(format!("{mode}-stderr.log")),
    );
    let (Ok(stdout), Ok(stderr)) = (private_log(&logs.0), private_log(&logs.1)) else {
        report["errorType"] = json!("log-open-failed");
        return report;
    };
    let mut env = super::environment(output);
    env.insert("PATH".into(), "/usr/bin:/bin".into());
    let spawned = Command::new(binary)
        .arg("--fixture-mode")
        .arg(mode)
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
        .arg(format!("{scale}"))
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
    let run = guard_with(
        Owned {
            child,
            status: None,
        },
        seconds,
    );
    let app = private::json(&output.join(format!("{mode}-app.json")), 65536)
        .unwrap_or_else(|_| json!({"status": "invalid-or-missing-report"}));
    let passed = run.owned.code() == 0
        && !run.interrupted
        && run.cleanup.observed_cleanup_complete
        && !run.cleanup.root_was_running_at_cleanup
        && run.cleanup.remaining_before_external_cleanup.is_empty()
        && run.observation_failures == 0
        && valid(&app, mode, pid);
    report["pid"] = json!(pid);
    report["identityPinFailures"] = json!(run.observation_failures);
    report["exitCode"] = json!(run.owned.code());
    report["cleanup"] = json!(&run.cleanup);
    report["observedIdentities"] = json!(run.observed);
    report["app"] = app;
    report["status"] = json!(if passed { "passed" } else { "failed" });
    if let Err(error) = super::private_json(&output.join(format!("{mode}-owned.json")), &report) {
        report["errorType"] = json!(format!("owned-report-failed: {error}"));
    }
    report
}

/// `xtask qualify draft-restart --output PUBLIC-draft-restart-... [--binary] [--scale]`.
pub fn run(root: &Path, requested: &Path, binary_arg: &Path, scale: f64) -> i32 {
    // New private state only; every artifact below inherits owner-only modes.
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
        .map(|name| name.to_string_lossy().starts_with("PUBLIC-draft-restart-"))
        .unwrap_or(false)
    {
        eprintln!("draft-restart failed: output basename must start PUBLIC-draft-restart-");
        return 1;
    }
    let output = match super::prepare(root, &requested) {
        Ok(output) => match output.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                eprintln!("draft-restart failed: {error}");
                return 1;
            }
        },
        Err(message) => {
            eprintln!("draft-restart failed: {message}");
            return 1;
        }
    };
    let mut summary = json!({
        "status": "failed",
        "scope": "Two owned native App processes, joined save/shutdown and cold UNSENT recovery; no Send, bounded App intent/PUBLIC records, network unqualified.",
        "phases": [],
        "backendIndependentZeroPromptsQualified": false,
        "networkTraceQualified": false,
        "physicalInputQualified": false,
        "crashDurabilityQualified": false,
        "sourceClosureComplete": false,
        "artifactDeploymentQualified": false,
    });
    let source_before = sources(root);
    summary["sourceSha256Before"] = json!(&source_before);
    let original = match binary_arg.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            eprintln!("draft-restart failed: fixture binary is not ready");
            return 1;
        }
    };
    let binary = output.join("draft_composed_smoke");
    let copied = fs::copy(&original, &binary).and_then(|_| {
        let file = fs::OpenOptions::new().write(true).open(&binary)?;
        file.sync_all()
    });
    if let Err(error) = copied {
        eprintln!("draft-restart failed: cannot deploy the fixture binary: {error}");
        return 1;
    }
    let _ = fs::set_permissions(&binary, fs::Permissions::from_mode(0o700));
    let artifact = digest(&binary).unwrap_or_default();
    summary["artifactSha256"] = json!(&artifact);
    if digest(&original).as_deref() != Some(artifact.as_str()) {
        eprintln!("draft-restart failed: binary changed during copy");
        return 1;
    }
    let runtime = root
        .parent()
        .map(|parent| parent.join("deepseek-harness-linux/apps/cli"))
        .and_then(|path| path.canonicalize().ok());
    let Some(runtime) = runtime else {
        eprintln!("draft-restart failed: the alpha runtime is missing");
        return 1;
    };
    let deadline = Instant::now() + Duration::from_secs(170);
    let mut failed = false;
    for mode in ["write", "recover"] {
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .as_secs()
            .min(80);
        let report = phase(&output, &binary, &runtime, mode, scale, remaining.max(1));
        let passed = report["status"].as_str() == Some("passed");
        if let Some(phases) = summary["phases"].as_array_mut() {
            phases.push(report);
        }
        if !passed {
            failed = true;
            break;
        }
    }
    if !failed {
        let phases = summary["phases"].as_array().cloned().unwrap_or_default();
        let (first, second) = (phases[0]["app"].clone(), phases[1]["app"].clone());
        let manifest = private::json(&output.join("session-manifest.json"), 65536)
            .unwrap_or_else(|_| json!({}));
        let document = private::json(&output.join("harness/native-drafts.v1.json"), 65536)
            .unwrap_or_else(|_| json!({}));
        if !valid_restart(&first, &second, &manifest, &document) {
            summary["errorType"] = json!("cold-process-or-independent-durable-body-mismatch");
            failed = true;
        }
    }
    let source_after = sources(root);
    let source_unchanged = source_after == source_before;
    let artifact_unchanged = digest(&binary).as_deref() == Some(artifact.as_str());
    summary["sourceSha256After"] = json!(&source_after);
    summary["sourceUnchanged"] = json!(source_unchanged);
    summary["artifactUnchanged"] = json!(artifact_unchanged);
    if !failed && source_unchanged && artifact_unchanged {
        summary["fullProcessRestartObserved"] = json!(true);
        summary["independentlyReadPUBLICDraftMatched"] = json!(true);
        summary["status"] = json!("passed");
    }
    if let Err(error) =
        super::private_json(&output.join("draft-restart-qualification.json"), &summary)
    {
        eprintln!("cannot write the draft-restart report: {error}");
        return 1;
    }
    println!(
        "{}",
        json!({"status": summary["status"], "report": output.join("draft-restart-qualification.json").to_string_lossy()})
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

    fn report(mode: &str, pid: u64) -> Value {
        let mut value = json!({
            "status": "passed", "mode": mode, "pid": pid,
            "sessionId": "session-PUBLIC-1", "version": "0.2.1-alpha.1",
            "windowBackend": "wayland",
            "actualAppPromptCommands": 0, "promptSerial": 0, "unexpectedWork": 0,
            "manualBusinessMessagesForwarded": 0, "modelCatalogReads": 1,
            "stop": {"exited": true, "graceful": true, "containmentUnknown": false,
                     "observedDescendantsRemaining": 0, "exitCode": 0},
            "observedPUBLICRecordKinds": ["draft/save", "draft/select"],
        });
        for key in COMMON_TRUE {
            value[key] = json!(true);
        }
        for key in FALSE_FLAGS {
            value[key] = json!(false);
        }
        if mode == "write" {
            value["stoppedWhileBusy"] = json!(true);
            value["exitAnywayBlockedWhileBusy"] = json!(true);
            value["heldOutcomeReleased"] = json!(true);
            value["savedOutcomes"] = json!(2);
            value["heldRevision"] = json!(4);
            value["latestSavedRevision"] = json!(5);
        } else {
            value["recoveredNeedsReview"] = json!(true);
            value["coldExplicitSelectOnly"] = json!(true);
            value["coldRosterContainedSubject"] = json!(true);
            value["sendAllowed"] = json!(false);
            value["savedOutcomes"] = json!(0);
        }
        value
    }

    fn manifest() -> Value {
        json!({"version": 1, "publicOnly": true, "firstPid": 4100, "sessionId": "session-PUBLIC-1"})
    }

    fn document() -> Value {
        json!({"version": 1, "drafts": {"session-PUBLIC-1": LATEST}})
    }

    #[test]
    fn a_write_report_needs_the_joined_save_and_no_llm_records() {
        assert!(valid(&report("write", 4100), "write", 4100));
        // A model record, a counter or a missing held outcome fails.
        let mut llm = report("write", 4100);
        llm["observedPUBLICRecordKinds"] = json!(["llm/response"]);
        assert!(!valid(&llm, "write", 4100));
        let mut step = report("write", 4100);
        step["observedPUBLICRecordKinds"] = json!(["step/start"]);
        assert!(!valid(&step, "write", 4100));
        let mut prompt = report("write", 4100);
        prompt["actualAppPromptCommands"] = json!(1);
        assert!(!valid(&prompt, "write", 4100));
        let mut unheld = report("write", 4100);
        unheld["heldOutcomeReleased"] = json!(false);
        assert!(!valid(&unheld, "write", 4100));
        let mut revisions = report("write", 4100);
        revisions["latestSavedRevision"] = json!(4);
        assert!(
            !valid(&revisions, "write", 4100),
            "the latest save must be newer"
        );
        // A pid mismatch is not this run.
        assert!(!valid(&report("write", 4100), "write", 4101));
    }

    #[test]
    fn a_recover_report_must_cold_select_without_allowing_send() {
        assert!(valid(&report("recover", 4200), "recover", 4200));
        let mut sends = report("recover", 4200);
        sends["sendAllowed"] = json!(true);
        assert!(!valid(&sends, "recover", 4200));
        let mut saved = report("recover", 4200);
        saved["savedOutcomes"] = json!(1);
        assert!(!valid(&saved, "recover", 4200));
        // A session id with whitespace or a control character is not a session id.
        let mut spacey = report("recover", 4200);
        spacey["sessionId"] = json!("session PUBLIC");
        assert!(!valid(&spacey, "recover", 4200));
        assert!(!session_id(&json!("")), "an empty session id is refused");
        assert!(session_id(&json!("PUBLIC-session-1")));
    }

    #[test]
    fn a_restart_requires_two_pids_sharing_one_session_and_the_public_document() {
        let first = report("write", 4100);
        let second = report("recover", 4200);
        assert!(valid_restart(&first, &second, &manifest(), &document()));
        // The same pid twice is not a restart.
        assert!(!valid_restart(
            &first,
            &report("recover", 4100),
            &manifest(),
            &document()
        ));
        // A different session does not correlate.
        let mut moved = report("recover", 4200);
        moved["sessionId"] = json!("session-PUBLIC-2");
        assert!(!valid_restart(&first, &moved, &manifest(), &document()));
        // A manifest for another pid or a document with other content fails.
        let mut other_manifest = manifest();
        other_manifest["firstPid"] = json!(4200);
        assert!(!valid_restart(
            &first,
            &second,
            &other_manifest,
            &document()
        ));
        let mut other_document = document();
        other_document["drafts"]["session-PUBLIC-1"] = json!("another draft");
        assert!(!valid_restart(
            &first,
            &second,
            &manifest(),
            &other_document
        ));
        // An extra top-level document key is refused.
        let mut extra = document();
        extra["extra"] = json!(1);
        assert!(!valid_restart(&first, &second, &manifest(), &extra));
    }
}
