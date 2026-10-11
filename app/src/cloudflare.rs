//! Explicit local Cloudflare configuration; navigation never dispatches requests.
use super::{Editor, Input, InputValue, Ticket};
use crate::{
    design,
    ui::{DANGER, MUTED, TEXT, label},
};
use dsh_native_core::{
    CloudflareAuthPhase, CloudflareAuthStatus, CloudflareOauthSetup, CloudflareSnapshot,
    SecretApiKey,
};
use iced::{
    Element, Length,
    widget::{button, checkbox, column, text_input},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug)]
pub enum Action {
    Connect(Ticket),
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
    pub fn mutation(&self) -> bool {
        !matches!(self, Self::Read(_) | Self::OpenBrowser { .. })
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
        self.account.clear();
        self.models = false;
        self.browser = false;
        self.free = false;
        self.panel += 1;
        self.serial += 1;
    }
    pub fn leave(&mut self) {
        self.clear();
        self.visible = false;
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
                        || !self.token.0.is_empty())))
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
            | Action::UseAuthMode(t, _) => *t,
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
            Action::ClientId(_, value) if self.can_write(enabled) => {
                if value.len() <= 256 && value.bytes().all(|b| (33..=126).contains(&b)) {
                    self.client_id = value;
                    self.changed();
                }
                None
            }
            Action::Scopes(_, value) if self.can_write(enabled) => {
                if value.len() <= 2048
                    && value
                        .bytes()
                        .all(|b| b == b' ' || b == b'\n' || (33..=126).contains(&b))
                {
                    self.scopes = value;
                    self.changed();
                }
                None
            }
            Action::CallbackPort(_, value) if self.can_write(enabled) => {
                if value.len() <= 5 && value.bytes().all(|b| b.is_ascii_digit()) {
                    self.port = value;
                    self.changed();
                }
                None
            }
            Action::SaveSetup(_) if self.can_write(enabled) => {
                let setup = CloudflareOauthSetup {
                    client_id: self.client_id.clone(),
                    scopes: self
                        .scopes
                        .split_ascii_whitespace()
                        .map(str::to_owned)
                        .collect(),
                    callback_port: self.port.parse().ok()?,
                };
                if !setup.valid(true) {
                    self.warning = Some((
                        "Invalid OAuth setup: use your own public client ID, exact registered scopes and a port from 1024 to 65535.",
                        true,
                    ));
                    return None;
                }
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
                let token = if self.token.0.is_empty() {
                    None
                } else {
                    match SecretApiKey::new(std::mem::take(&mut self.token.0)) {
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
                    self.changed();
                }
                None
            }
            Action::Token(_, _) if self.can_write(enabled) => {
                match input {
                    Some(InputValue::Valid(edit)) => self.token = edit.editor,
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
            if !auth.setup_configured || !auth.credential_writable || self.require_read {
                self.warning = Some((
                    "Save your own OAuth client setup and manual Account ID before connecting.",
                    true,
                ));
                self.setup_open = true;
                self.stop_chain();
                return None;
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
        let mut body = column![label("Cloudflare", TEXT).size(17),
            label("Use Workers Free. Paid accounts can bill. This app cannot enforce your Cloudflare account plan or global quota.", DANGER),
            label("Open the model selector and choose a Cloudflare model. Saving does not send a request or change this chat’s model. No account probes or automatic catalog switching.", MUTED),
            button("Read metadata explicitly").style(design::button_style).on_press_maybe((enabled && !self.pending() && self.reading.is_none()).then_some(Action::Read(ticket)))].spacing(14);
        if let Some(meta) = &self.metadata {
            body = body.push(label(
                format!(
                    "Token configured={} · writable={} · revision={}",
                    meta.token_configured, meta.writable, meta.revision
                ),
                MUTED,
            ));
        } else {
            body = body.push(label(
                "Metadata unknown; read explicitly before editing or writing.",
                MUTED,
            ));
        }
        let active =
            self.auth.as_ref().is_some_and(|a| a.phase.active()) || self.connection.is_some();
        let connected = self.auth.as_ref().is_some_and(|a| {
            a.credential_stored && a.auth_mode == dsh_native_core::CloudflareAuthMode::Oauth
        });
        body = body.push(label(
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
                        Some(Error::SetupRequired) => {
                            "complete client setup and save the account ID"
                        }
                        Some(Error::CallbackUnavailable) => {
                            "callback unavailable; check the registered port"
                        }
                        Some(Error::OauthDenied) => "authorization declined",
                        Some(Error::OauthFailed) => "sign-in failed",
                        Some(Error::ReconnectRequired) => "authorization needs renewal",
                        Some(Error::Conflict) => "configuration changed; read metadata",
                        Some(Error::Indeterminate) => {
                            "outcome unknown; read metadata before changes"
                        }
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
                }),
            if self
                .auth
                .as_ref()
                .is_some_and(|a| a.retry_blocked || a.error_code.is_some())
            {
                DANGER
            } else {
                MUTED
            },
        ));
        body = body.push(
            button(if active {
                "Cancel sign-in"
            } else if connected {
                "Cloudflare connected"
            } else {
                "Connect Cloudflare"
            })
            .style(design::primary_button)
            .on_press_maybe(
                (enabled
                    && if active {
                        self.cancel_pending.is_none()
                    } else {
                        !connected
                            && !self.pending()
                            && self.connection.is_none()
                            && !self.require_read
                            && !self.auth.as_ref().is_some_and(|a| a.retry_blocked)
                    })
                .then_some(if active {
                    Action::CancelConnect(ticket)
                } else {
                    Action::Connect(ticket)
                }),
            ),
        );
        if connected
            || self.auth.as_ref().is_some_and(|a| {
                a.auth_mode == dsh_native_core::CloudflareAuthMode::Oauth && a.retry_blocked
            })
        {
            body = body.push(
                button("Disconnect Cloudflare…")
                    .style(design::button_style)
                    .on_press_maybe(
                        (enabled && !self.pending() && !active)
                            .then_some(Action::ReviewDisconnect(ticket)),
                    ),
            );
        }
        if self.disconnect_review {
            body=body.push(label("Disconnect disables both features and deletes the local OAuth grant. Issuer revocation is attempted but cannot be guaranteed.",DANGER))
            .push(button("Confirm disconnect").style(design::button_style).on_press(Action::ConfirmDisconnect(ticket)))
            .push(button("Cancel review").on_press(Action::Cancel(ticket)));
        }
        body = body.push(
            button(if self.setup_open {
                "Hide OAuth client setup"
            } else {
                "OAuth client setup…"
            })
            .style(design::button_style)
            .on_press_maybe((enabled && !self.pending()).then_some(Action::ToggleSetup(ticket))),
        );
        if self.setup_open {
            body=body.push(label("Register your own account-private public OAuth client (no client secret). Enter its exact registered scope IDs. PKCE S256 is automatic. Saving switches to browser authentication; features stay off until reviewed.",MUTED));
            let client = text_input("Public client ID", &self.client_id)
                .padding(12)
                .style(design::input_style);
            let scopes = text_input("Registered scope IDs (space separated)", &self.scopes)
                .padding(12)
                .style(design::input_style);
            let port = text_input("Callback port (default 8977)", &self.port)
                .padding(12)
                .style(design::input_style);
            body = body
                .push(if editable {
                    client.on_input(move |v| Action::ClientId(ticket, v))
                } else {
                    client
                })
                .push(if editable {
                    scopes.on_input(move |v| Action::Scopes(ticket, v))
                } else {
                    scopes
                })
                .push(if editable {
                    port.on_input(move |v| Action::CallbackPort(ticket, v))
                } else {
                    port
                })
                .push(label(
                    format!(
                        "Registered redirect: http://127.0.0.1:{}/cloudflare/callback",
                        self.port
                    ),
                    MUTED,
                ))
                .push(
                    button("Save browser setup")
                        .style(design::button_style)
                        .on_press_maybe(editable.then_some(Action::SaveSetup(ticket))),
                );
        }
        let account = text_input("Account ID (32 lowercase hex)", &self.account)
            .padding(12)
            .style(design::input_style);
        body = body.push(if editable {
            account.on_input(move |value| Action::Account(ticket, value))
        } else {
            account
        });
        if let Some(auth) = &self.auth {
            let (title, mode) = if auth.auth_mode == dsh_native_core::CloudflareAuthMode::Oauth {
                (
                    "Use saved API token",
                    dsh_native_core::CloudflareAuthMode::ApiToken,
                )
            } else {
                (
                    "Use browser authentication",
                    dsh_native_core::CloudflareAuthMode::Oauth,
                )
            };
            body=body.push(button(title).style(design::button_style).on_press_maybe(editable.then_some(Action::UseAuthMode(ticket,mode))))
                .push(label("Changing authentication mode turns both features off. A nonblank API token save selects API-token mode; blank preserves the selected mode.",MUTED).size(12));
        }
        let token = text_input("API token (masked; blank keeps existing)", &self.token.0)
            .secure(true)
            .padding(12)
            .style(design::input_style);
        body = body.push(if editable {
            token.on_input(move |value| Action::Token(ticket, Input::new(value)))
        } else {
            token
        });
        let models = checkbox(self.models).label("Enable Workers AI models");
        let browser = checkbox(self.browser).label("Enable Browser Run");
        let free = checkbox(self.free)
            .label("REQUIRED: I confirm this account uses Workers Free for enabled features");
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
            });
        if self.review {
            body = body.push(label(format!("Confirm save: account {} · models={} · browser={} · Workers Free attested={}. {}", self.account, self.models, self.browser, self.free, if self.token.0.is_empty() { "Keep existing token." } else { "Replace stored token." }), DANGER))
                .push(button("Confirm Cloudflare save").style(design::primary_button).on_press_maybe(self.can_save(enabled).then_some(Action::Confirm(ticket))))
                .push(button("Cancel review").on_press(Action::Cancel(ticket)));
        } else {
            body = body.push(
                button("Review Cloudflare save")
                    .style(design::primary_button)
                    .on_press_maybe(self.can_save(enabled).then_some(Action::Review(ticket))),
            );
        }
        body = body.push(button("Disable both features").style(design::button_style).on_press_maybe(self.can_write(enabled).then_some(Action::Disable(ticket))))
            .push(label("Disabling retains the token. Draft token clears on navigation, close, disconnect and submission. Masking is not memory protection; editor, clipboard, Node and kernel copies are outside owned-buffer wiping.", MUTED).size(12));
        if let Some((warning, attention)) = self.footer() {
            body = body.push(label(warning, if attention { DANGER } else { MUTED }));
        }
        body.width(Length::Fill).into()
    }
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
