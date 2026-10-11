//! Private unsent-text persistence; admitted blocking IO is serialized and joined.
use super::*;
use draft_recovery::{Kind, Outcome, Ticket};
use draft_store::{Drafts, Failure};
impl App {
    pub(super) fn capture_draft(&mut self) {
        if self.options.smoke.is_some() {
            return;
        }
        let Some(session) = self.selected.clone() else {
            return;
        };
        let text = self.editor.text();
        self.draft_versions
            .entry(session.clone())
            .or_insert(self.draft_serial);
        if self.drafts.get(&session).map(String::as_str).unwrap_or("") == text {
            return;
        }
        self.touch_draft();
    }
    pub(super) fn touch_draft(&mut self) {
        if self.options.smoke.is_some() {
            return;
        }
        if let Some(session) = self.selected.clone() {
            let text = self.editor.text();
            self.draft_versions
                .insert(session.clone(), self.draft_serial);
            if text.is_empty() {
                self.draft_recovered.remove(&session);
                self.drafts.remove(&session);
            } else {
                self.drafts.insert(session.clone(), text);
            }
            self.draft_state.touch(&session);
        }
    }
    pub(super) fn draft_task(&self, ticket: Ticket, snapshot: Option<Drafts>) -> Task<Message> {
        Task::perform(
            operation(ticket, self.draft_store.clone(), snapshot),
            Message::DraftOutcome,
        )
    }
    pub(super) fn load_drafts(&mut self, explicit: bool) -> Task<Message> {
        if self.options.smoke.is_some() {
            return Task::none();
        }
        let ticket = if explicit {
            self.draft_state.reload()
        } else {
            self.draft_state.start_load()
        };
        ticket.map_or_else(Task::none, |ticket| self.draft_task(ticket, None))
    }
    pub(super) fn flush_drafts(&mut self) -> Task<Message> {
        if self.options.smoke.is_some() {
            return Task::none();
        }
        self.capture_draft();
        let Some(ticket) = self.draft_state.next_save() else {
            return Task::none();
        };
        self.draft_task(ticket, Some(self.draft_snapshot()))
    }
    pub(super) fn draft_snapshot(&self) -> Drafts {
        let mut snapshot = self.drafts.clone();
        if let Some(session) = &self.selected {
            let text = self.editor.text();
            if text.is_empty() {
                snapshot.remove(session);
            } else {
                snapshot.insert(session.clone(), text);
            }
        }
        for original in self.pending.values() {
            if self.draft_versions.get(&original.session) == Some(&original.draft_serial)
                && snapshot.get(&original.session) == Some(&original.text)
            {
                snapshot.remove(&original.session);
            }
        }
        snapshot.retain(|_, text| !text.is_empty());
        snapshot
    }
    pub(super) fn drafts_completed(&mut self, outcome: Outcome) -> Task<Message> {
        let ticket = match &outcome {
            Outcome::Loaded { ticket, .. } | Outcome::Saved { ticket, .. } => *ticket,
        };
        if self.draft_state.pending() != Some(ticket) {
            return Task::none();
        }
        if let Some(loaded) = self.draft_state.apply(outcome) {
            self.drafts.retain(|session, _| {
                self.draft_state.touched(session) || loaded.contains_key(session)
            });
            self.draft_recovered
                .retain(|session| self.drafts.contains_key(session));
            for (session, text) in loaded {
                if !self.draft_state.touched(&session) {
                    self.draft_recovered.insert(session.clone());
                    self.drafts.insert(session, text);
                }
            }
            if let Some(session) = self.selected.as_ref()
                && !self.draft_state.touched(session)
                && self.editor.text().is_empty()
                && self.pending.is_empty()
                && let Some(text) = self.drafts.get(session)
            {
                self.editor = text_editor::Content::with_text(text);
                self.draft_serial = self.draft_serial.saturating_add(1);
            }
        }
        if self.draft_exit_requested {
            self.finish_draft_exit()
        } else {
            Task::none()
        }
    }
    pub(super) fn finish_draft_exit(&mut self) -> Task<Message> {
        if self.options.smoke.is_some() {
            return iced::exit();
        }
        if self.draft_state.busy() || self.local_files.busy() || self.sidebar_organization.busy() {
            return Task::none();
        }
        // A completed draft save cannot acknowledge an uncertain backend stop.
        if self.stopped && self.last_stop.as_ref().is_some_and(|result|!matches!(result,Ok(stop) if stop.exited && !stop.containment_unknown && stop.observed_descendants_remaining==0)){return Task::none();}
        if self.draft_discard_on_exit && self.stopped {
            return iced::exit();
        }
        if self.draft_state.failure().is_some() || self.sidebar_organization.failure().is_some() {
            self.close=Close::Failed("Draft or sidebar persistence failed or was not durably confirmed. Stay to reconcile, or explicitly exit without saving. Closing is not rollback.".into());
            return Task::none();
        }
        if self.draft_state.needs_flush() {
            if self.draft_state.ready() {
                return self.flush_drafts();
            }
            self.close=Close::Failed("Draft recovery was not loaded; local text is not saved. Stay or explicitly exit without saving.".into());
            return Task::none();
        }
        if self.stopped && self.draft_state.can_exit() {
            iced::exit()
        } else {
            Task::none()
        }
    }
    pub(super) fn recovered_draft_needs_review(&self) -> bool {
        self.selected
            .as_ref()
            .is_some_and(|session| self.draft_recovered.contains(session))
            && !self.editor.text().is_empty()
            && self.draft_review != Some((self.generation, self.draft_serial))
    }
    pub(super) fn draft_load_allows_send(&self) -> bool {
        !self.draft_state.is_loading() && !self.recovered_draft_needs_review()
    }
}
// Same function driven by App Tasks and PUBLIC disk integration tests; no second executor/model.
pub(super) async fn operation(
    ticket: Ticket,
    store: Option<std::sync::Arc<std::sync::Mutex<draft_store::Store>>>,
    snapshot: Option<Drafts>,
) -> Outcome {
    let joined = tokio::task::spawn_blocking(move || match (ticket.kind, store) {
        (Kind::Load, Some(store)) => Outcome::Loaded {
            ticket,
            result: store
                .lock()
                .map_err(|_| Failure::Unavailable)
                .and_then(|mut store| store.load()),
        },
        (Kind::Save, Some(store)) => Outcome::Saved {
            ticket,
            result: store
                .lock()
                .map_err(|_| Failure::Indeterminate)
                .and_then(|mut store| store.save(&snapshot.unwrap_or_default())),
        },
        (Kind::Load, None) => Outcome::Loaded {
            ticket,
            result: Err(Failure::Unavailable),
        },
        (Kind::Save, None) => Outcome::Saved {
            ticket,
            result: Err(Failure::NotSaved),
        },
    })
    .await;
    joined.unwrap_or(match ticket.kind {
        Kind::Load => Outcome::Loaded {
            ticket,
            result: Err(Failure::Unavailable),
        },
        Kind::Save => Outcome::Saved {
            ticket,
            result: Err(Failure::Indeterminate),
        },
    })
}
