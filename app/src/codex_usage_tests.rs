//! PUBLIC synthetic local preferences, production reducer only; no backend, account or provider.
use super::super::{Action as CodexAction, Context, Controller, Effect, Tab};
use super::*;

fn context() -> Context {
    Context {
        epoch: 1,
        visible: true,
        enabled: true,
    }
}
fn controller() -> Controller {
    let mut c = Controller::default();
    c.enter(1);
    c
}
fn action(c: &mut Controller, action: Action) -> Option<Effect> {
    c.handle(
        CodexAction::Usage {
            ticket: c.ticket(),
            action,
        },
        context(),
    )
}
fn read(c: &mut Controller, snapshot: CodexUsageSnapshot) {
    let Some(Effect::UsageRead { ticket }) = action(c, Action::Refresh) else {
        panic!("private read")
    };
    c.completed(ticket, Outcome::UsageSnapshot(snapshot));
}
fn save(c: &mut Controller) -> (Ticket, u64, CodexUsagePreferences) {
    let Some(Effect::UsageSave {
        ticket,
        expected_revision,
        preferences,
    }) = action(c, Action::Save)
    else {
        panic!("private save")
    };
    (ticket, expected_revision, preferences)
}
fn ack(revision: u64, preferences: CodexUsagePreferences) -> Outcome {
    Outcome::UsageSnapshot(CodexUsageSnapshot {
        version: 1,
        revision: revision + 1,
        preferences,
        persisted: true,
    })
}
fn loaded() -> Controller {
    let mut c = controller();
    read(&mut c, CodexUsageSnapshot::default());
    c
}
#[test]
fn unwritten_defaults_cannot_save_until_read_and_all_modes_are_local_drafts() {
    let mut c = controller();
    assert!(action(&mut c, Action::Save).is_none());
    assert!(c.usage.snapshot.is_none());
    read(&mut c, CodexUsageSnapshot::default());
    let baseline = c.usage.snapshot.unwrap();
    assert_eq!(baseline.revision, 0);
    assert!(!baseline.persisted);
    for mode in [
        CodexComposerDisplay::Off,
        CodexComposerDisplay::Percent,
        CodexComposerDisplay::ProgressBar,
        CodexComposerDisplay::Runway,
    ] {
        assert!(action(&mut c, Action::Composer(mode)).is_none());
        assert_eq!(c.usage.draft.composer_display, mode);
    }
    for mode in [
        CodexUsageAlert::Off,
        CodexUsageAlert::Percent20,
        CodexUsageAlert::EarlyWarning,
        CodexUsageAlert::Custom,
    ] {
        assert!(action(&mut c, Action::Alert(mode)).is_none());
        assert_eq!(c.usage.draft.usage_alert, mode);
    }
    assert!(action(&mut c, Action::Retry(true)).is_none());
    assert_eq!(c.usage.snapshot, Some(baseline));
    assert!(c.metadata.is_none() && c.connection.is_none() && c.flight.is_none());
    assert!(!c.effective_consent());
}
#[test]
fn bounded_fractional_threshold_rejects_invalid_text_without_coercion() {
    let mut c = loaded();
    for (raw, expected) in [("0", 0.0), ("100", 100.0), ("12.375", 12.375)] {
        action(&mut c, Action::Threshold(raw.into()));
        assert!(c.usage.can_save());
        assert_eq!(c.usage.draft.custom_remaining_percent.as_f64(), expected);
    }
    for raw in ["", "abc", "NaN", "inf", "-0.01", "100.001", " 20 "] {
        action(&mut c, Action::Threshold(raw.into()));
        assert!(!c.usage.can_save(), "{raw}");
        assert!(action(&mut c, Action::Save).is_none());
        assert_eq!(c.usage.draft.custom_remaining_percent.as_f64(), 12.375);
    }
}
#[test]
fn save_captures_revision_and_only_matching_actual_snapshot_ack_commits() {
    let mut c = loaded();
    action(&mut c, Action::Composer(CodexComposerDisplay::Percent));
    let before = c.usage.snapshot;
    let (ticket, revision, preferences) = save(&mut c);
    assert_eq!(revision, 0);
    assert_eq!(c.usage.snapshot, before);
    assert!(action(&mut c, Action::Save).is_none());
    c.completed(
        Ticket {
            serial: ticket.serial + 1,
            ..ticket
        },
        ack(revision, preferences),
    );
    assert!(c.usage.pending());
    c.completed(ticket, ack(revision, preferences));
    assert!(!c.usage.pending());
    assert_eq!(c.usage.snapshot.unwrap().preferences, preferences);
    assert!(c.usage.status().starts_with("Preferences saved locally"));
    let snapshot = c.usage.snapshot;
    c.completed(ticket, Outcome::UsageUnavailable);
    assert_eq!(c.usage.snapshot, snapshot);
}
#[test]
fn conflict_preserves_draft_and_refresh_rebases_without_replay_or_false_ack() {
    let mut c = loaded();
    action(&mut c, Action::Composer(CodexComposerDisplay::Runway));
    let (ticket, _, _) = save(&mut c);
    c.completed(ticket, Outcome::UsageConflict);
    assert!(c.usage.status().starts_with("Save failed"));
    assert!(c.usage.attention());
    assert!(!c.usage.can_save());
    assert!(!c.usage.snapshot.unwrap().persisted);
    let snapshot = CodexUsageSnapshot {
        revision: 9,
        persisted: true,
        ..CodexUsageSnapshot::default()
    };
    read(&mut c, snapshot);
    assert_eq!(c.usage.draft.composer_display, CodexComposerDisplay::Runway);
    assert!(c.continue_connection(context()).is_none());
    let (_, revision, _) = save(&mut c);
    assert_eq!(revision, 9);
}
#[test]
fn corrupt_or_failed_load_never_authorizes_defaults_or_cached_fallback() {
    for cached in [false, true] {
        let mut c = if cached { loaded() } else { controller() };
        let Some(Effect::UsageRead { ticket }) = action(&mut c, Action::Refresh) else {
            panic!()
        };
        assert!(action(&mut c, Action::Save).is_none());
        c.completed(ticket, Outcome::UsageUnavailable);
        assert!(c.usage.attention());
        assert!(!c.usage.can_save());
        action(&mut c, Action::Retry(true));
        assert!(c.usage.status().starts_with("Preferences unavailable"));
        assert!(c.usage.status().contains("app restart"));
        assert!(action(&mut c, Action::Save).is_none());
        assert!(c.continue_connection(context()).is_none());
    }
}
#[test]
fn indeterminate_save_requires_fresh_read_and_old_receipts_cannot_reconcile() {
    let mut c = loaded();
    action(&mut c, Action::Consent(true));
    let (ticket, revision, preferences) = save(&mut c);
    c.completed(ticket, Outcome::UsageIndeterminate);
    assert!(!c.effective_consent());
    assert!(!c.usage.can_save());
    assert!(c.usage.status().starts_with("Save outcome unknown"));
    c.completed(ticket, ack(revision, preferences));
    assert!(!c.usage.snapshot.unwrap().persisted);
    read(
        &mut c,
        CodexUsageSnapshot {
            revision: 1,
            preferences,
            persisted: true,
            version: 1,
        },
    );
    assert!(!c.effective_consent()); // Read alone never ACKs a new opt-in.
    let (ticket, revision, preferences) = save(&mut c);
    c.completed(ticket, ack(revision, preferences));
    assert!(c.effective_consent());
}
#[test]
fn local_off_fences_old_true_ack_and_failed_persistence_and_read_then_explicit_rearm() {
    let mut c = loaded();
    action(&mut c, Action::Consent(true));
    let (ticket, revision, preferences) = save(&mut c);
    assert!(!c.effective_consent());
    action(&mut c, Action::Consent(false));
    c.completed(ticket, ack(revision, preferences));
    assert!(!c.usage.draft.experimental_quota);
    assert!(!c.effective_consent());
    let (off_ticket, _, _) = save(&mut c);
    c.completed(off_ticket, Outcome::NotSent);
    assert!(c.usage.status().starts_with("Save failed"));
    read(
        &mut c,
        CodexUsageSnapshot {
            version: 1,
            revision: 1,
            preferences,
            persisted: true,
        },
    );
    assert!(!c.effective_consent());
    assert!(!c.usage.draft.experimental_quota);
    action(&mut c, Action::Consent(true));
    assert!(!c.effective_consent());
    let (ticket, revision, preferences) = save(&mut c);
    c.completed(ticket, ack(revision, preferences));
    assert!(c.effective_consent());
    let ticket = c.ticket();
    c.handle(
        CodexAction::Usage {
            ticket,
            action: Action::Consent(false),
        },
        Context {
            enabled: false,
            ..context()
        },
    );
    assert!(!c.effective_consent());
}
#[test]
fn hide_tab_switch_epoch_and_stale_actions_cannot_ack_or_replace_drafts() {
    for kind in 0..3 {
        let mut c = loaded();
        action(&mut c, Action::Consent(true));
        let (ticket, revision, preferences) = save(&mut c);
        match kind {
            0 => {
                c.leave();
                c.enter(1);
                assert!(c.usage.pending());
            }
            1 => {
                c.handle(
                    CodexAction::SelectTab {
                        ticket: c.ticket(),
                        tab: Tab::Models,
                    },
                    context(),
                );
            }
            _ => c.adopt_epoch(2),
        }
        c.completed(ticket, ack(revision, preferences));
        assert!(!c.effective_consent());
        assert!(!c.usage.snapshot.unwrap().persisted);
        assert!(c.usage.draft.experimental_quota);
        assert!(!c.usage.can_save());
        c.handle(
            CodexAction::Usage {
                ticket,
                action: Action::Consent(false),
            },
            context(),
        );
        assert!(c.usage.draft.experimental_quota); // stale ticket cannot mutate current draft
    }
}
#[test]
fn preference_flight_survives_reserved_oauth_cancel_without_false_ack_or_permission_restore() {
    let mut c = loaded();
    c.metadata = Some(dsh_native_core::CodexMetadata {
        available: true,
        credential_stored: false,
        route_configured: false,
        phase: dsh_native_core::CodexPhase::WaitingBrowser,
        attempt: Some(11),
        prompt: None,
        browser_available: false,
        settings_revision: Some(7),
        retry_blocked: false,
    });
    c.fresh = true;
    action(&mut c, Action::Consent(true));
    let (save_ticket, revision, preferences) = save(&mut c);
    action(&mut c, Action::Consent(false));
    let Some(Effect::CancelLogin {
        ticket: cancel_ticket,
        ..
    }) = c.handle(
        CodexAction::Cancel {
            ticket: c.ticket(),
            attempt: 11,
        },
        context(),
    )
    else {
        panic!("reserved cancel")
    };
    assert_eq!(c.usage.ticket(), Some(save_ticket));
    c.completed(cancel_ticket, Outcome::Confirmed);
    assert!(c.usage.pending());
    c.completed(save_ticket, ack(revision, preferences));
    assert!(!c.usage.pending());
    assert!(!c.effective_consent());
    assert!(!c.usage.draft.experimental_quota);
}
#[test]
fn pending_read_cannot_replace_local_off_and_oversized_threshold_cannot_save_previous_value() {
    let mut c = loaded();
    action(&mut c, Action::Consent(true));
    let (ticket, revision, preferences) = save(&mut c);
    c.completed(ticket, ack(revision, preferences));
    assert!(c.effective_consent());
    let Some(Effect::UsageRead { ticket }) = action(&mut c, Action::Refresh) else {
        panic!()
    };
    action(&mut c, Action::Consent(false));
    assert!(!c.effective_consent());
    c.completed(
        ticket,
        Outcome::UsageSnapshot(CodexUsageSnapshot {
            version: 1,
            revision: 1,
            preferences,
            persisted: true,
        }),
    );
    assert!(!c.usage.draft.experimental_quota);
    assert!(!c.effective_consent());
    action(&mut c, Action::Threshold("2".repeat(65)));
    assert!(c.usage.threshold.len() <= 64);
    assert!(!c.usage.can_save());
}
#[test]
fn malformed_save_snapshot_is_unknown_not_success() {
    let mut c = loaded();
    let (ticket, revision, preferences) = save(&mut c);
    c.completed(
        ticket,
        Outcome::UsageSnapshot(CodexUsageSnapshot {
            version: 1,
            revision,
            preferences,
            persisted: true,
        }),
    );
    assert!(!c.usage.can_save());
    assert!(!c.usage.snapshot.unwrap().persisted);
    assert!(c.usage.status().starts_with("Save outcome unknown"));
}

