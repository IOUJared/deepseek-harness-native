//! Owned real native Worker/Core file-prompt ACK and durable inbox proof.
//!
//! Port of `scripts/qualify-native-file-prompt.py`. No admitted model step: the
//! counts prove three prompt ACKs, three durable inbox file references and three
//! claimed pre-step rejections, with every generation counter at zero.
use super::composed::{OwnedRun, run_owned};
use super::stage::{fixtures, workspace_fixtures};
use super::upload::{sha256_hex, verify_local_cases};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

const FIXED_COUNTS: [&str; 6] = [
    "turnStart",
    "turnEnd",
    "blockedTurns",
    "inboxInserted",
    "inboxClaims",
    "preSteps",
];
const ZERO_COUNTS: [&str; 7] = [
    "steps",
    "stepEnds",
    "requestHeaders",
    "toolCalls",
    "toolResults",
    "assistantMessages",
    "userMessages",
];

/// A counter group with the exact key set: the fixed counters equal `n`, the
/// generation counters zero.
pub(super) fn counts(value: &Value, n: i64) -> bool {
    value.as_object().is_some_and(|object| {
        object
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>()
            == FIXED_COUNTS
                .iter()
                .chain(ZERO_COUNTS.iter())
                .copied()
                .collect()
            && FIXED_COUNTS
                .iter()
                .all(|key| object[*key].as_i64() == Some(n))
            && ZERO_COUNTS
                .iter()
                .all(|key| object[*key].as_i64() == Some(0))
    })
}

pub fn valid_worker(value: &Value) -> bool {
    let stop = &value["stop"];
    let host = &value["host"];
    let expected_files: Vec<Value> = fixtures()
        .iter()
        .map(|(_, data, name)| json!({"name": name, "bytes": data.len()}))
        .collect();
    if value["status"].as_str() != Some("passed")
        || value["version"].as_str() != Some("0.2.1-alpha.1")
        || [
            "acceptedPrompts",
            "replayRefusals",
            "genericFileRefusals",
            "explicitPrompts",
        ]
        .iter()
        .any(|key| value[*key].as_i64() != Some(3))
        || ["unexpectedWork", "explicitModelPrompts"]
            .iter()
            .any(|key| value[*key].as_i64() != Some(0))
        || [
            "rootRegistered",
            "ordinaryHeaderValidated",
            "nativeWorkerExercised",
        ]
        .iter()
        .any(|key| value[*key].as_bool() != Some(true))
        || [
            "modelCatalogRequested",
            "browserSignInRequested",
            "realCredentialsUsed",
            "processWideNetworkTrace",
            "uiExercised",
            "historyDrivenReceiptRetirementQualified",
        ]
        .iter()
        .any(|key| value[*key].as_bool() != Some(false))
        || value["files"] != json!(expected_files)
        || value["snapshots"] != json!([["permission/preset", "sandbox/mode", "approval/policy"]])
        || value["phases"].as_array().map(Vec::len) != Some(3)
        || host["status"].as_str() != Some("passed")
        || ["rootDisposed", "turnClosed", "zeroAdmittedModelSteps"]
            .iter()
            .any(|key| host[*key].as_bool() != Some(true))
        || !counts(&host["counts"], 3)
        || stop["exited"].as_bool() != Some(true)
        || stop["graceful"].as_bool() != Some(true)
        || stop["containmentUnknown"].as_bool() != Some(false)
        || stop["exitCode"].as_i64() != Some(0)
        || stop["observedDescendantsRemaining"].as_i64() != Some(0)
    {
        return false;
    }
    value["phases"].as_array().is_some_and(|phases| {
        phases
            .iter()
            .zip(fixtures())
            .enumerate()
            .all(|(index, (phase, (_, data, name)))| {
                let n = index as i64 + 1;
                phase["phase"].as_i64() == Some(n)
                    && phase["requestId"].as_str()
                        == Some(&format!("PUBLIC_native_file_prompt_{n}"))
                    && phase["file"]
                        == json!({
                            "attachmentId": format!("sha256:{}", sha256_hex(&data)),
                            "name": name,
                            "bytes": data.len(),
                        })
                    && counts(&phase["counts"], n)
                    && [
                        "rootRegistered",
                        "durableInboxFileMatched",
                        "claimedFileMatched",
                        "blockedTurn",
                        "zeroAdmittedModelSteps",
                    ]
                    .iter()
                    .all(|key| phase[*key].as_bool() == Some(true))
            })
    })
}

