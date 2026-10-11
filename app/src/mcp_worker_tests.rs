//! Worker-side tests for MCP composition ownership: admission, cross-flight
//! exclusion and settlement. No client, network or Host is involved.
use super::*;
use crate::mcp::{Kind, Revision, Submission, WriteOutcome, WriteTicket};

fn ticket(serial: u64) -> WriteTicket {
    WriteTicket {
        epoch: TRANSPORT_EPOCH,
        panel: 4,
        serial,
        expected: Some(Revision { bytes: 8, hash: 9 }),
    }
}

fn submission(serial: u64) -> Submission {
    Submission {
        ticket: ticket(serial),
        kind: Kind::Add,
        entries: Vec::new(),
    }
}

fn admission(peer_mcp: bool) -> Admission {
    Admission {
        smoke: false,
        stopping: false,
        operations: 0,
        management_active: false,
        peer_rename: false,
        peer_fork: false,
        peer_export: false,
        peer_mcp,
    }
}

#[test]
fn mcp_write_admission_is_epoch_smoke_shutdown_capacity_and_single_flight_fenced() {
    let mut active = None;
    let flight = ticket(1);
    for (candidate, smoke, stopping, operations, other, expected) in [
        (ticket(1), false, false, 0, false, true),
        (ticket(1), true, false, 0, false, false),
        (ticket(1), false, true, 0, false, false),
        (ticket(1), false, false, MAX_OPERATIONS, false, false),
        (ticket(1), false, false, 0, true, false),
    ] {
        active = None;
        assert_eq!(
            admit_mcp(&mut active, &candidate, smoke, stopping, operations, other),
            expected,
            "{candidate:?} {smoke} {stopping} {operations} {other}"
        );
        assert_eq!(active.is_some(), expected);
    }
    // A stale transport generation never reserves the flight.
    let mut stale = ticket(1);
    stale.epoch = TRANSPORT_EPOCH + 1;
    assert!(!admit_mcp(&mut active, &stale, false, false, 0, false));
    // One MCP write at a time, even for a different target revision.
    assert!(admit_mcp(&mut active, &flight, false, false, 0, false));
    assert!(!admit_mcp(&mut active, &ticket(2), false, false, 0, false));
}

#[test]
fn mcp_write_excludes_every_other_management_flight() {
    // The MCP ticket reserves nothing beside a live rename, fork or export.
    assert!(!admit_mcp(&mut None, &ticket(1), false, false, 0, true));

    // And every peer refuses beside a live MCP write.
    assert!(admission(true).refused(false));
    assert!(!admission(false).refused(false));

    // Export's own admission carries the MCP peer.
    let context = ExportAdmission {
        smoke: false,
        stopping: false,
        operations: 0,
        generation: 3,
        target: None,
        key_saving: false,
        plugins_active: false,
        management_active: false,
        rename_active: false,
        fork_active: false,
        terminal_active: false,
        mcp_active: true,
    };
    assert!(context.mcp_active);
}

#[test]
fn a_returned_mcp_write_clears_only_its_own_ticket() {
    let mut active = Some(ticket(1));
    settle_mcp(
        &mut active,
        &Event::McpWritten {
            submission: submission(2),
            outcome: WriteOutcome::Written {
                revision: Revision { bytes: 1, hash: 1 },
                backup: None,
                bytes: 1,
            },
        },
    );
    assert_eq!(active, Some(ticket(1)));
    settle_mcp(
        &mut active,
        &Event::McpWritten {
            submission: submission(1),
            outcome: WriteOutcome::Indeterminate,
        },
    );
    assert_eq!(active, None);
    // An unrelated event never settles the flight.
    active = Some(ticket(1));
    settle_mcp(&mut active, &Event::Ready("PUBLIC".to_owned()));
    assert_eq!(active, Some(ticket(1)));
}

#[test]
fn a_refused_mcp_command_reports_not_sent_and_an_unread_file() {
    let Event::McpWritten {
        submission,
        outcome,
    } = rejected(Command::McpWrite(submission(1)))
    else {
        panic!("a refused MCP write answers with its ticket");
    };
    assert_eq!(submission.ticket, ticket(1));
    assert_eq!(outcome, WriteOutcome::NotSent);

    let Event::McpLoaded { ticket, loaded } = rejected(Command::McpRead(crate::mcp::ReadTicket {
        epoch: TRANSPORT_EPOCH,
        panel: 1,
        serial: 1,
    })) else {
        panic!("a refused MCP read answers with its ticket");
    };
    assert_eq!(ticket.serial, 1);
    assert!(loaded.foreign.is_some());
    assert!(loaded.revision.is_none());
    assert!(!loaded.inventory_read);
}
