use crate::{
    CodexCacheRetention, CodexIdleMillis, CodexImageBudget, CodexRuntimeNatural,
    CodexRuntimeSnapshot, CodexRuntimeTransport, CodexRuntimeValues, Command, Error, PublicReply,
    protocol,
};
use serde_json::{Value, json};

const COMMANDS: [&str; 2] = ["codex-runtime-read", "codex-runtime-save"];
const MAX: u64 = 9_007_199_254_740_991;
const FIELDS: [&str; 8] = [
    "transport",
    "cacheRetention",
    "timeoutMs",
    "websocketConnectTimeoutMs",
    "streamIdleTimeoutMs",
    "requestImagePixelBudget",
    "requestImageMaxBytes",
    "maxRequestImageBytes",
];
const PROJECTIONS: [&str; 3] = ["values", "baseValues", "overrides"];

fn sample() -> Value {
    json!({
        "version": 1, "revision": 0, "configured": true, "writable": true,
        "values": CodexRuntimeValues::default().encode(),
        "baseValues": CodexRuntimeValues::default().encode(),
        "overrides": CodexRuntimeValues::default().encode(),
    })
}
fn pinned() -> CodexRuntimeValues {
    CodexRuntimeValues {
        transport: Some(CodexRuntimeTransport::WebsocketCached),
        cache_retention: Some(CodexCacheRetention::Short),
        timeout_ms: Some(CodexRuntimeNatural::new(0).unwrap()),
        websocket_connect_timeout_ms: Some(CodexRuntimeNatural::new(MAX).unwrap()),
        stream_idle_timeout_ms: Some(CodexIdleMillis::new(32_000.375).unwrap()),
        request_image_pixel_budget: Some(CodexImageBudget::new(1).unwrap()),
        request_image_max_bytes: Some(CodexImageBudget::new(524_288).unwrap()),
        max_request_image_bytes: Some(CodexImageBudget::new(MAX).unwrap()),
    }
}
fn parse_frame(value: Value) -> Result<PublicReply, Error> {
    let frame = format!("DSH_TAURI:{value}");
    match protocol::parse(frame.as_bytes(), "0.2.1-alpha.1")? {
        protocol::Event::Reply { reply, .. } => reply,
        _ => panic!("typed runtime reply expected"),
    }
}
fn reply(command: &str, value: Value) -> Result<PublicReply, Error> {
    parse_frame(json!({
        "type": "native-result", "requestId": "runtime_1", "command": command, "value": value,
    }))
}
fn snapshot(command: &str, value: Value) -> CodexRuntimeSnapshot {
    match reply(command, value).unwrap() {
        PublicReply::CodexRuntime(snapshot) => snapshot,
        _ => panic!("runtime snapshot expected"),
    }
}
fn reject(value: Value) {
    for command in COMMANDS {
        assert_eq!(reply(command, value.clone()), Err(Error::Protocol));
    }
}

#[test]
fn wrappers_are_copy_eq_bounded_and_fraction_preserving() {
    fn copy_eq<T: Copy + Eq>() {}
    copy_eq::<CodexRuntimeNatural>();
    copy_eq::<CodexImageBudget>();
    copy_eq::<CodexIdleMillis>();
    copy_eq::<CodexRuntimeValues>();
    copy_eq::<CodexRuntimeSnapshot>();
    for number in [0, 1, MAX] {
        assert_eq!(CodexRuntimeNatural::new(number).unwrap().get(), number);
    }
    for number in [1, MAX] {
        assert_eq!(CodexImageBudget::new(number).unwrap().get(), number);
    }
    for number in [MAX + 1, u64::MAX] {
        assert_eq!(CodexRuntimeNatural::new(number), Err(Error::InvalidCommand));
        assert_eq!(CodexImageBudget::new(number), Err(Error::InvalidCommand));
    }
    assert_eq!(CodexImageBudget::new(0), Err(Error::InvalidCommand));
    for number in [f64::MIN_POSITIVE, 0.125, 1.0, 32_000.375, 2_147_483_647.0] {
        let idle = CodexIdleMillis::new(number).unwrap();
        assert_eq!(idle.as_f64().to_bits(), number.to_bits());
        assert_eq!(idle, CodexIdleMillis::new(number).unwrap());
    }
    for number in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -1.0,
        -f64::MIN_POSITIVE,
        -0.0,
        0.0,
        2_147_483_647.5,
        f64::MAX,
    ] {
        assert_eq!(CodexIdleMillis::new(number), Err(Error::InvalidCommand));
    }
    assert_eq!(
        CodexRuntimeValues::default()
            .encode()
            .as_object()
            .unwrap()
            .len(),
        8
    );
    for field in FIELDS {
        assert_eq!(CodexRuntimeValues::default().encode()[field], Value::Null);
    }
}

