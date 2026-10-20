//! Owned actual native UI/worker/Gateway/registered-root decisions.
//!
//! Port of `scripts/qualify-composed-decisions.py`. Uses the shared guarded
//! runner for containment and adds its own raw-Host receipt and capture checks.
use super::composed::{GuardedRun, guard};
use super::composed_files::private_bytes;
use serde_json::{Value, json};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;

const APP_CLASS: &str = "ai.deepseek.harness.native.decisions-composed-fixture";

fn private_report(path: &Path) -> Result<Value, String> {
    let data = private_bytes(path, 16384)?;
    let value: Value = serde_json::from_slice(&data).map_err(|error| error.to_string())?;
    if !value.is_object() {
        return Err("report-object-required".into());
    }
    Ok(value)
}

fn number(value: &Value, key: &str) -> Option<i64> {
    value[key].as_i64()
}

/// The driver report plus the independent raw-Host receipts must agree.
pub fn valid_reports(output: &Path, app: &Value, pid: i64, scale: f64, viewport: &str) -> bool {
    if !matches!(viewport, "full" | "minimum") {
        return false;
    }
    let expected_viewport = if viewport == "minimum" {
        json!([608, 448])
    } else {
        Value::Null
    };
    if app["logicalViewport"] != expected_viewport {
        return false;
    }
    if number(app, "pid").is_none()
        || number(app, "nativeAcks").is_none()
        || number(app, "hostSettlements").is_none()
    {
        return false;
    }
    if app["status"].as_str() != Some("passed")
        || app["pid"].as_i64() != Some(pid)
        || app["applicationScale"].as_f64() != Some(scale)
        || app["nativeAcks"].as_i64() != Some(3)
        || app["hostSettlements"].as_i64() != Some(4)
        || app["windowBackend"].as_str() != Some("wayland")
    {
        return false;
    }
    for flag in [
        "publicFixtureOnly",
        "scriptedUiMessages",
        "realWorkerAndRootReady",
        "actualRootAgentCorrelated",
        "hostAbortCancelObserved",
        "liveHostDecisionQualified",
        "actualRosterSucceeded",
    ] {
        if app[flag].as_bool() != Some(true) {
            return false;
        }
    }
    for flag in [
        "nativeKeyboardPointerInputQualified",
        "realCredentialsUsed",
        "modelCatalogRequested",
        "browserSignInRequested",
        "manualBusinessMessagesForwarded",
        "timedUiEnabled",
        "outboundPacketTracePerformed",
    ] {
        if app[flag].as_bool() != Some(false) {
            return false;
        }
    }
    let control = output.join("fixture/control");
    let Ok(ready) = private_report(&control.join("ready.json")) else {
        return false;
    };
    let Some(agent) = ready["actualAgentId"]
        .as_str()
        .filter(|agent| !agent.is_empty())
    else {
        return false;
    };
    if ready["rootRegistered"].as_bool() != Some(true) {
        return false;
    }
    for ordinal in 1..=4i64 {
        let Ok(phase) = private_report(&control.join(format!("phase-{ordinal}.json"))) else {
            return false;
        };
        if phase["phase"].as_i64() != Some(ordinal)
            || phase["settled"].as_bool() != Some(true)
            || phase["actualAgentId"].as_str() != Some(agent)
        {
            return false;
        }
        let extra_ok = match ordinal {
            1 => {
                phase["rootRegistered"].as_bool() == Some(true)
                    && phase["approvalOutcome"].as_str() == Some("allowed-once")
            }
            2 => {
                phase["expectedAnswerMatched"].as_bool() == Some(true)
                    && phase["answerCount"].as_i64() == Some(3)
            }
            3 => phase["code"].as_str() == Some("ASK_CANCELLED"),
            _ => phase["code"].as_str() == Some("ASK_ABORTED"),
        };
        if !extra_ok {
            return false;
        }
    }
    let Ok(final_report) = private_report(&control.join("complete.json")) else {
        return false;
    };
    if app["host"] != final_report
        || [
            "disposed",
            "turnClosed",
            "approvalAuditPair",
            "zeroAdmittedModelSteps",
        ]
        .iter()
        .any(|flag| final_report[*flag].as_bool() != Some(true))
    {
        return false;
    }
    let expected = json!({"turnStart": 1, "turnEnd": 1, "steps": 0, "requestHeaders": 0,
        "toolCalls": 0, "toolResults": 0, "assistantMessages": 0,
        "approvalAsked": 1, "approvalDecided": 1});
    final_report["counts"] == expected
}

/// Both decision captures must be private PNGs with sane dimensions.
pub fn valid_captures(output: &Path) -> bool {
    for phase in 1..=2 {
        let path = output.join(format!("decision-{phase}.png"));
        let Ok(header) = private_bytes(&path, 32 * 1024 * 1024) else {
            return false;
        };
        if header.len() < 24
            || !header.starts_with(b"\x89PNG\r\n\x1a\n")
            || &header[12..16] != b"IHDR"
        {
            return false;
        }
        let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
        let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
        if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
            return false;
        }
    }
    true
}

