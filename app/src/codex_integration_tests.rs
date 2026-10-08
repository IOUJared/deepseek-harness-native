use super::*;
use crate::codex::{Action as CodexAction, Effect as CodexEffect, Outcome};
use dsh_native_core::{CodexMetadata, CodexPhase};
fn metadata(phase: CodexPhase) -> CodexMetadata {
    CodexMetadata {
        available: true,
        credential_stored: false,
        route_configured: false,
        phase,
        attempt: Some(11),
        prompt: Some(17),
        browser_available: true,
        settings_revision: Some(7),
        retry_blocked: false,
    }
}
fn open_codex(s: &mut Settings) {
    assert!(s.handle(Action::Open, 1, false).is_none());
    assert!(s.handle(Action::SelectPage(Page::Codex), 1, true).is_none());
}
fn read(s: &mut Settings) -> crate::codex::Ticket {
    let ticket = s.codex_ticket();
    let Some(Effect::Codex(CodexEffect::StatusRead { ticket })) =
        s.handle(Action::Codex(CodexAction::CheckStatus(ticket)), 1, true)
    else {
        panic!("explicit Codex read missing")
    };
    ticket
}
#[test]
fn codex_open_navigation_ready_and_reopen_do_not_authenticate_or_load_models() {
    let mut s = Settings::default();
    open_codex(&mut s);
    s.adopt_epoch(1);
    assert!(s.handle(Action::Close, 1, true).is_none());
    assert!(s.handle(Action::Open, 1, true).is_none());
    let t = read(&mut s);
    s.codex_completed(
        t,
        Outcome::Metadata(CodexMetadata {
            attempt: None,
            prompt: None,
            browser_available: false,
            ..metadata(CodexPhase::Idle)
        }),
    );
    assert!(s.codex_draft_ticket().is_none());
}
#[test]
fn visibility_actual_epoch_and_smoke_independently_block_codex_dispatch() {
    let mut s = Settings::default();
    open_codex(&mut s);
    let ticket = s.codex_ticket();
    assert!(
        s.handle(Action::Codex(CodexAction::CheckStatus(ticket)), 1, false)
            .is_none()
    );
    assert!(
        s.handle(Action::Codex(CodexAction::CheckStatus(ticket)), 2, true)
            .is_none()
    );
    assert!(
        s.handle(Action::SelectPage(Page::General), 1, true)
            .is_none()
    );
    assert!(
        s.handle(Action::Codex(CodexAction::CheckStatus(ticket)), 1, true)
            .is_none()
    );
}
#[test]
fn codex_flight_and_hidden_auth_block_api_and_plugin_mutations() {
    let mut s = Settings::default();
    open_codex(&mut s);
    let ticket = read(&mut s);
    s.codex_completed(
        ticket,
        Outcome::Metadata(metadata(CodexPhase::WaitingBrowser)),
    );
    assert!(
        s.handle(Action::SelectPage(Page::ApiLogin), 1, true)
            .is_none()
    );
    assert!(
        s.handle(
            Action::Edit {
                ticket: s.input_ticket(),
                input: Input::new("PUBLIC_NOT_A_KEY".into())
            },
            1,
            true
        )
        .is_none()
    );
    assert!(s.edit.editor.0.is_empty());
    assert!(s.handle(Action::Refresh(s.ticket()), 1, true).is_none());
    assert!(
        s.handle(Action::SelectPage(Page::Plugins), 1, true)
            .is_none()
    );
    assert!(
        s.handle(
            Action::Plugins(crate::plugins::Action::Refresh(s.plugin_read_ticket())),
            1,
            true
        )
        .is_none()
    );
    assert!(s.handle(Action::SelectPage(Page::Codex), 1, true).is_none());
    // Reattachment is an explicit read, not hydration of a hidden callback editor.
    assert!(s.codex_draft_ticket().is_none());
    let ticket = read(&mut s);
    s.codex_completed(
        ticket,
        Outcome::Metadata(metadata(CodexPhase::WaitingBrowser)),
    );
    assert!(s.codex_draft_ticket().is_some());
    let Some(Effect::Codex(CodexEffect::CancelLogin { attempt, .. })) = s.handle(
        Action::Codex(CodexAction::Cancel {
            ticket: s.codex_ticket(),
            attempt: 11,
        }),
        1,
        true,
    ) else {
        panic!("explicit cancellation must remain available")
    };
    assert_eq!(attempt, 11);
}
#[test]
fn stale_callback_confirmation_cannot_cross_settings_categories() {
    let mut s = Settings::default();
    open_codex(&mut s);
    let ticket = read(&mut s);
    s.codex_completed(
        ticket,
        Outcome::Metadata(metadata(CodexPhase::WaitingBrowser)),
    );
    let draft = s.codex_draft_ticket().unwrap();
    s.handle(
        Action::Codex(CodexAction::Edit {
            ticket: draft,
            input: crate::codex::Input::new("PUBLIC_FAKE_CALLBACK".into()),
        }),
        1,
        true,
    );
    let draft = s.codex_draft_ticket().unwrap();
    s.handle(Action::Codex(CodexAction::ReviewCallback(draft)), 1, true);
    let reviewed = s.codex_draft_ticket().unwrap();
    s.handle(Action::SelectPage(Page::General), 1, true);
    assert!(
        s.handle(
            Action::Codex(CodexAction::ConfirmCallback(reviewed)),
            1,
            true
        )
        .is_none()
    );
    s.disconnect();
    assert!(s.codex_draft_ticket().is_none());
}
#[test]
fn admitted_api_key_write_blocks_codex_status_and_start() {
    let mut s = Settings::default();
    s.handle(Action::Open, 1, false);
    s.save = SavePhase::Pending(s.ticket());
    s.handle(Action::SelectPage(Page::Codex), 1, true);
    let ticket = s.codex_ticket();
    assert!(
        s.handle(Action::Codex(CodexAction::CheckStatus(ticket)), 1, true)
            .is_none()
    );
    assert!(
        s.handle(Action::Codex(CodexAction::Start(ticket)), 1, true)
            .is_none()
    );
}
