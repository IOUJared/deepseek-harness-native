//! Explicit opt-in real-fork smoke. Never prompts a model or requests an external model catalog.
use dsh_native_core::{Backend, Command, PublicReply, RustBackendOptions, StopPolicy};
use dsh_native_transport::{Limits, NativeClient, dto::*};
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf, time::Duration};

fn options() -> Result<RustBackendOptions, &'static str> {
    let mut fields = HashMap::new();
    let mut args = std::env::args().skip(1);
    while let Some(key) = args.next() {
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
        if fields
            .insert(key, args.next().ok_or("missing-value")?)
            .is_some()
        {
            return Err("duplicate-argument");
        }
    }
    let mut take = |key: &str| fields.remove(key).ok_or("missing-required-argument");
    let value = RustBackendOptions {
        runtime: PathBuf::from(take("--runtime")?),
        expected_version: take("--expected-version")?,
        native_home: PathBuf::from(take("--home")?),
        working_directory: PathBuf::from(take("--cwd")?),
        user_home: PathBuf::from(take("--user-home")?),
        absolute_node_path: fields.remove("--node").map(PathBuf::from),
        startup_timeout: Duration::from_secs(15),
        stop_policy: StopPolicy::default(),
    };
    if [
        &value.runtime,
        &value.native_home,
        &value.working_directory,
        &value.user_home,
    ]
    .iter()
    .any(|p| !p.is_absolute() || !p.is_dir())
    {
        return Err("existing-absolute-directories-required");
    }
    Ok(value)
}

