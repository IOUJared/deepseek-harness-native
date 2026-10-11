//! Entering and leaving plan mode.
//!
//! Plan mode is not a prompt: `/plan` sent as conversation is ordinary text. The
//! Host executes it as a command, so this is the one place the app invokes Host
//! behaviour without a model turn. Every decision here is about refusing to
//! pretend: the control appears only for a session whose Host advertises `plan`,
//! it cannot be pressed twice, and the mode that results is read back from the
//! `plan` projection rather than assumed from the command's own reply.
use super::plan_mode::Mode;

/// Whether this Host offers the command that toggles plan mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    /// The advertised command list has not arrived, so nothing is offered.
    Unknown,
    /// The Host advertises `plan`.
    Available,
    /// The Host answered and does not advertise it.
    Absent,
}

impl Offer {
    /// Whether the Host published the `plan` projection: `None` before it answers,
    /// then whether the plan-mode plugin is mounted at all.
    pub fn from_published(published: Option<bool>) -> Self {
        match published {
            None => Self::Unknown,
            Some(true) => Self::Available,
            Some(false) => Self::Absent,
        }
    }
    pub fn available(self) -> bool {
        self == Self::Available
    }
}

/// The line that selects the opposite of the committed state.
///
/// A pending selection is not the committed one: `Mode::Applying` is active
/// already and `Mode::Leaving` is still active, so the toggle always asks for the
/// opposite of `active` and lets the projection confirm it.
pub fn toggle_line(mode: Mode) -> &'static str {
    match mode {
        Mode::Active | Mode::Leaving => "/plan off",
        Mode::Applying => "/plan",
    }
}

/// What the control should do for one session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Control {
    pub offer: Offer,
    pub mode: Option<Mode>,
    /// A toggle is in flight; the control is inert until its reply settles.
    pub pending: bool,
}

impl Control {
    pub fn visible(self) -> bool {
        self.offer.available()
    }
    /// The line to send, or nothing when the control must not act. An
    /// unavailable command is refused here as well as hidden, so a caller that
    /// dispatches without checking visibility still cannot invoke one.
    pub fn press(self) -> Option<&'static str> {
        let mode = self.mode?;
        (self.offer.available() && !self.pending).then(|| toggle_line(mode))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_control_appears_only_for_a_published_projection() {
        assert_eq!(Offer::from_published(None), Offer::Unknown);
        assert!(!Offer::from_published(None).available());
        assert!(Offer::from_published(Some(true)).available());
        assert_eq!(Offer::from_published(Some(false)), Offer::Absent);
        assert!(!Offer::from_published(Some(false)).available());
    }

    #[test]
    fn the_toggle_asks_for_the_opposite_of_the_committed_state() {
        // Active and leaving are both committed-active, so both ask to leave.
        assert_eq!(toggle_line(Mode::Active), "/plan off");
        assert_eq!(toggle_line(Mode::Leaving), "/plan off");
        // Applying is committed-inactive, so it asks to enter.
        assert_eq!(toggle_line(Mode::Applying), "/plan");
    }

    #[test]
    fn nothing_is_sent_without_a_state_or_while_one_is_in_flight() {
        let available = Offer::Available;
        assert_eq!(
            Control {
                offer: available,
                mode: None,
                pending: false
            }
            .press(),
            None,
            "no projection means no state to invert"
        );
        assert_eq!(
            Control {
                offer: available,
                mode: Some(Mode::Active),
                pending: true
            }
            .press(),
            None,
            "a second press must not queue a second command"
        );
        assert_eq!(
            Control {
                offer: Offer::Absent,
                mode: Some(Mode::Active),
                pending: false
            }
            .press(),
            None,
            "an absent command is never invoked"
        );
        assert_eq!(
            Control {
                offer: available,
                mode: Some(Mode::Active),
                pending: false
            }
            .press(),
            Some("/plan off")
        );
    }

    #[test]
    fn visibility_tracks_the_offer_alone() {
        for (offer, visible) in [
            (Offer::Unknown, false),
            (Offer::Available, true),
            (Offer::Absent, false),
        ] {
            assert_eq!(
                Control {
                    offer,
                    mode: None,
                    pending: false
                }
                .visible(),
                visible,
                "{offer:?}"
            );
        }
    }
}
