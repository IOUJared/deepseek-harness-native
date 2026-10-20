//! Agent-discovery, client-projection and turn-outline observations.
//!
//! Ports of `scripts/qualify-agent-discovery.py`,
//! `qualify-client-projections.py` and `qualify-turn-outline.py`, all of which
//! are configurations of the shared ownership runner.
use super::composed::{CommandFactory, OwnedRun, run_owned};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

fn exact_keys(value: &Value, expected: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.keys().map(String::as_str).collect::<BTreeSet<_>>()
            == expected.iter().copied().collect::<BTreeSet<_>>()
    })
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

const AGENT_FACTS: [&str; 7] = [
    "absentBefore",
    "actualRegisteredObject",
    "matchedActualIdentity",
    "unrelatedAbsent",
    "unchangedByRepeatedRead",
    "absentAfterDisposal",
    "registryReturnedToBaseline",
];
const AGENT_COUNTS: [&str; 9] = [
    "turnStart",
    "turnEnd",
    "stepStarts",
    "stepEnds",
    "requestHeaders",
    "toolCalls",
    "toolResults",
    "assistantMessages",
    "unexpectedWake",
];

pub fn valid_agent(report: &Value) -> bool {
    report["status"].as_str() == Some("passed")
        && report["phase"].as_str() == Some("complete")
        && report["fatal"].as_bool() == Some(false)
        && report["providerSubstituted"].as_bool() == Some(false)
        && report["discoveryPromptOrResumeRequested"].as_bool() == Some(false)
        && report["accountOrKeyMutationRequested"].as_bool() == Some(false)
        && report["processWideNetworkTrace"].as_bool() == Some(false)
        && exact_keys(&report["facts"], &AGENT_FACTS)
        && all_true(&report["facts"])
        && exact_keys(&report["counts"], &AGENT_COUNTS)
        && all_zero(&report["counts"])
}

pub fn valid_sdk(report: &Value) -> bool {
    let facts = &report["agentDiscovery"];
    let stop = &report["stop"];
    let forbidden = [
        "request/header",
        "tool/call",
        "tool/result",
        "assistant/message",
        "turn/start",
        "step/start",
        "step/end",
    ];
    let quiet = report["snapshot"]["eventTypes"]
        .as_array()
        .is_some_and(|events| {
            events.iter().all(|event| {
                event
                    .as_str()
                    .is_some_and(|event| !forbidden.contains(&event))
            })
        });
    report["status"].as_str() == Some("passed")
        && report["ready"]["version"].as_str() == Some("0.2.1-alpha.1")
        && report["modelPrompts"].as_i64() == Some(0)
        && report["modelCatalogRequested"].as_bool() == Some(false)
        && [
            "unregisteredReturnedNone",
            "registeredReturnedTypedId",
            "sessionEchoMatched",
            "repeatedReadStable",
            "alphaSharedBytesObserved",
        ]
        .iter()
        .all(|key| facts[*key].as_bool() == Some(true))
        && facts["agentIdInferred"].as_bool() == Some(false)
        && report["coreStopRejectsDiscovery"].as_bool() == Some(true)
        && report["coreStopInvalidatesTransport"].as_bool() == Some(true)
        && stop["exited"].as_bool() == Some(true)
        && stop["graceful"].as_bool() == Some(true)
        && stop["exitCode"].as_i64() == Some(0)
        && stop["containmentUnknown"].as_bool() == Some(false)
        && stop["observedDescendantsRemaining"].as_i64() == Some(0)
        && quiet
}

