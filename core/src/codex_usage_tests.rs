use crate::{
    CodexComposerDisplay, CodexRemainingPercent, CodexUsageAlert, CodexUsagePreferences,
    CodexUsageSnapshot, Command, Error, PublicReply, protocol,
};
use serde_json::{Value, json};

const COMMANDS: [&str; 2] = ["codex-usage-load", "codex-usage-save"];
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

fn sample() -> Value {
    json!({
        "version": 1,
        "revision": 0,
        "preferences": {
            "composerDisplay": "off",
            "usageAlert": "off",
            "customRemainingPercent": 20,
            "retryAfterReset": false,
            "experimentalQuota": false,
        },
        "persisted": false,
    })
}
fn parse_frame(value: Value) -> Result<PublicReply, Error> {
    let frame = format!("DSH_TAURI:{value}");
    match protocol::parse(frame.as_bytes(), "0.2.1-alpha.1")? {
        protocol::Event::Reply { reply, .. } => reply,
        _ => panic!("typed reply expected"),
    }
}
fn reply(command: &str, value: Value) -> Result<PublicReply, Error> {
    parse_frame(json!({
        "type": "native-result", "requestId": "usage_1", "command": command, "value": value,
    }))
}
fn snapshot(command: &str, value: Value) -> CodexUsageSnapshot {
    match reply(command, value).unwrap() {
        PublicReply::CodexUsage(snapshot) => snapshot,
        _ => panic!("usage snapshot expected"),
    }
}
fn reject(value: Value) {
    for command in COMMANDS {
        assert_eq!(reply(command, value.clone()), Err(Error::Protocol));
    }
}

#[test]
fn remaining_percent_is_eq_safe_finite_bounded_fractional_and_normalizes_negative_zero() {
    fn copy_eq<T: Copy + Eq>() {}
    copy_eq::<CodexRemainingPercent>();
    copy_eq::<CodexUsagePreferences>();
    copy_eq::<CodexUsageSnapshot>();
    fn eq<T: Eq>() {}
    eq::<Command>();
    eq::<PublicReply>();

    for value in [0.0, f64::MIN_POSITIVE, 0.125, 12.75, 20.0, 99.999, 100.0] {
        let percent = CodexRemainingPercent::new(value).unwrap();
        assert_eq!(percent.as_f64(), value);
        assert_eq!(percent, CodexRemainingPercent::new(value).unwrap());
    }
    let zero = CodexRemainingPercent::new(0.0).unwrap();
    let negative_zero = CodexRemainingPercent::new(-0.0).unwrap();
    assert_eq!(zero, negative_zero);
    assert_eq!(negative_zero.as_f64().to_bits(), 0.0_f64.to_bits());
    assert_eq!(CodexRemainingPercent::default().as_f64(), 20.0);
    for value in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -f64::MIN_POSITIVE,
        -0.001,
        100.0001,
        f64::MAX,
    ] {
        assert_eq!(
            CodexRemainingPercent::new(value),
            Err(Error::InvalidCommand)
        );
    }
}

#[test]
fn closed_enum_mappings_round_trip_through_real_command_and_reply_paths() {
    let displays = [
        (CodexComposerDisplay::Off, "off"),
        (CodexComposerDisplay::Percent, "percent"),
        (CodexComposerDisplay::ProgressBar, "progressBar"),
        (CodexComposerDisplay::Runway, "runway"),
    ];
    let alerts = [
        (CodexUsageAlert::Off, "off"),
        (CodexUsageAlert::Percent20, "percent20"),
        (CodexUsageAlert::EarlyWarning, "earlyWarning"),
        (CodexUsageAlert::Custom, "custom"),
    ];
    for (display, display_name) in displays {
        assert_eq!(display.as_str(), display_name);
        assert_eq!(CodexComposerDisplay::parse(display_name), Some(display));
        for (alert, alert_name) in alerts {
            assert_eq!(alert.as_str(), alert_name);
            assert_eq!(CodexUsageAlert::parse(alert_name), Some(alert));
            let preferences = CodexUsagePreferences {
                composer_display: display,
                usage_alert: alert,
                custom_remaining_percent: CodexRemainingPercent::new(12.75).unwrap(),
                retry_after_reset: true,
                experimental_quota: true,
            };
            let encoded = Command::CodexUsageSave {
                expected_revision: 2,
                preferences,
            }
            .encode("usage_1")
            .unwrap();
            assert_eq!(encoded["preferences"]["composerDisplay"], display_name);
            assert_eq!(encoded["preferences"]["usageAlert"], alert_name);
            let mut value = sample();
            value["preferences"] = encoded["preferences"].clone();
            value["revision"] = json!(3);
            value["persisted"] = json!(true);
            for command in COMMANDS {
                let found = snapshot(command, value.clone());
                assert_eq!(found.preferences, preferences);
                assert_eq!(found.revision, 3);
                assert!(found.persisted);
            }
        }
    }
    for invalid in ["", "OFF", "progress-bar", "percent20", "percent "] {
        assert_eq!(CodexComposerDisplay::parse(invalid), None);
    }
    for invalid in ["", "OFF", "early-warning", "percent", "custom "] {
        assert_eq!(CodexUsageAlert::parse(invalid), None);
    }
}