/// `xtask qualify native-file-prompt`.
pub fn run(root: &Path, output: &Path) -> i32 {
    let example = root.join("app/target/debug/examples/file_prompt_smoke");
    let runtime = root
        .parent()
        .map(|parent| parent.join("deepseek-harness-linux/apps/cli"))
        .unwrap_or_default();
    let spec = OwnedRun {
        script: &example,
        report_name: "worker.json",
        report_key: "worker",
        evidence_env: "DSH_NATIVE_FILE_PROMPT_EVIDENCE",
        scope: Some(
            "actual native private file receipt-to-prompt, full Alpha public profile plus fixed isolated owned-Agent reject fixture; no model step/user-message history/GUI",
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
            eprintln!("native file prompt failed: {message}");
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
        "scope": "three real prompt ACKs/durable file refs in inbox/claimed pre-step rejection, conservative one-attempt native tickets; not model generation or history-driven receipt retirement",
    });
    if owned["status"].as_str() == Some("passed") && valid_worker(&owned["worker"]) {
        match verify_local_cases(&resolved, &fixtures()) {
            Ok(storage) => {
                summary["storage"] = json!(storage);
                summary["status"] = json!("passed");
            }
            Err(error) => summary["refusal"] = json!(error),
        }
    } else {
        summary["refusal"] = json!("invalid-owned-worker-or-storage-evidence");
    }
    if let Err(error) =
        super::private_json(&resolved.join("native-prompt-qualification.json"), &summary)
    {
        eprintln!("cannot write the prompt report: {error}");
        return 1;
    }
    println!(
        "{}",
        json!({"status": summary["status"], "report": resolved.join("native-prompt-qualification.json").to_string_lossy()})
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
        let phases: Vec<Value> = fixtures()
            .iter()
            .enumerate()
            .map(|(index, (_, data, name))| {
                let n = index + 1;
                json!({
                    "phase": n,
                    "requestId": format!("PUBLIC_native_file_prompt_{n}"),
                    "file": {
                        "attachmentId": format!("sha256:{}", sha256_hex(data)),
                        "name": name,
                        "bytes": data.len(),
                    },
                    "counts": counts_value(n as i64),
                    "rootRegistered": true, "durableInboxFileMatched": true,
                    "claimedFileMatched": true, "blockedTurn": true,
                    "zeroAdmittedModelSteps": true,
                })
            })
            .collect();
        json!({
            "status": "passed", "version": "0.2.1-alpha.1",
            "acceptedPrompts": 3, "replayRefusals": 3, "genericFileRefusals": 3,
            "explicitPrompts": 3, "unexpectedWork": 0, "explicitModelPrompts": 0,
            "rootRegistered": true, "ordinaryHeaderValidated": true, "nativeWorkerExercised": true,
            "modelCatalogRequested": false, "browserSignInRequested": false,
            "realCredentialsUsed": false, "processWideNetworkTrace": false,
            "uiExercised": false, "historyDrivenReceiptRetirementQualified": false,
            "files": files, "snapshots": [["permission/preset", "sandbox/mode", "approval/policy"]],
            "phases": phases,
            "host": {"status": "passed", "rootDisposed": true, "turnClosed": true,
                     "zeroAdmittedModelSteps": true, "counts": counts_value(3)},
            "stop": {"exited": true, "graceful": true, "containmentUnknown": false,
                     "exitCode": 0, "observedDescendantsRemaining": 0},
        })
    }

    fn counts_value(n: i64) -> Value {
        let mut object = serde_json::Map::new();
        for key in FIXED_COUNTS {
            object.insert(key.to_string(), json!(n));
        }
        for key in ZERO_COUNTS {
            object.insert(key.to_string(), json!(0));
        }
        Value::Object(object)
    }

    #[test]
    fn a_prompt_worker_needs_three_acks_and_zero_generation_counters() {
        assert!(valid_worker(&worker()));
        let mut extra = worker();
        extra["host"]["counts"]["extra"] = json!(0);
        assert!(!valid_worker(&extra), "an unknown counter is not a pass");
        let mut stepped = worker();
        stepped["phases"][0]["counts"]["steps"] = json!(1);
        assert!(
            !valid_worker(&stepped),
            "an admitted model step is not a pass"
        );
        let mut wrong_request = worker();
        wrong_request["phases"][1]["requestId"] = json!("PUBLIC_other");
        assert!(!valid_worker(&wrong_request));
        let mut model = worker();
        model["explicitModelPrompts"] = json!(1);
        assert!(!valid_worker(&model));
        let mut unclosed = worker();
        unclosed["host"]["turnClosed"] = json!(false);
        assert!(!valid_worker(&unclosed));
        assert!(!valid_worker(&json!({})), "a truncated report is not valid");
    }

    #[test]
    fn the_phase_counters_walk_with_the_phase_number() {
        let mut value = worker();
        value["phases"][2]["counts"]["turnStart"] = json!(2);
        assert!(!valid_worker(&value), "each phase must count its own turn");
    }
}
