//! PUBLIC account display receipts only; no worker, OAuth, route edits or grant material.
use super::App;
use crate::{codex, settings};
use dsh_native_core::{
    CodexAccount, CodexAccountId, CodexAccountLabel, CodexAccountMode, CodexAccountsMetadata,
    CodexLegacyGrant, CodexMetadata, CodexPhase,
};

pub(super) fn seed(app: &mut App, preset: &str) {
    let ticket = app.settings.codex_ticket();
    let effect = app
        .settings
        .handle(
            settings::Action::Codex(codex::Action::RefreshAccounts(ticket)),
            app.transport_epoch,
            true,
        )
        .expect("PUBLIC local read receipt");
    let settings::Effect::Codex(codex::Effect::AccountsRead { ticket }) = effect else {
        panic!("PUBLIC expected accounts read");
    };
    let first = CodexAccountId::new("account-11111111-1111-4111-8111-111111111111".into()).unwrap();
    let second =
        CodexAccountId::new("account-22222222-2222-4222-8222-222222222222".into()).unwrap();
    let accounts = CodexAccountsMetadata {
        mode: CodexAccountMode::Managed,
        accounts: vec![
            CodexAccount {
                opaque_id: first.clone(),
                label: CodexAccountLabel::new("PUBLIC personal account".into()).unwrap(),
                email: None,
                plan: None,
            },
            CodexAccount {
                opaque_id: second,
                label: CodexAccountLabel::new("PUBLIC team account · 世界".into()).unwrap(),
                email: None,
                plan: None,
            },
        ],
        selected: Some(first.clone()),
        epoch: Some(7),
        legacy: CodexLegacyGrant::Unadopted,
        added: None,
    };
    app.settings.codex_completed(
        ticket,
        codex::Outcome::StatusSnapshot {
            metadata: CodexMetadata {
                available: true,
                credential_stored: true,
                route_configured: true,
                phase: CodexPhase::Idle,
                attempt: None,
                prompt: None,
                browser_available: false,
                settings_revision: Some(3),
                retry_blocked: false,
            },
            accounts,
        },
    );
    if preset != "settings-codex-managed" {
        let ticket = app.settings.codex_ticket();
        assert!(
            app.settings
                .handle(
                    settings::Action::Codex(codex::Action::ReviewSignOut {
                        ticket,
                        account_id: first,
                    }),
                    app.transport_epoch,
                    true,
                )
                .is_none()
        );
        if preset == "settings-codex-unknown" {
            let effect = app
                .settings
                .handle(
                    settings::Action::Codex(codex::Action::ConfirmAccount(ticket)),
                    app.transport_epoch,
                    true,
                )
                .expect("PUBLIC confirmation receipt without dispatch");
            let settings::Effect::Codex(codex::Effect::SignOutAccount { ticket, .. }) = effect
            else {
                panic!("PUBLIC expected local logout receipt");
            };
            app.settings
                .codex_completed(ticket, codex::Outcome::Indeterminate);
        }
    }
}
