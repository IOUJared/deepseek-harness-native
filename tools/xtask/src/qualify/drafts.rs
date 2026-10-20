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
