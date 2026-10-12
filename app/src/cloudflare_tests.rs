use super::*;
const ACCOUNT: &str = "0123456789abcdef0123456789abcdef";
fn snapshot() -> CloudflareSnapshot {
    CloudflareSnapshot {
        account_id: ACCOUNT.into(),
        models_enabled: false,
        browser_enabled: false,
        free_plan_confirmed: false,
        token_configured: true,
        writable: true,
        revision: 4,
        auth_status: dsh_native_core::CloudflareAuthStatus {
            credential_stored: true,
            credential_writable: true,
            ..Default::default()
        },
        oauth_setup: Default::default(),
    }
}
fn ready() -> Controller {
    let mut state = Controller::default();
    state.enter(1);
    let Some(Effect::Read(ticket)) = state.handle(Action::Read(state.ticket()), 1, true) else {
        panic!("explicit read")
    };
    state.completed(ticket, Outcome::Snapshot(snapshot()));
    state
}
fn edit(state: &mut Controller, action: impl FnOnce(Ticket) -> Action) {
    assert!(state.handle(action(state.ticket()), 1, true).is_none());
}
fn enable(state: &mut Controller) {
    edit(state, |t| Action::Models(t, true));
    edit(state, |t| Action::Free(t, true));
}
fn save(state: &mut Controller) -> Effect {
    edit(state, Action::Review);
    state
        .handle(Action::Confirm(state.ticket()), 1, true)
        .expect("reviewed save")
}
#[test]
fn enable_requires_free_plan_attestation_account_and_token() {
    let mut state = ready();
    edit(&mut state, |t| Action::Models(t, true));
    assert!(!state.can_save(true));
    edit(&mut state, |t| Action::Free(t, true));
    assert!(state.can_save(true));
    edit(&mut state, |t| Action::Account(t, String::new()));
    assert!(!state.can_save(true));
    edit(&mut state, |t| Action::Account(t, ACCOUNT.into()));
    assert!(!state.can_save(true));
    edit(&mut state, |t| Action::Free(t, true));
    state.metadata.as_mut().unwrap().token_configured = false;
    assert!(!state.can_save(true));
    edit(&mut state, |t| {
        Action::Token(t, key_input("PUBLIC_FAKE_TOKEN".into()))
    });
    assert!(state.can_save(true));
    edit(&mut state, |t| Action::Free(t, false));
    assert!(!state.can_save(true));
    assert!(
        state
            .handle(Action::Confirm(state.ticket()), 1, true)
            .is_none()
    );
}
#[test]
fn account_change_requires_fresh_free_plan_confirmation() {
    let mut state = ready();
    enable(&mut state);
    edit(&mut state, |t| Action::Account(t, ACCOUNT.into()));
    assert!(state.can_save(true));
    edit(&mut state, Action::Review);
    assert!(state.review);
    edit(&mut state, |t| Action::Account(t, "a".repeat(32)));
    assert!(!state.free);
    assert!(!state.review);
    assert!(!state.can_save(true));
    assert!(
        state
            .handle(Action::Confirm(state.ticket()), 1, true)
            .is_none()
    );
    edit(&mut state, |t| Action::Free(t, true));
    assert!(state.can_save(true));
}
#[test]
fn review_is_invalidated_by_edits_and_secret_is_consumed_once() {
    let mut state = ready();
    enable(&mut state);
    edit(&mut state, |t| {
        Action::Token(t, key_input("PUBLIC_FAKE_TOKEN".into()))
    });
    edit(&mut state, Action::Review);
    let old = state.ticket();
    edit(&mut state, |t| Action::Browser(t, true));
    assert!(state.handle(Action::Confirm(old), 1, true).is_none());
    let effect = save(&mut state);
    assert!(state.token.as_str().is_empty());
    assert!(
        state
            .handle(Action::Confirm(state.ticket()), 1, true)
            .is_none()
    );
    let Effect::Save {
        token,
        revision,
        models,
        browser,
        free,
        ..
    } = effect
    else {
        panic!("secret save")
    };
    assert!(token.is_some());
    assert_eq!(revision, 4);
    assert!(models && browser && free);
    assert!(!format!("{token:?}").contains("PUBLIC_FAKE"));
}
#[test]
fn blank_keeps_token_and_navigation_clears_draft_without_request() {
    let mut state = ready();
    enable(&mut state);
    let Effect::Save { token, .. } = save(&mut state) else {
        panic!("save")
    };
    assert!(token.is_none());
    let pending = state.pending;
    state.leave();
    assert_eq!(state.pending, pending);
    assert!(state.token.as_str().is_empty());
    state.enter(1);
    assert!(state.metadata.is_none());
    state.completed(pending.unwrap(), Outcome::Snapshot(snapshot()));
    assert!(!state.pending());
    assert!(state.metadata.is_none()); // receipt never installs old draft
    assert!(state.footer().is_some());
}
#[test]
fn unknown_write_requires_explicit_read_then_preserves_warning_and_never_retries() {
    let mut state = ready();
    enable(&mut state);
    let effect = save(&mut state);
    let ticket = effect.ticket();
    drop(effect);
    state.completed(ticket, Outcome::Unknown);
    assert!(state.require_read && !state.can_write(true));
    let warning = state.footer();
    assert!(
        state
            .handle(Action::Disable(state.ticket()), 1, true)
            .is_none()
    );
    state.leave();
    state.enter(1);
    assert_eq!(state.footer(), warning);
    let Some(Effect::Read(read)) = state.handle(Action::Read(state.ticket()), 1, true) else {
        panic!("explicit read recovery")
    };
    state.completed(read, Outcome::Snapshot(snapshot()));
    assert!(!state.require_read && state.can_write(true));
    assert_eq!(state.footer(), warning);
    assert!(state.pending.is_none());
}
#[test]
fn not_sent_and_failed_read_are_visible_and_stale_callbacks_cannot_change_drafts() {
    let mut state = ready();
    let effect = state
        .handle(Action::Disable(state.ticket()), 1, true)
        .unwrap();
    state.completed(effect.ticket(), Outcome::NotSent);
    assert!(state.footer().unwrap().0.contains("NotSent"));
    let Some(Effect::Read(read)) = state.handle(Action::Read(state.ticket()), 1, true) else {
        panic!("read")
    };
    state.completed(read, Outcome::Unknown);
    assert!(state.metadata.is_none());
    assert!(!state.can_write(true));
    let stale = state.ticket();
    state.leave();
    state.enter(1);
    state.completed(read, Outcome::Snapshot(snapshot()));
    assert!(state.metadata.is_none());
    let input = key_input("PUBLIC_FAKE_STALE".into());
    let replay = input.clone();
    assert!(state.handle(Action::Token(stale, input), 1, true).is_none());
    assert!(replay.take().is_none());
    assert!(state.token.as_str().is_empty());
}
#[test]
fn disconnect_unknown_settlement_and_replayed_receipt_are_fenced() {
    let mut state = ready();
    enable(&mut state);
    let effect = save(&mut state);
    let ticket = effect.ticket();
    drop(effect);
    state.disconnect();
    assert!(state.require_read);
    assert!(!state.pending());
    state.completed(ticket, Outcome::Snapshot(snapshot()));
    assert!(state.metadata.is_none());
    state.enter(2);
    assert!(state.handle(Action::Read(ticket), 2, true).is_none());
}
#[test]
fn settings_navigation_has_no_requests_and_warning_footer_survives_categories() {
    use crate::settings::{Action as SettingsAction, Effect as SettingsEffect, Page, Settings};
    let mut settings = Settings::default();
    settings.handle(SettingsAction::Open, 1, false);
    assert!(
        settings
            .handle(SettingsAction::SelectPage(Page::Cloudflare), 1, true)
            .is_none()
    );
    let ticket = settings.cloudflare.ticket();
    let Some(SettingsEffect::Cloudflare(Effect::Read(read))) =
        settings.handle(SettingsAction::Cloudflare(Action::Read(ticket)), 1, true)
    else {
        panic!("read")
    };
    settings.cloudflare_completed(read, Outcome::Snapshot(snapshot()));
    let ticket = settings.cloudflare.ticket();
    let Some(SettingsEffect::Cloudflare(effect)) =
        settings.handle(SettingsAction::Cloudflare(Action::Disable(ticket)), 1, true)
    else {
        panic!("disable")
    };
    let pending = effect.ticket();
    drop(effect);
    for page in Page::ALL {
        assert!(
            settings
                .handle(SettingsAction::SelectPage(page), 1, true)
                .is_none()
        );
        assert!(settings.cloudflare.pending());
        let _: Element<'_, SettingsAction> =
            settings.view_sized(true, 960.0, 700.0, &crate::settings::RouteFacts::default());
    }
    settings.cloudflare_completed(pending, Outcome::Unknown);
    let warning = settings.cloudflare.footer();
    for page in Page::ALL {
        assert!(
            settings
                .handle(SettingsAction::SelectPage(page), 1, true)
                .is_none()
        );
        assert_eq!(settings.cloudflare.footer(), warning);
        let _: Element<'_, SettingsAction> =
            settings.view_sized(false, 600.0, 420.0, &crate::settings::RouteFacts::default());
    }
}
#[test]
fn footer_distinguishes_acknowledgment_from_failures_and_reads_keep_unknown_warning() {
    let mut state = ready();
    let effect = state
        .handle(Action::Disable(state.ticket()), 1, true)
        .unwrap();
    assert!(!state.footer().unwrap().1); // Pending is neutral.
    state.completed(effect.ticket(), Outcome::Snapshot(snapshot()));
    let acknowledged = state.footer().unwrap();
    assert!(acknowledged.0.contains("acknowledged"));
    assert!(!acknowledged.1); // A confirmed local write is not an error.
    let Some(Effect::Read(read)) = state.handle(Action::Read(state.ticket()), 1, true) else {
        panic!("read")
    };
    state.completed(read, Outcome::Unknown);
    assert!(state.footer().unwrap().1); // Read failures still need attention.

    let mut state = ready();
    let effect = state
        .handle(Action::Disable(state.ticket()), 1, true)
        .unwrap();
    state.completed(effect.ticket(), Outcome::NotSent);
    assert!(state.footer().unwrap().1);
    let effect = state
        .handle(Action::Disable(state.ticket()), 1, true)
        .unwrap();
    state.completed(effect.ticket(), Outcome::Unknown);
    let unknown = state.footer().unwrap();
    assert!(unknown.0.contains("token identity") && unknown.1);
    for outcome in [
        Outcome::Unknown,
        Outcome::Snapshot(snapshot()),
        Outcome::Unknown,
    ] {
        let Some(Effect::Read(read)) = state.handle(Action::Read(state.ticket()), 1, true) else {
            panic!("explicit read")
        };
        state.completed(read, outcome);
        assert_eq!(state.footer(), Some(unknown));
    }
}
#[test]
fn native_elements_construct_read_edit_review_pending_and_failure_states() {
    let mut state = Controller::default();
    state.enter(1);
    let _: Element<'_, Action> = state.view(false);
    state = ready();
    enable(&mut state);
    let _: Element<'_, Action> = state.view(true);
    edit(&mut state, Action::Review);
    let _: Element<'_, Action> = state.view(true);
    let effect = state
        .handle(Action::Confirm(state.ticket()), 1, true)
        .unwrap();
    let _: Element<'_, Action> = state.view(true);
    state.completed(effect.ticket(), Outcome::Unknown);
    let _: Element<'_, Action> = state.view(true);
}

