//! Operator-reviewed Session fork. A fork creates a Session, so the child id is
//! authoritative only from the ACK: this machine never fabricates a roster entry,
//! never predicts an id, never retries, and never claims a rollback for a child the
//! Host may already have created. Revealing/selecting the child stays an explicit
//! App decision over that App's own authoritative roster or follow frames.
use crate::management::{Ticket, View};
use dsh_native_transport::dto::SessionId;

/// One admitted fork attempt, fenced to the reviewed source at its transport epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub ticket: Ticket,
    pub attempt: u64,
}
/// The reviewed, not-yet-submitted source. No request field is retained here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Review {
    pub ticket: Ticket,
}
/// Redacted failure class the worker derives from the transport error. It carries no
/// remote message and no child id, so it can never publish unacknowledged state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// `session/fork-unavailable`: the Host refused before creating any child.
    Unavailable,
    /// `session/workspace-attach-failed`: a child exists but was not attached.
    Unattached,
    /// Any other refusal, timeout or disconnect; child creation is unknown.
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The durable ACK: the exact new child Session identity, and its only authority.
    Forked {
        child: SessionId,
    },
    Unavailable,
    Unattached,
    /// The command never left the native queue.
    NotSent,
    /// Admitted, but the outcome is unknown; reconcile explicitly and never retry.
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    None,
    Pending,
    /// The Host-acknowledged child. `selected` is true only when the App's own
    /// authoritative roster or follow frames already contained it.
    Forked {
        child: SessionId,
        selected: bool,
    },
    Unavailable,
    Unattached,
    NotSent,
    Indeterminate,
}
/// The acknowledged child, returned only for the exact pending receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    pub source: SessionId,
    pub child: SessionId,
}
/// Cloning an input is not permission to submit it. Every admission checks its current ticket.
#[derive(Clone)]
pub enum Action {
    /// Open the one-review fork panel for the current source.
    Open {
        view: View,
    },
    Close(Ticket),
    Review(Ticket),
    Back(Ticket),
    Confirm(Review),
}
pub struct Context {
    pub view: Option<View>,
    pub allowed: bool,
}
/// One consuming operator-reviewed fork. The reviewed source is not retained in the submission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submission {
    pub receipt: Receipt,
    source: SessionId,
}
impl Submission {
    pub fn source(&self) -> &SessionId {
        &self.source
    }
}
struct Panel {
    ticket: Ticket,
}