#[test]
fn closed_enums_round_trip_through_real_command_and_reply_paths() {
    for (transport, name) in [
        (CodexRuntimeTransport::Sse, "sse"),
        (CodexRuntimeTransport::Websocket, "websocket"),
        (CodexRuntimeTransport::WebsocketCached, "websocket-cached"),
        (CodexRuntimeTransport::Auto, "auto"),
    ] {
        assert_eq!(transport.as_str(), name);
        assert_eq!(CodexRuntimeTransport::parse(name), Some(transport));
        for (cache, cache_name) in [
            (CodexCacheRetention::None, "none"),
            (CodexCacheRetention::Short, "short"),
            (CodexCacheRetention::Long, "long"),
        ] {
            assert_eq!(cache.as_str(), cache_name);
            assert_eq!(CodexCacheRetention::parse(cache_name), Some(cache));
            let values = CodexRuntimeValues {
                transport: Some(transport),
                cache_retention: Some(cache),
                ..pinned()
            };
            let encoded = Command::CodexRuntimeSave {
                expected_revision: 4,
                values,
            }
            .encode("runtime_1")
            .unwrap();
            assert_eq!(encoded["values"]["transport"], name);
            assert_eq!(encoded["values"]["cacheRetention"], cache_name);
            let mut value = sample();
            value["values"] = encoded["values"].clone();
            value["overrides"] = encoded["values"].clone();
            for command in COMMANDS {
                let parsed = snapshot(command, value.clone());
                assert_eq!(parsed.values, values);
                assert_eq!(parsed.overrides, values);
            }
        }
    }
    for value in [
        "",
        "SSE",
        "websocketCached",
        "websocket_cached",
        "auto ",
        "none",
    ] {
        assert_eq!(CodexRuntimeTransport::parse(value), None);
    }
    for value in ["", "NONE", "Short", "long ", "sse"] {
        assert_eq!(CodexCacheRetention::parse(value), None);
    }
}

#[test]
fn commands_encode_exact_eight_keys_and_null_inheritance_without_credential_or_path_fields() {
    assert_eq!(Command::CodexRuntimeRead.name(), COMMANDS[0]);
    assert_eq!(
        Command::CodexRuntimeRead.encode("42").unwrap(),
        json!({"type":"codex-runtime-read", "requestId":"42"})
    );
    let command = Command::CodexRuntimeSave {
        expected_revision: 7,
        values: pinned(),
    };
    assert_eq!(command.name(), COMMANDS[1]);
    assert_eq!(
        command.encode("42").unwrap(),
        json!({
            "type":"codex-runtime-save", "requestId":"42", "expectedRevision":7,
            "values": {
                "transport":"websocket-cached", "cacheRetention":"short", "timeoutMs":0,
                "websocketConnectTimeoutMs":MAX, "streamIdleTimeoutMs":32_000.375,
                "requestImagePixelBudget":1, "requestImageMaxBytes":524_288,
                "maxRequestImageBytes":MAX,
            },
        })
    );
    let encoded = Command::CodexRuntimeSave {
        expected_revision: 0,
        values: CodexRuntimeValues::default(),
    }
    .encode("42")
    .unwrap();
    assert_eq!(encoded.as_object().unwrap().len(), 4);
    assert_eq!(encoded["values"].as_object().unwrap().len(), 8);
    for field in FIELDS {
        assert_eq!(encoded["values"][field], Value::Null);
    }
}

