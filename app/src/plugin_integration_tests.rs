use super::*;
use crate::plugins::{self, ReadTicket, WriteTicket};
use dsh_native_transport::plugin::*;

fn snapshot() -> plugins::Snapshot {
    plugins::Snapshot {
        inventory: PluginInventorySnapshot {
            entries: vec![],
            management_available: false,
        },
        settings: SettingsDescribeValue {
            writable: true,
            namespaces: vec![SettingsNamespaceView {
                ns: "PUBLIC-fixture".into(),
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
        Action::Plugins(plugins::Action::Begin { ticket, field: 0 }),
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
            input: Input::new("PUBLIC_DUMMY".into()),
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
            input: Input::new("PUBLIC_DUMMY".into()),
        },
        1,
        true,
    );
    assert!(state.edit.editor.0.is_empty());
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
