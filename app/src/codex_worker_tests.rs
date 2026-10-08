use super::*;
fn ticket(serial: u64) -> Ticket {
    Ticket {
        epoch: TRANSPORT_EPOCH,
        panel: 1,
        serial,
    }
}
fn metadata(phase: CodexPhase) -> CodexMetadata {
    CodexMetadata {
        available: true,
        credential_stored: phase == CodexPhase::Authorized,
        route_configured: false,
        phase,
        attempt: Some(11),
        prompt: None,
        browser_available: false,
        settings_revision: Some(7),
        retry_blocked: false,
    }
}
#[test]
fn admission_rejects_smoke_epoch_other_write_and_task_limit_without_reserving() {
    let mut c = CodexCoordinator::default();
    let effect = Effect::StartLogin { ticket: ticket(1) };
    assert!(!c.admit(&effect, true, false, 0, false));
    assert!(!c.admit(&effect, false, true, 0, false));
    assert!(!c.admit(&effect, false, false, super::super::MAX_OPERATIONS, false));
    assert!(!c.admit(&effect, false, false, 0, true));
    assert!(!c.admit(
        &Effect::StatusRead {
            ticket: Ticket {
                epoch: 0,
                ..ticket(1)
            }
        },
        false,
        false,
        0,
        false
    ));
    assert!(!c.busy());
    assert!(c.admit(&effect, false, false, 0, false));
    assert!(!c.admit(&effect, false, false, 0, false));
}
#[test]
fn uncertain_read_does_not_poison_but_uncertain_start_blocks_new_writes() {
    let mut c = CodexCoordinator::default();
    assert!(c.admit(
        &Effect::StatusRead { ticket: ticket(1) },
        false,
        false,
        0,
        false
    ));
    c.settle(ticket(1), Outcome::Indeterminate);
    assert!(!c.busy());
    assert!(c.admit(
        &Effect::StartLogin { ticket: ticket(2) },
        false,
        false,
        0,
        false
    ));
    c.settle(ticket(2), Outcome::Indeterminate);
    assert!(c.busy());
    assert!(!c.admit(
        &Effect::StartLogin { ticket: ticket(3) },
        false,
        false,
        0,
        false
    ));
    assert!(c.admit(
        &Effect::StatusRead { ticket: ticket(3) },
        false,
        false,
        0,
        false
    ));
}
#[test]
fn auth_wait_blocks_other_writes_but_status_cancel_remain_explicit() {
    let mut c = CodexCoordinator::default();
    c.observe(metadata(CodexPhase::WaitingBrowser));
    assert!(c.busy());
    assert!(!c.admit(
        &Effect::EnableModels {
            ticket: ticket(1),
            expected_revision: 7
        },
        false,
        false,
        0,
        false
    ));
    assert!(!c.admit(
        &Effect::StartLogin { ticket: ticket(1) },
        false,
        false,
        0,
        false
    ));
    assert!(c.admit(
        &Effect::CancelLogin {
            ticket: ticket(1),
            attempt: 11
        },
        false,
        false,
        0,
        false
    ));
    c.settle(
        ticket(1),
        Outcome::Metadata(CodexMetadata {
            phase: CodexPhase::Indeterminate,
            retry_blocked: true,
            ..metadata(CodexPhase::WaitingBrowser)
        }),
    );
    assert!(c.busy());
}
#[test]
fn terminal_event_wins_over_late_waiting_start_receipt() {
    let mut c = CodexCoordinator::default();
    assert!(c.admit(
        &Effect::StartLogin { ticket: ticket(1) },
        false,
        false,
        0,
        false
    ));
    c.observe(metadata(CodexPhase::Authorized));
    c.settle(
        ticket(1),
        Outcome::Metadata(metadata(CodexPhase::WaitingBrowser)),
    );
    assert!(!c.busy());
    c.observe(CodexMetadata {
        attempt: Some(10),
        ..metadata(CodexPhase::WaitingBrowser)
    });
    assert!(!c.busy());
}
#[test]
fn mismatched_receipt_cannot_release_current_reservation() {
    let mut c = CodexCoordinator::default();
    assert!(c.admit(
        &Effect::StatusRead { ticket: ticket(1) },
        false,
        false,
        0,
        false
    ));
    c.settle(ticket(2), Outcome::NotSent);
    assert!(c.busy());
    c.settle(ticket(1), Outcome::NotSent);
    assert!(!c.busy());
}
