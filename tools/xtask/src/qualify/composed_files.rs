//! Owned actual App/Worker/file admission, keyless catalog and joined draft Tasks.
//!
//! Port of `scripts/qualify-composed-files.py`. Besides the driver report, this
//! cross-checks the raw Host receipts, re-verifies storage, and checks the four
//! captured PNGs by bounded private read — headers and dimensions only. Pixels
//! are not decoded and paint is not inspected.
use super::composed::{OwnedRun, run_owned};
use super::prompt::counts;
use super::stage::{REFUSED, fixtures, workspace_fixtures};
use super::upload::{sha256_hex, verify_local_cases};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::FromRawFd;
use std::path::Path;
use std::time::Duration;

const CAPTURES: [&str; 4] = [
    "file-review.png",
    "file-ready-1.png",
    "file-ready-2.png",
    "file-records.png",
];
const MAX_PNG: u64 = 32 * 1024 * 1024;
const MAX_JSON: u64 = 16384;

fn counters() -> Vec<(&'static str, i64)> {
    vec![
        ("nativeAcks", 3),
        ("actualFileStages", 3),
        ("hostSettlements", 3),
        ("explicitPrompts", 3),
        ("actualAppPromptCommands", 3),
        ("currentTicketProbeRefusals", 6),
        ("staleUploadRefusals", 3),
        ("duplicateSendRefusals", 6),
        ("staleRemoveRefusals", 2),
        ("staleAckRefusals", 2),
        ("unexpectedWork", 0),
        ("explicitModelPrompts", 0),
        ("manualBusinessMessagesForwarded", 0),
        ("recordsDisclosureRoundTrips", 1),
        ("modelCatalogReads", 1),
        ("disclosedRows", 6),
        ("conversationRows", 2),
        ("submittedInputs", 1),
        ("blockedNotices", 1),
        ("permissionPresets", 1),
        ("sandboxModes", 1),
        ("approvalPolicies", 1),
        ("routineClaims", 1),
    ]
}

const TRUE_FLAGS: [&str; 14] = [
    "actualAppExercised",
    "nativeWorkerExercised",
    "scriptedUiMessages",
    "actualRosterSucceeded",
    "ordinaryHeaderValidated",
    "actualRootAgentCorrelated",
    "publicFixtureOnly",
    "submittedInputProjectionObserved",
    "retainedRecordsRevealed",
    "modelCatalogRequested",
    "modelCatalogSucceeded",
    "draftRecoveryLoaded",
    "draftIoJoined",
    "draftsCleanAtExit",
];
const FALSE_FLAGS: [&str; 5] = [
    "browserSignInRequested",
    "realCredentialsUsed",
    "processWideNetworkTrace",
    "nativeKeyboardPointerInputQualified",
    "historyDrivenReceiptRetirementQualified",
];

fn expected_files() -> Value {
    json!(
        fixtures()
            .iter()
            .map(|(_, data, name)| json!({"name": name, "bytes": data.len()}))
            .collect::<Vec<Value>>()
    )
}

fn expected_refs() -> Vec<Value> {
    fixtures()
        .iter()
        .map(|(_, data, name)| {
            json!({
                "attachmentId": format!("sha256:{}", sha256_hex(data)),
                "name": name,
                "bytes": data.len(),
            })
        })
        .collect()
}

fn stop() -> Value {
    json!({"exited": true, "exitCode": 0, "graceful": true,
           "containmentUnknown": false, "observedDescendantsRemaining": 0})
}

/// The ticket/request regex: `native-<pid>-<epoch>-<generation>-<ordinal>`.
fn parse_request(request: &str) -> Option<(i64, i64, i64, i64)> {
    let rest = request.strip_prefix("native-")?;
    let parts: Vec<&str> = rest.split('-').collect();
    if parts.len() != 4 {
        return None;
    }
    let number = |part: &str| -> Option<i64> {
        if part.is_empty()
            || part.starts_with('0')
            || !part.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        part.parse().ok()
    };
    let ordinal = number(parts[3])?;
    if !(1..=3).contains(&ordinal) {
        return None;
    }
    Some((
        number(parts[0])?,
        number(parts[1])?,
        number(parts[2])?,
        ordinal,
    ))
}

