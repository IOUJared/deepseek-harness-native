//! Explicit ChatGPT authorization controls. URLs, credentials and provider diagnostics stay private.
use crate::design;
use crate::ui::{ACCENT, DANGER, MUTED, TEXT, label};
use dsh_native_core::{
    CodexAccountId, CodexAccountLabel, CodexAccountsMetadata, CodexMetadata, CodexPhase,
    SecretCodexCallback,
};
use iced::widget::{button, column, container, row, text_input};
use iced::{Element, Length};
use std::sync::{
    Arc, Mutex,
    atomic::{Ordering, compiler_fence},
};

#[cfg(test)]
#[path = "codex_connect_tests.rs"]
mod connect_tests;
#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
const MAX_INPUT: usize = 16_384;
#[path = "codex_managed.rs"]
mod managed;
#[path = "codex_settings_view.rs"]
mod settings_view;
#[path = "codex_usage.rs"]
mod usage;
pub use usage::Action as UsageAction;
#[path = "codex_runtime.rs"]
mod runtime;
pub(crate) use runtime::ack_matches as runtime_ack_matches;
pub use runtime::{Action as RuntimeAction, Field as RuntimeField};
#[path = "codex_runtime_view.rs"]
mod runtime_view;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub epoch: u64,
    pub panel: u64,
    pub serial: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DraftTicket {
    pub ticket: Ticket,
    pub editor: u64,
    pub attempt: u64,
    pub prompt: u64,
}
#[derive(Clone, Copy)]
pub struct Context {
    pub epoch: u64,
    pub visible: bool,
    pub enabled: bool,
}
#[derive(Default)]
struct Editor(String);
impl Drop for Editor {
    fn drop(&mut self) {
        // SAFETY: overwrite the live owned allocation, including spare capacity; zeros preserve UTF-8.
        unsafe {
            let bytes = self.0.as_mut_vec();
            for offset in 0..bytes.capacity() {
                bytes.as_mut_ptr().add(offset).write_volatile(0);
            }
        }
        compiler_fence(Ordering::SeqCst);
    }
}
enum InputValue {
    Valid(Editor),
    Rejected,
}
/// Clones share a single consuming owner, never a plaintext copy or replayable secret.
#[derive(Clone)]
pub struct Input(Arc<Mutex<Option<InputValue>>>);
impl Input {
    pub fn new(raw: String) -> Self {
        let value = if raw.len() > MAX_INPUT || !raw.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
            drop(Editor(raw));
            InputValue::Rejected
        } else {
            InputValue::Valid(Editor(raw))
        };
        Self(Arc::new(Mutex::new(Some(value))))
    }
    fn take(&self) -> Option<InputValue> {
        self.0.lock().ok()?.take()
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Account,
    Models,
    Images,
    Maintenance,
}
impl Tab {
    pub const ALL: [Self; 4] = [Self::Account, Self::Models, Self::Images, Self::Maintenance];
    fn title(self) -> &'static str {
        match self {
            Self::Account => "Account",
            Self::Models => "Models & runtime",
            Self::Images => "Images",
            Self::Maintenance => "Maintenance",
        }
    }
}
#[derive(Clone)]
pub enum Action {
    Runtime {
        ticket: Ticket,
        action: RuntimeAction,
    },
    Usage {
        ticket: Ticket,
        action: UsageAction,
    },
    SelectTab {
        ticket: Ticket,
        tab: Tab,
    },
    Connect(Ticket),
    RefreshAccounts(Ticket),
    EditLabel {
        ticket: Ticket,
        label: String,
    },
    AddAccount(Ticket),
    ReviewSelect {
        ticket: Ticket,
        account_id: CodexAccountId,
    },
    ReviewSignOut {
        ticket: Ticket,
        account_id: CodexAccountId,
    },
    ReviewSignOutAll(Ticket),
    ConfirmAccount(Ticket),
    DismissAccount(Ticket),
    ToggleAdvanced(Ticket),
    CheckStatus(Ticket),
    Start(Ticket),
    OpenBrowser {
        ticket: Ticket,
        attempt: u64,
    },
    Edit {
        ticket: DraftTicket,
        input: Input,
    },
    ReviewCallback(DraftTicket),
    ConfirmCallback(DraftTicket),
    DiscardCallback(DraftTicket),
    Cancel {
        ticket: Ticket,
        attempt: u64,
    },
    ReviewEnable(Ticket),
    ConfirmEnable(Ticket),
    DismissReview(Ticket),
}
pub enum Effect {
    RuntimeRead {
        ticket: Ticket,
    },
    RuntimeSave {
        ticket: Ticket,
        expected_revision: u64,
        values: dsh_native_core::CodexRuntimeValues,
        base_values: dsh_native_core::CodexRuntimeValues,
    },
    UsageRead {
        ticket: Ticket,
    },
    UsageSave {
        ticket: Ticket,
        expected_revision: u64,
        preferences: dsh_native_core::CodexUsagePreferences,
    },
    AccountsRead {
        ticket: Ticket,
    },
    AddAccount {
        ticket: Ticket,
        label: CodexAccountLabel,
        expected_epoch: u64,
    },
    SelectAccount {
        ticket: Ticket,
        account_id: CodexAccountId,
        expected_epoch: u64,
    },
    SignOutAccount {
        ticket: Ticket,
        account_id: CodexAccountId,
        expected_epoch: u64,
    },
    SignOutAll {
        ticket: Ticket,
        expected_epoch: u64,
    },
    StatusRead {
        ticket: Ticket,
    },
    StartLogin {
        ticket: Ticket,
    },
    OpenBrowser {
        ticket: Ticket,
        attempt: u64,
    },
    SubmitCallback {
        ticket: Ticket,
        attempt: u64,
        prompt: u64,
        secret: SecretCodexCallback,
    },
    CancelLogin {
        ticket: Ticket,
        attempt: u64,
    },
    EnableModels {
        ticket: Ticket,
        expected_revision: u64,
    },
}
/// Metadata is not evidence that a particular login or model-registration operation succeeded.
#[derive(Clone)]
pub enum Outcome {
    Runtime(dsh_native_core::CodexRuntimeSnapshot),
    RuntimeConflict,
    RuntimeIndeterminate,
    RuntimeUnavailable,
    UsageSnapshot(dsh_native_core::CodexUsageSnapshot),
    UsageConflict,
    UsageUnavailable,
    UsageIndeterminate,
    Accounts(CodexAccountsMetadata),
    StatusSnapshot {
        metadata: CodexMetadata,
        accounts: CodexAccountsMetadata,
    },
    Metadata(CodexMetadata),
    Confirmed,
    Refused,
    // Reserved for a future exact Core conflict classification; Core currently reports uncertainty.
    #[allow(dead_code)]
    Conflict,
    Indeterminate,
    NotSent,
}
macro_rules! redacted_debug {
    ($($ty:ty),*) => { $(impl std::fmt::Debug for $ty {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("NativeCodex([REDACTED])") }
    })* };
}
redacted_debug!(Editor, Input, Action, Effect, Outcome, Controller);
#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Accounts,
    Add,
    Select,
    SignOut,
    SignOutAll,
    Status,
    Start,
    Browser,
    Callback,
    Cancel,
    Enable,
}
struct Flight {
    ticket: Ticket,
    operation: Operation,
    revision: Option<u64>,
    // Only a standalone status action may schedule a preference read; OAuth receipts cannot.
    follow_usage: bool,
}
/// An explicit click authorizes this finite chain, not future panels or retries.
#[derive(Clone, Copy)]
struct Connection {
    epoch: u64,
    panel: u64,
    step: ConnectStep,
    auto_select: bool,
}
#[derive(Clone, Copy)]
enum ConnectStep {
    Checking(Ticket),
    Checked(CodexMetadata),
    Starting(Ticket),
    Adding(Ticket),
    Login(LoginState),
    Opening { ticket: Ticket, login: LoginState },
    Enabling(Ticket),
    ReadingAdded { ticket: Ticket, attempt: u64 },
    Selecting(Ticket),
    Selected,
    CheckingSelected(Ticket),
    CheckedSelected,
}
#[derive(Clone, Copy)]
struct LoginState {
    attempt: u64,
    browser_issued: bool,
    browser_confirmed: bool,
}
impl Connection {
    fn progress(self) -> &'static str {
        match self.step {
            ConnectStep::Checking(_) | ConnectStep::Checked(_) => "Checking local configuration…",
            ConnectStep::Starting(_) | ConnectStep::Adding(_) => "Preparing browser sign-in…",
            ConnectStep::Login(_) => {
                "Complete sign-in/approval in your browser. Waiting for confirmation…"
            }
            ConnectStep::Opening { .. } => "Opening your browser…",
            ConnectStep::ReadingAdded { .. }
            | ConnectStep::Selecting(_)
            | ConnectStep::Selected
            | ConnectStep::CheckingSelected(_)
            | ConnectStep::CheckedSelected => "Selecting the just-added local account…",
            ConnectStep::Enabling(_) => "Enabling ChatGPT models…",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Review {
    Callback(DraftTicket),
    Enable { ticket: Ticket, revision: u64 },
}
#[derive(Clone, Copy)]
enum Notice {
    Idle,
    Loaded,
    Invalid,
    Submitted,
    BrowserOpened,
    Cancelled,
    Enabled,
    Connected,
    Configured,
    Refused,
    Conflict,
    Indeterminate,
    NotSent,
    Earlier,
}
impl Notice {
    fn text(self) -> &'static str {
        match self {
            Self::Idle => {
                "Opening this section performs no request. Choose Connect ChatGPT to begin."
            }
            Self::Loaded => {
                "Local status checked; account identity and this login attempt are not verified."
            }
            Self::Invalid => {
                "Invalid callback. Use 1–16384 printable nonwhitespace ASCII bytes; input is never trimmed. Draft cleared; nothing sent."
            }
            Self::Submitted => {
                "Callback accepted for processing. This is not confirmation of sign-in or credential persistence."
            }
            Self::BrowserOpened => {
                "Browser opener accepted the request. Complete sign-in in your browser; no account success is implied."
            }
            Self::Cancelled => {
                "Cancellation requested. Credentials may still be persisted by an already-admitted operation. No automatic retry."
            }
            Self::Enabled => {
                "Model-route operation acknowledged. Load models explicitly, then select a specific model for your session. No model request was sent."
            }
            Self::Connected => {
                "Connected: ChatGPT models are enabled locally. Your existing session model is unchanged; no model request was sent."
            }
            Self::Configured => {
                "ChatGPT is already configured locally (stored credential and model route). This is not account validation; your existing session model is unchanged."
            }
            Self::Refused => "Operation refused. No confirmed success; no automatic retry.",
            Self::Conflict => {
                "Settings revision changed. Check status and review again; no retry was sent and existing routes are preserved."
            }
            Self::Indeterminate => {
                "Outcome uncertain: an admitted operation may still finish or persist credentials. Check status explicitly; no automatic retry."
            }
            Self::NotSent => {
                "Operation was not sent. Draft cleared; retry only through an explicit user action."
            }
            Self::Earlier => {
                "An earlier panel's operation returned. Its result was not applied to this panel. Check status explicitly."
            }
        }
    }
}
pub struct Controller {
    runtime: runtime::Runtime,
    usage: usage::Usage,
    managed: managed::Managed,
    open: bool,
    epoch: u64,
    panel: u64,
    serial: u64,
    editor: u64,
    metadata: Option<CodexMetadata>,
    deferred: Option<CodexMetadata>,
    fresh: bool,
    detached_login: bool,
    owner_uncertain: bool,
    submitted_prompt: Option<(u64, u64)>,
    draft: Editor,
    review: Option<Review>,
    flight: Option<Flight>,
    connection: Option<Connection>,
    advanced: bool,
    tab: Tab,
    notice: Notice,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            runtime: runtime::Runtime::default(),
            usage: usage::Usage::default(),
            managed: managed::Managed::default(),
            open: false,
            epoch: 0,
            panel: 0,
            serial: 0,
            editor: 0,
            metadata: None,
            deferred: None,
            fresh: false,
            detached_login: false,
            owner_uncertain: false,
            submitted_prompt: None,
            draft: Editor::default(),
            review: None,
            flight: None,
            connection: None,
            advanced: false,
            tab: Tab::default(),
            notice: Notice::Idle,
        }
    }
}
impl Controller {
    pub fn ticket(&self) -> Ticket {
        Ticket {
            epoch: self.epoch,
            panel: self.panel,
            serial: self.serial,
        }
    }
    pub fn operation_ticket(&self) -> Option<Ticket> {
        self.flight
            .as_ref()
            .map(|f| f.ticket)
            .or_else(|| self.usage.ticket())
            .or_else(|| self.runtime.ticket())
    }
    /// Permission gate for future usage consumers; no usage execution is connected.
    #[allow(dead_code)]
    pub fn effective_consent(&self) -> bool {
        self.usage.effective_consent()
    }
    pub fn draft_ticket(&self) -> Option<DraftTicket> {
        let m = self.metadata?;
        if !self.open
            || !self.fresh
            || !active(m.phase)
            || m.retry_blocked
            || self.owner_uncertain
            || self.managed.uncertain
        {
            return None;
        }
        if self.submitted_prompt == Some((m.attempt?, m.prompt?)) {
            return None;
        }
        Some(DraftTicket {
            ticket: self.ticket(),
            editor: self.editor,
            attempt: m.attempt?,
            prompt: m.prompt?,
        })
    }
    /// Short operations and an active/uncertain login both block unrelated credential edits.
    pub fn pending(&self) -> bool {
        self.runtime.pending() || self.pending_without_runtime()
    }
    /// Closed runtime warning/status for the fixed modal footer, outside scrolling content.
    pub(crate) fn runtime_footer(&self) -> Option<(&'static str, bool)> {
        (self.runtime.pending() || self.runtime.attention())
            .then(|| (self.runtime.status(), self.runtime.attention()))
    }
    /// Preserve the generic preference reducer's editable draft contract, but rebase the
    /// native read-only scalar only after an exact pending read settles owned and fresh.
    pub(crate) fn completed_native(&mut self, ticket: Ticket, outcome: Outcome) {
        let native_read = self.usage.read_pending_for(ticket);
        self.completed(ticket, outcome);
        if native_read {
            self.usage.rebase_native_read_only_threshold();
        }
    }
    /// Exact-alpha native surface has no observer/alert/retry executor. Allow readback
    /// and explicit reductions only; durable preference foundations are not active features.
    pub(crate) fn usage_surface_action_allowed(&self, action: &UsageAction) -> bool {
        use dsh_native_core::{CodexComposerDisplay, CodexUsageAlert};
        match action {
            UsageAction::Refresh
            | UsageAction::Composer(CodexComposerDisplay::Off)
            | UsageAction::Alert(CodexUsageAlert::Off)
            | UsageAction::Retry(false)
            | UsageAction::Consent(false) => true,
            UsageAction::Save => self.usage.snapshot.as_ref().is_some_and(|snapshot| {
                let before = &snapshot.preferences;
                let after = &self.usage.draft;
                after != before
                    && (after.composer_display == before.composer_display
                        || after.composer_display == CodexComposerDisplay::Off)
                    && (after.usage_alert == before.usage_alert
                        || after.usage_alert == CodexUsageAlert::Off)
                    && (!after.retry_after_reset || before.retry_after_reset)
                    && (!after.experimental_quota || before.experimental_quota)
                    && after.custom_remaining_percent == before.custom_remaining_percent
            }),
            _ => false,
        }
    }
    /// Closed local-preferences status for the fixed footer on every Settings category.
    pub(crate) fn usage_footer(&self) -> Option<(&'static str, bool)> {
        (self.usage.pending() || self.usage.attention())
            .then(|| (self.usage.status(), self.usage.attention()))
    }
    fn pending_without_runtime(&self) -> bool {
        self.usage.pending() || self.pending_account_operation()
    }
    fn pending_account_operation(&self) -> bool {
        self.flight.is_some()
            || self.connection.is_some()
            || self.detached_login
            || self.owner_uncertain
            || self.managed.uncertain
            || self
                .metadata
                .is_some_and(|m| active(m.phase) || m.retry_blocked)
    }
    pub fn enter(&mut self, epoch: u64) {
        self.leave();
        self.open = true;
        self.adopt_epoch(epoch);
    }
    pub fn leave(&mut self) {
        self.usage.initial_queued = false;
        self.managed_reset_panel();
        self.connection = None;
        self.advanced = false;
        self.tab = Tab::default();
        self.clear_draft();
        self.open = false;
        self.fresh = false;
        self.panel += 1;
        self.serial += 1;
        self.notice = Notice::Idle;
        // Closing the panel is not cancellation of an admitted operation or login.
    }
    pub fn adopt_epoch(&mut self, epoch: u64) {
        if epoch == self.epoch {
            return;
        }
        let uncertain = self.pending_without_runtime();
        self.runtime.disconnect();
        self.usage.disconnect();
        self.connection = None;
        self.advanced = false;
        self.clear_draft();
        self.panel += 1;
        self.serial += 1;
        self.epoch = epoch;
        self.managed = managed::Managed::default();
        self.metadata = None;
        self.deferred = None;
        self.fresh = false;
        self.detached_login = false;
        self.owner_uncertain = false;
        self.submitted_prompt = None;
        self.flight = None;
        self.notice = if uncertain {
            Notice::Indeterminate
        } else {
            Notice::Idle
        };
    }
    pub fn disconnect(&mut self) {
        self.adopt_epoch(0);
    }
    fn clear_draft(&mut self) {
        self.draft = Editor::default();
        self.review = None;
        self.editor += 1;
    }
    fn allowed(&self, c: Context) -> bool {
        self.open && c.visible && c.enabled && self.epoch != 0 && c.epoch == self.epoch
    }
    fn ready(&self) -> bool {
        self.flight.is_none() && !self.usage.pending() && !self.runtime.pending()
    }
    fn handle_runtime(
        &mut self,
        ticket: Ticket,
        action: RuntimeAction,
        context: Context,
    ) -> Option<Effect> {
        if !self.allowed(context)
            || ticket != self.ticket()
            || !matches!(self.tab, Tab::Models | Tab::Images)
        {
            return None;
        }
        // Runtime intent cannot resume an OAuth chain or queue a preference followup.
        self.connection = None;
        self.usage.initial_queued = false;
        match action {
            RuntimeAction::Refresh if self.ready() => {
                self.serial += 1;
                let ticket = self.ticket();
                self.runtime.read(ticket);
                Some(Effect::RuntimeRead { ticket })
            }
            RuntimeAction::Save if self.ready() && self.runtime.can_save() => {
                self.serial += 1;
                let ticket = self.ticket();
                let (expected_revision, values, base_values) = self.runtime.save(ticket)?;
                Some(Effect::RuntimeSave {
                    ticket,
                    expected_revision,
                    values,
                    base_values,
                })
            }
            RuntimeAction::Refresh | RuntimeAction::Save => None,
            other if self.flight.is_none() && !self.usage.pending() && !self.runtime.saving() => {
                self.runtime.edit(other);
                None
            }
            _ => None,
        }
    }
    fn usage_read(&mut self) -> Effect {
        self.serial += 1;
        let ticket = self.ticket();
        self.usage.read(ticket);
        Effect::UsageRead { ticket }
    }
    fn handle_usage(
        &mut self,
        ticket: Ticket,
        action: UsageAction,
        context: Context,
    ) -> Option<Effect> {
        // Local OFF is a permission fence even when other settings/backend work disables writes.
        let local = self.open
            && context.visible
            && context.epoch == self.epoch
            && ticket == self.ticket()
            && self.tab == Tab::Account;
        if !local {
            return None;
        }
        // Preference intent never authorizes or resumes an OAuth/model-route chain.
        self.connection = None;
        self.usage.initial_queued = false;
        if matches!(action, UsageAction::Consent(false)) {
            self.usage.edit(action);
            return None;
        }
        if !self.allowed(context) || !self.ready() {
            return None;
        }
        match action {
            UsageAction::Refresh => Some(self.usage_read()),
            UsageAction::Save if self.usage.can_save() => {
                self.serial += 1;
                let ticket = self.ticket();
                let (expected_revision, preferences) = self.usage.save(ticket)?;
                Some(Effect::UsageSave {
                    ticket,
                    expected_revision,
                    preferences,
                })
            }
            other => {
                self.usage.edit(other);
                None
            }
        }
    }
    fn may_start(&self) -> bool {
        self.fresh
            && !self.owner_uncertain
            && !self.managed.uncertain
            && !self.detached_login
            && self.metadata.is_some_and(|m| {
                m.available && !m.credential_stored && !m.retry_blocked && !active(m.phase)
            })
    }
    fn may_enable(&self) -> bool {
        !self.owner_uncertain
            && !self.managed.uncertain
            && self.fresh
            && self.metadata.is_some_and(|m| {
                m.available
                    && m.credential_stored
                    && !m.route_configured
                    && !m.retry_blocked
                    && !active(m.phase)
                    && m.settings_revision.is_some()
            })
    }
    fn begin(&mut self, operation: Operation) -> Ticket {
        self.serial += 1;
        let ticket = self.ticket();
        self.flight = Some(Flight {
            ticket,
            operation,
            revision: self.metadata.and_then(|m| m.settings_revision),
            follow_usage: false,
        });
        if matches!(operation, Operation::Start | Operation::Add) {
            // Login may add an unselected account without changing the selection epoch.
            // Losing the automatic chain must not make the pre-login roster writable.
            self.managed.fresh = false;
            self.deferred = None;
            self.detached_login = true;
        }
        ticket
    }
    /// Called only after a completion/observation. It never starts an intent by itself.
    pub fn continue_connection(&mut self, context: Context) -> Option<Effect> {
        let effect = self.continue_connection_step(context);
        if effect.is_some() {
            return effect;
        }
        // A preference receipt cannot advance login/models: auto-read only after the explicit chain ends.
        if self.connection.is_none()
            && self.usage.initial_queued
            && self.allowed(context)
            && self.ready()
            && self.tab == Tab::Account
        {
            return Some(self.usage_read());
        }
        None
    }
    fn continue_connection_step(&mut self, context: Context) -> Option<Effect> {
        let connection = self.connection?;
        if !self.allowed(context)
            || connection.epoch != self.epoch
            || connection.panel != self.panel
        {
            self.connection = None;
            return None;
        }
        if !self.ready() {
            return None;
        }
        let mut next = connection;
        if let ConnectStep::ReadingAdded { attempt, .. } = connection.step
            && let Some(added) = self.managed.added.clone().filter(|a| a.attempt == attempt)
        {
            let ticket = self.begin(Operation::Select);
            next.step = ConnectStep::Selecting(ticket);
            self.connection = Some(next);
            return Some(Effect::SelectAccount {
                ticket,
                account_id: added.account_id,
                expected_epoch: added.epoch,
            });
        }
        match connection.step {
            ConnectStep::Checked(receipt) => {
                // Only this explicit Status receipt authorizes the credential/no-credential branch.
                if !receipt.available
                    || receipt.retry_blocked
                    || active(receipt.phase)
                    || self.owner_uncertain
                    || self.detached_login
                {
                    self.stop_connection(Notice::Refused);
                    return None;
                }
                if self.managed_mode() && (!self.managed.fresh || self.managed.uncertain) {
                    self.stop_connection(Notice::Indeterminate);
                    return None;
                }
                if receipt.credential_stored {
                    let Some(current) = self.metadata.filter(|m| {
                        self.fresh
                            && m.attempt == receipt.attempt
                            && m.available
                            && m.credential_stored
                            && !m.retry_blocked
                            && !active(m.phase)
                    }) else {
                        self.stop_connection(Notice::Indeterminate);
                        return None;
                    };
                    if current.route_configured {
                        self.stop_connection(Notice::Configured);
                        return None;
                    }
                    return self.connect_enable(current);
                }
                if !self.may_start() {
                    self.stop_connection(Notice::Indeterminate);
                    return None;
                }
                let ticket = self.begin(Operation::Start);
                next.step = ConnectStep::Starting(ticket);
                self.connection = Some(next);
                return Some(Effect::StartLogin { ticket });
            }
            ConnectStep::Login(mut login) => {
                let Some(m) = self
                    .metadata
                    .filter(|m| self.fresh && m.attempt == Some(login.attempt))
                else {
                    self.stop_connection(Notice::Indeterminate);
                    return None;
                };
                if !m.available || m.retry_blocked || self.owner_uncertain {
                    self.stop_connection(Notice::Indeterminate);
                    return None;
                }
                match m.phase {
                    CodexPhase::Authorized if m.credential_stored || self.managed_mode() => {
                        // A terminal can precede Start ACK or Browser ACK. Never bypass an issued opener's receipt.
                        if login.browser_issued && !login.browser_confirmed {
                            return None;
                        }
                        if self.managed_mode() && !connection.auto_select {
                            self.managed.fresh = false;
                            self.stop_connection(Notice::Loaded);
                            return None;
                        }
                        if self.managed_mode() {
                            let ticket = self.begin(Operation::Accounts);
                            next.step = ConnectStep::ReadingAdded {
                                ticket,
                                attempt: login.attempt,
                            };
                            self.connection = Some(next);
                            return Some(Effect::AccountsRead { ticket });
                        }
                        if m.route_configured {
                            self.stop_connection(Notice::Connected);
                            return None;
                        }
                        return self.connect_enable(m);
                    }
                    CodexPhase::WaitingBrowser => {
                        if !login.browser_issued && m.browser_available {
                            // Record issuance before returning the effect: repeated snapshots cannot reopen it.
                            login.browser_issued = true;
                            let ticket = self.begin(Operation::Browser);
                            next.step = ConnectStep::Opening { ticket, login };
                            self.connection = Some(next);
                            return Some(Effect::OpenBrowser {
                                ticket,
                                attempt: login.attempt,
                            });
                        }
                    }
                    CodexPhase::Cancelled => self.stop_connection(Notice::Cancelled),
                    CodexPhase::Failed => self.stop_connection(Notice::Refused),
                    _ => self.stop_connection(Notice::Indeterminate),
                }
            }
            ConnectStep::Selected => {
                let ticket = self.begin(Operation::Status);
                next.step = ConnectStep::CheckingSelected(ticket);
                self.connection = Some(next);
                return Some(Effect::StatusRead { ticket });
            }
            ConnectStep::CheckingSelected(_) => {}
            ConnectStep::CheckedSelected => {
                let m = self.metadata?;
                if !m.credential_stored
                    || m.retry_blocked
                    || !self.fresh
                    || !self.managed.fresh
                    || !self.managed.added.as_ref().is_some_and(|a| {
                        self.managed.metadata.as_ref().is_some_and(|accounts| {
                            accounts.selected.as_ref() == Some(&a.account_id)
                                && accounts.epoch == a.epoch.checked_add(1)
                        })
                    })
                {
                    self.stop_connection(Notice::Indeterminate);
                    return None;
                }
                if m.route_configured {
                    self.stop_connection(Notice::Connected);
                    return None;
                }
                return self.connect_enable(m);
            }
            // Only completed() may advance these receipt gates, even if metadata is already fresh.
            ConnectStep::Checking(_)
            | ConnectStep::Starting(_)
            | ConnectStep::Adding(_)
            | ConnectStep::Opening { .. }
            | ConnectStep::Enabling(_)
            | ConnectStep::ReadingAdded { .. }
            | ConnectStep::Selecting(_) => {}
        }
        None
    }
    fn stop_connection(&mut self, notice: Notice) {
        self.connection = None;
        self.notice = notice;
    }
    fn connect_enable(&mut self, m: CodexMetadata) -> Option<Effect> {
        if !self.may_enable() {
            self.stop_connection(Notice::Indeterminate);
            return None;
        }
        let Some(expected_revision) = m.settings_revision else {
            self.stop_connection(Notice::Indeterminate);
            return None;
        };
        let ticket = self.begin(Operation::Enable);
        if let Some(connection) = &mut self.connection {
            connection.step = ConnectStep::Enabling(ticket);
        }
        Some(Effect::EnableModels {
            ticket,
            expected_revision,
        })
    }
    fn connection_receipt(
        &mut self,
        ticket: Ticket,
        operation: Operation,
        outcome: Outcome,
        accepted: bool,
    ) {
        let Some(mut connection) = self.connection else {
            return;
        };
        if connection.epoch != ticket.epoch || connection.panel != ticket.panel {
            self.connection = None;
            return;
        }
        let step = match (connection.step, operation, outcome.clone()) {
            (ConnectStep::Checking(expected), Operation::Status, Outcome::Metadata(m))
                if expected == ticket && accepted =>
            {
                Some(ConnectStep::Checked(m))
            }
            (ConnectStep::Starting(expected), Operation::Start, Outcome::Metadata(m))
            | (ConnectStep::Adding(expected), Operation::Add, Outcome::Metadata(m))
                if expected == ticket && accepted =>
            {
                m.attempt.map(|attempt| {
                    // The attempt comes from the matched Start receipt, never an unsolicited event.
                    ConnectStep::Login(LoginState {
                        attempt,
                        browser_issued: false,
                        browser_confirmed: false,
                    })
                })
            }
            (
                ConnectStep::Opening {
                    ticket: expected,
                    mut login,
                },
                Operation::Browser,
                Outcome::Confirmed,
            ) if expected == ticket => {
                login.browser_confirmed = true;
                Some(ConnectStep::Login(login))
            }
            (
                ConnectStep::ReadingAdded {
                    ticket: expected,
                    attempt,
                },
                Operation::Accounts,
                Outcome::Accounts(m),
            ) if expected == ticket && accepted && self.managed_mode() => {
                if let Some(added) = m.added.filter(|a| {
                    a.attempt == attempt
                        && m.epoch == Some(a.epoch)
                        && m.accounts
                            .iter()
                            .any(|account| account.opaque_id == a.account_id)
                }) {
                    self.managed.added = Some(added);
                    // The next effect is emitted only by continue_connection under the original panel receipt.
                    Some(ConnectStep::ReadingAdded { ticket, attempt })
                } else {
                    None
                }
            }
            (ConnectStep::Selecting(expected), Operation::Select, Outcome::Accounts(m))
                if expected == ticket
                    && accepted
                    && self.managed.added.as_ref().is_some_and(|a| {
                        m.selected.as_ref() == Some(&a.account_id)
                            && m.epoch == a.epoch.checked_add(1)
                    }) =>
            {
                Some(ConnectStep::Selected)
            }
            (ConnectStep::CheckingSelected(expected), Operation::Status, Outcome::Metadata(_))
                if expected == ticket && accepted =>
            {
                Some(ConnectStep::CheckedSelected)
            }
            (ConnectStep::Enabling(expected), Operation::Enable, Outcome::Metadata(_))
                if expected == ticket
                    && accepted
                    && matches!(self.notice, Notice::Enabled)
                    && self.metadata.is_some_and(|m| {
                        m.available && m.credential_stored && m.route_configured && !m.retry_blocked
                    }) =>
            {
                self.stop_connection(Notice::Connected);
                return;
            }
            _ => None,
        };
        if let Some(step) = step {
            connection.step = step;
            self.connection = Some(connection);
        } else {
            // Refusal, wrong receipt kind, queue rejection, conflict and uncertainty end the chain.
            self.connection = None;
            if matches!(outcome, Outcome::Metadata(_) | Outcome::Confirmed) {
                self.notice = Notice::Indeterminate;
            }
        }
    }
    pub fn handle(&mut self, action: Action, context: Context) -> Option<Effect> {
        if let Action::Runtime { ticket, action } = action {
            return self.handle_runtime(ticket, action, context);
        }
        if let Action::Usage { ticket, action } = action {
            return self.handle_usage(ticket, action, context);
        }
        if let Action::SelectTab { ticket, tab } = action {
            if self.open
                && context.visible
                && context.epoch == self.epoch
                && ticket == self.ticket()
            {
                self.connection = None;
                self.managed_reset_panel();
                if self.tab != tab {
                    self.usage.initial_queued = false;
                    self.panel += 1;
                    self.serial += 1;
                }
                self.tab = tab;
                self.clear_draft();
                self.advanced = false;
            }
            return None;
        }
        if !self.allowed(context) {
            self.connection = None;
        }
        if let Action::ToggleAdvanced(ticket) = action {
            if self.allowed(context) && ticket == self.ticket() {
                self.advanced = !self.advanced;
                if !self.advanced {
                    self.clear_draft();
                }
            }
            return None;
        }
        if matches!(
            &action,
            Action::RefreshAccounts(_)
                | Action::EditLabel { .. }
                | Action::AddAccount(_)
                | Action::ReviewSelect { .. }
                | Action::ReviewSignOut { .. }
                | Action::ReviewSignOutAll(_)
                | Action::ConfirmAccount(_)
                | Action::DismissAccount(_)
        ) {
            return self.handle_account(&action, context);
        }
        // Cancel uses a separate reserved worker slot; retire this panel's other ACK, never guess cleanup.
        if let Action::Cancel { ticket, attempt } = &action
            && self.allowed(context)
            && *ticket == self.ticket()
            && self.metadata.is_some_and(|m| {
                (active(m.phase) || self.owner_uncertain) && m.attempt == Some(*attempt)
            })
        {
            self.connection = None;
            self.managed.added = None;
            if self.flight.as_ref().is_some_and(|f| {
                matches!(
                    f.operation,
                    Operation::Start | Operation::Add | Operation::Callback
                )
            }) {
                self.owner_uncertain = true;
                self.managed.uncertain |= self.managed_mode();
            }
            self.clear_draft();
            return Some(Effect::CancelLogin {
                ticket: self.begin(Operation::Cancel),
                attempt: *attempt,
            });
        }
        // Consume even rejected/stale/hidden input holders so a clone cannot replay plaintext.
        if let Action::Edit { ticket, input } = &action {
            let value = input.take();
            if !self.allowed(context)
                || !self.ready()
                || self.review.is_some()
                || self.draft_ticket() != Some(*ticket)
            {
                return None;
            }
            value.as_ref()?;
            self.connection = None;
            match value {
                Some(InputValue::Valid(editor)) => {
                    self.draft = editor;
                    self.editor += 1;
                }
                Some(InputValue::Rejected) => {
                    self.clear_draft();
                    self.notice = Notice::Invalid;
                }
                None => {}
            }
            return None;
        }
        // A current manual action revokes automation before it can admit another effect.
        let manual_current = match &action {
            Action::CheckStatus(t)
            | Action::Start(t)
            | Action::ReviewEnable(t)
            | Action::ConfirmEnable(t)
            | Action::DismissReview(t) => *t == self.ticket(),
            Action::OpenBrowser { ticket, .. } | Action::Cancel { ticket, .. } => {
                *ticket == self.ticket()
            }
            Action::ReviewCallback(t) | Action::ConfirmCallback(t) | Action::DiscardCallback(t) => {
                self.draft_ticket() == Some(*t)
            }
            _ => false,
        };
        if self.allowed(context) && manual_current {
            self.connection = None;
        }
        if !self.allowed(context) || !self.ready() {
            return None;
        }
        match action {
            Action::Connect(t) if t == self.ticket() && self.connection.is_none() => {
                self.clear_draft();
                let ticket = self.begin(Operation::Status);
                self.connection = Some(Connection {
                    epoch: self.epoch,
                    panel: self.panel,
                    step: ConnectStep::Checking(ticket),
                    auto_select: true,
                });
                return Some(Effect::StatusRead { ticket });
            }
            Action::CheckStatus(t) if t == self.ticket() => {
                self.clear_draft();
                let ticket = self.begin(Operation::Status);
                self.flight.as_mut().expect("admitted status").follow_usage = true;
                return Some(Effect::StatusRead { ticket });
            }
            Action::Start(t) if t == self.ticket() && self.may_start() && self.review.is_none() => {
                self.clear_draft();
                return Some(Effect::StartLogin {
                    ticket: self.begin(Operation::Start),
                });
            }
            Action::OpenBrowser { ticket, attempt }
                if ticket == self.ticket()
                    && self.review.is_none()
                    && !self.owner_uncertain
                    && self.fresh
                    && self.metadata.is_some_and(|m| {
                        m.available
                            && active(m.phase)
                            && !m.retry_blocked
                            && m.browser_available
                            && m.attempt == Some(attempt)
                    }) =>
            {
                self.clear_draft();
                return Some(Effect::OpenBrowser {
                    ticket: self.begin(Operation::Browser),
                    attempt,
                });
            }
            Action::ReviewCallback(t)
                if self.draft_ticket() == Some(t) && !self.draft.0.is_empty() =>
            {
                self.review = Some(Review::Callback(t));
            }
            Action::ConfirmCallback(t)
                if self.draft_ticket() == Some(t) && self.review == Some(Review::Callback(t)) =>
            {
                let raw = std::mem::take(&mut self.draft.0);
                self.clear_draft();
                let Ok(secret) = SecretCodexCallback::new(raw) else {
                    self.notice = Notice::Invalid;
                    return None;
                };
                self.submitted_prompt = Some((t.attempt, t.prompt));
                return Some(Effect::SubmitCallback {
                    ticket: self.begin(Operation::Callback),
                    attempt: t.attempt,
                    prompt: t.prompt,
                    secret,
                });
            }
            Action::DiscardCallback(t) if self.draft_ticket() == Some(t) => self.clear_draft(),
            Action::Cancel { ticket, attempt }
                if ticket == self.ticket()
                    && self.metadata.is_some_and(|m| {
                        (active(m.phase) || self.owner_uncertain) && m.attempt == Some(attempt)
                    }) =>
            {
                self.clear_draft();
                return Some(Effect::CancelLogin {
                    ticket: self.begin(Operation::Cancel),
                    attempt,
                });
            }
            Action::ReviewEnable(t) if t == self.ticket() && self.may_enable() => {
                self.clear_draft();
                self.review = Some(Review::Enable {
                    ticket: t,
                    revision: self.metadata?.settings_revision?,
                });
            }
            Action::ConfirmEnable(t) if t == self.ticket() && self.may_enable() => {
                let Some(Review::Enable { ticket, revision }) = self.review else {
                    return None;
                };
                if ticket != t || self.metadata?.settings_revision != Some(revision) {
                    self.clear_draft();
                    return None;
                }
                self.clear_draft();
                return Some(Effect::EnableModels {
                    ticket: self.begin(Operation::Enable),
                    expected_revision: revision,
                });
            }
            Action::DismissReview(t) if t == self.ticket() => self.clear_draft(),
            _ => {}
        }
        None
    }
    pub fn completed(&mut self, ticket: Ticket, outcome: Outcome) {
        if self.runtime.ticket() == Some(ticket) {
            let owned = self.open && ticket.epoch == self.epoch && ticket.panel == self.panel;
            self.runtime.complete(ticket, outcome, owned);
            return;
        }
        if self.usage.ticket() == Some(ticket) {
            let owned = self.open && ticket.epoch == self.epoch && ticket.panel == self.panel;
            self.usage.complete(ticket, outcome, owned);
            return;
        }
        if self.operation_ticket() != Some(ticket) {
            return;
        }
        let flight = self.flight.take().expect("matched operation");
        let operation = flight.operation;
        let outcome = match outcome {
            Outcome::StatusSnapshot { metadata, accounts }
                if matches!(operation, Operation::Status | Operation::Accounts) =>
            {
                if self.open && ticket.epoch == self.epoch && ticket.panel == self.panel {
                    if operation == Operation::Status
                        && flight.follow_usage
                        && !self.usage.initial_attempted
                    {
                        self.usage.initial_queued = true;
                    }
                    // Both nonmutating receipts are required to reconcile managed uncertainty.
                    if accounts.mode == dsh_native_core::CodexAccountMode::Managed
                        && !active(metadata.phase)
                        && !metadata.retry_blocked
                    {
                        self.detached_login = false;
                        self.owner_uncertain = false;
                        self.managed.uncertain = false;
                    }
                    if operation == Operation::Accounts {
                        self.metadata = Some(metadata);
                        self.fresh = true;
                        Outcome::Accounts(accounts)
                    } else {
                        self.account_receipt(Operation::Accounts, accounts);
                        Outcome::Metadata(metadata)
                    }
                } else {
                    Outcome::Metadata(metadata)
                }
            }
            Outcome::StatusSnapshot { .. } => Outcome::Indeterminate,
            other => other,
        };
        let outcome = match outcome {
            Outcome::Metadata(_)
                if matches!(
                    operation,
                    Operation::Accounts
                        | Operation::Select
                        | Operation::SignOut
                        | Operation::SignOutAll
                ) =>
            {
                Outcome::Indeterminate
            }
            Outcome::Accounts(_)
                if !matches!(
                    operation,
                    Operation::Accounts
                        | Operation::Select
                        | Operation::SignOut
                        | Operation::SignOutAll
                ) =>
            {
                Outcome::Indeterminate
            }
            other => other,
        };
        if matches!(outcome, Outcome::Indeterminate) {
            if matches!(
                operation,
                Operation::Start | Operation::Add | Operation::Callback | Operation::Cancel
            ) {
                self.owner_uncertain = true;
            } else if operation == Operation::Enable {
                self.metadata = None;
                self.fresh = false;
            }
        }
        if matches!(outcome, Outcome::Indeterminate)
            && matches!(
                operation,
                Operation::Start
                    | Operation::Add
                    | Operation::Select
                    | Operation::SignOut
                    | Operation::SignOutAll
            )
            && self.managed_mode()
        {
            self.managed.uncertain = true;
            self.managed.fresh = false;
        }
        if !self.open || ticket.epoch != self.epoch || ticket.panel != self.panel {
            self.connection = None;
            // Retire the receipt without hydrating a hidden/new panel. Deferred state remains private.
            if matches!(operation, Operation::Start | Operation::Add) {
                self.deferred = None;
            }
            self.notice = Notice::Earlier;
            return;
        }
        self.clear_draft();
        let mut accepted = false;
        match outcome.clone() {
            Outcome::Accounts(m)
                if matches!(
                    operation,
                    Operation::Accounts
                        | Operation::Select
                        | Operation::SignOut
                        | Operation::SignOutAll
                ) =>
            {
                self.account_receipt(operation, m);
                accepted = true;
                self.notice = Notice::Loaded;
                // Selection/logout invalidates grant metadata until an explicit status receipt.
                if operation != Operation::Accounts {
                    self.fresh = false;
                }
            }
            Outcome::Accounts(_) | Outcome::StatusSnapshot { .. } => {
                self.notice = Notice::Indeterminate
            }
            Outcome::Metadata(mut m) => {
                // Start and status may introduce an attempt. A late Start ACK cannot downgrade a terminal observation.
                if matches!(operation, Operation::Start | Operation::Add)
                    && let Some(observed) = self
                        .deferred
                        .take()
                        .filter(|observed| observed.attempt == m.attempt)
                {
                    m = observed;
                }
                if matches!(
                    operation,
                    Operation::Start | Operation::Add | Operation::Status
                ) || self.same_attempt(m)
                {
                    if operation != Operation::Status
                        && let Some(current) = self.metadata.filter(|current| {
                            current.attempt == m.attempt
                                && !active(current.phase)
                                && active(m.phase)
                        })
                    {
                        m = current;
                    }
                    accepted = true;
                    self.metadata = Some(m);
                    self.fresh = true;
                    self.detached_login = false;
                    self.notice = if operation == Operation::Enable {
                        if m.route_configured
                            && flight
                                .revision
                                .zip(m.settings_revision)
                                .is_some_and(|(before, after)| after > before)
                        {
                            Notice::Enabled
                        } else {
                            Notice::Indeterminate
                        }
                    } else if operation == Operation::Cancel {
                        Notice::Cancelled
                    } else {
                        Notice::Loaded
                    };
                } else {
                    self.notice = Notice::Indeterminate;
                }
            }
            Outcome::Confirmed => {
                self.notice = match operation {
                    Operation::Browser => Notice::BrowserOpened,
                    Operation::Callback => Notice::Submitted,
                    Operation::Cancel => Notice::Cancelled,
                    Operation::Enable => Notice::Indeterminate,
                    _ => Notice::Indeterminate,
                }
            }
            Outcome::Refused => {
                self.notice = Notice::Refused;
                if operation == Operation::Callback {
                    self.submitted_prompt = None;
                }
            }
            Outcome::Conflict => {
                self.notice = Notice::Conflict;
                self.metadata = None;
                self.managed.fresh = false;
            }
            Outcome::Indeterminate
            | Outcome::UsageIndeterminate
            | Outcome::UsageSnapshot(_)
            | Outcome::UsageConflict
            | Outcome::UsageUnavailable
            | Outcome::Runtime(_)
            | Outcome::RuntimeConflict
            | Outcome::RuntimeIndeterminate
            | Outcome::RuntimeUnavailable => self.notice = Notice::Indeterminate,
            Outcome::NotSent => {
                self.notice = Notice::NotSent;
                if operation == Operation::Callback {
                    self.submitted_prompt = None;
                }
                if matches!(operation, Operation::Start | Operation::Add) {
                    self.detached_login = false;
                    self.deferred = None;
                }
            }
        }
        self.connection_receipt(ticket, operation, outcome, accepted);
    }
    fn same_attempt(&self, m: CodexMetadata) -> bool {
        self.metadata
            .is_some_and(|current| current.attempt == m.attempt)
    }
    /// Only the currently tracked attempt may publish unsolicited state. Never opens a panel.
    pub fn observed(&mut self, metadata: CodexMetadata) {
        if self.epoch == 0 {
            return;
        }
        if self
            .flight
            .as_ref()
            .is_some_and(|f| matches!(f.operation, Operation::Start | Operation::Add))
        {
            if let Some(attempt) = metadata.attempt {
                // Core attempt IDs increase monotonically. An old terminal must not pin this slot
                // before the new attempt's ACK; within one ID, a waiting snapshot cannot downgrade
                // a terminal. completed() still applies this only to the exact Start-receipt ID.
                if self.deferred.is_none_or(|m| {
                    m.attempt.is_none_or(|previous| {
                        attempt > previous
                            || attempt == previous && (active(m.phase) || !active(metadata.phase))
                    })
                }) {
                    self.deferred = Some(metadata);
                }
            }
            return;
        }
        if !self.open || !self.fresh || !self.same_attempt(metadata) {
            return;
        }
        if self.connection.is_some()
            && self
                .metadata
                .is_some_and(|m| !active(m.phase) && active(metadata.phase))
        {
            // A transport snapshot cannot downgrade the owned terminal while its opener ACK is pending.
            return;
        }
        if self.metadata.is_some_and(|m| {
            m.prompt != metadata.prompt
                || m.settings_revision != metadata.settings_revision
                || m.retry_blocked != metadata.retry_blocked
                || m.phase != metadata.phase
                || m.credential_stored != metadata.credential_stored
                || m.route_configured != metadata.route_configured
                || m.available != metadata.available
                || m.browser_available != metadata.browser_available
        }) {
            self.clear_draft();
            self.serial += 1;
        }
        self.metadata = Some(metadata);
    }
    fn review_view(&self, context: Context) -> Option<Element<'_, Action>> {
        let allowed = self.allowed(context) && self.ready();
        let t = self.ticket();
        let mut body = column![label("Codex · ChatGPT sign-in", TEXT).size(23)]
            .spacing(14)
            .width(Length::Fill);
        match self.review? {
            Review::Callback(ticket) if self.draft_ticket() == Some(ticket) => {
                body = body
                    .push(label(format!("Review callback · OpenAI attempt {} · prompt {}", ticket.attempt, ticket.prompt), ACCENT))
                    .push(text_input("Authorization callback", &self.draft.0).secure(true)
                        .padding(12).style(design::input_style))
                    .push(label("Submit this masked callback only to this current OpenAI login attempt. Its contents are consumed, never displayed or logged. Acceptance is not sign-in or persistence confirmation.", MUTED))
                    .push(row![
                        button("Submit callback").padding([10, 14]).style(design::primary_button)
                            .on_press_maybe(allowed.then_some(Action::ConfirmCallback(ticket))),
                        button("Cancel review").padding([10, 14]).style(design::button_style)
                            .on_press_maybe(allowed.then_some(Action::DismissReview(t)))
                    ].spacing(10))
                    .push(label("Cancel attempt may still race credential persistence. Closing or switching sections clears the draft, not the login.", DANGER))
                    .push(button("Cancel attempt").padding([8, 12]).style(design::danger)
                        .on_press_maybe(allowed.then_some(Action::Cancel { ticket: t, attempt: ticket.attempt })));
            }
            Review::Enable { ticket, revision } if ticket == t && self.may_enable() => {
                body = body
                    .push(label(format!("Review GPT model route · openai-codex · revision {revision}"), ACCENT))
                    .push(label("Only a missing native-default route is added with revision checking. Existing configuration is preserved; a changed revision refuses this write.", MUTED))
                    .push(label("This does not load models or send a request. After enabling, explicitly Load models and select a specific model for your session. Subscription plan and provider limits apply.", MUTED))
                    .push(row![
                        button("Enable GPT models").padding([10, 14]).style(design::primary_button)
                            .on_press_maybe(allowed.then_some(Action::ConfirmEnable(ticket))),
                        button("Cancel review").padding([10, 14]).style(design::button_style)
                            .on_press_maybe(allowed.then_some(Action::DismissReview(t)))
                    ].spacing(10));
            }
            _ => return None,
        }
        Some(container(body).width(Length::Fill).into())
    }
    fn connection_view(&self, context: Context) -> Element<'_, Action> {
        let allowed = self.allowed(context) && self.ready();
        let t = self.ticket();
        let mut body = column![
            button("Connect ChatGPT").padding([10, 14]).style(design::primary_button)
                .on_press_maybe((allowed && self.connection.is_none()).then_some(Action::Connect(t))),
            label("Opens OpenAI sign-in and connects model access. This chat’s model stays unchanged; credentials may be stored locally.", MUTED)]
            .spacing(14).width(Length::Fill);
        if self.connection.is_some() || !matches!(self.notice, Notice::Idle) {
            body = body.push(label(
                self.connection
                    .map_or(self.notice.text(), Connection::progress),
                MUTED,
            ));
        }
        if self.owner_uncertain {
            body = body.push(label("A sign-in operation has an unknown outcome. Do not retry. Cancel the tracked attempt if needed, then explicitly Refresh accounts to reconcile managed state once it settles. Legacy uncertainty requires restarting the application.", DANGER));
        }
        if let Some(m) = self.metadata.filter(|_| self.fresh) {
            if m.credential_stored && m.route_configured {
                body = body.push(label(
                    "Model entitlement is unverified; plan/provider limits apply.",
                    MUTED,
                ));
            }
            if m.retry_blocked {
                body = body.push(label("An earlier login may still be running. Restart before another sign-in. Cancellation does not guarantee credentials were not stored.", DANGER));
            }
            if (active(m.phase) || self.owner_uncertain)
                && let Some(attempt) = m.attempt
            {
                body = body.push(
                    button("Cancel attempt")
                        .padding([10, 14])
                        .style(design::danger)
                        .on_press_maybe(
                            self.allowed(context)
                                .then_some(Action::Cancel { ticket: t, attempt }),
                        ),
                );
            }
        }
        if self.pending() {
            body = body.push(label("Closing stops automatic next steps. Already-admitted login/model-route writes may still finish; cancellation may race credential persistence. No automatic retry.", MUTED));
        }
        body = body.push(
            button(
                label(
                    if self.advanced {
                        "▾ Advanced"
                    } else {
                        "▸ Advanced"
                    },
                    MUTED,
                )
                .size(12),
            )
            .padding([6, 8])
            .style(design::button_style)
            .on_press_maybe(self.allowed(context).then_some(Action::ToggleAdvanced(t))),
        );
        if !self.advanced {
            if let Some(m) = self.metadata.filter(|_| self.fresh) {
                body = body.push(label(
                    if !m.available {
                        "Codex sign-in is unavailable in this backend profile."
                    } else if m.credential_stored && m.route_configured {
                        "Credential and model route configured locally."
                    } else if m.credential_stored {
                        "A local credential is stored (not account validation)."
                    } else {
                        "No stored local credential is reported."
                    },
                    MUTED,
                ));
            }
            return container(body).width(Length::Fill).into();
        }
        body = body
            .push(label("Manual recovery & help", TEXT).size(16))
            .push(label("Use your ChatGPT account, not an OpenAI API key. OpenAI may require sign-in, MFA or approval; subscription availability, plan limits and provider policies apply.", MUTED))
            .push(label("Opening this section sends no request. Callback input stays masked and is consumed on submission; closing clears the owned draft, not admitted sign-in writes. Masking cannot guarantee clipboard, editor or system copies are wiped.", MUTED))
            .push(
                button("Check sign-in status")
                    .padding([10, 14])
                    .style(design::button_style)
                    .on_press_maybe(allowed.then_some(Action::CheckStatus(t))),
            );
        let Some(m) = self.metadata.filter(|_| self.fresh) else {
            return body.push(label("Sign-in status unavailable. No automatic account or model request is performed.", MUTED)).into();
        };
        if !m.available {
            return body
                .push(label(
                    "Codex sign-in is unavailable in this backend profile.",
                    DANGER,
                ))
                .into();
        }
        if m.retry_blocked {
            body = body.push(label("An earlier login may still be running. Restart the application before trying another sign-in. Cancellation does not guarantee credentials were not stored.", DANGER));
        }
        body = body.push(label(
            if m.credential_stored {
                "A local credential is stored (not account validation)."
            } else {
                "No stored local credential is reported."
            },
            TEXT,
        ));
        if active(m.phase) || self.owner_uncertain {
            body = body.push(label("A sign-in attempt is active or uncertain. Other credential/settings edits are blocked until it settles.", ACCENT));
            if let Some(attempt) = m.attempt {
                body = body.push(
                    button("Open OpenAI login page (manual retry)")
                        .padding([10, 14])
                        .style(design::button_style)
                        .on_press_maybe(
                            (allowed
                                && m.browser_available
                                && !self.owner_uncertain
                                && !m.retry_blocked
                                && self.review.is_none())
                            .then_some(Action::OpenBrowser { ticket: t, attempt }),
                        ),
                );
            }
            if let Some(ticket) = self.draft_ticket() {
                let input = text_input("Optional: paste authorization callback", &self.draft.0)
                    .secure(true)
                    .padding(12)
                    .style(design::input_style);
                body = body.push(if allowed && self.review.is_none() {
                    input.on_input(move |raw| Action::Edit {
                        ticket,
                        input: Input::new(raw),
                    })
                } else {
                    input
                });
                if self.review == Some(Review::Callback(ticket)) {
                    body = body.push(label("Submit this masked callback only to the current OpenAI login attempt. Its contents will be consumed, not displayed or logged.", DANGER))
                        .push(button("Submit callback").padding([10, 14]).style(design::primary_button)
                            .on_press_maybe(allowed.then_some(Action::ConfirmCallback(ticket))))
                        .push(button("Cancel review").padding([8, 12]).style(design::button_style)
                            .on_press_maybe(allowed.then_some(Action::DismissReview(t))));
                } else {
                    body = body.push(
                        button("Review callback")
                            .padding([10, 14])
                            .style(design::button_style)
                            .on_press_maybe(
                                (allowed && !self.draft.0.is_empty())
                                    .then_some(Action::ReviewCallback(ticket)),
                            ),
                    );
                }
                body = body.push(
                    button("Discard callback")
                        .padding([8, 12])
                        .style(design::button_style)
                        .on_press_maybe(allowed.then_some(Action::DiscardCallback(ticket))),
                );
            }
        } else {
            body = body.push(
                button("Sign in with ChatGPT")
                    .padding([10, 14])
                    .style(design::primary_button)
                    .on_press_maybe(
                        (allowed && self.may_start() && self.review.is_none())
                            .then_some(Action::Start(t)),
                    ),
            );
        }
        if m.route_configured {
            body = body.push(label("Existing openai-codex model route is preserved. Use Load models explicitly and select a specific model for your session; sending remains a separate action.", MUTED));
        } else if self.may_enable() {
            if let Some(Review::Enable { ticket, revision }) = self.review {
                body = body.push(label(format!("Review: enable openai-codex at settings revision {revision}. Only a missing native-default route is added; existing configuration is not replaced."), ACCENT))
                    .push(button("Enable GPT models").padding([10, 14]).style(design::primary_button)
                        .on_press_maybe(allowed.then_some(Action::ConfirmEnable(ticket))))
                    .push(button("Cancel review").padding([8, 12]).style(design::button_style)
                        .on_press_maybe(allowed.then_some(Action::DismissReview(t))));
            } else {
                body = body.push(
                    button("Review enabling GPT models")
                        .padding([10, 14])
                        .style(design::button_style)
                        .on_press_maybe(allowed.then_some(Action::ReviewEnable(t))),
                );
            }
        }
        container(body).width(Length::Fill).into()
    }
}

