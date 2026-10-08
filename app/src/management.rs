//! Operator-only registry mutations. Stream state is authoritative; HTTP ACKs never install sets.
use dsh_native_transport::dto::SessionId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Pin,
    Unpin,
    Archive,
    Restore,
}
impl Operation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pin => "Pin",
            Self::Unpin => "Unpin",
            Self::Archive => "Archive",
            Self::Restore => "Restore",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub epoch: u64,
    pub generation: u64,
    pub target: SessionId,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub epoch: u64,
    pub generation: u64,
    pub serial: u64,
    pub target: SessionId,
}
impl Ticket {
    fn view(&self) -> View {
        View {
            epoch: self.epoch,
            generation: self.generation,
            target: self.target.clone(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submission {
    pub ticket: Ticket,
    pub operation: Operation,
}
#[derive(Clone, Debug)]
pub enum Action {
    Open(View),
    Close(Ticket),
    Review {
        ticket: Ticket,
        operation: Operation,
    },
    Confirm(Ticket),
}
pub struct Context {
    pub view: Option<View>,
    pub allowed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    None,
    ReviewChanged,
    Pending,
    Confirmed,
    NotSent,
    Indeterminate,
}
struct Panel {
    view: View,
    lifetime: u64,
}
#[derive(Default)]
pub struct Registry {
    ready: bool,
    serial: u64,
    archived: BTreeSet<SessionId>,
    pinned: Vec<SessionId>,
    pin_ranks: BTreeMap<SessionId, usize>,
}
impl Registry {
    // Bounded response/frame bytes are also enforced by transport. Reject malformed snapshots,
    // rather than silently treating omitted/truncated registry membership as an unpin/restore.
    fn valid(ids: &[SessionId]) -> bool {
        ids.len() <= 8192 && ids.iter().collect::<BTreeSet<_>>().len() == ids.len()
    }
    /// Full authoritative Host pin order, most recently pinned first.
    pub fn ordered_pins(&self) -> &[SessionId] {
        &self.pinned
    }
    pub fn ready(&self) -> bool {
        self.ready
    }
    pub fn archived(&self, id: &SessionId) -> bool {
        self.archived.contains(id)
    }
    pub fn pin_rank(&self, id: &SessionId) -> Option<usize> {
        if self.archived(id) {
            None
        } else {
            self.pin_ranks.get(id).copied()
        }
    }
    pub fn pinned(&self, id: &SessionId) -> bool {
        self.pin_rank(id).is_some()
    }
    fn bump(&mut self) {
        self.serial = self.serial.saturating_add(1);
    }
    fn baseline(&mut self, archived: Vec<SessionId>, pinned: Vec<SessionId>) -> bool {
        self.bump();
        self.ready = Self::valid(&archived) && Self::valid(&pinned);
        if self.ready {
            self.archived = archived.into_iter().collect();
            self.set_pins(pinned);
        }
        self.ready
    }
    fn archives(&mut self, ids: Vec<SessionId>) -> bool {
        self.bump();
        self.ready = self.ready && Self::valid(&ids);
        if self.ready {
            self.archived = ids.into_iter().collect();
        }
        self.ready
    }
    fn set_pins(&mut self, pins: Vec<SessionId>) {
        // Cache once per full stream update, not an O(pin-count) scan per sort comparison.
        self.pin_ranks = pins
            .iter()
            .cloned()
            .enumerate()
            .map(|(rank, id)| (id, rank))
            .collect();
        self.pinned = pins;
    }
    fn pins(&mut self, ids: Vec<SessionId>) -> bool {
        self.bump();
        self.ready = self.ready && Self::valid(&ids);
        if self.ready {
            self.set_pins(ids);
        }
        self.ready
    }
    fn allows(&self, operation: Operation, target: &SessionId) -> bool {
        self.ready
            && match operation {
                Operation::Pin => !self.archived(target) && !self.pinned(target),
                Operation::Unpin => !self.archived(target) && self.pinned(target),
                Operation::Archive => !self.archived(target),
                Operation::Restore => self.archived(target),
            }
    }
}
pub struct Management {
    pub registry: Registry,
    serial: u64,
    panel: Option<Panel>,
    review: Option<(Submission, u64)>,
    pending: Option<(Submission, u64)>,
    notice: Notice,
}
impl Default for Management {
    fn default() -> Self {
        Self {
            registry: Registry::default(),
            serial: 0,
            panel: None,
            review: None,
            pending: None,
            notice: Notice::None,
        }
    }
}
impl Management {
    fn bump(&mut self) {
        self.serial = self.serial.saturating_add(1);
    }
    pub fn is_open(&self) -> bool {
        self.panel.is_some()
    }
    pub fn target(&self) -> Option<&SessionId> {
        self.panel.as_ref().map(|p| &p.view.target)
    }
    pub fn ticket(&self) -> Option<Ticket> {
        self.panel.as_ref().map(|p| Ticket {
            epoch: p.view.epoch,
            generation: p.view.generation,
            serial: self.serial,
            target: p.view.target.clone(),
        })
    }
    pub fn review(&self) -> Option<&Submission> {
        self.review.as_ref().map(|(s, _)| s)
    }
    pub fn notice(&self) -> Notice {
        self.notice
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn allows(&self, op: Operation) -> bool {
        !self.pending() && self.target().is_some_and(|id| self.registry.allows(op, id))
    }
    pub fn dismiss(&mut self) {
        self.bump();
        self.panel = None;
        self.review = None;
        self.notice = Notice::None;
        // Closing hides controls. It does NOT cancel or roll back an admitted registry write.
    }
    pub fn disconnect(&mut self) {
        self.dismiss();
        self.pending = None;
        self.registry.ready = false;
        self.registry.bump();
    }
    fn stream_changed(&mut self) {
        if self.review.take().is_some() {
            self.bump();
            self.notice = Notice::ReviewChanged;
        }
    }
    pub fn baseline(&mut self, archives: Vec<SessionId>, pins: Vec<SessionId>) -> bool {
        let valid = self.registry.baseline(archives, pins);
        self.stream_changed();
        valid
    }
    pub fn archives(&mut self, ids: Vec<SessionId>) -> bool {
        let valid = self.registry.archives(ids);
        self.stream_changed();
        valid
    }
    pub fn pins(&mut self, ids: Vec<SessionId>) -> bool {
        let valid = self.registry.pins(ids);
        self.stream_changed();
        valid
    }
    pub fn update(&mut self, action: Action, context: &Context) -> Option<Submission> {
        match action {
            Action::Close(ticket) if self.ticket().as_ref() == Some(&ticket) => self.dismiss(),
            Action::Open(view)
                if context.allowed
                    && context.view.as_ref() == Some(&view)
                    && self.registry.ready() =>
            {
                self.dismiss();
                self.panel = Some(Panel {
                    view,
                    lifetime: self.serial,
                });
                self.notice = if self.pending() {
                    Notice::Pending
                } else {
                    Notice::None
                };
            }
            Action::Review { ticket, operation }
                if context.allowed
                    && context.view.as_ref() == Some(&ticket.view())
                    && self.ticket().as_ref() == Some(&ticket)
                    && self.allows(operation) =>
            {
                self.bump();
                let submission = Submission {
                    ticket: self.ticket().unwrap(),
                    operation,
                };
                self.review = Some((submission, self.registry.serial));
                self.notice = Notice::None;
            }
            Action::Confirm(ticket)
                if context.allowed
                    && context.view.as_ref() == Some(&ticket.view())
                    && self.ticket().as_ref() == Some(&ticket) =>
            {
                if let Some((submission, revision)) = self.review.as_ref() {
                    if submission.ticket == ticket
                        && *revision == self.registry.serial
                        && self.allows(submission.operation)
                    {
                        let submission = submission.clone();
                        self.pending =
                            Some((submission.clone(), self.panel.as_ref().unwrap().lifetime));
                        self.review = None;
                        self.bump();
                        self.notice = Notice::Pending;
                        return Some(submission);
                    }
                }
            }
            _ => {}
        }
        None
    }
    pub fn finish(&mut self, submission: &Submission, result: Result<(), ()>, not_sent: bool) {
        if self.pending.as_ref().is_none_or(|(s, _)| s != submission) {
            return;
        }
        let (_, lifetime) = self.pending.take().unwrap();
        if self
            .panel
            .as_ref()
            .is_some_and(|p| p.lifetime == lifetime && p.view == submission.ticket.view())
        {
            self.notice = if not_sent {
                Notice::NotSent
            } else if result.is_ok() {
                Notice::Confirmed
            } else {
                Notice::Indeterminate
            };
        } else if self.notice == Notice::Pending {
            self.notice = Notice::None;
        }
        // Even a successful RPC's full sets could precede a newer stream frame: never install them.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(value: &str) -> SessionId {
        SessionId::new(value).unwrap()
    }
    fn context() -> Context {
        Context {
            allowed: true,
            view: Some(View {
                epoch: 1,
                generation: 7,
                target: id("a"),
            }),
        }
    }
    fn opened() -> Management {
        let mut m = Management::default();
        m.baseline(vec![], vec![]);
        m.update(Action::Open(context().view.unwrap()), &context());
        m
    }
    fn submit(m: &mut Management, op: Operation) -> Submission {
        m.update(
            Action::Review {
                ticket: m.ticket().unwrap(),
                operation: op,
            },
            &context(),
        );
        m.update(Action::Confirm(m.ticket().unwrap()), &context())
            .unwrap()
    }
    #[test]
    fn ordered_full_sets_replace_and_archive_masks_pin_until_coupled_frame() {
        let mut m = opened();
        m.pins(vec![id("b"), id("a")]);
        assert_eq!(m.registry.pin_rank(&id("b")), Some(0));
        assert_eq!(m.registry.pin_rank(&id("a")), Some(1));
        m.archives(vec![id("b")]);
        assert!(!m.registry.pinned(&id("b")));
        m.pins(vec![id("a")]);
        m.archives(vec![]);
        assert!(!m.registry.pinned(&id("b")));
        m.pins(vec![]);
        assert!(!m.registry.pinned(&id("a")));
    }
    #[test]
    fn stream_update_invalidates_review_and_stale_confirmation() {
        let mut m = opened();
        m.update(
            Action::Review {
                ticket: m.ticket().unwrap(),
                operation: Operation::Archive,
            },
            &context(),
        );
        let ticket = m.ticket().unwrap();
        m.pins(vec![id("b")]);
        assert!(m.update(Action::Confirm(ticket), &context()).is_none());
        assert_eq!(m.notice(), Notice::ReviewChanged);
    }
    #[test]
    fn acknowledgment_never_installs_state_or_implies_live_registry_observation() {
        let mut m = opened();
        let s = submit(&mut m, Operation::Pin);
        assert!(!m.registry.pinned(&id("a")));
        m.finish(&s, Ok(()), false);
        assert_eq!(m.notice(), Notice::Confirmed);
        assert!(!m.registry.pinned(&id("a")));
        m.pins(vec![id("b"), id("a")]);
        assert_eq!(m.registry.pin_rank(&id("a")), Some(1));
    }
    #[test]
    fn old_target_generation_epoch_serial_and_queued_double_confirm_refuse() {
        let mut m = opened();
        let stale = m.ticket().unwrap();
        let s = submit(&mut m, Operation::Archive);
        assert!(m.update(Action::Confirm(stale), &context()).is_none());
        let mut wrong = s.clone();
        wrong.ticket.serial += 1;
        m.finish(&wrong, Ok(()), false);
        assert!(m.pending());
        m.dismiss();
        m.update(Action::Open(context().view.unwrap()), &context());
        m.finish(&s, Ok(()), false);
        assert_eq!(m.notice(), Notice::None);
        assert!(!m.pending());
        let t = m.ticket().unwrap();
        let mut c = context();
        c.view.as_mut().unwrap().generation += 1;
        assert!(
            m.update(
                Action::Review {
                    ticket: t.clone(),
                    operation: Operation::Pin
                },
                &c
            )
            .is_none()
        );
        assert!(m.review().is_none());
        c = context();
        c.view.as_mut().unwrap().epoch += 1;
        m.update(
            Action::Review {
                ticket: t.clone(),
                operation: Operation::Pin,
            },
            &c,
        );
        assert!(m.review().is_none());
        c = context();
        c.view.as_mut().unwrap().target = id("b");
        m.update(
            Action::Review {
                ticket: t,
                operation: Operation::Pin,
            },
            &c,
        );
        assert!(m.review().is_none());
    }
    #[test]
    fn smoke_or_lifecycle_context_never_submits_and_disconnect_rejects_late_ack() {
        let mut m = opened();
        let s = submit(&mut m, Operation::Pin);
        m.disconnect();
        m.finish(&s, Ok(()), false);
        assert!(!m.pending());
        assert!(!m.registry.ready());
        let mut m = opened();
        let c = Context {
            allowed: false,
            ..context()
        };
        m.update(
            Action::Review {
                ticket: m.ticket().unwrap(),
                operation: Operation::Pin,
            },
            &c,
        );
        assert!(m.review().is_none());
        assert!(m.update(Action::Confirm(m.ticket().unwrap()), &c).is_none());
    }
    #[test]
    fn no_baseline_oversized_or_duplicate_snapshots_fail_closed() {
        let mut m = Management::default();
        assert!(!m.pins(vec![id("a")]));
        assert!(!m.baseline(vec![], vec![id("a"), id("a")]));
        assert!(!m.registry.ready());
        assert!(!m.baseline(vec![id("a"); 8193], vec![]));
        assert!(!m.registry.ready());
        assert!(m.baseline(vec![], vec![]));
    }
    #[test]
    fn failure_not_sent_and_restore_remain_explicit_and_never_autoretry() {
        let mut m = opened();
        let s = submit(&mut m, Operation::Archive);
        m.finish(&s, Err(()), false);
        assert_eq!(m.notice(), Notice::Indeterminate);
        assert!(!m.registry.archived(&id("a")));
        assert!(!m.pending());
        let s = submit(&mut m, Operation::Archive);
        m.finish(&s, Err(()), true);
        assert_eq!(m.notice(), Notice::NotSent);
        m.archives(vec![id("a")]);
        assert!(!m.allows(Operation::Archive));
        assert!(m.allows(Operation::Restore));
        let s = submit(&mut m, Operation::Restore);
        m.finish(&s, Ok(()), false);
        assert!(m.registry.archived(&id("a")));
        m.archives(vec![]);
        assert!(!m.registry.archived(&id("a")));
        assert!(!m.registry.pinned(&id("a")));
    }
}
