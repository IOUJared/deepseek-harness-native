//! Worker-issued live delivery tickets; UI schema checks are not reply admission.
use crate::interactions::{Key, ReplySpec, Submission};
use dsh_native_transport::dto::{AgentId, RemoteEventFrame, RemoteEventId};
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "decision_worker_tests.rs"]
mod tests;
const MAX_PENDING: usize = 64;
const MAX_RETAINED: usize = 512 * 1024;
struct Entry<D> {
    key: Key,
    _agent: AgentId,
    spec: Option<ReplySpec>,
    delivery: D,
    bytes: usize,
    last_attempt: u64,
    active: Option<u64>,
}
/// D is the worker's private carrier owner (Arc<EventDelivery> in production).
/// Tests substitute drop-tracked owners; no carrier or reply authority crosses the GUI.
pub(super) struct DecisionCoordinator<D> {
    entries: BTreeMap<RemoteEventId, Entry<D>>,
    serial: u64,
}
impl<D> Default for DecisionCoordinator<D> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            serial: 0,
        }
    }
}
pub(super) enum Admission<D> {
    Send(D),
    Reject,
    /// A duplicate must not publish an error that resets the real submitting attempt.
    Ignore,
}
impl<D> DecisionCoordinator<D> {
    pub(super) fn observe(
        &mut self,
        epoch: u64,
        frame: &RemoteEventFrame,
        delivery: D,
    ) -> Result<Key, &'static str> {
        let RemoteEventFrame::Waterfall {
            event_id,
            agent_id,
            event,
            request,
        } = frame
        else {
            return Err("Decision ticket requires a live waterfall");
        };
        if self.entries.contains_key(event_id) {
            return Err("Duplicate live decision delivery");
        }
        let (spec, request_bytes) = crate::interactions::reply_spec(event.clone(), request);
        let bytes = request_bytes
            .saturating_add(event_id.as_str().len())
            .saturating_add(agent_id.as_str().len())
            .saturating_add(256);
        if self.entries.len() >= MAX_PENDING
            || self
                .entries
                .values()
                .map(|entry| entry.bytes)
                .sum::<usize>()
                .saturating_add(bytes)
                > MAX_RETAINED
        {
            return Err("Native worker decision capacity reached; no reply sent");
        }
        let serial = self
            .serial
            .checked_add(1)
            .ok_or("Native decision ticket counter exhausted")?;
        let key = Key {
            epoch,
            event_id: event_id.clone(),
            serial,
        };
        self.entries.insert(
            event_id.clone(),
            Entry {
                key: key.clone(),
                _agent: agent_id.clone(),
                spec,
                delivery,
                bytes,
                last_attempt: 0,
                active: None,
            },
        );
        self.serial = serial;
        Ok(key)
    }
    pub(super) fn cancel(&mut self, event_id: &RemoteEventId) {
        self.entries.remove(event_id);
    }
    pub(super) fn delivery_live(&self, key: &Key, live: impl FnOnce(&D) -> bool) -> bool {
        self.entries
            .get(&key.event_id)
            .is_some_and(|entry| entry.key == *key && live(&entry.delivery))
    }
    pub(super) fn is_active(&self, submission: &Submission) -> bool {
        self.entries
            .get(&submission.key.event_id)
            .is_some_and(|entry| {
                entry.key == submission.key && entry.active == Some(submission.attempt)
            })
    }
    pub(super) fn admit(
        &mut self,
        submission: &Submission,
        epoch: u64,
        enabled: bool,
    ) -> Admission<D>
    where
        D: Clone,
    {
        let Some(entry) = self.entries.get_mut(&submission.key.event_id) else {
            return Admission::Reject;
        };
        if entry.key == submission.key && entry.active == Some(submission.attempt) {
            return Admission::Ignore;
        }
        if !enabled
            || submission.key.epoch != epoch
            || entry.key != submission.key
            || entry.active.is_some()
            || submission.attempt == 0
            || submission.attempt <= entry.last_attempt
            || !entry
                .spec
                .as_ref()
                .is_some_and(|spec| spec.allows(&submission.reply))
        {
            return Admission::Reject;
        }
        entry.last_attempt = submission.attempt;
        entry.active = Some(submission.attempt);
        Admission::Send(entry.delivery.clone())
    }
    /// RPC acknowledgement retires only this delivery; never infer business settlement.
    pub(super) fn settle(&mut self, submission: &Submission, acknowledged: bool) {
        let Some(entry) = self.entries.get_mut(&submission.key.event_id) else {
            return;
        };
        if entry.key != submission.key || entry.active != Some(submission.attempt) {
            return;
        }
        if acknowledged {
            self.entries.remove(&submission.key.event_id);
        } else {
            entry.active = None;
        }
    }
}
