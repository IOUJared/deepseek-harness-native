//! Explicit isolated raw-upload qualification; no prompt, account mutation or model catalog request.
#[path = "support/options.rs"]
mod options;
use dsh_native_core::{Backend, Command, PublicReply, RustBackendOptions};
use dsh_native_transport::{
    Error, Limits, NativeClient,
    dto::*,
    upload::{FileUpload, MAX_UPLOAD_BYTES},
};
use serde_json::{Value, json};
use std::time::Duration;

fn discovered(backend: &Backend, session: &SessionId) -> Result<bool, String> {
    match backend
        .request(
            Command::DiscoverSessionAgent(session.clone()),
            Duration::from_secs(5),
        )
        .map_err(|e| e.to_string())?
    {
        PublicReply::AgentDiscovery(value) if value.session_id == *session => {
            Ok(value.agent_id.is_some())
        }
        _ => Err("discovery-identity-mismatch".into()),
    }
}
fn source(case: &str) -> (Vec<u8>, Option<String>) {
    match case {
        "raw" => (
            (0..4097).map(|i| (i * 17 % 256) as u8).collect(),
            Some("PUBLIC report é:?.bin".into()),
        ),
        "alias" => (
            (0..4097).map(|i| (i * 17 % 256) as u8).collect(),
            Some("PUBLIC alias.bin".into()),
        ),
        "empty" => (Vec::new(), None),
        "ceiling" => (
            (0..MAX_UPLOAD_BYTES).map(|i| (i * 13 + 7) as u8).collect(),
            Some("PUBLIC ceiling.bin".into()),
        ),
        "cold" => (vec![80, 0, 255, 128, 10], Some("PUBLIC cold.bin".into())),
        _ => unreachable!(),
    }
}
async fn transfer(
    client: &NativeClient,
    target: &SessionId,
    case: &str,
    report: &mut Value,
) -> Result<(), String> {
    let (bytes, name) = source(case);
    let value = client
        .upload_file(
            target,
            FileUpload::new(bytes, name).map_err(|e| e.to_string())?,
        )
        .await
        .map_err(|e| e.to_string())?;
    report["uploads"]
        .as_array_mut()
        .unwrap()
        .push(json!({"case":case,"value":value}));
    Ok(())
}
async fn epoch(
    options: &RustBackendOptions,
    target: &mut Option<SessionId>,
    index: usize,
    report: &mut Value,
) -> Result<(), String> {
    let boot = options.clone();
    let (backend, _events) = std::thread::spawn(move || Backend::start(boot))
        .join()
        .map_err(|_| "startup-worker-failed")?
        .map_err(|e| e.to_string())?;
    let ready = backend.wait_ready().map_err(|e| e.to_string())?;
    report["versions"]
        .as_array_mut()
        .unwrap()
        .push(json!(ready.version));
    if backend
        .request(
            Command::OpenWorkspace(options.working_directory.clone()),
            Duration::from_secs(5),
        )
        .map_err(|e| e.to_string())?
        != PublicReply::WorkspaceAdded
    {
        return Err("workspace-open-failed".into());
    }
    let client = backend
        .connect_transport(Limits::default())
        .await
        .map_err(|e| e.to_string())?;
    let checked = async {
        let mux = client.connect_mux().await.map_err(|e| e.to_string())?;
        let mut workspaces = mux.workspace_follow().await.map_err(|e| e.to_string())?;
        let Some(Ok(WorkspaceFollowFrame::Baseline { value })) = workspaces.next().await else {
            return Err("workspace-baseline-required".into());
        };
        let workspace = value
            .items
            .iter()
            .find(|item| std::path::Path::new(&item.path) == options.working_directory)
            .ok_or("workspace-missing")?;
        if index == 0 {
            let created = client
                .session_create(SessionCreateRequest {
                    workspace_id: Some(workspace.workspace_id.clone()),
                    ..Default::default()
                })
                .await
                .map_err(|e| e.to_string())?;
            *target = Some(created.session_id);
            let session = target.as_ref().unwrap();
            if !discovered(&backend, session)? {
                return Err("created-agent-missing".into());
            }
            report["liveAgentObserved"] = json!(true);
            for case in ["raw", "empty", "ceiling", "alias"] {
                transfer(&client, session, case, report).await?;
            }
            let missing = SessionId::new("PUBLIC_UNKNOWN_FILE_UPLOAD_SESSION").unwrap();
            if !matches!(
                client
                    .upload_file(&missing, FileUpload::new(vec![80, 85, 66], None).unwrap())
                    .await,
                Err(Error::RemoteFailure)
            ) {
                return Err("missing-target-not-refused".into());
            }
            report["missingTargetRefused"] = json!(true);
        } else {
            let session = target.as_ref().ok_or("target-missing")?;
            if discovered(&backend, session)? {
                return Err("persisted-session-not-cold".into());
            }
            report["coldBeforeUpload"] = json!(true);
            transfer(&client, session, "cold", report).await?;
            if !discovered(&backend, session)? {
                return Err("cold-upload-did-not-resolve-agent".into());
            }
            report["resumedAfterUpload"] = json!(true);
        }
        let mut view = mux
            .session_follow(SessionFollowRequest {
                address: SessionAddress::Session {
                    session_id: target.as_ref().unwrap().clone(),
                },
                max_messages: Some(50),
                turn_window: None,
                assistant_stream: None,
            })
            .await
            .map_err(|e| e.to_string())?;
        let Some(Ok(SessionFollowFrame::Snapshot {
            header,
            records,
            has_more,
            ..
        })) = view.next().await
        else {
            return Err("snapshot-required".into());
        };
        if header.id != *target.as_ref().unwrap() || has_more {
            return Err("incomplete-or-wrong-snapshot".into());
        }
        let event_types: Vec<_> = records
            .iter()
            .map(|record| match record {
                HistoryRecord::Event { event } => event.event_type.as_str(),
            })
            .collect();
        report["postUploadSnapshots"]
            .as_array_mut()
            .unwrap()
            .push(json!(event_types));
        let mut expected = vec!["permission/preset", "sandbox/mode", "approval/policy"];
        if index == 1 {
            expected.push("session/end-seed");
        }
        if event_types != expected {
            return Err("unexpected-session-event".into());
        }
        drop(view);
        drop(workspaces);
        mux.close().await;
        Ok::<(), String>(())
    }
    .await;
    let stopping = backend.stop_async();
    client.close().await;
    let stale = client
        .upload_file(
            &SessionId::new("PUBLIC_STOPPED_UPLOAD").unwrap(),
            FileUpload::new(Vec::new(), None).unwrap(),
        )
        .await;
    let stop = backend.stop().map_err(|e| e.to_string());
    drop(stopping);
    let stop = stop?;
    report["stops"].as_array_mut().unwrap().push(json!({"exited":stop.exited,"graceful":stop.graceful,"exitCode":stop.exit_code,"containmentUnknown":stop.containment_unknown,"observedDescendantsRemaining":stop.observed_descendants_remaining}));
    if !stop.exited
        || !stop.graceful
        || stop.exit_code != Some(0)
        || stop.containment_unknown
        || stop.observed_descendants_remaining != 0
    {
        return Err("unclean-stop".into());
    }
    checked?;
    if !matches!(stale, Err(Error::Closed)) {
        return Err("stale-transport-accepted-upload".into());
    }
    Ok(())
}
#[tokio::main(worker_threads = 2)]
async fn main() {
    let mut report = json!({"status":"failed","versions":[],"uploads":[],"stops":[],"postUploadSnapshots":[],"explicitModelPrompts":0,"modelCatalogRequested":false,"processWideNetworkTrace":false,"uiExercised":false});
    let result = async {
        let options = options::parse().map_err(str::to_owned)?;
        let mut target = None;
        for index in 0..2 {
            epoch(&options, &mut target, index, &mut report).await?;
        }
        Ok::<(), String>(())
    }
    .await;
    match result {
        Ok(()) => report["status"] = json!("passed"),
        Err(error) => report["error"] = json!(error),
    }
    println!("{report}");
    if report["status"] != "passed" {
        std::process::exit(1);
    }
}
