use super::*;
use crate::plugins::{self, ReadTicket, WriteTicket, draft_input};
use dsh_native_transport::plugin::*;

fn snapshot() -> plugins::Snapshot {
    snapshot_named("PUBLIC-fixture")
}
fn snapshot_named(ns: &str) -> plugins::Snapshot {
    plugins::Snapshot {
        inventory: PluginInventorySnapshot {
            entries: vec![],
            management_available: false,
        },
        settings: SettingsDescribeValue {
            writable: true,
            namespaces: vec![SettingsNamespaceView {
                ns: ns.into(),
                revision: 0,
                auto_generate: true,
                unsupported_fields: 0,
                secret_fields: 0,
                fields: vec![SettingsField {
                    path: vec!["enabled".into()],
                    label: "enabled".into(),
                    kind: SettingsFieldKind::Bool,
                    value: Some(SettingsScalar::Bool(false)),
                    overridden: false,
                }],
            }],
        },
    }
}
/// Two namespaces with the foreign one first: the order in which the old code wrote the wrong one.
fn two_namespaces() -> plugins::Snapshot {
    let mut snapshot = snapshot_named("PUBLIC-other");
    snapshot.settings.namespaces.push(SettingsNamespaceView {
        ns: "llm-deepseek".into(),
        revision: 3,
        auto_generate: true,
        unsupported_fields: 0,
        secret_fields: 0,
        fields: vec![SettingsField {
            path: vec!["reasoningEffort".into()],
            label: "reasoningEffort".into(),
            kind: SettingsFieldKind::String { max_length: 64 },
            value: Some(SettingsScalar::String("medium".into())),
            overridden: false,
        }],
    });
    snapshot
}
fn opened_plugins() -> Settings {
    let mut state = Settings::default();
    state.handle(Action::Open, 1, true);
    assert!(
        state
            .handle(Action::SelectPage(Page::Plugins), 1, true)
            .is_none()
    );
    state
}
fn loaded_plugins(state: &mut Settings) -> ReadTicket {
    let rendered = state.plugin_read_ticket();
    let Some(Effect::Plugins(plugins::Effect::Read(ticket))) =
        state.handle(Action::Plugins(plugins::Action::Refresh(rendered)), 1, true)
    else {
        panic!("explicit plugin refresh required")
    };
    state.plugins_loaded(ticket, Ok(snapshot()));
    ticket
}
fn submit_plugin(state: &mut Settings) -> WriteTicket {
    loaded_plugins(state);
    let ticket = state.plugin_read_ticket();
    state.handle(
        Action::Plugins(plugins::Action::Begin {
            namespace: 0,
            ticket,
            field: 0,
        }),
        1,
        true,
    );
    let ticket = state.plugin_draft_ticket().unwrap();
    state.handle(
        Action::Plugins(plugins::Action::Bool {
            ticket,
            value: true,
        }),
        1,
        true,
    );
    let ticket = state.plugin_draft_ticket().unwrap();
    state.handle(Action::Plugins(plugins::Action::Review(ticket)), 1, true);
    let ticket = state.plugin_draft_ticket().unwrap();
    let Some(Effect::Plugins(plugins::Effect::Save {
        ticket,
        namespace,
        field,
        value,
    })) = state.handle(Action::Plugins(plugins::Action::Confirm(ticket)), 1, true)
    else {
        panic!("explicit reviewed plugin save required")
    };
    assert_eq!(namespace.revision, ticket.expected_revision);
    assert_eq!(field.path, vec!["enabled"]);
    assert_eq!(value, SettingsScalar::Bool(true));
    ticket
}
/// The DeepSeek page draws `llm-deepseek` with no namespace selector, so a draft begun from one of
/// its field controls must be resolved against the namespace that control was drawn from — not
/// whichever namespace happens to be selected. The old draft stored only the field index and
/// resolved it against the selection, so a host that advertised another namespace first wrote that
/// namespace's field at the same index, at that namespace's revision.
#[test]
fn a_draft_is_resolved_against_the_namespace_its_control_was_drawn_from() {
    let mut state = opened_plugins();
    let rendered = state.plugin_read_ticket();
    let Some(Effect::Plugins(plugins::Effect::Read(ticket))) =
        state.handle(Action::Plugins(plugins::Action::Refresh(rendered)), 1, true)
    else {
        panic!("explicit refresh required")
    };
    state.plugins_loaded(ticket, Ok(two_namespaces()));
    // The selection stays on the foreign namespace, as it would after a load.
    let read = state.plugin_read_ticket();
    state.handle(
        Action::Plugins(plugins::Action::Begin {
            namespace: 1,
            ticket: read,
            field: 0,
        }),
        1,
        true,
    );
    let draft = state
        .plugin_draft_ticket()
        .expect("the drawn namespace begins");
    state.handle(
        Action::Plugins(plugins::Action::Text {
            ticket: draft,
            input: draft_input("high".into()),
        }),
        1,
        true,
    );
    let draft = state.plugin_draft_ticket().unwrap();
    state.handle(Action::Plugins(plugins::Action::Review(draft)), 1, true);
    let draft = state.plugin_draft_ticket().unwrap();
    let Some(Effect::Plugins(plugins::Effect::Save {
        namespace, field, ..
    })) = state.handle(Action::Plugins(plugins::Action::Confirm(draft)), 1, true)
    else {
        panic!("the drawn namespace's write is admitted")
    };
    assert_eq!(namespace.ns, "llm-deepseek");
    assert_eq!(namespace.revision, 3);
    assert_eq!(field.label, "reasoningEffort");
}
#[test]
fn plugins_enter_reopen_and_epoch_adoption_never_read_automatically() {
    let mut state = opened_plugins();
    assert!(state.plugins.draft_ticket().is_none());
    state.close();
    assert!(state.handle(Action::Open, 1, true).is_none());
    state.adopt_epoch(2);
    assert!(state.plugins.draft_ticket().is_none());
    let ticket = state.plugin_read_ticket();
    assert!(matches!(
        state.handle(Action::Plugins(plugins::Action::Refresh(ticket)), 2, true),
        Some(Effect::Plugins(plugins::Effect::Read(_)))
    ));
}
#[test]
fn plugin_refresh_requires_current_visible_ready_settings_and_epoch() {
    for case in 0..4 {
        let mut state = opened_plugins();
        let ticket = state.plugin_read_ticket();
        let (epoch, enabled) = match case {
            0 => {
                state.close();
                (1, true)
            }
            1 => {
                state.handle(Action::SelectPage(Page::General), 1, true);
                (1, true)
            }
            2 => (1, false),
            _ => (2, true),
        };
        assert!(
            state
                .handle(
                    Action::Plugins(plugins::Action::Refresh(ticket)),
                    epoch,
                    enabled
                )
                .is_none()
        );
    }
}
#[test]
fn key_pending_blocks_plugin_read_without_losing_its_receipt() {
    let mut state = Settings::default();
    let Some(Effect::Read(ticket)) = state.handle(Action::Open, 1, true) else {
        panic!()
    };
    state.metadata(
        ticket,
        Ok(OnboardingMetadata {
            logged_in: false,
            has_api_key: false,
            writable: true,
        }),
    );
    state.handle(
        Action::Edit {
            ticket: state.input_ticket(),
            input: draft_input("PUBLIC_DUMMY".into()),
        },
        1,
        true,
    );
    let ticket = state.ticket();
    state.handle(Action::Review(ticket), 1, true);
    let Some(Effect::Save {
        ticket: submitted,
        secret,
    }) = state.handle(Action::Confirm(ticket), 1, true)
    else {
        panic!()
    };
    drop(secret);
    state.handle(Action::SelectPage(Page::Plugins), 1, true);
    assert!(
        state
            .handle(
                Action::Plugins(plugins::Action::Refresh(state.plugin_read_ticket())),
                1,
                true
            )
            .is_none()
    );
    assert!(matches!(state.save, SavePhase::Pending(ticket) if ticket == submitted));
    state.saved(submitted, SaveResult::Confirmed);
    assert!(!state.pending());
}
#[test]
fn plugin_pending_survives_navigation_and_blocks_key_input_and_save() {
    let mut state = opened_plugins();
    let submitted = submit_plugin(&mut state);
    assert!(state.plugins.pending());
    state.handle(Action::SelectPage(Page::ApiLogin), 1, true);
    state.handle(
        Action::Edit {
            ticket: state.input_ticket(),
            input: draft_input("PUBLIC_DUMMY".into()),
        },
        1,
        true,
    );
    assert!(state.edit.as_str().is_empty());
    assert!(!state.can_save(true));
    state.plugin_saved(submitted, plugins::SaveOutcome::NotSent);
    assert!(!state.plugins.pending());
}
#[test]
fn hidden_api_refresh_and_old_plugin_confirmation_cannot_dispatch() {
    let mut state = opened_plugins();
    assert!(
        state
            .handle(Action::Refresh(state.ticket()), 1, true)
            .is_none()
    );
    loaded_plugins(&mut state);
    state.handle(
        Action::Plugins(plugins::Action::Begin {
            namespace: 0,
            ticket: state.plugin_read_ticket(),
            field: 0,
        }),
        1,
        true,
    );
    let old = state.plugin_draft_ticket().unwrap();
    state.handle(
        Action::Plugins(plugins::Action::Bool {
            ticket: old,
            value: true,
        }),
        1,
        true,
    );
    state.handle(
        Action::Plugins(plugins::Action::Review(
            state.plugin_draft_ticket().unwrap(),
        )),
        1,
        true,
    );
    let reviewed = state.plugin_draft_ticket().unwrap();
    state.handle(Action::SelectPage(Page::General), 1, true);
    assert!(
        state
            .handle(Action::Plugins(plugins::Action::Confirm(reviewed)), 1, true)
            .is_none()
    );
    assert!(state.plugin_draft_ticket().is_none());
}
#[test]
fn plugin_queue_refusal_and_disconnect_never_retry_or_confirm_new_panel() {
    let mut state = opened_plugins();
    let submitted = submit_plugin(&mut state);
    state.close();
    assert!(state.handle(Action::Open, 1, true).is_none());
    state.plugin_saved(submitted, plugins::SaveOutcome::NotSent);
    assert!(!state.plugins.pending());
    assert!(state.plugin_draft_ticket().is_none());
    state.disconnect();
    assert!(
        state
            .handle(
                Action::Plugins(plugins::Action::Refresh(state.plugin_read_ticket())),
                1,
                true
            )
            .is_none()
    );
}