#[test]
fn exhausted_and_unsafe_save_revisions_fail_locally_without_encoding_a_mutation() {
    for revision in [0, 1, MAX - 1] {
        let encoded = Command::CodexRuntimeSave {
            expected_revision: revision,
            values: CodexRuntimeValues::default(),
        }
        .encode("42")
        .unwrap();
        assert_eq!(encoded["expectedRevision"].as_u64(), Some(revision));
    }
    for revision in [MAX, MAX + 1, u64::MAX] {
        assert_eq!(
            Command::CodexRuntimeSave {
                expected_revision: revision,
                values: CodexRuntimeValues::default(),
            }
            .encode("42"),
            Err(Error::InvalidCommand)
        );
    }
}

#[test]
fn parser_requires_exact_snapshot_and_three_exact_eight_field_projections() {
    for value in [
        Value::Null,
        json!([]),
        json!({}),
        json!(true),
        json!("snapshot"),
    ] {
        reject(value);
    }
    for field in [
        "version",
        "revision",
        "configured",
        "writable",
        "values",
        "baseValues",
        "overrides",
    ] {
        let mut value = sample();
        value.as_object_mut().unwrap().remove(field);
        reject(value);
        let mut value = sample();
        value[field] = Value::Null;
        reject(value);
    }
    for projection in PROJECTIONS {
        for invalid in [
            Value::Null,
            json!([]),
            json!({}),
            json!(false),
            json!("values"),
        ] {
            let mut value = sample();
            value[projection] = invalid;
            reject(value);
        }
        for field in FIELDS {
            let mut value = sample();
            value[projection].as_object_mut().unwrap().remove(field);
            reject(value);
        }
    }
    for field in [
        "extra",
        "credentials",
        "token",
        "apiKey",
        "path",
        "namespace",
        "ns",
        "provider",
        "accountId",
    ] {
        let mut value = sample();
        value[field] = json!("SECRET_MUST_NOT_ESCAPE");
        reject(value);
        for projection in PROJECTIONS {
            let mut value = sample();
            value[projection][field] = json!("SECRET_MUST_NOT_ESCAPE");
            reject(value);
        }
    }
}

#[test]
fn parser_rejects_mistyped_values_unknown_enums_and_invalid_numeric_domains() {
    for projection in PROJECTIONS {
        for field in FIELDS {
            for invalid in [json!(true), json!([]), json!({})] {
                let mut value = sample();
                value[projection][field] = invalid;
                reject(value);
            }
        }
        for (field, invalids) in [
            (
                "transport",
                vec![
                    json!("SSE"),
                    json!("websocketCached"),
                    json!("auto "),
                    json!(0),
                ],
            ),
            (
                "cacheRetention",
                vec![json!("NONE"), json!("none "), json!("auto"), json!(0)],
            ),
        ] {
            for invalid in invalids {
                let mut value = sample();
                value[projection][field] = invalid;
                reject(value);
            }
        }
        for field in &FIELDS[2..] {
            for invalid in [json!("1"), json!(-1), json!(MAX + 1), json!(u64::MAX)] {
                let mut value = sample();
                value[projection][*field] = invalid;
                reject(value);
            }
            if *field != "streamIdleTimeoutMs" {
                let mut value = sample();
                value[projection][*field] = json!(1.25);
                reject(value);
            }
        }
        for field in &FIELDS[4..] {
            for invalid in [json!(0), json!(0.0), json!(-0.0)] {
                let mut value = sample();
                value[projection][*field] = invalid;
                reject(value);
            }
        }
        let mut value = sample();
        value[projection]["streamIdleTimeoutMs"] = json!(2_147_483_647.25);
        reject(value);
    }
}

