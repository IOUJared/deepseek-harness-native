//! Fixed PUBLIC headless real Worker/Core/Alpha file prompts, rejected before any model step.
//! CLI and static workspace files match file_stage_smoke; no chooser, GUI or credentials are exercised.
#![allow(dead_code)]
#[path = "../src/codex.rs"]
mod codex;
#[path = "../src/config.rs"]
mod config;
#[path = "../src/design.rs"]
mod design;
#[path = "../src/export_file.rs"]
mod export_file;
#[path = "../src/exporter.rs"]
mod exporter;
#[path = "../src/interactions.rs"]
mod interactions;
#[path = "../src/known.rs"]
mod known;
#[path = "../src/management.rs"]
mod management;
#[path = "../../core/examples/support/options.rs"]
mod options;
#[path = "../src/plugins.rs"]
mod plugins;
#[path = "../src/reducer.rs"]
mod reducer;
#[path = "../src/settings.rs"]
mod settings;
#[path = "../src/smoke.rs"]
mod smoke;
#[path = "../src/ui.rs"]
mod ui;
#[path = "../src/worker.rs"]
mod worker;

use dsh_native_core::RustBackendOptions;
use dsh_native_transport::dto::*;
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
use worker::{
    Command, Event,
    attachments::{Outcome, PromptOutcome, Submission, Ticket},
    file_intake::SelectedFile,
};

type Events = std::pin::Pin<Box<dyn futures_util::Stream<Item = Event> + Send>>;
const PUBLIC_TEXT: &str = "PUBLIC native file prompt; reject before any model step.";
const GENERIC_FILE_REFUSAL: &str =
    "File receipts require exact private worker draft authority; prompt not sent";
const CASES: [(u64, &str, u64); 3] = [
    (1, "PUBLIC owned file.bin", 5003),
    (2, "PUBLIC empty.bin", 0),
    (3, "PUBLIC ceiling.bin", 4 * 1024 * 1024),
];

