//! Fixed PUBLIC headless actual native-worker intake; no GUI, prompt/catalog, sign-in or credential save.
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
use dsh_native_transport::dto::*;
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::time::Duration;
use worker::{
    Command, Event,
    attachments::{Outcome, Submission, Ticket},
    file_intake::SelectedFile,
};

async fn next(
    feed: &mut std::pin::Pin<Box<dyn futures_util::Stream<Item = Event> + Send>>,
) -> Result<Event, &'static str> {
    tokio::time::timeout(Duration::from_secs(15), feed.next())
        .await
        .map_err(|_| "worker-event-timeout")?
        .ok_or("worker-event-closed")
}
fn observe(event: &Event, report: &mut Value) -> Result<(), &'static str> {
    if let Event::Root {
        frame: RemoteEventFrame::Emit { event, .. },
        ..
    } = event
    {
        if [
            "turn/start",
            "turn/end",
            "step/start",
            "step/end",
            "request/header",
            "tool/call",
            "tool/result",
            "assistant/message",
            "user/message",
        ]
        .contains(&event.as_str())
        {
            report["unexpectedWork"] = json!(report["unexpectedWork"].as_u64().unwrap() + 1);
        }
    }
    if let Event::Selected {
        frame: SessionFollowFrame::Snapshot { records, .. },
        ..
    } = event
    {
        let kinds: Vec<_> = records
            .iter()
            .map(|record| {
                let HistoryRecord::Event { event } = record;
                &event.event_type
            })
            .collect();
        report["snapshots"]
            .as_array_mut()
            .unwrap()
            .push(json!(kinds));
    }
    if matches!(event, Event::Fault(_) | Event::FollowError { .. }) {
        return Err("native-worker-fault");
    }
    Ok(())
}
async fn exercise(
    handle: &worker::Handle,
    feed: &mut std::pin::Pin<Box<dyn futures_util::Stream<Item = Event> + Send>>,
    cwd: &std::path::Path,
    report: &mut Value,
) -> Result<(), &'static str> {
    loop {
        let event = next(feed).await?;
        observe(&event, report)?;
        if let Event::Ready(version) = event {
            report["version"] = json!(version);
            break;
        }
    }
    handle
        .send(Command::Create {
            workspace: None,
            cwd: Some(cwd.to_str().ok_or("cwd-encoding")?.into()),
        })
        .map_err(|_| "create-not-queued")?;
    let session = loop {
        let event = next(feed).await?;
        observe(&event, report)?;
        if let Event::Created(result) = event {
            break result.map_err(|_| "create-refused")?.session_id;
        }
    };
    let early = Ticket {
        epoch: worker::TRANSPORT_EPOCH,
        generation: 1,
        serial: 999,
        target: session.clone(),
    };
    handle
        .send(Command::StageFile(Submission {
            ticket: early.clone(),
            source: SelectedFile::new(cwd.join("PUBLIC refused.bin"))
                .map_err(|_| "invalid-early-selection")?,
        }))
        .map_err(|_| "early-not-queued")?;
    handle
        .send(Command::Select {
            generation: 1,
            address: SessionAddress::Session {
                session_id: session.clone(),
            },
        })
        .map_err(|_| "select-not-queued")?;
    let mut snapshot = false;
    loop {
        let event = next(feed).await?;
        observe(&event, report)?;
        if matches!(
            &event,
            Event::Selected {
                generation: 1,
                frame: SessionFollowFrame::Snapshot { .. }
            }
        ) {
            snapshot = true;
        }
        if let Event::FileStaged { ticket, outcome } = event {
            if ticket != early || outcome != Outcome::NotSent {
                return Err("pre-snapshot-intake-admitted");
            }
            report["preSnapshotRefused"] = json!(true);
        }
        if snapshot && report["preSnapshotRefused"] == true {
            break;
        }
    }
    for (serial, name, expected) in [
        (1, "PUBLIC owned file.bin", 5003u64),
        (2, "PUBLIC empty.bin", 0),
        (3, "PUBLIC ceiling.bin", 4 * 1024 * 1024),
    ] {
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
                        if metadata.name == name && metadata.bytes == expected =>
                    {
                        report["files"]
                            .as_array_mut()
                            .unwrap()
                            .push(json!({"name":metadata.name,"bytes":metadata.bytes}));
                    }
                    _ => return Err("file-not-staged"),
                }
                break;
            }
        }
        // Foreign discard cannot release the staged slot; the matching ticket does.
        let foreign = Ticket {
            serial: serial + 100,
            ..ticket.clone()
        };
        handle
            .send(Command::DiscardFile(foreign.clone()))
            .map_err(|_| "discard-not-queued")?;
        handle
            .send(Command::StageFile(Submission {
                ticket: foreign.clone(),
                source: SelectedFile::new(cwd.join("PUBLIC refused.bin"))
                    .map_err(|_| "invalid-refused-selection")?,
            }))
            .map_err(|_| "refusal-not-queued")?;
        loop {
            let event = next(feed).await?;
            observe(&event, report)?;
            if let Event::FileStaged {
                ticket: returned,
                outcome,
            } = event
            {
                if returned != foreign || outcome != Outcome::NotSent {
                    return Err("foreign-discard-released-slot");
                }
                report["foreignDiscardRefusals"] =
                    json!(report["foreignDiscardRefusals"].as_u64().unwrap() + 1);
                break;
            }
        }
        handle
            .send(Command::DiscardFile(ticket))
            .map_err(|_| "discard-not-queued")?;
    }
    // After selection replacement, even the same Session target cannot reuse an old generation.
    handle
        .send(Command::Select {
            generation: 2,
            address: SessionAddress::Session {
                session_id: session.clone(),
            },
        })
        .map_err(|_| "reselect-not-queued")?;
    loop {
        let event = next(feed).await?;
        observe(&event, report)?;
        if matches!(event, Event::Selected { generation: 2, .. }) {
            break;
        }
    }
    let ticket = Ticket {
        epoch: worker::TRANSPORT_EPOCH,
        generation: 1,
        serial: 4,
        target: session,
    };
    handle
        .send(Command::StageFile(Submission {
            ticket: ticket.clone(),
            source: SelectedFile::new(cwd.join("PUBLIC refused.bin"))
                .map_err(|_| "invalid-refused-selection")?,
        }))
        .map_err(|_| "stale-not-queued")?;
    loop {
        let event = next(feed).await?;
        observe(&event, report)?;
        if let Event::FileStaged {
            ticket: returned,
            outcome,
        } = event
        {
            if returned != ticket || outcome != Outcome::NotSent {
                return Err("stale-selection-admitted");
            }
            report["staleSelectionRefused"] = json!(true);
            break;
        }
    }
    Ok(())
}
fn main() {
    let options = options::parse().expect("explicit isolated qualification options");
    let cwd = options.working_directory.clone();
    let (handle, feed, owner) = worker::start(options, false).expect("native worker start");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut report = json!({"status":"failed","version":null,"files":[],"foreignDiscardRefusals":0,"preSnapshotRefused":false,"staleSelectionRefused":false,"snapshots":[],"unexpectedWork":0,"stop":null,"explicitModelPrompts":0,"modelCatalogRequested":false,"processWideNetworkTrace":false,"uiExercised":false,"nativeWorkerExercised":true});
    let result=runtime.block_on(async {
        let mut events=worker::stream(&feed);
        let result=exercise(&handle,&mut events,&cwd,&mut report).await;
        let _=handle.send(Command::Shutdown);
        loop {match next(&mut events).await {Ok(Event::Stopped(stop))=>{if let Ok(stop)=stop {report["stop"]=json!({"exited":stop.exited,"graceful":stop.graceful,"exitCode":stop.exit_code,"containmentUnknown":stop.containment_unknown,"observedDescendantsRemaining":stop.observed_descendants_remaining});}break;},Ok(event)=>{let _=observe(&event,&mut report);},Err(_)=>break}}
        result
    });
    drop(owner);
    if result.is_ok()
        && report["unexpectedWork"] == 0
        && report["stop"]["graceful"] == true
        && report["stop"]["exited"] == true
        && report["stop"]["containmentUnknown"] == false
        && report["stop"]["exitCode"] == 0
    {
        report["status"] = json!("passed");
    }
    println!("{}", report);
    if report["status"] != "passed" {
        std::process::exit(1);
    }
}
