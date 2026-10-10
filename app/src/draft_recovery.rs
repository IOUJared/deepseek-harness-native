//! Pure sequencing for local UNSENT text recovery; never send/replay/admission evidence.
//! App.drafts and the current editor own bodies. This controller owns only bounded metadata.
//! The parent captures a complete snapshot synchronously with `next_save`, performs one blocking
//! Store operation, joins it, then delivers its ticketed outcome. Do not cancel admitted IO or
//! release a ticket before that join. A loaded map is validated by Store, not a durability ACK;
//! merge only entries for which `touched` is false, including after explicit reconciliation.
use super::draft_store::{Drafts, Failure, MAX_DRAFTS};
use dsh_native_transport::dto::SessionId;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Load,
    Save,
}

/// The revision belongs to the snapshot captured by the parent, not the latest editor body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ticket {
    pub id: u64,
    pub revision: u64,
    pub kind: Kind,
}

/// Contains private plaintext on success; intentionally has no Debug implementation.
/// Loaded results must come from Store::load's bounded, validated document.
#[derive(Clone)]
pub enum Outcome {
    Loaded {
        ticket: Ticket,
        result: Result<Drafts, Failure>,
    },
    Saved {
        ticket: Ticket,
        result: Result<(), Failure>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Issue {
    Storage(Failure),
    CounterExhausted,
    TouchLimit,
}
impl Issue {
    pub fn message(self) -> &'static str {
        match self {
            Self::Storage(Failure::InvalidHome) => "Invalid native draft home",
            Self::Storage(Failure::Unavailable) => "Native draft storage unavailable",
            Self::Storage(Failure::InvalidData) => "Native draft document invalid",
            Self::Storage(Failure::Limit) => "Native draft storage limit exceeded",
            Self::Storage(Failure::Busy) => "Native draft storage busy; recovery paused",
            Self::Storage(Failure::Conflict) => "Native draft storage changed; recovery paused",
            Self::Storage(Failure::NotSaved) => "Native drafts not saved; recovery paused",
            Self::Storage(Failure::Indeterminate) => {
                "Native draft save uncertain; explicitly reload to reconcile"
            }
            Self::CounterExhausted => "Native draft revision limit reached; recovery disabled",
            Self::TouchLimit => "Native draft tracking limit reached; recovery disabled",
        }
    }
    fn terminal(self) -> bool {
        matches!(self, Self::CounterExhausted | Self::TouchLimit)
    }
}

#[derive(Default, Eq, PartialEq)]
enum LoadPhase {
    #[default]
    NotStarted,
    Loading,
    Ready,
    Failed,
}

/// No bodies, filesystem access, worker ownership, clocks, or implicit retries.
#[derive(Default)]
pub struct State {
    phase: LoadPhase,
    revision: u64,
    last_id: u64,
    pending: Option<Ticket>,
    dirty: bool,
    touched: BTreeSet<SessionId>,
    protect_all: bool,
    failure: Option<Issue>,
}
impl State {
    /// Admit the initial read once. Subsequent reads require explicit `reload`.
    pub fn start_load(&mut self) -> Option<Ticket> {
        if self.phase != LoadPhase::NotStarted {
            return None;
        }
        self.admit_load()
    }

    /// Explicit user reconciliation, never an automatic mutation retry.
    /// Success permits a new save of the parent's merged current map, but preserves dirty state:
    /// a readable document does not acknowledge an earlier indeterminate write's durability.
    /// Terminal metadata exhaustion cannot be reset without rebuilding the application state.
    pub fn reload(&mut self) -> Option<Ticket> {
        if matches!(self.phase, LoadPhase::NotStarted | LoadPhase::Loading) {
            return None;
        }
        self.admit_load()
    }

    fn admit_load(&mut self) -> Option<Ticket> {
        if self.busy() || self.terminal() {
            return None;
        }
        let ticket = self.ticket(Kind::Load)?;
        self.phase = LoadPhase::Loading;
        Some(ticket)
    }