#[test]
fn interrupted_connect_status_cannot_schedule_an_unowned_preference_followup() {
    use dsh_native_core::{
        CodexAccountMode, CodexAccountsMetadata, CodexLegacyGrant, CodexMetadata, CodexPhase,
    };
    let mut c = controller();
    let Some(Effect::StatusRead { ticket }) = c.handle(CodexAction::Connect(c.ticket()), context())
    else {
        panic!("explicit connect status")
    };
    assert!(
        c.continue_connection(Context {
            visible: false,
            ..context()
        })
        .is_none()
    );
    assert!(c.connection.is_none());
    c.completed(
        ticket,
        Outcome::StatusSnapshot {
            metadata: CodexMetadata {
                available: true,
                credential_stored: false,
                route_configured: false,
                phase: CodexPhase::Idle,
                attempt: None,
                prompt: None,
                browser_available: false,
                settings_revision: Some(0),
                retry_blocked: false,
            },
            accounts: CodexAccountsMetadata {
                mode: CodexAccountMode::Managed,
                accounts: vec![],
                selected: None,
                epoch: Some(0),
                legacy: CodexLegacyGrant::Absent,
                added: None,
            },
        },
    );
    assert!(c.continue_connection(context()).is_none());
    assert!(!c.usage.pending());
    assert!(c.usage.snapshot.is_none());
}

