//! Owned scripted native UI/worker/isolated Host export.
//!
//! Port of the report and capture half of `scripts/qualify-composed-export.py`.
//! The blank-session archive is checked independently (bounded inflate, CRC and
//! UTF-8/object JSONL) before this report can pass.
use super::registry::{MAX_PNG_BYTES, png_dimensions, private_regular};
use serde_json::Value;
use std::path::Path;

pub const APP_ID: &str = "ai.deepseek.harness.native.export-composed-fixture";
pub const ARCHIVE: &str = "PUBLIC-session-export.zip";
pub const MAX_ZIP: u64 = 4 * 1024 * 1024;
pub const MAX_ROOT: u64 = 1024 * 1024;
const CAPTURES: [(&str, &str); 2] = [
    ("exportReviewCapture", "export-review.png"),
    ("exportSavedCapture", "export-saved.png"),
];

const REQUIRED: [&str; 17] = [
    "publicFixtureOnly",
    "scriptedUiMessages",
    "realWorkerReady",
    "freshEmptyBaselineObserved",
    "ownedWorkspaceCreated",
    "oneOrdinaryPublicSessionCreated",
    "selectedBaselineObserved",
    "matchingSavedReceiptObserved",
    "rootOnlyExportConfirmed",
    "noOverwriteReceiptObserved",
    "privateZipVerified",
    "exactBytesUnchanged",
    "samePathBytesAndMetadataUnchanged",
    "twoExactReceiptsObserved",
    "gracefulStopped",
    "actualCloseUiDispatched",
    "actualCloseInspectionObserved",
];
const ABSENT: [&str; 10] = [
    "modelCatalogRequested",
    "browserSignInRequested",
    "apiKeySaveRequested",
    "pluginWritesRequested",
    "settingsWritesRequested",
    "manualBusinessMessagesForwarded",
    "sessionDeletionRequested",
    "stopActivityRequested",
    "includeDescendants",
    "realCredentialsUsed",
];

fn positive(value: &Value) -> bool {
    value.as_i64().is_some_and(|number| number >= 1)
}

/// Two exact save receipts: a new archive, then the same path refusing overwrite.
pub fn valid_app(app: &Value, pid: u64, scale: f64) -> bool {
    if app["status"].as_str() != Some("passed")
        || app["pid"].as_u64() != Some(pid)
        || app["applicationScale"].as_f64() != Some(scale)
        || app["windowBackend"].as_str() != Some("wayland")
        || app["targetCount"].as_i64() != Some(1)
        || app["zipArtifact"].as_str() != Some(ARCHIVE)
        || REQUIRED.iter().any(|key| app[*key].as_bool() != Some(true))
        || app["modelPrompts"].as_i64() != Some(0)
        || ABSENT.iter().any(|key| app[*key].as_bool() != Some(false))
    {
        return false;
    }
    let Some(stages) = app["stages"].as_array() else {
        return false;
    };
    if stages.len() != 2 {
        return false;
    }
    for stage in stages {
        if [
            "matchingExportedReceipt",
            "controllerPendingRetired",
            "enteredPathCleared",
        ]
        .iter()
        .any(|key| stage[*key].as_bool() != Some(true))
        {
            return false;
        }
        let receipt = &stage["receipt"];
        if !["epoch", "generation", "serial", "editor", "attempt"]
            .iter()
            .all(|key| positive(&receipt[*key]))
        {
            return false;
        }
    }
    let (first, second) = (&stages[0], &stages[1]);
    let (a, b) = (&first["receipt"], &second["receipt"]);
    first["name"].as_str() == Some("save-new-public-archive")
        && first["saved"].as_bool() == Some(true)
        && first["notCreated"].as_bool() == Some(false)
        && first["savedBytes"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0 && bytes <= MAX_ZIP)
        && second["name"].as_str() == Some("same-path-no-overwrite")
        && second["saved"].as_bool() == Some(false)
        && second["notCreated"].as_bool() == Some(true)
        && second["savedBytes"].is_null()
        && a["epoch"] == b["epoch"]
        && a["generation"] == b["generation"]
        && b["serial"].as_i64() > a["serial"].as_i64()
        && b["editor"].as_i64() > a["editor"].as_i64()
        && b["attempt"].as_i64() > a["attempt"].as_i64()
}