#[test]
fn root_commands_serialize_exactly_without_extra_fields() {
    assert_eq!(Command::CodexUsageLoad.name(), "codex-usage-load");
    assert_eq!(
        Command::CodexUsageLoad.encode("42").unwrap(),
        json!({"type":"codex-usage-load","requestId":"42"})
    );
    let preferences = CodexUsagePreferences {
        composer_display: CodexComposerDisplay::ProgressBar,
        usage_alert: CodexUsageAlert::Custom,
        custom_remaining_percent: CodexRemainingPercent::new(17.125).unwrap(),
        retry_after_reset: true,
        experimental_quota: false,
    };
    let command = Command::CodexUsageSave {
        expected_revision: 7,
        preferences,
    };
    assert_eq!(command.name(), "codex-usage-save");
    assert_eq!(
        command.encode("42").unwrap(),
        json!({
            "type":"codex-usage-save","requestId":"42","expectedRevision":7,
            "preferences":{
                "composerDisplay":"progressBar","usageAlert":"custom",
                "customRemainingPercent":17.125,"retryAfterReset":true,"experimentalQuota":false,
            },
        })
    );
}

#[test]
fn malformed_outbound_revisions_fail_as_invalid_command_not_protocol() {
    for revision in [0, 1, MAX_SAFE_INTEGER] {
        let encoded = Command::CodexUsageSave {
            expected_revision: revision,
            preferences: CodexUsagePreferences::default(),
        }
        .encode("42")
        .unwrap();
        assert_eq!(encoded["expectedRevision"].as_u64(), Some(revision));
    }
    for revision in [MAX_SAFE_INTEGER + 1, u64::MAX] {
        assert_eq!(
            Command::CodexUsageSave {
                expected_revision: revision,
                preferences: CodexUsagePreferences::default(),
            }
            .encode("42"),
            Err(Error::InvalidCommand)
        );
    }
}

