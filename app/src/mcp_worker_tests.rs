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

/// A clean MCP question, optionally with other flights already held.
fn admission(other: Flights) -> Admission {
    Admission::new(false, false, 0, other)
}
/// The flights a live MCP write holds, for the reverse assertions.
fn holding_mcp() -> Flights {
    Flights {
        mcp: true,
        ..Flights::NONE
    }
}

#[test]
fn mcp_write_admission_is_epoch_smoke_shutdown_capacity_and_single_flight_fenced() {
    let mut active = None;
    let flight = ticket(1);
    for (candidate, context, expected) in [
        (ticket(1), admission(Flights::NONE), true),
        (
            ticket(1),
            Admission::new(true, false, 0, Flights::NONE),
            false,
        ),
        (
            ticket(1),
            Admission::new(false, true, 0, Flights::NONE),
            false,
        ),
        (
            ticket(1),
            Admission::new(false, false, MAX_OPERATIONS, Flights::NONE),
            false,
        ),
        (
            ticket(1),
            admission(Flights {
                rename: true,
                ..Flights::NONE
            }),
            false,
        ),
    ] {
        active = None;
        assert_eq!(
            admit_mcp(&mut active, &candidate, &context),
            expected,
            "{candidate:?} must answer {expected}"
        );
        assert_eq!(active.is_some(), expected);
    }
    // A stale transport generation never reserves the flight.
    let mut stale = ticket(1);
    stale.epoch = TRANSPORT_EPOCH + 1;
    assert!(!admit_mcp(&mut active, &stale, &admission(Flights::NONE)));
    // One MCP write at a time, even for a different target revision.
    assert!(admit_mcp(&mut active, &flight, &admission(Flights::NONE)));
    assert!(!admit_mcp(
        &mut active,
        &ticket(2),
        &admission(Flights::NONE)
    ));
}

#[test]
fn mcp_write_excludes_every_other_management_flight() {
    // The MCP ticket reserves nothing beside any other live flight.
    for peer in [
        Flights {
            management: true,
            ..Flights::NONE
        },
        Flights {
            rename: true,
            ..Flights::NONE
        },
        Flights {
            fork: true,
            ..Flights::NONE
        },
        Flights {
            export: true,
            ..Flights::NONE
        },
        Flights {
            terminal: true,
            ..Flights::NONE
        },
    ] {
        assert!(!admit_mcp(&mut None, &ticket(1), &admission(peer)));
    }
    // And the reverse: a live MCP write blocks every admission, including
    // another MCP write, through the same table.
    assert!(admission(holding_mcp()).refused());
    assert!(!admit_mcp(&mut None, &ticket(1), &admission(holding_mcp())));
    assert!(!admission(Flights::NONE).refused());
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