/// Both captures must be private PNGs reporting the file they saved.
pub fn valid_captures(output: &Path, app: &Value) -> bool {
    for (key, filename) in CAPTURES {
        let capture = &app[key];
        let Some(data) = private_regular(&output.join(filename), MAX_PNG_BYTES) else {
            return false;
        };
        let Some((width, height)) = png_dimensions(&data) else {
            return false;
        };
        if capture["saved"].as_bool() != Some(true)
            || capture["artifact"].as_str() != Some(filename)
            || capture["physicalSize"] != serde_json::json!([width, height])
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn app() -> Value {
        json!({
            "status": "passed", "pid": 7100, "applicationScale": 1.0,
            "windowBackend": "wayland", "targetCount": 1, "zipArtifact": ARCHIVE,
            "modelPrompts": 0,
            "stages": [
                {"name": "save-new-public-archive", "saved": true, "notCreated": false,
                 "savedBytes": 1024, "matchingExportedReceipt": true,
                 "controllerPendingRetired": true, "enteredPathCleared": true,
                 "receipt": {"epoch": 3, "generation": 5, "serial": 7, "editor": 2, "attempt": 1}},
                {"name": "same-path-no-overwrite", "saved": false, "notCreated": true,
                 "savedBytes": null, "matchingExportedReceipt": true,
                 "controllerPendingRetired": true, "enteredPathCleared": true,
                 "receipt": {"epoch": 3, "generation": 5, "serial": 9, "editor": 3, "attempt": 2}},
            ],
        })
    }

    fn with_flags(mut value: Value) -> Value {
        for key in REQUIRED {
            value[key] = json!(true);
        }
        for key in ABSENT {
            value[key] = json!(false);
        }
        value
    }

    #[test]
    fn two_exact_save_receipts_are_required_and_no_overwrite_must_not_save() {
        let good = with_flags(app());
        assert!(valid_app(&good, 7100, 1.0));
        // An overwrite is not the refused second stage.
        let mut overwritten = good.clone();
        overwritten["stages"][1]["saved"] = json!(true);
        assert!(!valid_app(&overwritten, 7100, 1.0));
        let mut created = good.clone();
        created["stages"][1]["notCreated"] = json!(false);
        assert!(!valid_app(&created, 7100, 1.0));
        // The two receipts must advance serial, editor and attempt on one epoch.
        for (key, value) in [("serial", 7), ("editor", 2), ("attempt", 1)] {
            let mut replay = good.clone();
            replay["stages"][1]["receipt"][key] = json!(value);
            assert!(!valid_app(&replay, 7100, 1.0), "{key} did not advance");
        }
        let mut epoch = good.clone();
        epoch["stages"][1]["receipt"]["epoch"] = json!(4);
        assert!(!valid_app(&epoch, 7100, 1.0));
        // A saved byte count outside the bound is refused.
        let mut huge = good.clone();
        huge["stages"][0]["savedBytes"] = json!(MAX_ZIP + 1);
        assert!(!valid_app(&huge, 7100, 1.0));
        // A retired pending controller is part of the proof.
        let mut pending = good.clone();
        pending["stages"][0]["controllerPendingRetired"] = json!(false);
        assert!(!valid_app(&pending, 7100, 1.0));
        // Overclaims and a substituted pid are refused.
        let mut descendants = good.clone();
        descendants["includeDescendants"] = json!(true);
        assert!(!valid_app(&descendants, 7100, 1.0));
        assert!(!valid_app(&good, 7101, 1.0));
        assert!(!valid_app(&json!({}), 7100, 1.0));
    }
}
