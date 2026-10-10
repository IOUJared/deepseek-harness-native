//! Private scenario for the existing real native composition fixture, not a new reducer.
use super::*;
use dsh_native_core::{
    CodexCacheRetention as Cache, CodexIdleMillis, CodexImageBudget, CodexRuntimeNatural,
    CodexRuntimeSnapshot as RuntimeSnapshot, CodexRuntimeTransport as Transport,
    CodexRuntimeValues as RuntimeValues,
};

#[derive(Default)]
pub(super) struct Case {
    before: Option<RuntimeSnapshot>,
    acknowledged: Option<RuntimeSnapshot>,
}

pub(super) fn seed(options: &Options) -> Result<(), &'static str> {
    // JSON is a YAML sequence; this is only a new owned PUBLIC route, not authorization.
    let profiles = options.app.backend.native_home.join("profiles");
    fs::create_dir(&profiles).map_err(|_| "new-public-profiles-required")?;
    fs::set_permissions(&profiles, fs::Permissions::from_mode(0o700))
        .map_err(|_| "seed-permission-failed")?;
    let directory = profiles.join("desktop");
    fs::create_dir(&directory).map_err(|_| "new-public-profile-required")?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .map_err(|_| "seed-permission-failed")?;
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.join("cordis.patch.yml"))
        .map_err(|_| "new-public-route-patch-required")?;
    serde_json::to_writer(
        &file,
        &json!([{"id":"llm-pi-ai","name":"@deepseek-ai/dsh-llm-pi-ai",
        "config":{"providers":{"openai-codex":{"displayName":"PUBLIC preserved route label"}}}}]),
    )
    .map_err(|_| "seed-write-failed")?;
    file.sync_all().map_err(|_| "seed-sync-failed")?;
    fs::File::open(directory)
        .and_then(|d| d.sync_all())
        .map_err(|_| "seed-directory-sync-failed")?;
    Ok(())
}
fn pins() -> RuntimeValues {
    RuntimeValues {
        transport: Some(Transport::WebsocketCached),
        cache_retention: Some(Cache::Long),
        timeout_ms: Some(CodexRuntimeNatural::new(0).expect("PUBLIC zero timeout")),
        websocket_connect_timeout_ms: Some(
            CodexRuntimeNatural::new(0).expect("PUBLIC zero connection timeout"),
        ),
        stream_idle_timeout_ms: Some(CodexIdleMillis::new(12.5).expect("PUBLIC fractional idle")),
        request_image_pixel_budget: Some(
            CodexImageBudget::new(524_288).expect("PUBLIC pixel budget"),
        ),
        request_image_max_bytes: Some(CodexImageBudget::new(524_288).expect("PUBLIC image bytes")),
        max_request_image_bytes: Some(
            CodexImageBudget::new(8_388_608).expect("PUBLIC total bytes"),
        ),
    }
}
fn configured(value: RuntimeSnapshot) -> bool {
    value.version == 1
        && value.configured
        && value.writable
        && value.revision < 9_007_199_254_740_990
}
fn initial(value: RuntimeSnapshot) -> bool {
    configured(value)
        && value.overrides == RuntimeValues::default()
        && value.values == value.base_values
}
fn restored(value: RuntimeSnapshot) -> bool {
    configured(value) && value.overrides == pins() && value.values == pins()
}
fn values(value: RuntimeValues) -> Value {
    json!({
    "transport":value.transport.map(Transport::as_str),"cacheRetention":value.cache_retention.map(Cache::as_str),
    "timeoutMs":value.timeout_ms.map(CodexRuntimeNatural::get),"websocketConnectTimeoutMs":value.websocket_connect_timeout_ms.map(CodexRuntimeNatural::get),
    "streamIdleTimeoutMs":value.stream_idle_timeout_ms.map(CodexIdleMillis::as_f64),
    "requestImagePixelBudget":value.request_image_pixel_budget.map(CodexImageBudget::get),
    "requestImageMaxBytes":value.request_image_max_bytes.map(CodexImageBudget::get),"maxRequestImageBytes":value.max_request_image_bytes.map(CodexImageBudget::get)})
}
fn metadata(value: RuntimeSnapshot) -> Value {
    json!({"version":value.version,"revision":value.revision,"configured":value.configured,"writable":value.writable,
    "values":values(value.values),"baseValues":values(value.base_values),"overrides":values(value.overrides)})
}

