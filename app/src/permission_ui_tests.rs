//! PUBLIC App-state permission-chip tests: no worker, Host, model or write.
//!
//! They pin the contract the reducer test cannot: the chip is drawn only from a
//! recorded `permission/preset`, a pick submits the reference's command line
//! (`/permission <preset>`), and full access passes through the acknowledgement
//! guard before anything is sent.
use super::*;
use serde_json::{Value, json};
use tokio::sync::mpsc::Receiver;

fn fixture() -> (App, Receiver<Command>) {
    let (handle, feed, receiver, _, _) = worker::test_channels();
    let options = crate::config::parse(
        [
            "--runtime",
            "/PUBLIC-never-start/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/PUBLIC-permission-native-home",
            "--user-home",
            "/PUBLIC-permission-user-home",
            "--cwd",
            "/tmp",
        ]
        .map(str::to_owned),
    )
    .unwrap()
    .unwrap();
    let (mut app, task) = App::boot(options, handle, feed);
    drop(task);
    app.ready = true;
    app.root_ready = true;
    app.roster_pending = false;
    app.transport_epoch = worker::TRANSPORT_EPOCH;
    app.management.baseline(vec![], vec![]);
    app.selected = Some(serde_json::from_value(json!("PUBLIC-permission-session")).unwrap());
    (app, receiver)
}

fn event(seq: u64, kind: &str, data: Value) -> Value {
    json!({"type":"event","event":{"type":kind,"seq":seq,"time":1000+seq,"data":data}})
}

fn user(seq: u64, text: &str) -> Value {
    let mut value = event(
        seq,
        "user/message",
        json!({"role":"user","id":"PUBLIC-permission-message","source":{"kind":"user"},
            "content":[{"type":"text","text":text}]}),
    );
    value["event"]["surfaceOp"] = json!("append");
    value
}

fn deliver(app: &mut App, records: Vec<Value>) {
    let cursor = records
        .last()
        .and_then(|value| value["event"]["seq"].as_i64())
        .unwrap_or(-1);
    let frame = json!({
        "type":"snapshot", "records":records, "cursor":cursor, "hasMore":false,
        "header":{"version":4,"id":"PUBLIC-permission-session","createdAt":1000,"isSeeded":false},
        "projections":{"asOfSeq":cursor,"values":{}}, "assistantStream":{"revision":0}
    });
    let generation = app.generation;
    drop(app.update(Message::Worker(Event::Selected {
        generation,
        frame: serde_json::from_value(frame).unwrap(),
    })));
}

/// The command line one queued command submits, if it submits one.
fn permit_line(command: &Command) -> Option<&str> {
    match command {
        Command::CommandsExecute { line, .. } => Some(line.as_str()),
        _ => None,
    }
}

#[test]
fn the_chip_needs_a_recorded_preset_and_its_menu_reads_the_catalog() {
    let (mut app, mut receiver) = fixture();
    deliver(
        &mut app,
        vec![user(0, "PUBLIC prompt without a preset record")],
    );
    assert!(
        app.permission_chip().is_none(),
        "a session that recorded no preset has nothing to name"
    );

    deliver(
        &mut app,
        vec![
            event(0, "permission/preset", json!({"preset":"workspace-write"})),
            user(1, "PUBLIC prompt beside the chip"),
        ],
    );
    assert_eq!(app.transcript.permission_preset(), Some("workspace-write"));
    assert!(
        app.permission_chip().is_some(),
        "the recorded preset is what the chip draws"
    );

    // Opening the picker is the reference's `load()` point: one catalog read,
    // never a second while the reply is held.
    drop(app.update(Message::TogglePermissionMenu));
    assert!(app.permission_menu);
    assert!(matches!(
        receiver.try_recv(),
        Ok(Command::PermissionCatalogRead)
    ));
    drop(app.update(Message::TogglePermissionMenu));
    assert!(!app.permission_menu);
    // A failed read is a status line and leaves the catalog unheld, so the next
    // open asks again rather than showing the design set as the deployment's own.
    drop(app.update(Message::Worker(Event::PermissionCatalog(Err(
        "PUBLIC catalog unavailable".into(),
    )))));
    assert!(app.permission_catalog.is_none());
    assert!(app.status.contains("PUBLIC catalog unavailable"));
    drop(app.update(Message::TogglePermissionMenu));
    assert!(matches!(
        receiver.try_recv(),
        Ok(Command::PermissionCatalogRead)
    ));
    // Once the reply is held, reopening reads nothing.
    drop(app.update(Message::Worker(Event::PermissionCatalog(Ok(
        dsh_native_core::PermissionCatalog {
            options: vec![dsh_native_core::PresetOption {
                value: "workspace-write".to_owned(),
                name: "Workspace write".to_owned(),
                description: None,
            }],
            default_options: Vec::new(),
            default_preset: String::new(),
        },
    )))));
    drop(app.update(Message::TogglePermissionMenu));
    drop(app.update(Message::TogglePermissionMenu));
    assert!(
        receiver.try_recv().is_err(),
        "a held catalog is not re-read on the next open"
    );
}

#[test]
fn a_pick_submits_the_command_line_and_full_access_asks_first() {
    let (mut app, mut receiver) = fixture();
    deliver(
        &mut app,
        vec![
            event(0, "permission/preset", json!({"preset":"workspace-write"})),
            user(1, "PUBLIC prompt beside the chip"),
        ],
    );

    drop(app.update(Message::PermissionPick("workspace-write".into())));
    let command = receiver.try_recv().expect("the pick submits a command");
    assert_eq!(permit_line(&command), Some("/permission workspace-write"));
    assert!(!app.permission_menu);

    // Full access is guarded: the pick opens the confirmation and sends nothing.
    drop(app.update(Message::PermissionPick("full-access".into())));
    assert_eq!(app.permission_confirm.as_deref(), Some("full-access"));
    assert!(!app.permission_acknowledged);
    assert!(
        receiver.try_recv().is_err(),
        "the guarded pick must not reach the Host"
    );
    // An unacknowledged Enable is inert.
    drop(app.update(Message::PermissionConfirmEnable));
    assert!(receiver.try_recv().is_err());

    // The acknowledgement is what arms it.
    drop(app.update(Message::PermissionConfirmAcknowledge(true)));
    drop(app.update(Message::PermissionConfirmEnable));
    let command = receiver
        .try_recv()
        .expect("the acknowledged Enable submits");
    assert_eq!(permit_line(&command), Some("/permission full-access"));
    assert!(app.permission_confirm.is_none());

    // Cancel dismisses the guard without sending anything.
    drop(app.update(Message::PermissionPick("full-access".into())));
    drop(app.update(Message::PermissionConfirmCancel));
    assert!(app.permission_confirm.is_none());
    assert!(receiver.try_recv().is_err());
}
