//! Operator-reviewed root export admission. Paths stay in this editor or an owned submission.
//! Closing never cancels an admitted save; receipts cannot replace another panel's draft.
use crate::export_file::Destination;
pub use crate::management::{Ticket, View};

const MAX_PATH_BYTES: usize = 4096;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Saved { bytes: usize },
    NotSent,
    DownloadFailed,
    NotCreated,
    MayRemain,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    None,
    ReviewChanged,
    InvalidPath,
    Pending,
    Saved { bytes: usize },
    NotSent,
    DownloadFailed,
    NotCreated,
    MayRemain,
}
impl From<Outcome> for Notice {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Saved { bytes } => Self::Saved { bytes },
            Outcome::NotSent => Self::NotSent,
            Outcome::DownloadFailed => Self::DownloadFailed,
            Outcome::NotCreated => Self::NotCreated,
            Outcome::MayRemain => Self::MayRemain,
        }
    }
}

// Cloning an input is not permission to submit it. Every admission checks its current ticket.
#[derive(Clone)]
pub enum Action {
    Open(View),
    Close(Ticket),
    Edit { ticket: Ticket, path: String },
    Review(Ticket),
    Confirm(Review),
}
pub struct Context {
    pub view: Option<View>,
    pub allowed: bool,
}

/// One consuming operator-selected destination. No archive bytes or remote filename is retained.
pub struct Submission {
    pub receipt: Receipt,
    destination: Destination,
}
impl Submission {
    pub fn into_parts(self) -> (Receipt, Destination) {
        (self.receipt, self.destination)
    }
}
struct Panel {
    ticket: Ticket,
    editor: u64,
}