fn oauth_ready() -> Controller {
    let mut c = ready();
    let mut meta = snapshot();
    meta.oauth_setup = CloudflareOauthSetup {
        client_id: "PUBLIC-client".into(),
        scopes: vec!["PUBLIC:scope".into()],
        callback_port: 8977,
    };
    meta.auth_status.setup_configured = true;
    c.auth = Some(meta.auth_status.clone());
    c.apply_snapshot(meta);
    c
}
#[test]
fn pasted_text_lands_in_the_field_it_was_pasted_into() {
    let mut c = ready();
    c.client_id = "PUBLIC-".into();
    c.scopes = "account.read".into();
    let ticket = c.ticket();
    // Every tagged field is reachable by the id the widget tree reports, and its
    // paste adds to what is already typed instead of replacing it.
    assert_eq!(Field::of(Field::ClientId.id()), Some(Field::ClientId));
    assert_eq!(Field::of("native-not-ours"), None);
    let Action::ClientId(_, value) = Field::ClientId.paste(ticket, &c.client_id, "client-1") else {
        panic!("client id paste")
    };
    assert_eq!(value, "PUBLIC-client-1");
    c.client_id = value;
    assert_eq!(Field::ClientId.value(&c), "PUBLIC-client-1");
    let Action::Account(_, value) = Field::AccountId.paste(ticket, "abc", "def") else {
        panic!("account paste")
    };
    assert_eq!(value, "abcdef");
    let Action::Scopes(_, value) = Field::Scopes.paste(ticket, &c.scopes, " offline_access") else {
        panic!("scopes paste")
    };
    assert_eq!(value, "account.read offline_access");
    let Action::CallbackPort(_, value) = Field::Port.paste(ticket, "", "8977") else {
        panic!("port paste")
    };
    assert_eq!(value, "8977");
}

