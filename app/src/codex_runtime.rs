//! Shared, bounded runtime draft. Only explicit Refresh/Save can issue bridge work.
use super::{Outcome, Ticket};
use dsh_native_core::{
    CodexCacheRetention, CodexIdleMillis, CodexImageBudget, CodexRuntimeNatural,
    CodexRuntimeSnapshot, CodexRuntimeTransport, CodexRuntimeValues,
};

#[cfg(test)]
#[path = "codex_runtime_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Timeout,
    WebsocketConnectTimeout,
    StreamIdleTimeout,
    ImagePixelBudget,
    ImageMaxBytes,
    MaxRequestImageBytes,
}
impl Field {
    fn index(self) -> usize {
        match self {
            Self::Timeout => 0,
            Self::WebsocketConnectTimeout => 1,
            Self::StreamIdleTimeout => 2,
            Self::ImagePixelBudget => 3,
            Self::ImageMaxBytes => 4,
            Self::MaxRequestImageBytes => 5,
        }
    }
}
#[derive(Clone, Debug)]
pub enum Action {
    Refresh,
    Save,
    Reset,
    Transport(Option<CodexRuntimeTransport>),
    Cache(Option<CodexCacheRetention>),
    Number { field: Field, value: String },
}
#[derive(Clone)]
enum Request {
    Read,
    Save {
        revision: u64,
        /// Both value sets are boxed: together they dwarf every other variant,
        /// and this request is carried inside a `Result` back to the caller.
        values: Box<CodexRuntimeValues>,
        base: Box<CodexRuntimeValues>,
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
    Reset,
    Failed,
    Conflict,
    Unknown,
    Unavailable,
}

pub(super) struct Runtime {
    pub(super) snapshot: Option<CodexRuntimeSnapshot>,
    pub(super) transport: Option<CodexRuntimeTransport>,
    pub(super) cache: Option<CodexCacheRetention>,
    numbers: [String; 6],
    pending: Option<Pending>,
    fresh: bool,
    dirty: bool,
    notice: Notice,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            snapshot: None,
            transport: None,
            cache: None,
            numbers: std::array::from_fn(|_| String::new()),
            pending: None,
            fresh: false,
            dirty: false,
            notice: Notice::Idle,
        }
    }
}