pub(super) fn completed(
    f: &mut Fixture,
    ticket: codex::Ticket,
    outcome: codex::Outcome,
    task: Task<Message>,
) -> Task<Message> {
    // Fixture::update already applied the actual receipt to the actual App.
    let codex::Outcome::Runtime(value) = outcome else {
        return f.stop(Some("runtime-preferences-unavailable"));
    };
    if f.stage == Stage::Read(ticket) {
        let expected = match f.options.phase {
            Phase::Initial | Phase::Inherited => initial(value),
            Phase::Restored => restored(value),
        };
        if !expected {
            return f.stop(Some("wrong-owned-runtime-read"));
        }
        f.report["runtimeLoaded"] = metadata(value);
        f.report["initialOwnedSnapshotMatched"] = json!(true);
        if f.options.phase == Phase::Inherited {
            f.report["independentInheritedRestartMatched"] = json!(true);
            return Task::batch([task, f.paint()]);
        }
        f.runtime_case.before = Some(value);
        let mut tasks = vec![task];
        if f.options.phase == Phase::Initial {
            // Invalid decimal exponent cannot acquire a worker operation ticket.
            let before = ui::usage_ticket(&f.app);
            tasks.push(f.runtime(codex::RuntimeAction::Number {
                field: codex::RuntimeField::StreamIdleTimeout,
                value: "1e2".into(),
            }));
            tasks.push(f.runtime(codex::RuntimeAction::Save));
            if ui::usage_ticket(&f.app) != before {
                return f.stop(Some("invalid-runtime-save-admitted"));
            }
            f.report["invalidRuntimeSaveRejected"] = json!(true);
            tasks.push(f.runtime(codex::RuntimeAction::Transport(Some(
                Transport::WebsocketCached,
            ))));
            tasks.push(f.runtime(codex::RuntimeAction::Cache(Some(Cache::Long))));
            for (field, value) in [
                (codex::RuntimeField::Timeout, "0"),
                (codex::RuntimeField::WebsocketConnectTimeout, "0"),
                (codex::RuntimeField::StreamIdleTimeout, "12.5"),
                (codex::RuntimeField::ImagePixelBudget, "524288"),
                (codex::RuntimeField::ImageMaxBytes, "524288"),
                (codex::RuntimeField::MaxRequestImageBytes, "8388608"),
            ] {
                tasks.push(f.runtime(codex::RuntimeAction::Number {
                    field,
                    value: value.into(),
                }));
            }
        } else {
            f.report["independentPinnedRestartMatched"] = json!(true);
            tasks.push(f.runtime(codex::RuntimeAction::Reset));
            f.report["resetWasExplicitLocalDraftThenSave"] = json!(true);
        }
        tasks.push(f.runtime(codex::RuntimeAction::Save));
        f.stage = Stage::Save(ui::usage_ticket(&f.app));
        return Task::batch(tasks);
    }
    if f.stage == Stage::Save(ticket) {
        let Some(before) = f.runtime_case.before else {
            return f.stop(Some("runtime-save-without-owned-baseline"));
        };
        let target = if f.options.phase == Phase::Initial {
            pins()
        } else {
            RuntimeValues::default()
        };
        // Reuse production acceptance; this CLOSED raw-base-empty route additionally has exact pins.
        // General SDK unsets can normalize inherited pins, so this is not blanket equality policy.
        if !configured(value)
            || !codex::runtime_ack_matches(value, before.revision, target, before.base_values)
            || value.overrides != target
            || (f.options.phase == Phase::Initial && value.values != pins())
            || (f.options.phase == Phase::Restored && value.values != value.base_values)
        {
            return f.stop(Some("runtime-owned-ack-mismatch"));
        }
        f.runtime_case.acknowledged = Some(value);
        f.report["actualPersistenceAcknowledged"] = json!(true);
        f.report["runtimeAcknowledged"] = metadata(value);
        let refresh = f.runtime(codex::RuntimeAction::Refresh);
        f.stage = Stage::Readback(ui::usage_ticket(&f.app));
        return Task::batch([task, refresh]);
    }
    if f.stage == Stage::Readback(ticket) {
        if f.runtime_case.acknowledged != Some(value) {
            return f.stop(Some("runtime-readback-mismatch"));
        }
        f.report["sameProcessReadbackMatched"] = json!(true);
        f.report["runtimeReadback"] = metadata(value);
        return Task::batch([task, f.paint()]);
    }
    f.stop(Some("unexpected-runtime-ticket-or-phase"))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn inherited_snapshot() -> RuntimeSnapshot {
        RuntimeSnapshot {
            version: 1,
            revision: 0,
            configured: true,
            writable: true,
            values: RuntimeValues::default(),
            base_values: RuntimeValues::default(),
            overrides: RuntimeValues::default(),
        }
    }
    #[test]
    fn runtime_oracle_distinguishes_actual_pins_from_equal_effective_values() {
        let inherited = inherited_snapshot();
        assert!(initial(inherited));
        assert!(!restored(inherited));
        let saved = RuntimeSnapshot {
            values: pins(),
            overrides: pins(),
            ..inherited
        };
        assert!(restored(saved));
        assert!(!initial(saved));
        assert!(!restored(RuntimeSnapshot {
            overrides: RuntimeValues::default(),
            ..saved
        }));
        for bad in [
            RuntimeSnapshot {
                version: 2,
                ..saved
            },
            RuntimeSnapshot {
                configured: false,
                ..saved
            },
            RuntimeSnapshot {
                writable: false,
                ..saved
            },
        ] {
            assert!(!restored(bad));
        }
    }
    #[test]
    fn pins_preserve_zero_timeouts_fraction_and_bounded_image_values() {
        let value = pins();
        assert_eq!(value.timeout_ms.unwrap().get(), 0);
        assert_eq!(value.websocket_connect_timeout_ms.unwrap().get(), 0);
        assert_eq!(value.stream_idle_timeout_ms.unwrap().as_f64(), 12.5);
        assert_eq!(value.request_image_pixel_budget.unwrap().get(), 524288);
        assert_eq!(value.request_image_max_bytes.unwrap().get(), 524288);
        assert_eq!(value.max_request_image_bytes.unwrap().get(), 8388608);
    }
}
