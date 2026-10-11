//! Parent-owned opt-in smoke. Compile is safe; execution starts the explicitly supplied real fork.
use dsh_native_core::{
    AccountMetadata, AccountStatus, AttemptPhase, Backend, CloudflareAuthMode,
    CloudflareOauthSetup, CloudflareSaveChoice, CodexCacheRetention, CodexComposerDisplay,
    CodexIdleMillis, CodexImageBudget, CodexRemainingPercent, CodexRuntimeNatural,
    CodexRuntimeSnapshot, CodexRuntimeTransport, CodexRuntimeValues, CodexUsageAlert,
    CodexUsagePreferences, CodexUsageSnapshot, Command, Error, PublicReply, RustBackendOptions,
    SecretApiKey, StopPolicy, StopResult,
};
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf, thread, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UsageStage {
    Initial,
    Restored,
    Corrupt,
}
impl UsageStage {
    fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "initial" => Ok(Self::Initial),
            "restored" => Ok(Self::Restored),
            "corrupt" => Ok(Self::Corrupt),
            _ => Err("invalid-usage-stage"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeStage {
    Initial,
    Restored,
}
impl RuntimeStage {
    fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "initial" => Ok(Self::Initial),
            "restored" => Ok(Self::Restored),
            _ => Err("invalid-runtime-stage"),
        }
    }
}
fn stages(
    usage: Option<&str>,
    runtime: Option<&str>,
) -> Result<(Option<UsageStage>, Option<RuntimeStage>), &'static str> {
    if usage.is_some() && runtime.is_some() {
        return Err("usage-and-runtime-stages-overlap");
    }
    Ok((
        usage.map(UsageStage::parse).transpose()?,
        runtime.map(RuntimeStage::parse).transpose()?,
    ))
}

fn cloudflare_stage(
    usage: Option<UsageStage>,
    runtime: Option<RuntimeStage>,
    cloudflare: Option<&str>,
) -> Result<Option<RuntimeStage>, &'static str> {
    if cloudflare.is_some() && (usage.is_some() || runtime.is_some()) {
        return Err("cloudflare-stage-overlap");
    }
    cloudflare.map(RuntimeStage::parse).transpose()
}
fn options() -> Result<
    (
        RustBackendOptions,
        Option<UsageStage>,
        Option<RuntimeStage>,
        Option<RuntimeStage>,
        Option<RuntimeStage>,
    ),
    &'static str,
> {
    let mut fields = HashMap::new();
    let mut arguments = std::env::args().skip(1);
    while let Some(key) = arguments.next() {
        if ![
            "--runtime",
            "--expected-version",
            "--home",
            "--cwd",
            "--user-home",
            "--node",
            "--usage-stage",
            "--runtime-stage",
            "--cloudflare-stage",
            "--cloudflare-oauth-stage",
        ]
        .contains(&key.as_str())
        {
            return Err("invalid-arguments");
        }
        let value = arguments.next().ok_or("missing-argument-value")?;
        if fields.insert(key, value).is_some() {
            return Err("duplicate-argument");
        }
    }
    let usage = fields.remove("--usage-stage");
    let runtime = fields.remove("--runtime-stage");
    let (usage, runtime) = stages(usage.as_deref(), runtime.as_deref())?;
    let cloudflare = cloudflare_stage(
        usage,
        runtime,
        fields.remove("--cloudflare-stage").as_deref(),
    )?;
    let cloudflare_oauth = fields.remove("--cloudflare-oauth-stage");
    if cloudflare_oauth.is_some() && (usage.is_some() || runtime.is_some() || cloudflare.is_some())
    {
        return Err("cloudflare-oauth-stage-overlap");
    }
    let cloudflare_oauth = cloudflare_oauth
        .as_deref()
        .map(RuntimeStage::parse)
        .transpose()?;
    let mut take = |key: &str| fields.remove(key).ok_or("missing-required-argument");
    let options = RustBackendOptions {
        runtime: PathBuf::from(take("--runtime")?),
        expected_version: take("--expected-version")?,
        native_home: PathBuf::from(take("--home")?),
        working_directory: PathBuf::from(take("--cwd")?),
        user_home: PathBuf::from(take("--user-home")?),
        absolute_node_path: fields.remove("--node").map(PathBuf::from),
        startup_timeout: Duration::from_secs(15),
        stop_policy: StopPolicy::default(),
    };
    // The parent prepares all directories. This example does not create an application home.
    if [
        &options.runtime,
        &options.native_home,
        &options.working_directory,
        &options.user_home,
    ]
    .iter()
    .any(|path| !path.is_absolute() || !path.is_dir())
    {
        return Err("existing-absolute-directories-required");
    }
    Ok((options, usage, runtime, cloudflare, cloudflare_oauth))
}

