use super::*;

fn usage_save(serial: u64) -> Effect {
    Effect::UsageSave {
        ticket: ticket(serial),
        expected_revision: 0,
        preferences: dsh_native_core::CodexUsagePreferences::default(),
    }
}
#[test]
fn private_usage_admission_is_independent_of_issuer_but_obeys_normal_capacity_and_reserved_cancel()
{
    let mut c = CodexCoordinator::default();
    c.observe(metadata(CodexPhase::WaitingBrowser));
    let save = usage_save(1);
    assert!(!c.admit(&save, true, false, 0, false));
    assert!(!c.admit(&save, false, true, 0, false));
    assert!(!c.admit(&save, false, false, super::super::MAX_OPERATIONS, false));
    assert!(!c.admit(&save, false, false, 0, true));
    assert!(c.admit(&save, false, false, 0, false)); // No issuer/credential gate.
    assert!(!c.admit(
        &Effect::UsageRead { ticket: ticket(2) },
        false,
        false,
        0,
        false
    ));
    let cancel = Effect::CancelLogin {
        ticket: ticket(2),
        attempt: 11,
    };
    assert!(c.admit(&cancel, false, false, super::super::MAX_OPERATIONS, true));
    c.settle(ticket(99), Outcome::NotSent);
    assert_eq!(c.flight, Some(ticket(1)));
    c.settle(
        ticket(2),
        Outcome::Metadata(metadata(CodexPhase::Cancelled)),
    );
    assert_eq!(c.flight, Some(ticket(1))); // Cancel does not release the admitted Save.
    c.settle(ticket(1), Outcome::NotSent);
    assert!(!c.busy());
}
#[test]
fn usage_unknown_write_reconciles_only_fresh_owned_usage_read_not_auth_status_or_late_ack() {
    let mut c = CodexCoordinator::default();
    assert!(c.admit(&usage_save(1), false, false, 0, false));
    c.settle(ticket(1), Outcome::UsageIndeterminate);
    c.settle(
        ticket(1),
        Outcome::UsageSnapshot(dsh_native_core::CodexUsageSnapshot::default()),
    );
    assert!(c.usage_uncertain);
    assert!(!c.admit(&usage_save(2), false, false, 0, false));
    assert!(c.admit(
        &Effect::StatusRead { ticket: ticket(2) },
        false,
        false,
        0,
        false
    ));
    c.settle(ticket(2), Outcome::Metadata(metadata(CodexPhase::Idle)));
    assert!(c.usage_uncertain);
    assert!(c.admit(
        &Effect::UsageRead { ticket: ticket(3) },
        false,
        false,
        0,
        false
    ));
    c.settle(ticket(3), Outcome::UsageUnavailable);
    assert!(c.usage_uncertain);
    assert!(c.admit(
        &Effect::UsageRead { ticket: ticket(4) },
        false,
        false,
        0,
        false
    ));
    c.settle(
        ticket(99),
        Outcome::UsageSnapshot(dsh_native_core::CodexUsageSnapshot::default()),
    );
    assert_eq!(c.flight, Some(ticket(4)));
    assert!(c.usage_uncertain);
    c.settle(
        ticket(4),
        Outcome::UsageSnapshot(dsh_native_core::CodexUsageSnapshot::default()),
    );
    assert!(!c.usage_uncertain);
    assert!(c.admit(&usage_save(5), false, false, 0, false));
}
#[test]
fn usage_read_and_known_refusals_do_not_poison_auth_or_invent_unknown_writes() {
    for outcome in [
        Outcome::NotSent,
        Outcome::UsageConflict,
        Outcome::UsageUnavailable,
    ] {
        let mut c = CodexCoordinator::default();
        assert!(c.admit(&usage_save(1), false, false, 0, false));
        c.settle(ticket(1), outcome);
        assert!(!c.usage_uncertain && !c.uncertain && !c.busy());
    }
    let mut c = CodexCoordinator::default();
    assert!(c.admit(
        &Effect::UsageRead { ticket: ticket(1) },
        false,
        false,
        0,
        false
    ));
    c.settle(ticket(1), Outcome::UsageIndeterminate);
    assert!(!c.usage_uncertain && !c.uncertain && !c.busy());
}
#[test]
fn usage_projection_keeps_timeout_unknown_and_invalid_unsent_and_never_uses_auth_reply() {
    use dsh_native_core::Error;
    assert!(matches!(
        project_usage(Err(Error::RequestTimeout)),
        Outcome::UsageIndeterminate
    ));
    assert!(matches!(
        project_usage(Err(Error::WriteTimeout)),
        Outcome::UsageIndeterminate
    ));
    assert!(matches!(
        project_usage(Err(Error::CodexUsageIndeterminate)),
        Outcome::UsageIndeterminate
    ));
    assert!(matches!(
        project_usage(Err(Error::InvalidCommand)),
        Outcome::NotSent
    ));
    assert!(matches!(
        project_usage(Err(Error::CodexUsageConflict)),
        Outcome::UsageConflict
    ));
    assert!(matches!(
        project_usage(Err(Error::CodexUsageUnavailable)),
        Outcome::UsageUnavailable
    ));
    assert!(matches!(
        project_usage(Ok(PublicReply::Codex(metadata(CodexPhase::Idle)))),
        Outcome::UsageIndeterminate
    ));
    assert!(matches!(
        project_usage(Ok(PublicReply::CodexUsage(
            dsh_native_core::CodexUsageSnapshot::default()
        ))),
        Outcome::UsageSnapshot(_)
    ));
}
#[test]
fn private_save_postwrite_ack_mismatch_projects_unknown_for_both_worker_and_ui() {
    let preferences = dsh_native_core::CodexUsagePreferences::default();
    let valid = dsh_native_core::CodexUsageSnapshot {
        version: 1,
        revision: 4,
        preferences,
        persisted: true,
    };
    assert!(matches!(
        project_usage_save(Ok(PublicReply::CodexUsage(valid)), 3, preferences),
        Outcome::UsageSnapshot(_)
    ));
    for snapshot in [
        dsh_native_core::CodexUsageSnapshot {
            revision: 3,
            ..valid
        },
        dsh_native_core::CodexUsageSnapshot {
            persisted: false,
            ..valid
        },
        dsh_native_core::CodexUsageSnapshot {
            preferences: dsh_native_core::CodexUsagePreferences {
                retry_after_reset: true,
                ..preferences
            },
            ..valid
        },
    ] {
        assert!(matches!(
            project_usage_save(Ok(PublicReply::CodexUsage(snapshot)), 3, preferences),
            Outcome::UsageIndeterminate
        ));
    }
}
fn runtime_snapshot() -> dsh_native_core::CodexRuntimeSnapshot {
    let base = dsh_native_core::CodexRuntimeValues {
        transport: Some(dsh_native_core::CodexRuntimeTransport::Auto),
        timeout_ms: Some(dsh_native_core::CodexRuntimeNatural::new(100).unwrap()),
        ..Default::default()
    };
    dsh_native_core::CodexRuntimeSnapshot {
        version: 1,
        revision: 4,
        configured: true,
        writable: true,
        values: base,
        base_values: base,
        overrides: Default::default(),
    }
}
fn runtime_save(serial: u64) -> Effect {
    Effect::RuntimeSave {
        ticket: ticket(serial),
        expected_revision: 4,
        values: Default::default(),
        base_values: runtime_snapshot().base_values,
    }
}
#[test]
fn runtime_admission_capacity_epoch_and_cancel_never_forget_admitted_save() {
    let mut c = CodexCoordinator::default();
    c.observe(metadata(CodexPhase::WaitingBrowser));
    let save = runtime_save(1);
    assert!(!c.admit(&save, true, false, 0, false));
    assert!(!c.admit(&save, false, true, 0, false));
    assert!(!c.admit(&save, false, false, super::super::MAX_OPERATIONS, false));
    assert!(!c.admit(&save, false, false, 0, true));
    assert!(!c.admit(
        &Effect::RuntimeRead {
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
    assert!(c.admit(&save, false, false, 0, false));
    assert!(c.admit(
        &Effect::CancelLogin {
            ticket: ticket(2),
            attempt: 11
        },
        false,
        false,
        super::super::MAX_OPERATIONS,
        true
    ));
    c.settle(
        ticket(2),
        Outcome::Metadata(metadata(CodexPhase::Cancelled)),
    );
    assert_eq!(c.flight, Some(ticket(1)));
    assert!(c.runtime_operation.is_some());
    assert!(!c.admit(
        &Effect::RuntimeRead { ticket: ticket(3) },
        false,
        false,
        0,
        false
    ));
    c.settle(ticket(99), Outcome::Runtime(runtime_snapshot()));
    assert_eq!(c.flight, Some(ticket(1)));
    c.settle(ticket(1), Outcome::Runtime(runtime_snapshot())); // inherited no-op ACK
    assert!(!c.runtime_uncertain && !c.busy());
}
#[test]
fn runtime_unknown_or_conflict_only_reconciles_owned_runtime_read_not_usage_or_oauth() {
    for outcome in [
        Outcome::RuntimeIndeterminate,
        Outcome::RuntimeConflict,
        Outcome::RuntimeUnavailable,
    ] {
        let mut c = CodexCoordinator::default();
        assert!(c.admit(&runtime_save(1), false, false, 0, false));
        c.settle(ticket(1), outcome);
        assert!(c.runtime_uncertain);
        assert!(!c.uncertain && !c.usage_uncertain);
        c.settle(ticket(1), Outcome::Runtime(runtime_snapshot()));
        assert!(c.runtime_uncertain);
        assert!(!c.admit(&runtime_save(2), false, false, 0, false));
        assert!(c.admit(
            &Effect::UsageRead { ticket: ticket(2) },
            false,
            false,
            0,
            false
        ));
        c.settle(
            ticket(2),
            Outcome::UsageSnapshot(dsh_native_core::CodexUsageSnapshot::default()),
        );
        assert!(c.runtime_uncertain);
        assert!(c.admit(
            &Effect::StatusRead { ticket: ticket(3) },
            false,
            false,
            0,
            false
        ));
        c.settle(
            ticket(3),
            Outcome::StatusSnapshot {
                metadata: metadata(CodexPhase::Idle),
                accounts: managed_accounts(),
            },
        );
        assert!(c.runtime_uncertain);
        assert!(c.admit(
            &Effect::RuntimeRead { ticket: ticket(4) },
            false,
            false,
            0,
            false
        ));
        c.settle(ticket(4), Outcome::RuntimeUnavailable);
        assert!(c.runtime_uncertain);
        assert!(c.admit(
            &Effect::RuntimeRead { ticket: ticket(5) },
            false,
            false,
            0,
            false
        ));
        c.settle(ticket(99), Outcome::Runtime(runtime_snapshot()));
        assert!(c.runtime_uncertain);
        c.settle(ticket(5), Outcome::Runtime(runtime_snapshot()));
        assert!(!c.runtime_uncertain);
        assert!(c.admit(&runtime_save(6), false, false, 0, false));
    }
}
#[test]
fn runtime_projection_and_worker_ack_verification_fence_unknown_writes() {
    use dsh_native_core::Error;
    let baseline = runtime_snapshot();
    assert!(matches!(
        project_runtime(Err(Error::RequestTimeout)),
        Outcome::RuntimeIndeterminate
    ));
    assert!(matches!(
        project_runtime(Err(Error::WriteTimeout)),
        Outcome::RuntimeIndeterminate
    ));
    assert!(matches!(
        project_runtime(Err(Error::CodexRuntimeIndeterminate)),
        Outcome::RuntimeIndeterminate
    ));
    assert!(matches!(
        project_runtime(Err(Error::CodexRuntimeConflict)),
        Outcome::RuntimeConflict
    ));
    assert!(matches!(
        project_runtime(Err(Error::CodexRuntimeUnavailable)),
        Outcome::RuntimeUnavailable
    ));
    assert!(matches!(
        project_runtime(Err(Error::InvalidCommand)),
        Outcome::NotSent
    ));
    assert!(matches!(
        project_runtime(Ok(PublicReply::Codex(metadata(CodexPhase::Idle)))),
        Outcome::RuntimeIndeterminate
    ));
    assert!(matches!(
        project_runtime_save(
            Ok(PublicReply::CodexRuntime(baseline)),
            4,
            Default::default(),
            baseline.base_values
        ),
        Outcome::Runtime(_)
    ));
    for after in [
        dsh_native_core::CodexRuntimeSnapshot {
            revision: 3,
            ..baseline
        },
        dsh_native_core::CodexRuntimeSnapshot {
            revision: 6,
            ..baseline
        },
        dsh_native_core::CodexRuntimeSnapshot {
            configured: false,
            ..baseline
        },
        dsh_native_core::CodexRuntimeSnapshot {
            values: Default::default(),
            ..baseline
        },
        dsh_native_core::CodexRuntimeSnapshot {
            base_values: Default::default(),
            ..baseline
        },
    ] {
        assert!(matches!(
            project_runtime_save(
                Ok(PublicReply::CodexRuntime(after)),
                4,
                Default::default(),
                baseline.base_values
            ),
            Outcome::RuntimeIndeterminate
        ));
        let mut c = CodexCoordinator::default();
        assert!(c.admit(&runtime_save(1), false, false, 0, false));
        c.settle(ticket(1), Outcome::Runtime(after));
        assert!(c.runtime_uncertain);
        assert!(!c.uncertain && !c.usage_uncertain);
    }
}
#[test]
fn runtime_reads_and_unsent_saves_do_not_poison_usage_or_oauth_uncertainty() {
    let mut c = CodexCoordinator::default();
    assert!(c.admit(&runtime_save(1), false, false, 0, false));
    c.settle(ticket(1), Outcome::NotSent);
    assert!(!c.runtime_uncertain && !c.usage_uncertain && !c.uncertain);
    assert!(c.admit(
        &Effect::RuntimeRead { ticket: ticket(2) },
        false,
        false,
        0,
        false
    ));
    c.settle(ticket(2), Outcome::RuntimeIndeterminate);
    assert!(!c.runtime_uncertain && !c.usage_uncertain && !c.uncertain);
    assert!(c.admit(&usage_save(3), false, false, 0, false));
    c.settle(ticket(3), Outcome::UsageIndeterminate);
    assert!(c.usage_uncertain);
    assert!(c.admit(
        &Effect::RuntimeRead { ticket: ticket(4) },
        false,
        false,
        0,
        false
    ));
    c.settle(ticket(4), Outcome::Runtime(runtime_snapshot()));
    assert!(c.usage_uncertain); // Runtime cannot discharge preference uncertainty.
}
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

#[test]
fn cancel_has_reserved_priority_slot_while_login_and_normal_task_limit_are_busy() {
    let mut c = CodexCoordinator::default();
    assert!(c.admit(
        &Effect::StartLogin { ticket: ticket(1) },
        false,
        false,
        0,
        false
    ));
    let cancel = Effect::CancelLogin {
        ticket: ticket(2),
        attempt: 11,
    };
    assert!(c.admit(&cancel, false, false, super::super::MAX_OPERATIONS, true));
    assert!(!c.admit(&cancel, false, false, 0, false));
    c.settle(
        ticket(2),
        Outcome::Metadata(metadata(CodexPhase::Cancelled)),
    );
    assert!(c.busy()); // Start still owns its separate slot.
    c.settle(ticket(1), Outcome::Indeterminate);
    assert!(c.busy());
}

fn managed_accounts() -> dsh_native_core::CodexAccountsMetadata {
    dsh_native_core::CodexAccountsMetadata {
        mode: dsh_native_core::CodexAccountMode::Managed,
        accounts: Vec::new(),
        selected: None,
        epoch: Some(0),
        legacy: dsh_native_core::CodexLegacyGrant::Absent,
        added: None,
    }
}
#[test]
fn unknown_account_writes_and_late_ack_require_explicit_nonmutating_reconciliation() {
    for kind in 0..3 {
        let mut c = CodexCoordinator::default();
        let account_id = dsh_native_core::CodexAccountId::new(
            "account-00000000-0000-4000-8000-000000000001".into(),
        )
        .unwrap();
        let effect = match kind {
            0 => Effect::SelectAccount {
                ticket: ticket(1),
                account_id,
                expected_epoch: 0,
            },
            1 => Effect::SignOutAccount {
                ticket: ticket(1),
                account_id,
                expected_epoch: 0,
            },
            _ => Effect::SignOutAll {
                ticket: ticket(1),
                expected_epoch: 0,
            },
        };
        assert!(c.admit(&effect, false, false, 0, false));
        c.settle(ticket(1), Outcome::Indeterminate);
        c.settle(ticket(1), Outcome::Accounts(managed_accounts()));
        assert!(c.busy());
        assert!(!c.admit(
            &Effect::SignOutAll {
                ticket: ticket(2),
                expected_epoch: 0
            },
            false,
            false,
            0,
            false
        ));
        assert!(c.admit(
            &Effect::AccountsRead { ticket: ticket(2) },
            false,
            false,
            0,
            false
        ));
        c.settle(
            ticket(99),
            Outcome::StatusSnapshot {
                metadata: metadata(CodexPhase::Idle),
                accounts: managed_accounts(),
            },
        );
        assert!(c.busy());
        c.settle(
            ticket(2),
            Outcome::StatusSnapshot {
                metadata: metadata(CodexPhase::Idle),
                accounts: managed_accounts(),
            },
        );
        assert!(!c.busy());
    }
}
#[test]
fn account_reads_never_reconcile_active_or_legacy_unknown_writes() {
    for legacy in [false, true] {
        let mut c = CodexCoordinator::default();
        assert!(c.admit(
            &Effect::StartLogin { ticket: ticket(1) },
            false,
            false,
            0,
            false
        ));
        c.settle(ticket(1), Outcome::Indeterminate);
        assert!(c.admit(
            &Effect::AccountsRead { ticket: ticket(2) },
            false,
            false,
            0,
            false
        ));
        let mut accounts = managed_accounts();
        if legacy {
            accounts.mode = dsh_native_core::CodexAccountMode::Legacy;
            accounts.epoch = None;
        }
        c.settle(
            ticket(2),
            Outcome::StatusSnapshot {
                metadata: metadata(if legacy {
                    CodexPhase::Idle
                } else {
                    CodexPhase::WaitingBrowser
                }),
                accounts,
            },
        );
        assert!(c.busy());
    }
}
