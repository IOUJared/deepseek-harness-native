//! Cloudflare owns the full async auth span; cancellation has a reserved lane.
use super::{Backend, Event, MAX_OPERATIONS, TRANSPORT_EPOCH};
use crate::settings::{
    Ticket,
    cloudflare::{Effect, Outcome},
};
use dsh_native_core::CloudflareAuthStatus;
#[derive(Default)]
pub(super) struct Coordinator {
    flight: Option<Ticket>,
    cancel: Option<Ticket>,
    deferred_cancel: bool,
    metadata: Option<CloudflareAuthStatus>,
    uncertain: bool,
    start_baseline: Option<(u64, u64, Option<String>)>,
}
impl Coordinator {
    pub(super) fn busy(&self) -> bool {
        self.flight.is_some()
            || self.cancel.is_some()
            || self.uncertain
            || self
                .metadata
                .as_ref()
                .is_some_and(|m| m.phase.active() || m.retry_blocked)
    }
    pub(super) fn admit(
        &mut self,
        effect: &Effect,
        smoke: bool,
        stopping: bool,
        operations: usize,
        conflicting: bool,
    ) -> bool {
        if smoke || stopping || effect.ticket().epoch != TRANSPORT_EPOCH {
            return false;
        }
        if matches!(effect, Effect::CancelLogin { .. }) {
            if self.cancel.is_some() {
                return false;
            }
            self.cancel = Some(effect.ticket());
            return true;
        }
        if self.flight.is_some()
            || self.cancel.is_some()
            || operations >= MAX_OPERATIONS
            || conflicting
        {
            return false;
        }
        let recovery = matches!(
            effect,
            Effect::Setup { .. } | Effect::AuthMode { .. } | Effect::Disconnect { .. }
        );
        if !matches!(effect, Effect::Read(_) | Effect::OpenBrowser { .. })
            && self.busy()
            && !(recovery
                && !self.uncertain
                && !self.metadata.as_ref().is_some_and(|m| m.phase.active()))
        {
            return false;
        }
        if matches!(effect, Effect::Start { .. }) {
            self.start_baseline = self
                .metadata
                .as_ref()
                .map(|m| (m.auth_epoch, m.sequence, m.attempt_id.clone()));
        }
        self.flight = Some(effect.ticket());
        true
    }
    pub(super) fn observe(&mut self, status: CloudflareAuthStatus) {
        if self.metadata.as_ref().is_some_and(|old| {
            (status.auth_epoch, status.sequence) < (old.auth_epoch, old.sequence)
                || old.attempt_id == status.attempt_id
                    && !old.phase.active()
                    && old.phase != dsh_native_core::CloudflareAuthPhase::Idle
                    && status.phase.active()
        }) {
            return;
        }
        self.metadata = Some(status);
    }
    pub(super) fn defer_cancel(&mut self, effect: &Effect) -> bool {
        if matches!(effect, Effect::CancelLogin { attempt: None, .. }) {
            self.deferred_cancel = true;
            true
        } else {
            false
        }
    }
    fn owned_cancel_attempt(&self) -> Option<String> {
        let status = self.metadata.as_ref()?;
        if self
            .start_baseline
            .as_ref()
            .is_some_and(|(epoch, sequence, attempt)| {
                (status.auth_epoch, status.sequence) <= (*epoch, *sequence)
                    || &status.attempt_id == attempt
            })
        {
            return None;
        }
        status.attempt_id.clone()
    }
    pub(super) fn next_cancel(&mut self) -> Option<Effect> {
        if !self.deferred_cancel {
            return None;
        }
        let attempt = self.owned_cancel_attempt()?;
        self.deferred_cancel = false;
        Some(Effect::CancelLogin {
            ticket: self.cancel?,
            attempt: Some(attempt),
        })
    }
    pub(super) fn settle(&mut self, ticket: Ticket, outcome: &Outcome) {
        if self.flight == Some(ticket) {
            self.flight = None;
        }
        if self.cancel == Some(ticket) {
            self.cancel = None;
            self.deferred_cancel = false;
        }
        match outcome {
            Outcome::Snapshot(snapshot) => {
                self.observe(snapshot.auth_status.clone());
                self.uncertain = false;
            }
            Outcome::Unknown => self.uncertain = true,
            _ => {}
        }
    }
    pub(super) fn unsent_cancel(&mut self) -> Option<(Ticket, bool)> {
        if self.deferred_cancel && self.flight.is_none() && self.owned_cancel_attempt().is_none() {
            self.deferred_cancel = false;
            self.cancel.take().map(|ticket| (ticket, self.uncertain))
        } else {
            None
        }
    }
}
pub(super) fn dispatch_cancel(
    owner: &mut Coordinator,
    backend: &std::sync::Arc<Backend>,
    tasks: &mut tokio::task::JoinSet<Event>,
) {
    if let Some(effect) = owner.next_cancel() {
        let backend = backend.clone();
        let ticket = effect.ticket();
        tasks.spawn_blocking(move || Event::CloudflareCompleted {
            ticket,
            outcome: super::execute_cloudflare(&backend, effect),
        });
    } else if let Some((ticket, uncertain)) = owner.unsent_cancel() {
        tasks.spawn(async move {
            Event::CloudflareCompleted {
                ticket,
                outcome: if uncertain {
                    Outcome::Unknown
                } else {
                    Outcome::NotSent
                },
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn ticket(serial: u64) -> Ticket {
        Ticket {
            epoch: TRANSPORT_EPOCH,
            panel: 1,
            serial,
        }
    }
    #[test]
    fn cancellation_has_reserved_capacity_and_waits_for_owned_attempt() {
        let mut c = Coordinator::default();
        let start = Effect::Start {
            ticket: ticket(1),
            revision: 0,
            permit: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        };
        assert!(c.admit(&start, false, false, 0, false));
        let cancel = Effect::CancelLogin {
            ticket: ticket(2),
            attempt: None,
        };
        assert!(c.admit(&cancel, false, false, MAX_OPERATIONS, true));
        assert!(c.defer_cancel(&cancel));
        assert!(c.next_cancel().is_none());
        c.observe(CloudflareAuthStatus {
            phase: dsh_native_core::CloudflareAuthPhase::WaitingBrowser,
            attempt_id: Some("PUBLIC-attempt".into()),
            sequence: 1,
            ..Default::default()
        });
        assert!(
            matches!(c.next_cancel(),Some(Effect::CancelLogin {ticket:t,attempt:Some(a)}) if t==ticket(2)&&a=="PUBLIC-attempt")
        );
        assert!(c.next_cancel().is_none());
        assert!(c.busy());
    }
    #[test]
    fn stale_and_terminal_events_cannot_release_or_revive_auth() {
        let mut c = Coordinator::default();
        c.observe(CloudflareAuthStatus {
            phase: dsh_native_core::CloudflareAuthPhase::Connected,
            attempt_id: Some("PUBLIC-a".into()),
            sequence: 3,
            ..Default::default()
        });
        c.observe(CloudflareAuthStatus {
            phase: dsh_native_core::CloudflareAuthPhase::WaitingBrowser,
            attempt_id: Some("PUBLIC-a".into()),
            sequence: 4,
            ..Default::default()
        });
        assert_eq!(
            c.metadata.as_ref().unwrap().phase,
            dsh_native_core::CloudflareAuthPhase::Connected
        );
        assert!(!c.admit(&Effect::Read(ticket(4)), true, false, 0, false));
        assert!(!c.admit(
            &Effect::CancelLogin {
                ticket: ticket(5),
                attempt: None
            },
            false,
            true,
            0,
            false
        ));
    }
    #[test]
    fn deferred_cancel_never_uses_previous_attempt_from_forwarded_old_event() {
        let mut c = Coordinator::default();
        c.observe(CloudflareAuthStatus {
            phase: dsh_native_core::CloudflareAuthPhase::Connected,
            attempt_id: Some("PUBLIC-old".into()),
            sequence: 2,
            ..Default::default()
        });
        let start = Effect::Start {
            ticket: ticket(1),
            revision: 0,
            permit: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        };
        assert!(c.admit(&start, false, false, 0, false));
        let cancel = Effect::CancelLogin {
            ticket: ticket(2),
            attempt: None,
        };
        assert!(c.admit(&cancel, false, false, MAX_OPERATIONS, true));
        c.defer_cancel(&cancel);
        c.observe(CloudflareAuthStatus {
            phase: dsh_native_core::CloudflareAuthPhase::Connected,
            attempt_id: Some("PUBLIC-old".into()),
            sequence: 3,
            ..Default::default()
        });
        assert!(c.next_cancel().is_none());
        c.observe(CloudflareAuthStatus {
            phase: dsh_native_core::CloudflareAuthPhase::WaitingBrowser,
            attempt_id: Some("PUBLIC-new".into()),
            sequence: 4,
            ..Default::default()
        });
        assert!(
            matches!(c.next_cancel(),Some(Effect::CancelLogin {attempt:Some(a),..}) if a=="PUBLIC-new")
        );
    }
}