#[test]
fn deepseek_page_renders_only_the_loaded_llm_deepseek_namespace() {
    let mut state = Settings::default();
    let _ = state.handle(Action::Open, 1, true);
    assert_eq!(state.page, Page::ApiLogin);
    // Nothing is rendered before a read returns the namespace.
    assert!(
        state
            .plugins
            .view_namespace(state.plugin_context(true), "llm-deepseek")
            .is_none()
    );
    let rendered = state.plugin_read_ticket();
    let Some(Effect::Plugins(plugins::Effect::Read(ticket))) =
        state.handle(Action::Plugins(plugins::Action::Refresh(rendered)), 1, true)
    else {
        panic!("explicit plugin refresh required")
    };
    assert!(
        state
            .plugins
            .view_namespace(state.plugin_context(true), "llm-deepseek")
            .is_none()
    );
    state
        .plugins
        .loaded(ticket, Ok(snapshot_named("llm-deepseek")));
    assert!(
        state
            .plugins
            .view_namespace(state.plugin_context(true), "llm-deepseek")
            .is_some()
    );
    // A different namespace is never rendered on the DeepSeek page.
    assert!(
        state
            .plugins
            .view_namespace(state.plugin_context(true), "PUBLIC-other")
            .is_none()
    );
    let _: Element<'_, Action> = state.view_sized(true, 960.0, 700.0, &RouteFacts::default());
}

