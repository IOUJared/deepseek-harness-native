//! Single worker-owned staged file; GUI tickets and safe metadata never contain receipts or bytes.
use dsh_native_transport::dto::{
    FileUploadValue, PromptContentPart, PromptMode, SessionId, SessionPromptRequest,
    SessionRequestId,
};
use tokio::sync::watch;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub epoch: u64,
    pub generation: u64,
    pub serial: u64,
    pub target: SessionId,
}
pub struct Submission {
    pub ticket: Ticket,
    pub source: crate::worker::file_intake::SelectedFile,
}
impl std::fmt::Debug for Submission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AttachmentSubmission(redacted)")
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub name: String,
    pub bytes: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    NotSent,
    ReadRejected(crate::worker::file_intake::Failure),
    /// An admitted upload may have stored bytes/staged a receipt; cancellation is not rollback.
    Unknown,
    Staged(Metadata),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptOutcome {
    NotSent,
    Accepted,
    Unknown,
}
#[derive(Clone, PartialEq, Eq)]
struct PromptOwner {
    tickets: Vec<Ticket>,
    request_id: SessionRequestId,
}
pub(super) enum Failure {
    Read(crate::worker::file_intake::Failure),
    Upload,
}
pub(super) struct Completion {
    pub ticket: Ticket,
    pub result: Result<FileUploadValue, Failure>,
}
struct Operation {
    ticket: Ticket,
    cancellation: watch::Sender<bool>,
}
struct Staged {
    ticket: Ticket,
    value: FileUploadValue,
}
/// Bounded staged set: one active read/upload closure, at most this many private receipts.
/// Four 4 MiB files is the documented attachment bound; admission refuses beyond it.
pub(super) const MAX_STAGED: usize = 4;
#[derive(Default)]
pub(super) struct Coordinator {
    active: Option<Operation>,
    staged: Vec<Staged>,
    prompt: Option<PromptOwner>,
    last_serial: u64,
}
impl Coordinator {
    pub fn busy(&self) -> bool {
        self.active.is_some()
    }
    /// Private staged receipts for the caller's own display model; never exposed to the GUI as bytes.
    pub fn staged_tickets(&self) -> impl Iterator<Item = &Ticket> {
        self.staged.iter().map(|staged| &staged.ticket)
    }
    pub fn staged_metadata(&self, ticket: &Ticket) -> Option<Metadata> {
        self.staged
            .iter()
            .find(|staged| &staged.ticket == ticket)
            .map(|staged| Metadata {
                name: staged.value.file.name.clone(),
                bytes: staged.value.file.bytes,
            })
    }
    pub fn staged_count(&self) -> usize {
        self.staged.len()
    }
    /// Rejected/stale/replayed work performs no filesystem or network operation.
    pub fn admit(
        &mut self,
        ticket: &Ticket,
        epoch: u64,
        generation: u64,
        target: Option<&SessionId>,
        enabled: bool,
    ) -> Option<watch::Receiver<bool>> {
        if !enabled
            || self.active.is_some()
            || self.staged.len() >= MAX_STAGED
            || self.staged.iter().any(|staged| &staged.ticket == ticket)
            || self.prompt.is_some()
            || ticket.epoch != epoch
            || ticket.generation != generation
            || Some(&ticket.target) != target
            || ticket.serial == 0
            || ticket.serial <= self.last_serial
        {
            return None;
        }
        let (cancellation, receiver) = watch::channel(false);
        self.last_serial = ticket.serial;
        self.active = Some(Operation {
            ticket: ticket.clone(),
            cancellation,
        });
        Some(receiver)
    }
    /// Bind private authority to one explicit attempt over the whole staged set. The caller must name
    /// exactly the staged files: a stray or missing receipt refuses the prompt without consuming it.
    /// A failed/abandoned RPC never restores the receipts.
    pub fn prepare_prompt(
        &mut self,
        tickets: &[Ticket],
        mut request: SessionPromptRequest,
        epoch: u64,
        generation: u64,
        target: Option<&SessionId>,
        enabled: bool,
    ) -> Result<SessionPromptRequest, ()> {
        let bytes = request
            .content
            .iter()
            .try_fold(0usize, |total, part| match part {
                PromptContentPart::Text { text } => total.checked_add(text.len()),
                _ => None,
            });
        let complete = !tickets.is_empty()
            && tickets.len() == self.staged.len()
            && tickets
                .iter()
                .all(|ticket| self.staged.iter().any(|staged| &staged.ticket == ticket));
        if !enabled
            || self.active.is_some()
            || self.prompt.is_some()
            || tickets.iter().any(|ticket| {
                ticket.epoch != epoch
                    || ticket.generation != generation
                    || Some(&ticket.target) != target
            })
            || tickets
                .first()
                .is_none_or(|first| request.session_id != first.target)
            || !matches!(request.mode, PromptMode::Queue)
            || request.content.len() + tickets.len() > 8
            || bytes.is_none_or(|bytes| bytes > 32 * 1024)
            || !complete
        {
            return Err(());
        }
        for ticket in tickets {
            if let Some(index) = self
                .staged
                .iter()
                .position(|staged| &staged.ticket == ticket)
            {
                let staged = self.staged.remove(index);
                request.content.push(PromptContentPart::File {
                    receipt_id: staged.value.receipt_id,
                });
            }
        }
        self.staged.clear();
        self.prompt = Some(PromptOwner {
            tickets: tickets.to_vec(),
            request_id: request.request_id.clone(),
        });
        Ok(request)
    }
    /// Selection/discard cannot release a dispatched prompt; only its exact completion may do so.
    pub fn settle_prompt(&mut self, ticket: &Ticket, request_id: &SessionRequestId) -> bool {
        if self
            .prompt
            .as_ref()
            .is_some_and(|owner| owner.request_id == *request_id && owner.tickets.contains(ticket))
        {
            self.prompt = None;
            true
        } else {
            false
        }
    }
    /// Keep the active slot reserved until the non-abortable read/upload closure actually joins.
    pub fn invalidate(&mut self) {
        if let Some(active) = &self.active {
            active.cancellation.send_replace(true);
        }
        self.staged.clear();
    }
    pub fn discard(&mut self, ticket: &Ticket) {
        if self.active.as_ref().is_some_and(|op| &op.ticket == ticket) {
            self.active
                .as_ref()
                .unwrap()
                .cancellation
                .send_replace(true);
        }
        self.staged.retain(|staged| &staged.ticket != ticket);
    }
    pub fn settle(
        &mut self,
        completion: Completion,
        epoch: u64,
        generation: u64,
        target: Option<&SessionId>,
    ) -> Outcome {
        let Some(active) = self.active.as_ref() else {
            return Outcome::Unknown;
        };
        if active.ticket != completion.ticket {
            return Outcome::Unknown;
        }
        let current = !*active.cancellation.borrow()
            && completion.ticket.epoch == epoch
            && completion.ticket.generation == generation
            && Some(&completion.ticket.target) == target;
        self.active = None;
        match completion.result {
            Err(Failure::Read(error)) => Outcome::ReadRejected(error),
            Err(Failure::Upload) => Outcome::Unknown,
            Ok(value) if current => {
                if self.staged.len() >= MAX_STAGED {
                    return Outcome::Unknown;
                }
                let metadata = Metadata {
                    name: value.file.name.clone(),
                    bytes: value.file.bytes,
                };
                self.staged.push(Staged {
                    ticket: completion.ticket,
                    value,
                });
                Outcome::Staged(metadata)
            }
            Ok(_) => Outcome::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_native_transport::dto::{AttachmentId, FileAttachmentRef, FileUploadReceiptId};
    fn ticket(serial: u64) -> Ticket {
        Ticket {
            epoch: 1,
            generation: 3,
            serial,
            target: SessionId::new("PUBLIC_SELECTED").unwrap(),
        }
    }
    fn request() -> SessionPromptRequest {
        SessionPromptRequest {
            request_id: SessionRequestId::new("PUBLIC_prompt_1").unwrap(),
            session_id: ticket(1).target,
            mode: PromptMode::Queue,
            content: vec![PromptContentPart::Text {
                text: "PUBLIC text".into(),
            }],
            client_time_zone: Some("UTC".into()),
        }
    }
    fn staged_owner() -> (Coordinator, Ticket) {
        let t = ticket(1);
        let mut owner = Coordinator::default();
        let _receiver = admit(&mut owner, &t);
        owner.settle(completion(&t), 1, 3, Some(&t.target));
        (owner, t)
    }
    #[test]
    fn prompt_private_receipt_is_inserted_once_without_frontend_receipt_input() {
        let (mut owner, t) = staged_owner();
        let body = owner
            .prepare_prompt(
                std::slice::from_ref(&t),
                request(),
                1,
                3,
                Some(&t.target),
                true,
            )
            .unwrap();
        assert_eq!(body.content.len(), 2);
        assert!(
            matches!(&body.content[1],PromptContentPart::File{receipt_id} if receipt_id.as_str()=="PUBLIC_PRIVATE_RECEIPT")
        );
        assert!(owner.staged.is_empty());
        assert!(
            owner
                .prepare_prompt(
                    std::slice::from_ref(&t),
                    request(),
                    1,
                    3,
                    Some(&t.target),
                    true
                )
                .is_err()
        );
    }
    #[test]
    fn prompt_scope_and_forged_parts_refuse_without_consuming_ready_receipt() {
        for change in 0..8 {
            let (mut owner, t) = staged_owner();
            let mut request = request();
            let (mut epoch, mut generation, mut selected, mut enabled) =
                (1, 3, Some(t.target.clone()), true);
            match change {
                0 => epoch = 2,
                1 => generation = 4,
                2 => selected = None,
                3 => enabled = false,
                4 => request.session_id = SessionId::new("PUBLIC_foreign").unwrap(),
                5 => request.content.push(PromptContentPart::File {
                    receipt_id: FileUploadReceiptId::new("PUBLIC_forged").unwrap(),
                }),
                6 => {
                    request.content = vec![PromptContentPart::Text {
                        text: "x".repeat(32769),
                    }]
                }
                _ => {
                    request.content = vec![
                        PromptContentPart::Text {
                            text: String::new()
                        };
                        9
                    ]
                }
            }
            assert!(
                owner
                    .prepare_prompt(
                        std::slice::from_ref(&t),
                        request,
                        epoch,
                        generation,
                        selected.as_ref(),
                        enabled
                    )
                    .is_err()
            );
            assert!(!owner.staged.is_empty());
            assert!(owner.prompt.is_none());
        }
    }
    #[test]
    fn dispatched_prompt_keeps_slot_across_selection_discard_until_exact_settlement_never_restores_receipt()
     {
        let (mut owner, t) = staged_owner();
        let body = request();
        let id = body.request_id.clone();
        owner
            .prepare_prompt(std::slice::from_ref(&t), body, 1, 3, Some(&t.target), true)
            .unwrap();
        owner.invalidate();
        owner.discard(&t);
        assert!(
            owner
                .admit(&ticket(2), 1, 3, Some(&t.target), true)
                .is_none()
        );
        assert!(!owner.settle_prompt(&ticket(2), &id));
        assert!(!owner.settle_prompt(&t, &SessionRequestId::new("PUBLIC_other_request").unwrap()));
        assert!(owner.settle_prompt(&t, &id));
        assert!(!owner.settle_prompt(&t, &id));
        assert!(owner.staged.is_empty());
        assert!(
            owner
                .prepare_prompt(
                    std::slice::from_ref(&t),
                    request(),
                    1,
                    3,
                    Some(&t.target),
                    true
                )
                .is_err()
        );
        assert!(
            owner
                .admit(&ticket(2), 1, 3, Some(&t.target), true)
                .is_some()
        );
    }
    #[test]
    fn empty_text_file_only_prompt_is_supported_by_private_builder() {
        let (mut owner, t) = staged_owner();
        let mut request = request();
        request.content.clear();
        let request = owner
            .prepare_prompt(
                std::slice::from_ref(&t),
                request,
                1,
                3,
                Some(&t.target),
                true,
            )
            .unwrap();
        assert!(matches!(
            request.content.as_slice(),
            [PromptContentPart::File { .. }]
        ));
    }
    fn value() -> FileUploadValue {
        FileUploadValue {
            receipt_id: FileUploadReceiptId::new("PUBLIC_PRIVATE_RECEIPT").unwrap(),
            file: FileAttachmentRef {
                attachment_id: AttachmentId::new("PUBLIC_DIGEST").unwrap(),
                name: "PUBLIC.bin".into(),
                bytes: 3,
            },
        }
    }
    fn completion(ticket: &Ticket) -> Completion {
        Completion {
            ticket: ticket.clone(),
            result: Ok(value()),
        }
    }
    fn admit(owner: &mut Coordinator, ticket: &Ticket) -> watch::Receiver<bool> {
        owner
            .admit(ticket, 1, 3, Some(&ticket.target), true)
            .unwrap()
    }
    #[test]
    fn admission_refuses_wrong_epoch_generation_target_disabled_zero_and_replay() {
        let t = ticket(2);
        for (epoch, generation, target, enabled) in [
            (0, 3, Some(&t.target), true),
            (1, 4, Some(&t.target), true),
            (1, 3, None, true),
            (1, 3, Some(&t.target), false),
        ] {
            assert!(
                Coordinator::default()
                    .admit(&t, epoch, generation, target, enabled)
                    .is_none()
            );
        }
        assert!(
            Coordinator::default()
                .admit(&ticket(0), 1, 3, Some(&t.target), true)
                .is_none()
        );
        let mut owner = Coordinator::default();
        let _receiver = admit(&mut owner, &t);
        owner.invalidate();
        owner.settle(completion(&t), 1, 3, Some(&t.target));
        assert!(owner.admit(&t, 1, 3, Some(&t.target), true).is_none());
        assert!(
            owner
                .admit(&ticket(3), 1, 3, Some(&t.target), true)
                .is_some()
        );
    }
    #[test]
    fn stale_success_is_not_published_and_cancel_keeps_slot_until_joined() {
        let t = ticket(1);
        let mut owner = Coordinator::default();
        let receiver = admit(&mut owner, &t);
        owner.invalidate();
        assert!(*receiver.borrow());
        assert!(owner.busy());
        assert!(
            owner
                .admit(&ticket(2), 1, 3, Some(&t.target), true)
                .is_none()
        );
        assert_eq!(
            owner.settle(completion(&t), 1, 3, Some(&t.target)),
            Outcome::Unknown
        );
        assert!(!owner.busy());
        assert!(owner.staged.is_empty());
    }
    #[test]
    fn actual_selection_checks_reject_wrong_generation_or_target_after_success() {
        let t = ticket(1);
        for (epoch, generation, target) in [
            (2, 3, Some(&t.target)),
            (1, 4, Some(&t.target)),
            (1, 3, None),
        ] {
            let mut owner = Coordinator::default();
            let _receiver = admit(&mut owner, &t);
            assert_eq!(
                owner.settle(completion(&t), epoch, generation, target),
                Outcome::Unknown
            );
            assert!(owner.staged.is_empty());
        }
    }
    #[test]
    fn foreign_completion_or_discard_does_not_release_current_operation() {
        let t = ticket(1);
        let foreign = ticket(2);
        let mut owner = Coordinator::default();
        let receiver = admit(&mut owner, &t);
        owner.discard(&foreign);
        assert!(!*receiver.borrow());
        assert_eq!(
            owner.settle(completion(&foreign), 1, 3, Some(&t.target)),
            Outcome::Unknown
        );
        assert!(owner.busy());
        owner.discard(&t);
        assert!(*receiver.borrow());
        owner.settle(completion(&t), 1, 3, Some(&t.target));
        assert!(!owner.busy());
    }
    #[test]
    fn successful_receipt_stays_private_and_requires_explicit_discard_before_replacement() {
        let t = ticket(1);
        let mut owner = Coordinator::default();
        let _receiver = admit(&mut owner, &t);
        let outcome = owner.settle(completion(&t), 1, 3, Some(&t.target));
        assert_eq!(
            outcome,
            Outcome::Staged(Metadata {
                name: "PUBLIC.bin".into(),
                bytes: 3
            })
        );
        assert!(!format!("{outcome:?}").contains("PRIVATE_RECEIPT"));
        // A second file may join the staged set; the first receipt is neither sent nor replaced.
        let second = ticket(2);
        let _receiver = admit(&mut owner, &second);
        assert_eq!(owner.staged_count(), 1);
        assert!(matches!(
            owner.settle(completion(&second), 1, 3, Some(&second.target)),
            Outcome::Staged(_)
        ));
        assert_eq!(owner.staged_count(), 2);
        owner.discard(&second);
        assert_eq!(
            owner.staged_count(),
            1,
            "discard released another file's receipt"
        );
        assert!(
            owner.admit(&t, 1, 3, Some(&t.target), true).is_none(),
            "a replayed serial must stay refused"
        );
        owner.discard(&t);
        assert!(owner.staged.is_empty());
        let third = ticket(3);
        let _receiver = admit(&mut owner, &third);
        assert_eq!(owner.staged_count(), 0);
    }
    #[test]
    fn whole_staged_set_is_bound_into_one_prompt_with_one_file_part_each() {
        let (mut owner, first) = staged_owner();
        let second = ticket(2);
        let _receiver = admit(&mut owner, &second);
        assert!(matches!(
            owner.settle(completion(&second), 1, 3, Some(&second.target)),
            Outcome::Staged(_)
        ));
        assert_eq!(owner.staged_count(), 2);
        let body = owner
            .prepare_prompt(
                &[first.clone(), second.clone()],
                request(),
                1,
                3,
                Some(&first.target),
                true,
            )
            .expect("both staged receipts are owned");
        assert_eq!(body.content.len(), 3);
        assert!(matches!(body.content[0], PromptContentPart::Text { .. }));
        assert!(
            body.content[1..]
                .iter()
                .all(|part| matches!(part, PromptContentPart::File { .. }))
        );
        assert_eq!(owner.staged_count(), 0);
        assert!(owner.settle_prompt(&second, &body.request_id));
        assert!(!owner.settle_prompt(&second, &body.request_id));
    }
    #[test]
    fn prompt_refuses_when_the_named_set_omits_a_staged_receipt() {
        let (mut owner, first) = staged_owner();
        let second = ticket(2);
        let _receiver = admit(&mut owner, &second);
        assert!(matches!(
            owner.settle(completion(&second), 1, 3, Some(&second.target)),
            Outcome::Staged(_)
        ));
        assert!(
            owner
                .prepare_prompt(
                    std::slice::from_ref(&first),
                    request(),
                    1,
                    3,
                    Some(&first.target),
                    true
                )
                .is_err(),
            "a stray staged receipt must refuse the prompt instead of being dropped"
        );
        assert_eq!(owner.staged_count(), 2);
    }
    #[test]
    fn staged_authority_is_bounded_and_refuses_beyond_the_documented_maximum() {
        let mut owner = Coordinator::default();
        for serial in 1..=MAX_STAGED as u64 {
            let staged = ticket(serial);
            let _receiver = admit(&mut owner, &staged);
            assert!(matches!(
                owner.settle(completion(&staged), 1, 3, Some(&staged.target)),
                Outcome::Staged(_)
            ));
        }
        assert_eq!(owner.staged_count(), MAX_STAGED);
        let extra = ticket(MAX_STAGED as u64 + 1);
        assert!(
            owner
                .admit(&extra, 1, 3, Some(&extra.target), true)
                .is_none()
        );
    }
    #[test]
    fn local_read_rejection_is_not_network_uncertainty() {
        let t = ticket(1);
        let mut owner = Coordinator::default();
        let _receiver = admit(&mut owner, &t);
        assert_eq!(
            owner.settle(
                Completion {
                    ticket: t.clone(),
                    result: Err(Failure::Read(crate::worker::file_intake::Failure::Canceled))
                },
                1,
                3,
                Some(&t.target)
            ),
            Outcome::ReadRejected(crate::worker::file_intake::Failure::Canceled)
        );
        assert!(owner.staged.is_empty());
    }
}