#[test]
fn snapshot_and_preferences_require_exact_object_fields_not_null_arrays_or_extras() {
    for value in [
        Value::Null,
        json!([]),
        json!({}),
        json!(true),
        json!("snapshot"),
    ] {
        reject(value);
    }
    for field in ["version", "revision", "preferences", "persisted"] {
        let mut missing = sample();
        missing.as_object_mut().unwrap().remove(field);
        reject(missing);
        let mut null = sample();
        null[field] = Value::Null;
        reject(null);
    }
    let mut extra = sample();
    extra["extra"] = json!(false);
    reject(extra);
    for preferences in [json!([]), json!({}), json!(false), json!("preferences")] {
        let mut value = sample();
        value["preferences"] = preferences;
        reject(value);
    }
    for field in [
        "composerDisplay",
        "usageAlert",
        "customRemainingPercent",
        "retryAfterReset",
        "experimentalQuota",
    ] {
        let mut missing = sample();
        missing["preferences"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        reject(missing);
        let mut null = sample();
        null["preferences"][field] = Value::Null;
        reject(null);
    }
    let mut extra = sample();
    extra["preferences"]["extra"] = json!(false);
    reject(extra);
}

#[test]
fn parser_rejects_malformed_enums_and_mistyped_preference_fields_without_coercion() {
    for (field, invalid) in [
        ("composerDisplay", json!("OFF")),
        ("composerDisplay", json!("progress-bar")),
        ("composerDisplay", json!(false)),
        ("composerDisplay", json!(0)),
        ("composerDisplay", json!(["off"])),
        ("composerDisplay", json!({})),
        ("usageAlert", json!("early-warning")),
        ("usageAlert", json!("percent")),
        ("usageAlert", json!(false)),
        ("usageAlert", json!(20)),
        ("usageAlert", json!(["off"])),
        ("usageAlert", json!({})),
        ("customRemainingPercent", json!("12.5")),
        ("customRemainingPercent", json!(false)),
        ("customRemainingPercent", json!([20])),
        ("customRemainingPercent", json!({})),
        ("customRemainingPercent", json!(-0.001)),
        ("customRemainingPercent", json!(100.0001)),
        ("retryAfterReset", json!(0)),
        ("retryAfterReset", json!("false")),
        ("retryAfterReset", json!([])),
        ("retryAfterReset", json!({})),
        ("experimentalQuota", json!(1)),
        ("experimentalQuota", json!("true")),
        ("experimentalQuota", json!([])),
        ("experimentalQuota", json!({})),
    ] {
        let mut value = sample();
        value["preferences"][field] = invalid;
        reject(value);
    }
}

#[test]
fn parser_preserves_fractional_percent_and_inclusive_bounds_with_zero_normalization() {
    for command in COMMANDS {
        for percent in [0.0, -0.0, 0.125, 17.375, 99.999, 100.0] {
            let mut value = sample();
            value["preferences"]["customRemainingPercent"] = json!(percent);
            let parsed = snapshot(command, value)
                .preferences
                .custom_remaining_percent;
            assert_eq!(parsed.as_f64(), percent);
            if percent == 0.0 {
                assert_eq!(parsed.as_f64().to_bits(), 0.0_f64.to_bits());
            }
        }
    }
}

#[test]
fn parser_requires_version_one_safe_nonnegative_integral_revision_and_boolean_persisted() {
    for command in COMMANDS {
        for revision in [
            json!(0),
            json!(1),
            json!(1.0),
            json!(-0.0),
            json!(MAX_SAFE_INTEGER),
        ] {
            let mut value = sample();
            value["revision"] = revision.clone();
            assert_eq!(
                snapshot(command, value).revision as f64,
                revision.as_f64().unwrap()
            );
        }
    }
    for (field, invalid) in [
        ("version", json!(0)),
        ("version", json!(2)),
        ("version", json!(1.5)),
        ("version", json!("1")),
        ("version", json!(true)),
        ("version", json!([])),
        ("version", json!({})),
        ("revision", json!(-1)),
        ("revision", json!(0.5)),
        ("revision", json!(MAX_SAFE_INTEGER + 1)),
        ("revision", json!(u64::MAX)),
        ("revision", json!("0")),
        ("revision", json!(false)),
        ("revision", json!([])),
        ("revision", json!({})),
        ("persisted", json!(0)),
        ("persisted", json!("false")),
        ("persisted", json!([])),
        ("persisted", json!({})),
    ] {
        let mut value = sample();
        value[field] = invalid;
        reject(value);
    }
}

#[test]
fn nonfinite_wire_numbers_fail_in_the_actual_frame_parser() {
    for command in COMMANDS {
        for field in ["customRemainingPercent", "revision", "version"] {
            let valid = json!({
                "type":"native-result","requestId":"42","command":command,"value":sample(),
            })
            .to_string();
            let existing = match field {
                "customRemainingPercent" => "20",
                "revision" => "0",
                _ => "1",
            };
            for invalid in ["NaN", "Infinity", "-Infinity", "1e309", "-1e309"] {
                let frame = format!(
                    "DSH_TAURI:{}",
                    valid.replace(
                        &format!("\"{field}\":{existing}"),
                        &format!("\"{field}\":{invalid}")
                    )
                );
                assert!(matches!(
                    protocol::parse(frame.as_bytes(), "0.2.1-alpha.1"),
                    Err(Error::Protocol)
                ));
            }
        }
    }
}

#[test]
fn usage_reply_matching_is_closed_and_correlated_to_only_usage_commands() {
    let usage = PublicReply::CodexUsage(CodexUsageSnapshot::default());
    for command in COMMANDS {
        assert!(protocol::matches_reply(command, &usage));
        assert!(!protocol::matches_reply(
            command,
            &PublicReply::ApiKeySaved(true)
        ));
        let frame = json!({
            "type":"native-result","requestId":"usage_1","command":command,"value":sample(),
        });
        let bytes = format!("DSH_TAURI:{frame}");
        let protocol::Event::Reply {
            id,
            command: found,
            reply: Ok(reply),
        } = protocol::parse(bytes.as_bytes(), "0.2.1-alpha.1").unwrap()
        else {
            panic!("correlated usage reply expected");
        };
        assert_eq!(id, "usage_1");
        assert_eq!(found.as_deref(), Some(command));
        assert_eq!(reply, usage);
    }
    for command in [
        "codex-status",
        "codex-accounts-status",
        "inspect",
        "save-api-key",
        "codex-usage",
    ] {
        assert!(!protocol::matches_reply(command, &usage));
    }
    assert_eq!(reply("codex-usage", sample()), Err(Error::Protocol));
}

#[test]
fn receipt_codes_map_exactly_and_do_not_expose_external_messages() {
    for (code, expected) in [
        (json!("CODEX_USAGE_CONFLICT"), Error::CodexUsageConflict),
        (
            json!("CODEX_USAGE_INDETERMINATE"),
            Error::CodexUsageIndeterminate,
        ),
        (
            json!("CODEX_USAGE_UNAVAILABLE"),
            Error::CodexUsageUnavailable,
        ),
        (json!("CONFLICT"), Error::RequestFailed),
        (json!("INDETERMINATE"), Error::RequestFailed),
        (json!("READ_FAILED"), Error::RequestFailed),
        (json!("codex_usage_conflict"), Error::RequestFailed),
        (json!("CODEX_USAGE_CONFLICT "), Error::RequestFailed),
        (json!("UNKNOWN"), Error::RequestFailed),
        (Value::Null, Error::RequestFailed),
        (json!(["CODEX_USAGE_CONFLICT"]), Error::RequestFailed),
        (json!({"code":"CODEX_USAGE_CONFLICT"}), Error::RequestFailed),
    ] {
        for command in COMMANDS {
            let result = parse_frame(json!({
                "type":"request-error","requestId":"42","command":command,"code":code,
                "message":"EXTERNAL_DIAGNOSTIC_MUST_NOT_ESCAPE",
            }));
            assert_eq!(result, Err(expected));
            assert!(!format!("{result:?}").contains("EXTERNAL_DIAGNOSTIC"));
        }
    }
    assert_eq!(
        parse_frame(json!({"type":"request-error","requestId":"42"})),
        Err(Error::RequestFailed)
    );
}

#[test]
fn default_missing_snapshot_matches_the_actual_canonical_engine() {
    // Execute the inert production module, with no filesystem port, profile, provider or network.
    let script = format!(
        "{}\n{}",
        include_str!("../runtime/codex-usage-preferences.mjs"),
        r#"const store = createCodexUsagePreferencesStore({
            readDocument: () => null,
            commitDocument: () => { throw new Error('unexpected commit'); },
            maxPendingUpdates: 1,
            maxDocumentBytes: 4096,
        });
        process.stdout.write(JSON.stringify(await store.load()));
        await store.dispose();"#,
    );
    let output = std::process::Command::new("/usr/bin/node")
        .env_clear()
        .arg("--input-type=module")
        .arg("-e")
        .arg(script)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "canonical preferences engine failed"
    );
    let canonical: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(canonical, sample());
    for command in COMMANDS {
        assert_eq!(
            snapshot(command, canonical.clone()),
            CodexUsageSnapshot::default()
        );
    }
    let encoded = Command::CodexUsageSave {
        expected_revision: 0,
        preferences: CodexUsagePreferences::default(),
    }
    .encode("42")
    .unwrap();
    // Numbers are compared as numbers, not their integer-vs-f64 JSON rendering.
    assert_eq!(
        encoded["preferences"]["customRemainingPercent"].as_f64(),
        Some(20.0)
    );
    for field in [
        "composerDisplay",
        "usageAlert",
        "retryAfterReset",
        "experimentalQuota",
    ] {
        assert_eq!(
            encoded["preferences"][field],
            canonical["preferences"][field]
        );
    }
    assert_eq!(encoded["preferences"].as_object().unwrap().len(), 5);
}