#[test]
fn valid_threshold_clears_only_local_validation_warning_not_backend_reconciliation() {
    let mut c = loaded();
    action(&mut c, Action::Threshold("NaN".into()));
    assert!(c.usage_footer().unwrap().1);
    assert!(!c.usage.can_save());
    action(&mut c, Action::Threshold("17.5".into()));
    assert!(c.usage_footer().is_none());
    assert!(c.usage.can_save());
    assert!(c.usage.status().contains("Draft changes are not saved"));
}
#[test]
fn native_owned_refresh_rebases_readonly_threshold_without_losing_optouts() {
    let mut c = controller();
    let initial = CodexUsageSnapshot {
        version: 1,
        revision: 9,
        persisted: true,
        preferences: CodexUsagePreferences {
            composer_display: CodexComposerDisplay::ProgressBar,
            usage_alert: CodexUsageAlert::Custom,
            retry_after_reset: true,
            experimental_quota: true,
            ..CodexUsagePreferences::default()
        },
    };
    read(&mut c, initial);
    for a in [
        Action::Composer(CodexComposerDisplay::Off),
        Action::Alert(CodexUsageAlert::Off),
        Action::Retry(false),
        Action::Consent(false),
    ] {
        action(&mut c, a);
    }
    let Some(Effect::UsageRead { ticket }) = action(&mut c, Action::Refresh) else {
        panic!("read")
    };
    let mut updated = initial;
    updated.revision = 10;
    updated.preferences.custom_remaining_percent = CodexRemainingPercent::new(17.5).unwrap();
    c.completed_native(
        Ticket {
            serial: ticket.serial + 1,
            ..ticket
        },
        Outcome::UsageSnapshot(updated),
    );
    assert_eq!(c.usage.threshold, "20");
    assert!(c.usage.pending());
    c.completed_native(ticket, Outcome::UsageSnapshot(updated));
    assert_eq!(c.usage.threshold, "17.5");
    assert!(c.usage_surface_action_allowed(&Action::Save));
    assert!(!c.effective_consent());
    let (_, revision, preferences) = save(&mut c);
    assert_eq!(revision, 10);
    assert_eq!(preferences.custom_remaining_percent.as_f64(), 17.5);
    assert_eq!(preferences.composer_display, CodexComposerDisplay::Off);
    assert_eq!(preferences.usage_alert, CodexUsageAlert::Off);
    assert!(!preferences.retry_after_reset && !preferences.experimental_quota);
}
#[test]
fn native_failed_unowned_or_retired_read_cannot_rebase_threshold_or_restore_consent() {
    for kind in 0..4 {
        let mut c = loaded();
        action(&mut c, Action::Threshold("19.25".into())); // Generic foundation draft, not native UI.
        action(&mut c, Action::Consent(false));
        let Some(Effect::UsageRead { ticket }) = action(&mut c, Action::Refresh) else {
            panic!("read")
        };
        match kind {
            0 => c.leave(),
            1 => c.adopt_epoch(2),
            _ => {}
        }
        let mut updated = CodexUsageSnapshot::default();
        updated.preferences.custom_remaining_percent = CodexRemainingPercent::new(17.5).unwrap();
        if kind == 2 {
            c.completed_native(ticket, Outcome::UsageUnavailable);
        } else if kind == 3 {
            c.completed_native(
                ticket,
                Outcome::UsageSnapshot(CodexUsageSnapshot {
                    version: 2,
                    ..updated
                }),
            );
        } else {
            c.completed_native(ticket, Outcome::UsageSnapshot(updated));
        }
        assert_eq!(c.usage.threshold, "19.25");
        assert!(!c.usage.can_save());
        assert!(!c.effective_consent());
    }
}
#[test]
fn native_unrelated_and_duplicate_callbacks_do_not_rebase_generic_drafts() {
    let mut c = loaded();
    let (ticket, revision, preferences) = save(&mut c);
    c.completed_native(ticket, ack(revision, preferences));
    action(&mut c, Action::Threshold("19.25".into()));
    c.completed_native(ticket, ack(revision, preferences));
    assert_eq!(c.usage.threshold, "19.25");
    let Some(Effect::UsageRead { ticket }) = action(&mut c, Action::Refresh) else {
        panic!("read")
    };
    c.completed(
        ticket,
        Outcome::UsageSnapshot(CodexUsageSnapshot::default()),
    ); // Generic API stays editable.
    assert_eq!(c.usage.threshold, "19.25");
    assert!(!c.usage_surface_action_allowed(&Action::Save));
}
#[test]
fn native_save_policy_cannot_persist_enabling_or_threshold_changes_from_generic_foundation() {
    for action in [
        Action::Composer(CodexComposerDisplay::Percent),
        Action::Alert(CodexUsageAlert::Custom),
        Action::Retry(true),
        Action::Consent(true),
        Action::Threshold("17.5".into()),
    ] {
        let mut c = loaded();
        c.usage.edit(action); // The future-compatible store/reducer is not the native capability gate.
        assert!(c.usage.can_save());
        assert!(!c.usage_surface_action_allowed(&Action::Save));
    }
}
#[test]
fn reconciliation_warnings_survive_invalid_drafts_and_clear_only_after_owned_refresh() {
    for outcome in [
        Outcome::Indeterminate,
        Outcome::UsageConflict,
        Outcome::UsageUnavailable,
    ] {
        let mut c = loaded();
        let (ticket, _, _) = save(&mut c);
        c.completed(ticket, outcome);
        let warning = c.usage.status();
        for raw in ["NaN".into(), "101".into(), "2".repeat(65)] {
            action(&mut c, Action::Threshold(raw));
            assert_eq!(c.usage.status(), warning);
            assert!(!c.usage.valid_threshold());
            assert!(!c.usage.can_save());
            assert!(action(&mut c, Action::Save).is_none());
        }
        action(&mut c, Action::Threshold("17.5".into()));
        assert_eq!(c.usage.status(), warning);
        assert!(!c.usage.can_save());
        let (notice, attention) = c.usage_footer().unwrap();
        assert_eq!(notice, warning);
        assert!(attention);
        read(&mut c, CodexUsageSnapshot::default());
        assert!(c.usage_footer().is_none());
        assert!(c.usage.can_save());
        assert!(!c.effective_consent());
    }
}