fn account(value: AccountMetadata) -> Value {
    let status = match value.status {
        AccountStatus::SignedOut => "signed-out",
        AccountStatus::CredentialStored => "credential-stored",
    };
    let phase = value.phase.map(|phase| match phase {
        AttemptPhase::Initializing => "initializing",
        AttemptPhase::WaitingBrowser => "waiting-browser",
        AttemptPhase::Exchanging => "exchanging",
        AttemptPhase::Committing => "committing",
        AttemptPhase::Succeeded => "succeeded",
        AttemptPhase::Cancelled => "cancelled",
        AttemptPhase::Expired => "expired",
        AttemptPhase::Failed => "failed",
    });
    json!({"status":status,"phase":phase})
}
fn stop_metadata(value: StopResult) -> Value {
    json!({"exited":value.exited,"graceful":value.graceful,"exitCode":value.exit_code,
        "containmentUnknown":value.containment_unknown,"observedDescendantsRemaining":value.observed_descendants_remaining})
}

fn usage_preferences() -> Result<CodexUsagePreferences, Error> {
    Ok(CodexUsagePreferences {
        composer_display: CodexComposerDisplay::Runway,
        usage_alert: CodexUsageAlert::Custom,
        custom_remaining_percent: CodexRemainingPercent::new(13.375)?,
        retry_after_reset: true,
        experimental_quota: false,
    })
}

fn usage_checks(backend: &Backend, stage: UsageStage, report: &mut Value) -> Result<(), String> {
    let request = |command| backend.request(command, Duration::from_secs(5));
    if stage == UsageStage::Corrupt {
        for _ in 0..2 {
            if !matches!(
                request(Command::CodexUsageLoad),
                Err(Error::CodexUsageUnavailable)
            ) {
                return Err("corrupt-preferences-not-rejected".into());
            }
        }
        report["usagePreferences"] =
            json!({"stage":"corrupt", "unavailable":true, "writesIssued":0});
        return Ok(());
    }
    let PublicReply::CodexUsage(loaded) =
        request(Command::CodexUsageLoad).map_err(|error| error.to_string())?
    else {
        return Err("unexpected-usage-reply".into());
    };
    let expected = CodexUsageSnapshot {
        version: 1,
        revision: 1,
        preferences: usage_preferences().map_err(|error| error.to_string())?,
        persisted: true,
    };
    if stage == UsageStage::Restored {
        if loaded != expected {
            return Err("usage-restoration-mismatch".into());
        }
        report["usagePreferences"] = json!({"stage":"restored", "revision":loaded.revision, "persisted":loaded.persisted, "betaConsent":loaded.preferences.experimental_quota});
        return Ok(());
    }
    if loaded != CodexUsageSnapshot::default() {
        return Err("initial-usage-not-unwritten".into());
    }
    let saved = request(Command::CodexUsageSave {
        expected_revision: 0,
        preferences: expected.preferences,
    })
    .map_err(|error| error.to_string())?;
    if saved != PublicReply::CodexUsage(expected) {
        return Err("usage-commit-ack-mismatch".into());
    }
    if !matches!(
        request(Command::CodexUsageSave {
            expected_revision: 0,
            preferences: CodexUsagePreferences::default()
        }),
        Err(Error::CodexUsageConflict)
    ) {
        return Err("stale-usage-save-not-rejected".into());
    }
    if request(Command::CodexUsageLoad).map_err(|error| error.to_string())?
        != PublicReply::CodexUsage(expected)
    {
        return Err("stale-usage-save-changed-document".into());
    }
    report["usagePreferences"] = json!({"stage":"initial", "revision":expected.revision, "persisted":true, "betaConsent":false, "staleSaveRejected":true});
    Ok(())
}