    /// Call on every owned body mutation, including removal on the matching send ACK.
    /// Edits remain in the parent's memory even when false disables persistence. Distinct touched
    /// IDs are never evicted; exhaustion protects every session from a later recovery merge.
    pub fn touch(&mut self, session: &SessionId) -> bool {
        self.dirty = true;
        if self.terminal() {
            return false;
        }
        let Some(revision) = self.revision.checked_add(1) else {
            self.disable(Issue::CounterExhausted);
            return false;
        };
        self.revision = revision;
        if !self.touched.contains(session) {
            if self.touched.len() >= MAX_DRAFTS {
                self.disable(Issue::TouchLimit);
                return false;
            }
            self.touched.insert(session.clone());
        }
        true
    }

    /// This includes edits before/during a read and all previously owned edits or ACK removals.
    pub fn touched(&self, session: &SessionId) -> bool {
        self.protect_all || self.touched.contains(session)
    }

    /// Capture the complete current draft map before scheduling the returned ticket's IO.
    /// There is one in-flight read OR write; edits during it coalesce into the next snapshot.
    pub fn next_save(&mut self) -> Option<Ticket> {
        if !self.ready() || !self.dirty || self.busy() || self.failure.is_some() {
            return None;
        }
        self.ticket(Kind::Save)
    }

    /// Wrong kind, stale, forged revision, and duplicate results leave all state unchanged.
    pub fn loaded(&mut self, ticket: Ticket, result: Result<Drafts, Failure>) -> Option<Drafts> {
        if !self.matches(ticket, Kind::Load) {
            return None;
        }
        self.pending = None;
        if self.terminal() {
            self.phase = LoadPhase::Failed;
            return None;
        }
        match result {
            Ok(drafts) => {
                self.phase = LoadPhase::Ready;
                self.failure = None;
                Some(drafts)
            }
            Err(failure) => {
                self.phase = LoadPhase::Failed;
                self.failure = Some(Issue::Storage(failure));
                None
            }
        }
    }

    /// True means the owned joined outcome was consumed, not that persistence succeeded.
    /// Only a matching successful snapshot revision clears dirty; every error stops auto saves.
    pub fn saved(&mut self, ticket: Ticket, result: Result<(), Failure>) -> bool {
        if !self.matches(ticket, Kind::Save) {
            return false;
        }
        self.pending = None;
        if self.terminal() {
            return true;
        }
        match result {
            Ok(()) => {
                if self.revision == ticket.revision {
                    self.dirty = false;
                }
            }
            Err(failure) => self.failure = Some(Issue::Storage(failure)),
        }
        true
    }

    pub fn apply(&mut self, outcome: Outcome) -> Option<Drafts> {
        match outcome {
            Outcome::Loaded { ticket, result } => self.loaded(ticket, result),
            Outcome::Saved { ticket, result } => {
                self.saved(ticket, result);
                None
            }
        }
    }

    pub fn is_loading(&self) -> bool {
        self.phase == LoadPhase::Loading
    }
    /// A validated read completed. Check `failure` separately before offering saving/recovery.
    pub fn ready(&self) -> bool {
        self.phase == LoadPhase::Ready
    }
    pub fn pending(&self) -> Option<Ticket> {
        self.pending
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn dirty(&self) -> bool {
        self.dirty
    }
    /// Unsaved edits, including blocked or uncertain writes. This does not authorize a retry.
    pub fn needs_flush(&self) -> bool {
        self.dirty
    }
    pub fn failure(&self) -> Option<Issue> {
        self.failure
    }
    /// The parent must defer exit until true, or explicit discard AND !busy().
    /// Discard cannot cancel admitted blocking IO; keep the GUI alive until its outcome is joined.
    pub fn can_exit(&self) -> bool {
        !self.busy() && !self.dirty && self.failure.is_none()
    }
    pub fn status(&self) -> &'static str {
        if let Some(issue) = self.failure {
            issue.message()
        } else if self.is_loading() {
            "Loading native drafts"
        } else if self.busy() {
            "Saving native drafts"
        } else if self.dirty {
            "Native draft changes pending"
        } else if self.ready() {
            "Native draft recovery ready"
        } else {
            "Native draft recovery not started"
        }
    }

