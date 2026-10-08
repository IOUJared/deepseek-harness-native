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
        Effect::StatusRead { ticket }
        | Effect::StartLogin { ticket }
        | Effect::OpenBrowser { ticket, .. }
        | Effect::SubmitCallback { ticket, .. }
        | Effect::CancelLogin { ticket, .. }
        | Effect::EnableModels { ticket, .. } => *ticket,
    }
}
#[derive(Default)]
pub(super) struct CodexCoordinator {
    flight: Option<Ticket>,
    metadata: Option<CodexMetadata>,
    uncertain: bool,
    may_write: bool,
}
impl CodexCoordinator {
    pub(super) fn busy(&self) -> bool {
        self.flight.is_some()
            || self.uncertain
            || self
                .metadata
                .is_some_and(|m| m.retry_blocked || m.phase == CodexPhase::WaitingBrowser)
    }
    pub(super) fn admit(
        &mut self,
        effect: &Effect,
        smoke: bool,
        stopping: bool,
        operations: usize,
        conflicting_write: bool,
    ) -> bool {
        if smoke
            || stopping
            || conflicting_write
            || operations >= super::MAX_OPERATIONS
            || codex_ticket(effect).epoch != TRANSPORT_EPOCH
            || self.flight.is_some()
        {
            return false;
        }
        if matches!(
            effect,
            Effect::StartLogin { .. } | Effect::EnableModels { .. }
        ) && self.busy()
        {
            return false;
        }
        self.may_write = matches!(
            effect,
            Effect::StartLogin { .. } | Effect::SubmitCallback { .. } | Effect::CancelLogin { .. }
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
        if self.flight != Some(ticket) {
            return;
        }
        self.flight = None;
        match outcome {
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
pub(super) fn execute_codex(backend: &Backend, effect: Effect) -> Outcome {
    let timeout = Duration::from_secs(10);
    let command = match effect {
        Effect::StatusRead { .. } => CoreCommand::CodexStatus,
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
    match backend.request(command, timeout) {
        Ok(PublicReply::Codex(metadata)) => Outcome::Metadata(metadata),
        _ => Outcome::Indeterminate,
    }
}