#[test]
fn a_paste_lands_in_a_field_even_without_a_resolved_caret() {
    // A copied ID arrives with a trailing newline; the app trims it before the
    // field's own rules ever see it, so a valid ID is not silently dropped.
    let mut c = ready();
    let ticket = c.ticket();
    let pasted = "PUBLIC-abc123\n".trim();
    let Action::ClientId(_, value) = Field::ClientId.paste(ticket, "", pasted) else {
        panic!("client id paste")
    };
    assert_eq!(value, "PUBLIC-abc123");
    assert!(c.handle(Action::ClientId(ticket, value), 1, true).is_none());
    assert_eq!(c.client_id, "PUBLIC-abc123");
    assert_eq!(c.last_field, Some(Field::ClientId));

    // An untouched page pastes into the first empty field, then into whatever the
    // user edited last once the caret cannot be named.
    let empty = ready();
    assert_eq!(
        empty.paste_target(),
        Some(Field::ClientId),
        "an untouched page starts with the client ID"
    );
    assert_eq!(c.paste_target(), Some(Field::ClientId));
    c.account = "a".repeat(32);
    assert_eq!(c.paste_target(), Some(Field::ClientId));

    // A paste the field refuses says so instead of vanishing, and leaves what was
    // already there alone.
    let mut refusing = ready();
    let kept = refusing.account.clone();
    let ticket = refusing.ticket();
    assert!(
        refusing
            .handle(Action::Account(ticket, "not-hex!".into()), 1, true)
            .is_none()
    );
    assert_eq!(refusing.account, kept);
    assert!(
        refusing.warning.is_some_and(|(_, attention)| attention),
        "a refused paste is reported"
    );
}