fn runtime_values() -> Result<CodexRuntimeValues, Error> {
    Ok(CodexRuntimeValues {
        transport: Some(CodexRuntimeTransport::Sse),
        cache_retention: Some(CodexCacheRetention::None),
        timeout_ms: Some(CodexRuntimeNatural::new(17_000)?),
        websocket_connect_timeout_ms: Some(CodexRuntimeNatural::new(9_000)?),
        stream_idle_timeout_ms: Some(CodexIdleMillis::new(32_000.0)?),
        request_image_pixel_budget: Some(CodexImageBudget::new(1_048_576)?),
        request_image_max_bytes: Some(CodexImageBudget::new(524_288)?),
        max_request_image_bytes: Some(CodexImageBudget::new(8_388_608)?),
    })
}
fn runtime_values_metadata(values: CodexRuntimeValues) -> Value {
    json!({
        "transport":values.transport.map(CodexRuntimeTransport::as_str),
        "cacheRetention":values.cache_retention.map(CodexCacheRetention::as_str),
        "timeoutMs":values.timeout_ms.map(CodexRuntimeNatural::get),
        "websocketConnectTimeoutMs":values.websocket_connect_timeout_ms.map(CodexRuntimeNatural::get),
        "streamIdleTimeoutMs":values.stream_idle_timeout_ms.map(CodexIdleMillis::as_f64),
        "requestImagePixelBudget":values.request_image_pixel_budget.map(CodexImageBudget::get),
        "requestImageMaxBytes":values.request_image_max_bytes.map(CodexImageBudget::get),
        "maxRequestImageBytes":values.max_request_image_bytes.map(CodexImageBudget::get),
    })
}
fn runtime_metadata(snapshot: CodexRuntimeSnapshot) -> Value {
    json!({
        "version":snapshot.version, "revision":snapshot.revision,
        "configured":snapshot.configured, "writable":snapshot.writable,
        "values":runtime_values_metadata(snapshot.values),
        "baseValues":runtime_values_metadata(snapshot.base_values),
        "overrides":runtime_values_metadata(snapshot.overrides),
    })
}
fn runtime_checks(
    backend: &Backend,
    stage: RuntimeStage,
    report: &mut Value,
) -> Result<(), String> {
    let request = |command| backend.request(command, Duration::from_secs(5));
    let PublicReply::CodexRuntime(loaded) =
        request(Command::CodexRuntimeRead).map_err(|error| error.to_string())?
    else {
        return Err("unexpected-runtime-reply".into());
    };
    if !loaded.configured || !loaded.writable || loaded.revision >= 9_007_199_254_740_990 {
        return Err("runtime-fixture-not-configured-writable-with-revision-capacity".into());
    }
    let known = runtime_values().map_err(|error| error.to_string())?;
    let inherited = CodexRuntimeValues::default();
    if stage == RuntimeStage::Restored {
        if loaded.values != known || loaded.overrides != known {
            return Err("runtime-restoration-mismatch".into());
        }
        let PublicReply::CodexRuntime(reset) = request(Command::CodexRuntimeSave {
            expected_revision: loaded.revision,
            values: inherited,
        })
        .map_err(|error| error.to_string())?
        else {
            return Err("unexpected-runtime-reset-reply".into());
        };
        if reset.version != 1
            || reset.revision != loaded.revision + 1
            || !reset.configured
            || !reset.writable
            || reset.overrides != inherited
            || reset.base_values != loaded.base_values
            || reset.values != reset.base_values
        {
            return Err("runtime-inheritance-reset-mismatch".into());
        }
        if request(Command::CodexRuntimeRead).map_err(|error| error.to_string())?
            != PublicReply::CodexRuntime(reset)
        {
            return Err("runtime-reset-read-mismatch".into());
        }
        report["runtimeSettings"] = json!({
            "stage":"restored", "loaded":runtime_metadata(loaded),
            "reset":runtime_metadata(reset), "inheritanceRestored":true,
        });
        return Ok(());
    }
    if loaded.overrides != inherited || loaded.values != loaded.base_values {
        return Err("initial-runtime-not-inherited".into());
    }
    let PublicReply::CodexRuntime(saved) = request(Command::CodexRuntimeSave {
        expected_revision: loaded.revision,
        values: known,
    })
    .map_err(|error| error.to_string())?
    else {
        return Err("unexpected-runtime-save-reply".into());
    };
    if saved.version != 1
        || saved.revision != loaded.revision + 1
        || !saved.configured
        || !saved.writable
        || saved.values != known
        || saved.overrides != known
        || saved.base_values != loaded.base_values
    {
        return Err("runtime-commit-ack-mismatch".into());
    }
    if !matches!(
        request(Command::CodexRuntimeSave {
            expected_revision: loaded.revision,
            values: inherited,
        }),
        Err(Error::CodexRuntimeConflict)
    ) {
        return Err("stale-runtime-save-not-rejected".into());
    }
    if request(Command::CodexRuntimeRead).map_err(|error| error.to_string())?
        != PublicReply::CodexRuntime(saved)
    {
        return Err("stale-runtime-save-changed-settings".into());
    }
    report["runtimeSettings"] = json!({
        "stage":"initial", "loaded":runtime_metadata(loaded),
        "saved":runtime_metadata(saved), "staleSaveRejected":true,
        "unchangedAfterConflict":true,
    });
    Ok(())
}

