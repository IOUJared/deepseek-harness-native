//! Global catalog read ownership, independent of session/menu generations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ticket {
    epoch: u64,
    serial: u64,
}
impl Ticket {
    pub(crate) fn epoch(self) -> u64 {
        self.epoch
    }
}
#[derive(Default)]
pub(crate) struct State {
    serial: u64,
    flight: Option<Ticket>,
}
impl State {
    pub(crate) fn pending(&self) -> bool {
        self.flight.is_some()
    }
    pub(crate) fn begin(&mut self, epoch: u64) -> Option<Ticket> {
        if epoch == 0 || self.pending() {
            return None;
        }
        let serial = self.serial.checked_add(1)?;
        let ticket = Ticket { epoch, serial };
        self.serial = serial; // Queue rejection consumes the serial too; never replay it.
        self.flight = Some(ticket);
        Some(ticket)
    }
    pub(crate) fn owns(&self, ticket: Ticket, epoch: u64) -> bool {
        epoch != 0 && ticket.epoch == epoch && self.flight == Some(ticket)
    }
    pub(crate) fn complete(&mut self, ticket: Ticket, epoch: u64) -> bool {
        if !self.owns(ticket, epoch) {
            return false;
        }
        self.flight = None;
        true
    }
    pub(crate) fn retire(&mut self) {
        self.flight = None;
    }
    #[cfg(test)]
    pub(crate) fn current(&self) -> Option<Ticket> {
        self.flight
    }
    #[cfg(test)]
    pub(crate) fn exhausted() -> Self {
        Self { serial: u64::MAX, flight: None }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_global_read_survives_unrelated_context_and_completes_once() {
        let mut state = State::default();
        assert!(state.begin(0).is_none());
        let ticket = state.begin(1).unwrap();
        assert!(state.begin(1).is_none());
        assert!(!state.complete(ticket, 2));
        assert!(state.pending());
        assert!(state.complete(ticket, 1));
        assert!(!state.complete(ticket, 1));
    }
    #[test]
    fn retirement_never_reuses_a_ticket_even_with_the_same_backend_epoch() {
        let mut state = State::default();
        let old = state.begin(1).unwrap();
        state.retire();
        let new = state.begin(1).unwrap();
        assert_ne!(old, new);
        assert!(!state.complete(old, 1));
        assert_eq!(state.current(), Some(new));
        assert!(state.complete(new, 1));
    }
    #[test]
    fn serial_exhaustion_never_wraps_or_admits_a_new_read() {
        let mut state = State { serial: u64::MAX - 1, flight: None };
        let last = state.begin(1).unwrap();
        assert_eq!(last.serial, u64::MAX);
        state.retire();
        assert!(state.begin(1).is_none());
        assert!(!state.pending());
        assert!(!state.complete(last, 1));
    }
}