#[test]
fn a_named_paste_reaches_its_field_and_says_so() {
    // The button path names the field, so it cannot depend on a focus lookup.
    let mut c = ready();
    let ticket = c.ticket();
    let Action::Account(_, value) = Field::AccountId.paste(ticket, "", "a".repeat(32).as_str())
    else {
        panic!("account paste")
    };
    assert!(c.handle(Action::Account(ticket, value), 1, true).is_none());
    assert_eq!(c.account, "a".repeat(32));
    c.clipboard_result(Field::AccountId, 32);
    assert!(
        c.clipboard_notice
            .as_deref()
            .is_some_and(|notice| notice.contains("32 characters") && notice.contains("account ID")),
        "the paste is reported: {:?}",
        c.clipboard_notice
    );
    c.clipboard_empty();
    assert!(
        c.clipboard_notice
            .as_deref()
            .is_some_and(|notice| notice.contains("no text")),
        "an empty clipboard is reported too"
    );
}

#[test]
fn connect_saves_the_typed_client_and_then_starts_without_a_second_click() {
    // A host that knows no setup yet, with the page holding what the user typed.
    let mut c = ready();
    c.client_id = "PUBLIC-typed-client".into();
    c.scopes = "PUBLIC:scope".into();
    c.port = String::new();
    let Some(Effect::Setup { ticket, setup, .. }) = c.handle(Action::Connect(c.ticket()), 1, true)
    else {
        panic!("connect must save the client it was given")
    };
    assert_eq!(setup.client_id, "PUBLIC-typed-client");
    assert_eq!(setup.scopes, vec!["PUBLIC:scope".to_owned()]);
    assert_eq!(
        setup.callback_port, 8977,
        "the port shown as the registered redirect is the one saved"
    );
    // The chain waits for the save's own acknowledgement rather than starting a
    // sign-in against a setup the Host has not accepted.
    assert!(c.continue_connection(1, true).is_none());
    let mut meta = snapshot();
    meta.oauth_setup = setup.clone();
    meta.auth_status.setup_configured = true;
    c.completed(ticket, Outcome::Snapshot(meta));
    let Some(Effect::Start { .. }) = c.continue_connection(1, true) else {
        panic!("the same connect continues into the sign-in once the setup is saved")
    };

    // With nothing typed there is nothing to save, and the page still says so.
    let mut empty = ready();
    empty.scopes = "PUBLIC:scope".into();
    empty.port = "8977".into();
    assert!(
        empty
            .handle(Action::Connect(empty.ticket()), 1, true)
            .is_none()
    );
    assert!(empty.setup_open, "an unsaveable setup opens its fields");
    assert!(
        empty
            .warning
            .as_ref()
            .is_some_and(|(_, attention)| *attention),
        "and says why"
    );
}

