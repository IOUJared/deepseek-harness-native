//! Scripted actual native App/worker/Core reversible registry fixture.
//!
//! Port of the validation core of `scripts/qualify-composed-registry.py`:
//! receipts alone cannot pass — every planned state needs its independent
//! follow-up evidence, a strictly increasing ticket serial, and two distinct
//! public sessions that stay retained.
use serde_json::Value;

pub const APP_ID: &str = "ai.deepseek.harness.native.registry-composed-fixture";
pub const MAX_REPORT_BYTES: u64 = 65536;

/// `(name, operation, target slot, pin order, needs pin increment, needs archive increment)`
/// One planned registry transition and the evidence it must carry.
pub struct Stage {
    pub name: &'static str,
    pub operation: &'static str,
    pub target_slot: i64,
    pub pin_order: &'static [i64],
    pub needs_pin: bool,
    pub needs_archive: bool,
}

pub const STAGES: [Stage; 6] = [
    Stage {
        name: "pin-first",
        operation: "Pin",
        target_slot: 1,
        pin_order: &[1],
        needs_pin: true,
        needs_archive: false,
    },
    Stage {
        name: "pin-second",
        operation: "Pin",
        target_slot: 2,
        pin_order: &[2, 1],
        needs_pin: true,
        needs_archive: false,
    },
    Stage {
        name: "unpin-first",
        operation: "Unpin",
        target_slot: 1,
        pin_order: &[2],
        needs_pin: true,
        needs_archive: false,
    },
    Stage {
        name: "repin-first",
        operation: "Pin",
        target_slot: 1,
        pin_order: &[1, 2],
        needs_pin: true,
        needs_archive: false,
    },
    Stage {
        name: "archive-first",
        operation: "Archive",
        target_slot: 1,
        pin_order: &[2],
        needs_pin: true,
        needs_archive: true,
    },
    Stage {
        name: "restore-first",
        operation: "Restore",
        target_slot: 1,
        pin_order: &[2],
        needs_pin: false,
        needs_archive: true,
    },
];

const REQUIRED: [&str; 13] = [
    "publicFixtureOnly",
    "scriptedUiMessages",
    "realWorkerReady",
    "freshEmptyBaselineObserved",
    "ownedWorkspaceCreated",
    "twoDistinctPublicSessionsCreated",
    "ownedWorkspaceFeedMembershipObserved",
    "actualArchivedViewEntered",
    "restoredRecentViewVerified",
    "restoreDidNotRepin",
    "archiveAndUnpinCoupledFeedVerified",
    "allMatchingAcksAndIndependentFeedsVerified",
    "filesRetainedNotDeletionRollback",
];
const FORBIDDEN: [&str; 11] = [
    "realCredentialsUsed",
    "modelCatalogRequested",
    "browserSignInRequested",
    "apiKeySaveRequested",
    "stopActivityRequested",
    "sessionDeletionRequested",
    "forkOrRenameRequested",
    "manualBusinessMessagesForwarded",
    "rpcSetsInstalled",
    "nativeKeyboardPointerInputQualified",
    "liveHostDecisionQualified",
];

fn stop_ok(stop: &Value) -> bool {
    stop["exited"].as_bool() == Some(true)
        && stop["exitCode"].as_i64() == Some(0)
        && stop["graceful"].as_bool() == Some(true)
        && stop["containmentUnknown"].as_bool() == Some(false)
        && stop["observedDescendantsRemaining"].as_i64() == Some(0)
}

