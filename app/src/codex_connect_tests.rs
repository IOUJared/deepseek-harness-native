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
        attempt: Some(11),
        credential_stored: true,
        ..metadata()
    }
}
fn enabled() -> CodexMetadata {
    CodexMetadata {
        route_configured: true,
        settings_revision: Some(8),
        ..authorized()
    }
}
fn connect() -> Controller {
    let mut c = Controller::default();
    c.enter(1);
    assert!(!c.advanced);
    assert!(matches!(
        c.handle(Action::Connect(c.ticket()), context()),
        Some(Effect::StatusRead { .. })
    ));
    c
}
fn complete(c: &mut Controller, outcome: Outcome) {
    c.completed(c.operation_ticket().unwrap(), outcome);
}
fn starting() -> Controller {
    let mut c = connect();
    complete(&mut c, Outcome::Metadata(metadata()));
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::StartLogin { .. })
    ));
    c
}
fn opening() -> Controller {
    let mut c = starting();
    complete(&mut c, Outcome::Metadata(waiting()));
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::OpenBrowser { attempt: 11, .. })
    ));
    c
}
fn authenticating() -> Controller {
    let mut c = opening();
    complete(&mut c, Outcome::Confirmed);
    assert!(c.continue_connection(context()).is_none());
    c
}
fn enabling() -> Controller {
    let mut c = authenticating();
    c.observed(authorized());
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::EnableModels {
            expected_revision: 7,
            ..
        })
    ));
    c
}

#[test]
fn explicit_click_runs_local_status_start_browser_authorized_enable_once() {
    let mut c = enabling();
    assert!(c.pending());
    complete(&mut c, Outcome::Metadata(enabled()));
    assert!(matches!(c.notice, Notice::Connected));
    assert!(c.connection.is_none());
    assert!(!c.pending());
    assert!(c.continue_connection(context()).is_none());
    c.observed(enabled());
    assert!(c.continue_connection(context()).is_none());
}

#[test]
fn no_automation_on_enter_status_or_legacy_start() {
    let mut c = Controller::default();
    c.enter(1);
    assert!(c.continue_connection(context()).is_none());
    c.handle(Action::CheckStatus(c.ticket()), context());
    complete(&mut c, Outcome::Metadata(metadata()));
    assert!(c.continue_connection(context()).is_none());
    c.handle(Action::Start(c.ticket()), context());
    complete(&mut c, Outcome::Metadata(waiting()));
    assert!(c.continue_connection(context()).is_none());
    c.observed(authorized());
    assert!(c.continue_connection(context()).is_none());
}

#[test]
fn repeated_metadata_never_reopens_browser_or_starts_another_flight() {
    let mut c = opening();
    let flight = c.operation_ticket();
    for _ in 0..4 {
        c.observed(waiting());
        assert!(c.continue_connection(context()).is_none());
        assert_eq!(c.operation_ticket(), flight);
    }
    complete(&mut c, Outcome::Confirmed);
    for _ in 0..4 {
        c.observed(waiting());
        assert!(c.continue_connection(context()).is_none());
        assert!(c.operation_ticket().is_none());
    }
}

#[test]
fn already_configured_is_only_a_local_status_read() {
    let mut c = connect();
    complete(&mut c, Outcome::Metadata(enabled()));
    assert!(c.continue_connection(context()).is_none());
    assert!(matches!(c.notice, Notice::Configured));
    assert!(c.notice.text().contains("not account validation"));
    assert!(c.connection.is_none());
    assert!(c.operation_ticket().is_none());
}

#[test]
fn existing_credential_missing_route_enables_without_reauthentication() {
    // An existing credential need not carry an Authorized attempt from this click.
    let mut c = connect();
    let local = CodexMetadata {
        credential_stored: true,
        ..metadata()
    };
    complete(&mut c, Outcome::Metadata(local));
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
            route_configured: true,
            settings_revision: Some(8),
            ..local
        }),
    );
    assert!(matches!(c.notice, Notice::Connected));
    assert!(c.connection.is_none());
}

#[test]
fn existing_route_is_never_replaced_after_new_login() {
    let mut c = authenticating();
    c.observed(CodexMetadata {
        route_configured: true,
        ..authorized()
    });
    assert!(c.continue_connection(context()).is_none());
    assert!(matches!(c.notice, Notice::Connected));
    assert!(c.operation_ticket().is_none());
}