// An unset draft field means the effective lower layer, not necessarily null.
pub(crate) fn ack_matches(
    after: CodexRuntimeSnapshot,
    revision: u64,
    draft: CodexRuntimeValues,
    base: CodexRuntimeValues,
) -> bool {
    let upper = revision.checked_add(1).unwrap_or(revision);
    after.version == 1
        && after.configured
        && after.revision >= revision
        && after.revision <= upper
        && after.base_values == base
        && after.values
            == CodexRuntimeValues {
                transport: draft.transport.or(after.base_values.transport),
                cache_retention: draft.cache_retention.or(after.base_values.cache_retention),
                timeout_ms: draft.timeout_ms.or(after.base_values.timeout_ms),
                websocket_connect_timeout_ms: draft
                    .websocket_connect_timeout_ms
                    .or(after.base_values.websocket_connect_timeout_ms),
                stream_idle_timeout_ms: draft
                    .stream_idle_timeout_ms
                    .or(after.base_values.stream_idle_timeout_ms),
                request_image_pixel_budget: draft
                    .request_image_pixel_budget
                    .or(after.base_values.request_image_pixel_budget),
                request_image_max_bytes: draft
                    .request_image_max_bytes
                    .or(after.base_values.request_image_max_bytes),
                max_request_image_bytes: draft
                    .max_request_image_bytes
                    .or(after.base_values.max_request_image_bytes),
            }
}
fn natural(raw: &str) -> Result<Option<CodexRuntimeNatural>, ()> {
    if raw.is_empty() {
        return Ok(None);
    }
    if !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(());
    }
    raw.parse::<u64>()
        .ok()
        .and_then(|v| CodexRuntimeNatural::new(v).ok())
        .map(Some)
        .ok_or(())
}
fn budget(raw: &str) -> Result<Option<CodexImageBudget>, ()> {
    if raw.is_empty() {
        return Ok(None);
    }
    if !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(());
    }
    raw.parse::<u64>()
        .ok()
        .and_then(|v| CodexImageBudget::new(v).ok())
        .map(Some)
        .ok_or(())
}
fn idle(raw: &str) -> Result<Option<CodexIdleMillis>, ()> {
    if raw.is_empty() {
        return Ok(None);
    }
    // Decimal notation only: no exponent, signs, whitespace, NaN or Infinity.
    if !raw.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return Err(());
    }
    raw.parse::<f64>()
        .ok()
        .and_then(|v| CodexIdleMillis::new(v).ok())
        .map(Some)
        .ok_or(())
}
impl Runtime {
    pub(super) fn number(&self, field: Field) -> &str {
        &self.numbers[field.index()]
    }
    pub(super) fn values(&self) -> Result<CodexRuntimeValues, ()> {
        Ok(CodexRuntimeValues {
            transport: self.transport,
            cache_retention: self.cache,
            timeout_ms: natural(&self.numbers[0])?,
            websocket_connect_timeout_ms: natural(&self.numbers[1])?,
            stream_idle_timeout_ms: idle(&self.numbers[2])?,
            request_image_pixel_budget: budget(&self.numbers[3])?,
            request_image_max_bytes: budget(&self.numbers[4])?,
            max_request_image_bytes: budget(&self.numbers[5])?,
        })
    }
    pub(super) fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub(super) fn saving(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|p| matches!(p.request, Request::Save { .. }))
    }
    pub(super) fn ticket(&self) -> Option<Ticket> {
        self.pending.as_ref().map(|p| p.ticket)
    }
    pub(super) fn can_save(&self) -> bool {
        self.fresh
            && !self.pending()
            && self.snapshot.is_some_and(|s| s.writable)
            && self.values().is_ok()
    }
    pub(super) fn attention(&self) -> bool {
        self.values().is_err()
            || matches!(
                self.notice,
                Notice::Failed | Notice::Conflict | Notice::Unknown | Notice::Unavailable
            )
    }
    pub(super) fn status(&self) -> &'static str {
        if self.values().is_err()
            && !matches!(
                self.notice,
                Notice::Reading
                    | Notice::Saving
                    | Notice::Conflict
                    | Notice::Unknown
                    | Notice::Unavailable
            )
        {
            return "Invalid runtime draft. Use whole numbers; only stream idle accepts decimals. Blank inherits. Draft not saved.";
        }
        match self.notice {
            Notice::Idle => "Runtime not read. Refresh before saving.",
            Notice::Reading => "Reading runtime settings… draft edits are retained.",
            Notice::Saving => "Runtime save pending… not yet acknowledged.",
            Notice::Loaded => "Runtime read. Draft changes are not saved.",
            Notice::Saved => "Runtime settings saved. No model request was sent.",
            Notice::Reset => "Draft reset to inherited defaults. Save explicitly to apply.",
            Notice::Failed => "Runtime not sent. Draft retained; retry only explicitly.",
            Notice::Conflict => {
                "Runtime revision changed. Draft retained; Refresh before another Save."
            }
            Notice::Unknown => {
                "Runtime outcome unknown. Draft retained; Refresh to reconcile before another Save."
            }
            Notice::Unavailable => {
                "Runtime unavailable. Draft retained; Refresh to reconcile. No cached fallback."
            }
        }
    }
    pub(super) fn edit(&mut self, action: Action) {
        if self.saving() {
            return;
        }
        match action {
            Action::Transport(value) => self.transport = value,
            Action::Cache(value) => self.cache = value,
            Action::Number { field, value } => {
                // Invalid oversized edits must never retain/save the previous valid value.
                self.numbers[field.index()] = if value.len() > 64 {
                    "Invalid: input too long".into()
                } else {
                    value
                };
            }
            Action::Reset => {
                self.transport = None;
                self.cache = None;
                self.numbers = std::array::from_fn(|_| String::new());
                if !matches!(
                    self.notice,
                    Notice::Reading
                        | Notice::Failed
                        | Notice::Conflict
                        | Notice::Unknown
                        | Notice::Unavailable
                ) {
                    self.notice = Notice::Reset;
                }
            }
            Action::Refresh | Action::Save => return,
        }
        self.dirty = true;
        if matches!(self.notice, Notice::Saved) {
            self.notice = Notice::Loaded;
        }
    }
    pub(super) fn read(&mut self, ticket: Ticket) {
        self.pending = Some(Pending {
            ticket,
            request: Request::Read,
        });
        self.notice = Notice::Reading;
    }
    pub(super) fn save(
        &mut self,
        ticket: Ticket,
    ) -> Option<(u64, CodexRuntimeValues, CodexRuntimeValues)> {
        if !self.can_save() {
            return None;
        }
        let snapshot = self.snapshot?;
        let values = self.values().ok()?;
        self.pending = Some(Pending {
            ticket,
            request: Request::Save {
                revision: snapshot.revision,
                values: Box::new(values),
                base: Box::new(snapshot.base_values),
            },
        });
        // An admitted full draft is intent, even a no-op: unknown/conflict reads retain it.
        self.dirty = true;
        self.notice = Notice::Saving;
        Some((snapshot.revision, values, snapshot.base_values))
    }
    pub(super) fn disconnect(&mut self) {
        if self.pending.take().is_some() {
            self.notice = Notice::Unknown;
        }
        self.fresh = false;
    }
    fn load_draft(&mut self, snapshot: CodexRuntimeSnapshot) {
        // Effective equality cannot distinguish inheritance from an explicit pin.
        let v = snapshot.overrides;
        self.transport = v.transport;
        self.cache = v.cache_retention;
        self.numbers = [
            v.timeout_ms.map(|n| n.get().to_string()),
            v.websocket_connect_timeout_ms.map(|n| n.get().to_string()),
            v.stream_idle_timeout_ms.map(|n| n.as_f64().to_string()),
            v.request_image_pixel_budget.map(|n| n.get().to_string()),
            v.request_image_max_bytes.map(|n| n.get().to_string()),
            v.max_request_image_bytes.map(|n| n.get().to_string()),
        ]
        .map(|value| value.unwrap_or_default());
    }
    pub(super) fn complete(&mut self, ticket: Ticket, outcome: Outcome, owned: bool) {
        if self.ticket() != Some(ticket) {
            return;
        }
        let request = self
            .pending
            .take()
            .expect("matched runtime operation")
            .request;
        if !owned {
            self.fresh = false;
            self.notice = if matches!(request, Request::Read) {
                Notice::Unavailable
            } else {
                Notice::Unknown
            };
            return;
        }
        match (request, outcome) {
            (Request::Read, Outcome::Runtime(snapshot)) if snapshot.version == 1 => {
                if !self.dirty {
                    self.load_draft(snapshot);
                }
                self.snapshot = Some(snapshot);
                self.fresh = true;
                self.notice = Notice::Loaded;
            }
            (
                Request::Save {
                    revision,
                    values,
                    base,
                },
                Outcome::Runtime(snapshot),
            ) if ack_matches(snapshot, revision, *values.clone(), *base.clone()) => {
                self.snapshot = Some(snapshot);
                self.load_draft(snapshot);
                self.fresh = true;
                self.dirty = false;
                self.notice = Notice::Saved;
            }
            (Request::Save { .. }, Outcome::NotSent) => self.notice = Notice::Failed,
            (_, Outcome::RuntimeConflict) => {
                self.fresh = false;
                self.notice = Notice::Conflict;
            }
            (_, Outcome::RuntimeUnavailable) => {
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
