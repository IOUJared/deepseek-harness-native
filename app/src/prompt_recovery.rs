//! One frozen submission outlives a lost ACK. Observations never authorize replay.
use dsh_native_transport::{
    dto::{SessionId, SessionPromptRequest, SessionRequestId},
    prompt_admission::{AdmissionRejection, PromptAdmission},
};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Ticket {
    pub epoch: u64,
    pub generation: u64,
    pub session: SessionId,
    pub request_id: SessionRequestId,
}
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct CheckTicket {
    pub submission: Ticket,
    pub serial: u64,
}
#[derive(Clone)]
pub(crate) struct Pending {
    pub session: SessionId,
    pub text: String,
    pub draft_serial: u64,
    pub file_request_id: Option<SessionRequestId>,
    pub epoch: u64,
    pub file: Option<super::attachments::Metadata>,
    pub file_ticket: Option<super::attachments::Ticket>,
    pub request: SessionPromptRequest,
}
impl Pending {
    pub fn ticket(&self, generation: u64) -> Ticket {
        Ticket {
            epoch: self.epoch,
            generation,
            session: self.session.clone(),
            request_id: self.request.request_id.clone(),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Sending,
    Unknown,
    Checking,
}
#[derive(Default)]
pub(crate) struct Controller {
    pending: Option<(u64, Pending, Phase)>,
    checking: Option<CheckTicket>,
    serial: u64,
    pub expanded: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Receipt {
    Accepted,
    NotSent,
    Unknown,
}
macro_rules! redacted {
    ($($ty:ty),*) => { $(impl std::fmt::Debug for $ty { fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result { f.write_str("PromptRecovery([REDACTED])") } })* };
}
redacted!(Ticket, CheckTicket, Pending, Controller);
impl Controller {
    pub fn is_empty(&self) -> bool {
        self.pending.is_none()
    }
    pub fn contains_key(&self, generation: &u64) -> bool {
        self.get(generation).is_some()
    }
    pub fn values(&self) -> impl Iterator<Item = &Pending> {
        self.pending.iter().map(|(_, p, _)| p)
    }
    pub fn get(&self, generation: &u64) -> Option<&Pending> {
        self.pending
            .as_ref()
            .filter(|(g, _, _)| g == generation)
            .map(|(_, p, _)| p)
    }
    pub fn ticket(&self) -> Option<Ticket> {
        self.pending.as_ref().map(|(g, p, _)| p.ticket(*g))
    }
    pub fn insert(&mut self, generation: u64, pending: Pending) -> bool {
        if self.pending.is_some()
            || pending.text.len() > 32 * 1024
            || pending.request.session_id != pending.session
            || pending.epoch == 0
            || !matches!(
                pending.request.mode,
                dsh_native_transport::dto::PromptMode::Queue
            )
            || !(pending.text.is_empty() && pending.request.content.is_empty()
                || matches!(&pending.request.content[..],[dsh_native_transport::dto::PromptContentPart::Text{text}] if text==&pending.text))
            || pending
                .file_request_id
                .as_ref()
                .is_some_and(|id| id != &pending.request.request_id)
            || pending.file_request_id.is_some() != pending.file_ticket.is_some()
            || pending.file_ticket.as_ref().is_some_and(|ticket| {
                ticket.epoch != pending.epoch
                    || ticket.generation != generation
                    || ticket.target != pending.session
            })
            || pending.file.as_ref().is_some_and(|metadata| {
                metadata.name.len() > 4096 || metadata.bytes > 4 * 1024 * 1024
            })
        {
            return false;
        }
        self.pending = Some((generation, pending, Phase::Sending));
        self.expanded = false;
        true
    }
    pub fn remove(&mut self, generation: &u64) -> Option<Pending> {
        if self.get(generation).is_none() {
            return None;
        }
        self.checking = None;
        self.expanded = false;
        self.pending.take().map(|(_, p, _)| p)
    }
    pub fn settle(&mut self, ticket: &Ticket, receipt: Receipt) -> Option<Pending> {
        if self.ticket().as_ref() != Some(ticket) {
            return None;
        }
        match receipt {
            Receipt::Accepted | Receipt::NotSent => self.remove(&ticket.generation),
            Receipt::Unknown => {
                self.pending.as_mut().unwrap().2 = Phase::Unknown;
                None
            }
        }
    }
    pub fn uncertain(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|(_, _, phase)| *phase != Phase::Sending)
    }
    pub fn checking(&self) -> bool {
        self.checking.is_some()
    }
    pub fn disconnect(&mut self) {
        if let Some((_, _, phase)) = &mut self.pending {
            *phase = Phase::Unknown;
        }
        self.checking = None;
    }
    pub fn begin_check(&mut self, ticket: &Ticket, live_epoch: u64) -> Option<CheckTicket> {
        if !self.uncertain()
            || self.checking.is_some()
            || self.ticket().as_ref() != Some(ticket)
            || ticket.epoch != live_epoch
            || live_epoch == 0
        {
            return None;
        }
        self.serial = self.serial.checked_add(1)?;
        let check = CheckTicket {
            submission: ticket.clone(),
            serial: self.serial,
        };
        self.checking = Some(check.clone());
        self.pending.as_mut().unwrap().2 = Phase::Checking;
        Some(check)
    }
    pub fn checked(
        &mut self,
        ticket: &CheckTicket,
        value: Option<PromptAdmission>,
    ) -> Option<(Pending, Receipt)> {
        if self.checking.as_ref() != Some(ticket)
            || self.ticket().as_ref() != Some(&ticket.submission)
        {
            return None;
        }
        self.checking = None;
        self.pending.as_mut().unwrap().2 = Phase::Unknown;
        let receipt = match value {
            Some(PromptAdmission::Accepted { .. }) => Receipt::Accepted,
            // These exact ledger codes are declared pre-dispatch rejection evidence by the BFF.
            Some(PromptAdmission::Rejected(
                AdmissionRejection::BadRequest
                | AdmissionRejection::InvalidTimeZone
                | AdmissionRejection::AttachmentInvalid
                | AdmissionRejection::NotFound
                | AdmissionRejection::AgentBusy
                | AdmissionRejection::PromptClosed,
            )) => Receipt::NotSent,
            // Internal errors may follow writes; no rollback or replay permission is inferred.
            _ => return None,
        };
        self.settle(&ticket.submission, receipt)
            .map(|pending| (pending, receipt))
    }
}
#[cfg(test)]
#[path = "prompt_recovery_tests.rs"]
mod tests;