#[test]
fn old_and_mismatched_terminals_cannot_authorize_enable() {
    let mut c = starting();
    c.observed(CodexMetadata {
        attempt: Some(99),
        ..authorized()
    });
    complete(&mut c, Outcome::Metadata(waiting()));
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::OpenBrowser { attempt: 11, .. })
    ));
    complete(&mut c, Outcome::Confirmed);
    c.observed(CodexMetadata {
        attempt: Some(99),
        ..authorized()
    });
    assert!(c.continue_connection(context()).is_none());
    assert_eq!(c.metadata.unwrap().attempt, Some(11));
    // Credentials alone, without the owned Authorized terminal, do not enable models.
    c.observed(CodexMetadata {
        credential_stored: true,
        ..waiting()
    });
    assert!(c.continue_connection(context()).is_none());
    assert!(c.operation_ticket().is_none());
}

#[test]
fn early_terminal_before_start_ack_is_preserved_and_needs_no_browser() {
    let mut c = starting();
    c.observed(authorized());
    c.observed(waiting());
    assert!(c.continue_connection(context()).is_none());
    complete(&mut c, Outcome::Metadata(waiting()));
    assert!(matches!(c.metadata.unwrap().phase, CodexPhase::Authorized));
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::EnableModels { .. })
    ));
}

#[test]
fn older_terminal_then_owned_terminal_before_start_ack_keeps_owned_authorization() {
    let mut c = starting();
    c.observed(CodexMetadata {
        attempt: Some(10),
        ..authorized()
    });
    c.observed(authorized());
    c.observed(waiting());
    complete(&mut c, Outcome::Metadata(waiting()));
    assert_eq!(c.metadata.unwrap().attempt, Some(11));
    assert!(matches!(c.metadata.unwrap().phase, CodexPhase::Authorized));
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::EnableModels { .. })
    ));
}

#[test]
fn owned_terminal_then_older_terminal_before_start_ack_keeps_owned_authorization() {
    let mut c = starting();
    c.observed(authorized());
    c.observed(CodexMetadata {
        attempt: Some(10),
        ..authorized()
    });
    c.observed(waiting());
    complete(&mut c, Outcome::Metadata(waiting()));
    assert_eq!(c.metadata.unwrap().attempt, Some(11));
    assert!(matches!(c.metadata.unwrap().phase, CodexPhase::Authorized));
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::EnableModels { .. })
    ));
}

#[test]
fn early_terminal_before_browser_ack_waits_for_exact_confirmed_receipt() {
    let mut c = opening();
    let opener = c.operation_ticket().unwrap();
    c.observed(authorized());
    assert!(c.ticket().serial > opener.serial);
    c.observed(waiting()); // A stale waiting snapshot must not downgrade that terminal.
    assert!(matches!(c.metadata.unwrap().phase, CodexPhase::Authorized));
    assert!(c.continue_connection(context()).is_none());
    c.completed(c.ticket(), Outcome::Confirmed); // Current view ticket is not the flight ticket.
    assert_eq!(c.operation_ticket(), Some(opener));
    assert!(c.continue_connection(context()).is_none());
    c.completed(opener, Outcome::Confirmed);
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::EnableModels { .. })
    ));
}

#[test]
fn start_receipt_without_attempt_never_adopts_deferred_attempt() {
    let mut c = starting();
    c.observed(authorized());
    complete(&mut c, Outcome::Metadata(metadata()));
    assert!(c.connection.is_none());
    assert!(c.continue_connection(context()).is_none());
    assert!(c.metadata.unwrap().attempt.is_none());
}

#[test]
fn browser_waits_for_exact_owned_link_availability() {
    let mut c = starting();
    complete(
        &mut c,
        Outcome::Metadata(CodexMetadata {
            browser_available: false,
            ..waiting()
        }),
    );
    assert!(c.continue_connection(context()).is_none());
    c.observed(CodexMetadata {
        attempt: Some(99),
        ..waiting()
    });
    assert!(c.continue_connection(context()).is_none());
    c.observed(waiting());
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::OpenBrowser { attempt: 11, .. })
    ));
}

#[test]
fn enable_uses_current_revision_not_start_revision() {
    let mut c = authenticating();
    c.observed(CodexMetadata {
        settings_revision: Some(42),
        ..authorized()
    });
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::EnableModels {
            expected_revision: 42,
            ..
        })
    ));
}

#[test]
fn all_failure_receipts_end_every_stage_without_retry() {
    for stage in 0..4 {
        for outcome in [
            Outcome::Refused,
            Outcome::Conflict,
            Outcome::Indeterminate,
            Outcome::NotSent,
        ] {
            let mut c = match stage {
                0 => connect(),
                1 => starting(),
                2 => opening(),
                _ => enabling(),
            };
            // Fresh metadata (including a terminal before opener ACK) is not a receipt.
            if stage == 2 || stage == 3 {
                c.observed(authorized());
            }
            complete(&mut c, outcome);
            assert!(c.connection.is_none());
            assert!(c.operation_ticket().is_none());
            assert!(c.continue_connection(context()).is_none());
            c.observed(enabled());
            assert!(c.continue_connection(context()).is_none());
        }
    }
}

