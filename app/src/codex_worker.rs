//! Worker admission and fixed-result projection for the private Codex bridge.
use super::{Backend, CoreCommand, PublicReply, TRANSPORT_EPOCH};
use crate::codex::{Effect, Outcome, Ticket};
use dsh_native_core::{CodexMetadata, CodexPhase};
use std::time::Duration;
#[cfg(test)]
#[path = "codex_worker_tests.rs"]
mod tests;

pub(super) fn codex_ticket(effect: &Effect) -> Ticket {
    match effect {
        Effect::RuntimeRead { ticket }
        | Effect::RuntimeSave { ticket, .. }
        | Effect::UsageRead { ticket }
        | Effect::UsageSave { ticket, .. }
        | Effect::AccountsRead { ticket }
        | Effect::AddAccount { ticket, .. }
        | Effect::SelectAccount { ticket, .. }
        | Effect::SignOutAccount { ticket, .. }
        | Effect::SignOutAll { ticket, .. }
        | Effect::StatusRead { ticket }
        | Effect::StartLogin { ticket }
        | Effect::OpenBrowser { ticket, .. }
        | Effect::SubmitCallback { ticket, .. }
        | Effect::CancelLogin { ticket, .. }
        | Effect::EnableModels { ticket, .. } => *ticket,
    }
}
enum RuntimeOperation {
    Read,
    Save { revision: u64, values: dsh_native_core::CodexRuntimeValues,
        base: dsh_native_core::CodexRuntimeValues },
}
#[derive(Default)]
pub(super) struct CodexCoordinator {
    flight: Option<Ticket>,
    cancel: Option<Ticket>,
    reconciling: bool,
    metadata: Option<CodexMetadata>,
    uncertain: bool,
    may_write: bool,
    usage_operation: Option<bool>, // true = save; never shares OAuth uncertainty.
    usage_uncertain: bool,
    runtime_operation: Option<RuntimeOperation>,
    runtime_uncertain: bool, // Never reconciled by OAuth/status or usage receipts.
}
impl CodexCoordinator {
    pub(super) fn busy(&self) -> bool {
        self.flight.is_some()
            || self.cancel.is_some()
            || self.uncertain
            || self.metadata.is_some_and(|m| {
                m.retry_blocked
                    || matches!(
                        m.phase,
                        CodexPhase::WaitingBrowser | CodexPhase::Indeterminate
                    )
            })
    }
    pub(super) fn admit(
        &mut self,
        effect: &Effect,
        smoke: bool,
        stopping: bool,
        operations: usize,
        conflicting_write: bool,
    ) -> bool {
        if matches!(effect, Effect::CancelLogin { .. }) {
            // Reserved cancellation slot is not queued behind a login, saves, or the normal task cap.
            if smoke
                || stopping
                || self.cancel.is_some()
                || codex_ticket(effect).epoch != TRANSPORT_EPOCH
            {
                return false;
            }
            self.cancel = Some(codex_ticket(effect));
            return true;
        }
        if smoke
            || stopping
            || conflicting_write
            || operations >= super::MAX_OPERATIONS
            || codex_ticket(effect).epoch != TRANSPORT_EPOCH
            || self.flight.is_some()
            || self.cancel.is_some()
        {
            return false;
        }
        if matches!(
            effect,
            Effect::StartLogin { .. }
                | Effect::EnableModels { .. }
                | Effect::AddAccount { .. }
                | Effect::SelectAccount { .. }
                | Effect::SignOutAccount { .. }
                | Effect::SignOutAll { .. }
        ) && self.busy()
        {
            return false;
        }
        if matches!(effect, Effect::UsageSave { .. }) && self.usage_uncertain {
            return false;
        }
        if matches!(effect, Effect::RuntimeSave { .. }) && self.runtime_uncertain {
            return false;
        }
        self.runtime_operation = match effect {
            Effect::RuntimeRead { .. } => Some(RuntimeOperation::Read),
            Effect::RuntimeSave { expected_revision, values, base_values, .. } => Some(RuntimeOperation::Save {
                revision: *expected_revision, values: *values, base: *base_values,
            }),
            _ => None,
        };
        self.usage_operation = match effect {
            Effect::UsageRead { .. } => Some(false),
            Effect::UsageSave { .. } => Some(true),
            _ => None,
        };
        self.may_write = matches!(
            effect,
            Effect::StartLogin { .. }
                | Effect::SubmitCallback { .. }
                | Effect::AddAccount { .. }
                | Effect::SelectAccount { .. }
                | Effect::SignOutAccount { .. }
                | Effect::SignOutAll { .. }
        );
        self.reconciling = matches!(
            effect,
            Effect::AccountsRead { .. } | Effect::StatusRead { .. }
        );
        self.flight = Some(codex_ticket(effect));
        true
    }
    pub(super) fn observe(&mut self, metadata: CodexMetadata) {
        if let Some(previous) = self.metadata {
            // An async command receipt can lag behind the same attempt's terminal event.
            if previous.attempt == metadata.attempt
                && terminal(previous.phase)
                && metadata.phase == CodexPhase::WaitingBrowser
            {
                return;
            }
            if previous
                .attempt
                .zip(metadata.attempt)
                .is_some_and(|(old, new)| new < old)
            {
                return;
            }
        }
        self.metadata = Some(metadata);
    }
    pub(super) fn settle(&mut self, ticket: Ticket, outcome: Outcome) {
        if self.cancel == Some(ticket) {
            self.cancel = None;
            match outcome {
                Outcome::Metadata(m) => self.observe(m),
                Outcome::Indeterminate => self.uncertain = true,
                _ => {}
            }
            return;
        }
        if self.flight != Some(ticket) {
            return;
        }
        self.flight = None;
        if let Some(operation) = self.runtime_operation.take() {
            match (operation, outcome) {
                (RuntimeOperation::Read, Outcome::Runtime(snapshot)) if snapshot.version == 1 => {
                    self.runtime_uncertain = false;
                }
                (RuntimeOperation::Save { revision, values, base }, Outcome::Runtime(snapshot))
                    if crate::codex::runtime_ack_matches(snapshot, revision, values, base) => {}
                (_, Outcome::NotSent) => {}
                (RuntimeOperation::Save { .. }, _) => self.runtime_uncertain = true,
                _ => {}
            }
            return;
        }
        if let Some(saving) = self.usage_operation.take() {
            match outcome {
                Outcome::UsageSnapshot(snapshot) if snapshot.version == 1 => {
                    if !saving { self.usage_uncertain = false; }
                }
                Outcome::NotSent | Outcome::UsageConflict | Outcome::UsageUnavailable => {}
                _ if saving => self.usage_uncertain = true,
                _ => {}
            }
            return;
        }
        match outcome {
            Outcome::StatusSnapshot { metadata, accounts } => {
                self.observe(metadata);
                if self.reconciling
                    && self.cancel.is_none()
                    && accounts.mode == dsh_native_core::CodexAccountMode::Managed
                    && !metadata.retry_blocked
                    && !matches!(
                        metadata.phase,
                        CodexPhase::WaitingBrowser | CodexPhase::Indeterminate
                    )
                {
                    self.uncertain = false;
                }
            }
            Outcome::Metadata(metadata) => self.observe(metadata),
            Outcome::Indeterminate if self.may_write => self.uncertain = true,
            _ => {}
        }
    }
}
fn terminal(phase: CodexPhase) -> bool {
    matches!(
        phase,
        CodexPhase::Authorized
            | CodexPhase::Cancelled
            | CodexPhase::Failed
            | CodexPhase::Indeterminate
    )
}
fn project_usage(result: Result<PublicReply, dsh_native_core::Error>) -> Outcome {
    match result {
        Ok(PublicReply::CodexUsage(snapshot)) => Outcome::UsageSnapshot(snapshot),
        Err(dsh_native_core::Error::CodexUsageConflict) => Outcome::UsageConflict,
        Err(dsh_native_core::Error::CodexUsageUnavailable) => Outcome::UsageUnavailable,
        Err(dsh_native_core::Error::InvalidCommand) => Outcome::NotSent,
        // RequestTimeout can follow transmission; never downgrade it to NotSent.
        _ => Outcome::UsageIndeterminate,
    }
}
fn project_usage_save(result: Result<PublicReply, dsh_native_core::Error>, expected_revision: u64,
    preferences: dsh_native_core::CodexUsagePreferences) -> Outcome {
    match project_usage(result) {
        Outcome::UsageSnapshot(snapshot) if snapshot.version == 1 && snapshot.persisted
            && expected_revision.checked_add(1) == Some(snapshot.revision)
            && snapshot.preferences == preferences => Outcome::UsageSnapshot(snapshot),
        Outcome::UsageSnapshot(_) => Outcome::UsageIndeterminate,
        other => other,
    }
}
fn project_runtime(result: Result<PublicReply, dsh_native_core::Error>) -> Outcome {
    match result {
        Ok(PublicReply::CodexRuntime(snapshot)) if snapshot.version == 1 => Outcome::Runtime(snapshot),
        Err(dsh_native_core::Error::CodexRuntimeConflict) => Outcome::RuntimeConflict,
        Err(dsh_native_core::Error::CodexRuntimeUnavailable) => Outcome::RuntimeUnavailable,
        Err(dsh_native_core::Error::InvalidCommand) => Outcome::NotSent,
        // Timeout or wrong reply after transmission is unknown, never an unsent write.
        _ => Outcome::RuntimeIndeterminate,
    }
}
fn project_runtime_save(result: Result<PublicReply, dsh_native_core::Error>, revision: u64,
    values: dsh_native_core::CodexRuntimeValues, base: dsh_native_core::CodexRuntimeValues) -> Outcome {
    match project_runtime(result) {
        Outcome::Runtime(snapshot) if crate::codex::runtime_ack_matches(snapshot, revision, values, base) => Outcome::Runtime(snapshot),
        Outcome::Runtime(_) => Outcome::RuntimeIndeterminate,
        other => other,
    }
}
pub(super) fn execute_codex(backend: &Backend, effect: Effect) -> Outcome {
    let timeout = Duration::from_secs(10);
    let command = match effect {
        Effect::RuntimeRead { .. } => {
            return project_runtime(backend.request(CoreCommand::CodexRuntimeRead, timeout));
        }
        Effect::RuntimeSave { expected_revision, values, base_values, .. } => {
            return project_runtime_save(backend.request(CoreCommand::CodexRuntimeSave {
                expected_revision, values,
            }, timeout), expected_revision, values, base_values);
        }
        Effect::UsageRead { .. } => {
            return project_usage(backend.request(CoreCommand::CodexUsageLoad, timeout));
        }
        Effect::UsageSave { expected_revision, preferences, .. } => {
            return project_usage_save(backend.request(CoreCommand::CodexUsageSave {
                expected_revision, preferences,
            }, timeout), expected_revision, preferences);
        }
        Effect::StatusRead { .. } | Effect::AccountsRead { .. } => {
            // Missing metadata after an admitted request is unknown, never NotSent or legacy fallback.
            let Ok(PublicReply::CodexAccounts(accounts)) =
                backend.request(CoreCommand::CodexAccountsStatus, timeout)
            else {
                return Outcome::Indeterminate;
            };
            return match backend.request(CoreCommand::CodexStatus, timeout) {
                Ok(PublicReply::Codex(metadata)) => Outcome::StatusSnapshot { metadata, accounts },
                _ => Outcome::Indeterminate,
            };
        }
        Effect::AddAccount {
            label,
            expected_epoch,
            ..
        } => CoreCommand::CodexAdd {
            label,
            expected_epoch,
        },
        Effect::SelectAccount {
            account_id,
            expected_epoch,
            ..
        } => CoreCommand::CodexSelect {
            account_id,
            expected_epoch,
        },
        Effect::SignOutAccount {
            account_id,
            expected_epoch,
            ..
        } => CoreCommand::CodexSignOut {
            account_id,
            expected_epoch,
        },
        Effect::SignOutAll { expected_epoch, .. } => {
            CoreCommand::CodexSignOutAll { expected_epoch }
        }
        Effect::StartLogin { .. } => CoreCommand::CodexStart,
        Effect::CancelLogin { attempt, .. } => CoreCommand::CodexCancel { attempt },
        Effect::EnableModels {
            expected_revision, ..
        } => CoreCommand::CodexEnableModels { expected_revision },
        Effect::OpenBrowser { attempt, .. } => {
            return match backend.open_codex_browser(attempt) {
                Ok(()) => Outcome::Confirmed,
                Err(_) => Outcome::Indeterminate,
            };
        }
        Effect::SubmitCallback {
            attempt,
            prompt,
            secret,
            ..
        } => {
            return match backend.submit_codex_callback(attempt, prompt, secret, timeout) {
                Ok(true) => Outcome::Confirmed,
                Ok(false) => Outcome::Refused,
                Err(_) => Outcome::Indeterminate,
            };
        }
    };
    let expects_accounts = matches!(
        &command,
        CoreCommand::CodexSelect { .. }
            | CoreCommand::CodexSignOut { .. }
            | CoreCommand::CodexSignOutAll { .. }
    );
    match backend.request(command, timeout) {
        Ok(PublicReply::Codex(metadata)) if !expects_accounts => Outcome::Metadata(metadata),
        Ok(PublicReply::CodexAccounts(metadata)) if expects_accounts => Outcome::Accounts(metadata),
        Err(dsh_native_core::Error::CodexEpochConflict) => Outcome::Conflict,
        _ => Outcome::Indeterminate,
    }
}
