//! PUBLIC synthetic reducer schedules only; no Core, OAuth or provider work.
use super::super::{Action as CodexAction, Context, Controller, Effect, Tab};
use super::*;

fn context() -> Context {
    Context {
        epoch: 1,
        visible: true,
        enabled: true,
    }
}
fn tab(c: &mut Controller, tab: Tab) {
    assert!(
        c.handle(
            CodexAction::SelectTab {
                ticket: c.ticket(),
                tab
            },
            context()
        )
        .is_none()
    );
}
fn controller() -> Controller {
    let mut c = Controller::default();
    c.enter(1);
    tab(&mut c, Tab::Models);
    c
}
fn action(c: &mut Controller, action: Action) -> Option<Effect> {
    c.handle(
        CodexAction::Runtime {
            ticket: c.ticket(),
            action,
        },
        context(),
    )
}
fn number(c: &mut Controller, field: Field, raw: &str) {
    assert!(
        action(
            c,
            Action::Number {
                field,
                value: raw.into()
            }
        )
        .is_none()
    );
}
fn snapshot() -> CodexRuntimeSnapshot {
    let base_values = CodexRuntimeValues {
        transport: Some(CodexRuntimeTransport::Auto),
        cache_retention: Some(CodexCacheRetention::Short),
        timeout_ms: Some(CodexRuntimeNatural::new(100).unwrap()),
        websocket_connect_timeout_ms: Some(CodexRuntimeNatural::new(200).unwrap()),
        stream_idle_timeout_ms: Some(CodexIdleMillis::new(300.5).unwrap()),
        request_image_pixel_budget: Some(CodexImageBudget::new(400).unwrap()),
        request_image_max_bytes: Some(CodexImageBudget::new(500).unwrap()),
        max_request_image_bytes: Some(CodexImageBudget::new(600).unwrap()),
    };
    CodexRuntimeSnapshot {
        version: 1,
        revision: 7,
        configured: true,
        writable: true,
        values: base_values,
        base_values,
        overrides: CodexRuntimeValues::default(),
    }
}
fn read(c: &mut Controller, snapshot: CodexRuntimeSnapshot) {
    let Some(Effect::RuntimeRead { ticket }) = action(c, Action::Refresh) else {
        panic!("explicit read")
    };
    c.completed(ticket, Outcome::Runtime(snapshot));
}
fn loaded() -> Controller {
    let mut c = controller();
    read(&mut c, snapshot());
    c
}
fn save(c: &mut Controller) -> (Ticket, u64, CodexRuntimeValues, CodexRuntimeValues) {
    let Some(Effect::RuntimeSave {
        ticket,
        expected_revision,
        values,
        base_values,
    }) = action(c, Action::Save)
    else {
        panic!("explicit save")
    };
    (ticket, expected_revision, values, base_values)
}
fn ack(revision: u64, draft: CodexRuntimeValues, base: CodexRuntimeValues) -> CodexRuntimeSnapshot {
    CodexRuntimeSnapshot {
        revision: revision + 1,
        values: CodexRuntimeValues {
            transport: draft.transport.or(base.transport),
            cache_retention: draft.cache_retention.or(base.cache_retention),
            timeout_ms: draft.timeout_ms.or(base.timeout_ms),
            websocket_connect_timeout_ms: draft
                .websocket_connect_timeout_ms
                .or(base.websocket_connect_timeout_ms),
            stream_idle_timeout_ms: draft.stream_idle_timeout_ms.or(base.stream_idle_timeout_ms),
            request_image_pixel_budget: draft
                .request_image_pixel_budget
                .or(base.request_image_pixel_budget),
            request_image_max_bytes: draft
                .request_image_max_bytes
                .or(base.request_image_max_bytes),
            max_request_image_bytes: draft
                .max_request_image_bytes
                .or(base.max_request_image_bytes),
        },
        base_values: base,
        overrides: draft,
        ..snapshot()
    }
}
#[test]
fn opens_and_tab_switches_never_read_or_save_automatically() {
    let mut c = controller();
    assert!(action(&mut c, Action::Save).is_none());
    for next in [Tab::Images, Tab::Account, Tab::Maintenance, Tab::Models] {
        tab(&mut c, next);
        assert!(c.continue_connection(context()).is_none());
        assert!(c.runtime.snapshot.is_none() && !c.runtime.pending());
        let _: iced::Element<'_, CodexAction> = c.view(context());
    }
    assert!(c.metadata.is_none() && c.flight.is_none() && c.connection.is_none());
}
#[test]
fn numeric_boundaries_blank_inheritance_and_only_idle_decimals() {
    let mut c = loaded();
    for field in [Field::Timeout, Field::WebsocketConnectTimeout] {
        for raw in ["0", "9007199254740991", ""] {
            number(&mut c, field, raw);
            assert!(c.runtime.can_save(), "{field:?}: {raw}");
        }
        for raw in [
            "-1",
            "+1",
            "1.5",
            "1e2",
            "9007199254740992",
            "18446744073709551616",
            " 2 ",
            "NaN",
        ] {
            number(&mut c, field, raw);
            assert!(!c.runtime.can_save(), "{field:?}: {raw}");
            assert!(action(&mut c, Action::Save).is_none());
        }
        number(&mut c, field, "");
    }
    for raw in ["0.5", "2147483647", ""] {
        number(&mut c, Field::StreamIdleTimeout, raw);
        assert!(c.runtime.can_save());
    }
    for raw in [
        "0",
        "-0",
        "2147483647.1",
        "NaN",
        "inf",
        "1e3",
        "..",
        "1.2.3",
    ] {
        number(&mut c, Field::StreamIdleTimeout, raw);
        assert!(!c.runtime.can_save());
    }
    number(&mut c, Field::StreamIdleTimeout, "");
    for field in [
        Field::ImagePixelBudget,
        Field::ImageMaxBytes,
        Field::MaxRequestImageBytes,
    ] {
        for raw in ["1", "9007199254740991", ""] {
            number(&mut c, field, raw);
            assert!(c.runtime.can_save());
        }
        for raw in ["0", "-1", "1.5", "1e2", "9007199254740992"] {
            number(&mut c, field, raw);
            assert!(!c.runtime.can_save());
        }
        number(&mut c, field, "");
    }
    assert_eq!(c.runtime.values().unwrap(), CodexRuntimeValues::default());
}
#[test]
fn oversized_invalid_text_never_saves_old_value_or_becomes_inheritance() {
    for field in [
        Field::Timeout,
        Field::WebsocketConnectTimeout,
        Field::StreamIdleTimeout,
        Field::ImagePixelBudget,
        Field::ImageMaxBytes,
        Field::MaxRequestImageBytes,
    ] {
        let mut c = loaded();
        number(&mut c, field, "10");
        number(&mut c, field, &"2".repeat(65));
        assert!(c.runtime.number(field).len() <= 64);
        assert!(!c.runtime.can_save());
        assert!(c.runtime.values().is_err());
        assert!(action(&mut c, Action::Save).is_none());
    }
}
#[test]
fn shared_draft_and_explicit_pins_equal_to_base_survive_other_tab_save() {
    let mut c = controller();
    let mut pin = snapshot();
    pin.overrides.timeout_ms = pin.base_values.timeout_ms;
    read(&mut c, pin);
    assert_eq!(c.runtime.number(Field::Timeout), "100");
    action(
        &mut c,
        Action::Transport(Some(CodexRuntimeTransport::WebsocketCached)),
    );
    action(&mut c, Action::Cache(Some(CodexCacheRetention::Long)));
    number(&mut c, Field::StreamIdleTimeout, "1.25");
    tab(&mut c, Tab::Images);
    number(&mut c, Field::ImagePixelBudget, "44");
    number(&mut c, Field::ImageMaxBytes, "55");
    number(&mut c, Field::MaxRequestImageBytes, "66");
    let (ticket, revision, values, base) = save(&mut c);
    assert_eq!(values.timeout_ms, pin.overrides.timeout_ms);
    assert_eq!(
        values.transport,
        Some(CodexRuntimeTransport::WebsocketCached)
    );
    assert_eq!(values.cache_retention, Some(CodexCacheRetention::Long));
    assert_eq!(values.stream_idle_timeout_ms.unwrap().as_f64(), 1.25);
    assert_eq!(values.request_image_pixel_budget.unwrap().get(), 44);
    assert_eq!(values.request_image_max_bytes.unwrap().get(), 55);
    assert_eq!(values.max_request_image_bytes.unwrap().get(), 66);
    c.completed(ticket, Outcome::Runtime(ack(revision, values, base)));
    assert!(c.runtime.status().starts_with("Runtime settings saved"));
    tab(&mut c, Tab::Models);
    assert_eq!(c.runtime.values().unwrap(), values);
}
#[test]
fn reset_edits_all_eight_fields_without_executing_and_inherited_noop_ack_is_valid() {
    let mut c = loaded();
    action(&mut c, Action::Transport(Some(CodexRuntimeTransport::Sse)));
    number(&mut c, Field::ImageMaxBytes, "77");
    assert!(action(&mut c, Action::Reset).is_none());
    assert!(!c.runtime.pending());
    assert_eq!(c.runtime.values().unwrap(), CodexRuntimeValues::default());
    assert!(c.runtime.status().starts_with("Draft reset"));
    let (ticket, revision, values, base) = save(&mut c);
    let mut after = ack(revision, values, base);
    after.revision = revision; // Public no-op need not increment.
    after.overrides = after.values; // Public persistence may normalize inherited fields.
    c.completed(ticket, Outcome::Runtime(after));
    assert!(c.runtime.status().starts_with("Runtime settings saved"));
    assert_eq!(c.runtime.values().unwrap(), after.overrides);
}
#[test]
fn saving_races_ignore_edits_double_save_and_wrong_or_duplicate_receipts() {
    let mut c = loaded();
    number(&mut c, Field::Timeout, "42");
    let (ticket, revision, values, base) = save(&mut c);
    number(&mut c, Field::Timeout, "99");
    action(&mut c, Action::Reset);
    assert_eq!(c.runtime.values().unwrap(), values);
    assert!(action(&mut c, Action::Save).is_none());
    c.completed(
        Ticket {
            serial: ticket.serial + 1,
            ..ticket
        },
        Outcome::Runtime(ack(revision, values, base)),
    );
    assert!(c.runtime.pending());
    c.completed(ticket, Outcome::Runtime(ack(revision, values, base)));
    assert!(!c.runtime.pending());
    let before = c.runtime.snapshot;
    c.completed(ticket, Outcome::RuntimeUnavailable);
    assert_eq!(c.runtime.snapshot, before);
}
#[test]
fn mismatched_ack_revision_base_configuration_and_all_effective_fields_are_unknown() {
    for field in 0..13 {
        let mut c = loaded();
        let (ticket, revision, values, base) = save(&mut c);
        let mut after = ack(revision, values, base);
        match field {
            0 => after.version = 2,
            1 => after.configured = false,
            2 => after.revision = revision - 1,
            3 => after.revision = revision + 2,
            4 => after.base_values.transport = Some(CodexRuntimeTransport::Sse),
            5 => after.values.transport = Some(CodexRuntimeTransport::Sse),
            6 => after.values.cache_retention = Some(CodexCacheRetention::None),
            7 => after.values.timeout_ms = None,
            8 => after.values.websocket_connect_timeout_ms = None,
            9 => after.values.stream_idle_timeout_ms = None,
            10 => after.values.request_image_pixel_budget = None,
            11 => after.values.request_image_max_bytes = None,
            12 => after.values.max_request_image_bytes = None,
            _ => unreachable!(),
        }
        c.completed(ticket, Outcome::Runtime(after));
        assert!(!c.runtime.can_save(), "{field}");
        assert!(c.runtime.status().contains("unknown"), "{field}");
        assert_eq!(c.runtime.snapshot, Some(snapshot()));
    }
}
#[test]
fn draft_edits_before_and_during_read_survive_explicit_refresh() {
    let mut c = loaded();
    number(&mut c, Field::Timeout, "77");
    let Some(Effect::RuntimeRead { ticket }) = action(&mut c, Action::Refresh) else {
        panic!()
    };
    number(&mut c, Field::ImageMaxBytes, "88");
    let mut after = snapshot();
    after.revision = 99;
    after.overrides.transport = Some(CodexRuntimeTransport::Sse);
    after.values.transport = after.overrides.transport;
    c.completed(ticket, Outcome::Runtime(after));
    assert_eq!(c.runtime.number(Field::Timeout), "77");
    assert_eq!(c.runtime.number(Field::ImageMaxBytes), "88");
    assert_eq!(c.runtime.transport, None);
    assert_eq!(save(&mut c).1, 99);
}
#[test]
fn conflict_and_unknown_require_read_without_retry_or_auth_usage_reconciliation() {
    for outcome in [
        Outcome::RuntimeConflict,
        Outcome::RuntimeIndeterminate,
        Outcome::RuntimeUnavailable,
    ] {
        let mut c = loaded();
        number(&mut c, Field::Timeout, "123");
        let (ticket, revision, values, base) = save(&mut c);
        c.completed(ticket, outcome);
        assert!(!c.runtime.can_save());
        c.completed(ticket, Outcome::Runtime(ack(revision, values, base)));
        assert!(!c.runtime.can_save());
        assert!(c.continue_connection(context()).is_none());
        assert!(action(&mut c, Action::Save).is_none());
        read(
            &mut c,
            CodexRuntimeSnapshot {
                revision: 22,
                ..snapshot()
            },
        );
        assert_eq!(c.runtime.number(Field::Timeout), "123");
        assert!(c.runtime.can_save());
        assert_eq!(save(&mut c).1, 22);
    }
}
#[test]
fn hide_reopen_retains_pending_and_draft_but_cannot_ack_prior_panel() {
    let mut c = loaded();
    number(&mut c, Field::Timeout, "42");
    let (ticket, revision, values, base) = save(&mut c);
    c.leave();
    assert!(c.runtime.pending());
    assert_eq!(c.runtime.number(Field::Timeout), "42");
    c.enter(1);
    tab(&mut c, Tab::Models);
    assert!(c.runtime.pending());
    c.completed(ticket, Outcome::Runtime(ack(revision, values, base)));
    assert!(!c.runtime.can_save());
    assert!(c.runtime.status().contains("unknown"));
    assert_eq!(c.runtime.snapshot, Some(snapshot()));
    read(&mut c, snapshot());
    assert_eq!(c.runtime.number(Field::Timeout), "42");
}
#[test]
fn changed_tab_ownership_and_epoch_fence_late_receipts_without_oauth_uncertainty() {
    for change_epoch in [false, true] {
        let mut c = loaded();
        let (ticket, revision, values, base) = save(&mut c);
        if change_epoch {
            c.adopt_epoch(2);
        } else {
            tab(&mut c, Tab::Images);
        }
        c.completed(ticket, Outcome::Runtime(ack(revision, values, base)));
        assert!(!c.runtime.can_save());
        assert_eq!(c.runtime.snapshot, Some(snapshot()));
        assert!(!c.owner_uncertain && !c.managed.uncertain);
    }
}
#[test]
fn stale_hidden_disabled_wrong_epoch_or_wrong_tab_intents_cannot_write() {
    let mut c = loaded();
    let stale = c.ticket();
    tab(&mut c, Tab::Images);
    for (ticket, ctx) in [
        (stale, context()),
        (
            c.ticket(),
            Context {
                visible: false,
                ..context()
            },
        ),
        (
            c.ticket(),
            Context {
                enabled: false,
                ..context()
            },
        ),
        (
            c.ticket(),
            Context {
                epoch: 2,
                ..context()
            },
        ),
    ] {
        for intent in [
            Action::Refresh,
            Action::Save,
            Action::Reset,
            Action::Number {
                field: Field::Timeout,
                value: "999".into(),
            },
        ] {
            assert!(
                c.handle(
                    CodexAction::Runtime {
                        ticket,
                        action: intent
                    },
                    ctx
                )
                .is_none()
            );
        }
    }
    assert_eq!(c.runtime.values().unwrap(), CodexRuntimeValues::default());
    tab(&mut c, Tab::Account);
    assert!(action(&mut c, Action::Save).is_none());
    assert!(action(&mut c, Action::Refresh).is_none());
}
#[test]
fn reserved_cancel_does_not_release_or_reassign_ui_runtime_flight() {
    for cancel_first in [false, true] {
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
        let (ticket, revision, values, base) = save(&mut c);
        let Some(Effect::CancelLogin { ticket: cancel, .. }) = c.handle(
            CodexAction::Cancel {
                ticket: c.ticket(),
                attempt: 11,
            },
            context(),
        ) else {
            panic!("reserved cancellation")
        };
        assert_eq!(c.runtime.ticket(), Some(ticket));
        let cancelled = Outcome::Metadata(dsh_native_core::CodexMetadata {
            phase: dsh_native_core::CodexPhase::Cancelled,
            ..c.metadata.unwrap()
        });
        if cancel_first {
            c.completed(cancel, cancelled.clone());
            assert_eq!(c.runtime.ticket(), Some(ticket));
        }
        c.completed(ticket, Outcome::Runtime(ack(revision, values, base)));
        assert!(c.runtime.status().starts_with("Runtime settings saved"));
        if !cancel_first {
            c.completed(cancel, cancelled);
        }
        assert!(!c.runtime.pending());
    }
}
#[test]
fn critical_unknown_status_cannot_be_hidden_by_invalid_edits_and_is_constructible_on_all_tabs() {
    let mut c = loaded();
    let (ticket, _, _, _) = save(&mut c);
    c.completed(ticket, Outcome::RuntimeIndeterminate);
    number(&mut c, Field::Timeout, "NaN");
    assert!(c.runtime.status().contains("unknown"));
    assert!(c.runtime.attention());
    for next in Tab::ALL {
        tab(&mut c, next);
        let _: iced::Element<'_, CodexAction> = c.view(context());
        assert!(c.runtime.status().contains("unknown"));
    }
}
#[test]
fn reset_keeps_reconciliation_warnings_until_owned_read() {
    for outcome in [Outcome::RuntimeIndeterminate, Outcome::RuntimeConflict, Outcome::RuntimeUnavailable] {
        let mut c = loaded();
        let (ticket, _, _, _) = save(&mut c);
        c.completed(ticket, outcome);
        let warning = c.runtime.status();
        assert!(action(&mut c, Action::Reset).is_none());
        assert_eq!(c.runtime.status(), warning);
        assert!(c.runtime.attention());
        assert!(!c.runtime.can_save());
        assert!(action(&mut c, Action::Save).is_none());
        assert_eq!(c.runtime.values().unwrap(), CodexRuntimeValues::default());
        read(&mut c, snapshot());
        assert!(c.runtime.can_save());
        assert!(!c.runtime.attention());
    }
}
#[test]
fn reset_keeps_pending_read_visible_and_does_not_dispatch() {
    let mut c = loaded();
    number(&mut c, Field::Timeout, "900");
    let Some(Effect::RuntimeRead { ticket }) = action(&mut c, Action::Refresh) else { panic!("read") };
    assert!(action(&mut c, Action::Reset).is_none());
    assert!(c.runtime.status().starts_with("Reading runtime settings"));
    assert_eq!(c.runtime.ticket(), Some(ticket));
    assert!(!c.runtime.can_save());
    c.completed(ticket, Outcome::Runtime(snapshot()));
    assert_eq!(c.runtime.values().unwrap(), CodexRuntimeValues::default());
}
#[test]
fn fixed_footer_reports_pending_and_uncertainty_across_tabs() {
    let mut c = loaded();
    assert!(c.runtime_footer().is_none());
    let (ticket, _, _, _) = save(&mut c);
    let (notice, attention) = c.runtime_footer().unwrap();
    assert!(notice.contains("pending"));
    assert!(!attention);
    c.completed(ticket, Outcome::RuntimeIndeterminate);
    for next in Tab::ALL {
        tab(&mut c, next);
        let (notice, attention) = c.runtime_footer().unwrap();
        assert!(notice.contains("unknown"));
        assert!(attention);
    }
    tab(&mut c, Tab::Models);
    read(&mut c, snapshot());
    assert!(c.runtime_footer().is_none());
}
#[test]
fn usage_local_off_remains_available_while_runtime_save_is_pending() {
    let mut c = loaded();
    c.usage.draft.experimental_quota = true;
    let (ticket, revision, values, base) = save(&mut c);
    tab(&mut c, Tab::Account);
    assert!(
        c.handle(
            CodexAction::Usage {
                ticket: c.ticket(),
                action: super::super::UsageAction::Consent(false),
            },
            Context {
                enabled: false,
                ..context()
            }
        )
        .is_none()
    );
    assert!(!c.usage.draft.experimental_quota);
    assert_eq!(c.runtime.ticket(), Some(ticket));
    c.completed(ticket, Outcome::Runtime(ack(revision, values, base)));
    assert!(!c.runtime.can_save()); // Changed panel is not allowed to ACK the old write.
    assert!(!c.effective_consent());
}
#[test]
fn unavailable_or_readonly_read_does_not_authorize_cached_or_default_writes() {
    let mut c = loaded();
    let Some(Effect::RuntimeRead { ticket }) = action(&mut c, Action::Refresh) else {
        panic!()
    };
    c.completed(ticket, Outcome::RuntimeUnavailable);
    assert!(!c.runtime.can_save());
    read(
        &mut c,
        CodexRuntimeSnapshot {
            writable: false,
            ..snapshot()
        },
    );
    assert!(!c.runtime.can_save());
    assert!(action(&mut c, Action::Save).is_none());
    let _: iced::Element<'_, CodexAction> = c.view(context());
}
