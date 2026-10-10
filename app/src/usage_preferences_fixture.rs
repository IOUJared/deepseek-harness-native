//! PUBLIC render-only preference receipt. No storage, observer, notification or retry runs.
use super::App;
use crate::{codex, settings};
use dsh_native_core::{
    CodexComposerDisplay, CodexRemainingPercent, CodexUsageAlert, CodexUsagePreferences,
    CodexUsageSnapshot,
};

/// Synthetic admitted local save only; no worker dispatch, file storage or credential action.
pub(super) fn save(app: &mut App, indeterminate: bool) {
    assert!(app.settings.handle(
        settings::Action::Codex(codex::Action::Usage {
            ticket: app.settings.codex_ticket(), action: codex::UsageAction::Retry(false),
        }), app.transport_epoch, true,
    ).is_none()); // A real opt-out, not a save of inert unchanged choices.
    let effect = app.settings.handle(
        settings::Action::Codex(codex::Action::Usage {
            ticket: app.settings.codex_ticket(), action: codex::UsageAction::Save,
        }), app.transport_epoch, true,
    ).expect("PUBLIC local save, never dispatched");
    let settings::Effect::Codex(codex::Effect::UsageSave { ticket, .. }) = effect else {
        panic!("PUBLIC expected local preference save");
    };
    if indeterminate {
        app.settings.codex_completed(ticket, codex::Outcome::Indeterminate);
    }
}

pub(super) fn seed(app: &mut App) {
    let ticket = app.settings.codex_ticket();
    let effect = app.settings.handle(
        settings::Action::Codex(codex::Action::Usage {
            ticket,
            action: codex::UsageAction::Refresh,
        }),
        app.transport_epoch,
        true,
    ).expect("PUBLIC preference read, never dispatched");
    let settings::Effect::Codex(codex::Effect::UsageRead { ticket }) = effect else {
        panic!("PUBLIC expected preference read");
    };
    app.settings.codex_completed(ticket, codex::Outcome::UsageSnapshot(CodexUsageSnapshot {
        version: 1,
        revision: 4,
        persisted: true,
        preferences: CodexUsagePreferences {
            composer_display: CodexComposerDisplay::ProgressBar,
            usage_alert: CodexUsageAlert::Custom,
            custom_remaining_percent: CodexRemainingPercent::new(17.5).unwrap(),
            retry_after_reset: true,
            experimental_quota: false,
        },
    }));
}