fn cloudflare_checks(
    backend: &Backend,
    stage: RuntimeStage,
    report: &mut Value,
) -> Result<(), String> {
    const ACCOUNT: &str = "0123456789abcdef0123456789abcdef";
    let read = || match backend.request(Command::CloudflareRead, Duration::from_secs(5)) {
        Ok(PublicReply::Cloudflare(value)) => Ok(value),
        _ => Err("cloudflare-read-failed".to_owned()),
    };
    let initial = read()?;
    if !initial.writable || initial.models_enabled || initial.browser_enabled {
        return Err("cloudflare-private-disabled-fixture-required".into());
    }
    if stage == RuntimeStage::Restored {
        if initial.account_id != ACCOUNT || !initial.token_configured || initial.free_plan_confirmed
        {
            return Err("cloudflare-restoration-mismatch".into());
        }
        report["cloudflare"] = json!({"stage":"restored", "tokenPresenceRestored":true, "featuresDisabled":true, "modelRequests":0, "browserRequests":0});
        return Ok(());
    }
    if !initial.account_id.is_empty() || initial.token_configured {
        return Err("cloudflare-new-private-home-required".into());
    }
    let secret = SecretApiKey::new("PUBLIC_CLOUDFLARE_PIPE_FIXTURE_NOT_A_REAL_TOKEN".into())
        .map_err(|error| error.to_string())?;
    let saved = backend
        .save_cloudflare(
            initial.revision,
            ACCOUNT,
            CloudflareSaveChoice {
                models_enabled: false,
                browser_enabled: false,
                free_plan_confirmed: false,
            },
            Some(secret),
            Duration::from_secs(5),
        )
        .map_err(|error| error.to_string())?;
    if saved.account_id != ACCOUNT
        || !saved.token_configured
        || saved.models_enabled
        || saved.browser_enabled
        || saved.free_plan_confirmed
        || saved.revision != initial.revision + 1
    {
        return Err("cloudflare-save-mismatch".into());
    }
    if backend
        .save_cloudflare(
            initial.revision,
            ACCOUNT,
            CloudflareSaveChoice {
                models_enabled: false,
                browser_enabled: false,
                free_plan_confirmed: false,
            },
            None,
            Duration::from_secs(5),
        )
        .is_ok()
    {
        return Err("cloudflare-stale-save-not-refused".into());
    }
    if read()? != saved {
        return Err("cloudflare-stale-save-changed-metadata".into());
    }
    let PublicReply::Cloudflare(disabled) = backend
        .request(
            Command::CloudflareDisable {
                expected_revision: saved.revision,
            },
            Duration::from_secs(5),
        )
        .map_err(|error| error.to_string())?
    else {
        return Err("cloudflare-disable-reply-required".into());
    };
    if disabled.models_enabled
        || disabled.browser_enabled
        || disabled.account_id != ACCOUNT
        || !disabled.token_configured
    {
        return Err("cloudflare-disable-mismatch".into());
    }
    report["cloudflare"] = json!({"stage":"initial", "tokenPersistenceAcknowledged":true, "staleSaveRejected":true, "featuresDisabled":true, "modelRequests":0, "browserRequests":0});
    Ok(())
}

