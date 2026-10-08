//! PUBLIC in-process Codex settings fixture. Effects are inspected and dropped, never dispatched.
//! No Backend, Node, browser opener, account, model, transport or credential-store access.
use crate::{codex, settings};
use dsh_native_core::{CodexMetadata, CodexPhase};
use serde_json::{Value, json};

const CONNECT: &str = "settings-codex-connect";
const READY: &str = "settings-codex-ready";
const CALLBACK: &str = "settings-codex-callback";
const ENABLE: &str = "settings-codex-enable";
const PUBLIC_CALLBACK: &str = "PUBLIC_FAKE_CALLBACK";

pub fn supports(mode: &str) -> bool {
    matches!(mode, CONNECT | READY | CALLBACK | ENABLE)
}
fn action(settings: &mut settings::Settings, action: codex::Action) -> Option<settings::Effect> {
    settings.handle(settings::Action::Codex(action), 1, true)
}
fn unsigned() -> CodexMetadata {
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
fn signed() -> CodexMetadata {
    CodexMetadata {
        credential_stored: true,
        phase: CodexPhase::Authorized,
        ..unsigned()
    }
}
fn waiting() -> CodexMetadata {
    CodexMetadata {
        phase: CodexPhase::WaitingBrowser,
        attempt: Some(11),
        prompt: Some(17),
        browser_available: true,
        ..unsigned()
    }
}
fn report(passed: bool) -> Value {
    json!({
        "fixturePassed": passed,
        "codexPublicMetadataOnly": true,
        "codexEffectsDroppedWithoutWorker": true,
        "backendStarted": false,
        "nodeStarted": false,
        "browserOpened": false,
        "accountRequestIssued": false,
        "modelRequestIssued": false,
        "rpcIssued": false,
        "credentialReadIssued": false,
        "credentialWriteIssued": false,
        "nativeKeyboardPointerInputQualified": false
    })
}
/// Prepare the real Settings owner for capture without acquiring any external capability.
pub fn setup(settings: &mut settings::Settings, mode: &str) -> Value {
    if !supports(mode) {
        return report(false);
    }
    // Disabled opening suppresses the API-login page's ordinary metadata read.
    let opened_locally = settings.handle(settings::Action::Open, 1, false).is_none();
    let selected_locally = settings
        .handle(settings::Action::SelectPage(settings::Page::Codex), 1, true)
        .is_none();
    if mode == CONNECT {
        let mut flags = report(opened_locally && selected_locally);
        flags["codexStatusPreviouslyChecked"] = json!(false);
        flags["categoryNavigationLocalOnly"] = json!(opened_locally && selected_locally);
        return flags;
    }
    let check = settings.codex_ticket();
    let Some(settings::Effect::Codex(codex::Effect::StatusRead { ticket })) =
        action(settings, codex::Action::CheckStatus(check))
    else {
        return report(false);
    };
    settings.codex_completed(
        ticket,
        codex::Outcome::Metadata(if mode == ENABLE { signed() } else { unsigned() }),
    );
    let mut passed = opened_locally && selected_locally;
    if mode == CALLBACK {
        let start = settings.codex_ticket();
        let Some(settings::Effect::Codex(codex::Effect::StartLogin { ticket })) =
            action(settings, codex::Action::Start(start))
        else {
            return report(false);
        };
        settings.codex_completed(ticket, codex::Outcome::Metadata(waiting()));
        let Some(edit) = settings.codex_draft_ticket() else {
            return report(false);
        };
        passed &= action(
            settings,
            codex::Action::Edit {
                ticket: edit,
                input: codex::Input::new(PUBLIC_CALLBACK.into()),
            },
        )
        .is_none();
        let Some(review) = settings.codex_draft_ticket() else {
            return report(false);
        };
        passed &= action(settings, codex::Action::ReviewCallback(review)).is_none();
    } else if mode == ENABLE {
        let review = settings.codex_ticket();
        passed &= action(settings, codex::Action::ReviewEnable(review)).is_none();
    }
    let mut flags = report(passed);
    flags["codexExplicitStatusEffectCheckedLocally"] = json!(true);
    flags["categoryNavigationLocalOnly"] = json!(opened_locally && selected_locally);
    flags["codexPublicStartEffectCheckedLocally"] = json!(mode == CALLBACK);
    flags["codexPublicCallbackReviewPrepared"] = json!(mode == CALLBACK && passed);
    flags["codexPublicEnableReviewPrepared"] = json!(mode == ENABLE && passed);
    flags
}
/// Exercise confirmations after capture. Even an opaque PUBLIC callback is never sent to a worker.
pub fn exercise(settings: &mut settings::Settings, mode: &str) -> Value {
    if !supports(mode) {
        return report(false);
    }
    if mode == CONNECT {
        let mut flags = report(exercise_connect(settings));
        flags["codexConnectUserActions"] = json!(1);
        flags["codexConnectionEffectsInspectedLocally"] = json!(4);
        flags["codexSimulatedReceiptsUsed"] = json!(true);
        return flags;
    }
    let (checked, replay_blocked, draft_consumed) = match mode {
        READY => {
            let start = settings.codex_ticket();
            let Some(settings::Effect::Codex(codex::Effect::StartLogin { ticket })) =
                action(settings, codex::Action::Start(start))
            else {
                return report(false);
            };
            let replay = action(settings, codex::Action::Start(start)).is_none();
            settings.codex_completed(ticket, codex::Outcome::NotSent);
            (true, replay, settings.codex_draft_ticket().is_none())
        }
        CALLBACK => {
            let Some(review) = settings.codex_draft_ticket() else {
                return report(false);
            };
            let Some(settings::Effect::Codex(codex::Effect::SubmitCallback {
                ticket,
                attempt,
                prompt,
                secret,
            })) = action(settings, codex::Action::ConfirmCallback(review))
            else {
                return report(false);
            };
            let exact = attempt == 11 && prompt == 17;
            let replay = action(settings, codex::Action::ConfirmCallback(review)).is_none();
            let consumed = settings.codex_draft_ticket().is_none();
            drop(secret); // Do not inspect, extract, serialize, log or report the callback contents.
            settings.codex_completed(ticket, codex::Outcome::NotSent);
            (exact, replay, consumed)
        }
        ENABLE => {
            let review = settings.codex_ticket();
            let Some(settings::Effect::Codex(codex::Effect::EnableModels {
                ticket,
                expected_revision,
            })) = action(settings, codex::Action::ConfirmEnable(review))
            else {
                return report(false);
            };
            let replay = action(settings, codex::Action::ConfirmEnable(review)).is_none();
            settings.codex_completed(ticket, codex::Outcome::NotSent);
            (
                expected_revision == 7,
                replay,
                settings.codex_draft_ticket().is_none(),
            )
        }
        _ => return report(false),
    };
    let mut flags = report(checked && replay_blocked && draft_consumed);
    flags["codexExactEffectTargetCheckedLocally"] = json!(checked);
    flags["codexConfirmReplayBlocked"] = json!(replay_blocked);
    flags["codexCallbackDraftConsumedLocally"] = json!(draft_consumed);
    flags["codexNoSuccessReceiptInvented"] = json!(true);
    flags
}

// Explicit PUBLIC receipt simulation only: inspect/drop effects, never acquire a browser or Host.
fn exercise_connect(s: &mut settings::Settings) -> bool {
    let clicked = s.codex_ticket();
    let Some(settings::Effect::Codex(codex::Effect::StatusRead { ticket })) =
        action(s, codex::Action::Connect(clicked))
    else {
        return false;
    };
    if action(s, codex::Action::Connect(clicked)).is_some() {
        return false;
    }
    s.codex_completed(ticket, codex::Outcome::Metadata(unsigned()));
    let Some(codex::Effect::StartLogin { ticket }) = s.codex_continue(true) else {
        return false;
    };
    s.codex_completed(ticket, codex::Outcome::Metadata(waiting()));
    let Some(codex::Effect::OpenBrowser { ticket, attempt }) = s.codex_continue(true) else {
        return false;
    };
    if attempt != 11 || s.codex_continue(true).is_some() {
        return false;
    }
    s.codex_completed(ticket, codex::Outcome::Confirmed);
    let authorized = CodexMetadata {
        credential_stored: true,
        phase: CodexPhase::Authorized,
        prompt: None,
        browser_available: false,
        ..waiting()
    };
    s.codex_observed(authorized);
    let Some(codex::Effect::EnableModels {
        ticket,
        expected_revision,
    }) = s.codex_continue(true)
    else {
        return false;
    };
    if expected_revision != 7 {
        return false;
    }
    s.codex_completed(
        ticket,
        codex::Outcome::Metadata(CodexMetadata {
            route_configured: true,
            settings_revision: Some(8),
            ..authorized
        }),
    );
    s.codex_continue(true).is_none()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_public_modes_prepare_and_exercise_production_settings_locally() {
        for mode in [CONNECT, READY, CALLBACK, ENABLE] {
            let mut settings = settings::Settings::default();
            let setup = setup(&mut settings, mode);
            assert_eq!(setup["fixturePassed"], true, "{mode}");
            let exercise = exercise(&mut settings, mode);
            assert_eq!(exercise["fixturePassed"], true, "{mode}");
            for flag in [
                "backendStarted",
                "nodeStarted",
                "browserOpened",
                "accountRequestIssued",
                "modelRequestIssued",
                "rpcIssued",
                "credentialReadIssued",
                "credentialWriteIssued",
            ] {
                assert_eq!(setup[flag], false);
                assert_eq!(exercise[flag], false);
            }
            assert!(!setup.to_string().contains(PUBLIC_CALLBACK));
            assert!(!exercise.to_string().contains(PUBLIC_CALLBACK));
        }
    }
    #[test]
    fn unsupported_mode_does_not_open_settings_or_issue_effects() {
        let mut settings = settings::Settings::default();
        assert!(!supports("unsupported"));
        assert_eq!(setup(&mut settings, "unsupported")["fixturePassed"], false);
        assert!(!settings.is_open());
        assert_eq!(
            exercise(&mut settings, "unsupported")["fixturePassed"],
            false
        );
    }
    #[test]
    fn confirmation_replay_fails_without_a_new_explicit_setup() {
        for mode in [CALLBACK, ENABLE] {
            let mut settings = settings::Settings::default();
            assert_eq!(setup(&mut settings, mode)["fixturePassed"], true);
            assert_eq!(exercise(&mut settings, mode)["fixturePassed"], true);
            assert_eq!(exercise(&mut settings, mode)["fixturePassed"], false);
        }
    }
}