fn active(phase: CodexPhase) -> bool {
    matches!(
        phase,
        CodexPhase::WaitingBrowser | CodexPhase::Indeterminate
    )
}
#[cfg(test)]
mod disclosure_tests {
    use super::*;

    fn context() -> Context {
        Context {
            epoch: 1,
            visible: true,
            enabled: true,
        }
    }

    #[test]
    fn advanced_toggle_is_queueless_and_does_not_revoke_connect_intent() {
        let mut state = Controller::default();
        state.enter(1);
        let Some(Effect::StatusRead { ticket }) =
            state.handle(Action::Connect(state.ticket()), context())
        else {
            panic!("the existing one-click status step must remain");
        };
        for expanded in [true, false] {
            assert!(
                state
                    .handle(Action::ToggleAdvanced(state.ticket()), context())
                    .is_none()
            );
            assert_eq!(state.advanced, expanded);
            assert_eq!(state.operation_ticket(), Some(ticket));
            assert_eq!(state.ticket(), ticket);
            assert!(
                matches!(state.connection, Some(Connection { step: ConnectStep::Checking(t), .. }) if t == ticket)
            );
        }
        state.completed(
            ticket,
            Outcome::Metadata(CodexMetadata {
                available: true,
                credential_stored: false,
                route_configured: false,
                phase: CodexPhase::Idle,
                attempt: None,
                prompt: None,
                browser_available: false,
                settings_revision: Some(7),
                retry_blocked: false,
            }),
        );
        assert!(matches!(
            state.continue_connection(context()),
            Some(Effect::StartLogin { .. })
        ));
    }

