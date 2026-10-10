use super::*;
use dsh_native_core::CodexAccount;

fn context() -> Context {
    Context {
        epoch: 1,
        visible: true,
        enabled: true,
    }
}
fn id(n: u8) -> CodexAccountId {
    CodexAccountId::new(format!("account-00000000-0000-4000-8000-{n:012x}")).unwrap()
}
fn accounts(epoch: u64) -> CodexAccountsMetadata {
    CodexAccountsMetadata {
        mode: CodexAccountMode::Managed,
        accounts: vec![CodexAccount {
            opaque_id: id(1),
            label: CodexAccountLabel::new("PUBLIC local alias".into()).unwrap(),
            email: None,
            plan: None,
        }],
        selected: None,
        epoch: Some(epoch),
        legacy: CodexLegacyGrant::Absent,
        added: None,
    }
}
fn local() -> CodexMetadata {
    CodexMetadata {
        available: true,
        credential_stored: false,
        route_configured: false,
        phase: CodexPhase::Idle,
        attempt: None,
        prompt: None,
        browser_available: false,
        settings_revision: Some(7),
        retry_blocked: false,
    }
}
fn waiting() -> CodexMetadata {
    CodexMetadata {
        phase: CodexPhase::WaitingBrowser,
        attempt: Some(11),
        browser_available: true,
        prompt: Some(12),
        ..local()
    }
}
fn authorized() -> CodexMetadata {
    CodexMetadata {
        phase: CodexPhase::Authorized,
        ..waiting()
    }
}
fn complete(c: &mut Controller, outcome: Outcome) {
    c.completed(c.operation_ticket().unwrap(), outcome);
}
fn loaded() -> Controller {
    let mut c = Controller::default();
    c.enter(1);
    assert!(
        c.handle(Action::RefreshAccounts(c.ticket()), context())
            .is_some()
    );
    complete(
        &mut c,
        Outcome::StatusSnapshot {
            metadata: local(),
            accounts: accounts(0),
        },
    );
    c
}
fn connecting() -> Controller {
    let mut c = Controller::default();
    c.enter(1);
    assert!(matches!(
        c.handle(Action::Connect(c.ticket()), context()),
        Some(Effect::StatusRead { .. })
    ));
    complete(
        &mut c,
        Outcome::StatusSnapshot {
            metadata: local(),
            accounts: accounts(0),
        },
    );
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::StartLogin { .. })
    ));
    complete(&mut c, Outcome::Metadata(waiting()));
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::OpenBrowser { attempt: 11, .. })
    ));
    complete(&mut c, Outcome::Confirmed);
    c.observed(authorized());
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::AccountsRead { .. })
    ));
    c
}
fn added(epoch: u64, attempt: u64) -> CodexAccountsMetadata {
    let mut m = accounts(epoch);
    m.added = Some(CodexAddedAccount {
        account_id: id(1),
        epoch,
        attempt,
    });
    m
}
#[test]
fn zero_epoch_exact_add_receipt_selects_only_that_account_then_enables_by_revision() {
    let mut c = connecting();
    complete(
        &mut c,
        Outcome::StatusSnapshot {
            metadata: authorized(),
            accounts: added(0, 11),
        },
    );
    assert!(
        matches!(c.continue_connection(context()), Some(Effect::SelectAccount { account_id, expected_epoch: 0, .. }) if account_id == id(1))
    );
    let mut selected = accounts(1);
    selected.selected = Some(id(1));
    complete(&mut c, Outcome::Accounts(selected.clone()));
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::StatusRead { .. })
    ));
    complete(
        &mut c,
        Outcome::StatusSnapshot {
            metadata: CodexMetadata {
                credential_stored: true,
                ..authorized()
            },
            accounts: selected,
        },
    );
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::EnableModels {
            expected_revision: 7,
            ..
        })
    ));
    complete(
        &mut c,
        Outcome::Metadata(CodexMetadata {
            credential_stored: true,
            route_configured: true,
            settings_revision: Some(8),
            ..authorized()
        }),
    );
    assert!(c.connection.is_none());
    assert!(matches!(c.notice, Notice::Connected));
}
#[test]
fn different_attempt_missing_receipt_or_later_describe_epoch_never_selects() {
    for m in [added(0, 99), accounts(0), {
        let mut m = added(0, 11);
        m.epoch = Some(1);
        m
    }] {
        let mut c = connecting();
        complete(&mut c, Outcome::Accounts(m));
        assert!(c.continue_connection(context()).is_none());
        assert!(c.connection.is_none());
    }
}
#[test]
fn status_after_selection_cannot_enable_a_different_or_later_selection() {
    for epoch in [1, 2] {
        let mut c = connecting();
        complete(&mut c, Outcome::Accounts(added(0, 11)));
        assert!(matches!(
            c.continue_connection(context()),
            Some(Effect::SelectAccount { .. })
        ));
        let mut selected = accounts(1);
        selected.selected = Some(id(1));
        complete(&mut c, Outcome::Accounts(selected));
        c.continue_connection(context());
        let mut later = accounts(epoch);
        if epoch == 2 {
            later.selected = Some(id(1));
        }
        complete(
            &mut c,
            Outcome::StatusSnapshot {
                metadata: CodexMetadata {
                    credential_stored: true,
                    ..authorized()
                },
                accounts: later,
            },
        );
        assert!(c.continue_connection(context()).is_none());
        assert!(c.connection.is_none());
    }
}
#[test]
fn closing_tabs_disconnect_hidden_and_manual_refresh_stop_receipt_selection() {
    for intervention in 0..6 {
        let mut c = connecting();
        let flight = c.operation_ticket().unwrap();
        match intervention {
            0 => c.leave(),
            1 => {
                c.handle(
                    Action::SelectTab {
                        ticket: c.ticket(),
                        tab: Tab::Models,
                    },
                    context(),
                );
            }
            2 => c.disconnect(),
            3 => {
                c.handle(Action::RefreshAccounts(c.ticket()), context());
            }
            4 => {
                c.continue_connection(Context {
                    visible: false,
                    ..context()
                });
            }
            _ => c.adopt_epoch(2),
        }
        c.completed(flight, Outcome::Accounts(added(0, 11)));
        assert!(c.connection.is_none());
        assert!(c.continue_connection(context()).is_none());
    }
}
#[test]
fn add_local_label_opens_once_but_never_selects_or_enables() {
    let mut c = loaded();
    c.handle(
        Action::EditLabel {
            ticket: c.ticket(),
            label: "PUBLIC new local alias".into(),
        },
        context(),
    );
    assert!(matches!(
        c.handle(Action::AddAccount(c.ticket()), context()),
        Some(Effect::AddAccount {
            expected_epoch: 0,
            ..
        })
    ));
    complete(&mut c, Outcome::Metadata(waiting()));
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::OpenBrowser { .. })
    ));
    complete(&mut c, Outcome::Confirmed);
    c.observed(authorized());
    assert!(c.continue_connection(context()).is_none());
    assert!(c.connection.is_none());
    assert!(c.managed.metadata.as_ref().unwrap().selected.is_none());
}
#[test]
fn empty_fresh_roster_rejects_signout_all_review_and_confirmation() {
    let mut c = loaded();
    c.managed.metadata.as_mut().unwrap().accounts.clear();
    let ticket = c.ticket();
    assert_eq!(c.account_epoch(), Some(0));
    assert!(!c.has_accounts());
    assert!(
        c.handle(Action::ReviewSignOutAll(ticket), context())
            .is_none()
    );
    assert!(c.managed.review.is_none());
    assert!(
        c.handle(Action::ConfirmAccount(ticket), context())
            .is_none()
    );
    assert!(c.flight.is_none());
    assert!(c.ticket() == ticket);
}
#[test]
fn signout_all_confirmation_rechecks_nonempty_roster() {
    let mut c = loaded();
    let ticket = c.ticket();
    c.handle(Action::ReviewSignOutAll(ticket), context());
    assert!(matches!(c.managed.review, Some(AccountReview::All { .. })));
    c.managed.metadata.as_mut().unwrap().accounts.clear();
    assert!(
        c.handle(Action::ConfirmAccount(ticket), context())
            .is_none()
    );
    assert!(c.flight.is_none());
    assert!(c.ticket() == ticket);
    assert!(c.managed.review.is_none());
}
#[test]
fn selected_account_switch_review_and_confirmation_do_not_admit_operation() {
    let mut c = loaded();
    c.managed.metadata.as_mut().unwrap().selected = Some(id(1));
    c.metadata = Some(CodexMetadata {
        credential_stored: true,
        ..local()
    });
    let ticket = c.ticket();
    assert!(
        c.handle(
            Action::ReviewSelect {
                ticket,
                account_id: id(1),
            },
            context(),
        )
        .is_none()
    );
    assert!(c.managed.review.is_none());
    assert!(
        c.handle(Action::ConfirmAccount(ticket), context())
            .is_none()
    );
    assert!(c.flight.is_none());
    assert!(c.ticket() == ticket);
    assert_eq!(c.account_epoch(), Some(0));
}
#[test]
fn selected_account_switch_confirmation_rechecks_current_target() {
    for selected_or_removed in 0..2 {
        let mut c = loaded();
        let ticket = c.ticket();
        c.handle(
            Action::ReviewSelect {
                ticket,
                account_id: id(1),
            },
            context(),
        );
        assert!(matches!(
            c.managed.review,
            Some(AccountReview::Select { .. })
        ));
        // Defensive current-metadata eligibility recheck; actual fresh receipts clear reviews.
        let current = c.managed.metadata.as_mut().unwrap();
        if selected_or_removed == 0 {
            current.selected = Some(id(1));
        } else {
            current.accounts.clear();
        }
        assert!(
            c.handle(Action::ConfirmAccount(ticket), context())
                .is_none()
        );
        assert!(c.flight.is_none());
        assert!(c.ticket() == ticket);
        assert!(c.managed.review.is_none());
        assert_eq!(c.account_epoch(), Some(0));
    }
}
#[test]
fn review_confirm_switch_logout_and_all_are_bound_to_current_committed_epoch() {
    for kind in 0..3 {
        let mut c = loaded();
        let t = c.ticket();
        let action = match kind {
            0 => Action::ReviewSelect {
                ticket: t,
                account_id: id(1),
            },
            1 => Action::ReviewSignOut {
                ticket: t,
                account_id: id(1),
            },
            _ => Action::ReviewSignOutAll(t),
        };
        assert!(c.handle(action, context()).is_none());
        let effect = c.handle(Action::ConfirmAccount(t), context()).unwrap();
        assert!(matches!(
            effect,
            Effect::SelectAccount {
                expected_epoch: 0,
                ..
            } | Effect::SignOutAccount {
                expected_epoch: 0,
                ..
            } | Effect::SignOutAll {
                expected_epoch: 0,
                ..
            }
        ));
        complete(&mut c, Outcome::Conflict);
        assert!(
            c.handle(Action::ConfirmAccount(c.ticket()), context())
                .is_none()
        );
        assert!(!c.managed.fresh);
    }
}
#[test]
fn stale_epoch_review_and_cross_panel_ack_are_not_published() {
    let mut c = loaded();
    let t = c.ticket();
    c.handle(
        Action::ReviewSelect {
            ticket: t,
            account_id: id(1),
        },
        context(),
    );
    c.managed.metadata.as_mut().unwrap().epoch = Some(1);
    assert!(c.handle(Action::ConfirmAccount(t), context()).is_none());
    c.handle(Action::RefreshAccounts(c.ticket()), context());
    let flight = c.operation_ticket().unwrap();
    c.leave();
    c.enter(1);
    c.completed(flight, Outcome::Accounts(accounts(99)));
    assert!(!c.managed.fresh);
    assert_ne!(c.managed.metadata.as_ref().unwrap().epoch, Some(99));
}
#[test]
fn unknown_mutations_remain_blocked_until_explicit_nonmutating_reconciliation() {
    for kind in 0..3 {
        let mut c = loaded();
        let t = c.ticket();
        let review = match kind {
            0 => Action::ReviewSelect {
                ticket: t,
                account_id: id(1),
            },
            1 => Action::ReviewSignOut {
                ticket: t,
                account_id: id(1),
            },
            _ => Action::ReviewSignOutAll(t),
        };
        c.handle(review, context());
        c.handle(Action::ConfirmAccount(t), context());
        complete(&mut c, Outcome::Indeterminate);
        assert!(c.managed.uncertain);
        assert!(c.pending());
        c.observed(local());
        assert!(c.managed.uncertain);
        c.handle(Action::RefreshAccounts(c.ticket()), context());
        complete(
            &mut c,
            Outcome::StatusSnapshot {
                metadata: local(),
                accounts: accounts(1),
            },
        );
        assert!(!c.managed.uncertain);
        assert!(!c.pending());
    }
}
#[test]
fn cancel_preempts_callback_ack_and_late_ack_never_resumes_connect() {
    let mut c = loaded();
    c.metadata = Some(waiting());
    c.fresh = true;
    let draft = c.draft_ticket().unwrap();
    c.handle(
        Action::Edit {
            ticket: draft,
            input: Input::new("PUBLIC_CALLBACK".into()),
        },
        context(),
    );
    let draft = c.draft_ticket().unwrap();
    c.handle(Action::ReviewCallback(draft), context());
    c.handle(Action::ConfirmCallback(draft), context());
    let callback = c.operation_ticket().unwrap();
    assert!(matches!(
        c.handle(
            Action::Cancel {
                ticket: c.ticket(),
                attempt: 11
            },
            context()
        ),
        Some(Effect::CancelLogin { .. })
    ));
    let cancel = c.operation_ticket().unwrap();
    c.completed(callback, Outcome::Confirmed);
    assert_eq!(c.operation_ticket(), Some(cancel));
    assert!(c.connection.is_none());
    c.completed(
        cancel,
        Outcome::Metadata(CodexMetadata {
            phase: CodexPhase::Cancelled,
            ..waiting()
        }),
    );
    assert!(c.owner_uncertain);
    assert!(c.continue_connection(context()).is_none());
}
#[test]
fn hidden_label_actions_never_retain_input_or_submit_and_callback_holder_is_consumed() {
    let mut c = loaded();
    c.handle(
        Action::EditLabel {
            ticket: c.ticket(),
            label: "PUBLIC hidden label".into(),
        },
        Context {
            visible: false,
            ..context()
        },
    );
    assert!(c.managed.label.is_empty());
    c.metadata = Some(waiting());
    c.fresh = true;
    let t = c.draft_ticket().unwrap();
    let input = Input::new("PUBLIC_CALLBACK".into());
    let replay = input.clone();
    c.handle(
        Action::Edit { ticket: t, input },
        Context {
            visible: false,
            ..context()
        },
    );
    c.handle(
        Action::Edit {
            ticket: t,
            input: replay,
        },
        context(),
    );
    assert!(c.draft.0.is_empty());
}