/// The General page's session-log row is a write like any other: when the app is read-only it must
/// not dispatch one. The arm used to build its context with a hard-coded `true`, so this one
/// control stayed live while every sibling refused — and a bounced write was reported as a save
/// failure rather than the read-only state it was.
#[test]
fn the_session_log_row_writes_only_when_the_app_is_writable() {
    let mut state = opened_plugins();
    let rendered = state.plugin_read_ticket();
    let Some(Effect::Plugins(plugins::Effect::Read(ticket))) =
        state.handle(Action::Plugins(plugins::Action::Refresh(rendered)), 1, true)
    else {
        panic!("explicit plugin refresh required")
    };
    state.plugins_loaded(ticket, Ok(snapshot_named("session-log-deepseek")));
    assert!(
        state.plugins.has_snapshot(),
        "the snapshot must be loaded before the row can write"
    );
    // The snapshot's stored value is `false`, so the row turns it on: an unchanged value is a
    // local no-op, not a write.
    let Some(Effect::Plugins(plugins::Effect::Save { .. })) =
        state.handle(Action::SessionLog(true), 1, true)
    else {
        panic!(
            "a writable page writes (draft: {:?})",
            state.plugin_draft_ticket()
        )
    };
    // Same loaded snapshot, read-only caller: no effect, and nothing marked pending.
    assert!(
        state.handle(Action::SessionLog(false), 1, false).is_none(),
        "a read-only app does not write"
    );
    assert!(
        state.handle(Action::SessionLog(true), 1, false).is_none(),
        "not even a real change"
    );
}

/// A failed preferences read and a non-confirmed save both leave the General row unable to do
/// anything, and the page had no way to try again. One control re-runs the same read.
#[test]
fn the_general_page_can_retry_its_preferences_read() {
    let mut state = opened_plugins();
    // The read fails: no snapshot, so the row is the stated absence with a retry.
    let rendered = state.plugin_read_ticket();
    let Some(Effect::Plugins(plugins::Effect::Read(ticket))) =
        state.handle(Action::Plugins(plugins::Action::Refresh(rendered)), 1, true)
    else {
        panic!("explicit refresh required")
    };
    state.plugins_loaded(ticket, Err(crate::plugins::ReadFailure::Unavailable));
    assert!(!state.plugins.has_snapshot());
    let Some(Effect::Plugins(plugins::Effect::Read(_))) =
        state.handle(Action::RefreshPreferences, 1, true)
    else {
        panic!("the retry reads again")
    };
}