    #[test]
    fn advanced_rejects_hidden_disabled_wrong_epoch_and_stale_tickets() {
        let mut state = Controller::default();
        state.enter(1);
        for rejected in [
            Context {
                visible: false,
                ..context()
            },
            Context {
                enabled: false,
                ..context()
            },
            Context {
                epoch: 2,
                ..context()
            },
        ] {
            assert!(
                state
                    .handle(Action::ToggleAdvanced(state.ticket()), rejected)
                    .is_none()
            );
            assert!(!state.advanced);
            assert!(state.operation_ticket().is_none());
        }
        let old = state.ticket();
        state.leave();
        assert!(
            state
                .handle(Action::ToggleAdvanced(old), context())
                .is_none()
        );
        state.enter(1);
        assert!(
            state
                .handle(Action::ToggleAdvanced(old), context())
                .is_none()
        );
        assert!(!state.advanced);
        assert!(
            state
                .handle(Action::ToggleAdvanced(state.ticket()), context())
                .is_none()
        );
        assert!(state.advanced);
        assert!(state.operation_ticket().is_none());
    }

    #[test]
    fn advanced_resets_on_panel_and_epoch_changes_without_autoload() {
        let mut state = Controller::default();
        state.enter(1);
        assert!(!state.advanced);
        state.handle(Action::ToggleAdvanced(state.ticket()), context());
        assert!(state.advanced);
        state.leave();
        assert!(!state.advanced);
        state.enter(1);
        state.handle(Action::ToggleAdvanced(state.ticket()), context());
        let old = state.ticket();
        state.adopt_epoch(2);
        assert!(!state.advanced);
        assert!(
            state
                .handle(
                    Action::ToggleAdvanced(old),
                    Context {
                        epoch: 2,
                        ..context()
                    }
                )
                .is_none()
        );
        assert!(!state.advanced);
        assert!(state.metadata.is_none() && state.operation_ticket().is_none());
        assert!(
            state
                .continue_connection(Context {
                    epoch: 2,
                    ..context()
                })
                .is_none()
        );
    }
}
