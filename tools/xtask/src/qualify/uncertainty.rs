//! Actual public alpha HTTP uncertainty barriers, with independent byte/mode checks.
//!
//! Port of `scripts/qualify-upload-uncertainty.py`. An injected failure must be
//! *refused* and cleaned up, never counted as a successful upload. The three
//! validator shapes below are the whole point, so they are ported exactly.
use super::composed::{OwnedRun, run_owned};
use super::upload::{sha256_hex, verify_local_cases};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

pub const CASES: [&str; 3] = ["ack-loss", "scope", "late-disposal"];
const FACTS: [&str; 21] = [
    "authenticatedRawRoute",
    "storedBeforeAbort",
    "stagedBeforeAbort",
    "clientAbortedBeforeAck",
    "serverObservedDisconnect",
    "receiptRetainedAfterAckLoss",
    "foreignResolveRefused",
    "foreignBindingRefused",
    "foreignRetirementHarmless",
    "rollbackRetainedUnbound",
    "rollbackRestoredPrior",
    "committedRetirementExact",
    "oldSessionDisposalRetiredReceipt",
    "sameIdReplacementDifferentSession",
    "sameIdReplacementRefusedOldReceipt",
    "storedBeforeAgentDisposal",
    "disposedBeforeStorePromiseReturned",
    "lateDisposalRejectedNoReceipt",
    "handlersJoined",
    "descriptorsRestored",
    "registryReturnedToBaseline",
];
const COUNTS: [&str; 10] = [
    "turnStarts",
    "turnEnds",
    "stepStarts",
    "stepEnds",
    "requestHeaders",
    "toolCalls",
    "toolResults",
    "assistantMessages",
    "userMessages",
    "unexpectedWake",
];
const CLEANUP: [&str; 7] = [
    "handlersJoined",
    "descriptorsRestored",
    "observerDisposersRan",
    "ownedHandlesDisposed",
    "registryReturnedToBaseline",
    "hostShutdown",
    "drainTimedOut",
];

pub fn source(case: &str) -> Vec<u8> {
    let (count, step) = match case {
        "ack-loss" => (5003usize, 43usize),
        "scope" => (6117, 71),
        "late-disposal" => (7109, 29),
        _ => (0, 0),
    };
    (0..count)
        .map(|index| ((index * step + 19) % 256) as u8)
        .collect()
}

fn exact_keys(value: &Value, expected: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>()
            == expected.iter().copied().collect()
    })
}

fn all_bools(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.values().all(Value::is_boolean))
}

fn all_true(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.values().all(|value| value.as_bool() == Some(true)))
}

fn all_zero(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.values().all(|value| value.as_i64() == Some(0)))
}

/// The observation half: exact fact/count/bridge/cleanup sets and per-case files.
pub fn valid_observation(value: &Value, cases: &[&str]) -> bool {
    if value["fatal"].as_bool() != Some(false)
        || value["testOnlyInstanceBarriers"].as_bool() != Some(true)
        || [
            "scriptedModelPromptOrCatalog",
            "scriptedSignInOrCredentialSave",
            "providerSubstituted",
            "processWideNetworkTrace",
            "rustSdkExercised",
            "uiExercised",
        ]
        .iter()
        .any(|key| value[*key].as_bool() != Some(false))
        || !exact_keys(&value["facts"], &FACTS)
        || !all_bools(&value["facts"])
        || !exact_keys(&value["counts"], &COUNTS)
        || !all_zero(&value["counts"])
        || !exact_keys(
            &value["bridge"],
            &["active", "complete", "errors", "methodActive"],
        )
        || value["bridge"]
            .as_object()
            .is_some_and(|object| !object.values().all(Value::is_i64))
        || value["bridge"]
            != json!({"active": 0, "complete": cases.len(), "errors": 0, "methodActive": 0})
        || !exact_keys(&value["cleanup"], &CLEANUP)
        || value["cleanup"].as_object().is_none_or(|object| {
            object
                .iter()
                .any(|(key, flag)| flag.as_bool() != Some(key != "drainTimedOut"))
        })
    {
        return false;
    }
    value["files"].as_array().is_some_and(|files| {
        files.len() == cases.len()
            && files.iter().zip(cases).all(|(entry, case)| {
                entry["case"].as_str() == Some(case) && exact_keys(entry, &["case", "file"]) && {
                    let data = source(case);
                    let file = &entry["file"];
                    exact_keys(file, &["attachmentId", "name", "bytes"])
                        && file["attachmentId"].as_str()
                            == Some(&format!("sha256:{}", sha256_hex(&data)))
                        && file["name"].as_str() == Some(&format!("PUBLIC {case}.bin"))
                        && file["bytes"].as_i64() == Some(data.len() as i64)
                }
            })
    })
}

