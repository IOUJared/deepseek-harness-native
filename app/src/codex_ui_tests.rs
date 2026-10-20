//! Actual App message/worker-queue routing with public metadata only; no backend or browser.
use super::*;
use crate::codex::{Action as ConnectAction, Effect, Outcome};
use crate::settings::{Action as SettingsAction, Page};
use dsh_native_core::{CodexMetadata, CodexPhase};

fn usage_action(app: &mut App, action: crate::codex::UsageAction) {
    let ticket = app.settings.codex_ticket();
    let _ = app.update(Message::Settings(SettingsAction::Codex(
        ConnectAction::Usage { ticket, action },
    )));
}
fn usage_snapshot() -> dsh_native_core::CodexUsageSnapshot {
    dsh_native_core::CodexUsageSnapshot {
        version: 1,
        revision: 4,
        persisted: true,
        preferences: dsh_native_core::CodexUsagePreferences {
            composer_display: dsh_native_core::CodexComposerDisplay::Runway,
            usage_alert: dsh_native_core::CodexUsageAlert::Custom,
            custom_remaining_percent: dsh_native_core::CodexRemainingPercent::new(19.25).unwrap(),
            retry_after_reset: true,
            experimental_quota: true,
        },
    }
}
fn usage_read(app: &mut App, receiver: &mut tokio::sync::mpsc::Receiver<Command>) {
    usage_action(app, crate::codex::UsageAction::Refresh);
    let Effect::UsageRead { ticket } = effect(receiver) else {
        panic!("private preferences only")
    };
    complete(app, ticket, Outcome::UsageSnapshot(usage_snapshot()));
    assert!(receiver.try_recv().is_err());
}
#[test]
fn usage_actual_ui_queue_reads_saves_and_reloads_without_login_models_or_provider_calls() {
    use crate::codex::UsageAction;
    let (mut app, mut receiver) = app();
    usage_read(&mut app, &mut receiver);
    for action in [
        UsageAction::Composer(dsh_native_core::CodexComposerDisplay::Runway),
        UsageAction::Alert(dsh_native_core::CodexUsageAlert::Custom),
        UsageAction::Threshold("19.25".into()),
        UsageAction::Retry(true),
        UsageAction::Consent(true),
    ] {
        usage_action(&mut app, action);
        usage_action(&mut app, UsageAction::Save); // Enabling and unchanged writes are rejected by native facade.
        assert!(receiver.try_recv().is_err());
    }
    for action in [
        UsageAction::Composer(dsh_native_core::CodexComposerDisplay::Off),
        UsageAction::Alert(dsh_native_core::CodexUsageAlert::Off),
        UsageAction::Retry(false),
        UsageAction::Consent(false),
    ] {
        usage_action(&mut app, action);
        assert!(receiver.try_recv().is_err());
    }
    usage_action(&mut app, UsageAction::Save);
    let Effect::UsageSave {
        ticket,
        expected_revision,
        preferences,
    } = effect(&mut receiver)
    else {
        panic!("private opt-out save")
    };
    assert_eq!(expected_revision, 4);
    assert_eq!(
        preferences.composer_display,
        dsh_native_core::CodexComposerDisplay::Off
    );
    assert_eq!(
        preferences.usage_alert,
        dsh_native_core::CodexUsageAlert::Off
    );
    assert!(!preferences.retry_after_reset && !preferences.experimental_quota);
    assert_eq!(preferences.custom_remaining_percent.as_f64(), 19.25);
    usage_action(&mut app, UsageAction::Save);
    assert!(receiver.try_recv().is_err());
    let saved = dsh_native_core::CodexUsageSnapshot {
        version: 1,
        revision: 5,
        preferences,
        persisted: true,
    };
    complete(&mut app, ticket, Outcome::UsageSnapshot(saved));
    assert!(receiver.try_recv().is_err());
    usage_action(&mut app, UsageAction::Refresh);
    let Effect::UsageRead { ticket } = effect(&mut receiver) else {
        panic!()
    };
    complete(&mut app, ticket, Outcome::UsageSnapshot(saved));
    assert!(receiver.try_recv().is_err());
    assert_eq!(app.smoke_evidence.model_prompts, 0);
}
#[test]
fn initial_real_status_receipt_admits_one_usage_read_and_failed_read_never_loops() {
    let (mut app, mut receiver) = app();
    let ticket = app.settings.codex_ticket();
    let _ = app.update(Message::Settings(SettingsAction::Codex(
        ConnectAction::CheckStatus(ticket),
    )));
    let Effect::StatusRead { ticket } = effect(&mut receiver) else {
        panic!()
    };
    let accounts = dsh_native_core::CodexAccountsMetadata {
        mode: dsh_native_core::CodexAccountMode::Managed,
        accounts: vec![],
        selected: None,
        epoch: Some(0),
        legacy: dsh_native_core::CodexLegacyGrant::Absent,
        added: None,
    };
    complete(
        &mut app,
        ticket,
        Outcome::StatusSnapshot {
            metadata: idle(),
            accounts: accounts.clone(),
        },
    );
    let Effect::UsageRead { ticket } = effect(&mut receiver) else {
        panic!("one bounded preference read")
    };
    complete(&mut app, ticket, Outcome::UsageUnavailable);
    for _ in 0..3 {
        app.continue_codex_connection();
    }
    assert!(receiver.try_recv().is_err());
    let ticket = app.settings.codex_ticket();
    let _ = app.update(Message::Settings(SettingsAction::Codex(
        ConnectAction::CheckStatus(ticket),
    )));
    let Effect::StatusRead { ticket } = effect(&mut receiver) else {
        panic!()
    };
    complete(
        &mut app,
        ticket,
        Outcome::StatusSnapshot {
            metadata: idle(),
            accounts,
        },
    );
    assert!(receiver.try_recv().is_err());
}
#[test]
fn usage_queue_rejection_drains_without_replaying_save_and_hide_preserves_pending_write() {
    use crate::codex::UsageAction;
    let (mut app, mut receiver) = app();
    usage_read(&mut app, &mut receiver);
    usage_action(&mut app, UsageAction::Retry(false)); // Functional opt-out of prior saved choice.
    for _ in 0..32 {
        assert!(app.handle.send(Command::Inspect).is_ok());
    }
    usage_action(&mut app, UsageAction::Save); // Actual queue admission rejects, not an ACK.
    while receiver.try_recv().is_ok() {}
    app.continue_codex_connection();
    assert!(receiver.try_recv().is_err());
    usage_action(&mut app, UsageAction::Save); // Only a new explicit action can retry known NotSent.
    let Effect::UsageSave {
        ticket,
        expected_revision,
        preferences,
    } = effect(&mut receiver)
    else {
        panic!()
    };
    let _ = app.update(Message::Settings(SettingsAction::Close));
    app.settings.handle(SettingsAction::Open, 1, false);
    let _ = app.update(Message::Settings(SettingsAction::SelectPage(Page::Codex)));
    usage_action(&mut app, UsageAction::Save);
    assert!(receiver.try_recv().is_err());
    complete(
        &mut app,
        ticket,
        Outcome::UsageSnapshot(dsh_native_core::CodexUsageSnapshot {
            version: 1,
            revision: expected_revision + 1,
            preferences,
            persisted: true,
        }),
    );
    usage_action(&mut app, UsageAction::Save);
    assert!(receiver.try_recv().is_err()); // Old-panel write cannot silently authorize another Save.
    usage_action(&mut app, UsageAction::Refresh);
    assert!(matches!(effect(&mut receiver), Effect::UsageRead { .. }));
}