async fn checks(
    backend: &Backend,
    client: &NativeClient,
    workspace: PathBuf,
    report: &mut Value,
) -> Result<(), String> {
    let mux = client.connect_mux().await.map_err(|e| e.to_string())?;
    let result = async {
        let mut events = mux.events().await.map_err(|e| e.to_string())?;
        if !matches!(
            events.next().await,
            Some(Ok(RemoteEventFrame::Ready { .. }))
        ) {
            return Err("events-ready-required".into());
        }
        report["eventsReady"] = json!(true);
        let mut workspaces = mux.workspace_follow().await.map_err(|e| e.to_string())?;
        let Some(Ok(WorkspaceFollowFrame::Baseline { value })) = workspaces.next().await else {
            return Err("workspace-baseline-required".into());
        };
        let entry = value
            .items
            .iter()
            .find(|w| PathBuf::from(&w.path) == workspace)
            .ok_or("opened-workspace-missing")?;
        let workspace_id = entry.workspace_id.clone();
        report["workspaceCount"] = json!(value.items.len());
        let initial = client.session_list().await.map_err(|e| e.to_string())?;
        report["initialSessionCount"] = json!(initial.items.len());
        let absent = SessionId::new("PUBLIC_unregistered_native_agent_probe").map_err(|e| e.to_string())?;
        let PublicReply::AgentDiscovery(missing) = backend.request(Command::DiscoverSessionAgent(absent.clone()), Duration::from_secs(5)).map_err(|e| e.to_string())? else { return Err("discovery-reply-required".into()); };
        if missing.session_id != absent || missing.agent_id.is_some() { return Err("unregistered-discovery-mismatch".into()); }
        let created = client
            .session_create(SessionCreateRequest {
                workspace_id: Some(workspace_id),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        report["sessionCreated"] = json!(true);
        let command = Command::DiscoverSessionAgent(created.session_id.clone());
        let found = backend.request(command.clone(), Duration::from_secs(5)).map_err(|e| e.to_string())?;
        let PublicReply::AgentDiscovery(identity) = &found else { return Err("discovery-reply-required".into()); };
        if identity.session_id != created.session_id || identity.agent_id.is_none() { return Err("registered-discovery-mismatch".into()); }
        // Check this concrete alpha registry's documented equality; never construct AgentId from SessionId.
        if identity.agent_id.as_ref().unwrap().as_str() != created.session_id.as_str() { return Err("alpha-registry-id-invariant".into()); }
        if backend.request(command, Duration::from_secs(5)).map_err(|e| e.to_string())? != found { return Err("repeated-discovery-changed".into()); }
        report["agentDiscovery"] = json!({"unregisteredReturnedNone":true,"registeredReturnedTypedId":true,"sessionEchoMatched":true,"repeatedReadStable":true,"alphaSharedBytesObserved":true,"agentIdInferred":false});
        let address = SessionAddress::Session {
            session_id: created.session_id.clone(),
        };
        let mut session = mux
            .session_follow(SessionFollowRequest {
                address: address.clone(),
                max_messages: Some(50),
                turn_window: None,
                assistant_stream: None,
            })
            .await
            .map_err(|e| e.to_string())?;
        let Some(Ok(SessionFollowFrame::Snapshot {
            header,
            cursor,
            records,
            has_more,
            ..
        })) = session.next().await
        else {
            return Err("session-snapshot-required".into());
        };
        if header.id != created.session_id {
            return Err("snapshot-identity-mismatch".into());
        }
        let types: Vec<_> = records.iter().map(|record| match record {
            HistoryRecord::Event { event } => event.event_type.as_str(),
        }).collect();
        report["snapshot"] =
            json!({"cursor":cursor, "recordCount":records.len(), "hasMore":has_more, "eventTypes":types});
        let page = client
            .session_page(SessionPageRequest {
                address,
                through_seq: cursor,
                before_seq: None,
                max_messages: Some(50),
                turn_window: None,
            })
            .await
            .map_err(|e| e.to_string())?;
        report["page"] = json!({"recordCount":page.records.len(), "hasMore":page.has_more});
        let roster = client.session_list().await.map_err(|e| e.to_string())?;
        if !roster
            .items
            .iter()
            .any(|row| row.session_id == created.session_id)
        {
            return Err("created-session-missing-from-roster".into());
        }
        report["rosterIncludesCreatedSession"] = json!(true);
        let mut control = mux.control().await.map_err(|e| e.to_string())?;
        if !matches!(
            control.next().await,
            Some(Ok(ControlFrame::Baseline { .. }))
        ) {
            return Err("control-baseline-required".into());
        }
        report["controlBaseline"] = json!(true);
        session.cancel().await;
        workspaces.cancel().await;
        control.cancel().await;
        events.cancel().await;
        report["logicalStreamsCancelled"] = json!(true);
        Ok(())
    }
    .await;
    mux.close().await;
    result
}

fn main() {
    let mut report = json!({"status":"failed", "scope":"parent-owned-real-fork-native-core-http-ws-smoke", "modelPrompts":0, "modelCatalogRequested":false});
    let result = (|| -> Result<(), String> {
        let options = options().map_err(str::to_owned)?;
        let workspace = options.working_directory.clone();
        let (backend, _events) = std::thread::spawn(move || Backend::start(options))
            .join()
            .map_err(|_| "startup-worker-failed")?
            .map_err(|e| e.to_string())?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|_| "runtime-start-failed")?;
        let checked = runtime.block_on(async {
            let ready = backend.wait_ready().map_err(|e| e.to_string())?;
            report["ready"] = json!({"version":ready.version});
            if backend
                .request(
                    Command::OpenWorkspace(workspace.clone()),
                    Duration::from_secs(5),
                )
                .map_err(|e| e.to_string())?
                != PublicReply::WorkspaceAdded
            {
                return Err("workspace-open-failed".into());
            }
            let client = backend
                .connect_transport(Limits {
                    timeout: Duration::from_secs(5),
                    ..Default::default()
                })
                .await
                .map_err(|e| e.to_string())?;
            let result = tokio::time::timeout(
                Duration::from_secs(15),
                checks(&backend, &client, workspace, &mut report),
            )
            .await
            .map_err(|_| "checks-deadline".to_owned())
            .and_then(|r| r);
            // Verify Core revokes every clone immediately, before Host shutdown/reaping.
            let receiver = backend.stop_async();
            if !matches!(
                backend.request(
                    Command::DiscoverSessionAgent(SessionId::new("PUBLIC_stopped_probe").unwrap()),
                    Duration::from_secs(1)
                ),
                Err(dsh_native_core::Error::Stopping)
            ) {
                return Err("core-stop-did-not-fence-discovery".into());
            }
            report["coreStopRejectsDiscovery"] = json!(true);
            if !matches!(
                client.session_list().await,
                Err(dsh_native_transport::Error::Closed)
            ) {
                return Err("core-stop-did-not-invalidate-client".into());
            }
            report["coreStopInvalidatesTransport"] = json!(true);
            client.close().await;
            drop(receiver);
            result
        });
        let stopped = backend.stop().map_err(|e| e.to_string());
        if let Ok(value) = &stopped {
            report["stop"] = json!({"exited":value.exited, "graceful":value.graceful, "exitCode":value.exit_code, "containmentUnknown":value.containment_unknown, "observedDescendantsRemaining":value.observed_descendants_remaining});
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
    println!("{report}");
    if report["status"] != "passed" {
        std::process::exit(1);
    }
}
