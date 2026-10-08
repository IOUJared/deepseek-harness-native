use super::*;
fn context() -> Context {
    Context {
        epoch: 1,
        visible: true,
        enabled: true,
    }
}
fn metadata() -> CodexMetadata {
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
        prompt: Some(12),
        browser_available: true,
        ..metadata()
    }
}
fn authorized() -> CodexMetadata {
    CodexMetadata {
        phase: CodexPhase::Authorized,
        credential_stored: true,
        attempt: Some(11),
        ..metadata()
    }
}
fn checked(m: CodexMetadata) -> Controller {
    let mut c = Controller::default();
    c.enter(1);
    assert!(matches!(
        c.handle(Action::CheckStatus(c.ticket()), context()),
        Some(Effect::StatusRead { .. })
    ));
    c.completed(c.operation_ticket().unwrap(), Outcome::Metadata(m));
    c
}
fn edit(c: &mut Controller, raw: &str) -> DraftTicket {
    let ticket = c.draft_ticket().unwrap();
    assert!(
        c.handle(
            Action::Edit {
                ticket,
                input: Input::new(raw.into())
            },
            context()
        )
        .is_none()
    );
    c.draft_ticket().unwrap()
}
#[test]
fn navigation_and_ready_never_issue_operations() {
    let mut c = Controller::default();
    c.enter(1);
    c.adopt_epoch(2);
    c.leave();
    c.enter(2);
    assert!(c.operation_ticket().is_none());
    assert!(c.metadata.is_none());
    assert!(!c.pending());
    assert!(
        c.handle(
            Action::Start(c.ticket()),
            Context {
                epoch: 2,
                ..context()
            }
        )
        .is_none()
    );
}
#[test]
fn hidden_disabled_and_wrong_epoch_actions_have_no_effects() {
    for ctx in [
        Context {
            visible: false,
            ..context()
        },
        Context {
            enabled: false,
            ..context()
        },
        Context {
            epoch: 2,
            ..context()
        },
    ] {
        let mut c = checked(metadata());
        assert!(c.handle(Action::CheckStatus(c.ticket()), ctx).is_none());
        assert!(c.handle(Action::Start(c.ticket()), ctx).is_none());
        assert!(c.operation_ticket().is_none());
    }
}
#[test]
fn single_short_operation_and_stale_completion_fences() {
    let mut c = checked(metadata());
    let previous = c.ticket();
    assert!(matches!(
        c.handle(Action::Start(previous), context()),
        Some(Effect::StartLogin { .. })
    ));
    let ticket = c.operation_ticket().unwrap();
    assert!(c.handle(Action::Start(c.ticket()), context()).is_none());
    c.completed(previous, Outcome::Metadata(authorized()));
    assert_eq!(c.operation_ticket(), Some(ticket));
    c.completed(ticket, Outcome::Metadata(waiting()));
    assert!(c.pending());
    assert!(c.operation_ticket().is_none());
}
#[test]
fn wrong_attempt_observation_is_ignored() {
    let mut c = checked(waiting());
    let t = edit(&mut c, "callback-secret");
    c.observed(CodexMetadata {
        attempt: Some(99),
        prompt: Some(100),
        ..authorized()
    });
    assert_eq!(c.draft_ticket(), Some(t));
    assert_eq!(c.draft.0, "callback-secret");
    assert!(matches!(
        c.metadata.unwrap().phase,
        CodexPhase::WaitingBrowser
    ));
}
#[test]
fn prompt_withdrawal_clears_secret_and_review() {
    let mut c = checked(waiting());
    let t = edit(&mut c, "callback-secret");
    c.handle(Action::ReviewCallback(t), context());
    c.observed(CodexMetadata {
        prompt: None,
        ..waiting()
    });
    assert!(c.draft.0.is_empty());
    assert!(c.review.is_none());
    assert!(c.draft_ticket().is_none());
    assert!(c.handle(Action::ConfirmCallback(t), context()).is_none());
}
#[test]
fn prompt_replacement_never_retargets_draft() {
    let mut c = checked(waiting());
    let t = edit(&mut c, "callback-secret");
    c.observed(CodexMetadata {
        prompt: Some(77),
        ..waiting()
    });
    assert!(c.draft.0.is_empty());
    assert_ne!(c.draft_ticket(), Some(t));
    assert!(
        c.handle(
            Action::Edit {
                ticket: t,
                input: Input::new("old-secret".into())
            },
            context()
        )
        .is_none()
    );
    assert!(c.draft.0.is_empty());
}
#[test]
fn cloned_input_cannot_replay_plaintext() {
    let mut c = checked(waiting());
    let input = Input::new("secret-once".into());
    let copy = input.clone();
    let t = c.draft_ticket().unwrap();
    c.handle(Action::Edit { ticket: t, input }, context());
    let now = c.draft_ticket().unwrap();
    c.handle(
        Action::Edit {
            ticket: now,
            input: copy,
        },
        context(),
    );
    assert_eq!(c.draft_ticket(), Some(now));
    assert_eq!(c.draft.0, "secret-once");
}
#[test]
fn hidden_input_is_consumed_not_replayable() {
    let mut c = checked(waiting());
    let input = Input::new("secret-once".into());
    let copy = input.clone();
    let t = c.draft_ticket().unwrap();
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
            input: copy,
        },
        context(),
    );
    assert!(c.draft.0.is_empty());
}
#[test]
fn invalid_paste_wipes_previous_draft_without_trimming() {
    for raw in [
        "has space".to_string(),
        "\n".to_string(),
        "é".to_string(),
        "x".repeat(MAX_INPUT + 1),
    ] {
        let mut c = checked(waiting());
        edit(&mut c, "previous-secret");
        let t = c.draft_ticket().unwrap();
        c.handle(
            Action::Edit {
                ticket: t,
                input: Input::new(raw),
            },
            context(),
        );
        assert!(c.draft.0.is_empty());
        assert!(matches!(c.notice, Notice::Invalid));
    }
}
#[test]
fn debug_faces_never_show_secret() {
    let mut c = checked(waiting());
    let t = edit(&mut c, "private-callback-marker");
    let action = Action::Edit {
        ticket: t,
        input: Input::new("private-callback-marker".into()),
    };
    for debug in [
        format!("{c:?}"),
        format!("{:?}", c.draft),
        format!("{action:?}"),
    ] {
        assert!(!debug.contains("private-callback-marker"));
        assert!(debug.contains("REDACTED"));
    }
}
#[test]
fn callback_requires_review_and_is_consumed_once() {
    let mut c = checked(waiting());
    let t = edit(&mut c, "callback-secret");
    assert!(c.handle(Action::ConfirmCallback(t), context()).is_none());
    c.handle(Action::ReviewCallback(t), context());
    let effect = c.handle(Action::ConfirmCallback(t), context());
    assert!(matches!(
        effect,
        Some(Effect::SubmitCallback {
            attempt: 11,
            prompt: 12,
            ..
        })
    ));
    assert!(c.draft.0.is_empty());
    assert!(c.review.is_none());
    assert!(c.handle(Action::ConfirmCallback(t), context()).is_none());
    c.completed(c.operation_ticket().unwrap(), Outcome::Confirmed);
    assert!(matches!(c.notice, Notice::Submitted));
    assert!(c.draft_ticket().is_none());
    assert!(!c.metadata.unwrap().credential_stored);
}
#[test]
fn stale_editor_review_cannot_confirm_replacement() {
    let mut c = checked(waiting());
    let old = edit(&mut c, "old-secret");
    let now = edit(&mut c, "new-secret");
    c.handle(Action::ReviewCallback(old), context());
    assert!(c.review.is_none());
    c.handle(Action::ReviewCallback(now), context());
    assert!(c.handle(Action::ConfirmCallback(old), context()).is_none());
    assert_eq!(c.draft.0, "new-secret");
}
#[test]
fn browser_open_requires_current_attempt_and_link() {
    let mut c = checked(waiting());
    let t = c.ticket();
    assert!(
        c.handle(
            Action::OpenBrowser {
                ticket: t,
                attempt: 99
            },
            context()
        )
        .is_none()
    );
    assert!(matches!(
        c.handle(
            Action::OpenBrowser {
                ticket: t,
                attempt: 11
            },
            context()
        ),
        Some(Effect::OpenBrowser { attempt: 11, .. })
    ));
    c.completed(c.operation_ticket().unwrap(), Outcome::Confirmed);
    assert!(matches!(c.notice, Notice::BrowserOpened));
    assert!(!c.metadata.unwrap().credential_stored);
    c.observed(CodexMetadata {
        browser_available: false,
        ..waiting()
    });
    assert!(
        c.handle(
            Action::OpenBrowser {
                ticket: c.ticket(),
                attempt: 11
            },
            context()
        )
        .is_none()
    );
}
#[test]
fn cancellation_is_not_credential_deletion_or_automatic_retry() {
    let mut c = checked(waiting());
    assert!(matches!(
        c.handle(
            Action::Cancel {
                ticket: c.ticket(),
                attempt: 11
            },
            context()
        ),
        Some(Effect::CancelLogin { attempt: 11, .. })
    ));
    c.completed(
        c.operation_ticket().unwrap(),
        Outcome::Metadata(CodexMetadata {
            phase: CodexPhase::Cancelled,
            retry_blocked: true,
            credential_stored: true,
            prompt: None,
            ..waiting()
        }),
    );
    assert!(c.pending());
    assert!(c.draft_ticket().is_none());
    assert!(c.handle(Action::Start(c.ticket()), context()).is_none());
    assert!(
        c.handle(Action::ReviewEnable(c.ticket()), context())
            .is_none()
    );
    assert!(matches!(c.notice, Notice::Cancelled));
}
#[test]
fn leaving_preserves_login_blocking_without_hydrating_new_panel() {
    let mut c = checked(waiting());
    edit(&mut c, "secret");
    c.leave();
    assert!(c.pending());
    assert!(c.draft.0.is_empty());
    c.observed(CodexMetadata {
        prompt: Some(55),
        ..waiting()
    });
    c.enter(1);
    assert!(c.pending());
    assert!(c.draft_ticket().is_none());
    assert!(
        c.handle(
            Action::OpenBrowser {
                ticket: c.ticket(),
                attempt: 11
            },
            context()
        )
        .is_none()
    );
    assert!(matches!(
        c.handle(Action::CheckStatus(c.ticket()), context()),
        Some(Effect::StatusRead { .. })
    ));
    c.completed(c.operation_ticket().unwrap(), Outcome::Metadata(waiting()));
    assert!(c.draft_ticket().is_some());
}
#[test]
fn earlier_panel_receipt_retires_without_adopting_metadata() {
    let mut c = checked(metadata());
    c.handle(Action::CheckStatus(c.ticket()), context());
    let t = c.operation_ticket().unwrap();
    c.leave();
    c.enter(1);
    c.completed(t, Outcome::Metadata(waiting()));
    assert!(c.operation_ticket().is_none());
    assert!(c.draft_ticket().is_none());
    assert!(matches!(c.metadata.unwrap().phase, CodexPhase::Idle));
    assert!(matches!(c.notice, Notice::Earlier));
}
#[test]
fn epoch_change_and_disconnect_reject_old_receipts() {
    let mut c = checked(metadata());
    c.handle(Action::Start(c.ticket()), context());
    let t = c.operation_ticket().unwrap();
    c.adopt_epoch(2);
    c.completed(t, Outcome::Metadata(authorized()));
    assert!(c.metadata.is_none());
    assert!(matches!(c.notice, Notice::Indeterminate));
    c.disconnect();
    c.observed(waiting());
    assert!(c.metadata.is_none());
}
#[test]
fn terminal_before_start_ack_wins_over_waiting_snapshot() {
    let mut c = checked(metadata());
    c.handle(Action::Start(c.ticket()), context());
    let t = c.operation_ticket().unwrap();
    c.observed(authorized());
    c.observed(waiting());
    c.completed(t, Outcome::Metadata(waiting()));
    assert!(matches!(c.metadata.unwrap().phase, CodexPhase::Authorized));
    assert!(c.metadata.unwrap().credential_stored);
    assert!(!c.pending());
}
#[test]
fn deferred_wrong_attempt_cannot_replace_start_receipt() {
    let mut c = checked(metadata());
    c.handle(Action::Start(c.ticket()), context());
    let t = c.operation_ticket().unwrap();
    c.observed(CodexMetadata {
        attempt: Some(99),
        ..authorized()
    });
    c.completed(t, Outcome::Metadata(waiting()));
    assert_eq!(c.metadata.unwrap().attempt, Some(11));
    assert!(matches!(
        c.metadata.unwrap().phase,
        CodexPhase::WaitingBrowser
    ));
}
#[test]
fn stale_start_reply_keeps_uncertainty_until_explicit_check() {
    let mut c = checked(metadata());
    c.handle(Action::Start(c.ticket()), context());
    let t = c.operation_ticket().unwrap();
    c.leave();
    c.enter(1);
    c.completed(t, Outcome::Metadata(waiting()));
    assert!(c.pending());
    assert!(!c.fresh);
    c.handle(Action::CheckStatus(c.ticket()), context());
    c.completed(
        c.operation_ticket().unwrap(),
        Outcome::Metadata(authorized()),
    );
    assert!(!c.pending());
}
#[test]
fn enable_requires_review_and_captured_revision() {
    let mut c = checked(authorized());
    let t = c.ticket();
    assert!(c.handle(Action::ConfirmEnable(t), context()).is_none());
    c.handle(Action::ReviewEnable(t), context());
    assert!(matches!(
        c.handle(Action::ConfirmEnable(t), context()),
        Some(Effect::EnableModels {
            expected_revision: 7,
            ..
        })
    ));
    assert!(c.handle(Action::ConfirmEnable(t), context()).is_none());
    c.completed(
        c.operation_ticket().unwrap(),
        Outcome::Metadata(CodexMetadata {
            route_configured: true,
            settings_revision: Some(8),
            ..authorized()
        }),
    );
    assert!(matches!(c.notice, Notice::Enabled));
}
#[test]
fn enable_boolean_ack_is_not_route_registration_confirmation() {
    let mut c = checked(authorized());
    let t = c.ticket();
    c.handle(Action::ReviewEnable(t), context());
    c.handle(Action::ConfirmEnable(t), context());
    c.completed(c.operation_ticket().unwrap(), Outcome::Confirmed);
    assert!(matches!(c.notice, Notice::Indeterminate));
    assert!(!c.metadata.unwrap().route_configured);
}
#[test]
fn changed_revision_withdraws_enable_review() {
    let mut c = checked(authorized());
    let t = c.ticket();
    c.handle(Action::ReviewEnable(t), context());
    c.observed(CodexMetadata {
        settings_revision: Some(8),
        ..authorized()
    });
    assert!(c.review.is_none());
    assert!(c.handle(Action::ConfirmEnable(t), context()).is_none());
}
#[test]
fn existing_route_or_missing_revision_has_no_enable_effect() {
    for m in [
        CodexMetadata {
            route_configured: true,
            ..authorized()
        },
        CodexMetadata {
            settings_revision: None,
            ..authorized()
        },
    ] {
        let mut c = checked(m);
        let t = c.ticket();
        c.handle(Action::ReviewEnable(t), context());
        assert!(c.review.is_none());
        assert!(c.handle(Action::ConfirmEnable(t), context()).is_none());
    }
}
#[test]
fn conflict_clears_metadata_and_requires_explicit_read() {
    let mut c = checked(authorized());
    let t = c.ticket();
    c.handle(Action::ReviewEnable(t), context());
    c.handle(Action::ConfirmEnable(t), context());
    c.completed(c.operation_ticket().unwrap(), Outcome::Conflict);
    assert!(c.metadata.is_none());
    assert!(c.review.is_none());
    assert!(
        c.handle(Action::ReviewEnable(c.ticket()), context())
            .is_none()
    );
}
#[test]
fn not_sent_start_releases_detached_block_without_retry() {
    let mut c = checked(metadata());
    c.handle(Action::Start(c.ticket()), context());
    c.completed(c.operation_ticket().unwrap(), Outcome::NotSent);
    assert!(!c.pending());
    assert!(c.operation_ticket().is_none());
}
#[test]
fn unknown_auth_owner_remains_blocked_across_status_hide_and_terminal_observation() {
    for operation in [Operation::Start, Operation::Callback, Operation::Cancel] {
        let mut c = checked(waiting());
        let ticket = c.begin(operation);
        c.leave(); // The owner safety receipt applies even when its panel was retired.
        c.completed(ticket, Outcome::Indeterminate);
        assert!(c.owner_uncertain);
        c.enter(1);
        c.handle(Action::CheckStatus(c.ticket()), context());
        c.completed(c.operation_ticket().unwrap(), Outcome::Metadata(waiting()));
        assert!(c.pending());
        assert!(c.draft_ticket().is_none());
        assert!(
            c.handle(
                Action::OpenBrowser {
                    ticket: c.ticket(),
                    attempt: 11
                },
                context()
            )
            .is_none()
        );
        c.observed(authorized());
        assert!(c.pending());
        assert!(c.handle(Action::Start(c.ticket()), context()).is_none());
        assert!(
            c.handle(Action::ReviewEnable(c.ticket()), context())
                .is_none()
        );
        assert!(matches!(
            c.handle(
                Action::Cancel {
                    ticket: c.ticket(),
                    attempt: 11
                },
                context()
            ),
            Some(Effect::CancelLogin { attempt: 11, .. })
        ));
        c.completed(
            c.operation_ticket().unwrap(),
            Outcome::Metadata(authorized()),
        );
        assert!(c.owner_uncertain);
        c.adopt_epoch(2);
        assert!(!c.owner_uncertain);
        assert!(!c.pending());
    }
}
#[test]
fn unknown_read_or_browser_does_not_poison_auth_owner() {
    for operation in [Operation::Status, Operation::Browser] {
        let mut c = checked(metadata());
        let ticket = c.begin(operation);
        c.completed(ticket, Outcome::Indeterminate);
        assert!(!c.owner_uncertain);
        assert!(!c.pending());
        c.handle(Action::CheckStatus(c.ticket()), context());
        c.completed(c.operation_ticket().unwrap(), Outcome::Metadata(metadata()));
        assert!(matches!(
            c.handle(Action::Start(c.ticket()), context()),
            Some(Effect::StartLogin { .. })
        ));
    }
}
#[test]
fn unknown_enable_releases_flight_but_requires_explicit_fresh_status() {
    let mut c = checked(authorized());
    let t = c.ticket();
    c.handle(Action::ReviewEnable(t), context());
    c.handle(Action::ConfirmEnable(t), context());
    c.completed(c.operation_ticket().unwrap(), Outcome::Indeterminate);
    assert!(!c.owner_uncertain);
    assert!(!c.pending());
    assert!(c.metadata.is_none());
    assert!(!c.fresh);
    c.observed(authorized()); // Unsolicited metadata cannot replace the required explicit read.
    assert!(c.metadata.is_none());
    assert!(
        c.handle(Action::ReviewEnable(c.ticket()), context())
            .is_none()
    );
    c.handle(Action::CheckStatus(c.ticket()), context());
    c.completed(
        c.operation_ticket().unwrap(),
        Outcome::Metadata(authorized()),
    );
    c.handle(Action::ReviewEnable(c.ticket()), context());
    assert!(matches!(c.review, Some(Review::Enable { .. })));
}
#[test]
fn focused_reviews_use_separate_view_without_issuing_operations() {
    let mut callback = checked(waiting());
    let t = edit(&mut callback, "PUBLIC_FAKE_CALLBACK");
    callback.handle(Action::ReviewCallback(t), context());
    assert!(callback.review_view(context()).is_some());
    assert!(callback.operation_ticket().is_none());
    callback.handle(Action::DismissReview(callback.ticket()), context());
    assert!(callback.review_view(context()).is_none());
    let mut enable = checked(authorized());
    enable.handle(Action::ReviewEnable(enable.ticket()), context());
    assert!(enable.review_view(context()).is_some());
    assert!(enable.operation_ticket().is_none());
    enable.observed(CodexMetadata {
        settings_revision: Some(8),
        ..authorized()
    });
    assert!(enable.review_view(context()).is_none());
}