fn idle() -> CodexMetadata {
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
fn waiting(browser: bool) -> CodexMetadata {
    CodexMetadata {
        phase: CodexPhase::WaitingBrowser,
        attempt: Some(11),
        prompt: Some(17),
        browser_available: browser,
        ..idle()
    }
}
fn authorized() -> CodexMetadata {
    CodexMetadata {
        phase: CodexPhase::Authorized,
        credential_stored: true,
        prompt: None,
        browser_available: false,
        ..waiting(false)
    }
}
fn app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
    let (handle, feed, mut receiver, _events, _closing) = worker::test_channels();
    let options = crate::config::parse(
        [
            "--runtime",
            "/explicit-fork/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/isolated-native-home",
            "--user-home",
            "/user-home",
            "--cwd",
            "/tmp",
        ]
        .map(str::to_owned),
    )
    .unwrap()
    .unwrap();
    let (mut app, _) = App::boot(options, handle, feed);
    let _ = app.receive(Event::Ready("0.2.1-alpha.1".into()));
    assert!(matches!(receiver.try_recv(), Ok(Command::Catalog(_))));
    // The header's reminder count reads the Host's schedule catalog
    // (`ScheduleCatalogAction.tsx:58-76`); it is a read, not an auth or model-selection call.
    assert!(matches!(receiver.try_recv(), Ok(Command::ScheduleCatalog)));
    // The workspace opener's installed-app catalog is the third automatic startup read.
    assert!(matches!(receiver.try_recv(), Ok(Command::OpenInAppApps)));
    assert!(receiver.try_recv().is_err()); // No unsolicited auth/settings/model-selection calls.
    app.settings.handle(SettingsAction::Open, 1, false);
    let _ = app.update(Message::Settings(SettingsAction::SelectPage(Page::Codex)));
    (app, receiver)
}
fn connect(app: &mut App) {
    let ticket = app.settings.codex_ticket();
    let _ = app.update(Message::Settings(SettingsAction::Codex(
        ConnectAction::Connect(ticket),
    )));
}
fn effect(receiver: &mut tokio::sync::mpsc::Receiver<Command>) -> Effect {
    let Command::Codex(effect) = receiver.try_recv().expect("expected connection step") else {
        panic!("connection must not issue unrelated business/model commands")
    };
    effect
}
fn complete(app: &mut App, ticket: crate::codex::Ticket, outcome: Outcome) {
    let _ = app.receive(Event::CodexCompleted { ticket, outcome });
}
#[test]
fn one_click_actual_ui_queue_chains_status_auth_browser_and_route_without_model_calls() {
    let (mut app, mut receiver) = app();
    assert!(receiver.try_recv().is_err());
    connect(&mut app);
    let Effect::StatusRead { ticket } = effect(&mut receiver) else {
        panic!("local status first")
    };
    complete(&mut app, ticket, Outcome::Metadata(idle()));
    let Effect::StartLogin { ticket } = effect(&mut receiver) else {
        panic!("start once")
    };
    complete(&mut app, ticket, Outcome::Metadata(waiting(false)));
    assert!(receiver.try_recv().is_err());
    let _ = app.receive(Event::Core(PublicEvent::Codex(waiting(true))));
    let Effect::OpenBrowser { ticket, attempt } = effect(&mut receiver) else {
        panic!("browser step")
    };
    assert_eq!(attempt, 11);
    let _ = app.receive(Event::Core(PublicEvent::Codex(waiting(true))));
    assert!(receiver.try_recv().is_err());
    complete(&mut app, ticket, Outcome::Confirmed);
    assert!(receiver.try_recv().is_err());
    let _ = app.receive(Event::Core(PublicEvent::Codex(authorized())));
    let Effect::EnableModels {
        ticket,
        expected_revision,
    } = effect(&mut receiver)
    else {
        panic!("route step")
    };
    assert_eq!(expected_revision, 7);
    complete(
        &mut app,
        ticket,
        Outcome::Metadata(CodexMetadata {
            route_configured: true,
            settings_revision: Some(8),
            ..authorized()
        }),
    );
    assert!(receiver.try_recv().is_err());
    assert_eq!(app.smoke_evidence.model_prompts, 0);
    assert!(app.smoke_evidence.catalog_requested); // Only the startup read drained by app().
}
#[test]
fn closing_modal_stops_follow_up_even_if_start_ack_returns() {
    let (mut app, mut receiver) = app();
    connect(&mut app);
    let Effect::StatusRead { ticket } = effect(&mut receiver) else {
        panic!()
    };
    complete(&mut app, ticket, Outcome::Metadata(idle()));
    let Effect::StartLogin { ticket } = effect(&mut receiver) else {
        panic!()
    };
    let _ = app.update(Message::Settings(SettingsAction::Close));
    complete(&mut app, ticket, Outcome::Metadata(waiting(true)));
    let _ = app.receive(Event::Core(PublicEvent::Codex(authorized())));
    assert!(receiver.try_recv().is_err());
}
#[test]
fn lifecycle_fault_prevents_connection_follow_up() {
    let (mut app, mut receiver) = app();
    connect(&mut app);
    let Effect::StatusRead { ticket } = effect(&mut receiver) else {
        panic!()
    };
    let _ = app.receive(Event::Fault("Public fixture fault".into()));
    complete(&mut app, ticket, Outcome::Metadata(idle()));
    assert!(receiver.try_recv().is_err());
    assert!(!app.ready);
}
#[test]
fn queue_rejection_stops_intent_and_never_replays_on_metadata() {
    let (mut app, mut receiver) = app();
    for _ in 0..32 {
        assert!(app.handle.send(Command::Inspect).is_ok());
    }
    connect(&mut app); // StatusRead rejected, settled as NotSent through actual command() arm.
    while receiver.try_recv().is_ok() {}
    let _ = app.receive(Event::Core(PublicEvent::Codex(idle())));
    app.continue_codex_connection();
    assert!(receiver.try_recv().is_err());
}
#[test]
fn reconnect_existing_local_configuration_never_restarts_auth_or_loads_models() {
    let (mut app, mut receiver) = app();
    connect(&mut app);
    let Effect::StatusRead { ticket } = effect(&mut receiver) else {
        panic!()
    };
    complete(
        &mut app,
        ticket,
        Outcome::Metadata(CodexMetadata {
            credential_stored: true,
            route_configured: true,
            ..idle()
        }),
    );
    assert!(receiver.try_recv().is_err());
    assert!(app.smoke_evidence.catalog_requested); // Reconnect adds no catalog or model calls.
}

