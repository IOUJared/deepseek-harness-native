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

/// The line the chip sends.
///
/// The reference's chip can only leave: it renders while the effective target is plan mode and its
/// click always executes `/plan off` (`ui-plan/src/client/PlanModeControl.tsx:32-56`). Sending
/// `/plan` from here would also enter in the one state the reference hides — a logged-active
/// session with a pending change — so the chip never does.
pub fn toggle_line(_mode: Mode) -> &'static str {
    "/plan off"
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
    /// Whether the chip is drawn at all: only while plan mode is the effective target, which is
    /// the reference's own `if (!target) return null`.
    pub fn shown(self) -> bool {
        self.visible() && self.mode.is_some_and(Mode::effective)
    }
    /// The line to send, or nothing when the control must not act. An
    /// unavailable command, a hidden state and an in-flight toggle are all refused here as well as
    /// hidden, so a caller that dispatches without checking visibility still cannot invoke one.
    pub fn press(self) -> Option<&'static str> {
        let mode = self.mode?;
        (self.shown() && !self.pending).then(|| toggle_line(mode))
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
    fn the_chip_can_only_leave_and_only_while_plan_mode_is_the_effective_target() {
        // The reference's chip always executes `/plan off`; it never enters.
        for mode in [Mode::Active, Mode::Applying, Mode::Leaving] {
            assert_eq!(toggle_line(mode), "/plan off", "{mode:?}");
        }
        // Effective target: active, or inactive with a pending enter.
        assert!(Mode::Active.effective());
        assert!(Mode::Applying.effective());
        assert!(!Mode::Leaving.effective());
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

    #[test]
    fn the_chip_is_hidden_while_a_pending_change_would_turn_plan_mode_off() {
        let control = |mode, pending| Control {
            offer: Offer::Available,
            mode: Some(mode),
            pending,
        };
        // Active and a pending enter are both drawn; the reference hides the leaving state.
        assert!(control(Mode::Active, false).shown());
        assert!(control(Mode::Applying, false).shown());
        assert!(!control(Mode::Leaving, false).shown());
        assert!(!control(Mode::Leaving, false).press().is_some());
        assert_eq!(control(Mode::Applying, false).press(), Some("/plan off"));
        // A hidden state refuses even if a caller dispatches without checking `shown`.
        assert!(control(Mode::Active, true).press().is_none());
        assert!(
            !control(Mode::Active, true).shown() || control(Mode::Active, true).press().is_none()
        );
    }
}