pub struct Fork {
    panel: Option<Panel>,
    review: Option<Review>,
    pending: Option<Receipt>,
    notice: Notice,
    serial: u64,
    attempt: u64,
    exhausted: bool,
}
impl Default for Fork {
    fn default() -> Self {
        Self {
            panel: None,
            review: None,
            pending: None,
            notice: Notice::None,
            serial: 0,
            attempt: 0,
            exhausted: false,
        }
    }
}
impl Fork {
    pub fn ticket(&self) -> Option<Ticket> {
        self.panel.as_ref().map(|panel| panel.ticket.clone())
    }
    pub fn review(&self) -> Option<&Review> {
        self.review.as_ref()
    }
    pub fn pending(&self) -> Option<&Receipt> {
        self.pending.as_ref()
    }
    pub fn notice(&self) -> &Notice {
        &self.notice
    }
    pub fn is_open(&self) -> bool {
        self.panel.is_some()
    }
    /// Hide and clear the panel without cancelling or rolling back an admitted fork.
    pub fn dismiss(&mut self) {
        self.panel = None;
        self.review = None;
        self.notice = Notice::None;
    }
    /// Fence late receipts. The Host may already have created the child.
    pub fn disconnect(&mut self) {
        let admitted = self.pending.take().is_some();
        self.dismiss();
        if admitted {
            self.notice = Notice::Indeterminate;
        }
    }
    /// Record whether the App's own frames contained the acknowledged child. Only the
    /// acknowledged notice is amended; a foreign or retired notice is left alone.
    pub fn reveal(&mut self, selected: bool) {
        if let Notice::Forked { child, .. } = &self.notice {
            self.notice = Notice::Forked {
                child: child.clone(),
                selected,
            };
        }
    }
    fn exhausted(&mut self) {
        self.exhausted = true;
        self.disconnect();
    }
    fn current(&self, ticket: &Ticket, context: &Context) -> bool {
        !self.exhausted
            && context.allowed
            && self
                .panel
                .as_ref()
                .is_some_and(|panel| &panel.ticket == ticket)
            && context.view.as_ref().is_some_and(|view| {
                view.epoch == ticket.epoch
                    && view.generation == ticket.generation
                    && view.target == ticket.target
            })
    }
    pub fn apply(&mut self, action: Action, context: &Context) -> Option<Submission> {
        match action {
            Action::Close(ticket) if self.ticket().as_ref() == Some(&ticket) => self.dismiss(),
            Action::Open { view }
                if !self.exhausted && context.allowed && context.view.as_ref() == Some(&view) =>
            {
                let Some(serial) = self.serial.checked_add(1) else {
                    self.exhausted();
                    return None;
                };
                self.serial = serial;
                self.dismiss();
                self.panel = Some(Panel {
                    ticket: Ticket {
                        epoch: view.epoch,
                        generation: view.generation,
                        serial,
                        target: view.target,
                    },
                });
                self.notice = if self.pending.is_some() {
                    Notice::Pending
                } else {
                    Notice::None
                };
            }
            Action::Back(ticket) if self.current(&ticket, context) && self.pending.is_none() => {
                self.review = None;
            }
            Action::Review(ticket) if self.current(&ticket, context) && self.pending.is_none() => {
                self.review = Some(Review { ticket });
            }
            Action::Confirm(review)
                if self.current(&review.ticket, context)
                    && self.pending.is_none()
                    && self.review.as_ref() == Some(&review) =>
            {
                let Some(attempt) = self.attempt.checked_add(1) else {
                    self.exhausted();
                    return None;
                };
                let source = review.ticket.target.clone();
                self.review = None;
                self.attempt = attempt;
                let receipt = Receipt {
                    ticket: review.ticket,
                    attempt,
                };
                self.pending = Some(receipt.clone());
                self.notice = Notice::Pending;
                return Some(Submission { receipt, source });
            }
            _ => {}
        }
        None
    }
    /// Retire only the exact pending receipt. A malformed or self-referential ACK is
    /// never installed as a child.
    pub fn finish(&mut self, receipt: &Receipt, outcome: Outcome) -> Option<Applied> {
        if self.pending.as_ref() != Some(receipt) {
            return None;
        }
        self.pending = None;
        let mut applied = None;
        let notice = match outcome {
            Outcome::Forked { child } if child != receipt.ticket.target => {
                applied = Some(Applied {
                    source: receipt.ticket.target.clone(),
                    child: child.clone(),
                });
                Notice::Forked {
                    child,
                    selected: false,
                }
            }
            // A child equal to its own source cannot be a real fork result.
            Outcome::Forked { .. } => Notice::Indeterminate,
            Outcome::Unavailable => Notice::Unavailable,
            Outcome::Unattached => Notice::Unattached,
            Outcome::NotSent => Notice::NotSent,
            Outcome::Unknown => Notice::Indeterminate,
        };
        if self
            .panel
            .as_ref()
            .is_some_and(|panel| panel.ticket == receipt.ticket)
        {
            self.notice = notice;
        } else if self.notice == Notice::Pending {
            self.notice = Notice::None;
        }
        applied
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
            view: Some(View {
                epoch: 1,
                generation: 7,
                target: id("PUBLIC-fork-source"),
            }),
            allowed: true,
        }
    }
    fn opened() -> Fork {
        let mut fork = Fork::default();
        fork.apply(
            Action::Open {
                view: context().view.unwrap(),
            },
            &context(),
        );
        fork
    }
    fn review(fork: &mut Fork) -> Review {
        let ticket = fork.ticket().unwrap();
        assert!(fork.apply(Action::Review(ticket), &context()).is_none());
        fork.review().unwrap().clone()
    }
    fn submit(fork: &mut Fork) -> Submission {
        let review = review(fork);
        fork.apply(Action::Confirm(review), &context())
            .expect("reviewed public fork")
    }

    #[test]
    fn open_requires_review_and_close_changes_nothing() {
        let mut fork = opened();
        assert_eq!(fork.notice(), &Notice::None);
        let ticket = fork.ticket().unwrap();
        assert!(fork.apply(Action::Close(ticket), &context()).is_none());
        assert!(!fork.is_open());
        assert!(fork.review().is_none());
        assert!(fork.pending().is_none());
        assert_eq!(fork.notice(), &Notice::None);
        // A confirm without the reviewed step is refused.
        let mut fork = opened();
        let forged = Review {
            ticket: fork.ticket().unwrap(),
        };
        assert!(fork.apply(Action::Confirm(forged), &context()).is_none());
        assert!(fork.pending().is_none());
    }

    #[test]
    fn review_is_required_and_confirm_consumes_one_exact_source() {
        let mut fork = opened();
        let reviewed = review(&mut fork);
        let replay = Action::Confirm(reviewed.clone());
        let submission = fork.apply(replay.clone(), &context()).unwrap();
        assert_eq!(submission.source(), &id("PUBLIC-fork-source"));
        assert_eq!(submission.receipt.ticket.target, id("PUBLIC-fork-source"));
        assert!(fork.review().is_none());
        assert_eq!(fork.pending(), Some(&submission.receipt));
        assert_eq!(fork.notice(), &Notice::Pending);
        // A second fork is refused while this one is in flight.
        assert!(fork.apply(replay, &context()).is_none());
        assert!(fork.pending().is_some());
    }

    #[test]
    fn back_retires_the_review_without_submitting() {
        let mut fork = opened();
        let review = review(&mut fork);
        let ticket = fork.ticket().unwrap();
        assert!(fork.apply(Action::Back(ticket), &context()).is_none());
        assert!(fork.review().is_none());
        assert!(fork.pending().is_none());
        // A retired review cannot be replayed into a write.
        assert!(fork.apply(Action::Confirm(review), &context()).is_none());
        assert_eq!(fork.notice(), &Notice::None);
    }

    #[test]
    fn acknowledged_child_is_returned_once_and_never_locally_predicted() {
        let mut fork = opened();
        let submission = submit(&mut fork);
        assert!(fork.pending().is_some());
        let receipt = submission.receipt.clone();
        let applied = fork
            .finish(
                &receipt,
                Outcome::Forked {
                    child: id("PUBLIC-fork-child"),
                },
            )
            .unwrap();
        assert_eq!(applied.source, id("PUBLIC-fork-source"));
        assert_eq!(applied.child, id("PUBLIC-fork-child"));
        assert_eq!(
            fork.notice(),
            &Notice::Forked {
                child: id("PUBLIC-fork-child"),
                selected: false
            }
        );
        // The App may record that its own frames already contained the child.
        fork.reveal(true);
        assert_eq!(
            fork.notice(),
            &Notice::Forked {
                child: id("PUBLIC-fork-child"),
                selected: true
            }
        );
        assert!(fork.pending().is_none());
        // A duplicate ACK for the retired receipt changes nothing.
        assert!(
            fork.finish(
                &receipt,
                Outcome::Forked {
                    child: id("PUBLIC-forged-child")
                }
            )
            .is_none()
        );
        assert_eq!(
            fork.notice(),
            &Notice::Forked {
                child: id("PUBLIC-fork-child"),
                selected: true
            }
        );
    }

    #[test]
    fn a_child_equal_to_its_own_source_is_never_installed() {
        let mut fork = opened();
        let submission = submit(&mut fork);
        assert!(
            fork.finish(
                &submission.receipt,
                Outcome::Forked {
                    child: id("PUBLIC-fork-source")
                }
            )
            .is_none()
        );
        assert_eq!(fork.notice(), &Notice::Indeterminate);
        assert!(fork.pending().is_none());
    }

    #[test]
    fn stale_target_epoch_generation_and_denied_context_refuse_without_writing() {
        let mutations: [fn(&mut View); 3] = [
            |view| view.epoch += 1,
            |view| view.generation += 1,
            |view| view.target = id("PUBLIC-other-source"),
        ];
        for mutate in mutations {
            let mut fork = opened();
            let review = review(&mut fork);
            let mut stale = context();
            mutate(stale.view.as_mut().unwrap());
            assert!(fork.apply(Action::Confirm(review), &stale).is_none());
            assert!(fork.pending().is_none());
        }
        let mut fork = opened();
        let denied = Context {
            view: context().view,
            allowed: false,
        };
        let ticket = fork.ticket().unwrap();
        assert!(
            fork.apply(Action::Review(ticket.clone()), &denied)
                .is_none()
        );
        assert!(fork.review().is_none());
        assert!(
            fork.apply(Action::Confirm(Review { ticket }), &denied)
                .is_none()
        );
        assert!(fork.pending().is_none());
    }

    #[test]
    fn unavailable_unattached_and_unknown_outcomes_stay_distinct_and_never_retry() {
        for (outcome, expected) in [
            (Outcome::Unavailable, Notice::Unavailable),
            (Outcome::Unattached, Notice::Unattached),
            (Outcome::Unknown, Notice::Indeterminate),
            (Outcome::NotSent, Notice::NotSent),
        ] {
            let mut fork = opened();
            let submission = submit(&mut fork);
            assert!(fork.finish(&submission.receipt, outcome).is_none());
            assert_eq!(fork.notice(), &expected);
            assert!(fork.pending().is_none());
            // The consumed review is not re-armed: another Confirm needs a new review.
            let stale = Review {
                ticket: submission.receipt.ticket.clone(),
            };
            assert!(fork.apply(Action::Confirm(stale), &context()).is_none());
            assert!(fork.pending().is_none());
        }
    }

    #[test]
    fn disconnect_fences_late_possessive_acks_without_claiming_rollback() {
        let mut fork = opened();
        let submission = submit(&mut fork);
        fork.disconnect();
        assert!(fork.pending().is_none());
        assert_eq!(fork.notice(), &Notice::Indeterminate);
        assert!(
            fork.finish(
                &submission.receipt,
                Outcome::Forked {
                    child: id("PUBLIC-late-child")
                }
            )
            .is_none()
        );
    }
}