#[test]
fn managed_roster_freshness_actual_app_requires_readback_after_interrupted_add() {
    use dsh_native_core::{
        CodexAccount, CodexAccountId, CodexAccountLabel, CodexAccountMode, CodexAccountsMetadata,
        CodexLegacyGrant,
    };
    let id =
        |n: u8| CodexAccountId::new(format!("account-00000000-0000-4000-8000-{n:012x}")).unwrap();
    let row = |n| CodexAccount {
        opaque_id: id(n),
        label: CodexAccountLabel::new(format!("PUBLIC account {n}")).unwrap(),
        email: None,
        plan: None,
    };
    let mut accounts = CodexAccountsMetadata {
        mode: CodexAccountMode::Managed,
        accounts: vec![row(1)],
        selected: None,
        epoch: Some(0),
        legacy: CodexLegacyGrant::Absent,
        added: None,
    };
    let (mut app, mut receiver) = app();
    let dispatch = |app: &mut App, action| {
        let _ = app.update(Message::Settings(SettingsAction::Codex(action)));
    };
    let ticket = app.settings.codex_ticket();
    dispatch(&mut app, ConnectAction::RefreshAccounts(ticket));
    let Effect::AccountsRead { ticket } = effect(&mut receiver) else {
        panic!("explicit read")
    };
    complete(
        &mut app,
        ticket,
        Outcome::StatusSnapshot {
            metadata: idle(),
            accounts: accounts.clone(),
        },
    );
    assert!(receiver.try_recv().is_err());
    let ticket = app.settings.codex_ticket();
    dispatch(
        &mut app,
        ConnectAction::EditLabel {
            ticket,
            label: "PUBLIC new alias".into(),
        },
    );
    dispatch(&mut app, ConnectAction::AddAccount(ticket));
    let Effect::AddAccount {
        ticket,
        expected_epoch: 0,
        ..
    } = effect(&mut receiver)
    else {
        panic!("explicit Add")
    };
    complete(&mut app, ticket, Outcome::Metadata(waiting(false))); // Never dispatch an opener.
    assert!(receiver.try_recv().is_err());
    let ticket = app.settings.codex_ticket();
    dispatch(&mut app, ConnectAction::RefreshAccounts(ticket));
    let Effect::AccountsRead { ticket } = effect(&mut receiver) else {
        panic!("manual read interrupts chain")
    };
    complete(&mut app, ticket, Outcome::Indeterminate);
    let terminal = CodexMetadata {
        credential_stored: false,
        ..authorized()
    };
    let _ = app.receive(Event::Core(PublicEvent::Codex(terminal)));
    let ticket = app.settings.codex_ticket();
    dispatch(
        &mut app,
        ConnectAction::ReviewSelect {
            ticket,
            account_id: id(1),
        },
    );
    dispatch(&mut app, ConnectAction::ConfirmAccount(ticket));
    dispatch(
        &mut app,
        ConnectAction::EditLabel {
            ticket,
            label: "PUBLIC denied stale Add".into(),
        },
    );
    dispatch(&mut app, ConnectAction::AddAccount(ticket));
    assert!(receiver.try_recv().is_err()); // Old roster cannot authorize account work.
    dispatch(&mut app, ConnectAction::RefreshAccounts(ticket));
    let Effect::AccountsRead { ticket } = effect(&mut receiver) else {
        panic!("explicit recovery")
    };
    accounts.accounts.push(row(2)); // A committed Add can preserve epoch zero.
    complete(
        &mut app,
        ticket,
        Outcome::StatusSnapshot {
            metadata: terminal,
            accounts,
        },
    );
    assert!(receiver.try_recv().is_err()); // No automatic select/enable revived.
    let ticket = app.settings.codex_ticket();
    dispatch(
        &mut app,
        ConnectAction::ReviewSelect {
            ticket,
            account_id: id(2),
        },
    );
    assert!(receiver.try_recv().is_err());
    dispatch(&mut app, ConnectAction::ConfirmAccount(ticket));
    assert!(
        matches!(effect(&mut receiver), Effect::SelectAccount { account_id, expected_epoch: 0, .. } if account_id == id(2))
    );
    assert!(receiver.try_recv().is_err());
    assert_eq!(app.smoke_evidence.model_prompts, 0);
}