fn cloudflare_oauth_checks(
    backend: &Backend,
    stage: RuntimeStage,
    report: &mut Value,
) -> Result<(), String> {
    const ACCOUNT: &str = "0123456789abcdef0123456789abcdef";
    let setup = CloudflareOauthSetup {
        client_id: "PUBLIC_OWN_DESKTOP_CLIENT".into(),
        scopes: vec!["PUBLIC.product.read".into(), "offline_access".into()],
        callback_port: 8977,
    };
    let request = |command| match backend.request(command, Duration::from_secs(5)) {
        Ok(PublicReply::Cloudflare(value)) => Ok(value),
        _ => Err("cloudflare-oauth-metadata-request-failed".to_owned()),
    };
    let initial = request(Command::CloudflareRead)?;
    if !initial.writable
        || initial.models_enabled
        || initial.browser_enabled
        || initial.free_plan_confirmed
    {
        return Err("cloudflare-oauth-disabled-private-fixture-required".into());
    }
    let mut configured = if stage == RuntimeStage::Initial {
        if !initial.account_id.is_empty()
            || initial.token_configured
            || !initial.oauth_setup.client_id.is_empty()
        {
            return Err("cloudflare-oauth-new-private-home-required".into());
        }
        let token = SecretApiKey::new("PUBLIC_CLOUDFLARE_PIPE_FIXTURE_NOT_A_REAL_TOKEN".into())
            .map_err(|e| e.to_string())?;
        let saved = backend
            .save_cloudflare(
                initial.revision,
                ACCOUNT,
                CloudflareSaveChoice {
                    models_enabled: false,
                    browser_enabled: false,
                    free_plan_confirmed: false,
                },
                Some(token),
                Duration::from_secs(5),
            )
            .map_err(|e| e.to_string())?;
        let configured = request(Command::CloudflareSetupSave {
            expected_revision: saved.revision,
            setup: setup.clone(),
        })?;
        if backend
            .request(
                Command::CloudflareAuthModeSave {
                    expected_revision: initial.revision,
                    auth_mode: CloudflareAuthMode::ApiToken,
                },
                Duration::from_secs(5),
            )
            .is_ok()
        {
            return Err("cloudflare-oauth-stale-mode-not-refused".into());
        }
        configured
    } else {
        initial
    };
    if configured.account_id != ACCOUNT
        || configured.oauth_setup != setup
        || configured.auth_status.auth_mode != CloudflareAuthMode::Oauth
        || configured.token_configured
        || !configured.auth_status.setup_configured
        || configured.models_enabled
        || configured.browser_enabled
        || configured.free_plan_confirmed
    {
        return Err("cloudflare-oauth-registration-restoration-mismatch".into());
    }
    configured = request(Command::CloudflareAuthModeSave {
        expected_revision: configured.revision,
        auth_mode: CloudflareAuthMode::ApiToken,
    })?;
    if !configured.token_configured
        || configured.auth_status.auth_mode != CloudflareAuthMode::ApiToken
        || configured.models_enabled
        || configured.browser_enabled
    {
        return Err("cloudflare-oauth-explicit-saved-api-selection-failed".into());
    }
    configured = request(Command::CloudflareAuthModeSave {
        expected_revision: configured.revision,
        auth_mode: CloudflareAuthMode::Oauth,
    })?;
    if configured.token_configured
        || configured.auth_status.auth_mode != CloudflareAuthMode::Oauth
        || configured.models_enabled
        || configured.browser_enabled
    {
        return Err("cloudflare-oauth-no-automatic-api-fallback-failed".into());
    }
    report["cloudflareOAuth"] = json!({"stage":if stage == RuntimeStage::Initial {"initial"} else {"restored"},
        "registrationMetadataChecked":true,"savedApiTokenPreserved":true,"explicitModeSelection":true,
        "oauthCommandsIssued":false,"modelPromptsIssued":false,"featuresDisabled":true,"liveRegistrationVerified":false});
    Ok(())
}

