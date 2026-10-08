//! Actual App message/worker-queue routing with public metadata only; no backend or browser.
use super::*;
use crate::codex::{Action as ConnectAction, Effect, Outcome};
use crate::settings::{Action as SettingsAction, Page};
use dsh_native_core::{CodexMetadata, CodexPhase};

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
    let (handle, feed, receiver, _events, _closing) = worker::test_channels();
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
    assert!(!app.smoke_evidence.catalog_requested);
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
    assert!(!app.smoke_evidence.catalog_requested);
}