    fn matches(&self, ticket: Ticket, kind: Kind) -> bool {
        ticket.kind == kind && self.pending == Some(ticket)
    }
    fn terminal(&self) -> bool {
        self.failure.is_some_and(Issue::terminal)
    }
    fn disable(&mut self, issue: Issue) {
        self.failure = Some(issue);
        self.protect_all = true;
    }
    fn ticket(&mut self, kind: Kind) -> Option<Ticket> {
        let Some(id) = self.last_id.checked_add(1) else {
            self.disable(Issue::CounterExhausted);
            return None;
        };
        self.last_id = id;
        let ticket = Ticket {
            id,
            revision: self.revision,
            kind,
        };
        self.pending = Some(ticket);
        Some(ticket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str) -> SessionId {
        SessionId::new(id).unwrap()
    }
    fn ready() -> State {
        let mut state = State::default();
        let ticket = state.start_load().unwrap();
        assert!(state.loaded(ticket, Ok(Drafts::new())).is_some());
        state
    }
    fn recovered(id: &str, body: &str) -> Drafts {
        Drafts::from([(session(id), body.to_owned())])
    }
    // Mirrors the required parent merge without giving the controller a second body owner.
    fn merge(state: &State, current: &mut Drafts, loaded: Drafts) {
        for (id, body) in loaded {
            if !state.touched(&id) {
                current.insert(id, body);
            }
        }
    }

    #[test]
    fn initial_read_once_protects_edits_before_and_during_load() {
        let mut state = State::default();
        let edited = session("edited");
        state.touch(&edited);
        assert!(state.next_save().is_none());
        let ticket = state.start_load().unwrap();
        assert!(state.is_loading());
        assert!(state.start_load().is_none());
        assert!(state.reload().is_none());
        assert!(state.next_save().is_none());
        state.touch(&session("during"));
        let drafts = state
            .loaded(ticket, Ok(recovered("edited", "old")))
            .unwrap();
        assert_eq!(drafts.len(), 1);
        assert!(state.touched(&edited));
        assert!(state.touched(&session("during")));
        assert!(state.ready());
        assert!(state.dirty());
        assert!(state.start_load().is_none());
        assert!(state.next_save().is_some());
    }

    #[test]
    fn failed_initial_read_never_enables_a_save_or_implicit_retry() {
        let mut state = State::default();
        state.touch(&session("edited"));
        let ticket = state.start_load().unwrap();
        assert!(state.loaded(ticket, Err(Failure::InvalidData)).is_none());
        assert!(!state.ready());
        assert!(!state.busy());
        assert!(!state.can_exit());
        assert!(state.next_save().is_none());
        assert!(state.start_load().is_none());
        let retry = state.reload().unwrap();
        assert!(state.loaded(retry, Ok(Drafts::new())).is_some());
        assert!(state.touched(&session("edited")));
        assert!(state.dirty());
        assert!(state.failure().is_none());
    }

    #[test]
    fn stale_duplicate_wrong_kind_and_forged_revision_cannot_complete_io() {
        let mut state = State::default();
        let read = state.start_load().unwrap();
        assert!(
            state
                .loaded(
                    Ticket {
                        id: read.id + 1,
                        ..read
                    },
                    Ok(Drafts::new())
                )
                .is_none()
        );
        assert!(
            state
                .loaded(
                    Ticket {
                        revision: read.revision + 1,
                        ..read
                    },
                    Ok(Drafts::new())
                )
                .is_none()
        );
        assert!(!state.saved(read, Ok(())));
        assert_eq!(state.pending(), Some(read));
        assert!(state.loaded(read, Ok(Drafts::new())).is_some());
        state.touch(&session("s"));
        let write = state.next_save().unwrap();
        assert!(state.loaded(read, Ok(recovered("s", "stale"))).is_none());
        assert!(state.loaded(write, Ok(Drafts::new())).is_none());
        assert!(!state.saved(
            Ticket {
                kind: Kind::Load,
                ..write
            },
            Ok(())
        ));
        assert_eq!(state.pending(), Some(write));
        assert!(state.saved(write, Ok(())));
        assert!(!state.saved(write, Err(Failure::Indeterminate)));
        assert!(state.failure().is_none());
        assert!(state.can_exit());
    }

    #[test]
    fn one_operation_and_many_inflight_edits_coalesce_to_latest_snapshot() {
        let mut state = ready();
        let id = session("s");
        state.touch(&id);
        let first = state.next_save().unwrap();
        assert!(state.next_save().is_none());
        assert!(state.reload().is_none());
        for _ in 0..20 {
            state.touch(&id);
        }
        assert!(state.saved(first, Ok(())));
        assert!(state.needs_flush());
        let second = state.next_save().unwrap();
        assert_eq!(second.revision, first.revision + 20);
        assert_eq!(second.id, first.id + 1);
        assert!(!state.saved(first, Ok(())));
        assert_eq!(state.pending(), Some(second));
        assert!(state.saved(second, Ok(())));
        assert!(!state.dirty());
        assert!(state.next_save().is_none());
        assert!(state.can_exit());
    }

    #[test]
    fn parent_owned_send_ack_removal_does_not_clear_a_new_editor_body() {
        let mut state = ready();
        let id = session("s");
        let mut current = recovered("s", "sent body");
        state.touch(&id);
        let sent_revision = state.revision;
        let old_save = state.next_save().unwrap();
        // Parent send ownership uses its captured editor revision, never text equality.
        current.insert(id.clone(), "new body".to_owned());
        state.touch(&id);
        if state.revision == sent_revision {
            current.remove(&id);
            state.touch(&id);
        }
        assert!(state.saved(old_save, Ok(())));
        assert_eq!(current.get(&id).map(String::as_str), Some("new body"));
        assert!(state.dirty());
        let newest = state.next_save().unwrap();
        assert!(state.saved(newest, Ok(())));
        // An actual matching ACK removal is also a mutation and needs its own empty snapshot.
        current.remove(&id);
        state.touch(&id);
        let removed = state.next_save().unwrap();
        assert!(current.is_empty());
        assert!(state.saved(removed, Ok(())));
        let read = state.reload().unwrap();
        let loaded = state.loaded(read, Ok(recovered("s", "sent body"))).unwrap();
        merge(&state, &mut current, loaded);
        assert!(current.is_empty());
    }

    #[test]
    fn every_storage_error_latches_and_reconcile_is_not_a_durability_ack() {
        for failure in [
            Failure::InvalidHome,
            Failure::Unavailable,
            Failure::InvalidData,
            Failure::Limit,
            Failure::Busy,
            Failure::Conflict,
            Failure::NotSaved,
            Failure::Indeterminate,
        ] {
            let mut state = ready();
            state.touch(&session("s"));
            let write = state.next_save().unwrap();
            assert!(state.saved(write, Err(failure)));
            assert_eq!(state.failure(), Some(Issue::Storage(failure)));
            assert!(state.dirty());
            assert!(!state.can_exit());
            assert!(state.next_save().is_none());
            state.touch(&session("s"));
            assert!(state.next_save().is_none());
            let read = state.reload().unwrap();
            assert!(state.next_save().is_none());
            assert!(state.loaded(read, Ok(recovered("s", "on disk"))).is_some());
            assert!(state.failure().is_none());
            assert!(state.dirty());
            assert!(!state.can_exit());
            assert!(state.next_save().is_some());
        }
    }

    #[test]
    fn explicit_reconcile_never_replaces_edited_body_or_during_read_edits() {
        let mut state = ready();
        let mut current = recovered("s", "new body");
        state.touch(&session("s"));
        let write = state.next_save().unwrap();
        state.saved(write, Err(Failure::Indeterminate));
        let read = state.reload().unwrap();
        current.insert(session("other"), "during read".to_owned());
        state.touch(&session("other"));
        let mut disk = recovered("s", "old body");
        disk.insert(session("other"), "older".to_owned());
        disk.insert(session("untouched"), "recover me".to_owned());
        let loaded = state.loaded(read, Ok(disk)).unwrap();
        merge(&state, &mut current, loaded);
        assert_eq!(
            current.get(&session("s")).map(String::as_str),
            Some("new body")
        );
        assert_eq!(
            current.get(&session("other")).map(String::as_str),
            Some("during read")
        );
        assert_eq!(
            current.get(&session("untouched")).map(String::as_str),
            Some("recover me")
        );
        assert!(state.dirty());
    }

    #[test]
    fn revision_exhaustion_saturates_and_join_cannot_clear_terminal_failure() {
        let mut state = ready();
        state.revision = u64::MAX - 1;
        let id = session("s");
        assert!(state.touch(&id));
        let ticket = state.next_save().unwrap();
        assert_eq!(ticket.revision, u64::MAX);
        assert!(!state.touch(&id));
        assert_eq!(state.revision, u64::MAX);
        assert_eq!(state.failure(), Some(Issue::CounterExhausted));
        assert!(!state.can_exit());
        assert!(state.busy());
        assert!(state.touched(&session("never edited")));
        assert!(state.saved(ticket, Ok(())));
        assert!(!state.busy());
        assert!(state.dirty());
        assert_eq!(state.failure(), Some(Issue::CounterExhausted));
        assert!(state.reload().is_none());
        assert!(state.next_save().is_none());
    }

    #[test]
    fn ticket_exhaustion_cannot_wrap_or_admit_io() {
        let mut state = ready();
        state.last_id = u64::MAX;
        state.touch(&session("s"));
        assert!(state.next_save().is_none());
        assert!(!state.busy());
        assert_eq!(state.last_id, u64::MAX);
        assert_eq!(state.failure(), Some(Issue::CounterExhausted));
        assert!(state.touched(&session("other")));
        assert!(state.reload().is_none());
        assert!(state.dirty());
    }

    #[test]
    fn touched_capacity_is_bounded_and_never_evicts_or_merges_after_overflow() {
        let mut state = State::default();
        let read = state.start_load().unwrap();
        for index in 0..MAX_DRAFTS {
            assert!(state.touch(&session(&format!("s{index}"))));
        }
        assert!(state.touch(&session("s0")));
        assert_eq!(state.touched.len(), MAX_DRAFTS);
        assert!(!state.touch(&session("overflow")));
        assert_eq!(state.touched.len(), MAX_DRAFTS);
        assert_eq!(state.failure(), Some(Issue::TouchLimit));
        assert!(state.touched(&session("unseen")));
        assert!(state.busy());
        assert!(state.loaded(read, Ok(recovered("unseen", "old"))).is_none());
        assert!(!state.busy());
        assert!(!state.ready());
        assert!(state.dirty());
        assert!(state.reload().is_none());
        assert!(state.next_save().is_none());
    }

    #[test]
    fn shutdown_waits_for_join_even_without_edits_and_failed_reload_stays_blocked() {
        let mut state = State::default();
        assert!(state.can_exit());
        let read = state.start_load().unwrap();
        assert!(!state.can_exit());
        state.apply(Outcome::Loaded {
            ticket: read,
            result: Ok(Drafts::new()),
        });
        assert!(state.can_exit());
        let reload = state.reload().unwrap();
        assert!(!state.can_exit());
        state.loaded(reload, Err(Failure::Unavailable));
        assert!(!state.can_exit());
        assert!(!state.busy());
        assert!(!state.dirty());
        assert!(state.next_save().is_none());
    }

    #[test]
    fn outcome_dispatch_does_not_accept_wrong_operation_or_ack_newer_revision() {
        let mut state = ready();
        state.touch(&session("s"));
        let write = state.next_save().unwrap();
        assert!(
            state
                .apply(Outcome::Loaded {
                    ticket: write,
                    result: Ok(Drafts::new())
                })
                .is_none()
        );
        assert_eq!(state.pending(), Some(write));
        state.touch(&session("s"));
        state.apply(Outcome::Saved {
            ticket: write,
            result: Ok(()),
        });
        assert!(!state.busy());
        assert!(state.dirty());
        assert!(state.failure().is_none());
    }
}
