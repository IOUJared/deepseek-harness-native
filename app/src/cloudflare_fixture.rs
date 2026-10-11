//! PUBLIC local render/reducer states. Effects are consumed here, never dispatched.
use super::App;
use crate::settings::{self, cloudflare};

pub(super) fn seed(app: &mut App, preset: &str) {
    let effect = app
        .settings
        .handle(
            settings::Action::Cloudflare(cloudflare::Action::Read(
                app.settings.cloudflare_ticket(),
            )),
            app.transport_epoch,
            true,
        )
        .expect("PUBLIC read, not dispatched");
    let settings::Effect::Cloudflare(cloudflare::Effect::Read(ticket)) = effect else {
        panic!("PUBLIC read")
    };
    let oauth = preset.starts_with("settings-cloudflare-oauth-");
    let mut auth = dsh_native_core::CloudflareAuthStatus {
        credential_stored: !oauth,
        credential_writable: true,
        ..Default::default()
    };
    if oauth {
        auth.auth_mode = dsh_native_core::CloudflareAuthMode::Oauth;
        auth.setup_configured = true;
    }
    if preset == "settings-cloudflare-oauth-connected" {
        auth.phase = dsh_native_core::CloudflareAuthPhase::Connected;
        auth.credential_stored = true;
    }
    if preset == "settings-cloudflare-oauth-indeterminate" {
        auth.phase = dsh_native_core::CloudflareAuthPhase::Indeterminate;
        auth.retry_blocked = true;
        auth.error_code = Some(dsh_native_core::CloudflareAuthError::Indeterminate);
    }
    app.settings.cloudflare_completed(
        ticket,
        cloudflare::Outcome::Snapshot(dsh_native_core::CloudflareSnapshot {
            account_id: "0123456789abcdef0123456789abcdef".into(),
            models_enabled: false,
            browser_enabled: false,
            free_plan_confirmed: false,
            token_configured: auth.credential_stored,
            writable: true,
            revision: 4,
            auth_status: auth.clone(),
            oauth_setup: if oauth {
                dsh_native_core::CloudflareOauthSetup {
                    client_id: "PUBLIC-client-not-real".into(),
                    scopes: vec!["PUBLIC:registered-scope".into()],
                    callback_port: 8977,
                }
            } else {
                Default::default()
            },
        }),
    );
    if oauth {
        if preset == "settings-cloudflare-oauth-setup" {
            let ticket = app.settings.cloudflare_ticket();
            drop(app.settings.handle(
                settings::Action::Cloudflare(cloudflare::Action::ToggleSetup(ticket)),
                app.transport_epoch,
                true,
            ));
        }
        if preset == "settings-cloudflare-oauth-waiting" {
            let ticket = app.settings.cloudflare_ticket();
            let Some(settings::Effect::Cloudflare(effect)) = app.settings.handle(
                settings::Action::Cloudflare(cloudflare::Action::Connect(ticket)),
                app.transport_epoch,
                true,
            ) else {
                panic!("PUBLIC start");
            };
            let ticket = effect.ticket();
            drop(effect);
            auth.phase = dsh_native_core::CloudflareAuthPhase::WaitingBrowser;
            auth.attempt_id = Some("PUBLIC-owned-attempt".into());
            auth.sequence = 1;
            app.settings.cloudflare_completed(
                ticket,
                cloudflare::Outcome::Snapshot(dsh_native_core::CloudflareSnapshot {
                    account_id: "0123456789abcdef0123456789abcdef".into(),
                    models_enabled: false,
                    browser_enabled: false,
                    free_plan_confirmed: false,
                    token_configured: false,
                    writable: true,
                    revision: 4,
                    auth_status: auth.clone(),
                    oauth_setup: dsh_native_core::CloudflareOauthSetup {
                        client_id: "PUBLIC-client-not-real".into(),
                        scopes: vec!["PUBLIC:registered-scope".into()],
                        callback_port: 8977,
                    },
                }),
            );
            auth.browser_available = true;
            app.settings.cloudflare_observed(auth);
            drop(app.settings.cloudflare_continue(true)); // PUBLIC effect consumed; no opener.
        }
        return;
    }
    {
        let action = cloudflare::Action::Models(app.settings.cloudflare_ticket(), true);
        assert!(
            app.settings
                .handle(
                    settings::Action::Cloudflare(action),
                    app.transport_epoch,
                    true
                )
                .is_none()
        );
    }
    let ticket = app.settings.cloudflare_ticket();
    assert!(
        app.settings
            .handle(
                settings::Action::Cloudflare(cloudflare::Action::Free(ticket, true)),
                app.transport_epoch,
                true
            )
            .is_none()
    );
    let ticket = app.settings.cloudflare_ticket();
    assert!(
        app.settings
            .handle(
                settings::Action::Cloudflare(cloudflare::Action::Review(ticket)),
                app.transport_epoch,
                true
            )
            .is_none()
    );
    if preset != "settings-cloudflare-review" {
        let effect = app
            .settings
            .handle(
                settings::Action::Cloudflare(cloudflare::Action::Confirm(
                    app.settings.cloudflare_ticket(),
                )),
                app.transport_epoch,
                true,
            )
            .expect("PUBLIC mutation, not dispatched");
        let settings::Effect::Cloudflare(effect) = effect else {
            panic!("PUBLIC mutation")
        };
        let ticket = effect.ticket();
        drop(effect);
        if preset != "settings-general-cloudflare-pending" {
            app.settings
                .cloudflare_completed(ticket, cloudflare::Outcome::Unknown);
        }
    }
}
