//! PUBLIC App-state MCP-panel tests: no worker, Host, file write or network.
//!
//! They assert the App-level contract the unit tests cannot: opening dispatches
//! exactly one read, the reviewed Confirm is the only command that can write,
//! Cancel changes nothing, the worker's own outcomes are the only state, and a
//! peer management flight blocks the panel (and vice versa).
use super::*;
use crate::mcp::{
    Action, Kind, Loaded, Notice, ReadTicket, Revision, ServerEntry, Transport, WriteOutcome,
};
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
            "/PUBLIC-mcp-native-home",
            "--user-home",
            "/PUBLIC-mcp-user-home",
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
    // The owned transport epoch becomes live with the ready event; a zero epoch
    // is never an authority token.
    app.transport_epoch = worker::TRANSPORT_EPOCH;
    app.management.baseline(vec![], vec![]);
    (app, receiver)
}

fn stdio(name: &str, command: &str) -> ServerEntry {
    ServerEntry {
        server_name: name.to_owned(),
        transport: Transport::Stdio,
        enabled: true,
        command: command.to_owned(),
        args: vec!["--serve".to_owned()],
        env: vec![(
            "API_TOKEN".to_owned(),
            crate::mcp::secret("PUBLIC-token".to_owned()),
        )],
        cwd: String::new(),
        url: String::new(),
        headers: Vec::new(),
        tool_call_timeout_ms: 60_000,
        fail_on_startup_error: false,
    }
}

fn loaded(entries: Vec<ServerEntry>) -> Loaded {
    Loaded {
        path: "/PUBLIC-mcp-native-home/cordis.patch.yml".to_owned(),
        revision: Some(Revision {
            bytes: 10,
            hash: 20,
        }),
        entries,
        foreign: None,
        read_error: None,
        inventory: Vec::new(),
        inventory_read: true,
    }
}

/// Open through the real More-menu message and return the read it dispatched.
fn open(app: &mut App, commands: &mut Receiver<Command>) -> ReadTicket {
    let _ = app.update(Message::Mcp(Action::Open));
    match commands.try_recv() {
        Ok(Command::McpRead(ticket)) => ticket,
        _ => panic!("opening must dispatch exactly one MCP read"),
    }
}

/// Deliver one read answer through the real worker event route.
fn deliver(app: &mut App, ticket: ReadTicket, answer: Loaded) {
    let _ = app.update(Message::Worker(Event::McpLoaded {
        ticket,
        loaded: answer,
    }));
}

fn reviewed_write(app: &mut App, commands: &mut Receiver<Command>) -> crate::mcp::Submission {
    let _ = app.update(Message::Mcp(Action::Review(app.mcp.token())));
    assert_eq!(app.mcp.mode(), "review");
    let _ = app.update(Message::Mcp(Action::Confirm(app.mcp.token())));
    assert!(app.mcp.pending());
    match commands.try_recv() {
        Ok(Command::McpWrite(submission)) => submission,
        _ => panic!("Confirm must dispatch exactly one MCP write"),
    }
}

#[test]
fn the_more_menu_opens_the_panel_and_reads_once_without_writing() {
    let (mut app, mut commands) = fixture();
    let ticket = open(&mut app, &mut commands);
    assert!(app.mcp.is_open());
    assert_eq!(app.mcp.mode(), "list");
    assert!(!app.conversation_menu);
    // Opening dispatched a read and nothing else.
    assert!(commands.try_recv().is_err());
    deliver(
        &mut app,
        ticket,
        loaded(vec![stdio("alpha", "/usr/bin/npx")]),
    );
    assert_eq!(app.mcp.notice(), Notice::Loaded);
    assert_eq!(app.mcp.entries().len(), 1);
    // Receiving a snapshot never produces a command.
    assert!(commands.try_recv().is_err());

    // Refresh is a read, never a write.
    let _ = app.update(Message::Mcp(Action::Refresh(app.mcp.read_ticket())));
    match commands.try_recv() {
        Ok(Command::McpRead(next)) => assert_ne!(next.serial, ticket.serial),
        _ => panic!("refresh must dispatch a read"),
    }

    // A second opening press while the panel is open writes nothing.
    let _ = app.update(Message::Mcp(Action::Open));
    assert!(app.mcp.is_open());
}