#[test]
fn fixed_usage_footer_retains_pending_and_unknown_across_tabs_and_hiding() {
    let mut c = loaded();
    assert!(c.usage_footer().is_none());
    let (ticket, revision, preferences) = save(&mut c);
    let (notice, attention) = c.usage_footer().unwrap();
    assert!(notice.contains("not yet acknowledged"));
    assert!(!attention);
    assert!(!c.pending_account_operation()); // Local preferences are not sign-in work.
    for tab in Tab::ALL {
        c.handle(
            CodexAction::SelectTab {
                ticket: c.ticket(),
                tab,
            },
            context(),
        );
        assert!(c.usage_footer().unwrap().0.contains("not yet acknowledged"));
        let _: iced::Element<'_, CodexAction> = c.view(context());
    }
    c.leave();
    c.completed(ticket, ack(revision, preferences));
    let warning = c.usage_footer().unwrap();
    assert!(warning.0.contains("unknown") && warning.1);
    c.enter(1);
    assert_eq!(c.usage_footer(), Some(warning));
    assert!(!c.usage.can_save());
    assert!(!c.effective_consent());
}

#[test]
fn lost_read_is_unavailable_not_false_write_uncertainty_and_late_receipt_cannot_clear_it() {
    let mut c = loaded();
    let Some(Effect::UsageRead { ticket }) = action(&mut c, Action::Refresh) else {
        panic!("read")
    };
    assert!(c.usage_footer().unwrap().0.contains("Reading local"));
    c.adopt_epoch(2);
    let warning = c.usage_footer().unwrap();
    assert!(warning.0.contains("Preferences unavailable"));
    assert!(!warning.0.contains("Save outcome unknown"));
    assert!(warning.1);
    c.completed(
        ticket,
        Outcome::UsageSnapshot(CodexUsageSnapshot::default()),
    );
    assert_eq!(c.usage_footer(), Some(warning));
    assert!(!c.usage.can_save());
    assert!(!c.effective_consent());
}
