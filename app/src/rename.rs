//! Operator-reviewed Session title rename. The typed title stays in this editor or an
//! owned submission; the sidebar title changes only from the acknowledged normalized value.
//! Closing hides the editor; it never rolls back an admitted write or retries one.
use crate::management::{Ticket, View};
use dsh_native_transport::dto::SessionId;

/// Maximum accepted UTF-8 bytes in a proposed title. This matches the 1024-byte bound
/// `App::observe_title` already applies to observed `session/title` values; a title that
/// normalizes within it can never be truncated by this editor.
pub const MAX_TITLE_BYTES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub ticket: Ticket,
    pub editor: u64,
    pub attempt: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Review {
    pub ticket: Ticket,
    pub editor: u64,
}
/// A locally refused proposal. The editor keeps the typed text so the operator can correct it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    Empty,
    TooLong,
    Control,
}
impl Invalid {
    pub fn text(self) -> &'static str {
        match self {
            Self::Empty => "Enter a title with visible characters.",
            Self::TooLong => "Title is limited to 1024 UTF-8 bytes.",
            Self::Control => "Title cannot contain control or invisible formatting characters.",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The durable ACK: Host-normalized title and the event seq that committed it.
    Renamed { title: String, seq: i64 },
    /// The command never left the native queue.
    NotSent,
    /// Admitted, but the outcome is unknown; reconcile explicitly and never retry.
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    None,
    Invalid(Invalid),
    Pending,
    Renamed { title: String },
    NotSent,
    Indeterminate,
}
/// The accepted value, returned only for the exact pending receipt. Title display uses it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    pub target: SessionId,
    pub title: String,
    pub seq: i64,
}

/// Cloning an input is not permission to submit it. Every admission checks its current ticket.
#[derive(Clone)]
pub enum Action {
    /// Open the editor for one target, pre-filled with the title the operator currently sees.
    Open {
        view: View,
        title: String,
    },
    Close(Ticket),
    Edit {
        ticket: Ticket,
        title: String,
    },
    Review(Ticket),
    Back(Ticket),
    Confirm(Review),
}
pub struct Context {
    pub view: Option<View>,
    pub allowed: bool,
}
/// One consuming operator-reviewed title. The typed draft is not retained in the submission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submission {
    pub receipt: Receipt,
    title: String,
}
impl Submission {
    pub fn title(&self) -> &str {
        &self.title
    }
}
struct Panel {
    ticket: Ticket,
    editor: u64,
}

/// Invisible/directional formatting controls the Host strips during normalization.
/// Refusing them locally keeps this rule aligned with `normalizeSessionTitle`, so a
/// title that could only normalize to empty never becomes an admitted write.
fn invisible(character: char) -> bool {
    matches!(character,
        '\u{200B}' | '\u{200E}' | '\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{206F}'
        | '\u{FEFF}')
}
/// Local validation shared by Review, Confirm and the ACK. Never normalizes the input.
fn invalid(title: &str) -> Option<Invalid> {
    if title.trim().is_empty() {
        Some(Invalid::Empty)
    } else if title.len() > MAX_TITLE_BYTES {
        Some(Invalid::TooLong)
    } else if title.chars().any(|c| c.is_control() || invisible(c)) {
        Some(Invalid::Control)
    } else {
        None
    }
}

