//! UI-only, bounded up-to-four-file authority. No receipts, file bodies, filesystem reads or
//! network calls. The caller invalidates on identity changes and discards returned tickets on
//! the worker. Context-free display accessors describe that caller-maintained visible authority.
//! [`Attachments`] reuses the proven per-file [`Draft`] state machine unchanged.
use crate::worker::{attachments, file_intake};
use attachments::{Metadata, Outcome, PromptOutcome, Submission, Ticket};
use dsh_native_transport::dto::{SessionId, SessionRequestId};
use std::{fmt, path::PathBuf};

pub const UPLOAD_CONSENT: &str =
    "Uploads now; Send submits later. Removing or canceling may leave stored bytes.";
const INVALID_PATH: &str =
    "Enter an absolute file path of at most 4096 bytes without NUL characters.";
const EXHAUSTED: &str = "Attachment authority exhausted. Restart before attaching another file.";
const UPLOAD_UNKNOWN: &str = "Upload outcome unknown. Stored bytes may remain. Remove before sending or choosing another file.";
const PROMPT_UNKNOWN: &str =
    "Send outcome unknown. Do not retry automatically. Remove before sending again.";

pub struct Context {
    pub epoch: u64,
    pub generation: u64,
    pub target: Option<SessionId>,
    pub allowed: bool,
}
impl Context {
    fn matches(&self, ticket: &Ticket) -> bool {
        self.epoch == ticket.epoch
            && self.generation == ticket.generation
            && self.target.as_ref() == Some(&ticket.target)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorStamp {
    opening: u64,
    serial: u64,
    epoch: u64,
    generation: u64,
}
struct Editor {
    stamp: EditorStamp,
    target: SessionId,
    text: String,
}
#[derive(Default)]
enum State {
    #[default]
    Empty,
    Reading(Ticket),
    Ready(Ticket, Metadata),
    Sending(Ticket, Metadata),
    Rejected(Ticket),
    Unknown(Ticket, Option<Metadata>),
}
impl State {
    fn ticket(&self) -> Option<&Ticket> {
        match self {
            Self::Empty => None,
            Self::Reading(t)
            | Self::Ready(t, _)
            | Self::Sending(t, _)
            | Self::Rejected(t)
            | Self::Unknown(t, _) => Some(t),
        }
    }
}
#[derive(Default)]
pub struct Draft {
    state: State,
    editor: Option<Editor>,
    read_owner: Option<Ticket>,
    prompt_owner: Option<(Ticket, SessionRequestId)>,
    upload_serial: u64,
    editor_serial: u64,
    exhausted: bool,
    notice: Option<&'static str>,
}
impl fmt::Debug for Draft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AttachmentDraft { .. }")
    }
}
impl Draft {
    fn next_editor(&mut self, c: &Context) -> Option<EditorStamp> {
        let Some(serial) = self.editor_serial.checked_add(1) else {
            self.exhausted = true;
            self.notice = Some(EXHAUSTED);
            if let Some(e) = &mut self.editor {
                e.text.clear();
            }
            return None;
        };
        self.editor_serial = serial;
        Some(EditorStamp {
            opening: serial,
            serial,
            epoch: c.epoch,
            generation: c.generation,
        })
    }
    fn matches_editor(&self, stamp: EditorStamp, c: &Context, exact: bool) -> bool {
        self.editor.as_ref().is_some_and(|e| {
            c.allowed
                && e.stamp.opening == stamp.opening
                && stamp.serial >= e.stamp.opening
                && stamp.serial <= e.stamp.serial
                && (!exact || e.stamp == stamp)
                && e.stamp.epoch == c.epoch
                && e.stamp.generation == c.generation
                && stamp.epoch == c.epoch
                && stamp.generation == c.generation
                && c.target.as_ref() == Some(&e.target)
        })
    }
    pub fn open(&mut self, c: &Context) -> bool {
        if !c.allowed
            || c.target.is_none()
            || self.exhausted
            || self.editor.is_some()
            || self.pending()
            || self.prompt_pending()
            || !matches!(self.state, State::Empty | State::Rejected(_))
            || self.state.ticket().is_some_and(|t| !c.matches(t))
        {
            return false;
        }
        let Some(stamp) = self.next_editor(c) else {
            return false;
        };
        self.state = State::Empty;
        self.editor = Some(Editor {
            stamp,
            target: c.target.clone().unwrap(),
            text: String::new(),
        });
        self.notice = None;
        true
    }
    pub fn editor(&self) -> Option<(EditorStamp, &str)> {
        self.editor.as_ref().map(|e| (e.stamp, e.text.as_str()))
    }
    pub fn editor_open(&self) -> bool {
        self.editor.is_some()
    }
    /// FIFO input may use an older revision of this opening; consent/cancel require the latest.
    /// Refresh even on rejected text, so queued consent cannot use an earlier path.
    pub fn edit(&mut self, stamp: EditorStamp, text: String, c: &Context) -> bool {
        if self.exhausted || !self.matches_editor(stamp, c, false) {
            return false;
        }
        let Some(next) = self.next_editor(c) else {
            return false;
        };
        let valid = text.len() <= 4096 && !text.contains('\0');
        let e = self.editor.as_mut().unwrap();
        e.stamp = EditorStamp {
            opening: stamp.opening,
            ..next
        };
        e.text = if valid { text } else { String::new() };
        self.notice = if valid { None } else { Some(INVALID_PATH) };
        valid
    }
    pub fn stage(&mut self, stamp: EditorStamp, c: &Context) -> Option<Submission> {
        if self.exhausted
            || self.pending()
            || self.prompt_pending()
            || !self.matches_editor(stamp, c, true)
        {
            return None;
        }
        let text = std::mem::take(&mut self.editor.as_mut()?.text);
        let source = match file_intake::SelectedFile::new(PathBuf::from(text)) {
            Ok(source) => source,
            Err(_) => {
                self.notice = Some(INVALID_PATH);
                return None;
            }
        };
        let Some(serial) = self.upload_serial.checked_add(1) else {
            self.exhausted = true;
            self.notice = Some(EXHAUSTED);
            return None;
        };
        self.upload_serial = serial;
        let ticket = Ticket {
            epoch: c.epoch,
            generation: c.generation,
            serial,
            target: c.target.clone()?,
        };
        self.editor = None;
        self.read_owner = Some(ticket.clone());
        self.state = State::Reading(ticket.clone());
        self.notice = None;
        Some(Submission { ticket, source })
    }
    pub fn complete(&mut self, ticket: &Ticket, outcome: Outcome, c: &Context) {
        if self.read_owner.as_ref() != Some(ticket) {
            return;
        }
        self.read_owner = None;
        if !matches!(&self.state, State::Reading(t) if t == ticket) {
            return;
        }
        self.state = State::Empty;
        if !c.matches(ticket) {
            self.notice = None;
            return;
        }
        self.state = match outcome {
            Outcome::Staged(metadata) => State::Ready(ticket.clone(), metadata),
            Outcome::Unknown => {
                self.notice = Some(UPLOAD_UNKNOWN);
                State::Unknown(ticket.clone(), None)
            }
            Outcome::NotSent => {
                self.notice = Some("Upload was not sent. Remove or choose the file again.");
                State::Rejected(ticket.clone())
            }
            Outcome::ReadRejected(failure) => {
                self.notice = Some(match failure {
                    file_intake::Failure::InvalidSelection => INVALID_PATH,
                    file_intake::Failure::Unavailable => {
                        "Selected file is unavailable. Remove or choose another file."
                    }
                    file_intake::Failure::NotRegular => {
                        "Selected file is not a regular file. Remove or choose another file."
                    }
                    file_intake::Failure::TooLarge => {
                        "Selected file exceeds the intake limit. Remove or choose another file."
                    }
                    file_intake::Failure::Canceled => {
                        "File intake canceled. Remove or choose the file again."
                    }
                });
                State::Rejected(ticket.clone())
            }
        };
    }
    pub fn ticket(&self) -> Option<&Ticket> {
        self.state.ticket()
    }
    pub fn ready(&self) -> Option<(&Ticket, &Metadata)> {
        match &self.state {
            State::Ready(t, m) => Some((t, m)),
            _ => None,
        }
    }
    pub fn metadata(&self) -> Option<&Metadata> {
        match &self.state {
            State::Ready(_, m) | State::Sending(_, m) | State::Unknown(_, Some(m)) => Some(m),
            _ => None,
        }
    }
    /// Global read reservation, including canceled or no-longer-visible work.
    pub fn pending(&self) -> bool {
        self.read_owner.is_some()
    }
    pub fn prompt_pending(&self) -> bool {
        self.prompt_owner.is_some()
    }
    pub fn reading(&self) -> bool {
        matches!(self.state, State::Reading(_))
    }
    pub fn notice(&self) -> Option<&'static str> {
        self.notice
    }
    pub fn cancel_editor(&mut self, stamp: EditorStamp, c: &Context) -> bool {
        if !self.matches_editor(stamp, c, true) {
            return false;
        }
        self.editor = None;
        self.notice = self.exhausted.then_some(EXHAUSTED);
        true
    }
    /// Only local authority is removed; the worker must discard the returned ticket.
    pub fn remove(&mut self, c: &Context) -> Option<Ticket> {
        if self.state.ticket().is_some_and(|t| !c.matches(t)) {
            return None;
        }
        if self.editor.as_ref().is_some_and(|e| {
            e.stamp.epoch != c.epoch
                || e.stamp.generation != c.generation
                || c.target.as_ref() != Some(&e.target)
        }) {
            return None;
        }
        self.invalidate()
    }
    /// Revoke visibility, but keep both global reservations until their actual callbacks.
    pub fn invalidate(&mut self) -> Option<Ticket> {
        let ticket = self
            .state
            .ticket()
            .or(self.read_owner.as_ref())
            .or(self.prompt_owner.as_ref().map(|(t, _)| t))
            .cloned();
        self.state = State::Empty;
        self.editor = None;
        self.notice = self.exhausted.then_some(EXHAUSTED);
        ticket
    }
    pub fn disconnect(&mut self) {
        self.invalidate();
        self.read_owner = None;
        self.prompt_owner = None;
    }
    pub fn has_file(&self, c: &Context) -> bool {
        c.allowed && self.ready().is_some_and(|(t, _)| c.matches(t))
    }
    /// Plaintext eligibility is separate: Empty needs no attachment capability.
    pub fn can_send(&self, c: &Context) -> bool {
        !self.editor_open()
            && !self.pending()
            && !self.prompt_pending()
            && (matches!(self.state, State::Empty) || (!self.exhausted && self.has_file(c)))
    }
    pub fn begin_prompt(&mut self, c: &Context, request_id: SessionRequestId) -> Option<Ticket> {
        if !self.can_send(c) || !self.has_file(c) {
            return None;
        }
        let State::Ready(ticket, metadata) = std::mem::take(&mut self.state) else {
            return None;
        };
        self.prompt_owner = Some((ticket.clone(), request_id));
        self.state = State::Sending(ticket.clone(), metadata);
        self.notice = None;
        Some(ticket)
    }
    pub fn prompt_complete(
        &mut self,
        ticket: &Ticket,
        request_id: &SessionRequestId,
        outcome: PromptOutcome,
        c: &Context,
    ) {
        if !self
            .prompt_owner
            .as_ref()
            .is_some_and(|(t, r)| t == ticket && r == request_id)
        {
            return;
        }
        if matches!(&self.state, State::Unknown(t, _) if t == ticket) && outcome != PromptOutcome::Unknown {
            self.reconcile_prompt(ticket, request_id, outcome == PromptOutcome::Accepted, c);
            return;
        }
        if outcome != PromptOutcome::Unknown { self.prompt_owner = None; }
        if !matches!(&self.state, State::Sending(t, _) if t == ticket) {
            return;
        }
        let State::Sending(t, metadata) = std::mem::take(&mut self.state) else {
            return;
        };
        if !c.matches(ticket) {
            self.notice = None;
            return;
        }
        self.state = match outcome {
            PromptOutcome::NotSent => {
                self.notice = Some("Send was not admitted. The uploaded file is still ready.");
                State::Ready(t, metadata)
            }
            PromptOutcome::Accepted => {
                self.notice = None;
                State::Empty
            }
            PromptOutcome::Unknown => {
                self.notice = Some(PROMPT_UNKNOWN);
                State::Unknown(t, Some(metadata))
            }
        };
    }
    /// Exact identity observation retires unknown prompt ownership, never restores an upload receipt.
    pub fn reconcile_prompt(&mut self, ticket: &Ticket, request_id: &SessionRequestId, accepted: bool, c: &Context) {
        if self.prompt_owner.as_ref().is_none_or(|(t,r)|t!=ticket || r!=request_id) { return; }
        self.prompt_owner=None;
        if !c.matches(ticket) || !matches!(&self.state,State::Unknown(t,_) | State::Sending(t,_) if t==ticket) { return; }
        self.state=State::Empty;
        self.notice=(!accepted).then_some("Original submission was rejected before dispatch. Upload receipt is not reused; choose the file again explicitly.");
    }
}

