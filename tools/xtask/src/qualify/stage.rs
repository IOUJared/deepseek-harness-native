//! Owned actual native-worker selected-file intake, with independent static bytes.
//!
//! Port of `scripts/qualify-native-file-stage.py`. The worker runs through the
//! shared ownership runner; the storage check reuses the upload qualifier's
//! independent private-store verification, and the refusal fixture must never
//! appear in the store.
use super::composed::{OwnedRun, run_owned};
use super::upload::{sha256_hex, verify_local_cases};
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `(case, body, name)` for the three stored fixtures.
pub fn fixtures() -> Vec<(&'static str, Vec<u8>, &'static str)> {
    vec![
        (
            "raw",
            (0..5003usize)
                .map(|index| (index * 43 + 19) as u8)
                .collect(),
            "PUBLIC owned file.bin",
        ),
        ("empty", Vec::new(), "PUBLIC empty.bin"),
        (
            "ceiling",
            (0..4 * 1024 * 1024usize)
                .map(|index| ((index * 13 + 7) % 256) as u8)
                .collect(),
            "PUBLIC ceiling.bin",
        ),
    ]
}

pub const REFUSED: &[u8] = b"PUBLIC refusal fixture must never be stored";

fn base_snapshot() -> Value {
    json!(["permission/preset", "sandbox/mode", "approval/policy"])
}

pub fn valid_worker(value: &Value) -> bool {
    let stop = &value["stop"];
    let expected: Vec<Value> = fixtures()
        .iter()
        .map(|(_, data, name)| json!({"name": name, "bytes": data.len()}))
        .collect();
    value["status"].as_str() == Some("passed")
        && value["version"].as_str() == Some("0.2.1-alpha.1")
        && value["files"] == json!(expected)
        && value["foreignDiscardRefusals"].as_i64() == Some(3)
        && value["preSnapshotRefused"].as_bool() == Some(true)
        && value["staleSelectionRefused"].as_bool() == Some(true)
        && value["unexpectedWork"].as_i64() == Some(0)
        && value["explicitModelPrompts"].as_i64() == Some(0)
        && value["snapshots"] == json!([base_snapshot(), base_snapshot()])
        && value["nativeWorkerExercised"].as_bool() == Some(true)
        && [
            "modelCatalogRequested",
            "processWideNetworkTrace",
            "uiExercised",
        ]
        .iter()
        .all(|key| value[*key].as_bool() == Some(false))
        && stop["exited"].as_bool() == Some(true)
        && stop["graceful"].as_bool() == Some(true)
        && stop["containmentUnknown"].as_bool() == Some(false)
        && stop["exitCode"].as_i64() == Some(0)
        && stop["observedDescendantsRemaining"].as_i64() == Some(0)
}

pub(super) fn workspace_fixtures(output: &Path) -> Result<(), String> {
    for (_, data, name) in fixtures() {
        let path = output.join("workspace").join(name);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|error| error.to_string())?;
        std::io::Write::write_all(&mut file, &data).map_err(|error| error.to_string())?;
    }
    let refused = output.join("workspace").join("PUBLIC refused.bin");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&refused)
        .map_err(|error| error.to_string())?;
    std::io::Write::write_all(&mut file, REFUSED).map_err(|error| error.to_string())?;
    Ok(())
}