/// The verdict over the guarded run, the driver report and the captures.
#[allow(clippy::too_many_arguments)]
fn decisions_passed(
    run: &GuardedRun,
    pid: i64,
    scale: f64,
    viewport: &str,
    verified: bool,
    binary_unchanged: bool,
) -> bool {
    run.owned.code() == 0
        && !(run.forced || run.cleanup.root_was_running_at_cleanup)
        && run.failure.is_none()
        && verified
        && run.cleanup.observed_cleanup_complete
        && run.cleanup.remaining_before_external_cleanup.is_empty()
        && run.observation_failures == 0
        && run.facts.as_ref().is_some_and(|facts| {
            facts["class"].as_str() == Some(APP_CLASS) && facts["xwayland"].as_bool() == Some(false)
        })
        && binary_unchanged
        && pid > 0
        && (scale == 1.0 || scale == 1.25)
        && matches!(viewport, "full" | "minimum")
}

/// `xtask qualify composed-decisions --output NEW_DIR --scale --viewport`.
pub fn run(root: &Path, output: &Path, scale: f64, viewport: &str) -> i32 {
    let output = match super::prepare(root, output) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("decisions qualification failed: {message}");
            return 1;
        }
    };
    let Ok(binary) = root
        .join("app/target/release/examples/decisions_composed_smoke")
        .canonicalize()
    else {
        eprintln!("decisions qualification failed: release fixture not ready");
        return 1;
    };
    let before_hash = match crate::sha256_file(&binary) {
        Ok(hash) => hash,
        Err(error) => {
            eprintln!("decisions qualification failed: {error}");
            return 1;
        }
    };
    // Safe: constant signals and a handler that only stores an atomic flag.
    let previous_int = unsafe {
        libc::signal(
            libc::SIGINT,
            super::composed::interrupt as *const () as libc::sighandler_t,
        )
    };
    let previous_term = unsafe {
        libc::signal(
            libc::SIGTERM,
            super::composed::interrupt as *const () as libc::sighandler_t,
        )
    };
    super::composed::INTERRUPTED.store(false, Ordering::SeqCst);
    let child = match Command::new(&binary)
        .arg("--output")
        .arg(&output)
        .arg("--scale")
        .arg(format!("{scale}"))
        .arg("--viewport")
        .arg(viewport)
        .env_clear()
        .envs(super::environment(&output))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            eprintln!("decisions qualification failed: {error}");
            return 1;
        }
    };
    let pid = child.id() as i64;
    let run = guard(super::composed::Owned {
        child,
        status: None,
    });
    // Safe: restore the handlers this run installed.
    unsafe {
        libc::signal(libc::SIGINT, previous_int);
        libc::signal(libc::SIGTERM, previous_term);
    }
    let app = private_report(&output.join("app.json"))
        .unwrap_or_else(|_| json!({"status": "invalid-or-missing-report"}));
    let verified = valid_reports(&output, &app, pid, scale, viewport) && valid_captures(&output);
    let binary_unchanged = crate::sha256_file(&binary)
        .map(|hash| hash == before_hash)
        .unwrap_or(false);
    let passed = decisions_passed(&run, pid, scale, viewport, verified, binary_unchanged);
    let report = json!({
        "status": if passed { "passed" } else { "failed" },
        "scope": "Owned actual native UI/worker/Gateway/registered-root decisions; no physical-input or outbound-trace proof.",
        "binarySha256": before_hash,
        "exitCode": run.owned.code(),
        "forced": run.forced || run.cleanup.root_was_running_at_cleanup,
        "interrupted": run.interrupted,
        "failureCode": run.failure,
        "ownedWindow": run.facts,
        "observedOwnedProcessCount": run.observed,
        "ownershipObservationFailures": run.observation_failures,
        "cleanup": &run.cleanup,
        "app": app,
        "separateHostReceiptsVerified": verified,
        "paintInspected": false,
        "outboundPacketTracePerformed": false,
        "nativeKeyboardPointerInputQualified": false,
        "desktopSettingsChanged": false,
        "installedRuntimeChanged": false,
        "realCredentialsUsed": false,
    });
    if let Err(error) = super::private_json(&output.join("result.json"), &report) {
        eprintln!("cannot write the decisions report: {error}");
        return 1;
    }
    println!(
        "{}",
        json!({"status": report["status"], "report": output.join("result.json").to_string_lossy()})
    );
    if passed { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn private(path: &Path, body: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::write(path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn png() -> Vec<u8> {
        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR");
        png.extend_from_slice(&608u32.to_be_bytes());
        png.extend_from_slice(&448u32.to_be_bytes());
        png
    }

    fn complete() -> Value {
        json!({
            "disposed": true, "turnClosed": true, "approvalAuditPair": true,
            "zeroAdmittedModelSteps": true,
            "counts": {"turnStart": 1, "turnEnd": 1, "steps": 0, "requestHeaders": 0,
                        "toolCalls": 0, "toolResults": 0, "assistantMessages": 0,
                        "approvalAsked": 1, "approvalDecided": 1},
        })
    }

    fn app() -> Value {
        json!({
            "status": "passed", "pid": 4242, "nativeAcks": 3, "hostSettlements": 4,
            "applicationScale": 1.0, "windowBackend": "wayland", "logicalViewport": null,
            "publicFixtureOnly": true, "scriptedUiMessages": true,
            "realWorkerAndRootReady": true, "actualRootAgentCorrelated": true,
            "hostAbortCancelObserved": true, "liveHostDecisionQualified": true,
            "actualRosterSucceeded": true,
            "nativeKeyboardPointerInputQualified": false, "realCredentialsUsed": false,
            "modelCatalogRequested": false, "browserSignInRequested": false,
            "manualBusinessMessagesForwarded": false, "timedUiEnabled": false,
            "outboundPacketTracePerformed": false,
            "host": complete(),
        })
    }

    fn evidence(base: &Path, value: &Value) {
        let control = base.join("fixture/control");
        private(
            &control.join("ready.json"),
            serde_json::to_string(
                &json!({"rootRegistered": true, "actualAgentId": "PUBLIC-agent"}),
            )
            .unwrap()
            .as_bytes(),
        );
        for (index, (code, extra)) in [
            (
                None,
                json!({"rootRegistered": true, "approvalOutcome": "allowed-once"}),
            ),
            (
                None,
                json!({"expectedAnswerMatched": true, "answerCount": 3}),
            ),
            (Some("ASK_CANCELLED"), json!({})),
            (Some("ASK_ABORTED"), json!({})),
        ]
        .iter()
        .enumerate()
        {
            let ordinal = index + 1;
            let mut phase =
                json!({"phase": ordinal, "settled": true, "actualAgentId": "PUBLIC-agent"});
            for (key, value) in extra.as_object().unwrap() {
                phase[key] = value.clone();
            }
            if let Some(code) = code {
                phase["code"] = json!(code);
            }
            private(
                &control.join(format!("phase-{ordinal}.json")),
                serde_json::to_string(&phase).unwrap().as_bytes(),
            );
        }
        private(
            &control.join("complete.json"),
            serde_json::to_string(&value["host"]).unwrap().as_bytes(),
        );
        for name in ["decision-1.png", "decision-2.png"] {
            private(&base.join(name), &png());
        }
    }

    #[test]
    fn complete_decision_evidence_passes_and_each_check_is_required() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "decisions-").unwrap();
        let value = app();
        evidence(&base, &value);
        assert!(valid_reports(&base, &value, 4242, 1.0, "full"));
        assert!(valid_captures(&base));

        // The minimum viewport preset needs its own logical bounds.
        assert!(!valid_reports(&base, &value, 4242, 1.0, "minimum"));
        let mut minimum = value.clone();
        minimum["logicalViewport"] = json!([608, 448]);
        assert!(valid_reports(&base, &minimum, 4242, 1.0, "minimum"));

        // A substituted live-root identity or UI cancel is refused.
        let mut substituted = value.clone();
        substituted["hostAbortCancelObserved"] = json!(false);
        assert!(!valid_reports(&base, &substituted, 4242, 1.0, "full"));
        // A model step breaks the expected host counts.
        let mut stepped = value.clone();
        stepped["host"]["counts"]["steps"] = json!(1);
        let control = base.join("fixture/control");
        fs::write(
            control.join("complete.json"),
            serde_json::to_string(&stepped["host"]).unwrap(),
        )
        .unwrap();
        assert!(!valid_reports(&base, &stepped, 4242, 1.0, "full"));
        evidence(&base, &value);

        // The wrong branch code is refused.
        fs::write(
            control.join("phase-3.json"),
            serde_json::to_string(&json!({"phase": 3, "settled": true,
                "actualAgentId": "PUBLIC-agent", "code": "ASK_ABORTED"}))
            .unwrap(),
        )
        .unwrap();
        assert!(!valid_reports(&base, &value, 4242, 1.0, "full"));
        evidence(&base, &value);

        // A public capture mode is refused, as is a symlinked one.
        fs::set_permissions(
            base.join("decision-1.png"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(!valid_captures(&base));
        fs::set_permissions(
            base.join("decision-1.png"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        fs::remove_file(base.join("decision-1.png")).unwrap();
        std::os::unix::fs::symlink(base.join("decision-2.png"), base.join("decision-1.png"))
            .unwrap();
        assert!(!valid_captures(&base));
        private(&base.join("decision-1.png"), &png());

        // A symlinked or non-object private report is refused.
        let ready = control.join("ready.json");
        fs::remove_file(&ready).unwrap();
        std::os::unix::fs::symlink(control.join("complete.json"), &ready).unwrap();
        assert!(!valid_reports(&base, &value, 4242, 1.0, "full"));
        fs::remove_file(&ready).unwrap();
        private(&ready, b"[]");
        assert!(!valid_reports(&base, &value, 4242, 1.0, "full"));
        evidence(&base, &value);

        // With everything restored it passes again.
        assert!(valid_reports(&base, &value, 4242, 1.0, "full"));
        fs::remove_dir_all(base).unwrap();
    }
}