#[test]
fn unknown_add_and_missing_add_metadata_never_select_or_replay_after_reconciliation() {
    for outcome in [Outcome::Indeterminate, Outcome::Accounts(added(0, 11))] {
        let mut c = loaded();
        c.handle(
            Action::EditLabel {
                ticket: c.ticket(),
                label: "PUBLIC unknown add".into(),
            },
            context(),
        );
        c.handle(Action::AddAccount(c.ticket()), context());
        let original = c.operation_ticket().unwrap();
        complete(&mut c, outcome);
        assert!(c.owner_uncertain && c.managed.uncertain);
        assert!(c.connection.is_none());
        c.completed(original, Outcome::Metadata(waiting()));
        assert!(c.continue_connection(context()).is_none());
        c.handle(Action::RefreshAccounts(c.ticket()), context());
        complete(&mut c, Outcome::Indeterminate); // Core read remains unavailable while Add may run.
        assert!(c.owner_uncertain && c.managed.uncertain);
        c.handle(Action::RefreshAccounts(c.ticket()), context());
        complete(
            &mut c,
            Outcome::StatusSnapshot {
                metadata: local(),
                accounts: added(0, 11),
            },
        );
        assert!(!c.pending());
        assert!(c.managed.metadata.as_ref().unwrap().selected.is_none());
        assert!(c.continue_connection(context()).is_none());
    }
}

