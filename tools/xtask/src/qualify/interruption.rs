//! Draft interruption campaign: the checkpoint contract.
//!
//! Port of the validation half of `scripts/qualify-draft-interruption.py`. Nine
//! checkpoints exercise hold/recover/crash at each write barrier; a graceful
//! phase may not borrow the crash phase's external descendant cleanup, and a
//! crash observation must latch its gate until the owner kills it.
use super::drafts::session_id;
use serde_json::Value;

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