/// The driver report, not its independent on-disk storage/capture/Host evidence.
pub fn valid_app(value: &Value, scale: f64) -> bool {
    if (scale != 1.0 && scale != 1.25)
        || value["status"].as_str() != Some("passed")
        || value["version"].as_str() != Some("0.2.1-alpha.1")
        || value["pid"].as_i64().is_none_or(|pid| pid <= 0)
        || value["applicationScale"].as_f64() != Some(scale)
        || value["windowBackend"].as_str() != Some("wayland")
        || counters()
            .iter()
            .any(|(key, expected)| value[*key].as_i64() != Some(*expected))
        || TRUE_FLAGS
            .iter()
            .any(|key| value[*key].as_bool() != Some(true))
        || FALSE_FLAGS
            .iter()
            .any(|key| value[*key].as_bool() != Some(false))
        || value["files"] != expected_files()
        || value["captures"] != json!(CAPTURES)
        || value["requests"].as_array().map(Vec::len) != Some(3)
        || value["tickets"].as_array().map(Vec::len) != Some(3)
        || value["phases"].as_array().map(Vec::len) != Some(3)
        || value["stop"] != stop()
    {
        return false;
    }
    let (Some(tickets), Some(requests), Some(phases)) = (
        value["tickets"].as_array(),
        value["requests"].as_array(),
        value["phases"].as_array(),
    ) else {
        return false;
    };
    let first = &tickets[0];
    for (index, ((ticket, request), phase)) in tickets.iter().zip(requests).zip(phases).enumerate()
    {
        let n = index as i64 + 1;
        let ticket_ok = ticket.as_object().is_some_and(|object| {
            object
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>()
                == ["epoch", "generation", "serial", "target"]
                    .into_iter()
                    .collect()
        }) && ["epoch", "generation", "serial"]
            .iter()
            .all(|key| ticket[*key].as_i64().is_some_and(|value| value > 0))
            && ticket["serial"].as_i64() == Some(n)
            && ticket["epoch"] == first["epoch"]
            && ticket["generation"] == first["generation"]
            && ticket["target"] == first["target"]
            && ticket["target"].as_str().is_some_and(|target| {
                !target.is_empty() && target.len() <= 512 && !target.contains('\0')
            });
        let Some(request) = request.as_str() else {
            return false;
        };
        let request_ok =
            parse_request(request).is_some_and(|(pid, _epoch, generation, ordinal)| {
                pid == value["pid"].as_i64().unwrap_or(-1)
                    && generation == ticket["generation"].as_i64().unwrap_or(-3)
                    && ordinal == n
            });
        let reference = &expected_refs()[index];
        let phase_ok = phase["phase"].as_i64() == Some(n)
            && phase["requestId"].as_str() == Some(request)
            && phase["file"] == *reference
            && counts(&phase["counts"], n)
            && [
                "rootRegistered",
                "durableInboxFileMatched",
                "claimedFileMatched",
                "blockedTurn",
                "zeroAdmittedModelSteps",
            ]
            .iter()
            .all(|key| phase[*key].as_bool() == Some(true));
        if !ticket_ok || !request_ok || !phase_ok {
            return false;
        }
    }
    let host = &value["host"];
    let unique = requests
        .iter()
        .filter_map(Value::as_str)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        == 3;
    unique
        && host["status"].as_str() == Some("passed")
        && ["rootDisposed", "turnClosed", "zeroAdmittedModelSteps"]
            .iter()
            .all(|key| host[*key].as_bool() == Some(true))
        && counts(&host["counts"], 3)
}

/// Read a bounded owner-private regular artifact, never following a final symlink.
pub fn private_bytes(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let info = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !info.file_type().is_file()
        || info.mode() & 0o777 != 0o600
        || info.uid() != unsafe { libc::geteuid() }
        || info.size() == 0
        || info.size() > limit
    {
        return Err("private-regular-artifact-required".into());
    }
    let c_path =
        std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| "path-with-nul")?;
    // Safe: a NUL-terminated path and constant flags; the descriptor is closed below.
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err("private-artifact-not-openable".into());
    }
    let mut opened: libc::stat = unsafe { std::mem::zeroed() };
    // Safe: a live descriptor and a valid out-parameter.
    let stat_ok = unsafe { libc::fstat(fd, &mut opened) } == 0;
    let matches = stat_ok
        && opened.st_dev == info.dev()
        && opened.st_ino == info.ino()
        && opened.st_mode & libc::S_IFMT == libc::S_IFREG
        && opened.st_mode & 0o777 == 0o600
        && opened.st_uid == info.uid()
        && opened.st_size as u64 == info.size();
    if !matches {
        unsafe { libc::close(fd) };
        return Err("private-artifact-changed".into());
    }
    let mut file = unsafe { fs::File::from_raw_fd(fd) };
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|error| error.to_string())?;
    if data.len() as u64 != info.size() || data.len() as u64 > limit {
        return Err("private-artifact-size-changed".into());
    }
    Ok(data)
}