#[test]
fn stale_view_actions_and_wrong_mutation_reply_kinds_cannot_write_or_claim_success() {
    let mut c = loaded();
    let old = c.ticket();
    c.handle(
        Action::ReviewSignOut {
            ticket: old,
            account_id: id(1),
        },
        context(),
    );
    c.handle(Action::RefreshAccounts(old), context());
    complete(
        &mut c,
        Outcome::StatusSnapshot {
            metadata: local(),
            accounts: accounts(1),
        },
    );
    assert!(c.handle(Action::ConfirmAccount(old), context()).is_none());
    let current = c.ticket();
    c.handle(
        Action::ReviewSignOut {
            ticket: current,
            account_id: id(1),
        },
        context(),
    );
    c.handle(Action::ConfirmAccount(current), context());
    complete(&mut c, Outcome::Metadata(local()));
    assert!(c.managed.uncertain && !c.managed.fresh);
    assert!(matches!(c.notice, Notice::Indeterminate));
}
#[test]
fn account_consent_replaces_connection_and_account_list_in_actual_top_level_view() {
    for kind in 0..3 {
        let mut c = loaded();
        c.managed
            .metadata
            .as_mut()
            .unwrap()
            .accounts
            .push(CodexAccount {
                opaque_id: id(2),
                label: CodexAccountLabel::new("PUBLIC second account".into()).unwrap(),
                email: None,
                plan: None,
            });
        assert!(c.managed_review_view(context()).is_none());
        assert_eq!(c.view(context()).as_widget().children().len(), 3);
        let ticket = c.ticket();
        let action = match kind {
            0 => Action::ReviewSelect {
                ticket,
                account_id: id(2),
            },
            1 => Action::ReviewSignOut {
                ticket,
                account_id: id(1),
            },
            _ => Action::ReviewSignOutAll(ticket),
        };
        assert!(c.handle(action, context()).is_none());
        assert!(c.managed_review_view(context()).is_some());
        let view = c.view(context());
        let tree = view.as_widget().children();
        assert_eq!(tree.len(), 2); // Container delegates its tree: card heading and consent body only.
        assert_eq!(tree[1].children.len(), 1);
        assert_eq!(tree[1].children[0].children.len(), 4); // Consent, local target, warning, Confirm/Cancel.
        assert_eq!(tree[1].children[0].children[3].children.len(), 2); // No connection/account/tab actions inside consent.
        drop(view);
        assert!(c.flight.is_none());
        assert!(
            c.handle(Action::DismissAccount(ticket), context())
                .is_none()
        );
        assert!(c.managed_review_view(context()).is_none());
        assert_eq!(c.view(context()).as_widget().children().len(), 3);
    }
}