#[test]
fn queue_failure_stops_even_after_fresh_route_metadata() {
    let mut c = enabling();
    let flight = c.operation_ticket().unwrap();
    c.observed(enabled());
    assert!(c.ticket().serial > flight.serial);
    c.completed(flight, Outcome::NotSent);
    assert!(matches!(c.notice, Notice::NotSent));
    assert!(c.connection.is_none());
    assert!(c.continue_connection(context()).is_none());
}

#[test]
fn cached_metadata_cannot_skip_status_receipt_and_queue_failure_stops() {
    let mut c = Controller::default();
    c.enter(1);
    c.handle(Action::CheckStatus(c.ticket()), context());
    complete(&mut c, Outcome::Metadata(authorized()));
    c.handle(Action::Connect(c.ticket()), context());
    c.observed(enabled());
    assert!(c.continue_connection(context()).is_none());
    complete(&mut c, Outcome::NotSent);
    assert!(c.connection.is_none());
    assert!(c.continue_connection(context()).is_none());
    assert!(matches!(c.notice, Notice::NotSent));
}

#[test]
fn deferred_authorized_does_not_override_failed_start_receipt() {
    for outcome in [Outcome::NotSent, Outcome::Refused, Outcome::Indeterminate] {
        let mut c = starting();
        c.observed(authorized());
        assert!(c.continue_connection(context()).is_none());
        complete(&mut c, outcome);
        assert!(c.connection.is_none());
        assert!(c.continue_connection(context()).is_none());
        c.observed(authorized());
        assert!(c.continue_connection(context()).is_none());
    }
}

#[test]
fn wrong_receipt_kinds_do_not_advance_chain() {
    for stage in 0..4 {
        let mut c = match stage {
            0 => connect(),
            1 => starting(),
            2 => opening(),
            _ => enabling(),
        };
        let outcome = if stage == 2 {
            Outcome::Metadata(authorized())
        } else {
            Outcome::Confirmed
        };
        complete(&mut c, outcome);
        assert!(c.connection.is_none());
        assert!(c.continue_connection(context()).is_none());
    }
}

#[test]
fn unsuccessful_enable_metadata_is_not_connected() {
    for m in [
        authorized(),
        CodexMetadata {
            route_configured: true,
            ..authorized()
        },
        CodexMetadata {
            attempt: Some(99),
            ..enabled()
        },
    ] {
        let mut c = enabling();
        complete(&mut c, Outcome::Metadata(m));
        assert!(c.connection.is_none());
        assert!(!matches!(c.notice, Notice::Connected));
        assert!(c.continue_connection(context()).is_none());
    }
}

#[test]
fn local_status_receipt_cannot_be_replaced_by_unsolicited_credential_metadata() {
    let mut c = connect();
    complete(&mut c, Outcome::Metadata(metadata()));
    c.observed(CodexMetadata {
        credential_stored: true,
        ..metadata()
    });
    assert!(c.continue_connection(context()).is_none());
    assert!(c.connection.is_none());
    assert!(c.operation_ticket().is_none());
}

#[test]
fn blocked_unavailable_active_or_revisionless_configuration_stops() {
    for m in [
        CodexMetadata {
            available: false,
            ..metadata()
        },
        CodexMetadata {
            retry_blocked: true,
            ..metadata()
        },
        waiting(),
        CodexMetadata {
            credential_stored: true,
            settings_revision: None,
            ..metadata()
        },
    ] {
        let mut c = connect();
        complete(&mut c, Outcome::Metadata(m));
        assert!(c.continue_connection(context()).is_none());
        assert!(c.connection.is_none());
    }
}

#[test]
fn owned_failed_cancelled_indeterminate_or_retry_blocked_never_enables() {
    for m in [
        CodexMetadata {
            phase: CodexPhase::Failed,
            ..authorized()
        },
        CodexMetadata {
            phase: CodexPhase::Cancelled,
            ..authorized()
        },
        CodexMetadata {
            phase: CodexPhase::Indeterminate,
            ..authorized()
        },
        CodexMetadata {
            retry_blocked: true,
            ..authorized()
        },
        CodexMetadata {
            credential_stored: false,
            ..authorized()
        },
    ] {
        let mut c = authenticating();
        c.observed(m);
        assert!(c.continue_connection(context()).is_none());
        assert!(c.connection.is_none());
    }
}