pub fn valid_projections(report: &Value) -> bool {
    let facts = [
        "registryPresent",
        "allNamedKeysPublished",
        "blankSessionShapes",
    ];
    let named = ["turnOutline", "tokenUsage", "inbox", "contextPressure"];
    let published = report["published"].as_array();
    let keys_are_strings = |values: Option<&Vec<Value>>| {
        values.is_some_and(|values| {
            values
                .iter()
                .all(|value| value.as_str().is_some_and(|value| !value.is_empty()))
        })
    };
    let published_names: BTreeSet<&str> = published
        .map(|values| values.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    report["status"].as_str() == Some("passed")
        && report["phase"].as_str() == Some("complete")
        && report["fatal"].as_bool() == Some(false)
        && report["providerSubstituted"].as_bool() == Some(false)
        && report["modelPromptsRequested"].as_bool() == Some(false)
        && report["catalogRequested"].as_bool() == Some(false)
        && exact_keys(&report["facts"], &facts)
        && all_true(&report["facts"])
        && report["missing"].as_array().is_some_and(Vec::is_empty)
        && ["turnOutline", "tokenUsage", "inbox"]
            .iter()
            .all(|key| published_names.contains(key))
        && report["named"]
            .as_object()
            .is_some_and(|object| named.iter().all(|key| object.contains_key(*key)))
        && keys_are_strings(published)
}

pub fn valid_outline(report: &Value) -> bool {
    let facts = [
        "registryPresent",
        "keyPublished",
        "blankSessionIsEmpty",
        "keysAreClientVisible",
    ];
    let keys = report["keys"].as_array();
    report["status"].as_str() == Some("passed")
        && report["phase"].as_str() == Some("complete")
        && report["fatal"].as_bool() == Some(false)
        && report["providerSubstituted"].as_bool() == Some(false)
        && report["modelPromptsRequested"].as_bool() == Some(false)
        && report["catalogRequested"].as_bool() == Some(false)
        && exact_keys(&report["facts"], &facts)
        && all_true(&report["facts"])
        && keys.is_some_and(|keys| {
            keys.iter().any(|key| key.as_str() == Some("turnOutline"))
                && keys
                    .iter()
                    .all(|key| key.as_str().is_some_and(|key| !key.is_empty()))
        })
}

fn node_command(script: std::path::PathBuf) -> CommandFactory<'static> {
    let script = script.to_string_lossy().into_owned();
    Box::new(move |_| vec!["/usr/bin/node".to_string(), script.clone()])
}

