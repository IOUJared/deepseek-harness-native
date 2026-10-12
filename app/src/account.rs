//! DeepSeek account sign-in and sign-out, shown on Settings → API login.
//!
//! The Host owns the whole flow: this controller starts an attempt, shows the
//! phase it reports, offers the browser once while the Host waits for the
//! callback, and cancels or signs out. It never sees the destination itself -
//! the backend holds it privately and one-use, and only says whether one is
//! available - and it never sees a token, because none is projected.
//!
//! The permit below is the panel's own authority to open a browser: closing the
//! panel, starting another attempt or signing out withdraws it, so a click that
//! is already in flight cannot open a window for a flow the user has left.
use crate::design;
use crate::settings::Ticket;
use crate::ui::{ACCENT, DANGER, MUTED, TEXT, label};
use dsh_native_core::{AccountMetadata, AccountStatus, AttemptPhase};
use iced::{
    Element, Length,
    widget::{button, column, row},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Start,
    Cancel,
    SignOut,
    OpenBrowser,
}
pub enum Effect {
    /// Subscribe to the Host's account stream. The stream is what arms a
    /// sign-in's browser, so the panel asks for it before it starts an attempt.
    Subscribe {
        ticket: Ticket,
    },
    Start {
        ticket: Ticket,
    },
    Cancel {
        ticket: Ticket,
        attempt_id: String,
    },
    SignOut {
        ticket: Ticket,
    },
    OpenBrowser {
        ticket: Ticket,
        attempt_id: String,
        permit: Arc<AtomicBool>,
    },
}
impl Effect {
    pub fn ticket(&self) -> Ticket {
        match self {
            Self::Subscribe { ticket }
            | Self::Start { ticket }
            | Self::Cancel { ticket, .. }
            | Self::SignOut { ticket }
            | Self::OpenBrowser { ticket, .. } => *ticket,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The Host's view of the account, from a read or a mutation.
    Observed(AccountMetadata),
    /// The account stream is not available, so no sign-in can be started.
    Unavailable,
    /// The launcher exited successfully. That is not proof of sign-in.
    BrowserOpened,
    NotSent,
    Unknown,
}

pub struct Controller {
    metadata: Option<AccountMetadata>,
    pending: Option<Ticket>,
    /// One permit per attempt: withdrawn when the panel leaves, the attempt ends
    /// or another attempt starts.
    permit: Arc<AtomicBool>,
    serial: u64,
    /// A failure sentence the user must see, cleared by the next attempt.
    notice: Option<&'static str>,
    /// Whether this panel has asked the Host for the account stream yet.
    subscribed: bool,
    /// The outstanding subscription, if any. It is not a mutation: its ack must
    /// not block the sign-in the same click is asking for.
    subscribing: Option<Ticket>,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            metadata: None,
            pending: None,
            // A panel that has not left yet may open the browser of the attempt
            // it is watching; `Default` must not mean "already withdrawn".
            permit: Arc::new(AtomicBool::new(true)),
            serial: 0,
            notice: None,
            subscribed: false,
            subscribing: None,
        }
    }
}
impl Controller {
    fn ticket(&mut self) -> Ticket {
        self.serial = self.serial.wrapping_add(1);
        Ticket {
            epoch: 0,
            panel: 0,
            serial: self.serial,
        }
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn observed(&mut self, metadata: AccountMetadata) {
        // An observed view never settles a mutation: only the ack for that exact
        // ticket does, so a stream push cannot clear an outstanding request.
        if metadata.status == AccountStatus::SignedOut {
            self.notice = None;
        }
        self.metadata = Some(metadata);
    }
    pub fn completed(&mut self, ticket: Ticket, outcome: Outcome) {
        if self.subscribing == Some(ticket) {
            self.subscribing = None;
            match outcome {
                Outcome::Observed(metadata) => self.observed(metadata),
                Outcome::Unavailable => {
                    self.subscribed = false;
                    self.notice = Some("Account state is unavailable; sign-in cannot start yet.");
                }
                _ => {}
            }
            return;
        }
        if self.pending != Some(ticket) {
            return;
        }
        self.pending = None;
        match outcome {
            Outcome::Observed(metadata) => self.observed(metadata),
            Outcome::Unavailable => {
                self.subscribed = false;
                self.notice = Some("Account state is unavailable; sign-in cannot start yet.");
            }
            Outcome::BrowserOpened => {}
            Outcome::NotSent => self.notice = Some("Sign-in was not started; try again."),
            Outcome::Unknown => {
                self.notice = Some("Sign-in outcome unknown; the browser may already be open.")
            }
        }
    }
    /// Leaving the panel withdraws every authority this panel holds.
    pub fn leave(&mut self) {
        self.permit.store(false, Ordering::Release);
        self.permit = Arc::new(AtomicBool::new(true));
        self.pending = None;
    }
    /// The stream is subscribed when a sign-in is asked for, and never before: a
    /// run that does not sign in queues no work at all, and the stream must be
    /// live before the attempt exists or its browser could never be armed.
    pub fn subscribe_effect(&mut self) -> Option<Effect> {
        if self.subscribed {
            return None;
        }
        self.subscribed = true;
        let ticket = self.ticket();
        self.subscribing = Some(ticket);
        Some(Effect::Subscribe { ticket })
    }
    pub fn handle(&mut self, action: Action, enabled: bool) -> Option<Effect> {
        let metadata = self.metadata.clone();
        let status = metadata.as_ref().map(|metadata| metadata.status);
        let phase = metadata.as_ref().and_then(|metadata| metadata.phase);
        let attempt = metadata
            .as_ref()
            .and_then(|metadata| metadata.attempt.as_ref());
        let waiting = phase == Some(AttemptPhase::WaitingBrowser);
        let busy = self.pending.is_some();
        let ticket = self.ticket();
        match action {
            Action::Start
                if enabled && !busy && status != Some(AccountStatus::CredentialStored) =>
            {
                // A new attempt owns the only permit; the previous one is spent.
                self.permit.store(false, Ordering::Release);
                self.permit = Arc::new(AtomicBool::new(true));
                self.notice = None;
                self.pending = Some(ticket);
                Some(Effect::Start { ticket })
            }
            // The browser is offered only while the Host waits for a callback and
            // only for an attempt that has not spent its destination.
            Action::OpenBrowser
                if enabled
                    && !busy
                    && waiting
                    && attempt.is_some_and(|attempt| attempt.browser_available) =>
            {
                let attempt_id = attempt.map(|attempt| attempt.id.clone())?;
                self.pending = Some(ticket);
                Some(Effect::OpenBrowser {
                    ticket,
                    attempt_id,
                    permit: Arc::clone(&self.permit),
                })
            }
            Action::Cancel if enabled && !busy && waiting => {
                let attempt_id = attempt.map(|attempt| attempt.id.clone())?;
                self.permit.store(false, Ordering::Release);
                self.pending = Some(ticket);
                Some(Effect::Cancel { ticket, attempt_id })
            }
            Action::SignOut
                if enabled && !busy && status == Some(AccountStatus::CredentialStored) =>
            {
                self.permit.store(false, Ordering::Release);
                self.pending = Some(ticket);
                Some(Effect::SignOut { ticket })
            }
            _ => None,
        }
    }
    /// The account block on the API login page. Only live states appear: no
    /// revision, no ids, no destination.
    pub fn view(&self, enabled: bool) -> Element<'_, Action> {
        let status = self.metadata.as_ref().map(|metadata| metadata.status);
        let phase = self.metadata.as_ref().and_then(|metadata| metadata.phase);
        let attempt = self
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.attempt.as_ref());
        let mut body = column![
            label("Account", TEXT).size(13),
            label(
                match status {
                    Some(AccountStatus::CredentialStored) =>
                        "Signed in through the browser. The credential stays in the Host's store.",
                    Some(AccountStatus::SignedOut) => {
                        "Sign in through your browser to use this account for model requests."
                    }
                    None => "Account state not read yet.",
                },
                MUTED,
            )
            .size(11),
        ]
        .spacing(4);
        if let Some(phase) = phase {
            let (text, danger) = phase_line(phase);
            body = body.push(label(text, if danger { DANGER } else { MUTED }).size(11));
        }
        if let Some(notice) = self.notice {
            body = body.push(label(notice, DANGER).size(11));
        }
        let busy = self.pending.is_some();
        let mut controls = row![].spacing(6);
        match status {
            Some(AccountStatus::CredentialStored) => {
                controls = controls.push(
                    button(label("Sign out", MUTED).size(11))
                        .style(design::ghost)
                        .padding([6, 9])
                        .on_press_maybe((enabled && !busy).then_some(Action::SignOut)),
                );
            }
            _ => {
                let waiting = phase == Some(AttemptPhase::WaitingBrowser);
                let can_open = waiting && attempt.is_some_and(|attempt| attempt.browser_available);
                controls = controls
                    .push(
                        button(
                            label(
                                if busy {
                                    "Starting sign-in…"
                                } else {
                                    "Sign in with DeepSeek"
                                },
                                ACCENT,
                            )
                            .size(11),
                        )
                        .style(design::primary)
                        .padding([6, 9])
                        .on_press_maybe((enabled && !busy && !waiting).then_some(Action::Start)),
                    )
                    .push(
                        button(label("Open browser", TEXT).size(11))
                            .style(design::ghost)
                            .padding([6, 9])
                            .on_press_maybe(
                                (enabled && !busy && can_open).then_some(Action::OpenBrowser),
                            ),
                    )
                    .push(
                        button(label("Cancel sign-in", MUTED).size(11))
                            .style(design::ghost)
                            .padding([6, 9])
                            .on_press_maybe(
                                (enabled && !busy && waiting).then_some(Action::Cancel),
                            ),
                    );
            }
        }
        body = body.push(controls);
        body.width(Length::Fill).into()
    }
}
fn phase_line(phase: AttemptPhase) -> (&'static str, bool) {
    match phase {
        AttemptPhase::Initializing => ("Preparing sign-in…", false),
        AttemptPhase::WaitingBrowser => (
            "Waiting for the browser. Finish signing in there, then come back.",
            false,
        ),
        AttemptPhase::Exchanging | AttemptPhase::Committing => ("Finishing sign-in…", false),
        AttemptPhase::Succeeded => ("Signed in.", false),
        AttemptPhase::Cancelled => ("Sign-in cancelled.", false),
        AttemptPhase::Expired => ("Sign-in expired. Start again to get a new link.", true),
        AttemptPhase::Failed => ("Sign-in failed. Start again, or use an API key.", true),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use dsh_native_core::AccountAttempt;

    fn account_with(status: AccountStatus, phase: Option<AttemptPhase>) -> Controller {
        let mut controller = Controller::default();
        controller.observed(AccountMetadata {
            status,
            phase,
            attempt: phase.map(|_| AccountAttempt {
                id: "attempt-1".to_owned(),
                browser_available: phase == Some(AttemptPhase::WaitingBrowser),
            }),
        });
        controller
    }

    #[test]
    fn a_signed_out_account_offers_sign_in_and_nothing_else() {
        let mut controller = account_with(AccountStatus::SignedOut, None);
        assert!(matches!(
            controller.handle(Action::Start, true),
            Some(Effect::Start { .. })
        ));
        assert!(controller.pending());
        // Nothing else is admitted while the start is outstanding.
        assert!(controller.handle(Action::Cancel, true).is_none());
        assert!(controller.handle(Action::SignOut, true).is_none());
        assert!(controller.handle(Action::OpenBrowser, true).is_none());
    }

    #[test]
    fn the_browser_is_offered_once_while_the_host_waits() {
        let mut controller =
            account_with(AccountStatus::SignedOut, Some(AttemptPhase::WaitingBrowser));
        let effect = controller
            .handle(Action::OpenBrowser, true)
            .expect("a waiting attempt with a destination opens the browser");
        match effect {
            Effect::OpenBrowser {
                attempt_id, permit, ..
            } => {
                assert_eq!(attempt_id, "attempt-1");
                assert!(permit.load(Ordering::Acquire));
            }
            _ => panic!("expected a browser effect"),
        }
        // Starting again is refused while this attempt is running.
        assert!(
            controller.handle(Action::Start, true).is_none(),
            "an attempt already waiting for the browser is not replaced"
        );
        // An account with no destination available does not offer one.
        let mut armed = account_with(AccountStatus::SignedOut, Some(AttemptPhase::WaitingBrowser));
        armed.metadata.as_mut().unwrap().attempt = Some(AccountAttempt {
            id: "attempt-1".to_owned(),
            browser_available: false,
        });
        assert!(armed.handle(Action::OpenBrowser, true).is_none());
    }

    #[test]
    fn a_signed_in_account_offers_sign_out_and_never_a_new_sign_in() {
        let mut controller = account_with(AccountStatus::CredentialStored, None);
        assert!(matches!(
            controller.handle(Action::SignOut, true),
            Some(Effect::SignOut { .. })
        ));
        assert!(controller.handle(Action::Start, true).is_none());
    }

    #[test]
    fn leaving_or_a_withdrawn_permit_stops_a_browser_opening() {
        let mut controller =
            account_with(AccountStatus::SignedOut, Some(AttemptPhase::WaitingBrowser));
        let permit = match controller.handle(Action::OpenBrowser, true) {
            Some(Effect::OpenBrowser { permit, .. }) => permit,
            other => panic!("expected a browser effect, got {:?}", other.is_some()),
        };
        controller.leave();
        assert!(
            !permit.load(Ordering::Acquire),
            "leaving the panel must withdraw a browser that has not opened yet"
        );
        // A completion that arrives after the panel left is stale: it settles
        // nothing and shows nothing.
        controller.completed(
            Ticket {
                epoch: 0,
                panel: 0,
                serial: controller.serial,
            },
            Outcome::NotSent,
        );
        assert!(!controller.pending());
        assert!(controller.notice.is_none());
    }

    #[test]
    fn subscribing_carries_the_view_without_blocking_the_sign_in_it_precedes() {
        let mut controller = Controller::default();
        let subscribe = controller
            .subscribe_effect()
            .expect("the first ask subscribes");
        assert!(!controller.pending(), "a subscription is not a mutation");
        // The very same click may now start the sign-in, even though the
        // subscription has not answered yet.
        let start = controller
            .handle(Action::Start, true)
            .expect("the click starts the attempt");
        // The stream's own ack carries the current view and settles nothing.
        controller.completed(
            subscribe.ticket(),
            Outcome::Observed(AccountMetadata {
                status: AccountStatus::SignedOut,
                phase: None,
                attempt: None,
            }),
        );
        assert!(
            controller.pending(),
            "the sign-in is still outstanding after the subscription answered"
        );
        // And it is not asked for twice.
        assert!(controller.subscribe_effect().is_none());
        controller.completed(start.ticket(), Outcome::Unknown);
        assert!(!controller.pending());
    }

    #[test]
    fn only_the_outstanding_ticket_settles_a_request() {
        let mut controller = account_with(AccountStatus::SignedOut, None);
        let effect = controller.handle(Action::Start, true).unwrap();
        let stale = Ticket {
            epoch: 0,
            panel: 0,
            serial: 999,
        };
        controller.completed(stale, Outcome::NotSent);
        assert!(controller.pending(), "a stale ticket changes nothing");
        assert!(controller.notice.is_none());
        controller.completed(effect.ticket(), Outcome::BrowserOpened);
        assert!(!controller.pending());
    }
}