#[test]
fn escape_dismisses_and_a_stale_dismiss_cannot_close_a_newer_panel() {
    let (mut app, mut commands) = fixture();
    let old = open(&mut app, &mut commands);
    deliver(&mut app, old, loaded(vec![]));
    let _ = app.update(Message::DismissPanel);
    assert!(!app.mcp.is_open());

    // Reopening mints a new read identity; the previous ticket is inert.
    let _ = open(&mut app, &mut commands);
    let _ = app.update(Message::Mcp(Action::Dismiss(old)));
    assert!(app.mcp.is_open());
    let _ = app.update(Message::Mcp(Action::Dismiss(app.mcp.read_ticket())));
    assert!(!app.mcp.is_open());
    // Dismissal never dispatched a command.
    assert!(commands.try_recv().is_err());
}

#[test]
fn opening_mcp_replaces_the_work_records_panel_and_a_peer_panel_dismisses_mcp() {
    let (mut app, mut commands) = fixture();
    let _ = app.update(Message::Work(work_panel_ui::Action::Open));
    assert!(app.work_open);
    let _ = open(&mut app, &mut commands);
    assert!(app.mcp.is_open());
    assert!(!app.work_open, "opening MCP replaces the other panel");

    // Closing MCP and opening the work panel again leaves MCP closed.
    let _ = app.update(Message::Mcp(Action::Dismiss(app.mcp.read_ticket())));
    let _ = app.update(Message::Work(work_panel_ui::Action::Open));
    assert!(app.work_open);
    assert!(!app.mcp.is_open());
}

#[test]
fn add_requires_a_reviewed_confirm_and_cancel_changes_nothing() {
    let (mut app, mut commands) = fixture();
    let ticket = open(&mut app, &mut commands);
    deliver(&mut app, ticket, loaded(Vec::new()));

    let _ = app.update(Message::Mcp(Action::BeginAdd(app.mcp.read_ticket())));
    assert_eq!(app.mcp.mode(), "form");
    for (field, value) in [
        (crate::mcp::Field::ServerName, "alpha"),
        (crate::mcp::Field::Command, "/usr/bin/npx"),
    ] {
        let _ = app.update(Message::Mcp(Action::Field {
            token: app.mcp.token(),
            field,
            value: value.to_owned(),
        }));
    }
    // No command exists before Review and Confirm.
    assert!(commands.try_recv().is_err());
    let _ = app.update(Message::Mcp(Action::Confirm(app.mcp.token())));
    assert!(!app.mcp.pending());
    assert!(commands.try_recv().is_err());

    // Cancel discards the draft and leaves the loaded document untouched.
    let entries = app.mcp.entries().to_vec();
    let _ = app.update(Message::Mcp(Action::Cancel(app.mcp.token())));
    assert_eq!(app.mcp.mode(), "list");
    assert_eq!(app.mcp.entries(), entries.as_slice());
    assert!(app.mcp.draft().is_none());
    assert!(commands.try_recv().is_err());

    // The reviewed path does write, and it writes the whole document.
    let _ = app.update(Message::Mcp(Action::BeginAdd(app.mcp.read_ticket())));
    for (field, value) in [
        (crate::mcp::Field::ServerName, "alpha"),
        (crate::mcp::Field::Command, "/usr/bin/npx"),
    ] {
        let _ = app.update(Message::Mcp(Action::Field {
            token: app.mcp.token(),
            field,
            value: value.to_owned(),
        }));
    }
    let submission = reviewed_write(&mut app, &mut commands);
    assert_eq!(submission.kind, Kind::Add);
    assert_eq!(
        submission.ticket.expected,
        Some(Revision {
            bytes: 10,
            hash: 20
        })
    );
    assert_eq!(submission.entries.len(), 1);
    assert_eq!(submission.entries[0].server_name, "alpha");
    // No optimistic state: the acknowledged document is not installed yet.
    assert!(app.mcp.entries().is_empty());
}

