use super::*;
use dsh_native_transport::plugin::{FiberPhase, PluginEntry};
fn context() -> Context {
    Context {
        epoch: 1,
        visible: true,
        enabled: true,
    }
}
fn namespace(revision: u64, value: &str) -> SettingsNamespaceView {
    SettingsNamespaceView {
        ns: "public-demo".into(),
        revision,
        auto_generate: true,
        unsupported_fields: 1,
        secret_fields: 1,
        fields: vec![SettingsField {
            path: vec!["title".into()],
            label: "Title".into(),
            kind: SettingsFieldKind::String { max_length: 32 },
            value: Some(SettingsScalar::String(value.into())),
            overridden: false,
        }],
    }
}
fn snapshot() -> Snapshot {
    Snapshot {
        inventory: PluginInventorySnapshot {
            management_available: false,
            entries: vec![PluginEntry {
                entry_id: "tree/public-entry".into(),
                module_name: "public-module".into(),
                enabled: true,
                fiber_phase: FiberPhase::Active,
            }],
        },
        settings: SettingsDescribeValue {
            writable: true,
            namespaces: vec![namespace(7, "PUBLIC_OLD")],
        },
    }
}
fn loaded() -> Controller {
    let mut c = Controller::default();
    c.enter(1);
    let Some(Effect::Read(t)) = c.handle(Action::Refresh(c.read_ticket()), context()) else {
        panic!("explicit refresh")
    };
    c.loaded(t, Ok(snapshot()));
    c
}
fn draft(c: &mut Controller) {
    assert!(
        c.handle(
            Action::Begin {
                ticket: c.read_ticket(),
                field: 0
            },
            context()
        )
        .is_none()
    );
    let ticket = c.draft_ticket().expect("draft");
    c.handle(
        Action::Text {
            ticket,
            input: Input::new("PUBLIC_NEW".into()),
        },
        context(),
    );
}
fn submit(c: &mut Controller) -> WriteTicket {
    draft(c);
    let t = c.draft_ticket().unwrap();
    assert!(c.handle(Action::Confirm(t), context()).is_none());
    c.handle(Action::Review(t), context());
    let reviewed = c.draft_ticket().unwrap();
    let Some(Effect::Save {
        ticket,
        namespace,
        field,
        value,
    }) = c.handle(Action::Confirm(reviewed), context())
    else {
        panic!("review/confirm required")
    };
    assert_eq!(namespace.revision, 7);
    assert_eq!(field.path, vec!["title"]);
    assert_eq!(value, SettingsScalar::String("PUBLIC_NEW".into()));
    assert!(c.draft.is_none());
    ticket
}
#[test]
fn opening_ready_and_namespace_search_are_local_only() {
    let mut c = Controller::default();
    c.enter(0);
    c.adopt_epoch(1);
    assert!(c.reading.is_none());
    assert!(c.snapshot.is_none());
    assert!(
        c.handle(Action::Search("public".into()), context())
            .is_none()
    );
    let mut c = loaded();
    let t = c.read_ticket();
    assert!(
        c.handle(
            Action::SelectNamespace {
                ticket: t,
                index: 0
            },
            context()
        )
        .is_none()
    );
    assert!(c.reading.is_none());
}
#[test]
fn hidden_closed_smoke_and_wrong_epoch_never_request_or_write() {
    let mut c = Controller::default();
    let t = c.read_ticket();
    assert!(c.handle(Action::Refresh(t), context()).is_none());
    c.enter(1);
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
        assert!(c.handle(Action::Refresh(c.read_ticket()), ctx).is_none());
    }
    assert!(c.reading.is_none());
}
#[test]
fn refresh_duplicate_and_late_old_panel_are_fenced() {
    let mut c = Controller::default();
    c.enter(1);
    let old = c.read_ticket();
    let Some(Effect::Read(t)) = c.handle(Action::Refresh(old), context()) else {
        panic!()
    };
    assert!(c.handle(Action::Refresh(old), context()).is_none());
    c.leave();
    c.enter(1);
    c.loaded(t, Ok(snapshot()));
    assert!(c.snapshot.is_none());
}
#[test]
fn changed_field_only_explicit_single_review_confirm_and_matching_ack() {
    let mut c = loaded();
    let ticket = submit(&mut c);
    assert!(c.pending());
    assert!(
        c.handle(
            Action::Confirm(c.draft_ticket().unwrap_or(DraftTicket {
                epoch: 1,
                panel: 0,
                description: 0,
                namespace: 0,
                field: 0,
                editor: 0,
                revision: 0
            })),
            context()
        )
        .is_none()
    );
    c.saved(ticket, SaveOutcome::Confirmed(namespace(8, "PUBLIC_NEW")));
    assert!(!c.pending());
    assert!(c.fresh);
    assert!(c.notice == Notice::Confirmed);
    assert_eq!(
        c.snapshot.as_ref().unwrap().settings.namespaces[0].revision,
        8
    );
}
#[test]
fn canceled_review_confirmation_cannot_replay_after_rereview() {
    let mut c = loaded();
    draft(&mut c);
    let t = c.draft_ticket().unwrap();
    c.handle(Action::Review(t), context());
    let old = c.draft_ticket().unwrap();
    c.handle(Action::CancelReview(old), context());
    c.handle(Action::Review(old), context());
    assert_ne!(c.draft_ticket(), Some(old));
    assert!(c.handle(Action::Confirm(old), context()).is_none());
    assert!(!c.pending());
}
#[test]
fn batched_edits_share_editor_lifetime_but_old_review_does_not() {
    let mut c = loaded();
    c.handle(
        Action::Begin {
            ticket: c.read_ticket(),
            field: 0,
        },
        context(),
    );
    let ticket = c.draft_ticket().unwrap();
    for raw in ["PUBLIC_A", "PUBLIC_B"] {
        c.handle(
            Action::Text {
                ticket,
                input: Input::new(raw.into()),
            },
            context(),
        );
    }
    assert_eq!(c.draft.as_ref().unwrap().raw.0, "PUBLIC_B");
    assert_ne!(c.draft_ticket(), Some(ticket));
    assert!(c.handle(Action::Review(ticket), context()).is_none());
    assert!(c.review.is_none());
}
#[test]
fn one_take_string_holder_is_redacted_and_replay_does_not_restore_draft() {
    let mut c = loaded();
    draft(&mut c);
    let ticket = c.draft_ticket().unwrap();
    let input = Input::new("PUBLIC_A".into());
    assert!(!format!("{input:?}").contains("PUBLIC_A"));
    c.handle(
        Action::Text {
            ticket,
            input: input.clone(),
        },
        context(),
    );
    c.handle(
        Action::Text {
            ticket,
            input: Input::new("PUBLIC_B".into()),
        },
        context(),
    );
    c.handle(
        Action::Text {
            ticket,
            input: input.clone(),
        },
        context(),
    );
    assert!(input.take().is_none());
    assert_eq!(c.draft.as_ref().unwrap().raw.0, "PUBLIC_B");
}
#[test]
fn category_leave_discards_cache_draft_and_stale_input_holder() {
    let mut c = loaded();
    draft(&mut c);
    let ticket = c.draft_ticket().unwrap();
    c.leave();
    c.enter(1);
    let input = Input::new("PUBLIC_OLD".into());
    c.handle(
        Action::Text {
            ticket,
            input: input.clone(),
        },
        context(),
    );
    assert!(input.take().is_none());
    assert!(c.snapshot.is_none());
    assert!(c.draft.is_none());
}
#[test]
fn pending_receipt_survives_leave_but_never_retargets_reopened_panel() {
    let mut c = loaded();
    let ticket = submit(&mut c);
    c.leave();
    c.enter(1);
    assert!(c.pending());
    assert!(
        c.handle(Action::Refresh(c.read_ticket()), context())
            .is_none()
    );
    let mut stale = ticket;
    stale.serial += 1;
    c.saved(stale, SaveOutcome::NotSent);
    assert!(c.pending());
    c.saved(ticket, SaveOutcome::Confirmed(namespace(8, "PUBLIC_NEW")));
    assert!(!c.pending());
    assert!(c.snapshot.is_none());
    assert!(c.notice == Notice::Earlier);
}
#[test]
fn conflict_indeterminate_refusal_and_not_sent_never_retry_or_restore() {
    for outcome in [
        SaveOutcome::Conflict,
        SaveOutcome::Indeterminate,
        SaveOutcome::Refused,
        SaveOutcome::NotSent,
    ] {
        let mut c = loaded();
        let ticket = submit(&mut c);
        c.saved(ticket, outcome);
        assert!(!c.fresh);
        assert!(c.draft.is_none());
        assert!(!c.pending());
        assert!(c.reading.is_none());
        assert!(
            c.handle(
                Action::Begin {
                    ticket: c.read_ticket(),
                    field: 0
                },
                context()
            )
            .is_none()
        );
        assert!(c.draft.is_none());
    }
}
#[test]
fn disconnect_and_epoch_change_make_admitted_result_indeterminate() {
    let mut c = loaded();
    let ticket = submit(&mut c);
    c.disconnect();
    c.saved(ticket, SaveOutcome::Confirmed(namespace(8, "PUBLIC_NEW")));
    assert!(c.notice == Notice::Indeterminate);
    assert!(c.snapshot.is_none());
    let mut c = loaded();
    submit(&mut c);
    c.adopt_epoch(2);
    assert!(c.notice == Notice::Indeterminate);
    assert!(!c.pending());
}
#[test]
fn wrong_namespace_or_nonadvancing_ack_is_indeterminate() {
    for bad in [
        namespace(7, "PUBLIC_NEW"),
        SettingsNamespaceView {
            ns: "different".into(),
            ..namespace(8, "PUBLIC_NEW")
        },
    ] {
        let mut c = loaded();
        let ticket = submit(&mut c);
        c.saved(ticket, SaveOutcome::Confirmed(bad));
        assert!(c.notice == Notice::Indeterminate);
        assert!(!c.fresh);
    }
}
#[test]
fn missing_values_unchanged_values_and_bad_indices_never_submit() {
    let mut c = loaded();
    c.handle(
        Action::Begin {
            ticket: c.read_ticket(),
            field: 0,
        },
        context(),
    );
    c.handle(Action::Review(c.draft_ticket().unwrap()), context());
    assert!(c.review.is_none());
    let mut c = loaded();
    c.snapshot.as_mut().unwrap().settings.namespaces[0].fields[0].value = None;
    c.handle(
        Action::Begin {
            ticket: c.read_ticket(),
            field: 0,
        },
        context(),
    );
    assert!(c.draft.is_none());
    c.handle(
        Action::SelectNamespace {
            ticket: c.read_ticket(),
            index: usize::MAX,
        },
        context(),
    );
    assert_eq!(c.selected, Some(0));
}
#[test]
fn primitive_number_type_finiteness_bounds_and_safe_integer_validation() {
    let mut f = namespace(1, "x").fields.remove(0);
    f.kind = SettingsFieldKind::Number {
        min: Some(0.0),
        max: Some(10.0),
        integer: true,
    };
    for bad in [f64::NAN, f64::INFINITY, -1.0, 11.0, 2.5] {
        assert!(!valid(&f, &SettingsScalar::Number(bad)));
    }
    assert!(valid(&f, &SettingsScalar::Number(2.0)));
    assert!(!valid(&f, &SettingsScalar::Bool(true)));
    f.kind = SettingsFieldKind::Number {
        min: None,
        max: None,
        integer: true,
    };
    assert!(!valid(&f, &SettingsScalar::Number(SAFE_INTEGER + 1.0)));
}
#[test]
fn string_utf8_limits_sensitive_paths_duplicates_and_snapshot_bounds_fail_closed() {
    let mut f = namespace(1, "x").fields.remove(0);
    f.kind = SettingsFieldKind::String { max_length: 6 };
    assert!(valid(&f, &SettingsScalar::String("日本".into())));
    assert!(!valid(&f, &SettingsScalar::String("日本語".into())));
    for path in [
        "api_key",
        "password",
        "accessToken",
        "credentials",
        "private-key",
    ] {
        f.path = vec![path.into()];
        assert!(!safe_field(&f));
    }
    let mut s = snapshot();
    let duplicate = s.settings.namespaces[0].fields[0].clone();
    s.settings.namespaces[0].fields.push(duplicate);
    assert!(!safe_snapshot(&s));
}
#[test]
fn custom_ui_namespace_and_policy_fields_never_edit_or_confirm() {
    let mut c = loaded();
    c.snapshot.as_mut().unwrap().settings.namespaces[0].auto_generate = false;
    assert!(!c.editable());
    c.handle(
        Action::Begin {
            ticket: c.read_ticket(),
            field: 0,
        },
        context(),
    );
    assert!(c.draft.is_none());
    // Even a forged preexisting draft/review cannot bypass the namespace opt-out.
    c.draft = Some(Draft {
        field: 0,
        raw: Editor("PUBLIC_NEW".into()),
    });
    let ticket = c.draft_ticket().unwrap();
    c.review = Some(ticket);
    assert!(c.handle(Action::Confirm(ticket), context()).is_none());
    assert!(!c.pending());
    for key in [
        "defaultPreset",
        "permissions",
        "approvalPolicy",
        "sandbox",
        "jsexpr",
        "constructor",
    ] {
        let mut field = namespace(1, "PUBLIC").fields.remove(0);
        field.path = vec![key.into()];
        assert!(!safe_field(&field));
    }
}
#[test]
fn inventory_and_namespace_disclosures_are_local_and_selection_closes_choices() {
    let mut c = loaded();
    let ticket = c.read_ticket();
    assert!(!c.inventory_open && !c.namespaces_open);
    assert!(
        c.handle(Action::ToggleInventory(ticket), context())
            .is_none()
    );
    assert!(
        c.handle(Action::ToggleNamespaces(ticket), context())
            .is_none()
    );
    assert!(c.inventory_open && c.namespaces_open);
    assert!(
        c.handle(Action::SelectNamespace { ticket, index: 0 }, context())
            .is_none()
    );
    assert!(!c.namespaces_open && c.reading.is_none());
}
#[test]
fn oversize_input_clears_old_draft() {
    let mut c = loaded();
    draft(&mut c);
    let ticket = c.draft_ticket().unwrap();
    c.handle(
        Action::Text {
            ticket,
            input: Input::new("x".repeat(MAX_INPUT + 1)),
        },
        context(),
    );
    assert!(c.draft.is_none());
    assert!(c.review.is_none());
}
