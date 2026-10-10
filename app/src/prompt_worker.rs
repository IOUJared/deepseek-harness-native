//! Shared submission ownership and reserved Stop admission, before the normal worker cap.
use super::{Command, Event, MAX_OPERATIONS, TRANSPORT_EPOCH};
use crate::worker::prompt_recovery::{CheckTicket, Ticket};
use dsh_native_transport::dto::{SessionId, SessionPromptRequest};
use dsh_native_transport::prompt_admission::{AdmissionRejection, PromptAdmission};

pub(super) fn ticket(generation: u64, request: &SessionPromptRequest) -> Ticket {
    Ticket {
        epoch: TRANSPORT_EPOCH,
        generation,
        session: request.session_id.clone(),
        request_id: request.request_id.clone(),
    }
}
pub(super) fn priority(command: &Command) -> bool {
    matches!(
        command,
        Command::Cancel { .. } | Command::Codex(crate::codex::Effect::CancelLogin { .. })
    )
}
pub(super) fn normal_capacity_rejects(command: &Command, count: usize) -> bool {
    count >= MAX_OPERATIONS && !priority(command)
}
// Full wire content (including a file receipt) stays worker-private and is never replayed.
struct OwnedSubmission {
    ticket: Ticket,
    request: Option<SessionPromptRequest>,
}
#[derive(Default)]
pub(super) struct Coordinator {
    owner: Option<OwnedSubmission>,
    check: Option<CheckTicket>,
    stop: Option<(u64, SessionId)>,
}
impl Coordinator {
    fn owned_ticket(&self) -> Option<&Ticket> {
        self.owner.as_ref().map(|owner| &owner.ticket)
    }
    pub fn freeze(&mut self, ticket: &Ticket, request: &SessionPromptRequest) -> bool {
        let Some(owner) = self.owner.as_mut() else {
            return false;
        };
        if &owner.ticket != ticket
            || owner.request.is_some()
            || request.session_id != ticket.session
            || request.request_id != ticket.request_id
            || serde_json::to_vec(request).map_or(true, |body| body.len() > 262_144)
        {
            return false;
        }
        owner.request = Some(request.clone());
        true
    }
    pub fn busy(&self) -> bool {
        self.owner.is_some()
    }
    /// Duplicate input must not publish NotSent for an already dispatched original intent.
    pub fn duplicate(&self, command: &Command) -> bool {
        let submission = match command {
            Command::Prompt {
                generation,
                request,
            } => Some(ticket(*generation, request)),
            Command::PromptFile {
                ticket: attachment,
                request,
            } => Some(ticket(attachment.generation, request)),
            _ => None,
        };
        submission
            .as_ref()
            .is_some_and(|ticket| self.owned_ticket() == Some(ticket))
    }
    pub fn admit(
        &mut self,
        ticket: Ticket,
        generation: u64,
        target: Option<&SessionId>,
        enabled: bool,
    ) -> bool {
        if !enabled
            || self.owner.is_some()
            || ticket.epoch != TRANSPORT_EPOCH
            || ticket.generation != generation
            || Some(&ticket.session) != target
        {
            return false;
        }
        self.owner = Some(OwnedSubmission {
            ticket,
            request: None,
        });
        true
    }
    pub fn check(&mut self, ticket: &CheckTicket) -> bool {
        if self.owned_ticket() != Some(&ticket.submission)
            || self.check.is_some()
            || ticket.submission.epoch != TRANSPORT_EPOCH
        {
            return false;
        }
        self.check = Some(ticket.clone());
        true
    }
    pub fn settle(&mut self, event: &Event) {
        match event {
            Event::Prompt {
                generation,
                request_id,
                result,
            } if self
                .owned_ticket()
                .is_some_and(|t| t.generation == *generation && &t.request_id == request_id) =>
            {
                if result.as_ref().is_ok_and(|value| value.accepted) {
                    self.owner = None;
                    self.check = None;
                }
            }
            Event::PromptNotSent { ticket } if self.owned_ticket() == Some(ticket) => {
                self.owner = None;
                self.check = None;
            }
            Event::FilePrompt {
                ticket,
                request_id,
                outcome,
            } if self.owned_ticket().is_some_and(|t| {
                t.generation == ticket.generation
                    && t.session == ticket.target
                    && &t.request_id == request_id
            }) =>
            {
                if matches!(
                    outcome,
                    super::attachments::PromptOutcome::Accepted
                        | super::attachments::PromptOutcome::NotSent
                ) {
                    self.owner = None;
                    self.check = None;
                }
            }
            Event::SubmissionChecked { ticket, value } if self.check.as_ref() == Some(ticket) => {
                self.check = None;
                if matches!(
                    value,
                    Some(PromptAdmission::Accepted { .. })
                        | Some(PromptAdmission::Rejected(
                            AdmissionRejection::BadRequest
                                | AdmissionRejection::InvalidTimeZone
                                | AdmissionRejection::AttachmentInvalid
                                | AdmissionRejection::NotFound
                                | AdmissionRejection::AgentBusy
                                | AdmissionRejection::PromptClosed
                        ))
                ) {
                    self.owner = None;
                }
            }
            Event::Cancel { generation, .. }
                if self.stop.as_ref().is_some_and(|(g, _)| g == generation) =>
            {
                self.stop = None
            }
            _ => {}
        }
    }
    pub fn stop(
        &mut self,
        generation: u64,
        session: &SessionId,
        current: u64,
        target: Option<&SessionId>,
        enabled: bool,
    ) -> bool {
        if !enabled
            || self.stop.is_some()
            || !(generation == current && Some(session) == target
                || self.owned_ticket().is_some_and(|owner| {
                    owner.generation == generation && &owner.session == session
                }))
        {
            return false;
        }
        self.stop = Some((generation, session.clone()));
        true
    }
}
#[cfg(test)]
#[path = "prompt_worker_tests.rs"]
mod tests;