#[test]
fn edit_restates_every_field_and_remove_disables_rather_than_deletes() {
    let (mut app, mut commands) = fixture();
    let ticket = open(&mut app, &mut commands);
    let other = stdio("beta", "/usr/bin/other");
    deliver(
        &mut app,
        ticket,
        loaded(vec![stdio("alpha", "/usr/bin/npx"), other.clone()]),
    );

    let _ = app.update(Message::Mcp(Action::BeginEdit {
        ticket: app.mcp.read_ticket(),
        index: 0,
    }));
    let _ = app.update(Message::Mcp(Action::Field {
        token: app.mcp.token(),
        field: crate::mcp::Field::Command,
        value: "/usr/bin/third".to_owned(),
    }));
    let submission = reviewed_write(&mut app, &mut commands);
    assert_eq!(submission.kind, Kind::Edit { index: 0 });
    assert_eq!(submission.entries.len(), 2);
    let edited = &submission.entries[0];
    // Every field is restated, including the loaded secret and the other
    // transport-independent fields, and the peer row is untouched.
    assert_eq!(edited.server_name, "alpha");
    assert_eq!(edited.command, "/usr/bin/third");
    assert_eq!(edited.args, vec!["--serve".to_owned()]);
    assert_eq!(
        edited.env,
        vec![(
            "API_TOKEN".to_owned(),
            crate::mcp::secret("PUBLIC-token".to_owned())
        )]
    );
    assert_eq!(edited.tool_call_timeout_ms, 60_000);
    assert_eq!(submission.entries[1], other);

    // Remove is a disable: the entry and its configuration stay.
    let _ = app.update(Message::Worker(Event::McpWritten {
        submission: submission.clone(),
        outcome: WriteOutcome::Written {
            revision: Revision {
                bytes: 40,
                hash: 40,
            },
            backup: Some("cordis.patch.yml.backup-1".to_owned()),
            bytes: 40,
        },
    }));
    assert_eq!(app.mcp.notice(), Notice::Confirmed);
    assert_eq!(app.mcp.entries().len(), 2);
    let _ = app.update(Message::Mcp(Action::BeginDisable {
        ticket: app.mcp.read_ticket(),
        index: 0,
    }));
    assert_eq!(app.mcp.mode(), "review");
    let submission = reviewed_write(&mut app, &mut commands);
    assert_eq!(submission.kind, Kind::Disable { index: 0 });
    assert_eq!(submission.entries.len(), 2, "nothing is deleted");
    assert!(!submission.entries[0].enabled);
    assert_eq!(submission.entries[0].command, "/usr/bin/third");
    assert_eq!(submission.entries[0].env.len(), 1);
    // The confirmed write advanced the expected revision, so a second write
    // without a refresh is not a conflict.
    assert_eq!(
        submission.ticket.expected,
        Some(Revision {
            bytes: 40,
            hash: 40
        })
    );
}

#[test]
fn an_indeterminate_outcome_is_reported_and_never_retried() {
    let (mut app, mut commands) = fixture();
    let ticket = open(&mut app, &mut commands);
    deliver(
        &mut app,
        ticket,
        loaded(vec![stdio("alpha", "/usr/bin/npx")]),
    );
    let _ = app.update(Message::Mcp(Action::BeginDisable {
        ticket: app.mcp.read_ticket(),
        index: 0,
    }));
    let submission = reviewed_write(&mut app, &mut commands);
    assert!(app.mcp.pending());

    let _ = app.update(Message::Worker(Event::McpWritten {
        submission: submission.clone(),
        outcome: WriteOutcome::Indeterminate,
    }));
    assert_eq!(app.mcp.notice(), Notice::Indeterminate);
    assert!(!app.mcp.pending());
    assert!(app.mcp.report().is_none());
    // No retry, and no command was produced by the outcome.
    assert!(commands.try_recv().is_err());

    // A duplicate late receipt for the settled ticket changes nothing.
    let _ = app.update(Message::Worker(Event::McpWritten {
        submission,
        outcome: WriteOutcome::Written {
            revision: Revision { bytes: 1, hash: 1 },
            backup: None,
            bytes: 1,
        },
    }));
    assert_eq!(app.mcp.notice(), Notice::Indeterminate);
    assert!(app.mcp.report().is_none());
    assert!(commands.try_recv().is_err());

    // A queued write refusal is reported as not sent rather than left pending.
    let _ = app.update(Message::Mcp(Action::BeginDisable {
        ticket: app.mcp.read_ticket(),
        index: 0,
    }));
    let submission = reviewed_write(&mut app, &mut commands);
    let _ = app.update(Message::Worker(Event::McpWritten {
        submission,
        outcome: WriteOutcome::NotSent,
    }));
    assert_eq!(app.mcp.notice(), Notice::NotSent);
    assert!(!app.mcp.pending());
}

#[test]
fn an_unrecognised_file_makes_the_panel_read_only() {
    let (mut app, mut commands) = fixture();
    let ticket = open(&mut app, &mut commands);
    deliver(
        &mut app,
        ticket,
        Loaded {
            path: "/PUBLIC-mcp-native-home/cordis.patch.yml".to_owned(),
            revision: Some(Revision { bytes: 4, hash: 4 }),
            entries: Vec::new(),
            foreign: Some("line 1 is not a patch entry this app can preserve: - id: x".to_owned()),
            read_error: None,
            inventory: Vec::new(),
            inventory_read: true,
        },
    );
    assert_eq!(app.mcp.notice(), Notice::Foreign);
    assert!(app.mcp.foreign_reason().is_some());
    assert!(app.mcp.defect().is_none());
    let _ = app.update(Message::Mcp(Action::BeginAdd(app.mcp.read_ticket())));
    assert_eq!(
        app.mcp.mode(),
        "list",
        "no draft may open over unreadable content"
    );
    assert!(commands.try_recv().is_err());
}