#[test]
fn parser_accepts_canonical_natural_zero_safe_bounds_fractional_idle_and_null_inheritance() {
    for command in COMMANDS {
        assert_eq!(
            snapshot(command, sample()).values,
            CodexRuntimeValues::default()
        );
        for projection in PROJECTIONS {
            for number in [json!(0), json!(1), json!(1.0), json!(-0.0), json!(MAX)] {
                let mut value = sample();
                value[projection]["timeoutMs"] = number.clone();
                value[projection]["websocketConnectTimeoutMs"] = number.clone();
                let parsed = snapshot(command, value);
                let values = match projection {
                    "values" => parsed.values,
                    "baseValues" => parsed.base_values,
                    _ => parsed.overrides,
                };
                assert_eq!(
                    values.timeout_ms.unwrap().get() as f64,
                    number.as_f64().unwrap()
                );
                assert_eq!(values.websocket_connect_timeout_ms, values.timeout_ms);
                assert_eq!(
                    values.encode()["timeoutMs"].as_u64(),
                    Some(values.timeout_ms.unwrap().get())
                );
            }
            for number in [1, MAX] {
                let mut value = sample();
                for field in &FIELDS[5..] {
                    value[projection][*field] = json!(number);
                }
                let parsed = snapshot(command, value);
                let values = match projection {
                    "values" => parsed.values,
                    "baseValues" => parsed.base_values,
                    _ => parsed.overrides,
                };
                assert_eq!(values.request_image_pixel_budget.unwrap().get(), number);
                assert_eq!(values.request_image_max_bytes.unwrap().get(), number);
                assert_eq!(values.max_request_image_bytes.unwrap().get(), number);
            }
            for number in [f64::MIN_POSITIVE, 0.125, 32_000.375, 2_147_483_647.0] {
                let mut value = sample();
                value[projection]["streamIdleTimeoutMs"] = json!(number);
                let parsed = snapshot(command, value);
                let values = match projection {
                    "values" => parsed.values,
                    "baseValues" => parsed.base_values,
                    _ => parsed.overrides,
                };
                assert_eq!(values.stream_idle_timeout_ms.unwrap().as_f64(), number);
            }
        }
    }
}

#[test]
fn snapshot_version_revision_boolean_types_and_writable_invariant_are_closed() {
    for command in COMMANDS {
        for revision in [json!(0), json!(1.0), json!(-0.0), json!(MAX)] {
            let mut value = sample();
            value["revision"] = revision.clone();
            value["writable"] = json!(false);
            assert_eq!(
                snapshot(command, value).revision as f64,
                revision.as_f64().unwrap()
            );
        }
        let mut value = sample();
        value["version"] = json!(1.0);
        assert_eq!(snapshot(command, value).version, 1);
        let mut value = sample();
        value["configured"] = json!(false);
        value["writable"] = json!(false);
        assert!(!snapshot(command, value).configured);
    }
    for (field, invalid) in [
        ("version", json!(0)),
        ("version", json!(2)),
        ("version", json!(1.5)),
        ("version", json!("1")),
        ("version", json!(true)),
        ("version", json!([])),
        ("revision", json!(-1)),
        ("revision", json!(0.5)),
        ("revision", json!(MAX + 1)),
        ("revision", json!(u64::MAX)),
        ("revision", json!("0")),
        ("revision", json!(false)),
        ("revision", json!([])),
        ("revision", json!({})),
        ("configured", json!(1)),
        ("configured", json!("true")),
        ("configured", json!([])),
        ("writable", json!(0)),
        ("writable", json!("false")),
        ("writable", json!({})),
    ] {
        let mut value = sample();
        value[field] = invalid;
        reject(value);
    }
    let mut value = sample();
    value["configured"] = json!(false);
    reject(value);
    let mut value = sample();
    value["revision"] = json!(MAX);
    reject(value);
}

#[test]
fn explicit_pins_equal_to_base_remain_distinct_from_null_inheritance() {
    let mut value = sample();
    value["values"] = pinned().encode();
    value["baseValues"] = pinned().encode();
    value["overrides"] = pinned().encode();
    for command in COMMANDS {
        let pinned_snapshot = snapshot(command, value.clone());
        assert_eq!(pinned_snapshot.values, pinned_snapshot.base_values);
        assert_eq!(pinned_snapshot.overrides, pinned());
        let mut inherited = value.clone();
        inherited["overrides"] = CodexRuntimeValues::default().encode();
        let inherited_snapshot = snapshot(command, inherited);
        assert_eq!(inherited_snapshot.values, pinned_snapshot.values);
        assert_eq!(inherited_snapshot.base_values, pinned_snapshot.base_values);
        assert_ne!(inherited_snapshot.overrides, pinned_snapshot.overrides);
    }
}