fn private_json(path: &Path, limit: u64) -> Result<Value, String> {
    let data = private_bytes(path, limit)?;
    let value: Value = serde_json::from_slice(&data).map_err(|error| error.to_string())?;
    if !value.is_object() {
        return Err("private-report-object-required".into());
    }
    Ok(value)
}

fn verify_captures(output: &Path, app: &Value) -> Result<Vec<Value>, String> {
    if app["captures"] != json!(CAPTURES) {
        return Err("capture-list-mismatch".into());
    }
    let mut captures = Vec::new();
    for name in CAPTURES {
        let data = private_bytes(&output.join(name), MAX_PNG)?;
        if data.len() < 33
            || data[..8] != *b"\x89PNG\r\n\x1a\n"
            || data[8..16] != *b"\x00\x00\x00\x0dIHDR"
        {
            return Err("capture-png-header-invalid".into());
        }
        let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
            return Err("capture-dimensions-invalid".into());
        }
        captures.push(json!({
            "artifact": name,
            "bytes": data.len(),
            "sha256": sha256_hex(&data),
            "physicalSize": [width, height],
            "privateMode": "0600",
        }));
    }
    Ok(captures)
}

/// Cross-check raw Host receipts and independently open fixed storage/capture paths.
fn verify_evidence(output: &Path, app: &Value, scale: f64) -> Result<Value, String> {
    if !valid_app(app, scale) {
        return Err("composed-app-report-invalid".into());
    }
    let control = output.join("fixture/control");
    let ready = private_json(&control.join("ready.json"), MAX_JSON)?;
    if ready["rootRegistered"].as_bool() != Some(true)
        || ready["headerVersion"].as_i64() != Some(4)
        || ready["ordinary"].as_bool() != Some(true)
        || ready["actualAgentId"] != app["tickets"][0]["target"]
    {
        return Err("actual-root-ticket-correlation-invalid".into());
    }
    let phases = app["phases"].as_array().ok_or("missing phases")?;
    for (index, phase) in phases.iter().enumerate() {
        let name = format!("phase-{}.json", index + 1);
        if private_json(&control.join(name), MAX_JSON)? != *phase {
            return Err("raw-host-phase-mismatch".into());
        }
    }
    if private_json(&control.join("complete.json"), MAX_JSON)? != app["host"]
        || control.join("failed.json").symlink_metadata().is_ok()
    {
        return Err("raw-host-completion-invalid".into());
    }
    let storage = verify_local_cases(output, &fixtures())?;
    let digest = sha256_hex(REFUSED);
    if output
        .join("harness/attachments/v1/file-objects")
        .join(&digest[..2])
        .join(&digest)
        .symlink_metadata()
        .is_ok()
    {
        return Err("refused-fixture-was-stored".into());
    }
    Ok(json!({
        "storage": storage,
        "captures": verify_captures(output, app)?,
        "separateHostReceiptsVerified": true,
        "actualRootTicketCorrelationVerified": true,
        "refusedFixtureNotStored": true,
        "paintInspected": false,
        "pngHeaderAndDimensionsChecked": true,
        "pngPixelsDecoded": false,
    }))
}

