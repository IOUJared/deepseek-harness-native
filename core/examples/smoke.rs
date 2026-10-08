//! Parent-owned opt-in smoke. Compile is safe; execution starts the explicitly supplied real fork.
use dsh_native_core::{
    AccountMetadata, AccountStatus, AttemptPhase, Backend, Command, PublicReply,
    RustBackendOptions, StopPolicy, StopResult,
};
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf, thread, time::Duration};

fn options() -> Result<RustBackendOptions, &'static str> {
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
    Ok(options)
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

fn checks(backend: &Backend, workspace: PathBuf, report: &mut Value) -> Result<(), String> {
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
        let options = options().map_err(str::to_owned)?;
        let workspace = options.working_directory.clone();
        // The startup caller exits while the core's actual spawning/ownership thread remains alive.
        let (backend, _events) = thread::spawn(move || Backend::start(options))
            .join()
            .map_err(|_| "startup-worker-failed".to_owned())?
            .map_err(|error| error.to_string())?;
        let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            checks(&backend, workspace, &mut report)
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