/// `xtask qualify native-file-stage`.
pub fn run(root: &Path, output: &Path) -> i32 {
    let example = root.join("app/target/debug/examples/file_stage_smoke");
    let runtime = root
        .parent()
        .map(|parent| parent.join("deepseek-harness-linux/apps/cli"))
        .unwrap_or_default();
    let spec = OwnedRun {
        script: &example,
        report_name: "worker.json",
        report_key: "worker",
        evidence_env: "DSH_NATIVE_FILE_STAGE_EVIDENCE",
        scope: Some(
            "actual headless native worker/owned blocking file intake/Core/public alpha raw HTTP; not chooser, GUI or prompt binding",
        ),
        command: {
            let example = example.to_string_lossy().into_owned();
            let runtime = runtime.to_string_lossy().into_owned();
            Box::new(move |output: &Path| {
                let _ = workspace_fixtures(output);
                vec![
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
                ]
            })
        },
        capture_stdout: true,
        timeout: Duration::from_secs(35),
    };
    let owned = match run_owned(root, output, &spec, valid_worker) {
        Ok(owned) => owned,
        Err(message) => {
            eprintln!("native file stage failed: {message}");
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
        "ownedWorkerStatus": owned["status"],
        "scope": "complete static source/default-provider bytes, modes and aliases; selected header and ticket admission/refusal; no real input/model or generic provider guarantees",
    });
    if owned["status"].as_str() == Some("passed") && valid_worker(&owned["worker"]) {
        let fixtures = fixtures();
        match verify_local_cases(&resolved, &fixtures) {
            Ok(storage) => {
                let digest = sha256_hex(REFUSED);
                let refused: PathBuf = resolved
                    .join("harness/attachments/v1/file-objects")
                    .join(&digest[..2])
                    .join(&digest);
                if refused.symlink_metadata().is_ok() {
                    summary["refusal"] = json!("refused-file-was-published");
                } else {
                    summary["storage"] = json!(storage);
                    summary["refusedFixtureNotStored"] = json!(true);
                    summary["status"] = json!("passed");
                }
            }
            Err(error) => summary["refusal"] = json!(error),
        }
    } else {
        summary["refusal"] = json!("invalid-owned-worker-or-storage-evidence");
    }
    if let Err(error) =
        super::private_json(&resolved.join("native-stage-qualification.json"), &summary)
    {
        eprintln!("cannot write the stage report: {error}");
        return 1;
    }
    println!(
        "{}",
        json!({"status": summary["status"], "report": resolved.join("native-stage-qualification.json").to_string_lossy()})
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

    fn worker() -> Value {
        let files: Vec<Value> = fixtures()
            .iter()
            .map(|(_, data, name)| json!({"name": name, "bytes": data.len()}))
            .collect();
        json!({
            "status": "passed", "version": "0.2.1-alpha.1", "files": files,
            "foreignDiscardRefusals": 3, "preSnapshotRefused": true, "staleSelectionRefused": true,
            "unexpectedWork": 0, "explicitModelPrompts": 0,
            "snapshots": [base_snapshot(), base_snapshot()],
            "nativeWorkerExercised": true, "modelCatalogRequested": false,
            "processWideNetworkTrace": false, "uiExercised": false,
            "stop": {"exited": true, "graceful": true, "containmentUnknown": false,
                     "exitCode": 0, "observedDescendantsRemaining": 0},
        })
    }

    #[test]
    fn a_worker_report_needs_the_exact_files_refusals_and_an_owned_stop() {
        assert!(valid_worker(&worker()));
        let mut short = worker();
        short["files"][0]["bytes"] = json!(1);
        assert!(!valid_worker(&short), "a wrong stored size is not a pass");
        let mut unrejected = worker();
        unrejected["foreignDiscardRefusals"] = json!(2);
        assert!(
            !valid_worker(&unrejected),
            "every foreign discard must be refused"
        );
        let mut extra_work = worker();
        extra_work["unexpectedWork"] = json!(1);
        assert!(!valid_worker(&extra_work));
        let mut stale = worker();
        stale["staleSelectionRefused"] = json!(false);
        assert!(!valid_worker(&stale));
        let mut uncontained = worker();
        uncontained["stop"]["containmentUnknown"] = json!(true);
        assert!(!valid_worker(&uncontained));
        assert!(!valid_worker(&json!({})), "a truncated report is not valid");
    }

    #[test]
    fn the_fixture_bodies_are_deterministic_and_distinct() {
        let fixtures = fixtures();
        assert_eq!(fixtures[0].1.len(), 5003);
        assert_eq!(fixtures[2].1.len(), 4 * 1024 * 1024);
        assert!(fixtures[1].1.is_empty());
        assert_ne!(
            sha256_hex(&fixtures[0].1),
            sha256_hex(&fixtures[1].1),
            "raw and empty must not share an object"
        );
        assert_ne!(sha256_hex(REFUSED), sha256_hex(&fixtures[0].1));
    }
}