#[test]
fn actual_frame_parser_rejects_nonfinite_literals_and_unsafe_integer_literals_before_rounding() {
    for command in COMMANDS {
        let mut value = sample();
        value["values"] = pinned().encode();
        let valid =
            json!({"type":"native-result", "requestId":"42", "command":command, "value":value})
                .to_string();
        for (field, existing) in [
            ("revision", "0"),
            ("timeoutMs", "0"),
            ("streamIdleTimeoutMs", "32000.375"),
        ] {
            for invalid in ["NaN", "Infinity", "-Infinity", "1e309", "-1e309"] {
                let raw = valid.replace(
                    &format!("\"{field}\":{existing}"),
                    &format!("\"{field}\":{invalid}"),
                );
                assert_ne!(raw, valid, "test must mutate production frame");
                let frame = format!("DSH_TAURI:{raw}");
                assert!(matches!(
                    protocol::parse(frame.as_bytes(), "0.2.1-alpha.1"),
                    Err(Error::Protocol)
                ));
            }
        }
        for field in [
            "revision",
            "timeoutMs",
            "websocketConnectTimeoutMs",
            "requestImagePixelBudget",
            "requestImageMaxBytes",
            "maxRequestImageBytes",
        ] {
            let mut value = sample();
            value["values"] = pinned().encode();
            let target = if field == "revision" {
                &mut value["revision"]
            } else {
                &mut value["values"][field]
            };
            *target = json!(MAX + 1);
            reject(value);
        }
    }
}

#[test]
fn reply_matching_and_request_correlation_include_only_runtime_commands() {
    let runtime = PublicReply::CodexRuntime(snapshot(COMMANDS[0], sample()));
    for command in COMMANDS {
        assert!(protocol::matches_reply(command, &runtime));
        assert!(!protocol::matches_reply(
            command,
            &PublicReply::ApiKeySaved(true)
        ));
        let frame = format!(
            "DSH_TAURI:{}",
            json!({
                "type":"native-result", "requestId":"runtime_1", "command":command, "value":sample(),
            })
        );
        let protocol::Event::Reply {
            id,
            command: found,
            reply: Ok(reply),
        } = protocol::parse(frame.as_bytes(), "0.2.1-alpha.1").unwrap()
        else {
            panic!("correlated runtime reply expected")
        };
        assert_eq!(id, "runtime_1");
        assert_eq!(found.as_deref(), Some(command));
        assert_eq!(reply, runtime);
    }
    for command in [
        "codex-status",
        "codex-accounts-status",
        "codex-usage-load",
        "codex-usage-save",
        "inspect",
        "save-api-key",
        "codex-runtime",
    ] {
        assert!(!protocol::matches_reply(command, &runtime));
    }
    assert_eq!(reply("codex-runtime", sample()), Err(Error::Protocol));
}

#[test]
fn runtime_error_codes_are_closed_and_external_diagnostics_never_escape() {
    for (code, expected) in [
        (json!("CODEX_RUNTIME_CONFLICT"), Error::CodexRuntimeConflict),
        (
            json!("CODEX_RUNTIME_INDETERMINATE"),
            Error::CodexRuntimeIndeterminate,
        ),
        (
            json!("CODEX_RUNTIME_UNAVAILABLE"),
            Error::CodexRuntimeUnavailable,
        ),
        (json!("CONFLICT"), Error::RequestFailed),
        (json!("codex_runtime_conflict"), Error::RequestFailed),
        (json!("CODEX_RUNTIME_CONFLICT "), Error::RequestFailed),
        (json!(["CODEX_RUNTIME_CONFLICT"]), Error::RequestFailed),
        (
            json!({"code":"CODEX_RUNTIME_CONFLICT"}),
            Error::RequestFailed,
        ),
        (Value::Null, Error::RequestFailed),
    ] {
        for command in COMMANDS {
            let result = parse_frame(json!({
                "type":"request-error", "requestId":"42", "command":command, "code":code,
                "message":"SECRET_EXTERNAL_DIAGNOSTIC_MUST_NOT_ESCAPE", "path":"SECRET_PATH",
            }));
            assert_eq!(result, Err(expected));
            assert!(!format!("{result:?}").contains("SECRET"));
        }
    }
}
