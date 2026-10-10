//! Owned PUBLIC stored-record fixture; all observations come from real App worker receipts.
use super::*;
use dsh_native_core::{
    CodexAccountId, CodexAccountMode, CodexAccountsMetadata, CodexLegacyGrant, CodexMetadata,
    CodexPhase,
};
const A: &str = "account-11111111-1111-4111-8111-111111111111";
const B: &str = "account-22222222-2222-4222-8222-222222222222";
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Step {
    #[default]
    Initial,
    CancelSelectA,
    ReadAfterCancelledSelect,
    SelectA,
    ReadA,
    ReadAfterRejectedSelect,
    SelectB,
    ReadB,
    SignOutB,
    ReadOne,
    SignOutAll,
    ReadEmpty,
    Maintenance,
}
#[derive(Default)]
pub(super) struct Case {
    step: Step,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ReviewCapture {
    CancelA,
    SelectA,
    SelectB,
    SignOutB,
    All,
}
impl ReviewCapture {
    fn for_step(step: Step) -> Option<Self> {
        match step {
            Step::CancelSelectA => Some(Self::CancelA),
            Step::SelectA => Some(Self::SelectA),
            Step::SelectB => Some(Self::SelectB),
            Step::SignOutB => Some(Self::SignOutB),
            Step::SignOutAll => Some(Self::All),
            _ => None,
        }
    }
    fn filename(self) -> &'static str {
        match self {
            Self::CancelA => "own-review-cancel-select.png",
            Self::SelectA => "own-review-select-first.png",
            Self::SelectB => "own-review-select-second.png",
            Self::SignOutB => "own-review-local-logout-second.png",
            Self::All => "own-review-all.png",
        }
    }
}
fn current_capture(f: &Fixture, ticket: codex::Ticket, purpose: ReviewCapture) -> bool {
    f.options.scope == Scope::Accounts
        && f.stage == Stage::Consent(ticket, purpose)
        && ui::usage_ticket(&f.app) == ticket
        && ReviewCapture::for_step(f.accounts_case.step) == Some(purpose)
}
pub(super) fn capture(
    f: &mut Fixture,
    id: window::Id,
    ticket: codex::Ticket,
    purpose: ReviewCapture,
) -> Task<Message> {
    if !current_capture(f, ticket, purpose) || f.window != Some(id) {
        return f.stop(Some("stale-consent-capture-owner"));
    }
    window::screenshot(id).map(move |image| Message::ConsentScreenshot {
        ticket,
        purpose,
        image,
    })
}
pub(super) fn screenshot(
    f: &mut Fixture,
    ticket: codex::Ticket,
    purpose: ReviewCapture,
    image: window::Screenshot,
) -> Task<Message> {
    if !current_capture(f, ticket, purpose) {
        return f.stop(Some("stale-consent-screenshot-owner"));
    }
    let path = f.options.output.join(purpose.filename());
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || smoke::save_screenshot(&path, &image).is_ok())
                .await
                .unwrap_or(false)
        },
        move |saved| Message::ConsentSaved {
            ticket,
            purpose,
            saved,
        },
    )
}
pub(super) fn saved(
    f: &mut Fixture,
    ticket: codex::Ticket,
    purpose: ReviewCapture,
    saved: bool,
) -> Task<Message> {
    if !current_capture(f, ticket, purpose) {
        return f.stop(Some("stale-consent-saved-owner"));
    }
    if !saved {
        return f.stop(Some("consent-capture-save-failed-before-confirm"));
    }
    f.report["accountConsentCaptures"]
        .as_array_mut()
        .expect("fixture consent list")
        .push(json!(purpose.filename()));
    if purpose == ReviewCapture::CancelA {
        let dismiss = f.account(codex::Action::DismissAccount(ticket));
        if ui::usage_ticket(&f.app) != ticket {
            return f.stop(Some("review-dismiss-admitted-command"));
        }
        let confirm = f.account(codex::Action::ConfirmAccount(ticket));
        if ui::usage_ticket(&f.app) != ticket {
            return f.stop(Some("dismissed-review-confirmation-admitted"));
        }
        f.report["localReviewCancelAndNoReviewConfirmNotAdmitted"] = json!(true);
        let read = refresh(f, Step::ReadAfterCancelledSelect);
        return Task::batch([dismiss, confirm, read]);
    }
    admit(
        f,
        f.accounts_case.step,
        codex::Action::ConfirmAccount(ticket),
    )
}
fn id(value: &str) -> CodexAccountId {
    CodexAccountId::new(value.into()).expect("fixed PUBLIC UUIDv4")
}
pub(super) fn seed(options: &Options) -> Result<(), &'static str> {
    // Same new-file PUBLIC vault precedent as scripts/qualify-managed-codex.mjs; no OAuth or importer.
    let directory = options.app.backend.native_home.join("codex-accounts");
    fs::create_dir(&directory).map_err(|_| "new-public-vault-required")?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .map_err(|_| "public-vault-permission-failed")?;
    let accounts = json!([{"opaqueId":A,"label":"PUBLIC first","email":null,"plan":null},{"opaqueId":B,"label":"PUBLIC second","email":null,"plan":null}]);
    let records= [A,B].into_iter().enumerate().map(|(n,key)|json!({"key":format!("llm-pi-ai-codex-accounts/{key}"),
        "record":{"kind":"grant","payload":{"type":"oauth","access":format!("PUBLIC-access-{n}"),"refresh":format!("PUBLIC-refresh-{n}"),
        "expires":4_102_444_800_000_u64,"accountId":format!("PUBLIC-issuer-{n}"),"extra":{"untouched":["PUBLIC-extra",null]}}}})).collect::<Vec<_>>();
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.join("accounts.v1.json"))
        .map_err(|_| "new-public-account-document-required")?;
    serde_json::to_writer(&file,&json!({"version":1,"manifest":{"accounts":accounts,"selected":null,"epoch":0},"records":records})).map_err(|_|"public-account-seed-failed")?;
    file.sync_all()
        .map_err(|_| "public-account-seed-sync-failed")?;
    fs::File::open(directory)
        .and_then(|f| f.sync_all())
        .map_err(|_| "public-vault-directory-sync-failed")?;
    Ok(())
}
fn matches(
    value: &CodexAccountsMetadata,
    epoch: u64,
    ids: &[&str],
    selected: Option<&str>,
) -> bool {
    value.mode == CodexAccountMode::Managed
        && value.epoch == Some(epoch)
        && value.legacy == CodexLegacyGrant::Absent
        && value.added.is_none()
        && value.selected.as_ref().map(CodexAccountId::as_str) == selected
        && value.accounts.len() == ids.len()
        && value.accounts.iter().zip(ids).all(|(a, key)| {
            a.opaque_id.as_str() == *key
                && a.label.as_str()
                    == if *key == A {
                        "PUBLIC first"
                    } else {
                        "PUBLIC second"
                    }
                && a.email.is_none()
                && a.plan.is_none()
        })
}
fn local(metadata: CodexMetadata, selected: bool) -> bool {
    metadata.available
        && metadata.credential_stored == selected
        && !metadata.route_configured
        && metadata.phase == CodexPhase::Idle
        && metadata.attempt.is_none()
        && metadata.prompt.is_none()
        && !metadata.browser_available
        && !metadata.retry_blocked
        && metadata.settings_revision.is_some()
}
fn projection(value: &CodexAccountsMetadata) -> Value {
    json!({"mode":"managed","epoch":value.epoch,"selected":value.selected.as_ref().map(CodexAccountId::as_str),
    "accounts":value.accounts.iter().map(|a|json!({"opaqueId":a.opaque_id.as_str(),"label":a.label.as_str(),"email":null,"plan":null})).collect::<Vec<_>>(),"legacy":"absent","added":null})
}
fn final_state(phase: Phase) -> (u64, &'static [&'static str], Option<&'static str>) {
    if phase == Phase::Initial {
        (2, &[A, B], Some(B))
    } else {
        (4, &[], None)
    }
}
fn admit(f: &mut Fixture, step: Step, action: codex::Action) -> Task<Message> {
    let before = ui::usage_ticket(&f.app);
    let task = f.account(action);
    let after = ui::usage_ticket(&f.app);
    if before == after {
        return f.stop(Some("account-command-not-admitted"));
    }
    f.accounts_case.step = step;
    f.stage = Stage::Read(after);
    let n = f.report["accountOrAuthorizationCommands"]
        .as_u64()
        .unwrap_or(0);
    f.report["accountOrAuthorizationCommands"] = json!(n + 1);
    task
}
fn refresh(f: &mut Fixture, step: Step) -> Task<Message> {
    admit(
        f,
        step,
        codex::Action::RefreshAccounts(ui::usage_ticket(&f.app)),
    )
}
fn mutation(f: &mut Fixture, step: Step, key: Option<&str>, signout: bool) -> Task<Message> {
    let ticket = ui::usage_ticket(&f.app);
    let review = match (key, signout) {
        (Some(key), false) => codex::Action::ReviewSelect {
            ticket,
            account_id: id(key),
        },
        (Some(key), true) => codex::Action::ReviewSignOut {
            ticket,
            account_id: id(key),
        },
        _ => codex::Action::ReviewSignOutAll(ticket),
    };
    let task = f.account(review);
    if ui::usage_ticket(&f.app) != ticket {
        return f.stop(Some("review-started-account-mutation"));
    }
    let Some(purpose) = ReviewCapture::for_step(step) else {
        return f.stop(Some("invalid-consent-step"));
    };
    let Some(window) = f.window else {
        return f.stop(Some("consent-window-unavailable"));
    };
    f.accounts_case.step = step;
    f.stage = Stage::Consent(ticket, purpose);
    Task::batch([
        task,
        iced::widget::operation::snap_to(
            "native-settings-body",
            iced::widget::scrollable::RelativeOffset::START,
        ),
        Task::perform(
            async move {
                tokio::time::sleep(Duration::from_millis(400)).await;
                window
            },
            move |id| Message::ConsentCapture {
                id,
                ticket,
                purpose,
            },
        ),
    ])
}
pub(super) fn maintenance(f: &mut Fixture) -> Task<Message> {
    let tab = f.account(codex::Action::SelectTab {
        ticket: ui::usage_ticket(&f.app),
        tab: codex::Tab::Maintenance,
    });
    let read = admit(
        f,
        Step::Maintenance,
        codex::Action::CheckStatus(ui::usage_ticket(&f.app)),
    );
    Task::batch([tab, read])
}
fn guard(f: &mut Fixture, empty: bool) -> Task<Message> {
    let ticket = ui::usage_ticket(&f.app);
    let mut tasks = Vec::new();
    // Confirm without review, ids absent from roster, invalid Add label cannot admit effects.
    for action in [
        codex::Action::ConfirmAccount(ticket),
        codex::Action::ReviewSelect {
            ticket,
            account_id: id("account-33333333-3333-4333-8333-333333333333"),
        },
        codex::Action::ReviewSignOut {
            ticket,
            account_id: id("account-33333333-3333-4333-8333-333333333333"),
        },
        codex::Action::ConfirmAccount(ticket),
        codex::Action::EditLabel {
            ticket,
            label: "\n".into(),
        },
        codex::Action::AddAccount(ticket),
    ] {
        tasks.push(f.account(action));
        if ui::usage_ticket(&f.app) != ticket {
            return f.stop(Some("invalid-account-action-admitted"));
        }
    }
    if empty {
        tasks.push(f.account(codex::Action::ReviewSignOutAll(ticket)));
        tasks.push(f.account(codex::Action::ConfirmAccount(ticket)));
        if ui::usage_ticket(&f.app) != ticket {
            return f.stop(Some("empty-signout-all-admitted"));
        }
        f.report["emptySignoutAllRejectedByActualApp"] = json!(true);
    }
    f.report["invalidAccountActionsRejectedByActualApp"] = json!(true);
    Task::batch(tasks)
}
pub(super) fn completed(
    f: &mut Fixture,
    ticket: codex::Ticket,
    outcome: codex::Outcome,
    task: Task<Message>,
) -> Task<Message> {
    if f.stage != Stage::Read(ticket) {
        return f.stop(Some("unexpected-account-receipt-owner"));
    }
    let step = f.accounts_case.step;
    let (epoch, ids, selected) = match step {
        Step::CancelSelectA => return f.stop(Some("unexpected-write-during-cancel-consent")),
        Step::ReadAfterCancelledSelect => (0, &[A, B][..], None),
        Step::Initial => match f.options.phase {
            Phase::Initial => (0, &[A, B][..], None),
            Phase::Restored => (2, &[A, B][..], Some(B)),
            Phase::Inherited => (4, &[][..], None),
        },
        Step::SelectA | Step::ReadA | Step::ReadAfterRejectedSelect => (1, &[A, B][..], Some(A)),
        Step::SelectB | Step::ReadB => (2, &[A, B][..], Some(B)),
        Step::SignOutB | Step::ReadOne => (3, &[A][..], None),
        Step::SignOutAll | Step::ReadEmpty => (4, &[][..], None),
        Step::Maintenance => final_state(f.options.phase),
    };
    let value = match outcome {
        codex::Outcome::StatusSnapshot { metadata, accounts }
            if matches!(
                step,
                Step::Initial
                    | Step::ReadAfterCancelledSelect
                    | Step::ReadA
                    | Step::ReadAfterRejectedSelect
                    | Step::ReadB
                    | Step::ReadOne
                    | Step::ReadEmpty
                    | Step::Maintenance
            ) =>
        {
            if !local(metadata, selected.is_some()) {
                return f.stop(Some("account-local-status-mismatch"));
            }
            f.report["actualLocalStatusMatched"] = json!(true);
            accounts
        }
        codex::Outcome::Accounts(value)
            if matches!(
                step,
                Step::SelectA | Step::SelectB | Step::SignOutB | Step::SignOutAll
            ) =>
        {
            value
        }
        _ => return f.stop(Some("account-outcome-unavailable-or-wrong-kind")),
    };
    if !matches(&value, epoch, ids, selected) {
        return f.stop(Some("account-roster-selection-epoch-mismatch"));
    }
    if matches!(
        step,
        Step::SelectA | Step::SelectB | Step::SignOutB | Step::SignOutAll
    ) {
        f.report["actualPersistenceAcknowledged"] = json!(true);
        f.report["localAccountMutationAcks"]
            .as_array_mut()
            .expect("fixture ack list")
            .push(projection(&value));
    }
    let next = match step {
        Step::Initial => {
            f.report["initialOwnedSnapshotMatched"] = json!(true);
            f.report["accountLoaded"] = projection(&value);
            f.report["publicStoredFixturesOnly"] = json!(true);
            f.report["oauthOrBrowserOrEnableModelsAdmitted"] = json!(false);
            f.report["localAccountMutationAcks"] = json!([]);
            f.report["accountConsentCaptures"] = json!([]);
            f.report["accountOrAuthorizationCommands"] = json!(1);
            let guards = guard(f, ids.is_empty());
            let next = match f.options.phase {
                Phase::Initial => mutation(f, Step::CancelSelectA, Some(A), false),
                Phase::Restored => mutation(f, Step::SignOutB, Some(B), true),
                Phase::Inherited => {
                    f.report["independentEmptyRestartMatched"] = json!(true);
                    f.report["accountFinal"] = projection(&value);
                    f.paint()
                }
            };
            Task::batch([guards, next])
        }
        Step::CancelSelectA => return f.stop(Some("unexpected-cancel-consent-completion")),
        Step::ReadAfterCancelledSelect => {
            f.report["localReviewCancelEpochAndRosterReadbackMatched"] = json!(true);
            f.report["localReviewCancelReadback"] = projection(&value);
            mutation(f, Step::SelectA, Some(A), false)
        }
        Step::SelectA => refresh(f, Step::ReadA),
        Step::ReadA => {
            let ticket = ui::usage_ticket(&f.app);
            let review = f.account(codex::Action::ReviewSelect {
                ticket,
                account_id: id(A),
            });
            if ui::usage_ticket(&f.app) != ticket {
                return f.stop(Some("selected-switch-review-admitted"));
            }
            let confirm = f.account(codex::Action::ConfirmAccount(ticket));
            if ui::usage_ticket(&f.app) != ticket {
                return f.stop(Some("selected-switch-confirmation-admitted"));
            }
            f.report["selectedSwitchActionsRejectedByActualApp"] = json!(true);
            f.report["selectedSwitchRejection"] =
                json!({"epoch":epoch,"selected":A,"ticketUnchangedAfterEachAction":true});
            let read = refresh(f, Step::ReadAfterRejectedSelect);
            Task::batch([review, confirm, read])
        }
        Step::ReadAfterRejectedSelect => {
            f.report["selectedSwitchEpochAndSelectionUnchangedAfterExplicitReadback"] = json!(true);
            f.report["selectedSwitchReadback"] = projection(&value);
            mutation(f, Step::SelectB, Some(B), false)
        }
        Step::SelectB => refresh(f, Step::ReadB),
        Step::SignOutB => refresh(f, Step::ReadOne),
        Step::ReadOne => {
            f.report["selectedLogoutHasNoReplacement"] = json!(true);
            mutation(f, Step::SignOutAll, None, true)
        }
        Step::SignOutAll => refresh(f, Step::ReadEmpty),
        Step::ReadB | Step::ReadEmpty => {
            f.report["sameProcessReadbackMatched"] = json!(true);
            f.report["accountFinal"] = projection(&value);
            if step == Step::ReadEmpty {
                let guards = guard(f, true);
                Task::batch([guards, f.paint()])
            } else {
                f.paint()
            }
        }
        Step::Maintenance => {
            f.report["maintenanceStatusMatched"] = json!(true);
            f.report["maintenanceAccountSnapshot"] = projection(&value);
            let ticket = ui::usage_ticket(&f.app);
            let rejected = f.account(codex::Action::ReviewSignOutAll(ticket));
            let confirm = f.account(codex::Action::ConfirmAccount(ticket));
            if ui::usage_ticket(&f.app) != ticket {
                return f.stop(Some("account-mutation-admitted-from-maintenance"));
            }
            f.report["maintenanceAccountActionsRejected"] = json!(true);
            Task::batch([rejected, confirm, f.paint()])
        }
    };
    Task::batch([task, next])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_roster_oracle_distinguishes_persisted_epoch_and_nullable_selection() {
        let mut value = CodexAccountsMetadata {
            mode: CodexAccountMode::Managed,
            accounts: vec![],
            selected: None,
            epoch: Some(4),
            legacy: CodexLegacyGrant::Absent,
            added: None,
        };
        assert!(matches(&value, 4, &[], None));
        assert!(!matches(&value, 0, &[], None));
        value.selected = Some(id(A));
        assert!(!matches(&value, 4, &[], None));
    }
    #[test]
    fn consent_save_failure_or_stale_owner_cannot_confirm_a_mutation() {
        for kind in 0..5 {
            let (handle, feed, mut commands, _events, closing) = worker::test_channels();
            let mut options = super::super::fixture_tests::unstarted_options();
            options.scope = Scope::Accounts;
            let (mut f, _boot) = Fixture::boot(options, handle, feed);
            let ticket = ui::usage_ticket(&f.app);
            f.accounts_case.step = Step::SelectA;
            f.stage = Stage::Consent(ticket, ReviewCapture::SelectA);
            f.report["accountConsentCaptures"] = json!([]);
            let purpose = if kind == 1 {
                ReviewCapture::SelectB
            } else {
                ReviewCapture::SelectA
            };
            let callback_ticket = if kind == 2 {
                codex::Ticket {
                    serial: ticket.serial + 1,
                    ..ticket
                }
            } else {
                ticket
            };
            if kind == 3 {
                f.stage = Stage::Stopping;
            }
            if kind == 4 {
                f.options.scope = Scope::Usage;
            }
            let _ = saved(&mut f, callback_ticket, purpose, kind != 0);
            assert!(f.failed);
            assert!(f.stage == Stage::Stopping);
            assert!(
                f.report["accountConsentCaptures"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            // Failing closed invokes actual App shutdown, retiring Settings ownership.
            assert!(ui::usage_ticket(&f.app) != ticket);
            assert!(*closing.borrow());
            assert!(commands.try_recv().is_err());
        }
    }
    #[test]
    fn consent_outputs_are_closed_to_only_explicit_review_steps() {
        let purposes = [
            Step::CancelSelectA,
            Step::SelectA,
            Step::SelectB,
            Step::SignOutB,
            Step::SignOutAll,
        ]
        .map(|step| ReviewCapture::for_step(step).unwrap());
        let names = purposes.map(ReviewCapture::filename);
        assert_eq!(
            names
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            5
        );
        for step in [
            Step::Initial,
            Step::ReadAfterCancelledSelect,
            Step::ReadA,
            Step::ReadAfterRejectedSelect,
            Step::ReadB,
            Step::ReadOne,
            Step::ReadEmpty,
            Step::Maintenance,
        ] {
            assert!(ReviewCapture::for_step(step).is_none());
        }
    }
    #[test]
    fn local_status_does_not_equate_stored_selection_to_route_or_entitlement() {
        let value = CodexMetadata {
            available: true,
            credential_stored: false,
            route_configured: false,
            phase: CodexPhase::Idle,
            attempt: None,
            prompt: None,
            browser_available: false,
            settings_revision: Some(17),
            retry_blocked: false,
        };
        assert!(local(value, false));
        assert!(!local(value, true));
        assert!(!local(
            CodexMetadata {
                route_configured: true,
                ..value
            },
            false
        ));
    }
}