/// `xtask qualify composed-files --output NEW_DIR --scale 1.0|1.25`.
pub fn run(root: &Path, output: &Path, scale: f64) -> i32 {
    let example = root.join("app/target/debug/examples/file_composed_smoke");
    let runtime = root
        .parent()
        .map(|parent| parent.join("deepseek-harness-linux/apps/cli"))
        .unwrap_or_default();
    let spec = OwnedRun {
        script: &example,
        report_name: "app.json",
        report_key: "app",
        evidence_env: "DSH_NATIVE_COMPOSED_FILES_EVIDENCE",
        scope: Some(
            "actual App/Worker raw upload/private PromptFile binding/full Alpha Host, keyless catalog and joined draft Tasks; isolated reject fixture/scripted window, not physical input, model history or full-process restart",
        ),
        command: {
            let example = example.to_string_lossy().into_owned();
            let runtime = runtime.to_string_lossy().into_owned();
            Box::new(move |output: &Path| {
                let _ = workspace_fixtures(output);
                let mut argv = vec![
                    example.clone(),
                    "--runtime".into(),
                    runtime.clone(),
                    "--expected-version".into(),
                    "0.2.1-alpha.1".into(),
                    "--home".into(),
                    output.join("harness").to_string_lossy().into_owned(),
                    "--cwd".into(),
                    output.join("workspace").to_string_lossy().into_owned(),
                    "--user-home".into(),
                    output.join("user").to_string_lossy().into_owned(),
                ];
                argv.push("--scale".into());
                argv.push(format!("{scale}"));
                argv
            })
        },
        capture_stdout: true,
        timeout: Duration::from_secs(35),
    };
    let owned = match run_owned(root, output, &spec, |value| valid_app(value, scale)) {
        Ok(owned) => owned,
        Err(message) => {
            eprintln!("composed files failed: {message}");
            return 1;
        }
    };
    let resolved = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(output))
            .unwrap_or_else(|_| output.to_path_buf())
    };
    let mut summary = json!({
        "status": "failed",
        "scope": "Owned actual App/Worker/file admission, keyless catalog and joined draft Tasks; no physical input, model history or full-process restart proof.",
        "paintInspected": false,
        "nativeKeyboardPointerInputQualified": false,
        "historyDrivenReceiptRetirementQualified": false,
        "ownedAppStatus": owned["status"],
    });
    if owned["status"].as_str() == Some("passed") {
        match verify_evidence(&resolved, &owned["app"], scale) {
            Ok(evidence) => {
                if let Some(object) = summary.as_object_mut()
                    && let Some(fields) = evidence.as_object()
                {
                    object.extend(fields.clone());
                }
                summary["status"] = json!("passed");
            }
            Err(error) => summary["refusal"] = json!(error),
        }
    } else {
        summary["refusal"] = json!("invalid-owned-app-or-independent-file-evidence");
    }
    if let Err(error) = super::private_json(
        &resolved.join("composed-files-qualification.json"),
        &summary,
    ) {
        eprintln!("cannot write the composed-files report: {error}");
        return 1;
    }
    println!(
        "{}",
        json!({"status": summary["status"], "report": resolved.join("composed-files-qualification.json").to_string_lossy()})
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

    pub(super) fn app() -> Value {
        let mut counters_object = serde_json::Map::new();
        for (key, value) in counters() {
            counters_object.insert(key.to_string(), json!(value));
        }
        let requests: Vec<Value> = (1..=3)
            .map(|n| json!(format!("native-1234-7-9-{n}")))
            .collect();
        let tickets: Vec<Value> = (1..=3)
            .map(|n| json!({"epoch": 7, "generation": 9, "serial": n, "target": "PUBLIC-agent"}))
            .collect();
        let phases: Vec<Value> = (1..=3)
            .map(|n| {
                json!({
                    "phase": n,
                    "requestId": format!("native-1234-7-9-{n}"),
                    "file": expected_refs()[(n - 1) as usize],
                    "counts": {
                        "turnStart": n, "turnEnd": n, "blockedTurns": n,
                        "inboxInserted": n, "inboxClaims": n, "preSteps": n,
                        "steps": 0, "stepEnds": 0, "requestHeaders": 0, "toolCalls": 0,
                        "toolResults": 0, "assistantMessages": 0, "userMessages": 0,
                    },
                    "rootRegistered": true, "durableInboxFileMatched": true,
                    "claimedFileMatched": true, "blockedTurn": true,
                    "zeroAdmittedModelSteps": true,
                })
            })
            .collect();
        let mut value = json!({
            "status": "passed", "version": "0.2.1-alpha.1", "pid": 1234,
            "applicationScale": 1.0, "windowBackend": "wayland",
            "files": expected_files(), "captures": CAPTURES,
            "requests": requests, "tickets": tickets, "phases": phases,
            "host": {"status": "passed", "rootDisposed": true, "turnClosed": true,
                     "zeroAdmittedModelSteps": true,
                     "counts": {"turnStart": 3, "turnEnd": 3, "blockedTurns": 3,
                                 "inboxInserted": 3, "inboxClaims": 3, "preSteps": 3,
                                 "steps": 0, "stepEnds": 0, "requestHeaders": 0,
                                 "toolCalls": 0, "toolResults": 0, "assistantMessages": 0,
                                 "userMessages": 0}},
            "stop": stop(),
        });
        for (key, _) in counters() {
            value[key] = json!(counters().iter().find(|(name, _)| name == &key).unwrap().1);
        }
        for key in TRUE_FLAGS {
            value[key] = json!(true);
        }
        for key in FALSE_FLAGS {
            value[key] = json!(false);
        }
        value
    }

    #[test]
    fn a_composed_report_needs_the_exact_ticket_request_and_phase_chain() {
        assert!(valid_app(&app(), 1.0));
        let mut wrong_scale = app();
        wrong_scale["applicationScale"] = json!(1.25);
        assert!(!valid_app(&wrong_scale, 1.0));
        let mut replayed = app();
        replayed["requests"][1] = json!("native-1234-7-9-1");
        assert!(
            !valid_app(&replayed, 1.0),
            "a replayed ordinal is not a pass"
        );
        let mut foreign_pid = app();
        foreign_pid["requests"][0] = json!("native-9999-7-9-1");
        assert!(!valid_app(&foreign_pid, 1.0));
        let mut other_ticket = app();
        other_ticket["tickets"][2]["generation"] = json!(10);
        assert!(!valid_app(&other_ticket, 1.0));
        let mut stepped = app();
        stepped["phases"][0]["counts"]["steps"] = json!(1);
        assert!(!valid_app(&stepped, 1.0));
        let mut uncounted = app();
        uncounted["routineClaims"] = json!(2);
        assert!(!valid_app(&uncounted, 1.0));
        assert!(
            !valid_app(&json!({}), 1.0),
            "a truncated report is not valid"
        );
    }

    #[test]
    fn a_capture_needs_a_bounded_private_png_with_sane_dimensions() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "captures-").unwrap();
        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR");
        png.extend_from_slice(&1200u32.to_be_bytes());
        png.extend_from_slice(&800u32.to_be_bytes());
        png.extend_from_slice(&[0u8; 9]);
        for name in CAPTURES {
            let path = base.join(name);
            fs::write(&path, &png).unwrap();
            fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))
                .unwrap();
        }
        let mut app = app();
        let captures = verify_captures(&base, &app).expect("all four private captures");
        assert_eq!(captures.len(), 4);
        assert_eq!(captures[0]["physicalSize"], json!([1200, 800]));
        // A public mode is not a private artifact.
        fs::set_permissions(
            base.join(CAPTURES[0]),
            std::os::unix::fs::PermissionsExt::from_mode(0o644),
        )
        .unwrap();
        assert!(verify_captures(&base, &app).is_err());
        fs::set_permissions(
            base.join(CAPTURES[0]),
            std::os::unix::fs::PermissionsExt::from_mode(0o600),
        )
        .unwrap();
        // Zero dimensions are rejected.
        let mut bad = png.clone();
        bad[16..20].copy_from_slice(&0u32.to_be_bytes());
        fs::write(base.join(CAPTURES[0]), &bad).unwrap();
        assert!(verify_captures(&base, &app).is_err());
        // The capture list is part of the proof.
        app["captures"] = json!(["file-review.png"]);
        assert!(verify_captures(&base, &app).is_err());
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod evidence_tests {
    use super::*;
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
        png.extend_from_slice(&1200u32.to_be_bytes());
        png.extend_from_slice(&800u32.to_be_bytes());
        png.extend_from_slice(&[0u8; 9]);
        png
    }

    /// Build the private store for the stage fixtures, with hardlinked aliases.
    fn store(output: &Path) {
        let root = output.join("harness/attachments/v1");
        for directory in [
            output.to_path_buf(),
            output.join("harness"),
            output.join("harness/attachments"),
            root.clone(),
            root.join("file-objects"),
            root.join("files"),
        ] {
            fs::create_dir_all(&directory).unwrap();
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        }
        for (_, data, name) in fixtures() {
            let digest = sha256_hex(&data);
            let objects = root.join("file-objects").join(&digest[..2]);
            let aliases = root.join("files").join(&digest[..2]).join(&digest);
            for directory in [&objects, &aliases] {
                fs::create_dir_all(directory).unwrap();
                fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let canonical = objects.join(&digest);
            let alias = aliases.join(name);
            // Rebuilding replaces a previous read-only object, so drop both links first.
            let _ = fs::remove_file(&alias);
            let _ = fs::remove_file(&canonical);
            fs::write(&canonical, &data).unwrap();
            fs::set_permissions(&canonical, fs::Permissions::from_mode(0o400)).unwrap();
            fs::hard_link(&canonical, &alias).unwrap();
        }
    }

    fn evidence(output: &Path, app: &Value) {
        store(output);
        let control = output.join("fixture/control");
        private(
            &control.join("ready.json"),
            serde_json::to_string(&json!({
                "rootRegistered": true, "headerVersion": 4, "ordinary": true,
                "actualAgentId": app["tickets"][0]["target"],
            }))
            .unwrap()
            .as_bytes(),
        );
        for (index, phase) in app["phases"].as_array().unwrap().iter().enumerate() {
            private(
                &control.join(format!("phase-{}.json", index + 1)),
                serde_json::to_string(phase).unwrap().as_bytes(),
            );
        }
        private(
            &control.join("complete.json"),
            serde_json::to_string(&app["host"]).unwrap().as_bytes(),
        );
        for name in CAPTURES {
            private(&output.join(name), &png());
        }
    }

    fn app() -> Value {
        // Reuse the driver fixture from the validator tests.
        super::tests::app()
    }

    #[test]
    fn complete_evidence_passes_and_each_cross_check_is_required() {
        let base =
            crate::tempdir::make_temp_dir(&std::env::temp_dir(), "composed-evidence-").unwrap();
        let value = app();
        evidence(&base, &value);
        let proof = verify_evidence(&base, &value, 1.0).expect("complete evidence");
        assert_eq!(proof["separateHostReceiptsVerified"].as_bool(), Some(true));
        assert_eq!(proof["captures"].as_array().map(Vec::len), Some(4));

        // A foreign root ticket correlation is refused.
        fs::write(
            base.join("fixture/control/ready.json"),
            serde_json::to_string(&json!({
                "rootRegistered": true, "headerVersion": 4, "ordinary": true,
                "actualAgentId": "another-agent",
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(verify_evidence(&base, &value, 1.0).is_err());
        evidence(&base, &value);

        // A mismatched raw phase is refused.
        fs::write(
            base.join("fixture/control/phase-2.json"),
            serde_json::to_string(&json!({"phase": 2})).unwrap(),
        )
        .unwrap();
        assert!(verify_evidence(&base, &value, 1.0).is_err());
        evidence(&base, &value);

        // A failed marker invalidates the completion.
        private(&base.join("fixture/control/failed.json"), b"{}");
        assert!(verify_evidence(&base, &value, 1.0).is_err());
        fs::remove_file(base.join("fixture/control/failed.json")).unwrap();

        // A stored refusal fixture is refused.
        let digest = sha256_hex(REFUSED);
        let refused = base
            .join("harness/attachments/v1/file-objects")
            .join(&digest[..2])
            .join(&digest);
        fs::create_dir_all(refused.parent().unwrap()).unwrap();
        fs::write(&refused, REFUSED).unwrap();
        assert!(verify_evidence(&base, &value, 1.0).is_err());
        fs::remove_file(&refused).unwrap();

        // A symlinked capture is not a private artifact.
        fs::remove_file(base.join(CAPTURES[0])).unwrap();
        std::os::unix::fs::symlink(base.join(CAPTURES[1]), base.join(CAPTURES[0])).unwrap();
        assert!(verify_evidence(&base, &value, 1.0).is_err());
        fs::remove_file(base.join(CAPTURES[0])).unwrap();
        private(&base.join(CAPTURES[0]), &png());

        // A FIFO capture is refused without blocking.
        fs::remove_file(base.join(CAPTURES[2])).unwrap();
        let fifo =
            std::ffi::CString::new(base.join(CAPTURES[2]).to_string_lossy().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(verify_evidence(&base, &value, 1.0).is_err());
        fs::remove_file(base.join(CAPTURES[2])).unwrap();
        private(&base.join(CAPTURES[2]), &png());

        // With everything restored the evidence passes again.
        assert!(verify_evidence(&base, &value, 1.0).is_ok());
        fs::remove_dir_all(base).unwrap();
    }
}