fn checks(
    backend: &Backend,
    workspace: PathBuf,
    usage: Option<UsageStage>,
    runtime: Option<RuntimeStage>,
    cloudflare: Option<RuntimeStage>,
    cloudflare_oauth: Option<RuntimeStage>,
    report: &mut Value,
) -> Result<(), String> {
    let ready = backend.wait_ready().map_err(|error| error.to_string())?;
    report["ready"] = json!({"version":ready.version});
    let request = |command| {
        backend
            .request(command, Duration::from_secs(5))
            .map_err(|error| error.to_string())
    };
    let PublicReply::Inspection(inspection) = request(Command::Inspect)? else {
        return Err("unexpected-inspection-reply".into());
    };
    report["inspection"] = json!({"activeTasks":inspection.active_tasks,"scheduledTasks":inspection.scheduled_tasks,"unknown":inspection.unknown});
    if !inspection.is_known_idle() {
        return Err("inspection-not-known-idle".into());
    }
    if let Some(stage) = cloudflare_oauth {
        return cloudflare_oauth_checks(backend, stage, report);
    }
    if let Some(stage) = cloudflare {
        return cloudflare_checks(backend, stage, report);
    }
    if let Some(stage) = usage {
        // Preference qualification never issues account, login, workspace or model commands.
        return usage_checks(backend, stage, report);
    }
    if let Some(stage) = runtime {
        // Runtime preference qualification never issues account, login, workspace or model commands.
        return runtime_checks(backend, stage, report);
    }
    let PublicReply::Onboarding(onboarding) = request(Command::OnboardingRead)? else {
        return Err("unexpected-onboarding-reply".into());
    };
    report["onboarding"] = json!({"loggedIn":onboarding.logged_in,"hasApiKey":onboarding.has_api_key,"writable":onboarding.writable});
    let PublicReply::Account(state) = request(Command::AccountState)? else {
        return Err("unexpected-account-reply".into());
    };
    report["account"] = account(state);
    let PublicReply::Account(state) = request(Command::AccountSubscribe)? else {
        return Err("unexpected-subscription-reply".into());
    };
    report["accountSubscriptionSnapshot"] = account(state);
    if request(Command::AccountUnsubscribe)? != PublicReply::Unsubscribed {
        return Err("unexpected-unsubscription-reply".into());
    }
    report["accountUnsubscribed"] = json!(true);
    if request(Command::OpenWorkspace(workspace))? != PublicReply::WorkspaceAdded {
        return Err("unexpected-workspace-reply".into());
    }
    report["workspaceAdded"] = json!(true);
    Ok(())
}

