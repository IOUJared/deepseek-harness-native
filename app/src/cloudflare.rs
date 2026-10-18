//! Explicit local Cloudflare configuration; navigation never dispatches requests.
use super::{Editor, Input, InputValue, Ticket, key_input};
use crate::{
    design,
    ui::{ACCENT, DANGER, MUTED, TEXT, label},
};
use dsh_native_core::{
    CloudflareAuthPhase, CloudflareAuthStatus, CloudflareOauthSetup, CloudflareSnapshot,
    SecretApiKey,
};
use iced::{
    Element, Length,
    widget::{button, checkbox, column, row, text_input},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// The two Cloudflare pages that explain where an ID comes from. Constants this
/// app ships, so there is no authority to validate and nothing to withdraw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guide {
    OauthClient,
    AccountId,
}
impl Guide {
    pub fn url(self) -> &'static str {
        match self {
            Self::OauthClient => {
                "https://developers.cloudflare.com/fundamentals/oauth/create-an-oauth-client/"
            }
            Self::AccountId => {
                "https://developers.cloudflare.com/fundamentals/account/find-account-and-zone-ids/"
            }
        }
    }
}
/// The text fields on this page that accept a pasted value. Iced's single-line
/// text input implements no copy or paste binding at all, so the page tags the
/// fields it owns and the app routes the clipboard to whichever one has focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    ClientId,
    AccountId,
    Scopes,
    Port,
}
impl Field {
    pub const ALL: [Self; 4] = [Self::ClientId, Self::AccountId, Self::Scopes, Self::Port];
    pub fn id(self) -> &'static str {
        match self {
            Self::ClientId => "native-cloudflare-client",
            Self::AccountId => "native-cloudflare-account",
            Self::Scopes => "native-cloudflare-scopes",
            Self::Port => "native-cloudflare-port",
        }
    }
    pub fn of(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|field| field.id() == id)
    }
    /// The value this field holds now, so a paste can add to it rather than
    /// replace what the user already typed.
    pub fn value(self, controller: &Controller) -> &str {
        match self {
            Self::ClientId => &controller.client_id,
            Self::AccountId => &controller.account,
            Self::Scopes => &controller.scopes,
            Self::Port => &controller.port,
        }
    }
    /// The action that inserts pasted text at the end of this field.
    pub fn paste(self, ticket: Ticket, current: &str, text: &str) -> Action {
        let mut value = current.to_owned();
        value.push_str(text);
        match self {
            Self::ClientId => Action::ClientId(ticket, value),
            Self::AccountId => Action::Account(ticket, value),
            Self::Scopes => Action::Scopes(ticket, value),
            Self::Port => Action::CallbackPort(ticket, value),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    Connect(Ticket),
    CopyGuide(Ticket, Guide),
    /// Read the clipboard into one named field. Answered by the app rather than
    /// the Host, and offered as a button so it cannot depend on key delivery.
    RequestPaste(Field),
    CancelConnect(Ticket),
    ReviewDisconnect(Ticket),
    ConfirmDisconnect(Ticket),
    ToggleSetup(Ticket),
    ClientId(Ticket, String),
    Scopes(Ticket, String),
    CallbackPort(Ticket, String),
    SaveSetup(Ticket),
    UseAuthMode(Ticket, dsh_native_core::CloudflareAuthMode),
    Read(Ticket),
    Account(Ticket, String),
    Token(Ticket, Input),
    Models(Ticket, bool),
    Browser(Ticket, bool),
    Free(Ticket, bool),
    Review(Ticket),
    Confirm(Ticket),
    Cancel(Ticket),
    Disable(Ticket),
}
pub enum Effect {
    AuthMode {
        ticket: Ticket,
        revision: u64,
        mode: dsh_native_core::CloudflareAuthMode,
    },
    Setup {
        ticket: Ticket,
        revision: u64,
        setup: CloudflareOauthSetup,
    },
    Start {
        ticket: Ticket,
        revision: u64,
        permit: Arc<AtomicBool>,
    },
    OpenBrowser {
        ticket: Ticket,
        attempt: String,
        permit: Arc<AtomicBool>,
    },
    CancelLogin {
        ticket: Ticket,
        attempt: Option<String>,
    },
    Disconnect {
        ticket: Ticket,
        revision: u64,
        revoke: bool,
    },
    Read(Ticket),
    Save {
        ticket: Ticket,
        revision: u64,
        account: String,
        models: bool,
        browser: bool,
        free: bool,
        token: Option<SecretApiKey>,
    },
    Disable {
        ticket: Ticket,
        revision: u64,
    },
}
impl Effect {
    pub fn ticket(&self) -> Ticket {
        match self {
            Self::Read(t)
            | Self::Save { ticket: t, .. }
            | Self::Disable { ticket: t, .. }
            | Self::AuthMode { ticket: t, .. }
            | Self::Setup { ticket: t, .. }
            | Self::Start { ticket: t, .. }
            | Self::OpenBrowser { ticket: t, .. }
            | Self::CancelLogin { ticket: t, .. }
            | Self::Disconnect { ticket: t, .. } => *t,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Snapshot(CloudflareSnapshot),
    NotSent,
    Unknown,
    BrowserOpened,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum AuthOperation {
    Mode,
    Setup,
    Start,
    Browser,
    Disconnect,
}
struct Connection {
    permit: Arc<AtomicBool>,
    panel: u64,
    epoch: u64,
    started: bool,
    browser_issued: bool,
}
#[derive(Default)]
pub struct Controller {
    epoch: u64,
    panel: u64,
    serial: u64,
    visible: bool,
    /// The field the user last edited, so a paste has a destination even when
    /// the widget tree cannot name the focused field.
    last_field: Option<Field>,
    /// What the last clipboard action did, said on the page.
    clipboard_notice: Option<String>,
    metadata: Option<CloudflareSnapshot>,
    account: String,
    token: Editor,
    models: bool,
    browser: bool,
    free: bool,
    review: bool,
    reading: Option<Ticket>,
    pending: Option<Ticket>,
    require_read: bool,
    warning: Option<(&'static str, bool)>,
    auth: Option<CloudflareAuthStatus>,
    setup_open: bool,
    client_id: String,
    scopes: String,
    port: String,
    disconnect_review: bool,
    auth_pending: Option<(Ticket, AuthOperation)>,
    cancel_pending: Option<Ticket>,
    connection: Option<Connection>,
}
impl Controller {
    pub fn ticket(&self) -> Ticket {
        Ticket {
            epoch: self.epoch,
            panel: self.panel,
            serial: self.serial,
        }
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some() || self.auth_pending.is_some() || self.cancel_pending.is_some()
    }
    fn stop_chain(&mut self) {
        if let Some(connection) = self.connection.take() {
            connection.permit.store(false, Ordering::Release);
        }
    }
    fn clear(&mut self) {
        self.stop_chain();
        self.setup_open = false;
        self.disconnect_review = false;
        self.client_id.clear();
        self.scopes.clear();
        self.port.clear();
        self.token = Editor::default();
        self.review = false;
        self.reading = None;
        self.metadata = None;
        self.clipboard_notice = None;
        self.account.clear();
        self.models = false;
        self.browser = false;
        self.free = false;
        self.panel += 1;
        self.serial += 1;
    }
    pub fn leave(&mut self) {
        self.clear();
        self.last_field = None;
        self.visible = false;
    }
    /// Where an unaddressed paste goes: the field the user last edited, or the
    /// first still-empty field on a page nobody has typed in yet.
    /// Record what a clipboard action did, in the words the page shows.
    pub fn clipboard_result(&mut self, field: Field, characters: usize) {
        self.clipboard_notice = Some(format!(
            "Pasted {characters} character{} into {}.",
            if characters == 1 { "" } else { "s" },
            match field {
                Field::ClientId => "the client ID",
                Field::AccountId => "the account ID",
                Field::Scopes => "the scopes",
                Field::Port => "the callback port",
            }
        ));
    }
    pub fn clipboard_closed(&mut self) {
        self.clipboard_notice =
            Some("The Cloudflare page is not the one on screen; nothing was pasted.".to_owned());
    }
    pub fn clipboard_empty(&mut self) {
        self.clipboard_notice = Some("The clipboard held no text; nothing was pasted.".to_owned());
    }
    /// Whether Connect may be pressed. A metadata read in flight owns this page's serial, so a
    /// Connect that bumped it would have its reply dropped on arrival and leave `reading` set —
    /// the page read-only until Cancel. The view reads this, and it is the condition `can_write`
    /// applies, so the two cannot disagree.
    pub fn connect_available(&self, enabled: bool) -> bool {
        enabled
            && !self.pending()
            && self.reading.is_none()
            && self.connection.is_none()
            && !self.require_read
            && !self.auth.as_ref().is_some_and(|a| a.retry_blocked)
    }
    pub fn paste_target(&self) -> Option<Field> {
        self.last_field.or_else(|| {
            Field::ALL
                .into_iter()
                .find(|field| field.value(self).is_empty())
        })
    }
    /// A paste the field's own rules refused, said out loud rather than dropped.
    fn rejected_field(&mut self, field: Field) {
        self.warning = Some((
            match field {
                Field::ClientId => {
                    "The client ID accepts printable characters only; nothing was pasted."
                }
                Field::AccountId => {
                    "The account ID is 32 lowercase hex digits; nothing was pasted."
                }
                Field::Scopes => "Scopes accept letters, digits and spaces; nothing was pasted.",
                Field::Port => "The callback port is digits only; nothing was pasted.",
            },
            true,
        ));
    }
    pub fn enter(&mut self, epoch: u64) {
        self.adopt_epoch(epoch);
        self.clear();
        self.visible = true;
    }
    pub fn adopt_epoch(&mut self, epoch: u64) {
        if self.epoch != epoch {
            self.disconnect();
            self.epoch = epoch;
        }
    }
    pub fn disconnect(&mut self) {
        self.stop_chain();
        let config_pending = self.pending.take().is_some();
        let auth_pending = self.auth_pending.take().is_some();
        let cancel_pending = self.cancel_pending.take().is_some();
        if config_pending || auth_pending || cancel_pending {
            self.require_read = true;
            self.warning = Some((
                "Cloudflare write outcome unknown. Read metadata explicitly before another write; token identity cannot be verified. No blind retries.",
                true,
            ));
        }
        self.clear();
        self.auth = None;
        self.epoch = 0;
    }
    pub fn footer(&self) -> Option<(&'static str, bool)> {
        if self.cancel_pending.is_some() {
            Some((
                "Cancellation requested; an already-admitted credential commit may finish. No automatic retry.",
                true,
            ))
        } else if self.auth_pending.is_some()
            || self.auth.as_ref().is_some_and(|a| a.phase.active())
        {
            Some((
                "Cloudflare sign-in active. Closing stops future browser opens, not an already-admitted credential write.",
                false,
            ))
        } else if self.pending() {
            Some((
                "Cloudflare write pending; closing or switching categories cannot undo it.",
                false,
            ))
        } else {
            self.warning
        }
    }
    fn can_write(&self, enabled: bool) -> bool {
        enabled
            && self.visible
            && !self.pending()
            && self.reading.is_none()
            && !self.require_read
            && !self.auth.as_ref().is_some_and(|a| a.phase.active())
            && self
                .metadata
                .as_ref()
                .is_some_and(|m| m.writable && m.revision < 9_007_199_254_740_991)
    }
    fn can_save(&self, enabled: bool) -> bool {
        self.can_write(enabled)
            && valid_account(&self.account)
            && (!(self.models || self.browser)
                || (!self.account.is_empty()
                    && self.free
                    && (self.metadata.as_ref().is_some_and(|m| m.token_configured)
                        || !self.token.as_str().is_empty())))
    }
    pub fn handle(&mut self, action: Action, epoch: u64, enabled: bool) -> Option<Effect> {
        // Consume stale/replayed opaque holders too, never retain an invisible secret.
        let input = match &action {
            Action::Token(_, input) => input.take(),
            _ => None,
        };
        let ticket = match &action {
            Action::Connect(t)
            | Action::CancelConnect(t)
            | Action::ReviewDisconnect(t)
            | Action::ConfirmDisconnect(t)
            | Action::ToggleSetup(t)
            | Action::ClientId(t, _)
            | Action::Scopes(t, _)
            | Action::CallbackPort(t, _)
            | Action::SaveSetup(t)
            | Action::CopyGuide(t, _)
            | Action::UseAuthMode(t, _) => *t,
            Action::RequestPaste(_) => self.ticket(),
            Action::Read(t)
            | Action::Account(t, _)
            | Action::Token(t, _)
            | Action::Models(t, _)
            | Action::Browser(t, _)
            | Action::Free(t, _)
            | Action::Review(t)
            | Action::Confirm(t)
            | Action::Cancel(t)
            | Action::Disable(t) => *t,
        };
        if matches!(&action, Action::CancelConnect(_))
            && enabled
            && self.visible
            && epoch == self.epoch
            && ticket == self.ticket()
            && self.cancel_pending.is_none()
            && (self.connection.is_some() || self.auth.as_ref().is_some_and(|a| a.phase.active()))
        {
            let start_issued = self.connection.as_ref().is_some_and(|c| c.started);
            let active = self.auth.as_ref().is_some_and(|a| a.phase.active());
            self.stop_chain();
            self.serial += 1;
            if !start_issued && !active {
                self.reading = None;
                self.warning = Some((
                    "Connection cancelled before OAuth start; no login dispatched.",
                    false,
                ));
                return None;
            }
            let ticket = self.ticket();
            self.cancel_pending = Some(ticket);
            return Some(Effect::CancelLogin {
                ticket,
                attempt: self
                    .auth
                    .as_ref()
                    .filter(|a| a.phase.active())
                    .and_then(|a| a.attempt_id.clone()),
            });
        }
        if !enabled
            || !self.visible
            || epoch != self.epoch
            || ticket != self.ticket()
            || self.pending()
        {
            return None;
        }
        match action {
            Action::Connect(_)
                if !self
                    .auth
                    .as_ref()
                    .is_some_and(|a| a.phase.active() || a.retry_blocked)
                    && !self.require_read =>
            {
                self.stop_chain();
                self.serial += 1;
                self.connection = Some(Connection {
                    permit: Arc::new(AtomicBool::new(true)),
                    panel: self.panel,
                    epoch: self.epoch,
                    started: false,
                    browser_issued: false,
                });
                if self.metadata.is_none() {
                    let ticket = self.ticket();
                    self.reading = Some(ticket);
                    return Some(Effect::Read(ticket));
                }
                self.continue_connection(epoch, enabled)
            }
            Action::UseAuthMode(_, mode) if self.can_write(enabled) => {
                self.stop_chain();
                let revision = self.metadata.as_ref()?.revision;
                self.serial += 1;
                let ticket = self.ticket();
                self.auth_pending = Some((ticket, AuthOperation::Mode));
                Some(Effect::AuthMode {
                    ticket,
                    revision,
                    mode,
                })
            }
            Action::ToggleSetup(_) => {
                self.setup_open = !self.setup_open;
                None
            }
            // A field edit while the page is read-only is refused, and says why:
            // it looked identical to a broken paste before.
            Action::ClientId(_, value) | Action::Account(_, value) if !self.can_write(enabled) => {
                let _ = value;
                self.warning = Some((
                    "Read the Cloudflare metadata before editing: this page cannot write yet.",
                    true,
                ));
                None
            }
            Action::ClientId(_, value) if self.can_write(enabled) => {
                if value.len() <= 256 && value.bytes().all(|b| (33..=126).contains(&b)) {
                    self.client_id = value;
                    self.last_field = Some(Field::ClientId);
                    self.changed();
                } else {
                    self.rejected_field(Field::ClientId);
                }
                None
            }
            // Scopes, the port and the token were the silent ones: a paste or an edit while the
            // page cannot write fell through the final wildcard, so only the app's own "Pasted N
            // characters" line appeared. They refuse out loud now, like the two required fields.
            Action::Scopes(_, _) | Action::CallbackPort(_, _) | Action::Token(_, _)
                if !self.can_write(enabled) =>
            {
                self.warning = Some((
                    "Read the Cloudflare metadata before editing: this page cannot write yet.",
                    true,
                ));
                None
            }
            Action::Scopes(_, value) if self.can_write(enabled) => {
                if value.len() <= 2048
                    && value
                        .bytes()
                        .all(|b| b == b' ' || b == b'\n' || (33..=126).contains(&b))
                {
                    self.scopes = value;
                    self.last_field = Some(Field::Scopes);
                    self.changed();
                } else {
                    self.rejected_field(Field::Scopes);
                }
                None
            }
            Action::CallbackPort(_, value) if self.can_write(enabled) => {
                if value.len() <= 5 && value.bytes().all(|b| b.is_ascii_digit()) {
                    self.port = value;
                    self.last_field = Some(Field::Port);
                    self.changed();
                } else {
                    self.rejected_field(Field::Port);
                }
                None
            }
            Action::SaveSetup(_) if self.can_write(enabled) => {
                let setup = match self.typed_setup() {
                    Ok(setup) => setup,
                    Err(reason) => {
                        self.warning = Some((reason, true));
                        return None;
                    }
                };
                let revision = self.metadata.as_ref()?.revision;
                self.serial += 1;
                let ticket = self.ticket();
                self.auth_pending = Some((ticket, AuthOperation::Setup));
                Some(Effect::Setup {
                    ticket,
                    revision,
                    setup,
                })
            }
            Action::ReviewDisconnect(_)
                if self.can_write(enabled)
                    && !self.auth.as_ref().is_some_and(|a| a.phase.active()) =>
            {
                self.disconnect_review = true;
                None
            }
            Action::ConfirmDisconnect(_) if self.disconnect_review && self.can_write(enabled) => {
                self.stop_chain();
                self.disconnect_review = false;
                let revision = self.metadata.as_ref()?.revision;
                self.serial += 1;
                let ticket = self.ticket();
                self.auth_pending = Some((ticket, AuthOperation::Disconnect));
                Some(Effect::Disconnect {
                    ticket,
                    revision,
                    revoke: true,
                })
            }
            Action::Read(_) if self.reading.is_none() => {
                self.token = Editor::default();
                self.review = false;
                self.serial += 1;
                let ticket = self.ticket();
                self.reading = Some(ticket);
                Some(Effect::Read(ticket))
            }
            Action::Cancel(_) => {
                self.review = false;
                self.disconnect_review = false;
                self.serial += 1;
                None
            }
            Action::Review(_) if self.can_save(enabled) => {
                self.review = true;
                None
            }
            Action::Confirm(_) if self.review && self.can_save(enabled) => {
                let token = if self.token.as_str().is_empty() {
                    None
                } else {
                    match SecretApiKey::new(self.token.take()) {
                        Ok(token) => Some(token),
                        Err(_) => {
                            self.warning =
                                Some(("Invalid Cloudflare token; not sent. Draft cleared.", true));
                            self.review = false;
                            return None;
                        }
                    }
                };
                let revision = self.metadata.as_ref()?.revision;
                self.token = Editor::default();
                self.review = false;
                self.serial += 1;
                let ticket = self.ticket();
                self.pending = Some(ticket);
                Some(Effect::Save {
                    ticket,
                    revision,
                    account: self.account.clone(),
                    models: self.models,
                    browser: self.browser,
                    free: self.free,
                    token,
                })
            }
            Action::Disable(_) if self.can_write(enabled) => {
                let revision = self.metadata.as_ref()?.revision;
                self.token = Editor::default();
                self.review = false;
                self.serial += 1;
                let ticket = self.ticket();
                self.pending = Some(ticket);
                Some(Effect::Disable { ticket, revision })
            }
            Action::Account(_, value) if self.can_write(enabled) => {
                if value.len() <= 32
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    if value != self.account {
                        self.free = false;
                    }
                    self.account = value;
                    self.last_field = Some(Field::AccountId);
                    self.changed();
                } else {
                    self.rejected_field(Field::AccountId);
                }
                None
            }
            Action::Token(_, _) if self.can_write(enabled) => {
                match input {
                    Some(InputValue::Valid(editor)) => self.token = editor,
                    Some(InputValue::Rejected) => {
                        self.token = Editor::default();
                        self.warning = Some((
                            "Invalid Cloudflare token; draft cleared. Use printable non-whitespace ASCII, at most 16384 bytes.",
                            true,
                        ));
                    }
                    None => return None,
                }
                self.changed();
                None
            }
            Action::Models(_, value) if self.can_write(enabled) => {
                self.models = value;
                self.changed();
                None
            }
            Action::Browser(_, value) if self.can_write(enabled) => {
                self.browser = value;
                self.changed();
                None
            }
            Action::Free(_, value) if self.can_write(enabled) => {
                self.free = value;
                self.changed();
                None
            }
            _ => None,
        }
    }
    fn changed(&mut self) {
        self.review = false;
        self.serial += 1;
    }
    pub fn observed(&mut self, status: CloudflareAuthStatus) {
        if self.auth.as_ref().is_some_and(|old| {
            (status.auth_epoch, status.sequence) < (old.auth_epoch, old.sequence)
                || old.attempt_id == status.attempt_id
                    && !old.phase.active()
                    && old.phase != CloudflareAuthPhase::Idle
                    && status.phase.active()
        }) {
            return;
        }
        if status.retry_blocked
            || matches!(
                status.phase,
                CloudflareAuthPhase::Failed
                    | CloudflareAuthPhase::Cancelled
                    | CloudflareAuthPhase::Indeterminate
            )
        {
            self.stop_chain();
        }
        self.auth = Some(status);
    }
    pub fn continue_connection(&mut self, epoch: u64, enabled: bool) -> Option<Effect> {
        let c = self.connection.as_ref()?;
        if !enabled
            || !self.visible
            || epoch != self.epoch
            || c.epoch != self.epoch
            || c.panel != self.panel
            || !c.permit.load(Ordering::Acquire)
        {
            self.stop_chain();
            return None;
        }
        if self.pending() || self.reading.is_some() {
            return None;
        }
        let auth = self.auth.as_ref()?;
        if auth.retry_blocked {
            self.stop_chain();
            return None;
        }
        if !c.started {
            if auth.auth_mode == dsh_native_core::CloudflareAuthMode::Oauth
                && auth.credential_stored
                && auth.phase == CloudflareAuthPhase::Connected
            {
                self.stop_chain();
                return None;
            }
            if !auth.credential_writable || self.require_read {
                self.warning = Some((
                    "Save your own OAuth client setup and manual Account ID before connecting.",
                    true,
                ));
                self.setup_open = true;
                self.stop_chain();
                return None;
            }
            if !auth.setup_configured {
                // Connect is the only button the user should need: the client ID
                // and scopes typed on the page are saved here, then the chain
                // continues to the sign-in once the Host reports them saved.
                let setup = match self.typed_setup() {
                    Ok(setup) => setup,
                    Err(reason) => {
                        self.warning = Some((reason, true));
                        self.setup_open = true;
                        self.stop_chain();
                        return None;
                    }
                };
                let revision = self.metadata.as_ref()?.revision;
                self.serial += 1;
                let ticket = self.ticket();
                self.auth_pending = Some((ticket, AuthOperation::Setup));
                return Some(Effect::Setup {
                    ticket,
                    revision,
                    setup,
                });
            }
            let revision = self.metadata.as_ref()?.revision;
            let c = self.connection.as_mut()?;
            c.started = true;
            let permit = c.permit.clone();
            self.serial += 1;
            let ticket = self.ticket();
            self.auth_pending = Some((ticket, AuthOperation::Start));
            return Some(Effect::Start {
                ticket,
                revision,
                permit,
            });
        }
        if auth.phase == CloudflareAuthPhase::WaitingBrowser
            && auth.browser_available
            && !c.browser_issued
        {
            let attempt = auth.attempt_id.clone()?;
            let c = self.connection.as_mut()?;
            c.browser_issued = true;
            let permit = c.permit.clone();
            self.serial += 1;
            let ticket = self.ticket();
            self.auth_pending = Some((ticket, AuthOperation::Browser));
            return Some(Effect::OpenBrowser {
                ticket,
                attempt,
                permit,
            });
        }
        if !auth.phase.active() {
            self.stop_chain();
        }
        None
    }
    pub fn completed(&mut self, ticket: Ticket, outcome: Outcome) {
        if ticket.epoch != self.epoch {
            return;
        }
        if self.cancel_pending == Some(ticket)
            || self.auth_pending.is_some_and(|(t, _)| t == ticket)
        {
            let operation = self
                .auth_pending
                .filter(|(t, _)| *t == ticket)
                .map(|(_, op)| op);
            if self.cancel_pending == Some(ticket) {
                self.cancel_pending = None;
            }
            if operation.is_some() {
                self.auth_pending = None;
            }
            match outcome {
                Outcome::Snapshot(snapshot) => {
                    let attention = snapshot.auth_status.retry_blocked
                        || snapshot.auth_status.error_code.is_some();
                    self.observed(snapshot.auth_status.clone());
                    if self.visible && ticket.epoch == self.epoch && ticket.panel == self.panel {
                        self.apply_snapshot(snapshot);
                    }
                    if attention {
                        if self.auth.as_ref().is_some_and(|a| {
                            a.phase == CloudflareAuthPhase::Indeterminate
                                || a.error_code
                                    == Some(dsh_native_core::CloudflareAuthError::Indeterminate)
                        }) {
                            self.require_read = true;
                        }
                        self.warning = Some((
                            "Cloudflare authentication reported a failure. Read local metadata before retrying; no account plan or API access was verified.",
                            true,
                        ));
                        return;
                    }
                    self.warning = Some((
                        "Cloudflare operation acknowledged locally. Account plan, API access and issuer revocation are not independently verified; features and this chat’s model stay unchanged.",
                        false,
                    ));
                }
                Outcome::BrowserOpened => {}
                Outcome::NotSent => {
                    self.stop_chain();
                    self.warning = Some((
                        "Cloudflare operation not sent; retry only explicitly.",
                        true,
                    ));
                }
                Outcome::Unknown => {
                    self.stop_chain();
                    self.require_read = true;
                    self.warning = Some((
                        "Cloudflare outcome unknown; credentials may have changed. Read metadata explicitly, never blindly retry.",
                        true,
                    ));
                }
            }
            return;
        }
        if self.pending == Some(ticket) {
            self.pending = None;
            match &outcome {
                Outcome::Unknown => {
                    self.require_read = true;
                    self.warning = Some((
                        "Cloudflare write outcome unknown. Read metadata explicitly before another write; token identity cannot be verified. No blind retries.",
                        true,
                    ));
                }
                Outcome::NotSent => {
                    self.warning = Some((
                        "Cloudflare write NotSent: queue or ownership rejected it. No persistence attempted; retry only explicitly.",
                        true,
                    ))
                }
                Outcome::BrowserOpened => {}
                Outcome::Snapshot(_) => {
                    self.warning = Some((
                        "Cloudflare write acknowledged. No account plan, token validity or global quota was verified. Open the model selector and choose a Cloudflare model explicitly; no model/request change.",
                        false,
                    ));
                }
            }
            // The mutation receipt owns settlement even after close; never apply it to another draft.
            if !self.visible || self.epoch != ticket.epoch || self.panel != ticket.panel {
                return;
            }
        } else if self.reading == Some(ticket) && self.visible && ticket == self.ticket() {
            self.reading = None;
            if matches!(outcome, Outcome::Snapshot(_)) {
                self.require_read = false;
            } else {
                // A read cannot settle an unknown token write; keep its stronger warning.
                if !self.warning.is_some_and(|(_, attention)| attention) {
                    self.warning = Some((
                        "Cloudflare metadata unavailable. Read again explicitly; no automatic retry.",
                        true,
                    ));
                }
                self.metadata = None;
                return;
            }
        } else {
            return;
        }
        if let Outcome::Snapshot(snapshot) = outcome {
            self.observed(snapshot.auth_status.clone());
            self.apply_snapshot(snapshot);
        }
    }
    fn apply_snapshot(&mut self, snapshot: CloudflareSnapshot) {
        self.client_id = snapshot.oauth_setup.client_id.clone();
        self.scopes = snapshot.oauth_setup.scopes.join(" ");
        self.port = snapshot.oauth_setup.callback_port.to_string();
        self.account = snapshot.account_id.clone();
        self.models = snapshot.models_enabled;
        self.browser = snapshot.browser_enabled;
        self.free = snapshot.free_plan_confirmed;
        self.metadata = Some(snapshot);
        self.serial += 1;
    }
    pub fn view(&self, enabled: bool) -> Element<'_, Action> {
        let ticket = self.ticket();
        let editable = self.can_write(enabled) && !self.review;
        // The page leads with what has to be typed and the one action that
        // connects. Everything that is a refinement, a fallback or a
        // consequence lives under the disclosures below.
        let port = if self.port.is_empty() {
            "8977"
        } else {
            self.port.as_str()
        };
        let active =
            self.auth.as_ref().is_some_and(|a| a.phase.active()) || self.connection.is_some();
        let connected = self.auth.as_ref().is_some_and(|a| {
            a.credential_stored && a.auth_mode == dsh_native_core::CloudflareAuthMode::Oauth
        });
        let mut body = column![
            label("Cloudflare", TEXT).size(17),
            label(
                "Register one account-private public OAuth client (PKCE S256, no secret), then paste its ID here.                  Workers Free only: a paid account can bill, and this app cannot check your plan or quota.",
                DANGER,
            ),
        ]
        .spacing(10);
        body = body.push(label(self.status_line(), self.status_color()));
        let primary = if active {
            button("Cancel sign-in")
                .style(design::danger)
                .on_press_maybe(
                    (enabled && self.cancel_pending.is_none())
                        .then_some(Action::CancelConnect(ticket)),
                )
        } else if connected {
            button("Cloudflare connected").style(design::primary)
        } else {
            button("Connect Cloudflare")
                .style(design::primary)
                .on_press_maybe(
                    self.connect_available(enabled)
                        .then_some(Action::Connect(ticket)),
                )
        };
        if connected {
            body = body.push(
                row![
                    primary,
                    button("Disconnect…").style(design::ghost).on_press_maybe(
                        (enabled && !self.pending() && !active)
                            .then_some(Action::ReviewDisconnect(ticket)),
                    ),
                ]
                .spacing(6),
            );
        } else {
            body = body.push(primary);
        }
        if self.disconnect_review {
            body = body
                .push(label(
                    "Disconnect disables both features and deletes the local OAuth grant. Issuer revocation is attempted but cannot be guaranteed.",
                    DANGER,
                ))
                .push(
                    button("Confirm disconnect")
                        .style(design::danger)
                        .on_press(Action::ConfirmDisconnect(ticket)),
                )
                .push(button("Cancel review").on_press(Action::Cancel(ticket)));
        }
        // The two values the connect flow cannot do without, with the redirect
        // the client must be registered for.
        body = body.push(setup_field(
            "OAuth client ID",
            "Public client ID from your Cloudflare client",
            Field::ClientId,
            &self.client_id,
            editable,
            move |value| Action::ClientId(ticket, value),
        ));
        body = body.push(setup_field(
            "Account ID",
            "32 lowercase hex digits from the dashboard URL",
            Field::AccountId,
            &self.account,
            editable,
            move |value| Action::Account(ticket, value),
        ));
        if let Some(notice) = &self.clipboard_notice {
            body = body.push(label(notice.clone(), MUTED).size(11));
        }
        body = body
            .push(label(
                format!("Register this redirect: http://127.0.0.1:{port}/cloudflare/callback"),
                MUTED,
            ))
            .push(guide_row(
                "Where to find the OAuth client ID",
                Guide::OauthClient,
                ticket,
            ))
            .push(guide_row(
                "Where to find the account ID",
                Guide::AccountId,
                ticket,
            ));
        // Everything below is optional, secondary or a disclosure the user asked
        // for; none of it is needed to connect an account.
        body = body.push(disclosure(
            self.setup_open,
            "Client and feature details",
            Action::ToggleSetup(ticket),
            enabled && !self.pending(),
        ));
        if self.setup_open {
            body = body
                .push(label(
                    "Scopes must be exactly what the client is registered for; the port must match the redirect above.",
                    MUTED,
                ))
                .push(setup_field(
                    "Registered scopes",
                    "Space separated, e.g. account.read offline_access",
                    Field::Scopes,
                    &self.scopes,
                    editable,
                    move |value| Action::Scopes(ticket, value),
                ))
                .push(setup_field(
                    "Callback port",
                    "Registered redirect port (default 8977)",
                    Field::Port,
                    &self.port,
                    editable,
                    move |value| Action::CallbackPort(ticket, value),
                ))
                // Saving the client was reachable only from a test: the fields stayed editable
                // while nothing could persist them, so a registered client could never be
                // replaced. The label names exactly what it writes.
                .push(
                    button("Save OAuth client (ID, scopes, port)")
                        .style(design::ghost)
                        .on_press_maybe(editable.then_some(Action::SaveSetup(ticket))),
                );
            let models = checkbox(self.models).label("Workers AI models");
            let browser = checkbox(self.browser).label("Browser Run");
            let free = checkbox(self.free)
                .label("Required: this account uses Workers Free for what is enabled");
            body = body
                .push(if editable {
                    models.on_toggle(move |value| Action::Models(ticket, value))
                } else {
                    models
                })
                .push(if editable {
                    browser.on_toggle(move |value| Action::Browser(ticket, value))
                } else {
                    browser
                })
                .push(if editable {
                    free.on_toggle(move |value| Action::Free(ticket, value))
                } else {
                    free
                })
                .push(label(
                    "The account ID, the feature switches and the Workers Free confirmation are saved together by the reviewed save below; enabling them stays off until you confirm it.",
                    MUTED,
                ))
                .push(label(
                    "Prefer an API token? It replaces browser authentication and needs no client.",
                    MUTED,
                ));
            if let Some(auth) = &self.auth {
                let (title, mode) = if auth.auth_mode == dsh_native_core::CloudflareAuthMode::Oauth
                {
                    (
                        "Use an API token instead",
                        dsh_native_core::CloudflareAuthMode::ApiToken,
                    )
                } else {
                    (
                        "Use browser authentication",
                        dsh_native_core::CloudflareAuthMode::Oauth,
                    )
                };
                body = body.push(
                    button(title)
                        .style(design::ghost)
                        .on_press_maybe(editable.then_some(Action::UseAuthMode(ticket, mode))),
                );
            }
            let token = text_input(
                "API token (masked; blank keeps the stored one)",
                self.token.as_str(),
            )
            .secure(true)
            .padding(12)
            .style(design::field);
            body = body.push(if editable {
                token.on_input(move |value| Action::Token(ticket, key_input(value)))
            } else {
                token
            });
        }
        if self.review {
            body = body
                .push(label(
                    format!(
                        "Confirm save: account {} · models={} · browser={} · Workers Free attested={}. {}",
                        self.account,
                        self.models,
                        self.browser,
                        self.free,
                        if self.token.as_str().is_empty() {
                            "Keep existing token."
                        } else {
                            "Replace stored token."
                        }
                    ),
                    DANGER,
                ))
                .push(
                    button("Confirm Cloudflare save")
                        .style(design::primary)
                        .on_press_maybe(self.can_save(enabled).then_some(Action::Confirm(ticket))),
                )
                .push(button("Cancel review").on_press(Action::Cancel(ticket)));
        } else {
            body = body.push(
                button("Save account and features")
                    .style(design::primary)
                    .on_press_maybe(self.can_save(enabled).then_some(Action::Review(ticket))),
            );
        }
        body = body.push(
            row![
                button("Read metadata").style(design::ghost).on_press_maybe(
                    (enabled && !self.pending() && self.reading.is_none())
                        .then_some(Action::Read(ticket)),
                ),
                button("Disable both features")
                    .style(design::ghost)
                    .on_press_maybe(self.can_write(enabled).then_some(Action::Disable(ticket))),
            ]
            .spacing(6),
        );
        body = body.push(label(
            match &self.metadata {
                Some(meta) => format!(
                    "Token configured={} · writable={} · revision={}",
                    meta.token_configured, meta.writable, meta.revision
                ),
                None => "Metadata not read yet; read it before editing or writing.".to_owned(),
            },
            MUTED,
        ));
        body = body.push(label(
            "Saving sends no request and changes no chat model. Disabling retains the token; the token draft clears on navigation, close, disconnect and submission. Masking is not memory protection.",
            MUTED,
        ));
        if let Some((warning, attention)) = self.footer() {
            body = body.push(label(warning, if attention { DANGER } else { MUTED }));
        }
        body.width(Length::Fill).into()
    }
    /// The client setup exactly as typed: the port defaults to the one the page
    /// shows as the registered redirect, and scopes are the registered IDs in the
    /// order the user wrote them.
    fn typed_setup(&self) -> Result<CloudflareOauthSetup, &'static str> {
        let port = if self.port.trim().is_empty() {
            8977
        } else {
            self.port
                .trim()
                .parse()
                .map_err(|_| "The callback port must be a number from 1024 to 65535.")?
        };
        let setup = CloudflareOauthSetup {
            client_id: self.client_id.trim().to_owned(),
            scopes: self
                .scopes
                .split_ascii_whitespace()
                .map(str::to_owned)
                .collect(),
            callback_port: port,
        };
        if !setup.valid(true) {
            return Err(
                "Invalid OAuth setup: paste your own public client ID, its exact registered scopes and a port from 1024 to 65535.",
            );
        }
        Ok(setup)
    }
    fn status_line(&self) -> String {
        self.auth
            .as_ref()
            .map_or("Connection status unknown".to_owned(), |a| {
                use dsh_native_core::{
                    CloudflareAuthError as Error, CloudflareAuthMode as Mode,
                    CloudflareAuthPhase as Phase,
                };
                let mode = if a.auth_mode == Mode::Oauth {
                    "Browser authentication"
                } else {
                    "API token"
                };
                let state = match a.error_code {
                    Some(Error::SetupRequired) => "client ID and account ID are needed",
                    Some(Error::CallbackUnavailable) => {
                        "callback unavailable; check the registered port"
                    }
                    Some(Error::OauthDenied) => "authorization declined",
                    Some(Error::OauthFailed) => "sign-in failed",
                    Some(Error::ReconnectRequired) => "authorization needs renewal",
                    Some(Error::Conflict) => "configuration changed; read metadata",
                    Some(Error::Indeterminate) => "outcome unknown; read metadata before changes",
                    None => match a.phase {
                        Phase::Idle => {
                            if a.credential_stored {
                                "credential stored"
                            } else {
                                "not connected"
                            }
                        }
                        Phase::Starting => "preparing sign-in",
                        Phase::WaitingBrowser => "finish sign-in in your browser",
                        Phase::Exchanging => "verifying authorization",
                        Phase::Committing => "saving credential",
                        Phase::Connected => "authorized",
                        Phase::Cancelled => "sign-in cancelled",
                        Phase::Failed => "sign-in failed",
                        Phase::Indeterminate => "outcome unknown; read metadata before changes",
                    },
                };
                format!(
                    "{mode}: {state}{}",
                    if a.retry_blocked && a.error_code != Some(Error::Indeterminate) {
                        "; explicit read required"
                    } else {
                        ""
                    }
                )
            })
    }
    fn status_color(&self) -> iced::Color {
        if self
            .auth
            .as_ref()
            .is_some_and(|a| a.retry_blocked || a.error_code.is_some())
        {
            DANGER
        } else {
            MUTED
        }
    }
}

/// A documentation pointer: the label says what it explains, and the button
/// copies the link so it can be opened without retyping a URL.
fn guide_row<'a>(title: &'a str, guide: Guide, ticket: Ticket) -> Element<'a, Action> {
    row![
        label(title, MUTED).size(11).width(Length::Fill),
        button(label("Copy link", ACCENT).size(11))
            .style(design::ghost)
            .padding([4, 8])
            .on_press(Action::CopyGuide(ticket, guide)),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

/// One labelled setup field: the placeholder says what to paste, the label says
/// what it is, and a disabled field still shows what is stored.
fn setup_field<'a>(
    title: &'a str,
    hint: &'a str,
    tag: Field,
    value: &'a str,
    editable: bool,
    on_input: impl Fn(String) -> Action + 'a,
) -> Element<'a, Action> {
    let field = text_input(hint, value)
        .id(tag.id())
        .padding(12)
        .style(design::field);
    // The button is the same read the shortcut performs, offered without a
    // keyboard: a modal is exactly where a shortcut is easiest to miss.
    let paste = button(label("Paste", ACCENT).size(11))
        .style(design::ghost)
        .padding([6, 9])
        .on_press_maybe(editable.then_some(Action::RequestPaste(tag)));
    column![
        label(title, TEXT).size(12),
        row![
            if editable {
                field.on_input(on_input)
            } else {
                field
            }
            .width(Length::Fill),
            paste,
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(3)
    .into()
}

/// A secondary section the user opens on purpose.
fn disclosure<'a>(
    open: bool,
    title: &'a str,
    action: Action,
    enabled: bool,
) -> Element<'a, Action> {
    button(label(
        if open {
            format!("▾ {title}")
        } else {
            format!("▸ {title}")
        },
        MUTED,
    ))
    .style(design::ghost)
    .on_press_maybe(enabled.then_some(action))
    .into()
}

fn valid_account(account: &str) -> bool {
    account.is_empty()
        || (account.len() == 32
            && account
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}

#[cfg(test)]
#[path = "cloudflare_tests.rs"]
mod tests;