pub struct Rename {
    panel: Option<Panel>,
    title: String,
    review: Option<Review>,
    pending: Option<Receipt>,
    notice: Notice,
    serial: u64,
    editor: u64,
    attempt: u64,
    exhausted: bool,
}
impl Default for Rename {
    fn default() -> Self {
        Self {
            panel: None,
            title: String::new(),
            review: None,
            pending: None,
            notice: Notice::None,
            serial: 0,
            editor: 0,
            attempt: 0,
            exhausted: false,
        }
    }
}
impl Rename {
    pub fn ticket(&self) -> Option<Ticket> {
        self.panel.as_ref().map(|panel| panel.ticket.clone())
    }
    /// The operator's typed draft; never a Host value or a predicted normalized title.
    pub fn title(&self) -> &str {
        &self.title
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
    /// Hide and clear the editor without cancelling or rolling back an admitted write.
    pub fn dismiss(&mut self) {
        self.panel = None;
        self.title.clear();
        self.review = None;
        self.notice = Notice::None;
    }
    /// Fence late receipts. An admitted title may already be durable on the Host.
    pub fn disconnect(&mut self) {
        let admitted = self.pending.take().is_some();
        self.dismiss();
        if admitted {
            self.notice = Notice::Indeterminate;
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
            Action::Open { view, title }
                if !self.exhausted && context.allowed && context.view.as_ref() == Some(&view) =>
            {
                let (Some(serial), Some(editor)) =
                    (self.serial.checked_add(1), self.editor.checked_add(1))
                else {
                    self.exhausted();
                    return None;
                };
                self.serial = serial;
                self.editor = editor;
                self.dismiss();
                self.panel = Some(Panel {
                    ticket: Ticket {
                        epoch: view.epoch,
                        generation: view.generation,
                        serial,
                        target: view.target,
                    },
                    editor,
                });
                self.title = title;
                self.notice = if self.pending.is_some() {
                    Notice::Pending
                } else {
                    match invalid(&self.title) {
                        Some(reason) => Notice::Invalid(reason),
                        None => Notice::None,
                    }
                };
            }
            Action::Edit { ticket, title } if self.current(&ticket, context) => {
                let Some(editor) = self.editor.checked_add(1) else {
                    self.exhausted();
                    return None;
                };
                self.editor = editor;
                self.panel.as_mut().unwrap().editor = editor;
                // Keep the typed text even when it is refused; only the review is retired.
                self.title = title;
                self.review = None;
                self.notice = if self.pending.is_some() {
                    Notice::Pending
                } else {
                    match invalid(&self.title) {
                        Some(reason) => Notice::Invalid(reason),
                        None => Notice::None,
                    }
                };
            }
            Action::Back(ticket) if self.current(&ticket, context) && self.pending.is_none() => {
                if self.review.take().is_some() {
                    self.notice = match invalid(&self.title) {
                        Some(reason) => Notice::Invalid(reason),
                        None => Notice::None,
                    };
                }
            }
            Action::Review(ticket) if self.current(&ticket, context) && self.pending.is_none() => {
                self.review = None;
                match invalid(&self.title) {
                    Some(reason) => self.notice = Notice::Invalid(reason),
                    None => {
                        self.review = Some(Review {
                            ticket,
                            editor: self.panel.as_ref().unwrap().editor,
                        });
                        self.notice = Notice::None;
                    }
                }
            }
            Action::Confirm(review)
                if self.current(&review.ticket, context)
                    && self.pending.is_none()
                    && self.review.as_ref() == Some(&review)
                    && self
                        .panel
                        .as_ref()
                        .is_some_and(|panel| panel.editor == review.editor) =>
            {
                let Some(attempt) = self.attempt.checked_add(1) else {
                    self.exhausted();
                    return None;
                };
                // Validation is repeated on the consumed draft, never a default title.
                if let Some(reason) = invalid(&self.title) {
                    self.notice = Notice::Invalid(reason);
                    return None;
                }
                let title = self.title.clone();
                self.review = None;
                self.attempt = attempt;
                let receipt = Receipt {
                    ticket: review.ticket,
                    editor: review.editor,
                    attempt,
                };
                self.pending = Some(receipt.clone());
                self.notice = Notice::Pending;
                return Some(Submission { receipt, title });
            }
            _ => {}
        }
        None
    }
    /// Retire only the exact pending receipt. A malformed ACK is never installed as a title.
    pub fn finish(&mut self, receipt: &Receipt, outcome: Outcome) -> Option<Applied> {
        if self.pending.as_ref() != Some(receipt) {
            return None;
        }
        self.pending = None;
        let mut applied = None;
        let notice = match outcome {
            Outcome::Renamed { title, seq } if seq >= 0 && invalid(&title).is_none() => {
                applied = Some(Applied {
                    target: receipt.ticket.target.clone(),
                    title: title.clone(),
                    seq,
                });
                Notice::Renamed { title }
            }
            Outcome::Renamed { .. } => Notice::Indeterminate,
            Outcome::NotSent => Notice::NotSent,
            Outcome::Unknown => Notice::Indeterminate,
        };
        if self
            .panel
            .as_ref()
            .is_some_and(|panel| panel.ticket == receipt.ticket && panel.editor == receipt.editor)
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
                target: id("PUBLIC-rename-target"),
            }),
            allowed: true,
        }
    }
    fn opened() -> Rename {
        let mut rename = Rename::default();
        rename.apply(
            Action::Open {
                view: context().view.unwrap(),
                title: "PUBLIC current title".into(),
            },
            &context(),
        );
        rename
    }
    fn edit(rename: &mut Rename, title: &str) {
        let ticket = rename.ticket().unwrap();
        assert!(
            rename
                .apply(
                    Action::Edit {
                        ticket,
                        title: title.into()
                    },
                    &context()
                )
                .is_none()
        );
    }
    fn review(rename: &mut Rename) -> Review {
        let ticket = rename.ticket().unwrap();
        assert!(rename.apply(Action::Review(ticket), &context()).is_none());
        rename.review().unwrap().clone()
    }
    fn submit(rename: &mut Rename, title: &str) -> Submission {
        edit(rename, title);
        let review = review(rename);
        rename
            .apply(Action::Confirm(review), &context())
            .expect("reviewed public title")
    }

    #[test]
    fn open_prefills_the_visible_title_and_close_leaves_everything_unchanged() {
        let mut rename = opened();
        assert_eq!(rename.title(), "PUBLIC current title");
        assert_eq!(rename.notice(), &Notice::None);
        edit(&mut rename, "PUBLIC typed replacement");
        let review = review(&mut rename);
        assert_eq!(rename.review(), Some(&review));
        let ticket = rename.ticket().unwrap();
        assert!(rename.apply(Action::Close(ticket), &context()).is_none());
        assert!(!rename.is_open());
        assert_eq!(rename.title(), "");
        assert!(rename.review().is_none());
        assert!(rename.pending().is_none());
        assert_eq!(rename.notice(), &Notice::None);
    }

    #[test]
    fn local_validation_refuses_empty_long_and_control_titles_without_losing_the_draft() {
        let too_long = "é".repeat(MAX_TITLE_BYTES);
        for (title, reason) in [
            ("   ".to_owned(), Invalid::Empty),
            ("\u{0}PUBLIC control".to_owned(), Invalid::Control),
            ("\u{200B}\u{202E}".to_owned(), Invalid::Control),
            (too_long, Invalid::TooLong),
        ] {
            let mut rename = opened();
            edit(&mut rename, &title);
            assert_eq!(rename.notice(), &Notice::Invalid(reason));
            assert_eq!(rename.title(), title);
            let ticket = rename.ticket().unwrap();
            assert!(rename.apply(Action::Review(ticket), &context()).is_none());
            assert!(rename.review().is_none());
            assert_eq!(rename.notice(), &Notice::Invalid(reason));
            assert_eq!(rename.title(), title);
        }
        // Exactly the documented byte bound is accepted.
        let mut rename = opened();
        let title = "é".repeat(MAX_TITLE_BYTES / 2);
        edit(&mut rename, &title);
        assert_eq!(rename.notice(), &Notice::None);
        let review = review(&mut rename);
        assert!(rename.apply(Action::Confirm(review), &context()).is_some());
    }

    #[test]
    fn review_is_required_and_confirm_consumes_one_exact_draft() {
        let mut rename = opened();
        edit(&mut rename, "PUBLIC replacement title");
        let forged = Review {
            ticket: rename.ticket().unwrap(),
            editor: rename.editor,
        };
        assert!(rename.apply(Action::Confirm(forged), &context()).is_none());
        assert_eq!(rename.title(), "PUBLIC replacement title");
        let reviewed = review(&mut rename);
        let replay = Action::Confirm(reviewed.clone());
        let submission = rename.apply(replay.clone(), &context()).unwrap();
        assert_eq!(submission.title(), "PUBLIC replacement title");
        assert!(rename.review().is_none());
        assert_eq!(rename.pending(), Some(&submission.receipt));
        assert_eq!(rename.notice(), &Notice::Pending);
        // A second rename is refused while this one is in flight.
        assert!(rename.apply(replay, &context()).is_none());
        edit(&mut rename, "PUBLIC later draft");
        let ticket = rename.ticket().unwrap();
        assert!(rename.apply(Action::Review(ticket), &context()).is_none());
        assert!(rename.review().is_none());
        assert_eq!(rename.notice(), &Notice::Pending);
        assert_eq!(rename.title(), "PUBLIC later draft");
    }

    #[test]
    fn back_returns_to_the_editor_with_the_typed_text() {
        let mut rename = opened();
        edit(&mut rename, "PUBLIC typed replacement");
        let review = review(&mut rename);
        let ticket = rename.ticket().unwrap();
        assert!(
            rename
                .apply(Action::Back(ticket.clone()), &context())
                .is_none()
        );
        assert!(rename.review().is_none());
        assert_eq!(rename.title(), "PUBLIC typed replacement");
        assert_eq!(rename.notice(), &Notice::None);
        // A retired review cannot be replayed into a write.
        assert!(rename.apply(Action::Confirm(review), &context()).is_none());
        assert!(rename.pending().is_none());
    }

    #[test]
    fn acknowledged_title_is_returned_once_and_never_locally_predicted() {
        let mut rename = opened();
        let submission = submit(&mut rename, "PUBLIC typed replacement");
        // No local rename happened before the ACK.
        assert_eq!(rename.title(), "PUBLIC typed replacement");
        assert!(rename.pending().is_some());
        let receipt = submission.receipt.clone();
        let applied = rename
            .finish(
                &receipt,
                Outcome::Renamed {
                    title: "PUBLIC normalized replacement".into(),
                    seq: 9,
                },
            )
            .unwrap();
        assert_eq!(applied.target, id("PUBLIC-rename-target"));
        assert_eq!(applied.title, "PUBLIC normalized replacement");
        assert_eq!(applied.seq, 9);
        assert_eq!(
            rename.notice(),
            &Notice::Renamed {
                title: "PUBLIC normalized replacement".into()
            }
        );
        assert!(rename.pending().is_none());
        // A duplicate ACK for the retired receipt changes nothing.
        assert!(
            rename
                .finish(
                    &receipt,
                    Outcome::Renamed {
                        title: "PUBLIC forged replacement".into(),
                        seq: 10
                    }
                )
                .is_none()
        );
        assert_ne!(
            rename.notice(),
            &Notice::Renamed {
                title: "PUBLIC forged replacement".into()
            }
        );
    }

    #[test]
    fn malformed_or_negative_ack_is_never_installed_as_a_title() {
        for outcome in [
            Outcome::Renamed {
                title: "   ".into(),
                seq: 4,
            },
            Outcome::Renamed {
                title: "PUBLIC\ncontrol".into(),
                seq: 4,
            },
            Outcome::Renamed {
                title: "PUBLIC ok".into(),
                seq: -1,
            },
        ] {
            let mut rename = opened();
            let submission = submit(&mut rename, "PUBLIC typed replacement");
            assert!(rename.finish(&submission.receipt, outcome).is_none());
            assert_eq!(rename.notice(), &Notice::Indeterminate);
        }
    }

    #[test]
    fn stale_target_epoch_generation_and_editor_refuse_without_writing() {
        let mutations: [fn(&mut View); 3] = [
            |view| view.epoch += 1,
            |view| view.generation += 1,
            |view| view.target = id("PUBLIC-other-target"),
        ];
        for mutate in mutations {
            let mut rename = opened();
            edit(&mut rename, "PUBLIC typed replacement");
            let review = review(&mut rename);
            let mut stale = context();
            mutate(stale.view.as_mut().unwrap());
            assert!(rename.apply(Action::Confirm(review), &stale).is_none());
            assert!(rename.pending().is_none());
            let mut denied = context();
            denied.allowed = false;
            let ticket = rename.ticket().unwrap();
            assert!(
                rename
                    .apply(
                        Action::Edit {
                            ticket,
                            title: "PUBLIC forged".into()
                        },
                        &denied
                    )
                    .is_none()
            );
            assert_eq!(rename.title(), "PUBLIC typed replacement");
        }
    }

    #[test]
    fn unknown_outcome_is_surfaced_without_retry_or_a_second_admission() {
        let mut rename = opened();
        let submission = submit(&mut rename, "PUBLIC typed replacement");
        assert!(
            rename
                .finish(&submission.receipt, Outcome::Unknown)
                .is_none()
        );
        assert_eq!(rename.notice(), &Notice::Indeterminate);
        assert!(rename.pending().is_none());
        assert_eq!(rename.title(), "PUBLIC typed replacement");
        // Retiring the flight does not re-arm the consumed review; a new explicit
        // review is required before any further write.
        let stale = Review {
            ticket: submission.receipt.ticket.clone(),
            editor: submission.receipt.editor,
        };
        assert!(rename.apply(Action::Confirm(stale), &context()).is_none());
        assert!(rename.pending().is_none());
        let mut rename = opened();
        let submission = submit(&mut rename, "PUBLIC typed replacement");
        assert!(
            rename
                .finish(&submission.receipt, Outcome::NotSent)
                .is_none()
        );
        assert_eq!(rename.notice(), &Notice::NotSent);
    }

    #[test]
    fn disconnect_fences_late_possessive_acks_and_keeps_titles_local() {
        let mut rename = opened();
        let submission = submit(&mut rename, "PUBLIC typed replacement");
        rename.disconnect();
        assert!(rename.pending().is_none());
        assert_eq!(rename.notice(), &Notice::Indeterminate);
        assert!(
            rename
                .finish(
                    &submission.receipt,
                    Outcome::Renamed {
                        title: "PUBLIC late".into(),
                        seq: 11
                    }
                )
                .is_none()
        );
    }
}