/// A clean pass: every fact true, phase complete, no injected failure reached.
pub fn valid_barriers(value: &Value) -> bool {
    valid_observation(value, &CASES)
        && value["status"].as_str() == Some("passed")
        && value["phase"].as_str() == Some("complete")
        && value["failurePoint"].as_str() == Some("none")
        && value["injectionReached"].as_bool() == Some(false)
        && all_true(&value["facts"])
}

fn expected_failure_shape(
    point: &str,
) -> (
    &'static [&'static str],
    &'static str,
    &'static [&'static str],
) {
    match point {
        "ack-held" => (
            &CASES[..1],
            "ack-lost-after-real-staging",
            &[
                "storedBeforeAbort",
                "stagedBeforeAbort",
                "clientAbortedBeforeAck",
                "serverObservedDisconnect",
            ],
        ),
        _ => (
            &CASES,
            "agent-disposed-after-real-save-before-upload-commit",
            &[
                "storedBeforeAgentDisposal",
                "sameIdReplacementRefusedOldReceipt",
            ],
        ),
    }
}

/// An injected barrier must leave an owned observation that failed at the right
/// point, with complete containment and the right facts still false.
pub fn valid_expected_failure(owned: &Value, point: &str) -> bool {
    if !matches!(point, "ack-held" | "store-held") {
        return false;
    }
    let (cases, phase, required) = expected_failure_shape(point);
    let value = &owned["barriers"];
    let cleanup = &owned["cleanup"];
    let cleanup_ok = cleanup["rootPidfdPinned"].as_bool() == Some(true)
        && cleanup["rootReaped"].as_bool() == Some(true)
        && cleanup["observedCleanupComplete"].as_bool() == Some(true)
        && [
            "rootWasRunningAtCleanup",
            "ownedChildFallbackUsed",
            "killEscalated",
            "cleanupTimedOut",
            "cleanupProbeOrSignalFailed",
        ]
        .iter()
        .all(|key| cleanup[*key].as_bool() == Some(false))
        && cleanup["remainingBeforeExternalCleanup"] == json!([])
        && cleanup["remainingAfterExternalCleanup"] == json!([]);
    let still_false: &[&str] = if point == "ack-held" {
        &["receiptRetainedAfterAckLoss", "storedBeforeAgentDisposal"]
    } else {
        &[
            "disposedBeforeStorePromiseReturned",
            "lateDisposalRejectedNoReceipt",
        ]
    };
    owned["status"].as_str() == Some("failed")
        && owned["exitCode"].as_i64() == Some(1)
        && owned["sourceUnchanged"].as_bool() == Some(true)
        && owned["identityPinFailures"].as_i64() == Some(0)
        && owned["timedOut"].as_bool() == Some(false)
        && owned["cancelled"].as_bool() == Some(false)
        && cleanup_ok
        && valid_observation(value, cases)
        && value["status"].as_str() == Some("failed")
        && value["phase"].as_str() == Some(phase)
        && value["failurePoint"].as_str() == Some(point)
        && value["injectionReached"].as_bool() == Some(true)
        && required
            .iter()
            .all(|key| value["facts"][*key].as_bool() == Some(true))
        && still_false
            .iter()
            .all(|key| value["facts"][*key].as_bool() == Some(false))
        && !valid_barriers(value)
}