#[test]
fn a_named_defect_is_reported_when_a_review_is_refused() {
    let (mut app, mut commands) = fixture();
    let ticket = open(&mut app, &mut commands);
    deliver(&mut app, ticket, loaded(Vec::new()));
    let _ = app.update(Message::Mcp(Action::BeginAdd(app.mcp.read_ticket())));
    // A stdio row with no command cannot be reviewed.
    let _ = app.update(Message::Mcp(Action::Field {
        token: app.mcp.token(),
        field: crate::mcp::Field::ServerName,
        value: "alpha".to_owned(),
    }));
    let _ = app.update(Message::Mcp(Action::Review(app.mcp.token())));
    assert_eq!(app.mcp.notice(), Notice::Invalid);
    assert_eq!(app.mcp.mode(), "form");
    assert_eq!(
        app.mcp.defect().map(|defect| defect.text()),
        Some(crate::mcp::Defect::Command.text())
    );
    assert!(commands.try_recv().is_err());

    // The named defect clears once the draft is discarded.
    let _ = app.update(Message::Mcp(Action::Cancel(app.mcp.token())));
    assert!(app.mcp.defect().is_none());
}

#[test]
fn a_peer_management_flight_blocks_the_panel_and_an_admitted_mcp_write_blocks_the_peer() {
    let (mut app, mut commands) = fixture();
    // A live session is what the session-management context needs.
    let id = SessionId::new("PUBLIC-mcp-session").unwrap();
    app.sessions.insert(
        id.clone(),
        SessionSummary {
            agent_available: true,
            session_id: id.clone(),
            updated_at: 1000,
            running: false,
            blank: true,
            parent_session_id: None,
            origin: None,
            cwd: None,
            projections: None,
        },
    );
    app.selected = Some(id);
    app.generation = 7;

    let ticket = open(&mut app, &mut commands);
    deliver(
        &mut app,
        ticket,
        loaded(vec![stdio("alpha", "/usr/bin/npx")]),
    );
    // No peer flight yet: the panel may mutate.
    assert!(app.mcp_context().enabled);
    // Close it so the peer's own messages are not fenced by the open panel.
    let _ = app.update(Message::Mcp(Action::Dismiss(app.mcp.read_ticket())));
    assert!(!app.mcp.is_open());

    // Admit a registry mutation through the real messages.
    let view = app.management_context().view.expect("selected session");
    let _ = app.update(Message::Management(crate::management::Action::Open(view)));
    let review = app.management.ticket().expect("management ticket");
    let _ = app.update(Message::Management(crate::management::Action::Review {
        ticket: review,
        operation: crate::management::Operation::Pin,
    }));
    let confirm = app.management.ticket().expect("management ticket");
    let _ = app.update(Message::Management(crate::management::Action::Confirm(
        confirm,
    )));
    assert!(app.management.pending(), "the peer flight is admitted");
    let settled = match commands.try_recv() {
        Ok(Command::Manage(submission)) => submission,
        _ => panic!("Confirm must dispatch exactly one management write"),
    };
    // Hiding the peer panel neither cancels nor rolls back its admitted write.
    let close = app.management.ticket().expect("management ticket");
    let _ = app.update(Message::Management(crate::management::Action::Close(close)));
    assert!(!app.management.is_open());
    assert!(app.management.pending());

    // Opening the panel during the peer flight is allowed; mutating is not.
    let ticket = open(&mut app, &mut commands);
    deliver(
        &mut app,
        ticket,
        loaded(vec![stdio("alpha", "/usr/bin/npx")]),
    );
    assert!(!app.mcp_context().enabled);
    let _ = app.update(Message::Mcp(Action::BeginAdd(app.mcp.read_ticket())));
    assert_eq!(app.mcp.mode(), "list");
    assert!(commands.try_recv().is_err());

    // Settle the peer, then admit an MCP write and check the reverse direction.
    let _ = app.update(Message::Worker(Event::Managed {
        submission: settled,
        result: Ok(()),
    }));
    assert!(!app.management.pending());
    assert!(app.mcp_context().enabled);
    // Positive control: with no write in flight the export flight is admitted, so
    // the refusal below is the MCP flight and not a disabled preceding condition.
    assert!(app.export_context_for(app.selected.as_ref()).allowed);
    let _ = app.update(Message::Mcp(Action::BeginDisable {
        ticket: app.mcp.read_ticket(),
        index: 0,
    }));
    let _ = reviewed_write(&mut app, &mut commands);
    assert!(app.mcp.pending());
    assert!(
        !app.management_context().allowed,
        "an admitted MCP write must block the session-management flight"
    );
    assert!(!app.rename_context().allowed);
    assert!(!app.fork_context().allowed);
    assert!(!app.terminal_context_for(app.selected.as_ref()).allowed);
    assert!(
        !app.export_context_for(app.selected.as_ref()).allowed,
        "an admitted MCP write must block export, as worker::ExportAdmission already does"
    );
}