#[test]
fn disabled_hidden_or_wrong_epoch_context_never_admits_or_resumes() {
    for ctx in [
        Context {
            enabled: false,
            ..context()
        },
        Context {
            visible: false,
            ..context()
        },
        Context {
            epoch: 2,
            ..context()
        },
    ] {
        let mut empty = Controller::default();
        empty.enter(1);
        assert!(empty.handle(Action::Connect(empty.ticket()), ctx).is_none());
        assert!(empty.connection.is_none());
        let mut c = connect();
        complete(&mut c, Outcome::Metadata(metadata()));
        assert!(c.continue_connection(ctx).is_none());
        assert!(c.connection.is_none());
        assert!(c.continue_connection(context()).is_none());
    }
}

#[test]
fn leave_page_reopen_epoch_and_disconnect_cancel_all_automation_stages() {
    for stage in 0..7 {
        for lifecycle in 0..4 {
            let mut c = match stage {
                0 => connect(),
                1 => {
                    let mut c = connect();
                    complete(&mut c, Outcome::Metadata(metadata()));
                    c
                }
                2 => starting(),
                3 => {
                    let mut c = starting();
                    complete(&mut c, Outcome::Metadata(waiting()));
                    c
                }
                4 => opening(),
                5 => authenticating(),
                _ => enabling(),
            };
            let flight = c.operation_ticket();
            match lifecycle {
                0 => c.leave(),
                1 => {
                    c.leave();
                    c.enter(1);
                } // Page switch followed by reopening the same section.
                2 => c.adopt_epoch(2),
                _ => c.disconnect(),
            }
            if let Some(ticket) = flight {
                c.completed(ticket, Outcome::Metadata(authorized()));
            }
            c.observed(authorized());
            assert!(c.connection.is_none());
            assert!(c.continue_connection(context()).is_none());
            assert!(c.operation_ticket().is_none());
        }
    }
}

#[test]
fn cancel_revokes_intent_and_late_authorized_never_retries() {
    let mut c = authenticating();
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
    assert!(c.connection.is_none());
    c.observed(authorized());
    complete(&mut c, Outcome::Metadata(authorized()));
    assert!(c.continue_connection(context()).is_none());
    assert!(c.operation_ticket().is_none());
}

#[test]
fn manual_browser_retry_revokes_automation_and_is_only_explicit() {
    let mut c = authenticating();
    assert!(matches!(
        c.handle(
            Action::OpenBrowser {
                ticket: c.ticket(),
                attempt: 11
            },
            context()
        ),
        Some(Effect::OpenBrowser { attempt: 11, .. })
    ));
    assert!(c.connection.is_none());
    complete(&mut c, Outcome::Confirmed);
    c.observed(authorized());
    assert!(c.continue_connection(context()).is_none());
}

#[test]
fn advanced_toggle_is_local_and_does_not_cancel_intent() {
    let mut c = connect();
    assert!(
        c.handle(Action::ToggleAdvanced(c.ticket()), context())
            .is_none()
    );
    assert!(c.advanced);
    assert!(c.connection.is_some());
    assert!(
        c.handle(Action::ToggleAdvanced(c.ticket()), context())
            .is_none()
    );
    assert!(!c.advanced);
    assert!(c.connection.is_some());
    c.leave();
    assert!(!c.advanced);
}

#[test]
fn stale_callback_holder_is_consumed_and_cannot_replace_or_cancel_connect() {
    let mut c = authenticating();
    let stale = c.draft_ticket().unwrap();
    c.observed(CodexMetadata {
        prompt: Some(13),
        ..waiting()
    });
    let current = c.draft_ticket().unwrap();
    let input = Input::new("PUBLIC_FAKE_CALLBACK".into());
    let replay = input.clone();
    c.handle(
        Action::Edit {
            ticket: stale,
            input,
        },
        context(),
    );
    assert!(c.connection.is_some());
    c.handle(
        Action::Edit {
            ticket: current,
            input: replay,
        },
        context(),
    );
    assert!(c.draft.0.is_empty());
    // A current, actual manual edit does revoke automatic next steps.
    c.handle(
        Action::Edit {
            ticket: current,
            input: Input::new("PUBLIC_FAKE_CALLBACK".into()),
        },
        context(),
    );
    assert!(c.connection.is_none());
    c.handle(Action::ReviewCallback(stale), context());
    assert!(c.review.is_none());
    assert!(
        c.handle(Action::ConfirmCallback(stale), context())
            .is_none()
    );
}

#[test]
fn stale_completion_does_not_retire_or_advance_connect_flight() {
    let mut c = opening();
    let actual = c.operation_ticket().unwrap();
    let wrong = Ticket {
        serial: actual.serial - 1,
        ..actual
    };
    c.completed(wrong, Outcome::Confirmed);
    assert_eq!(c.operation_ticket(), Some(actual));
    assert!(c.continue_connection(context()).is_none());
    complete(&mut c, Outcome::Confirmed);
    c.observed(authorized());
    assert!(matches!(
        c.continue_connection(context()),
        Some(Effect::EnableModels { .. })
    ));
}