fn main() {
    let mut report = json!({"status":"failed","scope":"parent-owned-real-fork-native-core-smoke"});
    let result = (|| -> Result<(), String> {
        let (options, usage, runtime, cloudflare, cloudflare_oauth) =
            options().map_err(str::to_owned)?;
        let workspace = options.working_directory.clone();
        // The startup caller exits while the core's actual spawning/ownership thread remains alive.
        let (backend, _events) = thread::spawn(move || Backend::start(options))
            .join()
            .map_err(|_| "startup-worker-failed".to_owned())?
            .map_err(|error| error.to_string())?;
        let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            checks(
                &backend,
                workspace,
                usage,
                runtime,
                cloudflare,
                cloudflare_oauth,
                &mut report,
            )
        }))
        .map_err(|_| "check-worker-failed".to_owned())
        .and_then(|result| result);
        // Stop is unconditional after acquiring a handle, including every failed check/request.
        let stopped = backend.stop().map_err(|error| error.to_string());
        report["droppedPublicEvents"] = json!(backend.dropped_event_count());
        if let Ok(stopped) = &stopped {
            report["stop"] = stop_metadata(*stopped);
        }
        checked?;
        let stopped = stopped?;
        if !stopped.exited
            || !stopped.graceful
            || stopped.exit_code != Some(0)
            || stopped.containment_unknown
            || stopped.observed_descendants_remaining != 0
        {
            return Err("owned-shutdown-not-clean".into());
        }
        Ok(())
    })();
    match result {
        Ok(()) => report["status"] = json!("passed"),
        Err(error) => report["error"] = json!(error),
    }
    // This output is built exclusively from closed error codes and public metadata projections.
    println!("{report}");
    if report["status"] != "passed" {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usage_stages_are_explicit_closed_choices() {
        assert_eq!(UsageStage::parse("initial"), Ok(UsageStage::Initial));
        assert_eq!(UsageStage::parse("restored"), Ok(UsageStage::Restored));
        assert_eq!(UsageStage::parse("corrupt"), Ok(UsageStage::Corrupt));
        for value in ["", "initial ", "INITIAL", "default", "unknown"] {
            assert_eq!(UsageStage::parse(value), Err("invalid-usage-stage"));
        }
    }
    #[test]
    fn runtime_stages_are_closed_and_cannot_overlap_usage_stages() {
        assert_eq!(RuntimeStage::parse("initial"), Ok(RuntimeStage::Initial));
        assert_eq!(RuntimeStage::parse("restored"), Ok(RuntimeStage::Restored));
        for value in ["", "INITIAL", "initial ", "corrupt", "unknown"] {
            assert_eq!(RuntimeStage::parse(value), Err("invalid-runtime-stage"));
        }
        assert_eq!(stages(None, None), Ok((None, None)));
        assert_eq!(
            stages(Some("corrupt"), None),
            Ok((Some(UsageStage::Corrupt), None))
        );
        assert_eq!(
            stages(None, Some("initial")),
            Ok((None, Some(RuntimeStage::Initial)))
        );
        assert_eq!(
            stages(Some("initial"), Some("restored")),
            Err("usage-and-runtime-stages-overlap")
        );
        assert_eq!(
            stages(Some("invalid"), Some("invalid")),
            Err("usage-and-runtime-stages-overlap")
        );
    }
    #[test]
    fn runtime_qualification_values_pin_exactly_eight_public_fields() {
        let values = runtime_values().unwrap();
        assert_eq!(
            runtime_values_metadata(values),
            json!({
                "transport":"sse", "cacheRetention":"none", "timeoutMs":17_000,
                "websocketConnectTimeoutMs":9_000, "streamIdleTimeoutMs":32_000.0,
                "requestImagePixelBudget":1_048_576, "requestImageMaxBytes":524_288,
                "maxRequestImageBytes":8_388_608,
            })
        );
        let inherited = runtime_values_metadata(CodexRuntimeValues::default());
        assert_eq!(inherited.as_object().unwrap().len(), 8);
        assert!(inherited.as_object().unwrap().values().all(Value::is_null));
    }
    #[test]
    fn cloudflare_qualification_is_explicit_and_exclusive() {
        assert_eq!(
            cloudflare_stage(None, None, Some("initial")),
            Ok(Some(RuntimeStage::Initial))
        );
        assert_eq!(
            cloudflare_stage(None, None, Some("restored")),
            Ok(Some(RuntimeStage::Restored))
        );
        assert_eq!(cloudflare_stage(None, None, None), Ok(None));
        assert!(cloudflare_stage(None, None, Some("corrupt")).is_err());
        assert!(cloudflare_stage(Some(UsageStage::Initial), None, Some("initial")).is_err());
        assert!(cloudflare_stage(None, Some(RuntimeStage::Initial), Some("restored")).is_err());
    }
    #[test]
    fn qualification_preferences_preserve_fractions_and_never_consent_to_quota() {
        let preferences = usage_preferences().unwrap();
        assert_eq!(preferences.custom_remaining_percent.as_f64(), 13.375);
        assert_eq!(preferences.composer_display, CodexComposerDisplay::Runway);
        assert_eq!(preferences.usage_alert, CodexUsageAlert::Custom);
        assert!(preferences.retry_after_reset);
        assert!(!preferences.experimental_quota);
    }
}