#[test]
fn oauth_click_has_one_browser_grant_and_does_not_enable_features() {
    let mut c = oauth_ready();
    let Some(Effect::Start { ticket, .. }) = c.handle(Action::Connect(c.ticket()), 1, true) else {
        panic!("owned start")
    };
    let mut meta = c.metadata.clone().unwrap();
    meta.auth_status.phase = CloudflareAuthPhase::WaitingBrowser;
    meta.auth_status.attempt_id = Some("PUBLIC-attempt".into());
    meta.auth_status.sequence = 1;
    c.observed(CloudflareAuthStatus {
        browser_available: true,
        ..meta.auth_status.clone()
    });
    assert!(c.continue_connection(1, true).is_none()); // Start ACK is mandatory.
    c.completed(ticket, Outcome::Snapshot(meta));
    // Later same-sequence private browser capability augments metadata.
    let mut status = c.auth.clone().unwrap();
    status.browser_available = true;
    c.observed(status);
    let Some(Effect::OpenBrowser { ticket, permit, .. }) = c.continue_connection(1, true) else {
        panic!("owned opener")
    };
    assert!(permit.load(Ordering::Acquire));
    assert!(c.continue_connection(1, true).is_none());
    c.completed(ticket, Outcome::BrowserOpened);
    assert!(c.continue_connection(1, true).is_none());
    assert!(!c.models && !c.browser && !c.free);
    c.leave();
    assert!(!permit.load(Ordering::Acquire));
}
#[test]
fn cancel_before_start_receipt_retires_permit_and_late_events_cannot_open() {
    let mut c = oauth_ready();
    let Some(Effect::Start {
        ticket: start,
        permit,
        ..
    }) = c.handle(Action::Connect(c.ticket()), 1, true)
    else {
        panic!("start")
    };
    let Some(Effect::CancelLogin {
        ticket: cancel,
        attempt: None,
    }) = c.handle(Action::CancelConnect(c.ticket()), 1, true)
    else {
        panic!("reserved cancel")
    };
    assert!(!permit.load(Ordering::Acquire));
    let mut meta = c.metadata.clone().unwrap();
    meta.auth_status.phase = CloudflareAuthPhase::WaitingBrowser;
    meta.auth_status.attempt_id = Some("PUBLIC-late".into());
    meta.auth_status.sequence = 2;
    meta.auth_status.browser_available = true;
    c.completed(start, Outcome::Snapshot(meta));
    assert!(c.continue_connection(1, true).is_none());
    c.completed(cancel, Outcome::Unknown);
    assert!(c.require_read);
    assert!(c.handle(Action::Connect(c.ticket()), 1, true).is_none());
}
#[test]
fn terminal_before_start_ack_and_panel_epoch_changes_never_revive_browser() {
    let mut c = oauth_ready();
    let Some(Effect::Start { ticket, permit, .. }) = c.handle(Action::Connect(c.ticket()), 1, true)
    else {
        panic!("start")
    };
    let mut meta = c.metadata.clone().unwrap();
    meta.auth_status.phase = CloudflareAuthPhase::WaitingBrowser;
    meta.auth_status.attempt_id = Some("PUBLIC-a".into());
    meta.auth_status.sequence = 1;
    c.observed(CloudflareAuthStatus {
        phase: CloudflareAuthPhase::Connected,
        sequence: 2,
        ..meta.auth_status.clone()
    });
    c.completed(ticket, Outcome::Snapshot(meta));
    assert!(c.continue_connection(1, true).is_none());
    assert!(!permit.load(Ordering::Acquire));
    c.adopt_epoch(2);
    assert!(c.handle(Action::Connect(ticket), 2, true).is_none());
}
#[test]
fn cancel_during_initial_read_never_starts_oauth_and_setup_requires_exact_scope_tokens() {
    let mut c = Controller::default();
    c.enter(1);
    let Some(Effect::Read(ticket)) = c.handle(Action::Connect(c.ticket()), 1, true) else {
        panic!("local read")
    };
    assert!(
        c.handle(Action::CancelConnect(c.ticket()), 1, true)
            .is_none()
    );
    c.completed(ticket, Outcome::Snapshot(snapshot()));
    assert!(c.continue_connection(1, true).is_none());
    let mut c = oauth_ready();
    edit(&mut c, |t| Action::Scopes(t, "duplicate duplicate".into()));
    assert!(c.handle(Action::SaveSetup(c.ticket()), 1, true).is_none());
}

#[test]
fn disconnect_retires_both_start_and_reserved_cancel_and_old_epoch_settlement() {
    let mut c = oauth_ready();
    let Some(Effect::Start { ticket: start, .. }) = c.handle(Action::Connect(c.ticket()), 1, true)
    else {
        panic!("start");
    };
    let Some(Effect::CancelLogin { ticket: cancel, .. }) =
        c.handle(Action::CancelConnect(c.ticket()), 1, true)
    else {
        panic!("cancel");
    };
    c.adopt_epoch(2);
    assert!(!c.pending());
    assert!(c.require_read);
    let mut meta = snapshot();
    meta.auth_status.phase = CloudflareAuthPhase::WaitingBrowser;
    meta.auth_status.attempt_id = Some("PUBLIC-old".into());
    meta.auth_status.browser_available = true;
    c.completed(start, Outcome::Snapshot(meta.clone()));
    c.completed(cancel, Outcome::Snapshot(meta));
    assert!(c.auth.is_none());
    assert!(!c.pending());
}