#[test]
fn the_menu_count_survives_the_panel_closing_and_a_late_write_says_restart() {
    let (mut app, mut commands) = fixture();
    let ticket = open(&mut app, &mut commands);
    deliver(
        &mut app,
        ticket,
        loaded(vec![stdio("alpha", "/usr/bin/npx")]),
    );
    assert_eq!(app.mcp.configured(), 1);
    assert!(!app.mcp.restart_required());
    // Back clears the loaded file; the count is what the More-menu entry draws.
    let _ = app.update(Message::Mcp(Action::Dismiss(app.mcp.read_ticket())));
    assert!(!app.mcp.is_open());
    assert_eq!(
        app.mcp.configured(),
        1,
        "the menu entry must not claim zero servers once loaded"
    );
    // A confirmed write admitted before Back still replaced the file.
    let ticket = open(&mut app, &mut commands);
    deliver(
        &mut app,
        ticket,
        Loaded {
            path: "/PUBLIC-mcp-native-home/cordis.patch.yml".to_owned(),
            revision: Some(Revision {
                bytes: 10,
                hash: 20,
            }),
            entries: vec![stdio("alpha", "/usr/bin/npx")],
            foreign: None,
            read_error: None,
            inventory: Vec::new(),
            inventory_read: true,
        },
    );
    let _ = app.update(Message::Mcp(Action::BeginDisable {
        ticket: app.mcp.read_ticket(),
        index: 0,
    }));
    let submission = reviewed_write(&mut app, &mut commands);
    let _ = app.update(Message::Mcp(Action::Dismiss(app.mcp.read_ticket())));
    assert!(!app.mcp.is_open());
    let _ = app.update(Message::Worker(Event::McpWritten {
        submission,
        outcome: WriteOutcome::Written {
            revision: Revision {
                bytes: 12,
                hash: 21,
            },
            backup: Some("PUBLIC-backup".to_owned()),
            bytes: 12,
        },
    }));
    assert!(
        app.mcp.restart_required(),
        "a replacement that lands after Back still needs a restart"
    );
    assert_eq!(app.mcp.configured(), 1);
    assert!(
        app.status.contains("restart the session Host"),
        "the outcome is reported where the user can see it: {}",
        app.status
    );
}

#[test]
fn a_refused_read_is_not_reported_as_a_corrupt_file() {
    let (mut app, mut commands) = fixture();
    let ticket = open(&mut app, &mut commands);
    assert!(
        !app.mcp.config_empty(),
        "loading is not an empty configuration"
    );
    deliver(
        &mut app,
        ticket,
        Loaded::refused("The native worker refused the MCP read before it ran."),
    );
    assert_eq!(app.mcp.notice(), Notice::ReadRefused);
    assert!(
        !app.mcp.writable(),
        "nothing was inspected, so no mutation may be offered"
    );
    assert!(
        !app.mcp.config_empty(),
        "a refused read is not an empty patch list"
    );
    assert!(app.mcp.foreign_reason().is_none());
    // A real foreign file still takes the foreign notice.
    let ticket = app.mcp.read_ticket();
    let _ = app.update(Message::Mcp(Action::Refresh(ticket)));
    let ticket = match commands.try_recv() {
        Ok(Command::McpRead(ticket)) => ticket,
        _ => panic!("refresh must read"),
    };
    deliver(
        &mut app,
        ticket,
        Loaded {
            path: "/PUBLIC-mcp-native-home/cordis.patch.yml".to_owned(),
            revision: None,
            entries: Vec::new(),
            foreign: Some("not a patch entry".to_owned()),
            read_error: None,
            inventory: Vec::new(),
            inventory_read: true,
        },
    );
    assert_eq!(app.mcp.notice(), Notice::Foreign);
    assert!(!app.mcp.config_empty());
}

#[test]
fn a_successful_empty_read_is_the_only_empty_configuration() {
    let (mut app, mut commands) = fixture();
    let ticket = open(&mut app, &mut commands);
    deliver(&mut app, ticket, loaded(Vec::new()));
    assert!(app.mcp.config_empty());
    assert_eq!(app.mcp.notice(), Notice::Loaded);
}