fn private_write(path: &Path, bytes: &[u8]) -> Result<(), &'static str> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| "fixture-file-create")?;
    file.write_all(bytes).map_err(|_| "fixture-file-write")
}
fn stage_profile(options: &RustBackendOptions) -> Result<PathBuf, &'static str> {
    // This executable is an isolated qualifier, not a general profile editor.
    let home = options
        .native_home
        .canonicalize()
        .map_err(|_| "fixture-home-invalid")?;
    let cwd = options
        .working_directory
        .canonicalize()
        .map_err(|_| "fixture-cwd-invalid")?;
    let user = options
        .user_home
        .canonicalize()
        .map_err(|_| "fixture-user-invalid")?;
    let output = home.parent().ok_or("fixture-output-invalid")?;
    if home.file_name().is_none_or(|name| name != "harness")
        || cwd.file_name().is_none_or(|name| name != "workspace")
        || user.file_name().is_none_or(|name| name != "user")
        || cwd.parent() != Some(output)
        || user.parent() != Some(output)
    {
        return Err("isolated-owned-runner-layout-required");
    }
    let fixture = output.join("fixture");
    let profile = home.join("profiles/desktop");
    for path in [
        &fixture,
        &fixture.join("control"),
        &home.join("profiles"),
        &profile,
    ] {
        fs::create_dir(path).map_err(|_| "fixture-directory-create")?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| "fixture-directory-private")?;
    }
    let module = fixture.join("file-prompt-host.mjs");
    private_write(
        &module,
        include_bytes!("support/file_prompt_host_fixture.mjs"),
    )?;
    let plugin = json!(module).to_string();
    let runtime = json!(options.runtime).to_string();
    let workspace = json!(options.working_directory).to_string();
    let patch = format!(
        "- insert:\n    - id: native-file-prompt-host-fixture\n      name: {plugin}\n      config:\n        runtime: {runtime}\n        workspace: {workspace}\n"
    );
    private_write(&profile.join("cordis.patch.yml"), patch.as_bytes())?;
    Ok(fixture.join("control"))
}
fn private_json(path: &Path) -> Option<Value> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file()
        || meta.file_type().is_symlink()
        || meta.permissions().mode() & 0o7777 != 0o600
        || meta.len() > 16384
    {
        return None;
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(0o400000 | 0o4000 | 0o2000000) // Linux NOFOLLOW | NONBLOCK | CLOEXEC.
        .open(path)
        .ok()?;
    let opened = file.metadata().ok()?;
    if !opened.is_file() || opened.len() > 16384 || opened.permissions().mode() & 0o7777 != 0o600 {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(16385).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 16384 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}
async fn next(feed: &mut Events) -> Result<Event, &'static str> {
    tokio::time::timeout(Duration::from_secs(10), feed.next())
        .await
        .map_err(|_| "worker-event-timeout")?
        .ok_or("worker-event-closed")
}
fn observe(event: &Event, report: &mut Value) -> Result<(), &'static str> {
    if let Event::Selected {
        frame: SessionFollowFrame::Event { event },
        ..
    } = event
    {
        if [
            "step/start",
            "step/end",
            "request/header",
            "tool/call",
            "tool/result",
            "assistant/message",
            "user/message",
        ]
        .contains(&event.event_type.as_str())
        {
            report["unexpectedWork"] = json!(report["unexpectedWork"].as_u64().unwrap() + 1);
            return Err("unexpected-admitted-work");
        }
    }
    if matches!(
        event,
        Event::Fault(_) | Event::FollowError { .. } | Event::NoBackendStarted
    ) {
        return Err("native-worker-fault");
    }
    Ok(())
}
async fn receipt(
    control: &Path,
    name: &str,
    feed: &mut Events,
    report: &mut Value,
) -> Result<Value, &'static str> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if fs::symlink_metadata(control.join("failed.json")).is_ok() {
            return Err("host-fixture-failed");
        }
        if let Some(value) = private_json(&control.join(name)) {
            return Ok(value);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("host-receipt-timeout");
        }
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => return Err("host-receipt-timeout"),
            _ = tokio::time::sleep(Duration::from_millis(25)) => {},
            event = feed.next() => observe(&event.ok_or("worker-event-closed")?, report)?,
        }
    }
}
fn request(
    session: &SessionId,
    serial: u64,
    prefix: &str,
    text: bool,
) -> Result<SessionPromptRequest, &'static str> {
    Ok(SessionPromptRequest {
        request_id: SessionRequestId::new(format!("{prefix}_{serial}"))
            .map_err(|_| "fixture-request-invalid")?,
        session_id: session.clone(),
        mode: PromptMode::Queue,
        content: if text {
            vec![PromptContentPart::Text {
                text: PUBLIC_TEXT.into(),
            }]
        } else {
            vec![]
        },
        client_time_zone: Some("UTC".into()),
    })
}
fn valid_counts(value: &Value, count: u64) -> bool {
    [
        "turnStart",
        "turnEnd",
        "blockedTurns",
        "inboxInserted",
        "inboxClaims",
        "preSteps",
    ]
    .iter()
    .all(|key| value[key].as_u64() == Some(count))
        && [
            "steps",
            "stepEnds",
            "requestHeaders",
            "toolCalls",
            "toolResults",
            "assistantMessages",
            "userMessages",
        ]
        .iter()
        .all(|key| value[key].as_u64() == Some(0))
}
async fn file_prompt_result(
    feed: &mut Events,
    report: &mut Value,
    ticket: &Ticket,
    id: &SessionRequestId,
    expected: PromptOutcome,
) -> Result<(), &'static str> {
    loop {
        let event = next(feed).await?;
        observe(&event, report)?;
        if let Event::FilePrompt {
            ticket: returned,
            request_id,
            outcome,
        } = event
        {
            if returned != *ticket || request_id != *id || outcome != expected {
                return Err("file-prompt-result-invalid");
            }
            return Ok(());
        }
    }
}
async fn exercise(
    handle: &worker::Handle,
    feed: &mut Events,
    cwd: &Path,
    control: &Path,
    report: &mut Value,
) -> Result<(), &'static str> {
    loop {
        let event = next(feed).await?;
        observe(&event, report)?;
        if let Event::Ready(version) = event {
            if version != "0.2.1-alpha.1" {
                return Err("runtime-version-invalid");
            }
            report["version"] = json!(version);
            break;
        }
    }
    let ready = receipt(control, "ready.json", feed, report).await?;
    if ready["rootRegistered"] != true
        || ready["headerVersion"].as_u64() != Some(4)
        || ready["ordinary"] != true
    {
        return Err("owned-root-not-ready");
    }
    let session = SessionId::new(
        ready["actualAgentId"]
            .as_str()
            .ok_or("fixture-session-missing")?,
    )
    .map_err(|_| "fixture-session-invalid")?;
    report["rootRegistered"] = json!(true);
    handle
        .send(Command::Select {
            generation: 1,
            address: SessionAddress::Session {
                session_id: session.clone(),
            },
        })
        .map_err(|_| "select-not-queued")?;
    loop {
        let event = next(feed).await?;
        observe(&event, report)?;
        if let Event::Selected {
            generation: 1,
            frame: SessionFollowFrame::Snapshot {
                header, records, ..
            },
        } = event
        {
            if header.version != 4 || header.id != session || header.origin.is_some() {
                return Err("snapshot-target-invalid");
            }
            let kinds: Vec<_> = records
                .iter()
                .map(|record| {
                    let HistoryRecord::Event { event } = record;
                    event.event_type.clone()
                })
                .collect();
            if kinds != ["permission/preset", "sandbox/mode", "approval/policy"] {
                return Err("initial-snapshot-not-policy-only");
            }
            report["snapshots"]
                .as_array_mut()
                .unwrap()
                .push(json!(kinds));
            report["ordinaryHeaderValidated"] = json!(true);
            break;
        }
    }
    for (serial, name, bytes) in CASES {
        let ticket = Ticket {
            epoch: worker::TRANSPORT_EPOCH,
            generation: 1,
            serial,
            target: session.clone(),
        };
        handle
            .send(Command::StageFile(Submission {
                ticket: ticket.clone(),
                source: SelectedFile::new(cwd.join(name))
                    .map_err(|_| "invalid-fixture-selection")?,
            }))
            .map_err(|_| "stage-not-queued")?;
        loop {
            let event = next(feed).await?;
            observe(&event, report)?;
            if let Event::FileStaged {
                ticket: returned,
                outcome,
            } = event
            {
                if returned != ticket {
                    return Err("wrong-file-owner");
                }
                match outcome {
                    Outcome::Staged(metadata)
                        if metadata.name == name && metadata.bytes == bytes =>
                    {
                        report["files"]
                            .as_array_mut()
                            .unwrap()
                            .push(json!({"name":name,"bytes":bytes}));
                    }
                    _ => return Err("file-not-staged"),
                }
                break;
            }
        }
        // A foreign discard cannot erase the actual private draft's authority.
        handle
            .send(Command::DiscardFile(Ticket {
                serial: serial + 100,
                ..ticket.clone()
            }))
            .map_err(|_| "foreign-discard-not-queued")?;
        let mut forged = request(&session, serial, "PUBLIC_native_file_forged", false)?;
        forged.content.push(PromptContentPart::File {
            receipt_id: FileUploadReceiptId::new("PUBLIC_forged_receipt")
                .map_err(|_| "forged-fixture-id-invalid")?,
        });
        handle
            .send(Command::Prompt {
                generation: 1,
                request: forged,
            })
            .map_err(|_| "forged-prompt-not-queued")?;
        loop {
            let event = next(feed).await?;
            observe(&event, report)?;
            if let Event::Prompt { generation, result } = event {
                if generation != 1
                    || !matches!(result, Err(ref error) if error == GENERIC_FILE_REFUSAL)
                {
                    return Err("generic-file-prompt-not-refused");
                }
                report["genericFileRefusals"] =
                    json!(report["genericFileRefusals"].as_u64().unwrap() + 1);
                break;
            }
        }
        let body = request(&session, serial, "PUBLIC_native_file_prompt", serial != 2)?;
        let id = body.request_id.clone();
        handle
            .send(Command::PromptFile {
                ticket: ticket.clone(),
                request: body,
            })
            .map_err(|_| "file-prompt-not-queued")?;
        file_prompt_result(feed, report, &ticket, &id, PromptOutcome::Accepted).await?;
        report["acceptedPrompts"] = json!(report["acceptedPrompts"].as_u64().unwrap() + 1);
        let phase = receipt(control, &format!("phase-{serial}.json"), feed, report).await?;
        if phase["phase"].as_u64() != Some(serial)
            || phase["requestId"].as_str() != Some(id.as_str())
            || phase["file"]["name"] != name
            || phase["file"]["bytes"].as_u64() != Some(bytes)
            || !phase["file"]["attachmentId"]
                .as_str()
                .is_some_and(|id| id.starts_with("sha256:") && id.len() == 71)
            || phase["rootRegistered"] != true
            || phase["durableInboxFileMatched"] != true
            || phase["claimedFileMatched"] != true
            || phase["blockedTurn"] != true
            || phase["zeroAdmittedModelSteps"] != true
            || !valid_counts(&phase["counts"], serial)
        {
            return Err("host-phase-invalid");
        }
        report["phases"].as_array_mut().unwrap().push(phase);
        let replay = request(&session, serial, "PUBLIC_native_file_replay", serial != 2)?;
        let replay_id = replay.request_id.clone();
        handle
            .send(Command::PromptFile {
                ticket: ticket.clone(),
                request: replay,
            })
            .map_err(|_| "replay-not-queued")?;
        file_prompt_result(feed, report, &ticket, &replay_id, PromptOutcome::NotSent).await?;
        report["replayRefusals"] = json!(report["replayRefusals"].as_u64().unwrap() + 1);
    }
    private_write(&control.join("go-close"), b"PUBLIC")?;
    let complete = receipt(control, "complete.json", feed, report).await?;
    if complete["status"] != "passed"
        || complete["rootDisposed"] != true
        || complete["turnClosed"] != true
        || complete["zeroAdmittedModelSteps"] != true
        || !valid_counts(&complete["counts"], 3)
    {
        return Err("host-completion-invalid");
    }
    report["host"] = complete;
    Ok(())
}
fn main() {
    let options = options::parse().expect("explicit isolated qualification options");
    let control = stage_profile(&options).expect("fixed isolated profile fixture staging");
    let cwd = options.working_directory.clone();
    let (handle, feed, owner) = worker::start(options, false).expect("native worker start");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut report = json!({"status":"failed","version":null,"files":[],"phases":[],"host":null,
        "acceptedPrompts":0,"replayRefusals":0,"genericFileRefusals":0,"snapshots":[],
        "rootRegistered":false,"ordinaryHeaderValidated":false,"unexpectedWork":0,"stop":null,
        "explicitPrompts":3,"explicitModelPrompts":0,"modelCatalogRequested":false,
        "browserSignInRequested":false,"realCredentialsUsed":false,"processWideNetworkTrace":false,
        "uiExercised":false,"nativeWorkerExercised":true,"historyDrivenReceiptRetirementQualified":false,
        "scope":"actual native Worker file prompt ACK/durable inbox/pre-step rejection and local one-attempt draft authority; not user-message history or model/provider parity"});
    let result = runtime.block_on(async {
        let mut events = worker::stream(&feed);
        let result = tokio::time::timeout(Duration::from_secs(25), exercise(&handle, &mut events, &cwd, &control, &mut report))
            .await.map_err(|_| "fixture-overall-timeout").and_then(|result| result);
        let _ = handle.send(Command::Shutdown);
        loop {
            match next(&mut events).await {
                Ok(Event::Stopped(Ok(stop))) => {
                    report["stop"] = json!({"exited":stop.exited,"graceful":stop.graceful,"exitCode":stop.exit_code,
                        "containmentUnknown":stop.containment_unknown,"observedDescendantsRemaining":stop.observed_descendants_remaining});
                    break;
                }
                Ok(Event::Stopped(Err(_))) | Err(_) => break,
                Ok(event) => { let _ = observe(&event, &mut report); }
            }
        }
        result
    });
    drop(owner);
    if result.is_ok()
        && report["acceptedPrompts"] == 3
        && report["replayRefusals"] == 3
        && report["genericFileRefusals"] == 3
        && report["unexpectedWork"] == 0
        && report["stop"]["exited"] == true
        && report["stop"]["graceful"] == true
        && report["stop"]["exitCode"] == 0
        && report["stop"]["containmentUnknown"] == false
        && report["stop"]["observedDescendantsRemaining"] == 0
    {
        report["status"] = json!("passed");
    }
    if let Err(code) = result {
        report["refusal"] = json!(code);
    }
    println!("{report}");
    if report["status"] != "passed" {
        std::process::exit(1);
    }
}
