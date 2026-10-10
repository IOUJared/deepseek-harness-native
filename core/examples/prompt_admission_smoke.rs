//! Read-only cold-journal qualification against an explicitly supplied isolated alpha Host.
//! The parent seeds the session through public persistence APIs; this example never sends a prompt.
#[path = "support/options.rs"]
mod options;

use dsh_native_core::Backend;
use dsh_native_transport::{
    Limits, NativeClient,
    dto::{SessionId, SessionListValue, SessionRequestId},
    prompt_admission::{
        AdmissionEvidence, AdmissionPhase, PromptAdmission, PromptAdmissionRequest, PromptMessageId,
    },
};
use serde_json::{Value, json};
use std::{io::Write, process::ExitCode, sync::Arc, time::Duration};

const SESSION: &str = "PUBLIC-prompt-admission-cold";
const ORIGINAL: &str = "PUBLIC-original-request";
const UNKNOWN: &str = "PUBLIC-unknown-request";

fn cold_roster(value: &SessionListValue, session: &SessionId) -> Result<(), &'static str> {
    let mut rows = value.items.iter().filter(|row| &row.session_id == session);
    let row = rows.next().ok_or("cold-session-missing")?;
    if rows.next().is_some() {
        return Err("cold-session-ambiguous");
    }
    if row.agent_available {
        return Err("cold-agent-activated");
    }
    Ok(())
}

fn admitted_zero(value: PromptAdmission) -> Result<PromptMessageId, &'static str> {
    match value {
        PromptAdmission::Accepted {
            evidence: AdmissionEvidence::Journal,
            phase: AdmissionPhase::Admitted,
            message_id: Some(message_id),
            seq: Some(0),
        } => Ok(message_id),
        _ => Err("journal-admitted-zero-required"),
    }
}

async fn observations(
    backend: &Arc<Backend>,
    client: &NativeClient,
    report: &mut Value,
) -> Result<(), &'static str> {
    let session = SessionId::new(SESSION).map_err(|_| "fixture-session-id")?;
    let original = SessionRequestId::new(ORIGINAL).map_err(|_| "fixture-request-id")?;
    report["phase"] = json!("roster-before");
    let before = client.session_list().await.map_err(|_| "roster-before")?;
    cold_roster(&before, &session)?;
    report["agentAvailableBefore"] = json!(false);

    report["phase"] = json!("original-admission");
    let request = PromptAdmissionRequest {
        session_id: session.clone(),
        request_id: original,
    };
    let message = admitted_zero(
        client
            .session_prompt_admission(request.clone())
            .await
            .map_err(|_| "original-admission")?,
    )?;
    report["originalAdmission"] = json!({"state":"accepted","evidence":"journal","phase":"admitted","seq":0,"messageIdPresent":true});

    report["phase"] = json!("unknown-admission");
    let unknown = client
        .session_prompt_admission(PromptAdmissionRequest {
            session_id: session.clone(),
            request_id: SessionRequestId::new(UNKNOWN).map_err(|_| "fixture-unknown-id")?,
        })
        .await
        .map_err(|_| "unknown-admission")?;
    if !matches!(unknown, PromptAdmission::Unknown) {
        return Err("unknown-state-required");
    }
    report["unknownAdmission"] = json!("unknown");

    report["phase"] = json!("same-owned-host");
    let proven = backend
        .probe_owned_host(Duration::from_secs(3))
        .await
        .map_err(|_| "same-owned-host")?;
    report["ownedHostProbed"] = json!(true);
    report["phase"] = json!("original-admission-repeat");
    let repeated = admitted_zero(
        proven
            .session_prompt_admission(request)
            .await
            .map_err(|_| "original-admission-repeat")?,
    )?;
    if repeated != message {
        return Err("original-message-changed");
    }
    report["originalReceiptStable"] = json!(true);

    report["phase"] = json!("roster-after");
    let after = proven.session_list().await.map_err(|_| "roster-after")?;
    cold_roster(&after, &session)?;
    report["agentAvailableAfter"] = json!(false);
    Ok(())
}

fn run(report: &mut Value) -> Result<(), &'static str> {
    let options = options::parse()?;
    // Build the runtime before acquiring a Host, so its construction failure owns no child.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| "runtime-start")?;
    report["phase"] = json!("host-start");
    let (backend, _events) = Backend::start(options).map_err(|_| "host-start")?;
    let backend = Arc::new(backend);
    let checked = runtime.block_on(async {
        report["phase"] = json!("host-ready");
        backend.wait_ready().map_err(|_| "host-ready")?;
        report["hostReady"] = json!(true);
        report["phase"] = json!("transport-connect");
        let client = backend
            .connect_transport(Limits {
                timeout: Duration::from_secs(5),
                ..Default::default()
            })
            .await
            .map_err(|_| "transport-connect")?;
        report["transportInitializedOnce"] = json!(true);
        let result = tokio::time::timeout(
            Duration::from_secs(20),
            observations(&backend, &client, report),
        )
        .await
        .map_err(|_| "observation-deadline")
        .and_then(|result| result);
        client.close().await;
        report["clientClosed"] = json!(true);
        result
    });
    // Readiness, exchange and observation failures still stop and join the acquired owner.
    let clean_stop = match backend.stop() {
        Ok(stop) => {
            report["stop"] = json!({"confirmed":true,"exited":stop.exited,"graceful":stop.graceful,"containmentUnknown":stop.containment_unknown,"remainingDescendants":stop.observed_descendants_remaining,"exitCode":stop.exit_code});
            stop.exited
                && stop.graceful
                && !stop.containment_unknown
                && stop.observed_descendants_remaining == 0
                && stop.exit_code == Some(0)
        }
        Err(_) => {
            report["stop"] = json!({"confirmed":false});
            false
        }
    };
    runtime.shutdown_timeout(Duration::from_secs(2));
    checked?;
    if !clean_stop {
        return Err("shutdown-not-clean");
    }
    report["status"] = json!("passed");
    report["phase"] = json!("complete");
    Ok(())
}

fn main() -> ExitCode {
    let mut report = json!({"status":"failed","phase":"options","scope":"real-cold-journal-native-core-read-only","hostReady":false,"transportInitializedOnce":false,"ownedHostProbed":false,"clientClosed":false,"modelPrompts":0,"sessionFollows":0,"uploads":0,"modelCatalogRequests":0,"signInRequests":0});
    let passed = match run(&mut report) {
        Ok(()) => true,
        Err(phase) => {
            report["phase"] = json!(phase);
            false
        }
    };
    let mut output = std::io::stdout().lock();
    let written =
        serde_json::to_writer(&mut output, &report).is_ok() && output.write_all(b"\n").is_ok();
    if passed && written {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
