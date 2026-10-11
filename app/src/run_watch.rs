//! Watching the selected session settle, so work finished off-screen is announced
//! exactly once.
//!
//! The invariant is that only a *transition* counts. Selecting a session that is
//! already running, opening the app onto one, or receiving a roster refresh while
//! it runs are all snapshots of a state, not the completion of work. Announcing
//! those would notify the user about a turn they never started watching, and
//! re-announcing on every roster update would make the feature unusable. So the
//! watcher tracks the last observed state per session and reports falling edges
//! only, and never for a session it has not already seen running.

/// The last observed run state of the selected session.
#[derive(Default, Debug)]
pub struct Watch {
    /// The session whose state was last observed.
    session: Option<String>,
    /// Whether that session was running when it was last observed.
    running: bool,
}

/// A settle: the selected session stopped running after this watcher saw it run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settle;

impl Watch {
    /// Observe the selected session's run state, reporting a settle once.
    ///
    /// `running` is the Host's own roster flag for `session`, and `None` means no
    /// session is selected.
    pub fn observe(&mut self, session: Option<&str>, running: bool) -> Option<Settle> {
        let same_session = self.session.as_deref() == session;
        let previous_running = self.running;
        self.session = session.map(str::to_owned);
        self.running = running && session.is_some();
        // A different session is a new subject: its state is a snapshot, whoever
        // was running before is no longer what the user is watching.
        if !same_session {
            return None;
        }
        // Only a falling edge, and only from a state this watcher actually saw.
        (previous_running && !self.running).then_some(Settle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_running_session_settles_once() {
        let mut watch = Watch::default();
        assert_eq!(
            watch.observe(Some("a"), false),
            None,
            "idle is not a settle"
        );
        assert_eq!(
            watch.observe(Some("a"), true),
            None,
            "starting is not a settle"
        );
        assert_eq!(
            watch.observe(Some("a"), false),
            Some(Settle),
            "the falling edge is the settle"
        );
        assert_eq!(
            watch.observe(Some("a"), false),
            None,
            "a repeated idle report must not re-announce"
        );
    }

    #[test]
    fn selecting_an_already_running_session_is_not_a_settle() {
        // Opening the app or selecting a session that is mid-turn shows a state,
        // not the completion of work the user was watching.
        let mut watch = Watch::default();
        assert_eq!(watch.observe(Some("busy"), true), None);
        assert_eq!(watch.observe(Some("busy"), true), None);
        assert_eq!(watch.observe(Some("busy"), false), Some(Settle));

        // A roster refresh that repeats the running state is not a settle either.
        let mut fresh = Watch::default();
        assert_eq!(fresh.observe(Some("a"), true), None);
        assert_eq!(fresh.observe(Some("a"), true), None);
    }

    #[test]
    fn switching_sessions_does_not_borrow_the_previous_running_state() {
        let mut watch = Watch::default();
        assert_eq!(watch.observe(Some("a"), true), None);
        // `b` has never been seen running, so its idle state is not a settle.
        assert_eq!(watch.observe(Some("b"), false), None);
        assert_eq!(watch.observe(Some("b"), true), None);
        assert_eq!(watch.observe(Some("b"), false), Some(Settle));
    }

    #[test]
    fn deselecting_and_no_session_are_both_safe() {
        let mut watch = Watch::default();
        assert_eq!(watch.observe(Some("a"), true), None);
        // Losing the selection cancels the subject rather than settling it: the
        // turn did not finish, the user simply stopped looking at it.
        assert_eq!(watch.observe(None, false), None);
        assert_eq!(watch.observe(None, true), None, "no session never runs");
        // Returning to `a` is a fresh subject again.
        assert_eq!(watch.observe(Some("a"), false), None);
    }
}
