//! Durable local preferences only. No quota observer, notification or retry executor is wired.
use super::{Outcome, Ticket};
use dsh_native_core::{
    CodexComposerDisplay, CodexRemainingPercent, CodexUsageAlert, CodexUsagePreferences,
    CodexUsageSnapshot,
};

#[cfg(test)]
#[path = "codex_usage_tests.rs"]
mod tests;

#[derive(Clone, Debug)]
pub enum Action {
    Refresh,
    Save,
    Composer(CodexComposerDisplay),
    Alert(CodexUsageAlert),
    /// Reserved for an alert-threshold edit once the Host exposes one; the
    /// native surface rejects it today.
    #[allow(dead_code)]
    Threshold(String),
    Retry(bool),
    Consent(bool),
}
#[derive(Clone)]
enum Request {
    Read,
    Save {
        revision: u64,
        preferences: CodexUsagePreferences,
        edit: u64,
    },
}
struct Pending {
    ticket: Ticket,
    request: Request,
}
#[derive(Clone, Copy)]
enum Notice {
    Idle,
    Reading,
    Saving,
    Loaded,
    Saved,
    Invalid,
    Failed,
    Conflict,
    Unknown,
    Unavailable,
}
impl Notice {
    fn reconciliation_required(self) -> bool {
        matches!(self, Self::Conflict | Self::Unknown | Self::Unavailable)
    }
    fn text(self) -> &'static str {
        match self {
            Self::Idle => "Preferences not read. Refresh before saving.",
            Self::Reading => "Reading local preferences…",
            Self::Saving => "Saving draft… not yet acknowledged.",
            Self::Loaded => "Local preferences read. Draft changes are not saved.",
            Self::Saved => "Preferences saved locally. Usage effects are not connected.",
            Self::Invalid => "Enter a remaining percentage from 0 to 100; fractions are allowed.",
            Self::Failed => {
                "Save failed: draft retained; nothing acknowledged. Retry only explicitly."
            }
            Self::Conflict => {
                "Save failed: revision changed. Draft retained; Refresh before saving again."
            }
            Self::Unknown => {
                "Save outcome unknown. Draft retained; Refresh to reconcile before another Save."
            }
            Self::Unavailable => {
                "Preferences unavailable; draft retained. No cached fallback. Refresh to reconcile; storage may require an app restart."
            }
        }
    }
}
pub(super) struct Usage {
    pub(super) snapshot: Option<CodexUsageSnapshot>,
    pub(super) draft: CodexUsagePreferences,
    pub(super) threshold: String,
    pending: Option<Pending>,
    fresh: bool,
    dirty: bool,
    edit: u64,
    revoked: bool,
    consent_ack: bool,
    notice: Notice,
    pub(super) initial_attempted: bool,
    pub(super) initial_queued: bool,
}
impl Default for Usage {
    fn default() -> Self {
        Self {
            snapshot: None,
            draft: CodexUsagePreferences::default(),
            threshold: "20".into(),
            pending: None,
            fresh: false,
            dirty: false,
            edit: 0,
            revoked: false,
            consent_ack: false,
            notice: Notice::Idle,
            initial_attempted: false,
            initial_queued: false,
        }
    }
}
impl Usage {
    pub(super) fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub(super) fn ticket(&self) -> Option<Ticket> {
        self.pending.as_ref().map(|p| p.ticket)
    }
    pub(super) fn status(&self) -> &'static str {
        self.notice.text()
    }
    pub(super) fn attention(&self) -> bool {
        matches!(
            self.notice,
            Notice::Invalid
                | Notice::Failed
                | Notice::Conflict
                | Notice::Unknown
                | Notice::Unavailable
        )
    }
    pub(super) fn read_pending_for(&self, ticket: Ticket) -> bool {
        self.pending.as_ref().is_some_and(|pending| {
            pending.ticket == ticket && matches!(&pending.request, Request::Read)
        })
    }
    /// Native surface cannot edit this scalar; retain opt-out drafts while using the
    /// current owned read's threshold after CAS conflict/reconciliation, not a stale draft.
    pub(super) fn rebase_native_read_only_threshold(&mut self) {
        if self.fresh
            && !self.pending()
            && let Some(snapshot) = &self.snapshot
        {
            self.draft.custom_remaining_percent = snapshot.preferences.custom_remaining_percent;
            self.threshold = self.draft.custom_remaining_percent.as_f64().to_string();
        }
    }
    pub(super) fn can_save(&self) -> bool {
        self.fresh && !self.pending() && self.valid_threshold()
    }
    pub(super) fn valid_threshold(&self) -> bool {
        self.threshold
            .parse::<f64>()
            .ok()
            .and_then(|v| CodexRemainingPercent::new(v).ok())
            .is_some()
    }
    /// Future consumers must use this gate, not persisted experimental_quota alone.
    /// This permission does not start a CLI, quota query, notification or retry.
    pub(super) fn effective_consent(&self) -> bool {
        self.fresh
            && !self.revoked
            && self.consent_ack
            && self.draft.experimental_quota
            && self
                .snapshot
                .as_ref()
                .is_some_and(|s| s.persisted && s.preferences.experimental_quota)
    }
    pub(super) fn edit(&mut self, action: Action) {
        // Revocation remains available while a read/write is pending, and fences immediately.
        if self.pending() && !matches!(action, Action::Consent(false)) {
            return;
        }
        match action {
            Action::Composer(value) => self.draft.composer_display = value,
            Action::Alert(value) => self.draft.usage_alert = value,
            Action::Threshold(value) => {
                // Bound retained text without letting an oversized invalid edit save the old value.
                self.threshold = if value.len() > 64 {
                    String::new()
                } else {
                    value
                };
                if let Some(percent) = self
                    .threshold
                    .parse::<f64>()
                    .ok()
                    .and_then(|v| CodexRemainingPercent::new(v).ok())
                {
                    self.draft.custom_remaining_percent = percent;
                    if matches!(self.notice, Notice::Invalid) {
                        self.notice = if self.fresh {
                            Notice::Loaded
                        } else {
                            Notice::Idle
                        };
                    }
                } else if !self.notice.reconciliation_required() {
                    // Local validation must not hide a possibly committed write or failed read.
                    self.notice = Notice::Invalid;
                }
            }
            Action::Retry(value) => self.draft.retry_after_reset = value,
            Action::Consent(value) => {
                self.draft.experimental_quota = value;
                self.consent_ack = false;
                // A new explicit opt-in is only armed by its exact matching Save ACK.
                self.revoked = true;
            }
            Action::Refresh | Action::Save => return,
        }
        self.edit += 1;
        self.dirty = true;
    }
    pub(super) fn read(&mut self, ticket: Ticket) {
        self.initial_attempted = true;
        self.initial_queued = false;
        self.pending = Some(Pending {
            ticket,
            request: Request::Read,
        });
        self.notice = Notice::Reading;
    }
    pub(super) fn save(&mut self, ticket: Ticket) -> Option<(u64, CodexUsagePreferences)> {
        if !self.can_save() {
            return None;
        }
        let revision = self.snapshot.as_ref()?.revision;
        let preferences = self.draft;
        self.pending = Some(Pending {
            ticket,
            request: Request::Save {
                revision,
                preferences,
                edit: self.edit,
            },
        });
        self.notice = Notice::Saving;
        Some((revision, preferences))
    }
    pub(super) fn disconnect(&mut self) {
        if let Some(pending) = self.pending.take() {
            self.notice = match pending.request {
                Request::Read => Notice::Unavailable,
                Request::Save { .. } => Notice::Unknown,
            };
        }
        self.fresh = false;
        self.consent_ack = false;
        self.initial_queued = false;
        // Keep the once-only attempt consumed across epochs; uncertainty needs explicit Refresh.
    }
    pub(super) fn complete(&mut self, ticket: Ticket, outcome: Outcome, owned: bool) {
        if self.ticket() != Some(ticket) {
            return;
        }
        let pending = self.pending.take().expect("matched preference operation");
        if !owned {
            self.fresh = false;
            self.consent_ack = false;
            self.notice = match pending.request {
                Request::Read => Notice::Unavailable,
                Request::Save { .. } => Notice::Unknown,
            };
            return;
        }
        match (pending.request, outcome) {
            (Request::Read, Outcome::UsageSnapshot(snapshot)) if snapshot.version == 1 => {
                if !self.dirty {
                    self.draft = snapshot.preferences;
                    self.threshold = self.draft.custom_remaining_percent.as_f64().to_string();
                    self.consent_ack = !self.revoked
                        && snapshot.persisted
                        && snapshot.preferences.experimental_quota;
                }
                self.snapshot = Some(snapshot);
                self.fresh = true;
                self.notice = Notice::Loaded;
            }
            (
                Request::Save {
                    revision,
                    preferences,
                    edit,
                },
                Outcome::UsageSnapshot(snapshot),
            ) if snapshot.version == 1
                && snapshot.persisted
                && revision.checked_add(1) == Some(snapshot.revision)
                && snapshot.preferences == preferences =>
            {
                self.snapshot = Some(snapshot);
                self.fresh = true;
                if self.edit == edit {
                    self.dirty = false;
                    self.consent_ack = preferences.experimental_quota;
                    self.revoked = !preferences.experimental_quota;
                    self.notice = Notice::Saved;
                } else {
                    // A local opt-out after issuance fences an old true ACK and retains its draft.
                    self.notice = Notice::Loaded;
                }
            }
            (Request::Save { .. }, Outcome::UsageConflict) => {
                self.fresh = false;
                self.notice = Notice::Conflict;
            }
            (Request::Save { .. }, Outcome::NotSent) => {
                self.notice = Notice::Failed;
            }
            (_, Outcome::UsageUnavailable) => {
                self.fresh = false;
                self.notice = Notice::Unavailable;
            }
            (Request::Read, _) => {
                self.fresh = false;
                self.notice = Notice::Unavailable;
            }
            (Request::Save { .. }, _) => {
                self.fresh = false;
                self.notice = Notice::Unknown;
            }
        }
    }
}