pub struct Exporter {
    panel: Option<Panel>,
    path: String,
    review: Option<Review>,
    pending: Option<Receipt>,
    notice: Notice,
    serial: u64,
    editor: u64,
    attempt: u64,
    exhausted: bool,
}
impl Default for Exporter {
    fn default() -> Self {
        Self {
            panel: None,
            path: String::new(),
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
impl Exporter {
    pub fn ticket(&self) -> Option<Ticket> {
        self.panel.as_ref().map(|panel| panel.ticket.clone())
    }
    /// Only the operator's entered path; no Host path, archive content, or inferred filename.
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn review(&self) -> Option<&Review> {
        self.review.as_ref()
    }
    pub fn pending(&self) -> Option<&Receipt> {
        self.pending.as_ref()
    }
    pub fn notice(&self) -> Notice {
        self.notice
    }
    pub fn is_open(&self) -> bool {
        self.panel.is_some()
    }
    /// Hide and clear the editor without cancelling or rolling back an admitted operation.
    pub fn dismiss(&mut self) {
        self.panel = None;
        self.path.clear();
        self.review = None;
        self.notice = Notice::None;
    }
    /// Fence late receipts; an admitted save may already have created a file.
    pub fn disconnect(&mut self) {
        let admitted = self.pending.take().is_some() || self.notice == Notice::MayRemain;
        self.dismiss();
        if admitted {
            self.notice = Notice::MayRemain;
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
            Action::Open(view)
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
                if self.pending.is_some() {
                    self.notice = Notice::Pending;
                }
            }
            Action::Edit { ticket, path } if self.current(&ticket, context) => {
                let Some(editor) = self.editor.checked_add(1) else {
                    self.exhausted();
                    return None;
                };
                self.editor = editor;
                self.panel.as_mut().unwrap().editor = editor;
                let changed = self.review.take().is_some();
                self.path.clear();
                if path.len() > MAX_PATH_BYTES || path.chars().any(char::is_control) {
                    self.notice = Notice::InvalidPath;
                } else {
                    self.path = path;
                    self.notice = if self.pending.is_some() {
                        Notice::Pending
                    } else if changed {
                        Notice::ReviewChanged
                    } else {
                        Notice::None
                    };
                }
            }
            Action::Review(ticket) if self.current(&ticket, context) && self.pending.is_none() => {
                self.review = None;
                if Destination::parse(self.path.clone()).is_err() {
                    self.notice = Notice::InvalidPath;
                } else {
                    self.review = Some(Review {
                        ticket,
                        editor: self.panel.as_ref().unwrap().editor,
                    });
                    self.notice = Notice::None;
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
                // Validation is repeated on the consumed input, never a default/inferred path.
                let path = std::mem::take(&mut self.path);
                self.review = None;
                let Ok(destination) = Destination::parse(path) else {
                    self.notice = Notice::InvalidPath;
                    return None;
                };
                self.attempt = attempt;
                let receipt = Receipt {
                    ticket: review.ticket,
                    editor: review.editor,
                    attempt,
                };
                self.pending = Some(receipt.clone());
                self.notice = Notice::Pending;
                return Some(Submission {
                    receipt,
                    destination,
                });
            }
            _ => {}
        }
        None
    }
    /// Retire only the exact global receipt. Outcomes belong to the original unchanged editor.
    pub fn finish(&mut self, receipt: &Receipt, outcome: Outcome) -> bool {
        if self.pending.as_ref() != Some(receipt) {
            return false;
        }
        self.pending = None;
        if self
            .panel
            .as_ref()
            .is_some_and(|panel| panel.ticket == receipt.ticket && panel.editor == receipt.editor)
        {
            self.notice = outcome.into();
        } else if self.notice == Notice::Pending {
            self.notice = Notice::None;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_native_transport::dto::SessionId;

    const PUBLIC_PATH: &str = "/PUBLIC/export.zip";
    fn context() -> Context {
        Context {
            view: Some(View {
                epoch: 1,
                generation: 7,
                target: SessionId::new("PUBLIC-session").unwrap(),
            }),
            allowed: true,
        }
    }
    fn opened() -> Exporter {
        let mut exporter = Exporter::default();
        exporter.apply(Action::Open(context().view.unwrap()), &context());
        exporter
    }
    fn edit(exporter: &mut Exporter, path: &str) {
        let ticket = exporter.ticket().unwrap();
        assert!(
            exporter
                .apply(
                    Action::Edit {
                        ticket,
                        path: path.into()
                    },
                    &context()
                )
                .is_none()
        );
    }
    fn review(exporter: &mut Exporter) -> Review {
        let ticket = exporter.ticket().unwrap();
        assert!(exporter.apply(Action::Review(ticket), &context()).is_none());
        exporter.review().unwrap().clone()
    }
    fn submit(exporter: &mut Exporter) -> Submission {
        edit(exporter, PUBLIC_PATH);
        let review = review(exporter);
        exporter.apply(Action::Confirm(review), &context()).unwrap()
    }

    #[test]
    fn review_is_required_and_confirm_consumes_editor_and_destination_once() {
        let mut exporter = opened();
        edit(&mut exporter, PUBLIC_PATH);
        let forged = Review {
            ticket: exporter.ticket().unwrap(),
            editor: exporter.editor,
        };
        assert!(
            exporter
                .apply(Action::Confirm(forged), &context())
                .is_none()
        );
        assert_eq!(exporter.path(), PUBLIC_PATH);
        let reviewed = review(&mut exporter);
        let replay = Action::Confirm(reviewed.clone());
        let submission = exporter.apply(replay.clone(), &context()).unwrap();
        assert!(exporter.path().is_empty());
        assert!(exporter.review().is_none());
        assert_eq!(exporter.pending(), Some(&submission.receipt));
        assert_eq!(exporter.notice(), Notice::Pending);
        assert!(exporter.apply(replay, &context()).is_none());
        let (receipt, destination) = submission.into_parts();
        drop(destination); // This test creates no directory or file.
        assert!(exporter.finish(&receipt, Outcome::NotSent));
        assert_eq!(exporter.notice(), Notice::NotSent);
        assert!(exporter.path().is_empty());
        assert!(
            exporter
                .apply(Action::Confirm(reviewed), &context())
                .is_none()
        );
    }

    #[test]
    fn each_edit_invalidates_review_even_for_identical_path() {
        let mut exporter = opened();
        edit(&mut exporter, PUBLIC_PATH);
        let old = review(&mut exporter);
        edit(&mut exporter, PUBLIC_PATH);
        assert_eq!(exporter.notice(), Notice::ReviewChanged);
        assert!(exporter.review().is_none());
        assert!(
            exporter
                .apply(Action::Confirm(old.clone()), &context())
                .is_none()
        );
        let current = review(&mut exporter);
        assert!(current.editor > old.editor);
        assert!(exporter.apply(Action::Confirm(old), &context()).is_none());
        assert!(
            exporter
                .apply(Action::Confirm(current), &context())
                .is_some()
        );
    }

    #[test]
    fn invalid_pastes_clear_old_draft_without_trimming_or_path_logging() {
        let mut exporter = opened();
        for bad in [
            format!("/{}.zip", "a".repeat(MAX_PATH_BYTES)),
            "/PUBLIC/export.zip\n".into(),
            "/PUBLIC/\0.zip".into(),
            "/PUBLIC/\u{0085}.zip".into(),
        ] {
            edit(&mut exporter, PUBLIC_PATH);
            let old = review(&mut exporter);
            edit(&mut exporter, &bad);
            assert!(exporter.path().is_empty());
            assert_eq!(exporter.notice(), Notice::InvalidPath);
            assert!(exporter.review().is_none());
            assert!(exporter.apply(Action::Confirm(old), &context()).is_none());
        }
        let exact = format!("/{}.zip", "a".repeat(MAX_PATH_BYTES - 5));
        edit(&mut exporter, &exact);
        assert_eq!(exporter.path().len(), MAX_PATH_BYTES);
        assert!(
            exporter
                .apply(Action::Review(exporter.ticket().unwrap()), &context())
                .is_none()
        );
        assert!(exporter.review().is_some());
    }

    #[test]
    fn only_review_validates_absolute_new_zip_syntax_without_filesystem_io() {
        let mut exporter = opened();
        for bad in [
            "",
            "relative.zip",
            "/PUBLIC/a.txt",
            "/PUBLIC/a.ZIP",
            "/PUBLIC//a.zip",
            "/PUBLIC/../a.zip",
            " /PUBLIC/a.zip",
        ] {
            edit(&mut exporter, bad);
            assert_eq!(exporter.path(), bad);
            assert_eq!(exporter.notice(), Notice::None);
            exporter.apply(Action::Review(exporter.ticket().unwrap()), &context());
            assert!(exporter.review().is_none());
            assert_eq!(exporter.notice(), Notice::InvalidPath);
        }
        edit(&mut exporter, "/PUBLIC/Unicode-日本語.zip");
        let reviewed = review(&mut exporter);
        assert!(
            exporter
                .apply(Action::Confirm(reviewed), &context())
                .is_some()
        );
    }

    #[test]
    fn open_edit_review_and_confirm_require_allowed_current_view() {
        for mismatch in 0..5 {
            let mut exporter = opened();
            edit(&mut exporter, PUBLIC_PATH);
            let reviewed = review(&mut exporter);
            let ticket = exporter.ticket().unwrap();
            let mut denied = context();
            match mismatch {
                0 => denied.allowed = false,
                1 => denied.view = None,
                2 => denied.view.as_mut().unwrap().epoch += 1,
                3 => denied.view.as_mut().unwrap().generation += 1,
                _ => denied.view.as_mut().unwrap().target = SessionId::new("PUBLIC-other").unwrap(),
            }
            for action in [
                Action::Open(context().view.unwrap()),
                Action::Edit {
                    ticket: ticket.clone(),
                    path: "/PUBLIC/changed.zip".into(),
                },
                Action::Review(ticket.clone()),
                Action::Confirm(reviewed.clone()),
            ] {
                assert!(exporter.apply(action, &denied).is_none());
                assert_eq!(exporter.ticket(), Some(ticket.clone()));
                assert_eq!(exporter.path(), PUBLIC_PATH);
                assert_eq!(exporter.review(), Some(&reviewed));
                assert!(exporter.pending().is_none());
            }
            exporter.apply(Action::Close(ticket), &denied);
            assert!(!exporter.is_open());
            assert!(exporter.path().is_empty());
        }
    }

    #[test]
    fn stale_ticket_and_review_fields_never_mutate_or_admit() {
        let mut exporter = opened();
        edit(&mut exporter, PUBLIC_PATH);
        let reviewed = review(&mut exporter);
        for field in 0..4 {
            let mut stale = reviewed.ticket.clone();
            match field {
                0 => stale.epoch += 1,
                1 => stale.generation += 1,
                2 => stale.serial += 1,
                _ => stale.target = SessionId::new("PUBLIC-other").unwrap(),
            }
            for action in [
                Action::Close(stale.clone()),
                Action::Edit {
                    ticket: stale.clone(),
                    path: "/PUBLIC/stale.zip".into(),
                },
                Action::Review(stale.clone()),
                Action::Confirm(Review {
                    ticket: stale,
                    editor: reviewed.editor,
                }),
            ] {
                assert!(exporter.apply(action, &context()).is_none());
                assert_eq!(exporter.path(), PUBLIC_PATH);
                assert_eq!(exporter.review(), Some(&reviewed));
                assert!(exporter.is_open());
            }
        }
        let mut stale = reviewed.clone();
        stale.editor += 1;
        assert!(exporter.apply(Action::Confirm(stale), &context()).is_none());
        assert_eq!(exporter.review(), Some(&reviewed));
    }

    #[test]
    fn close_reopen_fences_queued_panel_actions_and_reviews() {
        let mut exporter = opened();
        edit(&mut exporter, PUBLIC_PATH);
        let old = review(&mut exporter);
        exporter.dismiss();
        exporter.apply(Action::Open(context().view.unwrap()), &context());
        assert!(exporter.ticket().unwrap().serial > old.ticket.serial);
        assert!(exporter.path().is_empty());
        assert_eq!(exporter.notice(), Notice::None);
        for action in [
            Action::Close(old.ticket.clone()),
            Action::Edit {
                ticket: old.ticket.clone(),
                path: PUBLIC_PATH.into(),
            },
            Action::Review(old.ticket.clone()),
            Action::Confirm(old),
        ] {
            assert!(exporter.apply(action, &context()).is_none());
            assert!(exporter.is_open());
            assert!(exporter.path().is_empty());
        }
    }

    #[test]
    fn one_global_pending_survives_close_reopen_and_preserves_new_draft() {
        let mut exporter = opened();
        let submission = submit(&mut exporter);
        exporter.dismiss();
        assert_eq!(exporter.pending(), Some(&submission.receipt));
        exporter.apply(Action::Open(context().view.unwrap()), &context());
        assert_eq!(exporter.notice(), Notice::Pending);
        edit(&mut exporter, "/PUBLIC/new.zip");
        exporter.apply(Action::Review(exporter.ticket().unwrap()), &context());
        assert!(exporter.review().is_none());
        assert_eq!(exporter.path(), "/PUBLIC/new.zip");
        assert!(exporter.finish(&submission.receipt, Outcome::Saved { bytes: 321 }));
        assert_eq!(exporter.notice(), Notice::None);
        assert_eq!(exporter.path(), "/PUBLIC/new.zip");
        assert!(exporter.pending().is_none());
        let reviewed = review(&mut exporter);
        let next = exporter
            .apply(Action::Confirm(reviewed), &context())
            .unwrap();
        assert!(next.receipt.attempt > submission.receipt.attempt);
        assert!(!exporter.finish(&submission.receipt, Outcome::MayRemain));
        assert_eq!(exporter.pending(), Some(&next.receipt));
    }

    #[test]
    fn global_pending_blocks_another_target_and_only_matching_receipt_releases_it() {
        let mut exporter = opened();
        let submission = submit(&mut exporter);
        let old = submission.receipt.ticket.clone();
        let mut other = context();
        other.view.as_mut().unwrap().target = SessionId::new("PUBLIC-other").unwrap();
        exporter.apply(Action::Open(other.view.clone().unwrap()), &other);
        let ticket = exporter.ticket().unwrap();
        exporter.apply(
            Action::Edit {
                ticket: ticket.clone(),
                path: "/PUBLIC/other.zip".into(),
            },
            &other,
        );
        exporter.apply(Action::Close(old.clone()), &other);
        assert!(exporter.is_open());
        assert!(
            exporter
                .apply(Action::Review(ticket.clone()), &other)
                .is_none()
        );
        assert!(exporter.review().is_none());
        assert!(
            exporter
                .apply(
                    Action::Confirm(Review {
                        ticket: old,
                        editor: submission.receipt.editor
                    }),
                    &other
                )
                .is_none()
        );
        assert_eq!(exporter.pending(), Some(&submission.receipt));
        assert!(exporter.finish(&submission.receipt, Outcome::NotCreated));
        assert_eq!(exporter.path(), "/PUBLIC/other.zip");
        assert_eq!(exporter.notice(), Notice::None);
        exporter.apply(Action::Review(ticket), &other);
        let reviewed = exporter.review().unwrap().clone();
        let next = exporter.apply(Action::Confirm(reviewed), &other).unwrap();
        assert_eq!(next.receipt.ticket.target, other.view.unwrap().target);
        assert!(next.receipt.attempt > submission.receipt.attempt);
    }

    #[test]
    fn confirm_revalidates_and_consumes_even_a_changed_internal_path() {
        let mut exporter = opened();
        edit(&mut exporter, PUBLIC_PATH);
        let reviewed = review(&mut exporter);
        // A normal edit invalidates the Review; this private fault injects the second parser guard.
        exporter.path = "relative.zip".into();
        assert!(
            exporter
                .apply(Action::Confirm(reviewed), &context())
                .is_none()
        );
        assert!(exporter.path().is_empty());
        assert!(exporter.review().is_none());
        assert!(exporter.pending().is_none());
        assert_eq!(exporter.notice(), Notice::InvalidPath);
    }

    #[test]
    fn late_outcome_cannot_overwrite_edited_original_panel_or_invalid_paste_notice() {
        let mut exporter = opened();
        let submission = submit(&mut exporter);
        edit(&mut exporter, "/PUBLIC/new.zip");
        assert!(exporter.finish(&submission.receipt, Outcome::MayRemain));
        assert_eq!(exporter.path(), "/PUBLIC/new.zip");
        assert_eq!(exporter.notice(), Notice::None);
        let submission = submit(&mut exporter);
        edit(&mut exporter, "/PUBLIC/bad.zip\n");
        assert!(exporter.finish(&submission.receipt, Outcome::Saved { bytes: 321 }));
        assert!(exporter.path().is_empty());
        assert_eq!(exporter.notice(), Notice::InvalidPath);
    }

    #[test]
    fn wrong_receipts_do_not_clear_global_pending_or_publish_results() {
        let mut exporter = opened();
        let submission = submit(&mut exporter);
        for field in 0..6 {
            let mut wrong = submission.receipt.clone();
            match field {
                0 => wrong.ticket.epoch += 1,
                1 => wrong.ticket.generation += 1,
                2 => wrong.ticket.serial += 1,
                3 => wrong.ticket.target = SessionId::new("PUBLIC-other").unwrap(),
                4 => wrong.editor += 1,
                _ => wrong.attempt += 1,
            }
            assert!(!exporter.finish(&wrong, Outcome::Saved { bytes: 321 }));
            assert_eq!(exporter.pending(), Some(&submission.receipt));
            assert_eq!(exporter.notice(), Notice::Pending);
        }
        assert!(exporter.finish(&submission.receipt, Outcome::NotSent));
        assert!(!exporter.finish(&submission.receipt, Outcome::MayRemain));
        assert_eq!(exporter.notice(), Notice::NotSent);
    }

    #[test]
    fn each_exact_outcome_is_metadata_only_and_reopening_forgets_it() {
        for outcome in [
            Outcome::Saved { bytes: 321 },
            Outcome::NotSent,
            Outcome::DownloadFailed,
            Outcome::NotCreated,
            Outcome::MayRemain,
        ] {
            let mut exporter = opened();
            let submission = submit(&mut exporter);
            assert!(exporter.finish(&submission.receipt, outcome));
            assert_eq!(exporter.notice(), Notice::from(outcome));
            assert!(exporter.path().is_empty());
            exporter.apply(Action::Open(context().view.unwrap()), &context());
            assert_eq!(exporter.notice(), Notice::None);
            assert!(!exporter.finish(&submission.receipt, Outcome::MayRemain));
        }
    }

    #[test]
    fn disconnected_admission_is_indeterminate_and_late_results_are_fenced() {
        let mut exporter = opened();
        let submission = submit(&mut exporter);
        exporter.dismiss();
        exporter.disconnect();
        assert_eq!(exporter.notice(), Notice::MayRemain);
        assert!(exporter.pending().is_none());
        assert!(!exporter.is_open());
        assert!(exporter.path().is_empty());
        assert!(!exporter.finish(&submission.receipt, Outcome::Saved { bytes: 321 }));
        exporter.disconnect();
        assert_eq!(exporter.notice(), Notice::MayRemain);
        let mut fresh = context();
        fresh.view.as_mut().unwrap().epoch += 1;
        fresh.view.as_mut().unwrap().generation += 1;
        exporter.apply(Action::Open(fresh.view.clone().unwrap()), &fresh);
        assert_eq!(exporter.notice(), Notice::None);
        let ticket = exporter.ticket().unwrap();
        exporter.apply(
            Action::Edit {
                ticket: ticket.clone(),
                path: PUBLIC_PATH.into(),
            },
            &fresh,
        );
        exporter.apply(Action::Review(ticket), &fresh);
        let reviewed = exporter.review().unwrap().clone();
        let next = exporter.apply(Action::Confirm(reviewed), &fresh).unwrap();
        assert!(next.receipt.attempt > submission.receipt.attempt);
        assert!(!exporter.finish(&submission.receipt, Outcome::NotSent));
        assert_eq!(exporter.pending(), Some(&next.receipt));
    }

    #[test]
    fn disconnected_unsubmitted_draft_is_cleared_without_claiming_a_file() {
        let mut exporter = opened();
        edit(&mut exporter, PUBLIC_PATH);
        review(&mut exporter);
        exporter.disconnect();
        assert_eq!(exporter.notice(), Notice::None);
        assert!(!exporter.is_open());
        assert!(exporter.path().is_empty());
        assert!(exporter.review().is_none());
    }

    #[test]
    fn counters_fail_closed_instead_of_reusing_tickets_editors_or_attempts() {
        for counter in 0..3 {
            let mut exporter = opened();
            match counter {
                0 => {
                    exporter.serial = u64::MAX;
                    exporter.apply(Action::Open(context().view.unwrap()), &context());
                }
                1 => {
                    exporter.editor = u64::MAX;
                    edit(&mut exporter, PUBLIC_PATH);
                }
                _ => {
                    edit(&mut exporter, PUBLIC_PATH);
                    let reviewed = review(&mut exporter);
                    exporter.attempt = u64::MAX;
                    assert!(
                        exporter
                            .apply(Action::Confirm(reviewed), &context())
                            .is_none()
                    );
                }
            }
            assert!(!exporter.is_open());
            assert!(exporter.path().is_empty());
            assert!(exporter.review().is_none());
            assert!(
                exporter
                    .apply(Action::Open(context().view.unwrap()), &context())
                    .is_none()
            );
            assert!(!exporter.is_open());
        }
    }
}