/// Documented native attachment bound: four files, each up to 4 MiB.
pub const MAX_ATTACHMENTS: usize = 4;
/// Bounded GUI-side set of up to [`MAX_ATTACHMENTS`] drafts. Every per-file rule stays owned by
/// one [`Draft`]; this type only owns slot membership, the single open editor and the exact
/// ticket set of one dispatched prompt. At most one slot is free (no file, no editor); a free
/// open slot is kept available while the set is below the cap.
pub struct Attachments {
    slots: Vec<Draft>,
    prompt: Option<(Vec<Ticket>, SessionRequestId)>,
    /// Manager-wide high-water marks. Each [`Draft`] owns its counters, so a fresh slot must
    /// continue the shared sequence instead of restarting it; otherwise two slots would mint the
    /// same ticket or editor stamp and a stale callback could address the wrong file.
    upload_serial: u64,
    editor_serial: u64,
}
impl Default for Attachments {
    fn default() -> Self {
        Self {
            slots: vec![Draft::default()],
            prompt: None,
            upload_serial: 0,
            editor_serial: 0,
        }
    }
}
impl fmt::Debug for Attachments {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Attachments { .. }")
    }
}
impl Attachments {
    fn free(slot: &Draft) -> bool {
        slot.ticket().is_none()
            && !slot.editor_open()
            && !slot.pending()
            && !slot.prompt_pending()
    }
    /// Occupied slots: every slot that owns a file identity, including a reading, rejected or
    /// unknown one. The cap counts these, never the free open slot.
    fn occupied(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.ticket().is_some())
            .count()
    }
    fn open_index(&self) -> Option<usize> {
        self.slots.iter().position(Self::free)
    }
    fn adopt_counters(&mut self) {
        for slot in &self.slots {
            self.upload_serial = self.upload_serial.max(slot.upload_serial);
            self.editor_serial = self.editor_serial.max(slot.editor_serial);
        }
    }
    fn fresh_slot(&self) -> Draft {
        Draft {
            upload_serial: self.upload_serial,
            editor_serial: self.editor_serial,
            ..Draft::default()
        }
    }
    /// Keep exactly one free open slot available without exceeding the cap.
    fn ensure_open_slot(&mut self) {
        self.adopt_counters();
        if self.slots.len() < MAX_ATTACHMENTS && self.open_index().is_none() {
            let slot = self.fresh_slot();
            self.slots.push(slot);
        }
    }
    /// Collapse surplus free slots to one, preserving every slot that still holds a file or an
    /// unsettled reservation, then restore the open slot.
    fn normalize(&mut self) {
        let mut normalized = Vec::with_capacity(self.slots.len());
        let mut kept_free = false;
        for slot in std::mem::take(&mut self.slots) {
            if Self::free(&slot) {
                if kept_free {
                    continue;
                }
                kept_free = true;
            }
            normalized.push(slot);
        }
        self.slots = normalized;
        self.ensure_open_slot();
    }
    fn editor_index(&self) -> Option<usize> {
        self.slots.iter().position(Draft::editor_open)
    }
    /// Every ready attachment in slot order; the UI list and the prompt set use it.
    pub fn attachments(&self) -> Vec<(&Ticket, &Metadata)> {
        self.slots.iter().filter_map(Draft::ready).collect()
    }
    /// Occupied slots that are not ready, as `(ticket, metadata, reading)` for display only.
    pub fn active(&self) -> Vec<(&Ticket, Option<&Metadata>, bool)> {
        self.slots
            .iter()
            .filter(|slot| slot.ready().is_none())
            .filter_map(|slot| {
                slot.ticket()
                    .map(|ticket| (ticket, slot.metadata(), slot.reading()))
            })
            .collect()
    }
    pub fn open(&mut self, c: &Context) -> bool {
        if self.pending() || self.prompt_pending() || self.at_capacity() {
            return false;
        }
        let Some(index) = self.open_index() else {
            return false;
        };
        let opened = self.slots[index].open(c);
        self.adopt_counters();
        opened
    }
    pub fn editor(&self) -> Option<(EditorStamp, &str)> {
        self.slots.iter().find_map(Draft::editor)
    }
    pub fn editor_open(&self) -> bool {
        self.editor_index().is_some()
    }
    pub fn edit(&mut self, stamp: EditorStamp, text: String, c: &Context) -> bool {
        let edited = match self.editor_index() {
            Some(index) => self.slots[index].edit(stamp, text, c),
            None => false,
        };
        self.adopt_counters();
        edited
    }
    pub fn stage(&mut self, stamp: EditorStamp, c: &Context) -> Option<Submission> {
        if self.pending() || self.prompt_pending() || self.at_capacity() {
            return None;
        }
        let index = self.editor_index()?;
        let submission = self.slots[index].stage(stamp, c);
        self.adopt_counters();
        submission
    }
    pub fn cancel_editor(&mut self, stamp: EditorStamp, c: &Context) -> bool {
        let Some(index) = self.editor_index() else {
            return false;
        };
        let canceled = self.slots[index].cancel_editor(stamp, c);
        if canceled {
            self.normalize();
        }
        canceled
    }
    /// Settle the exact reading slot, then make the open slot available again.
    pub fn complete(&mut self, ticket: &Ticket, outcome: Outcome, c: &Context) {
        for slot in &mut self.slots {
            slot.complete(ticket, outcome.clone(), c);
        }
        self.normalize();
    }
    /// Remove only the named slot's file and restore the free open slot.
    pub fn remove(&mut self, ticket: &Ticket, c: &Context) -> Option<Ticket> {
        let index = self
            .slots
            .iter()
            .position(|slot| slot.ticket() == Some(ticket))?;
        let removed = self.slots[index].remove(c)?;
        self.slots.remove(index);
        self.normalize();
        Some(removed)
    }
    /// Revoke visibility on every slot; each slot keeps its own reservations until its callback.
    pub fn invalidate(&mut self) {
        for slot in &mut self.slots {
            slot.invalidate();
        }
    }
    pub fn disconnect(&mut self) {
        for slot in &mut self.slots {
            slot.disconnect();
        }
        self.prompt = None;
    }
    pub fn has_file(&self, c: &Context) -> bool {
        self.slots.iter().any(|slot| slot.has_file(c))
    }
    /// Any occupied slot that is rejected or unknown blocks send, exactly as one slot did.
    pub fn can_send(&self, c: &Context) -> bool {
        !self.editor_open()
            && !self.pending()
            && !self.prompt_pending()
            && self.slots.iter().all(|slot| slot.can_send(c))
    }
    /// Display-only convenience accessors; the multi-file truth is [`Self::attachments`].
    pub fn ticket(&self) -> Option<&Ticket> {
        self.slots.iter().find_map(Draft::ticket)
    }
    /// Display-only convenience accessor; the multi-file truth is [`Self::attachments`].
    #[allow(dead_code)]
    pub fn metadata(&self) -> Option<&Metadata> {
        self.slots.iter().find_map(Draft::metadata)
    }
    pub fn ready(&self) -> Option<(&Ticket, &Metadata)> {
        self.slots.iter().find_map(Draft::ready)
    }
    /// Every ready ticket in slot order; the same set [`Self::begin_prompt`] returns.
    pub fn tickets(&self) -> Vec<Ticket> {
        self.attachments()
            .into_iter()
            .map(|(ticket, _)| ticket.clone())
            .collect()
    }
    /// Metadata for every occupied slot, including an in-flight prompt; display/validation only.
    pub fn metadata_all(&self) -> Vec<Metadata> {
        self.slots
            .iter()
            .filter_map(Draft::metadata)
            .cloned()
            .collect()
    }
    pub fn owns(&self, ticket: &Ticket) -> bool {
        self.slots.iter().any(|slot| slot.ticket() == Some(ticket))
    }
    pub fn pending(&self) -> bool {
        self.slots.iter().any(Draft::pending)
    }
    /// Display-only convenience accessor; the multi-file truth is [`Self::active`].
    #[allow(dead_code)]
    pub fn reading(&self) -> bool {
        self.slots.iter().any(Draft::reading)
    }
    pub fn prompt_pending(&self) -> bool {
        self.slots.iter().any(Draft::prompt_pending) || self.prompt.is_some()
    }
    pub fn notice(&self) -> Option<&'static str> {
        self.slots.iter().find_map(Draft::notice)
    }
    pub fn at_capacity(&self) -> bool {
        self.occupied() >= MAX_ATTACHMENTS
    }
    pub fn count(&self) -> usize {
        self.occupied()
    }
    /// Bind one explicit prompt attempt over every ready file. Requires at least one ready file,
    /// no pending read and no prompt already in flight; the returned set is exactly the tickets
    /// the worker must settle.
    pub fn begin_prompt(&mut self, c: &Context, request_id: SessionRequestId) -> Option<Vec<Ticket>> {
        if !self.can_send(c) || !self.has_file(c) || self.pending() || self.prompt_pending() {
            return None;
        }
        let mut tickets = Vec::new();
        for slot in self.slots.iter_mut().filter(|slot| slot.ready().is_some()) {
            let ticket = slot.begin_prompt(c, request_id.clone())?;
            tickets.push(ticket);
        }
        if tickets.is_empty() {
            return None;
        }
        self.prompt = Some((tickets.clone(), request_id));
        Some(tickets)
    }
    pub fn prompt_complete(
        &mut self,
        ticket: &Ticket,
        request_id: &SessionRequestId,
        outcome: PromptOutcome,
        c: &Context,
    ) {
        for slot in &mut self.slots {
            slot.prompt_complete(ticket, request_id, outcome, c);
        }
        self.retire_settled_prompt();
        self.normalize();
    }
    pub fn reconcile_prompt(
        &mut self,
        ticket: &Ticket,
        request_id: &SessionRequestId,
        accepted: bool,
        c: &Context,
    ) {
        for slot in &mut self.slots {
            slot.reconcile_prompt(ticket, request_id, accepted, c);
        }
        self.retire_settled_prompt();
        self.normalize();
    }
    /// The remembered set retires only once no slot still reserves a prompt callback.
    fn retire_settled_prompt(&mut self) {
        if !self.slots.iter().any(Draft::prompt_pending) {
            self.prompt = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> Context {
        Context {
            epoch: 1,
            generation: 4,
            target: Some(SessionId::new("draft-session").unwrap()),
            allowed: true,
        }
    }
    fn other_contexts() -> Vec<Context> {
        let mut epoch = context();
        epoch.epoch += 1;
        let mut generation = context();
        generation.generation += 1;
        let mut target = context();
        target.target = Some(SessionId::new("other-session").unwrap());
        let mut absent = context();
        absent.target = None;
        vec![epoch, generation, target, absent]
    }
    fn request() -> SessionRequestId {
        SessionRequestId::new("draft-request").unwrap()
    }
    fn metadata() -> Metadata {
        Metadata {
            name: "display.bin".into(),
            bytes: 17,
        }
    }
    fn edit(draft: &mut Draft, c: &Context, text: &str) -> EditorStamp {
        let stamp = draft.editor().unwrap().0;
        assert!(draft.edit(stamp, text.into(), c));
        draft.editor().unwrap().0
    }
    fn upload(draft: &mut Draft, c: &Context) -> Ticket {
        assert!(draft.open(c));
        let stamp = edit(draft, c, "/not-required-to-exist/file.bin");
        draft.stage(stamp, c).unwrap().ticket
    }
    fn staged(draft: &mut Draft, c: &Context) -> Ticket {
        let ticket = upload(draft, c);
        draft.complete(&ticket, Outcome::Staged(metadata()), c);
        ticket
    }

    #[test]
    fn empty_plaintext_is_independent_of_attachment_capability() {
        let mut draft = Draft::default();
        let mut c = context();
        c.allowed = false;
        c.target = None;
        assert!(draft.can_send(&c));
        assert!(!draft.has_file(&c));
        assert!(!draft.open(&c));
        assert!(draft.begin_prompt(&c, request()).is_none());
        assert!(draft.notice().is_none());
        assert!(draft.open(&context()));
        assert!(!draft.can_send(&context()));
        assert!(!draft.open(&context()));
        let stamp = draft.editor().unwrap().0;
        assert!(draft.cancel_editor(stamp, &context()));
        assert!(draft.can_send(&c));
    }

    #[test]
    fn editor_actions_require_exact_stamp_scope_and_current_permission() {
        let c = context();
        let mut draft = Draft::default();
        assert!(draft.open(&c));
        let stamp = edit(&mut draft, &c, "/review/file.bin");
        let mut contexts = other_contexts();
        let mut disabled = context();
        disabled.allowed = false;
        contexts.push(disabled);
        for wrong in contexts {
            assert!(!draft.edit(stamp, "/wrong/file.bin".into(), &wrong));
            assert!(draft.stage(stamp, &wrong).is_none());
            assert!(!draft.cancel_editor(stamp, &wrong));
            assert_eq!(draft.editor(), Some((stamp, "/review/file.bin")));
        }
        let mut serial = stamp;
        serial.serial += 1;
        let mut opening = stamp;
        opening.opening += 1;
        let mut epoch = stamp;
        epoch.epoch += 1;
        let mut generation = stamp;
        generation.generation += 1;
        for forged in [serial, opening, epoch, generation] {
            assert!(draft.stage(forged, &c).is_none());
            assert!(!draft.edit(forged, "/wrong/file.bin".into(), &c));
            assert!(!draft.cancel_editor(forged, &c));
        }
    }

    #[test]
    fn changed_path_and_reopened_editor_revoke_queued_actions() {
        let c = context();
        let mut draft = Draft::default();
        assert!(draft.open(&c));
        let old = edit(&mut draft, &c, "/old/file.bin");
        let fresh = edit(&mut draft, &c, "/new/file.bin");
        assert!(fresh.serial > old.serial);
        assert!(draft.stage(old, &c).is_none());
        assert!(!draft.cancel_editor(old, &c));
        assert_eq!(draft.editor(), Some((fresh, "/new/file.bin")));
        // Two FIFO inputs painted with one stamp must both land; consent remains exact-latest.
        assert!(draft.edit(old, "/queued/file.bin".into(), &c));
        let queued = draft.editor().unwrap().0;
        assert_eq!(queued.opening, old.opening);
        assert!(queued.serial > fresh.serial);
        assert_eq!(draft.editor().unwrap().1, "/queued/file.bin");
        assert!(draft.stage(fresh, &c).is_none());
        assert!(!draft.cancel_editor(fresh, &c));
        assert!(draft.cancel_editor(queued, &c));
        assert!(draft.open(&c));
        let reopened = draft.editor().unwrap().0;
        assert!(reopened.serial > queued.serial);
        assert_ne!(reopened.opening, queued.opening);
        assert!(!draft.cancel_editor(queued, &c));
        assert!(draft.stage(queued, &c).is_none());
        assert!(!draft.edit(queued, "/stale-opening/file.bin".into(), &c));
        assert_eq!(draft.editor(), Some((reopened, "")));
    }

    #[test]
    fn text_limits_clear_old_path_and_constructor_validation_is_io_free() {
        let c = context();
        let mut draft = Draft::default();
        assert!(draft.open(&c));
        for bad in [
            "x".repeat(4097),
            "é".repeat(2049),
            "/parent\0/file.bin".into(),
        ] {
            let old = edit(&mut draft, &c, "/previous/file.bin");
            assert!(!draft.edit(old, bad, &c));
            assert_eq!(draft.editor().unwrap().1, "");
            assert_ne!(draft.editor().unwrap().0, old);
            assert!(draft.notice().is_some());
            assert!(draft.stage(old, &c).is_none());
            assert!(draft.stage(draft.editor().unwrap().0, &c).is_none());
        }
        // Intermediate text is editable; only explicit consent validates the absolute path.
        for invalid in ["relative.bin", "/", "/parent/", "/parent/.", "/parent/.."] {
            let stamp = edit(&mut draft, &c, invalid);
            assert!(draft.stage(stamp, &c).is_none());
            assert_eq!(draft.editor().unwrap().1, "");
            assert!(!draft.pending());
        }
        let exact = format!("/{}/a", "é".repeat(2046) + "x");
        assert_eq!(exact.len(), 4096);
        let stamp = edit(&mut draft, &c, &exact);
        let submission = draft.stage(stamp, &c).unwrap();
        assert!(submission.ticket.serial > 0);
        assert!(!draft.editor_open());
        assert!(draft.pending());
    }

    #[test]
    fn ticket_and_editor_frontiers_survive_disconnect_without_reuse() {
        let c = context();
        let mut draft = Draft::default();
        let first = upload(&mut draft, &c);
        let first_editor = draft.editor_serial;
        draft.disconnect();
        let second = upload(&mut draft, &c);
        assert!(second.serial > first.serial);
        assert!(draft.editor_serial > first_editor);
        draft.complete(&first, Outcome::NotSent, &c);
        assert!(draft.pending());
        draft.complete(&second, Outcome::Staged(metadata()), &c);
        assert_eq!(draft.ready().unwrap().0, &second);
    }

    #[test]
    fn checked_counter_overflow_fails_closed_for_file_actions() {
        let c = context();
        let mut open_exhausted = Draft {
            editor_serial: u64::MAX,
            ..Draft::default()
        };
        assert!(!open_exhausted.open(&c));
        assert!(open_exhausted.notice().is_some());
        assert!(open_exhausted.can_send(&c)); // Plaintext has no file authority to reuse.
        open_exhausted.disconnect();
        assert!(!open_exhausted.open(&c));

        let mut edit_exhausted = Draft::default();
        assert!(edit_exhausted.open(&c));
        let stamp = edit(&mut edit_exhausted, &c, "/previous/file.bin");
        edit_exhausted.editor_serial = u64::MAX;
        assert!(!edit_exhausted.edit(stamp, "/next/file.bin".into(), &c));
        assert_eq!(edit_exhausted.editor().unwrap().1, "");
        assert!(edit_exhausted.stage(stamp, &c).is_none());
        assert!(edit_exhausted.cancel_editor(stamp, &c));
        assert!(edit_exhausted.can_send(&c));

        let mut upload_exhausted = Draft {
            upload_serial: u64::MAX - 1,
            ..Draft::default()
        };
        let last = upload(&mut upload_exhausted, &c);
        assert_eq!(last.serial, u64::MAX);
        upload_exhausted.disconnect();
        assert!(upload_exhausted.open(&c));
        let stamp = edit(&mut upload_exhausted, &c, "/next/file.bin");
        assert!(upload_exhausted.stage(stamp, &c).is_none());
        assert!(!upload_exhausted.pending());
        assert_eq!(upload_exhausted.editor().unwrap().1, "");
        assert!(upload_exhausted.cancel_editor(stamp, &c));
        assert!(upload_exhausted.can_send(&c));
        assert!(!upload_exhausted.open(&c));
    }

    #[test]
    fn ready_is_current_scope_only_and_requires_explicit_remove_before_replacement() {
        let c = context();
        let mut draft = Draft::default();
        let ticket = staged(&mut draft, &c);
        assert!(draft.has_file(&c));
        assert!(draft.can_send(&c));
        assert_eq!(draft.metadata(), Some(&metadata()));
        assert!(!draft.open(&c));
        for wrong in other_contexts() {
            assert!(!draft.has_file(&wrong));
            assert!(!draft.can_send(&wrong));
            assert!(draft.begin_prompt(&wrong, request()).is_none());
            assert!(draft.remove(&wrong).is_none());
            assert_eq!(draft.ready().unwrap().0, &ticket);
        }
        assert_eq!(draft.remove(&c), Some(ticket));
        assert!(draft.metadata().is_none());
        assert!(draft.can_send(&c));
        assert!(draft.open(&c));
    }

    #[test]
    fn foreign_upload_callbacks_including_queue_refusal_cannot_release_owned_slot() {
        let c = context();
        let mut draft = Draft::default();
        let ticket = upload(&mut draft, &c);
        let mut wrong_serial = ticket.clone();
        wrong_serial.serial += 1;
        let mut wrong_epoch = ticket.clone();
        wrong_epoch.epoch += 1;
        let mut wrong_generation = ticket.clone();
        wrong_generation.generation += 1;
        let mut wrong_target = ticket.clone();
        wrong_target.target = SessionId::new("foreign").unwrap();
        for foreign in [wrong_serial, wrong_epoch, wrong_generation, wrong_target] {
            draft.complete(&foreign, Outcome::NotSent, &c);
            draft.complete(&foreign, Outcome::Staged(metadata()), &c);
            assert!(draft.pending());
            assert!(draft.reading());
            assert!(draft.ready().is_none());
        }
        draft.complete(&ticket, Outcome::Staged(metadata()), &c);
        assert!(!draft.pending());
        assert_eq!(draft.ready().unwrap().0, &ticket);
        draft.complete(&ticket, Outcome::Unknown, &c);
        assert!(draft.notice().is_none());
        assert!(draft.has_file(&c));
    }

    #[test]
    fn canceled_read_reservation_survives_selection_until_exact_completion() {
        let c = context();
        let next = other_contexts().remove(1);
        let mut draft = Draft::default();
        let ticket = upload(&mut draft, &c);
        assert_eq!(draft.remove(&c), Some(ticket.clone()));
        assert_eq!(draft.invalidate(), Some(ticket.clone()));
        assert!(draft.pending());
        assert!(!draft.reading());
        assert!(!draft.can_send(&next));
        assert!(!draft.open(&next));
        let mut foreign = ticket.clone();
        foreign.serial += 1;
        draft.complete(&foreign, Outcome::NotSent, &next);
        assert!(draft.pending());
        draft.complete(&ticket, Outcome::Staged(metadata()), &next);
        assert!(!draft.pending());
        assert!(draft.ready().is_none());
        assert!(draft.notice().is_none());
        assert!(draft.open(&next));
        let fresh = edit(&mut draft, &next, "/fresh/file.bin");
        draft.complete(&ticket, Outcome::Unknown, &next);
        assert_eq!(draft.editor(), Some((fresh, "/fresh/file.bin")));
        assert!(draft.notice().is_none());
    }

    #[test]
    fn exact_upload_completion_releases_but_never_adopts_wrong_current_scope() {
        let c = context();
        for wrong in other_contexts() {
            for outcome in [
                Outcome::Staged(metadata()),
                Outcome::Unknown,
                Outcome::NotSent,
                Outcome::ReadRejected(file_intake::Failure::Canceled),
            ] {
                let mut draft = Draft::default();
                let ticket = upload(&mut draft, &c);
                draft.complete(&ticket, outcome, &wrong);
                assert!(!draft.pending());
                assert!(draft.ready().is_none());
                assert!(draft.metadata().is_none());
                assert!(draft.notice().is_none());
                assert!(draft.can_send(&wrong));
            }
        }
    }

    #[test]
    fn rejected_reads_require_remove_or_repick_unknown_upload_requires_remove() {
        let c = context();
        for outcome in [
            Outcome::NotSent,
            Outcome::ReadRejected(file_intake::Failure::InvalidSelection),
            Outcome::ReadRejected(file_intake::Failure::Unavailable),
            Outcome::ReadRejected(file_intake::Failure::NotRegular),
            Outcome::ReadRejected(file_intake::Failure::TooLarge),
            Outcome::ReadRejected(file_intake::Failure::Canceled),
        ] {
            let mut draft = Draft::default();
            let ticket = upload(&mut draft, &c);
            draft.complete(&ticket, outcome, &c);
            assert!(!draft.can_send(&c));
            assert!(draft.notice().is_some());
            assert!(draft.open(&c)); // Explicit repick, never automatic retry.
            assert!(draft.notice().is_none());
        }
        let mut draft = Draft::default();
        let ticket = upload(&mut draft, &c);
        draft.complete(&ticket, Outcome::Unknown, &c);
        assert!(!draft.can_send(&c));
        assert!(!draft.open(&c));
        assert_eq!(draft.notice(), Some(UPLOAD_UNKNOWN));
        assert!(draft.metadata().is_none());
        assert_eq!(draft.remove(&c), Some(ticket));
        assert!(draft.can_send(&c));
        assert!(draft.notice().is_none());
    }

    #[test]
    fn prompt_reserves_exact_ticket_and_request_and_accepted_consumes_authority() {
        let c = context();
        let mut draft = Draft::default();
        let ticket = staged(&mut draft, &c);
        let id = request();
        assert_eq!(draft.begin_prompt(&c, id.clone()), Some(ticket.clone()));
        assert!(draft.ready().is_none());
        assert_eq!(draft.metadata(), Some(&metadata()));
        assert!(draft.prompt_pending());
        assert!(!draft.can_send(&c));
        assert!(!draft.open(&c));
        assert!(draft.begin_prompt(&c, id.clone()).is_none());
        let mut foreign = ticket.clone();
        foreign.serial += 1;
        draft.prompt_complete(&foreign, &id, PromptOutcome::NotSent, &c);
        let other_id = SessionRequestId::new("foreign-request").unwrap();
        draft.prompt_complete(&ticket, &other_id, PromptOutcome::Accepted, &c);
        assert!(draft.prompt_pending());
        draft.prompt_complete(&ticket, &id, PromptOutcome::Accepted, &c);
        assert!(!draft.prompt_pending());
        assert!(draft.metadata().is_none());
        assert!(draft.can_send(&c));
        draft.prompt_complete(&ticket, &id, PromptOutcome::NotSent, &c);
        assert!(draft.ready().is_none());
    }

    #[test]
    fn queue_not_sent_restores_ready_and_hidden_panels_do_not_discard_owned_results() {
        let c = context();
        let mut hidden = context();
        hidden.allowed = false;
        let mut draft = Draft::default();
        let ticket = upload(&mut draft, &c);
        draft.complete(&ticket, Outcome::Staged(metadata()), &hidden);
        assert_eq!(draft.ready().unwrap().0, &ticket);
        assert!(!draft.can_send(&hidden));
        let id = request();
        assert_eq!(draft.begin_prompt(&c, id.clone()), Some(ticket.clone()));
        draft.prompt_complete(&ticket, &id, PromptOutcome::NotSent, &hidden);
        assert!(!draft.prompt_pending());
        assert_eq!(draft.ready().unwrap(), (&ticket, &metadata()));
        assert!(draft.notice().is_some());
        assert!(draft.can_send(&c));
        assert!(!draft.can_send(&hidden));
        let retry_id = SessionRequestId::new("explicit-retry-request").unwrap();
        assert_eq!(
            draft.begin_prompt(&c, retry_id.clone()),
            Some(ticket.clone())
        );
        draft.prompt_complete(&ticket, &id, PromptOutcome::Unknown, &c);
        assert!(draft.prompt_pending());
        assert!(draft.notice().is_none());
        draft.prompt_complete(&ticket, &retry_id, PromptOutcome::Accepted, &hidden);
        assert!(draft.can_send(&c));
    }

    #[test]
    fn unknown_prompt_retains_owner_until_exact_observation_and_never_restores_a_receipt() {
        let c = context();
        let mut draft = Draft::default();
        let ticket = staged(&mut draft, &c);
        let id = request();
        draft.begin_prompt(&c, id.clone()).unwrap();
        draft.prompt_complete(&ticket, &id, PromptOutcome::Unknown, &c);
        assert!(draft.prompt_pending());
        assert!(!draft.can_send(&c));
        assert!(!draft.has_file(&c));
        assert!(draft.ready().is_none());
        assert_eq!(draft.metadata(), Some(&metadata()));
        assert_eq!(draft.notice(), Some(PROMPT_UNKNOWN));
        assert!(draft.begin_prompt(&c, id.clone()).is_none());
        assert!(!draft.open(&c));
        let wrong=SessionRequestId::new("PUBLIC wrong observation").unwrap();
        draft.reconcile_prompt(&ticket,&wrong,true,&c);
        assert!(draft.prompt_pending());
        draft.reconcile_prompt(&ticket,&id,false,&c);
        assert!(!draft.prompt_pending());
        assert!(draft.ready().is_none());
        assert!(draft.can_send(&c));
        assert!(draft.metadata().is_none());
        assert!(draft.notice().unwrap().contains("not reused"));
    }

    #[test]
    fn exact_old_reconciliation_after_view_invalidation_does_not_clear_a_new_file_or_delayed_read() {
        let c=context();let next=other_contexts().remove(1);let mut draft=Draft::default();
        let old=staged(&mut draft,&c);let id=request();draft.begin_prompt(&c,id.clone());draft.prompt_complete(&old,&id,PromptOutcome::Unknown,&c);
        draft.invalidate();
        let fresh=Ticket{epoch:next.epoch,generation:next.generation,serial:old.serial+1,target:next.target.clone().unwrap()};
        draft.state=State::Ready(fresh.clone(),metadata());draft.read_owner=Some(fresh.clone());
        draft.reconcile_prompt(&old,&id,true,&next);
        assert!(!draft.prompt_pending());assert_eq!(draft.ready().unwrap().0,&fresh);assert!(draft.pending());
        draft.complete(&old,Outcome::Staged(metadata()),&next);
        assert_eq!(draft.ready().unwrap().0,&fresh);assert!(draft.pending());
    }
    #[test]
    fn stale_prompt_and_disconnect_callbacks_never_clobber_fresh_draft_and_debug_is_redacted() {
        let c = context();
        let next = other_contexts().remove(1);
        // Even if the parent has not invalidated yet, current identity prevents adoption.
        for wrong in other_contexts() {
            for outcome in [
                PromptOutcome::NotSent,
                PromptOutcome::Accepted,
                PromptOutcome::Unknown,
            ] {
                let mut draft = Draft::default();
                let ticket = staged(&mut draft, &c);
                let id = request();
                draft.begin_prompt(&c, id.clone()).unwrap();
                draft.prompt_complete(&ticket, &id, outcome, &wrong);
                if outcome==PromptOutcome::Unknown {assert!(draft.prompt_pending());assert!(!draft.can_send(&wrong));draft.reconcile_prompt(&ticket,&id,true,&wrong);}
                assert!(!draft.prompt_pending());
                assert!(draft.metadata().is_none());
                assert!(draft.notice().is_none());
                assert!(draft.can_send(&wrong));
            }
        }
        let mut disconnected = Draft::default();
        let old = staged(&mut disconnected, &c);
        let old_id = request();
        disconnected.begin_prompt(&c, old_id.clone()).unwrap();
        disconnected.disconnect();
        assert!(!disconnected.prompt_pending());
        let fresh = upload(&mut disconnected, &next);
        disconnected.prompt_complete(&old, &old_id, PromptOutcome::NotSent, &next);
        assert!(disconnected.pending());
        disconnected.complete(&fresh, Outcome::Staged(metadata()), &next);
        assert_eq!(disconnected.ready().unwrap().0, &fresh);
        for outcome in [
            PromptOutcome::NotSent,
            PromptOutcome::Accepted,
            PromptOutcome::Unknown,
        ] {
            let mut draft = Draft::default();
            let ticket = staged(&mut draft, &c);
            let id = request();
            draft.begin_prompt(&c, id.clone()).unwrap();
            assert_eq!(draft.remove(&c), Some(ticket.clone()));
            assert_eq!(draft.invalidate(), Some(ticket.clone()));
            assert!(draft.prompt_pending());
            assert!(!draft.open(&next));
            assert!(!draft.can_send(&next));
            draft.prompt_complete(&ticket, &id, outcome, &next);
            if outcome==PromptOutcome::Unknown {assert!(draft.prompt_pending());draft.reconcile_prompt(&ticket,&id,true,&next);}
            assert!(!draft.prompt_pending());
            assert!(draft.metadata().is_none());
            assert!(draft.notice().is_none());
            assert!(draft.can_send(&next));
            assert!(draft.open(&next));
            let stamp = edit(&mut draft, &next, "/PRIVATE_PARENT/PRIVATE_LEAF.bin");
            assert_eq!(format!("{draft:?}"), "AttachmentDraft { .. }");
            assert!(!format!("{stamp:?}").contains("PRIVATE_"));
            draft.prompt_complete(&ticket, &id, PromptOutcome::Unknown, &next);
            assert_eq!(
                draft.editor(),
                Some((stamp, "/PRIVATE_PARENT/PRIVATE_LEAF.bin"))
            );
            draft.disconnect();
            assert!(!draft.editor_open());
            let fresh = upload(&mut draft, &next);
            assert!(fresh.serial > ticket.serial);
            draft.complete(&ticket, Outcome::Unknown, &next);
            draft.prompt_complete(&ticket, &id, PromptOutcome::Unknown, &next);
            assert!(draft.pending());
            assert!(draft.notice().is_none());
            draft.complete(&fresh, Outcome::Staged(metadata()), &next);
            assert_eq!(draft.ready().unwrap().0, &fresh);
        }
    }

    fn ready_files(files: &mut Attachments, c: &Context, count: usize) -> Vec<Ticket> {
        let mut tickets = Vec::new();
        for _ in 0..count {
            assert!(files.open(c));
            let stamp = files.editor().unwrap().0;
            assert!(files.edit(stamp, "/not-required-to-exist/file.bin".into(), c));
            let stamp = files.editor().unwrap().0;
            let ticket = files.stage(stamp, c).unwrap().ticket;
            files.complete(&ticket, Outcome::Staged(metadata()), c);
            tickets.push(ticket);
        }
        tickets
    }
    fn forged_stamp() -> EditorStamp {
        EditorStamp {
            opening: 0,
            serial: 0,
            epoch: 0,
            generation: 0,
        }
    }

    #[test]
    fn two_ready_files_bind_one_exactly_two_ticket_prompt() {
        let c = context();
        let mut files = Attachments::default();
        let tickets = ready_files(&mut files, &c, 2);
        assert_eq!(files.attachments().len(), 2);
        assert_eq!(files.tickets(), tickets);
        // Slots must not restart the shared serial sequence and mint equal tickets.
        assert_ne!(tickets[0].serial, tickets[1].serial);
        assert!(!files.at_capacity());
        let held = files.begin_prompt(&c, request()).unwrap();
        assert_eq!(held, tickets);
        assert!(files.prompt_pending());
        assert!(!files.can_send(&c));
        assert!(files.attachments().is_empty());
        // The whole set settles one ticket at a time; ownership clears only after the last one.
        files.prompt_complete(&held[0], &request(), PromptOutcome::Accepted, &c);
        assert!(files.prompt_pending());
        assert!(files.active().iter().any(|(ticket, _, _)| *ticket == &held[1]));
        files.prompt_complete(&held[1], &request(), PromptOutcome::Accepted, &c);
        assert!(!files.prompt_pending());
        assert!(!files.pending());
        assert_eq!(files.count(), 0);
    }

    #[test]
    fn capacity_refuses_a_fifth_attachment() {
        let c = context();
        let mut files = Attachments::default();
        ready_files(&mut files, &c, MAX_ATTACHMENTS);
        assert_eq!(files.count(), MAX_ATTACHMENTS);
        assert!(files.at_capacity());
        // The cap consumes the open slot, so no fifth editor or stage can start.
        assert!(!files.open(&c));
        assert!(!files.editor_open());
        assert!(files.stage(forged_stamp(), &c).is_none());
        assert_eq!(files.tickets().len(), MAX_ATTACHMENTS);
    }

    #[test]
    fn remove_releases_only_the_named_slot_and_keeps_the_rest_sendable() {
        let c = context();
        let mut files = Attachments::default();
        let tickets = ready_files(&mut files, &c, 3);
        assert_eq!(files.remove(&tickets[1], &c), Some(tickets[1].clone()));
        assert_eq!(files.attachments().len(), 2);
        assert!(files.owns(&tickets[0]) && files.owns(&tickets[2]));
        assert!(!files.owns(&tickets[1]));
        assert!(files.can_send(&c) && files.has_file(&c));
        assert!(files.remove(&tickets[1], &c).is_none());
        let held = files.begin_prompt(&c, request()).unwrap();
        assert_eq!(held, vec![tickets[0].clone(), tickets[2].clone()]);
    }

    #[test]
    fn pending_prompt_blocks_staging_and_sending() {
        let c = context();
        let mut files = Attachments::default();
        let tickets = ready_files(&mut files, &c, 1);
        let held = files.begin_prompt(&c, request()).unwrap();
        assert_eq!(held, tickets);
        assert!(!files.can_send(&c));
        assert!(!files.open(&c));
        assert!(!files.at_capacity());
        assert!(files.stage(forged_stamp(), &c).is_none());
        assert!(files.begin_prompt(&c, request()).is_none());
    }

    #[test]
    fn notice_surfaces_a_rejected_slot_and_removal_restores_send() {
        let c = context();
        let mut files = Attachments::default();
        assert!(files.open(&c));
        let stamp = files.editor().unwrap().0;
        assert!(files.edit(stamp, "/rejected/file.bin".into(), &c));
        let stamp = files.editor().unwrap().0;
        let ticket = files.stage(stamp, &c).unwrap().ticket;
        files.complete(&ticket, Outcome::NotSent, &c);
        assert!(files.attachments().is_empty());
        assert_eq!(files.active().len(), 1);
        assert!(files.notice().is_some_and(|notice| notice.contains("not sent")));
        assert!(!files.can_send(&c));
        assert!(!files.at_capacity());
        assert!(files.open(&c), "an explicit repick slot stays available");
        files.cancel_editor(files.editor().unwrap().0, &c);
        assert_eq!(files.remove(&ticket, &c), Some(ticket));
        assert!(files.notice().is_none());
        assert!(files.can_send(&c));
        assert_eq!(files.count(), 0);
    }
}