/// Every planned stage, its ticket ordering and the retained sessions.
pub fn valid_app(app: &Value, child_pid: u64, scale: f64) -> bool {
    if app["status"].as_str() != Some("passed")
        || app["pid"].as_u64() != Some(child_pid)
        || app["applicationScale"].as_f64() != Some(scale)
        || app["windowBackend"].as_str() != Some("wayland")
        || app["modelPrompts"].as_i64() != Some(0)
        || REQUIRED.iter().any(|key| app[*key].as_bool() != Some(true))
        || FORBIDDEN
            .iter()
            .any(|key| app[*key].as_bool() != Some(false))
    {
        return false;
    }
    let Some(ids) = app["publicSessionIds"].as_array() else {
        return false;
    };
    if ids.len() != 2
        || ids.iter().any(|value| {
            value
                .as_str()
                .is_none_or(|value| value.is_empty() || value.len() > 4096)
        })
        || ids[0] == ids[1]
    {
        return false;
    }
    let Some(stages) = app["stages"].as_array() else {
        return false;
    };
    if stages.len() != STAGES.len() {
        return false;
    }
    let mut previous_serial = 0i64;
    for (observed, stage) in stages.iter().zip(STAGES.iter()) {
        if observed["name"].as_str() != Some(stage.name)
            || observed["operation"].as_str() != Some(stage.operation)
            || observed["targetSlot"].as_i64() != Some(stage.target_slot)
            || observed["pinOrderSlots"] != serde_json::json!(stage.pin_order)
        {
            return false;
        }
        for key in [
            "matchingSubmissionAcknowledged",
            "requiredFeedIncrementsObserved",
            "authoritativeRegistryVerified",
            "actualNavigationVerified",
            "bothSessionsAndAccountingRetained",
        ] {
            if observed[key].as_bool() != Some(true) {
                return false;
            }
        }
        if stage.needs_pin && observed["pinIncrementObserved"].as_bool() != Some(true) {
            return false;
        }
        if stage.needs_archive && observed["archiveIncrementObserved"].as_bool() != Some(true) {
            return false;
        }
        let ticket = &observed["ticket"];
        let Some(generation) = ticket["generation"].as_i64() else {
            return false;
        };
        let Some(serial) = ticket["serial"].as_i64() else {
            return false;
        };
        if ticket["epoch"].as_i64() != Some(1) || generation <= 0 || serial <= previous_serial {
            return false;
        }
        previous_serial = serial;
    }
    stop_ok(&app["stop"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn app() -> Value {
        let stages: Vec<Value> = STAGES
            .iter()
            .enumerate()
            .map(|(index, stage)| {
                json!({
                    "name": stage.name, "operation": stage.operation, "targetSlot": stage.target_slot,
                    "pinOrderSlots": stage.pin_order,
                    "matchingSubmissionAcknowledged": true,
                    "requiredFeedIncrementsObserved": true,
                    "authoritativeRegistryVerified": true,
                    "actualNavigationVerified": true,
                    "bothSessionsAndAccountingRetained": true,
                    "pinIncrementObserved": true,
                    "archiveIncrementObserved": true,
                    "ticket": {"epoch": 1, "generation": index + 1, "serial": index + 1},
                })
            })
            .collect();
        let mut value = json!({
            "status": "passed", "pid": 6100, "applicationScale": 1.0,
            "windowBackend": "wayland", "modelPrompts": 0,
            "publicSessionIds": ["PUBLIC-session-a", "PUBLIC-session-b"],
            "stages": stages,
            "stop": {"exited": true, "exitCode": 0, "graceful": true,
                     "containmentUnknown": false, "observedDescendantsRemaining": 0},
        });
        for key in REQUIRED {
            value[key] = json!(true);
        }
        for key in FORBIDDEN {
            value[key] = json!(false);
        }
        value
    }

    #[test]
    fn a_complete_registry_run_passes_and_any_stage_gap_fails() {
        assert!(valid_app(&app(), 6100, 1.0));
        // A receipt without its follow-up evidence is not a pass.
        let mut unverified = app();
        unverified["stages"][2]["requiredFeedIncrementsObserved"] = json!(false);
        assert!(!valid_app(&unverified, 6100, 1.0));
        // The pin order and target slot are part of the proof.
        let mut order = app();
        order["stages"][1]["pinOrderSlots"] = json!([1, 2]);
        assert!(!valid_app(&order, 6100, 1.0));
        let mut slot = app();
        slot["stages"][3]["targetSlot"] = json!(2);
        assert!(!valid_app(&slot, 6100, 1.0));
        // A restore must not claim a pin increment requirement.
        let mut repinned = app();
        repinned["stages"][5]["pinIncrementObserved"] = json!(false);
        assert!(valid_app(&repinned, 6100, 1.0), "restore needs no pin");
        // Two sessions must be distinct and non-empty.
        let mut same = app();
        same["publicSessionIds"] = json!(["PUBLIC-session-a", "PUBLIC-session-a"]);
        assert!(!valid_app(&same, 6100, 1.0));
        let mut empty = app();
        empty["publicSessionIds"] = json!(["PUBLIC-session-a", ""]);
        assert!(!valid_app(&empty, 6100, 1.0));
        // The ticket serial must increase and the epoch stay 1.
        let mut serial = app();
        serial["stages"][2]["ticket"]["serial"] = json!(2);
        assert!(!valid_app(&serial, 6100, 1.0));
        let mut epoch = app();
        epoch["stages"][0]["ticket"]["epoch"] = json!(2);
        assert!(!valid_app(&epoch, 6100, 1.0));
        // Overclaims and a substituted pid are refused.
        let mut deletion = app();
        deletion["sessionDeletionRequested"] = json!(true);
        assert!(!valid_app(&deletion, 6100, 1.0));
        assert!(!valid_app(&app(), 6101, 1.0));
        assert!(
            !valid_app(&json!({}), 6100, 1.0),
            "a truncated report is not valid"
        );
    }
}