/// `xtask qualify agent-discovery [--sdk]`.
pub fn agent_discovery(root: &Path, output: &Path, sdk: bool) -> i32 {
    if sdk {
        let binary = root.join("core/target/release/examples/transport_smoke");
        let runtime = root
            .parent()
            .map(|parent| parent.join("deepseek-harness-linux/apps/cli"))
            .unwrap_or_default();
        let spec = OwnedRun {
            script: &binary,
            report_name: "sdk.json",
            report_key: "sdk",
            evidence_env: "DSH_NATIVE_AGENT_EVIDENCE",
            scope: Some(
                "owned actual public alpha profile, typed Rust Core discovery and HTTP/WS; not GUI input or process-wide network trace",
            ),
            command: {
                let binary = binary.to_string_lossy().into_owned();
                let runtime = runtime.to_string_lossy().into_owned();
                Box::new(move |output: &Path| {
                    vec![
                        binary.clone(),
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
        return finish(run_owned(root, output, &spec, valid_sdk));
    }
    let script = root.join("scripts/qualify-agent-discovery.mjs");
    let spec = OwnedRun {
        script: &script,
        report_name: "agent.json",
        report_key: "agent",
        evidence_env: "DSH_NATIVE_AGENT_EVIDENCE",
        scope: Some(
            "owned actual public alpha Loader, real AgentRegistry/factory and private supervisor discovery; not Rust or GUI, no process-wide network trace",
        ),
        command: node_command(root.join("scripts/qualify-agent-discovery.mjs")),
        capture_stdout: false,
        timeout: Duration::from_secs(35),
    };
    finish(run_owned(root, output, &spec, valid_agent))
}

/// `xtask qualify client-projections`.
pub fn client_projections(root: &Path, output: &Path) -> i32 {
    let script = root.join("scripts/qualify-client-projections.mjs");
    let spec = OwnedRun {
        script: &script,
        report_name: "projections.json",
        report_key: "projections",
        evidence_env: "DSH_NATIVE_PROJECTIONS_EVIDENCE",
        scope: Some(
            "Real alpha base+web profile: every projection key native chat reads, on one blank session; not native transport, GUI or a model run",
        ),
        command: node_command(root.join("scripts/qualify-client-projections.mjs")),
        capture_stdout: false,
        timeout: Duration::from_secs(35),
    };
    finish(run_owned(root, output, &spec, valid_projections))
}

/// `xtask qualify turn-outline`.
pub fn turn_outline(root: &Path, output: &Path) -> i32 {
    let script = root.join("scripts/qualify-turn-outline.mjs");
    let spec = OwnedRun {
        script: &script,
        report_name: "turn-outline.json",
        report_key: "turnOutline",
        evidence_env: "DSH_NATIVE_TURNOUTLINE_EVIDENCE",
        scope: Some(
            "Real alpha base+web profile: turnOutline registration and one blank session snapshot; not native transport, GUI or a model run",
        ),
        command: node_command(root.join("scripts/qualify-turn-outline.mjs")),
        capture_stdout: false,
        timeout: Duration::from_secs(35),
    };
    finish(run_owned(root, output, &spec, valid_outline))
}

fn finish(outcome: Result<Value, String>) -> i32 {
    match outcome {
        Ok(result) => {
            if result["status"].as_str() == Some("passed") {
                0
            } else {
                1
            }
        }
        Err(message) => {
            eprintln!("qualification failed: {message}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn agent() -> Value {
        json!({
            "status": "passed", "phase": "complete", "fatal": false,
            "providerSubstituted": false, "discoveryPromptOrResumeRequested": false,
            "accountOrKeyMutationRequested": false, "processWideNetworkTrace": false,
            "facts": {"absentBefore": true, "actualRegisteredObject": true,
                      "matchedActualIdentity": true, "unrelatedAbsent": true,
                      "unchangedByRepeatedRead": true, "absentAfterDisposal": true,
                      "registryReturnedToBaseline": true},
            "counts": {"turnStart": 0, "turnEnd": 0, "stepStarts": 0, "stepEnds": 0,
                       "requestHeaders": 0, "toolCalls": 0, "toolResults": 0,
                       "assistantMessages": 0, "unexpectedWake": 0},
        })
    }

    #[test]
    fn an_agent_report_needs_every_fact_true_and_every_count_zero() {
        assert!(valid_agent(&agent()));
        let mut extra = agent();
        extra["counts"]["newEvent"] = json!(0);
        assert!(!valid_agent(&extra), "an unknown count is not a pass");
        let mut woke = agent();
        woke["counts"]["unexpectedWake"] = json!(1);
        assert!(!valid_agent(&woke));
        let mut missing = agent();
        missing["facts"]
            .as_object_mut()
            .unwrap()
            .remove("absentBefore");
        assert!(!valid_agent(&missing));
    }

    fn sdk() -> Value {
        json!({
            "status": "passed", "ready": {"version": "0.2.1-alpha.1"},
            "modelPrompts": 0, "modelCatalogRequested": false,
            "agentDiscovery": {"unregisteredReturnedNone": true, "registeredReturnedTypedId": true,
                               "sessionEchoMatched": true, "repeatedReadStable": true,
                               "alphaSharedBytesObserved": true, "agentIdInferred": false},
            "coreStopRejectsDiscovery": true, "coreStopInvalidatesTransport": true,
            "stop": {"exited": true, "graceful": true, "exitCode": 0,
                     "containmentUnknown": false, "observedDescendantsRemaining": 0},
            "snapshot": {"eventTypes": []},
        })
    }

    #[test]
    fn an_agent_fact_that_is_not_true_or_a_counter_that_is_not_zero_fails() {
        for key in AGENT_FACTS {
            let mut value = agent();
            value["facts"][key] = json!(false);
            assert!(!valid_agent(&value), "{key} false was accepted");
        }
        for key in AGENT_COUNTS {
            for bad in [json!(true), json!(1), json!(-1)] {
                let mut value = agent();
                value["counts"][key] = bad.clone();
                assert!(!valid_agent(&value), "{key} = {bad} was accepted");
            }
        }
        let mut extra = agent();
        extra["counts"]["extra"] = json!(0);
        assert!(!valid_agent(&extra));
        assert!(!valid_agent(&json!({})), "a truncated report is not valid");
    }

    #[test]
    fn an_sdk_run_needs_the_typed_echo_owned_shutdown_and_a_quiet_profile() {
        assert!(valid_sdk(&sdk()));
        for key in [
            "unregisteredReturnedNone",
            "registeredReturnedTypedId",
            "sessionEchoMatched",
            "repeatedReadStable",
            "alphaSharedBytesObserved",
        ] {
            let mut value = sdk();
            value["agentDiscovery"][key] = json!(false);
            assert!(!valid_sdk(&value), "{key} false was accepted");
        }
        let mut stopped = sdk();
        stopped["stop"]["exitCode"] = json!(9);
        assert!(!valid_sdk(&stopped));
        let mut inferred = sdk();
        inferred["agentDiscovery"]["agentIdInferred"] = json!(true);
        assert!(!valid_sdk(&inferred));
        for event in [
            "request/header",
            "tool/call",
            "tool/result",
            "assistant/message",
            "turn/start",
            "step/start",
            "step/end",
        ] {
            let mut value = sdk();
            value["snapshot"]["eventTypes"] = json!([event]);
            assert!(!valid_sdk(&value), "{event} was accepted");
        }
        let mut version = sdk();
        version["ready"]["version"] = json!("0.2.0-rc.2");
        assert!(!valid_sdk(&version));
        let mut boolean_prompts = sdk();
        boolean_prompts["modelPrompts"] = json!(false);
        assert!(
            !valid_sdk(&boolean_prompts),
            "a boolean cannot impersonate zero prompts"
        );
    }

    #[test]
    fn a_projection_report_needs_the_named_keys_and_no_missing_ones() {
        let mut report = json!({
            "status": "passed", "phase": "complete", "fatal": false,
            "providerSubstituted": false, "modelPromptsRequested": false, "catalogRequested": false,
            "facts": {"registryPresent": true, "allNamedKeysPublished": true, "blankSessionShapes": true},
            "missing": [],
            "published": ["turnOutline", "tokenUsage", "inbox", "contextPressure"],
            "named": {"turnOutline": 1, "tokenUsage": 1, "inbox": 1, "contextPressure": 1},
        });
        assert!(valid_projections(&report));
        report["published"] = json!(["turnOutline", "tokenUsage"]);
        assert!(!valid_projections(&report), "inbox must be published too");
        let mut missing = report.clone();
        missing["missing"] = json!(["turnOutline"]);
        assert!(!valid_projections(&missing));
    }

    #[test]
    fn an_outline_report_needs_the_key_and_a_quiet_lifecycle() {
        let mut report = json!({
            "status": "passed", "phase": "complete", "fatal": false,
            "providerSubstituted": false, "modelPromptsRequested": false, "catalogRequested": false,
            "facts": {"registryPresent": true, "keyPublished": true,
                      "blankSessionIsEmpty": true, "keysAreClientVisible": true},
            "keys": ["turnOutline", "tokenUsage"],
        });
        assert!(valid_outline(&report));
        report["keys"] = json!(["tokenUsage"]);
        assert!(!valid_outline(&report));
        report["keys"] = json!(["turnOutline", ""]);
        assert!(
            !valid_outline(&report),
            "an empty key is not client visible"
        );
    }
}