fn adding_local() -> Controller {
    let mut c = loaded();
    c.handle(
        Action::EditLabel {
            ticket: c.ticket(),
            label: "PUBLIC new local alias".into(),
        },
        context(),
    );
    assert!(matches!(
        c.handle(Action::AddAccount(c.ticket()), context()),
        Some(Effect::AddAccount { .. })
    ));
    assert!(!c.managed.fresh); // Admission invalidates the roster, before any ACK.
    complete(&mut c, Outcome::Metadata(waiting()));
    c
}
fn reconcile_added_without_selection(c: &mut Controller) {
    assert!(!c.managed.fresh);
    assert_eq!(c.account_epoch(), None);
    assert!(!c.managed.uncertain && !c.owner_uncertain);
    let old = c.ticket();
    for action in [
        Action::ReviewSelect {
            ticket: old,
            account_id: id(1),
        },
        Action::ReviewSignOut {
            ticket: old,
            account_id: id(1),
        },
        Action::ReviewSignOutAll(old),
    ] {
        assert!(c.handle(action, context()).is_none());
        assert!(c.managed.review.is_none() && c.flight.is_none());
        assert!(c.ticket() == old);
    }
    assert!(matches!(
        c.handle(Action::RefreshAccounts(old), context()),
        Some(Effect::AccountsRead { .. })
    ));
    let mut current = accounts(0); // Add does not increment the selection epoch.
    current.accounts.push(CodexAccount {
        opaque_id: id(2),
        label: CodexAccountLabel::new("PUBLIC newly added alias".into()).unwrap(),
        email: None,
        plan: None,
    });
    complete(
        c,
        Outcome::StatusSnapshot {
            metadata: authorized(),
            accounts: current,
        },
    );
    assert_eq!(c.account_epoch(), Some(0));
    assert_eq!(c.managed.metadata.as_ref().unwrap().accounts.len(), 2);
    assert!(c.managed.metadata.as_ref().unwrap().selected.is_none());
    assert!(c.managed.fresh && !c.pending());
    assert!(c.continue_connection(context()).is_none()); // No revived automatic Connect.
    let ticket = c.ticket();
    c.handle(
        Action::ReviewSelect {
            ticket,
            account_id: id(2),
        },
        context(),
    );
    assert!(matches!(
        c.managed.review,
        Some(AccountReview::Select { .. })
    ));
    assert!(c.flight.is_none()); // Review is not a write.
}
#[test]
fn managed_roster_freshness_failed_read_does_not_republish_cached_accounts() {
    for outcome in [Outcome::Indeterminate, Outcome::Refused, Outcome::NotSent] {
        let mut c = loaded();
        assert!(
            c.handle(Action::RefreshAccounts(c.ticket()), context())
                .is_some()
        );
        assert!(!c.managed.fresh);
        complete(&mut c, outcome);
        assert!(!c.pending()); // A failed read is not an uncertain account write.
        reconcile_added_without_selection(&mut c);
    }
}
#[test]
fn managed_roster_freshness_interrupted_add_requires_same_epoch_readback() {
    let mut c = adding_local();
    assert!(!c.managed.fresh);
    assert!(
        c.handle(Action::RefreshAccounts(c.ticket()), context())
            .is_some()
    );
    complete(&mut c, Outcome::Indeterminate); // Running login may refuse account reconciliation.
    c.observed(authorized());
    assert!(c.continue_connection(context()).is_none());
    assert!(c.connection.is_none() && !c.pending());
    reconcile_added_without_selection(&mut c);
}
#[test]
fn managed_roster_freshness_cancelled_add_does_not_restore_prelogin_roster() {
    let mut c = adding_local();
    assert!(!c.managed.fresh);
    assert!(matches!(
        c.handle(
            Action::Cancel {
                ticket: c.ticket(),
                attempt: 11
            },
            context()
        ),
        Some(Effect::CancelLogin { .. })
    ));
    complete(
        &mut c,
        Outcome::Metadata(CodexMetadata {
            phase: CodexPhase::Cancelled,
            ..waiting()
        }),
    );
    assert!(c.connection.is_none() && !c.pending());
    reconcile_added_without_selection(&mut c); // Cancel is not rollback of a possibly stored Add.
}