/// `xtask qualify upload-uncertainty [--fail-at none|ack-held|store-held]`.
pub fn run(root: &Path, output: &Path, fail_at: &str) -> i32 {
    let script = root.join("scripts/qualify-upload-uncertainty.mjs");
    let spec = OwnedRun {
        script: &script,
        report_name: "barriers.json",
        report_key: "barriers",
        evidence_env: "DSH_NATIVE_UPLOAD_UNCERTAINTY_EVIDENCE",
        scope: Some(
            "actual public alpha HTTP/Agent/storage with test-only post-real-operation barriers; no prompt or GUI",
        ),
        command: {
            let script = script.to_string_lossy().into_owned();
            let fail_at = fail_at.to_string();
            Box::new(move |_| vec!["/usr/bin/node".to_string(), script.clone(), fail_at.clone()])
        },
        capture_stdout: false,
        timeout: Duration::from_secs(35),
    };
    let owned = match run_owned(root, output, &spec, valid_barriers) {
        Ok(owned) => owned,
        Err(message) => {
            eprintln!("upload uncertainty failed: {message}");
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
    let mut report = json!({
        "status": "failed",
        "ownedHostStatus": owned["status"],
        "failurePoint": fail_at,
        "storage": [],
        "scope": "independent complete private default-provider bytes and joined handler/descriptor/Agent cleanup; injected failures must be refused, not counted as successful uploads",
    });
    let cases: &[&str] = if fail_at == "none" {
        if owned["status"].as_str() != Some("passed") {
            report["error"] = json!("owned-host-barriers-failed");
            let _ = super::private_json(&resolved.join("uncertainty-qualification.json"), &report);
            return 1;
        }
        &CASES
    } else {
        if owned["status"].as_str() != Some("failed") || !valid_expected_failure(&owned, fail_at) {
            report["error"] = json!("expected-owned-failure-or-cleanup-not-established");
            let _ = super::private_json(&resolved.join("uncertainty-qualification.json"), &report);
            return 1;
        }
        report["expectedFailureRefusedAndCleaned"] = json!(true);
        if fail_at == "ack-held" {
            &CASES[..1]
        } else {
            &CASES
        }
    };
    let fixtures: Vec<(&str, Vec<u8>, String)> = cases
        .iter()
        .map(|case| (*case, source(case), format!("PUBLIC {case}.bin")))
        .collect();
    let borrowed: Vec<(&str, Vec<u8>, &str)> = fixtures
        .iter()
        .map(|(case, data, name)| (*case, data.clone(), name.as_str()))
        .collect();
    match verify_local_cases(&resolved, &borrowed) {
        Ok(storage) => {
            report["storage"] = json!(storage);
            report["status"] = json!("passed");
        }
        Err(error) => report["error"] = json!(error),
    }
    if let Err(error) =
        super::private_json(&resolved.join("uncertainty-qualification.json"), &report)
    {
        eprintln!("cannot write the uncertainty report: {error}");
        return 1;
    }
    println!(
        "{}",
        json!({"status": report["status"], "report": resolved.join("uncertainty-qualification.json").to_string_lossy()})
    );
    if report["status"].as_str() == Some("passed") {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(cases: &[&str]) -> Value {
        let files: Vec<Value> = cases
            .iter()
            .map(|case| {
                let data = source(case);
                json!({
                    "case": case,
                    "file": {
                        "attachmentId": format!("sha256:{}", sha256_hex(&data)),
                        "name": format!("PUBLIC {case}.bin"),
                        "bytes": data.len(),
                    }
                })
            })
            .collect();
        json!({
            "fatal": false, "testOnlyInstanceBarriers": true,
            "scriptedModelPromptOrCatalog": false, "scriptedSignInOrCredentialSave": false,
            "providerSubstituted": false, "processWideNetworkTrace": false,
            "rustSdkExercised": false, "uiExercised": false,
            "facts": FACTS.iter().map(|key| ((*key).to_string(), json!(true))).collect::<serde_json::Map<_, _>>(),
            "counts": COUNTS.iter().map(|key| ((*key).to_string(), json!(0))).collect::<serde_json::Map<_, _>>(),
            "bridge": {"active": 0, "complete": cases.len(), "errors": 0, "methodActive": 0},
            "cleanup": {"handlersJoined": true, "descriptorsRestored": true,
                        "observerDisposersRan": true, "ownedHandlesDisposed": true,
                        "registryReturnedToBaseline": true, "hostShutdown": true,
                        "drainTimedOut": false},
            "files": files,
            "status": "passed", "phase": "complete", "failurePoint": "none",
            "injectionReached": false,
        })
    }

    fn owned(point: &str) -> Value {
        let (cases, phase, required) = expected_failure_shape(point);
        let mut barriers = observation(cases);
        barriers["status"] = json!("failed");
        barriers["phase"] = json!(phase);
        barriers["failurePoint"] = json!(point);
        barriers["injectionReached"] = json!(true);
        for key in FACTS {
            barriers["facts"][key] = json!(false);
        }
        for key in required {
            barriers["facts"][*key] = json!(true);
        }
        json!({
            "status": "failed", "exitCode": 1, "sourceUnchanged": true,
            "identityPinFailures": 0, "timedOut": false, "cancelled": false,
            "cleanup": {"rootPidfdPinned": true, "rootReaped": true,
                        "observedCleanupComplete": true, "rootWasRunningAtCleanup": false,
                        "ownedChildFallbackUsed": false, "killEscalated": false,
                        "cleanupTimedOut": false, "cleanupProbeOrSignalFailed": false,
                        "remainingBeforeExternalCleanup": [], "remainingAfterExternalCleanup": []},
            "barriers": barriers,
        })
    }

    #[test]
    fn a_clean_barrier_run_needs_every_fact_true_and_an_exact_observation() {
        let value = observation(&CASES);
        assert!(valid_barriers(&value));
        let mut open = value.clone();
        open["facts"]["foreignBindingRefused"] = json!(false);
        assert!(!valid_barriers(&open));
        let mut dirty = value.clone();
        dirty["counts"]["turnStarts"] = json!(1);
        assert!(!valid_barriers(&dirty));
        let mut leaked = value;
        leaked["cleanup"]["drainTimedOut"] = json!(true);
        assert!(!valid_barriers(&leaked));
    }

    #[test]
    fn no_provider_swap_model_run_or_unintended_surface_is_accepted() {
        for key in [
            "scriptedModelPromptOrCatalog",
            "scriptedSignInOrCredentialSave",
            "providerSubstituted",
            "processWideNetworkTrace",
            "rustSdkExercised",
            "uiExercised",
        ] {
            let mut value = observation(&CASES);
            value[key] = json!(true);
            assert!(!valid_barriers(&value), "{key} true was accepted");
        }
        let mut boolean_count = observation(&CASES);
        boolean_count["counts"]["turnStarts"] = json!(false);
        assert!(
            !valid_barriers(&boolean_count),
            "a boolean cannot impersonate zero"
        );
        let mut closed = observation(&CASES);
        closed["cleanup"]["handlersJoined"] = json!(false);
        assert!(!valid_barriers(&closed), "only drainTimedOut may be false");
    }

    #[test]
    fn only_the_independent_fixture_order_and_digest_are_accepted() {
        let mut reordered = observation(&CASES);
        let files = reordered["files"].as_array_mut().unwrap();
        files.swap(0, 1);
        assert!(
            !valid_barriers(&reordered),
            "the case order is part of the proof"
        );
        let mut wrong_digest = observation(&CASES);
        wrong_digest["files"][0]["file"]["attachmentId"] = json!("sha256:00");
        assert!(!valid_barriers(&wrong_digest));
        let mut extra_key = observation(&CASES);
        extra_key["files"][0]["extra"] = json!(1);
        assert!(
            !valid_barriers(&extra_key),
            "an extra file field is not the fixture shape"
        );
    }

    #[test]
    fn an_injected_barrier_must_fail_at_the_right_point_and_stay_contained() {
        for point in ["ack-held", "store-held"] {
            let owned = owned(point);
            assert!(valid_expected_failure(&owned, point), "{point}");
            // A missing containment fact disqualifies the expected failure.
            let mut uncontained = owned.clone();
            uncontained["cleanup"]["observedCleanupComplete"] = json!(false);
            assert!(!valid_expected_failure(&uncontained, point));
            // So does a wrong failure point.
            assert!(!valid_expected_failure(
                &owned,
                if point == "ack-held" {
                    "store-held"
                } else {
                    "ack-held"
                }
            ));
            // A clean pass is not an expected failure.
            assert!(!valid_expected_failure(&owned, "none"));
        }
    }

    #[test]
    fn an_injected_failure_is_not_a_clean_barrier_run() {
        let owned = owned("ack-held");
        assert!(!valid_barriers(&owned["barriers"]));
    }
}
