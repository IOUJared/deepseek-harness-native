//! Explicit native key persistence. Secret edits are opaque, one-use and never serialized.
use crate::design;
use crate::ui::{ACCENT, DANGER, MUTED, SURFACE, TEXT, label};
use dsh_native_core::{OnboardingMetadata, SecretApiKey};
use iced::widget::{button, column, container, row, scrollable, text_input};
use iced::{Element, Length};
const MAX_DRAFT: usize = 16_384;
/// Why a draft cannot be a key at all, in the desktop shell's own words
/// (`welcomeKeyInvalid`): it arrived wrapped in quotes, or copied as an
/// environment assignment such as `DEEPSEEK_API_KEY=…`.
///
/// This is deliberately *not* part of [`crate::secret_input::Input::ascii`]: that
/// rule is the Core frame's and is shared with callbacks, where `=` is ordinary
/// (`?code=…`, base64 padding). Whitespace needs no check here because the shared
/// rule already refuses every byte outside `0x21..=0x7e`.
fn key_shape_error(draft: &str) -> Option<&'static str> {
    const MESSAGE: &str =
        "Enter the API key itself, without quotes, spaces, or an environment-variable assignment.";
    let quoted = draft.len() >= 2
        && ((draft.starts_with('"') && draft.ends_with('"'))
            || (draft.starts_with('\'') && draft.ends_with('\'')));
    (quoted || draft.contains('=')).then_some(MESSAGE)
}
#[path = "account.rs"]
pub(crate) mod account;
#[path = "cloudflare.rs"]
pub(crate) mod cloudflare;
#[cfg(test)]
#[path = "codex_integration_tests.rs"]
mod codex_integration_tests;
#[cfg(test)]
#[path = "plugin_integration_tests.rs"]
mod plugin_integration_tests;
#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "settings_uncertainty_tests.rs"]
mod uncertainty_tests;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ticket {
    pub epoch: u64,
    pub panel: u64,
    pub serial: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveResult {
    Confirmed,
    Refused,
    Indeterminate,
    NotSent,
}
impl SaveResult {
    pub fn from_core(result: Result<bool, dsh_native_core::Error>) -> Self {
        match result {
            Ok(true) => Self::Confirmed,
            Ok(false) => Self::Refused,
            Err(_) => Self::Indeterminate,
        }
    }
    fn text(self) -> &'static str {
        match self {
            Self::Confirmed => "Saved. This does not sign in or validate the key.",
            Self::Refused => "The local backend refused the save. Nothing was written.",
            Self::Indeterminate => {
                "Save outcome unknown: the key may have been written. No automatic retry."
            }
            Self::NotSent => "Not sent. Re-enter the key to try again.",
        }
    }
}
/// The display name for one advertised preset, falling back to its own key, which is what
/// the Host's `optionOf` does for a missing label.
fn permission_display(current: &str, options: &[(String, String)]) -> String {
    options
        .iter()
        .find(|(value, _)| value == current)
        .map(|(_, name)| name.clone())
        .unwrap_or_else(|| current.to_owned())
}

/// Backend-reported DeepSeek route facts for the Models page. Display only; the
/// App owns the catalog read and the active selection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RouteFacts {
    /// `(display name, model id, active)` rows the backend reports for the route.
    pub models: Vec<(String, String, bool)>,
    /// True while the catalog read is still pending.
    pub loading: bool,
    /// True when the catalog read failed.
    pub failed: bool,
    /// The deployment's new-session permission default, or `None` when it exposes none —
    /// in which case the General page renders no row at all, as the reference does.
    pub permission: Option<PermissionDefault>,
}

/// The General page's permission row: the deployment's new-session default, the revision a
/// write must match, whether it is writable at all, and the presets it advertises for new
/// sessions (`PermissionRow.tsx:1-40`, `settings-store.ts:45-58`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionDefault {
    pub current: String,
    pub revision: u64,
    pub writable: bool,
    /// `(preset value, display name)` pairs.
    pub options: Vec<(String, String)>,
    /// A write is in flight, so the row is busy.
    pub saving: bool,
}
use crate::secret_input::Editor;
/// The one-use key draft the panel hands to the app.
pub use crate::secret_input::Input;
use crate::secret_input::Value as InputValue;
/// A typed API key, bounded by the same printable-ASCII rule the Core frame
/// enforces on the serialized save.
pub fn key_input(raw: String) -> Input {
    Input::ascii(raw, MAX_DRAFT)
}
/// Clipboard paste. Copied keys routinely carry a trailing newline or surrounding
/// spaces, so those are trimmed before that rule; the secret itself is unchanged.
pub fn pasted_key_input(raw: String) -> Input {
    key_input(raw.trim().to_owned())
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    General,
    #[default]
    ApiLogin,
    Codex,
    Plugins,
    AgentPresets,
    Cloudflare,
}
impl Page {
    const ALL: [Self; 6] = [
        Self::General,
        Self::ApiLogin,
        Self::Plugins,
        Self::Codex,
        Self::AgentPresets,
        Self::Cloudflare,
    ];
    fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::ApiLogin => "Models",
            Self::Codex => "Codex",
            Self::Plugins => "Built-in plugins",
            Self::AgentPresets => "Agent presets",
            Self::Cloudflare => "Cloudflare",
        }
    }
    fn glyph(self) -> &'static str {
        match self {
            Self::General => "⚙",
            Self::ApiLogin => "◇",
            Self::Plugins => "☷",
            Self::Codex => "✧",
            Self::AgentPresets => "⌘",
            Self::Cloudflare => "☁",
        }
    }
}
/// Static help only; these disclosures never read or change backend settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Info {
    Details,
    Shortcuts,
    ApiHelp,
    ContextPolicy,
}
impl Info {
    fn page(self) -> Page {
        match self {
            Self::Details | Self::Shortcuts | Self::ContextPolicy => Page::General,
            Self::ApiHelp => Page::ApiLogin,
        }
    }
}
#[derive(Clone)]
pub enum Action {
    Open,
    Close,
    SelectPage(Page),
    ToggleInfo {
        ticket: Ticket,
        info: Info,
    },
    /// Opens or dismisses the permission default row's preset list.
    TogglePermissionDefault,
    /// Persists one new-session permission default; the App gates it on the descriptor.
    PermissionDefault(String),
    Account(account::Action),
    Cloudflare(cloudflare::Action),
    Plugins(crate::plugins::Action),
    Codex(crate::codex::Action),
    Refresh(Ticket),
    Edit {
        ticket: Ticket,
        input: Input,
    },
    /// Clipboard read request; the App performs the read and replies with
    /// [`Action::Pasted`]. Reads no secret until the reply carries one.
    Paste {
        ticket: Ticket,
    },
    /// Clipboard result as a one-use holder, so no plaintext enters the
    /// message or action. The App trims and validates before sending it.
    Pasted {
        ticket: Ticket,
        input: Input,
    },
    /// The clipboard read returned nothing usable; no draft is touched.
    PasteUnavailable {
        ticket: Ticket,
    },
    Review(Ticket),
    CancelReview(Ticket),
    Confirm(Ticket),
}
impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeSettingsAction([REDACTED])")
    }
}
pub enum Effect {
    Account(account::Effect),
    Cloudflare(cloudflare::Effect),
    Plugins(crate::plugins::Effect),
    Codex(crate::codex::Effect),
    Read(Ticket),
    Save {
        ticket: Ticket,
        secret: SecretApiKey,
    },
}
enum SavePhase {
    Idle,
    Confirm(Ticket),
    Pending(Ticket),
    Done(SaveResult),
    EndedElsewhere,
}
pub struct Settings {
    open: bool,
    page: Page,
    info_generation: u64,
    details_open: bool,
    shortcuts_open: bool,
    /// The permission default row's preset list is open.
    permission_open: bool,
    api_help_open: bool,
    context_policy_open: bool,
    account: account::Controller,
    cloudflare: cloudflare::Controller,
    plugins: crate::plugins::Controller,
    codex: crate::codex::Controller,
    epoch: u64,
    panel: u64,
    serial: u64,
    editor_generation: u64,
    edit: Editor,
    metadata: Option<OnboardingMetadata>,
    reading: Option<Ticket>,
    save: SavePhase,
    // Unknown write receipt outlives panel/epoch changes; metadata cannot settle this attempt.
    uncertain_save: Option<Ticket>,
    error: Option<&'static str>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            open: false,
            page: Page::default(),
            info_generation: 0,
            details_open: false,
            shortcuts_open: false,
            permission_open: false,
            api_help_open: false,
            context_policy_open: false,
            account: account::Controller::default(),
            cloudflare: cloudflare::Controller::default(),
            plugins: crate::plugins::Controller::default(),
            codex: crate::codex::Controller::default(),
            epoch: 0,
            panel: 0,
            serial: 0,
            editor_generation: 0,
            edit: Editor::default(),
            metadata: None,
            reading: None,
            save: SavePhase::Idle,
            uncertain_save: None,
            error: None,
        }
    }
}
impl Settings {
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn layout_codex_tab(&mut self, tab: crate::codex::Tab) {
        let context = self.codex_context(false);
        assert!(
            self.codex
                .handle(
                    crate::codex::Action::SelectTab {
                        ticket: self.codex.ticket(),
                        tab
                    },
                    context
                )
                .is_none()
        );
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    fn info_ticket(&self) -> Ticket {
        Ticket {
            epoch: self.epoch,
            panel: self.panel,
            serial: self.info_generation,
        }
    }
    fn reset_info(&mut self) {
        self.info_generation += 1;
        self.details_open = false;
        self.shortcuts_open = false;
        self.api_help_open = false;
        self.context_policy_open = false;
    }
    fn info_button(&self, info: Info, title: &'static str, expanded: bool) -> Element<'_, Action> {
        button(
            label(
                format!("{} {title}", if expanded { "▾" } else { "▸" }),
                MUTED,
            )
            .size(12),
        )
        .padding([6, 8])
        .style(design::ghost)
        .on_press(Action::ToggleInfo {
            ticket: self.info_ticket(),
            info,
        })
        .into()
    }
    fn plugin_context(&self, enabled: bool) -> crate::plugins::Context {
        crate::plugins::Context {
            epoch: self.epoch,
            // The DeepSeek page renders the one `llm-deepseek` namespace, so it
            // needs the same live-settings authority as the plugin panel.
            visible: self.open && matches!(self.page, Page::Plugins | Page::ApiLogin),
            enabled: enabled
                && !self.pending()
                && !self.codex.pending()
                && !self.cloudflare.pending()
                && !self.account.pending(),
        }
    }
    fn codex_context(&self, enabled: bool) -> crate::codex::Context {
        crate::codex::Context {
            epoch: self.epoch,
            visible: self.open && self.page == Page::Codex,
            enabled: enabled
                && !self.pending()
                && !self.plugins.pending()
                && !self.cloudflare.pending()
                && !self.account.pending(),
        }
    }
    /// Only the API login page runs the account flow, and only while no other
    /// operation owns the panel.
    fn account_enabled(&self, enabled: bool) -> bool {
        enabled
            && self.open
            && self.page == Page::ApiLogin
            && !self.pending()
            && !self.plugins.pending()
            && !self.codex.pending()
            && !self.cloudflare.pending()
    }
    pub(crate) fn subscribe_account(&mut self) -> Option<account::Effect> {
        self.account.subscribe_effect()
    }
    /// The Cloudflare page's own ticket, when that page is the one on screen, so
    /// clipboard routing cannot reach a page the user is not looking at.
    pub(crate) fn cloudflare_field_ticket(&self) -> Option<crate::settings::Ticket> {
        // The page being on screen is the whole gate here: the controller's own
        // guards still check the ticket, panel and epoch it is handed.
        (self.open && self.page == Page::Cloudflare).then(|| self.cloudflare.ticket())
    }
    pub(crate) fn cloudflare_fields(&self) -> Option<&cloudflare::Controller> {
        (self.open && self.page == Page::Cloudflare).then_some(&self.cloudflare)
    }
    pub(crate) fn cloudflare_clipboard_result(
        &mut self,
        field: cloudflare::Field,
        characters: usize,
    ) {
        self.cloudflare.clipboard_result(field, characters);
    }
    pub(crate) fn cloudflare_clipboard_closed(&mut self) {
        self.cloudflare.clipboard_closed();
    }
    pub(crate) fn cloudflare_clipboard_empty(&mut self) {
        self.cloudflare.clipboard_empty();
    }
    pub(crate) fn account_observed(&mut self, metadata: dsh_native_core::AccountMetadata) {
        self.account.observed(metadata);
    }
    pub(crate) fn account_completed(&mut self, ticket: Ticket, outcome: account::Outcome) {
        self.account.completed(ticket, outcome);
    }
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn cloudflare_ticket(&self) -> Ticket {
        self.cloudflare.ticket()
    }
    pub(crate) fn cloudflare_completed(&mut self, ticket: Ticket, outcome: cloudflare::Outcome) {
        self.cloudflare.completed(ticket, outcome);
    }
    pub(crate) fn cloudflare_observed(&mut self, status: dsh_native_core::CloudflareAuthStatus) {
        self.cloudflare.observed(status);
    }
    pub(crate) fn cloudflare_continue(&mut self, enabled: bool) -> Option<cloudflare::Effect> {
        let enabled =
            self.cloudflare_enabled(enabled) && self.open && self.page == Page::Cloudflare;
        self.cloudflare.continue_connection(self.epoch, enabled)
    }
    fn cloudflare_enabled(&self, enabled: bool) -> bool {
        enabled && !self.pending() && !self.plugins.pending() && !self.codex.pending()
    }
    pub(crate) fn codex_completed(
        &mut self,
        ticket: crate::codex::Ticket,
        outcome: crate::codex::Outcome,
    ) {
        self.codex.completed_native(ticket, outcome);
    }
    pub(crate) fn codex_observed(&mut self, metadata: dsh_native_core::CodexMetadata) {
        self.codex.observed(metadata);
    }
    /// Continue only a connection explicitly started in this visible, enabled panel.
    pub(crate) fn codex_continue(&mut self, enabled: bool) -> Option<crate::codex::Effect> {
        let context = self.codex_context(enabled);
        self.codex.continue_connection(context)
    }
    /// The Codex panel's own ticket, for the composed examples and the tests that
    /// drive it; the panel mints its tickets from the live context internally.
    #[allow(dead_code)]
    pub(crate) fn codex_ticket(&self) -> crate::codex::Ticket {
        self.codex.ticket()
    }
    /// The reviewed-or-draft Codex callback ticket, read by the composed Codex
    /// example and the tests.
    #[allow(dead_code)]
    pub(crate) fn codex_draft_ticket(&self) -> Option<crate::codex::DraftTicket> {
        self.codex.draft_ticket()
    }
    /// Nonsecret view tickets; every action remains subject to panel and backend
    /// guards. The layout fixture and the composed dialog example read them.
    #[allow(dead_code)]
    pub(crate) fn plugin_read_ticket(&self) -> crate::plugins::ReadTicket {
        self.plugins.read_ticket()
    }
    /// The reviewed plugin-field ticket, read by the composed dialog example and
    /// the tests.
    #[allow(dead_code)]
    pub(crate) fn plugin_draft_ticket(&self) -> Option<crate::plugins::DraftTicket> {
        self.plugins.draft_ticket()
    }
    pub(crate) fn plugins_loaded(
        &mut self,
        ticket: crate::plugins::ReadTicket,
        result: Result<crate::plugins::Snapshot, crate::plugins::ReadFailure>,
    ) {
        self.plugins.loaded(ticket, result);
    }
    pub(crate) fn plugin_saved(
        &mut self,
        ticket: crate::plugins::WriteTicket,
        result: crate::plugins::SaveOutcome,
    ) {
        self.plugins.saved(ticket, result);
    }
    pub(crate) fn ticket(&self) -> Ticket {
        Ticket {
            epoch: self.epoch,
            panel: self.panel,
            serial: self.serial,
        }
    }
    fn next(&mut self) -> Ticket {
        self.serial += 1;
        self.ticket()
    }
    fn current(&self, ticket: Ticket) -> bool {
        self.open && ticket == self.ticket()
    }
    fn pending(&self) -> bool {
        matches!(self.save, SavePhase::Pending(_))
    }
    /// Nonsecret editor-lifetime fence, independent of per-operation/review serials.
    pub fn input_ticket(&self) -> Ticket {
        Ticket {
            epoch: self.epoch,
            panel: self.panel,
            serial: self.editor_generation,
        }
    }
    /// Startup readiness can arrive after the user opened this panel. Rebind safely;
    /// never read or write automatically, and never restore an old draft/confirmation.
    pub fn adopt_epoch(&mut self, epoch: u64) {
        self.cloudflare.adopt_epoch(epoch);
        self.plugins.adopt_epoch(epoch);
        self.codex.adopt_epoch(epoch);
        if self.epoch == epoch {
            return;
        }
        self.clear();
        self.reset_info();
        self.panel += 1;
        self.serial += 1;
        self.epoch = epoch;
        self.metadata = None;
        self.reading = None;
        self.error = None;
        if let SavePhase::Pending(ticket) = self.save {
            self.uncertain_save = Some(ticket);
            self.save = SavePhase::Done(SaveResult::Indeterminate);
        } else if matches!(self.save, SavePhase::Confirm(_)) {
            self.save = SavePhase::Idle;
        }
    }
    fn clear(&mut self) {
        self.editor_generation += 1;
        self.edit = Editor::default();
    }
    pub fn close(&mut self) {
        self.account.leave();
        self.cloudflare.leave();
        self.plugins.leave();
        self.codex.leave();
        self.clear();
        self.reset_info();
        self.open = false;
        self.panel += 1;
        self.serial += 1;
        self.metadata = None;
        self.reading = None;
        self.error = None;
        if !self.pending() {
            self.save = SavePhase::Idle;
        }
    }
    pub fn disconnect(&mut self) {
        self.cloudflare.disconnect();
        self.plugins.disconnect();
        self.codex.disconnect();
        self.clear();
        self.reset_info();
        self.epoch = 0;
        self.reading = None;
        self.metadata = None;
        if let SavePhase::Pending(ticket) = self.save {
            self.uncertain_save = Some(ticket);
            self.save = SavePhase::Done(SaveResult::Indeterminate);
        } else if matches!(self.save, SavePhase::Confirm(_)) {
            self.save = SavePhase::Idle;
        }
        self.error = Some("The local backend is unavailable.");
    }
    fn can_save(&self, enabled: bool) -> bool {
        enabled
            && self.open
            && !self.pending()
            && self.uncertain_save.is_none()
            && !self.cloudflare.pending()
            && !self.plugins.pending()
            && !self.codex.pending()
            && self.reading.is_none()
            && self.metadata.is_some_and(|m| m.writable)
            && !self.edit.as_str().is_empty()
    }
    /// A clipboard paste is a draft edit: the same panel, epoch, draft-holder
    /// and pending fences as [`Action::Edit`], plus the review/save gates.
    pub fn paste_allowed(&self, ticket: Ticket, epoch: u64, enabled: bool) -> bool {
        enabled
            && self.open
            && self.epoch == epoch
            && self.page == Page::ApiLogin
            && ticket == self.input_ticket()
            && !self.pending()
            && self.uncertain_save.is_none()
            && !self.cloudflare.pending()
            && !self.plugins.pending()
            && !self.codex.pending()
            && !matches!(self.save, SavePhase::Confirm(_))
    }
    /// Apply one validated draft value, invalidating any stale review.
    fn apply_input(&mut self, value: Option<InputValue>, rejected: &'static str) {
        match value {
            Some(InputValue::Valid(editor)) => {
                self.edit = editor;
                self.next();
                if matches!(self.save, SavePhase::Confirm(_)) {
                    self.save = SavePhase::Idle;
                }
                self.error = None;
            }
            Some(InputValue::Rejected) => {
                self.clear();
                self.next();
                if matches!(self.save, SavePhase::Confirm(_)) {
                    self.save = SavePhase::Idle;
                }
                self.error = Some(rejected);
            }
            None => {}
        }
    }
    fn read(&mut self) -> Effect {
        let ticket = self.next();
        self.reading = Some(ticket);
        self.metadata = None;
        self.error = None;
        Effect::Read(ticket)
    }
    pub fn handle(&mut self, action: Action, epoch: u64, enabled: bool) -> Option<Effect> {
        if matches!(action, Action::Close) {
            self.close();
            return None;
        }
        if matches!(action, Action::Open) {
            self.close();
            self.open = true;
            self.epoch = epoch;
            if self.page == Page::Cloudflare {
                self.cloudflare.enter(epoch);
            }
            if matches!(self.page, Page::Plugins | Page::ApiLogin) {
                self.plugins.enter(epoch);
            }
            if self.page == Page::Codex {
                self.codex.enter(epoch);
            }
            if enabled
                && self.page == Page::ApiLogin
                && !self.cloudflare.pending()
                && !self.plugins.pending()
                && !self.codex.pending()
            {
                return Some(self.read());
            }
            return None;
        }
        if let Action::SelectPage(page) = action {
            if self.open && self.page != page {
                self.clear(); // Do not keep a hidden unsaved credential draft.
                self.reset_info();
                if matches!(self.save, SavePhase::Confirm(_)) {
                    self.save = SavePhase::Idle;
                    self.next(); // Previously rendered confirmation can no longer write.
                }
                if self.page == Page::Cloudflare {
                    self.cloudflare.leave();
                }
                if matches!(self.page, Page::Plugins | Page::ApiLogin) {
                    self.plugins.leave();
                }
                if self.page == Page::Codex {
                    self.codex.leave();
                }
                self.page = page;
                if self.page == Page::Cloudflare {
                    self.cloudflare.enter(epoch);
                }
                if self.page == Page::Codex {
                    self.codex.enter(epoch);
                }
                if matches!(self.page, Page::Plugins | Page::ApiLogin) {
                    self.plugins.enter(epoch);
                }
            }
            return None; // Navigation never reads, authenticates or writes.
        }
        if let Action::TogglePermissionDefault = action {
            self.permission_open = !self.permission_open;
        }
        if let Action::ToggleInfo { ticket, info } = action {
            // Local help is available even read-only. Never touches drafts, reviews or workers.
            if self.open
                && self.epoch == epoch
                && self.page == info.page()
                && ticket == self.info_ticket()
            {
                let expanded = match info {
                    Info::Details => &mut self.details_open,
                    Info::Shortcuts => &mut self.shortcuts_open,
                    Info::ApiHelp => &mut self.api_help_open,
                    Info::ContextPolicy => &mut self.context_policy_open,
                };
                *expanded = !*expanded;
            }
            return None;
        }
        if let Action::Account(action) = action {
            let enabled = self.account_enabled(enabled);
            return self.account.handle(action, enabled).map(Effect::Account);
        }
        if let Action::Cloudflare(action) = action {
            let enabled =
                self.cloudflare_enabled(enabled) && self.open && self.page == Page::Cloudflare;
            return self
                .cloudflare
                .handle(action, epoch, enabled)
                .map(Effect::Cloudflare);
        }
        if let Action::Codex(action) = action {
            if let crate::codex::Action::Usage { action, .. } = &action
                && !self.codex.usage_surface_action_allowed(action)
            {
                return None; // No editable placeholder or enabling save for an unwired feature.
            }
            let mut context = self.codex_context(enabled);
            context.epoch = epoch;
            return self.codex.handle(action, context).map(Effect::Codex);
        }
        if let Action::Plugins(action) = action {
            let mut context = self.plugin_context(enabled);
            context.epoch = epoch;
            return self.plugins.handle(action, context).map(Effect::Plugins);
        }
        if let Action::Edit { ticket, input } = &action {
            let value = input.take(); // stale/replayed holders also lose their secret immediately.
            if !enabled
                || self.epoch != epoch
                || !self.open
                || self.page != Page::ApiLogin
                || self.cloudflare.pending()
                || self.plugins.pending()
                || self.codex.pending()
                || *ticket != self.input_ticket()
                || self.pending()
                || self.uncertain_save.is_some()
            {
                drop(value);
                return None;
            }
            match value {
                Some(InputValue::Valid(editor)) => {
                    self.edit = editor;
                    self.next();
                    if matches!(self.save, SavePhase::Confirm(_)) {
                        self.save = SavePhase::Idle;
                    }
                    self.error = None;
                }
                Some(InputValue::Rejected) => {
                    self.clear();
                    self.next();
                    if matches!(self.save, SavePhase::Confirm(_)) {
                        self.save = SavePhase::Idle;
                    }
                    self.error = Some(
                        "API keys are printable ASCII. Draft cleared; enter or paste a valid key.",
                    );
                }
                None => {}
            }
            return None;
        }
        if let Action::Pasted { ticket, input } = &action {
            let value = input.take(); // stale/replayed holders also lose their secret immediately.
            if !self.paste_allowed(*ticket, epoch, enabled) {
                drop(value);
                return None;
            }
            self.apply_input(
                value,
                "Clipboard text is not a usable API key. Draft cleared; paste a single printable-ASCII key.",
            );
            return None;
        }
        if let Action::PasteUnavailable { ticket } = &action {
            if self.paste_allowed(*ticket, epoch, enabled) {
                self.error = Some("Nothing to paste, or the clipboard is unavailable.");
            }
            return None;
        }
        let ticket = match action {
            Action::Refresh(t)
            | Action::Review(t)
            | Action::CancelReview(t)
            | Action::Confirm(t) => t,
            _ => return None,
        };
        if !enabled
            || self.epoch != epoch
            || self.page != Page::ApiLogin
            || self.cloudflare.pending()
            || self.plugins.pending()
            || self.codex.pending()
            || !self.current(ticket)
        {
            return None;
        }
        match action {
            Action::Refresh(_) if !self.pending() => {
                if matches!(self.save, SavePhase::Confirm(_)) {
                    self.save = SavePhase::Idle;
                }
                Some(self.read())
            }
            Action::Review(_) if self.can_save(enabled) => {
                // Refuse to move a draft that cannot be a key into review, so the
                // message appears while the user can still correct the field.
                if let Some(message) = key_shape_error(self.edit.as_str()) {
                    self.error = Some(message);
                    return None;
                }
                self.save = SavePhase::Confirm(ticket);
                None
            }
            Action::CancelReview(_) if matches!(self.save,SavePhase::Confirm(t) if t==ticket) => {
                self.save = SavePhase::Idle;
                None
            }
            Action::Confirm(_)
                if self.can_save(enabled)
                    && matches!(self.save,SavePhase::Confirm(t) if t==ticket) =>
            {
                // Last gate before the key is taken out of the editor and sent: a
                // quoted key or an environment assignment must never be stored.
                // The draft is left in place here (unlike the other refusals) so
                // the user can delete the quotes rather than paste again.
                if let Some(message) = key_shape_error(self.edit.as_str()) {
                    self.save = SavePhase::Idle;
                    self.error = Some(message);
                    return None;
                }
                let raw = self.edit.take();
                let secret = match SecretApiKey::new(raw) {
                    Ok(secret) => secret,
                    Err(_) => {
                        self.clear();
                        self.save = SavePhase::Idle;
                        self.error = Some("Invalid key; draft cleared. Nothing was saved.");
                        return None;
                    }
                };
                self.clear();
                let ticket = self.next();
                self.reading = None;
                self.metadata = None;
                self.save = SavePhase::Pending(ticket);
                self.error = None;
                Some(Effect::Save { ticket, secret })
            }
            _ => None,
        }
    }
    pub fn metadata(&mut self, ticket: Ticket, result: Result<OnboardingMetadata, ()>) {
        if self.open
            && self.epoch == ticket.epoch
            && self.panel == ticket.panel
            && self.reading == Some(ticket)
        {
            self.reading = None;
            match result {
                Ok(metadata) => {
                    self.metadata = Some(metadata);
                    self.error = None;
                }
                Err(()) => {
                    // The Models page renders its own short status plus Retry;
                    // no duplicate error line.
                    self.metadata = None;
                    self.error = None;
                }
            }
        }
    }
    pub fn saved(&mut self, ticket: Ticket, result: SaveResult) {
        if !matches!(self.save,SavePhase::Pending(t) if t==ticket) {
            return;
        }
        if result == SaveResult::Indeterminate {
            self.uncertain_save = Some(ticket);
        }
        if self.epoch == ticket.epoch && self.open && self.panel == ticket.panel {
            self.save = SavePhase::Done(result);
        } else {
            self.save = SavePhase::EndedElsewhere;
        } // never confirm a different panel/draft.
    }
    /// The settings view at its default surface size. The app always renders
    /// [`Self::view_sized`] with the live viewport, so this is the tests' entry.
    #[cfg(test)]
    pub fn view(&self, enabled: bool) -> Element<'_, Action> {
        self.view_sized(enabled, 960.0, 700.0, &RouteFacts::default())
    }
    pub fn view_sized(
        &self,
        enabled: bool,
        width: f32,
        height: f32,
        route: &RouteFacts,
    ) -> Element<'_, Action> {
        let navigation = Page::ALL
            .into_iter()
            .fold(column![].spacing(6), |nav, page| {
                nav.push(
                    button(
                        row![
                            label(page.glyph(), MUTED).size(16).width(22),
                            label(page.title(), TEXT).size(13)
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([11, 12])
                    .width(Length::Fill)
                    .style(move |theme, status| {
                        let mut style = design::navigation(theme, status, self.page == page);
                        style.text_color = TEXT;
                        style
                    })
                    .on_press(Action::SelectPage(page)),
                )
            });
        let body: Element<'_, Action> = match self.page {
            Page::ApiLogin => {
                self.api_view(
                    enabled && !self.plugins.pending() && !self.codex.pending() && !self.cloudflare.pending(),
                    route,
                )
            }
            Page::Cloudflare => self.cloudflare.view(self.cloudflare_enabled(enabled)).map(Action::Cloudflare),
            Page::General => self.general_view(route),
            Page::Codex => self
                .codex
                .view(self.codex_context(enabled))
                .map(Action::Codex),
            Page::Plugins => self
                .plugins
                .view(self.plugin_context(enabled))
                .map(Action::Plugins),
            Page::AgentPresets => column![
                label("Agent presets", TEXT).size(17),
                label("Presets define the tools and behavior of an agent. They are configured by the Harness profile; opening this section does not activate an agent or change a conversation.", MUTED).size(13),
            ].spacing(14).into(),
        };
        // Reserve a gutter so the overlaid scrollbar cannot cover privacy/confirmation text.
        let body = container(body)
            .padding(iced::Padding {
                right: 14.0,
                ..Default::default()
            })
            .width(Length::Fill);
        let content = container(
            scrollable(body)
                .id("native-settings-body")
                .height(Length::Fill),
        )
        .padding(iced::Padding {
            top: 12.0,
            right: 18.0,
            bottom: 16.0,
            left: 22.0,
        })
        .width(Length::Fill)
        .height(Length::Fill);
        let sidebar = container(navigation)
            .padding(8)
            .width(176)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(SURFACE.into()),
                ..Default::default()
            });
        let sections: Element<'_, Action> = if width >= 650.0 {
            row![sidebar, content].height(Length::Fill).into()
        } else {
            // Keep category access usable when the compositor supplies a narrow window.
            let tabs = Page::ALL.into_iter().fold(row![].spacing(4), |tabs, page| {
                tabs.push(
                    button(page.title())
                        .padding([8, 6])
                        .width(Length::Fill)
                        .style(move |theme, status| {
                            design::navigation(theme, status, self.page == page)
                        })
                        .on_press(Action::SelectPage(page)),
                )
            });
            column![container(tabs).padding([8, 12]), content]
                .height(Length::Fill)
                .into()
        };
        // The footer carries only live status; routine hints stay out of the way.
        let mut footer = column![].spacing(4);
        let mut has_footer = false;
        if self.pending() {
            footer = footer
                .push(label("API-key save submitted; closing does not undo it.", MUTED).size(11));
            has_footer = true;
        } else if self.plugins.pending() {
            footer = footer
                .push(label("Plugin setting submitted; closing does not undo it.", MUTED).size(11));
            has_footer = true;
        } else if self.codex.pending() {
            footer = footer.push(label("Codex operation pending.", MUTED).size(11));
            has_footer = true;
        }
        if let Some((status, attention)) = self.cloudflare.footer() {
            footer = footer.push(label(status, if attention { DANGER } else { MUTED }).size(11));
            has_footer = true;
        }
        if let Some((status, attention)) = self.codex.runtime_footer() {
            footer = footer.push(label(status, if attention { DANGER } else { MUTED }).size(11));
            has_footer = true;
        }
        if let Some((status, attention)) = self.codex.usage_footer() {
            footer = footer.push(label(status, if attention { DANGER } else { MUTED }).size(11));
            has_footer = true;
        }
        let mut shell = column![
            container(
                row![
                    label("Settings", TEXT).size(16).width(Length::Fill),
                    button(label("×", MUTED).size(20))
                        .padding([4, 10])
                        .style(design::ghost)
                        .on_press(Action::Close)
                ]
                .align_y(iced::Alignment::Center)
            )
            .padding([12, 16]),
            sections,
        ];
        if has_footer {
            shell = shell.push(container(footer).padding([10, 16]));
        }
        container(shell)
            .width(width.max(1.0))
            .height(height.max(1.0))
            .style(|_| design::card(SURFACE, 20.0))
            .clip(true)
            .into()
    }
    fn general_view(&self, route: &RouteFacts) -> Element<'_, Action> {
        let mut body = column![
            label("General", TEXT).size(17),
            label(
                "Native Iced interface over the local Harness backend, in the fixed dark Rosé Pine theme.",
                MUTED,
            ),
            self.info_button(Info::Details, "Details", self.details_open),
        ]
        .spacing(14);
        if self.details_open {
            body = body.push(label(
                "No embedded browser or webview, and no editable appearance preferences.",
                MUTED,
            ));
        }
        body = body.push(self.info_button(Info::Shortcuts, "Shortcuts", self.shortcuts_open));
        if self.shortcuts_open {
            body = body.push(label("Ctrl+B · navigation\nCtrl+F · find in conversation\nEnter · send non-empty text (Ctrl+Enter also works)\nShift+Enter · new line\nEscape · close settings and panels", TEXT));
        }
        // Why the harness is configured this way, from the harness-design study;
        // live numbers stay in the composer, which reads the Host's own report.
        // The reference's permission default row: the new-session preset, absent entirely
        // when the deployment exposes no permission settings (`PermissionRow.tsx:52-58`).
        if let Some(permission) = &route.permission {
            let usable = permission.writable && !permission.saving;
            body = body
                .push(label("Permission for new sessions", TEXT).size(13))
                .push(
                    button(
                        label(
                            permission_display(&permission.current, &permission.options),
                            if usable { TEXT } else { MUTED },
                        )
                        .size(13),
                    )
                    .style(design::ghost)
                    .padding([4, 8])
                    .on_press_maybe(usable.then_some(Action::TogglePermissionDefault)),
                );
            if self.permission_open && usable {
                for (value, name) in &permission.options {
                    let ink = if *value == permission.current {
                        TEXT
                    } else {
                        MUTED
                    };
                    body = body.push(
                        button(label(name.clone(), ink).size(13))
                            .style(design::ghost)
                            .padding([4, 8])
                            .width(Length::Fill)
                            .on_press(Action::PermissionDefault(value.clone())),
                    );
                }
            }
        }
        body = body.push(self.info_button(
            Info::ContextPolicy,
            "Context & tool policy",
            self.context_policy_open,
        ));
        if self.context_policy_open {
            body = body
                .push(label("Context: tool output over 8k characters is replaced by a stub, then older history is summarized near 80% of the window. Eliding before summarizing is what keeps peak context low. A window that overflows ends the run early, so the composer states it instead of only tinting it.", MUTED))
                .push(label("Tools: the predefined file, search and shell tools are always offered. Bash-only interfaces reduce cost for models with strong shell control but lower success for weaker ones, so this app does not offer one.", MUTED))
                .push(label("Loops: the guard asks the model to change approach after 3, 5 and 8 identical tool calls, and Trajectory names each reminder it injects. Planning is shown as a plan chip beside the composer.", MUTED));
        }
        body.into()
    }
    fn api_view(&self, enabled: bool, route: &RouteFacts) -> Element<'_, Action> {
        let ticket = self.ticket();
        let input_ticket = self.input_ticket();
        let can_save = self.can_save(enabled);
        let editable = self.paste_allowed(input_ticket, self.epoch, enabled);
        let account = self.account.view(can_save).map(Action::Account);
        let mut body = column![
            account,
            label("DeepSeek", TEXT).size(17),
            label("API key", ACCENT).size(16),
            label(
                "Paste the DeepSeek API key the local Harness backend should use. Nothing is saved until you review and confirm.",
                MUTED,
            )
        ]
        .spacing(14);
        if self.uncertain_save.is_some() {
            body = body.push(label(
                "A previous API-key save is unresolved, so saving stays disabled until the app restarts.",
                DANGER,
            ));
        }
        let input = text_input("Paste or type your API key", self.edit.as_str())
            .padding(14)
            .size(15)
            .style(design::field)
            .secure(true)
            .id("native-api-key-draft");
        let input = if editable {
            input
                .on_input(move |raw| Action::Edit {
                    ticket: input_ticket,
                    input: key_input(raw),
                })
                // Iced's own paste reads the clipboard itself; on Wayland that
                // returns nothing and this callback only sees the unchanged
                // field. Ask the App to read through the working reader instead.
                .on_paste(move |_| Action::Paste {
                    ticket: input_ticket,
                })
        } else {
            input
        };
        let paste = button(label("Paste", TEXT).size(13))
            .padding([12, 18])
            .style(design::ghost)
            .on_press_maybe(editable.then_some(Action::Paste {
                ticket: input_ticket,
            }));
        body = body.push(
            row![input.width(Length::Fill), paste]
                .spacing(8)
                .align_y(iced::Alignment::Center),
        );
        // One quiet status line; raw backend metadata is never rendered.
        if self.reading.is_some() {
            body = body.push(label("Checking the local backend…", MUTED).size(12));
        } else if self.metadata.is_none() {
            body = body.push(
                row![
                    label(
                        "Couldn't check the local backend, so saving is disabled.",
                        MUTED
                    )
                    .size(12)
                    .width(Length::Fill),
                    button(label("Retry", MUTED).size(12))
                        .padding([5, 9])
                        .style(design::ghost)
                        .on_press_maybe(
                            (enabled && !self.pending()).then_some(Action::Refresh(ticket))
                        )
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
        } else if self.metadata.is_some_and(|meta| !meta.writable) {
            body = body.push(label(
                "This backend is read-only, so saving is disabled.",
                DANGER,
            ));
        }
        // Models the backend actually reports for the DeepSeek route. Read-only:
        // the composer owns the active model, reasoning and speed selection.
        body = body.push(label("Models", ACCENT).size(16));
        if route.models.is_empty() {
            body = body.push(label(
                if route.loading {
                    "Loading the DeepSeek model catalog…"
                } else if route.failed {
                    "The backend did not report a DeepSeek model catalog. Open the model picker in the composer to retry."
                } else {
                    "No DeepSeek models reported yet."
                },
                MUTED,
            ));
        } else {
            let mut rows = column![].spacing(4);
            for (name, id, active) in &route.models {
                rows = rows.push(
                    row![
                        label(if *active { "✓" } else { "" }, ACCENT)
                            .size(12)
                            .width(14),
                        label(name.clone(), if *active { TEXT } else { MUTED }).size(13),
                        label(id.clone(), MUTED).size(11).width(Length::Fill),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                );
            }
            body = body.push(rows);
            body = body.push(label(
                "Choose the model, reasoning and speed in the composer before sending.",
                MUTED,
            ));
        }
        // Provider options come from the backend's own `llm-deepseek` namespace,
        // rendered through the shared live-settings editor. Nothing is shown
        // until a read actually returns it.
        let context = self.plugin_context(enabled);
        body = body.push(
            row![
                label("Provider options", ACCENT)
                    .size(16)
                    .width(Length::Fill),
                button(
                    label(
                        if self.plugins.has_snapshot() {
                            "Refresh"
                        } else {
                            "Load"
                        },
                        MUTED,
                    )
                    .size(12)
                )
                .padding([5, 9])
                .style(design::ghost)
                .on_press_maybe(
                    (context.enabled && context.visible && !self.plugins.pending()).then_some(
                        Action::Plugins(crate::plugins::Action::Refresh(
                            self.plugins.read_ticket()
                        ))
                    ),
                ),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        );
        match self.plugins.view_namespace(context, "llm-deepseek") {
            Some(editor) => body = body.push(editor.map(Action::Plugins)),
            None => {
                body = body.push(label(
                    "Load the DeepSeek endpoint, model catalog and reasoning settings from the local backend.",
                    MUTED,
                ))
            }
        }
        match self.save {
            SavePhase::Confirm(t) => {
                body = body
                    .push(label(
                        "Confirm saving this key to the local backend? It replaces any stored key and does not validate it or sign in.",
                        DANGER,
                    ))
                    .push(
                        column![
                            button("Save key")
                                .padding([11, 18])
                                .style(design::primary)
                                .on_press_maybe(can_save.then_some(Action::Confirm(t))),
                            button("Cancel")
                                .padding([10, 14])
                                .style(design::ghost)
                                .on_press(Action::CancelReview(t))
                        ]
                        .spacing(10),
                    );
            }
            SavePhase::Pending(_) => {
                body = body.push(label(
                    "Save submitted; waiting for the backend to confirm. Closing does not undo it.",
                    MUTED,
                ));
            }
            SavePhase::Done(result) => {
                body = body
                    .push(label(
                        result.text(),
                        if result == SaveResult::Confirmed {
                            ACCENT
                        } else {
                            DANGER
                        },
                    ))
                    .push(
                        button("Save again")
                            .padding([11, 18])
                            .style(design::primary)
                            .on_press_maybe(can_save.then_some(Action::Review(ticket))),
                    );
            }
            SavePhase::EndedElsewhere => {
                body = body.push(label(
                    "An earlier save returned after this panel closed; its outcome is unknown. Check the conversation, then save again if needed.",
                    DANGER,
                ))
                .push(
                    button("Save again")
                        .padding([11, 18])
                        .style(design::primary)
                        .on_press_maybe(can_save.then_some(Action::Review(ticket))),
                );
            }
            SavePhase::Idle => {
                body = body.push(
                    button("Review and save")
                        .padding([11, 18])
                        .style(design::primary)
                        .on_press_maybe(can_save.then_some(Action::Review(ticket))),
                );
            }
        }
        if !enabled {
            body = body.push(label(
                "Read-only: the local backend is unavailable.",
                DANGER,
            ));
        }
        if let Some(error) = self.error {
            body = body.push(label(error, DANGER));
        }
        body = body.push(self.info_button(Info::ApiHelp, "Help & privacy", self.api_help_open));
        if self.api_help_open {
            body = body
                .push(label("The key is written only to the local Harness backend. No browser sign-in, provider catalog, account probe or model validation.", MUTED))
                .push(label("The draft is cleared on save or close. Masking is not memory protection: this app wipes its owned draft, but clipboard, editor/render, allocator, kernel, Node and swap copies cannot be guaranteed wiped.", MUTED));
        }
        body.into()
    }
}

#[cfg(test)]
mod disclosure_tests {
    fn local_usage(
        state: &mut Settings,
        action: crate::codex::UsageAction,
        enabled: bool,
    ) -> Option<Effect> {
        state.handle(
            Action::Codex(crate::codex::Action::Usage {
                ticket: state.codex_ticket(),
                action,
            }),
            1,
            enabled,
        )
    }
    fn usage_state(preferences: dsh_native_core::CodexUsagePreferences) -> Settings {
        let mut state = Settings::default();
        state.handle(Action::Open, 1, false);
        state.handle(Action::SelectPage(Page::Codex), 1, true);
        let Some(Effect::Codex(crate::codex::Effect::UsageRead { ticket })) =
            local_usage(&mut state, crate::codex::UsageAction::Refresh, true)
        else {
            panic!("PUBLIC read")
        };
        state.codex_completed(
            ticket,
            crate::codex::Outcome::UsageSnapshot(dsh_native_core::CodexUsageSnapshot {
                version: 1,
                revision: 9,
                persisted: true,
                preferences,
            }),
        );
        state
    }
    #[test]
    fn native_usage_surface_rejects_unsupported_edits_and_unchanged_saves() {
        use crate::codex::UsageAction as U;
        use dsh_native_core::{CodexComposerDisplay as C, CodexUsageAlert as A};
        let mut state = usage_state(dsh_native_core::CodexUsagePreferences::default());
        for action in [
            U::Composer(C::Percent),
            U::Composer(C::ProgressBar),
            U::Composer(C::Runway),
            U::Alert(A::Percent20),
            U::Alert(A::EarlyWarning),
            U::Alert(A::Custom),
            U::Threshold("17.5".into()),
            U::Retry(true),
            U::Consent(true),
        ] {
            assert!(!state.codex.usage_surface_action_allowed(&action));
            assert!(local_usage(&mut state, action, true).is_none());
            assert!(local_usage(&mut state, U::Save, true).is_none());
            assert!(state.codex.usage_footer().is_none());
        }
        for action in [
            U::Composer(C::Off),
            U::Alert(A::Off),
            U::Retry(false),
            U::Consent(false),
        ] {
            assert!(local_usage(&mut state, action, true).is_none());
        }
        assert!(local_usage(&mut state, U::Save, true).is_none()); // No-op writes are not a feature.
        assert!(!state.codex.effective_consent());
        let _: Element<'_, Action> = state.view(true);
    }
    #[test]
    fn native_usage_optout_is_functional_captured_and_requires_matching_ack() {
        use crate::codex::UsageAction as U;
        use dsh_native_core::{CodexComposerDisplay as C, CodexUsageAlert as A};
        let original = dsh_native_core::CodexUsagePreferences {
            composer_display: C::ProgressBar,
            usage_alert: A::Custom,
            retry_after_reset: true,
            experimental_quota: true,
            ..dsh_native_core::CodexUsagePreferences::default()
        };
        let mut state = usage_state(original);
        assert!(local_usage(&mut state, U::Save, true).is_none());
        assert!(local_usage(&mut state, U::Consent(false), false).is_none()); // Immediate even read-only.
        assert!(!state.codex.effective_consent());
        for action in [U::Composer(C::Off), U::Alert(A::Off), U::Retry(false)] {
            assert!(local_usage(&mut state, action, true).is_none());
        }
        let Some(Effect::Codex(crate::codex::Effect::UsageSave {
            ticket,
            expected_revision,
            preferences,
        })) = local_usage(&mut state, U::Save, true)
        else {
            panic!("explicit opt-out save")
        };
        assert_eq!(expected_revision, 9);
        assert_eq!(preferences.composer_display, C::Off);
        assert_eq!(preferences.usage_alert, A::Off);
        assert!(!preferences.retry_after_reset && !preferences.experimental_quota);
        assert_eq!(
            preferences.custom_remaining_percent,
            original.custom_remaining_percent
        );
        assert!(
            state
                .codex
                .usage_footer()
                .unwrap()
                .0
                .contains("not yet acknowledged")
        );
        assert!(local_usage(&mut state, U::Save, true).is_none());
        state.codex_completed(
            ticket,
            crate::codex::Outcome::UsageSnapshot(dsh_native_core::CodexUsageSnapshot {
                version: 1,
                revision: 10,
                persisted: true,
                preferences,
            }),
        );
        assert!(state.codex.usage_footer().is_none());
        assert!(local_usage(&mut state, U::Save, true).is_none());
        assert!(!state.codex.effective_consent());
    }
    #[test]
    fn usage_warning_and_pending_footer_survive_every_settings_category_without_effects() {
        let mut state = Settings::default();
        state.handle(Action::Open, 1, true);
        state.handle(Action::SelectPage(Page::Codex), 1, true);
        let request = state.handle(
            Action::Codex(crate::codex::Action::Usage {
                ticket: state.codex_ticket(),
                action: crate::codex::UsageAction::Refresh,
            }),
            1,
            true,
        );
        let Some(Effect::Codex(crate::codex::Effect::UsageRead { ticket })) = request else {
            panic!("PUBLIC read, not dispatched")
        };
        let mut snapshot = dsh_native_core::CodexUsageSnapshot::default();
        snapshot.persisted = true;
        snapshot.revision = 4;
        snapshot.preferences.retry_after_reset = true; // Prior saved choice, never an active executor.
        state.codex_completed(ticket, crate::codex::Outcome::UsageSnapshot(snapshot));
        assert!(
            state
                .handle(
                    Action::Codex(crate::codex::Action::Usage {
                        ticket: state.codex_ticket(),
                        action: crate::codex::UsageAction::Retry(false),
                    }),
                    1,
                    true
                )
                .is_none()
        );
        let request = state.handle(
            Action::Codex(crate::codex::Action::Usage {
                ticket: state.codex_ticket(),
                action: crate::codex::UsageAction::Save,
            }),
            1,
            true,
        );
        let Some(Effect::Codex(crate::codex::Effect::UsageSave { ticket, .. })) = request else {
            panic!("PUBLIC save, not dispatched")
        };
        let pending = state.codex.usage_footer().unwrap();
        assert!(pending.0.contains("not yet acknowledged") && !pending.1);
        for page in Page::ALL {
            assert!(state.handle(Action::SelectPage(page), 1, true).is_none());
            assert_eq!(state.codex.usage_footer(), Some(pending));
            let _: Element<'_, Action> =
                state.view_sized(true, 960.0, 700.0, &RouteFacts::default());
        }
        state.codex_completed(ticket, crate::codex::Outcome::Indeterminate);
        let warning = state.codex.usage_footer().unwrap();
        assert!(warning.0.contains("unknown") && warning.1);
        for page in Page::ALL {
            assert!(state.handle(Action::SelectPage(page), 1, false).is_none());
            assert_eq!(state.codex.usage_footer(), Some(warning));
            let _: Element<'_, Action> =
                state.view_sized(false, 960.0, 700.0, &RouteFacts::default());
        }
        state.close();
        state.handle(Action::Open, 1, false);
        assert_eq!(state.codex.usage_footer(), Some(warning));
    }

    use super::*;

    fn toggle(state: &mut Settings, info: Info, epoch: u64) {
        let ticket = state.info_ticket();
        assert!(
            state
                .handle(Action::ToggleInfo { ticket, info }, epoch, false)
                .is_none()
        );
    }

    #[test]
    fn api_help_is_queueless_and_preserves_the_draft_and_save_review() {
        let mut state = Settings::default();
        let Some(Effect::Read(read)) = state.handle(Action::Open, 1, true) else {
            panic!("expected existing metadata read");
        };
        state.metadata(
            read,
            Ok(OnboardingMetadata {
                logged_in: false,
                has_api_key: false,
                writable: true,
            }),
        );
        state.handle(
            Action::Edit {
                ticket: state.input_ticket(),
                input: key_input("PUBLIC_FAKE_KEY".into()),
            },
            1,
            true,
        );
        let operation = state.ticket();
        let input = state.input_ticket();
        assert!(state.handle(Action::Review(operation), 1, true).is_none());
        for expanded in [true, false] {
            toggle(&mut state, Info::ApiHelp, 1);
            assert_eq!(state.api_help_open, expanded);
            assert_eq!(state.ticket(), operation);
            assert_eq!(state.input_ticket(), input);
            assert!(matches!(state.save, SavePhase::Confirm(t) if t == operation));
            assert_eq!(state.edit.as_str(), "PUBLIC_FAKE_KEY");
            assert!(state.reading.is_none());
            assert!(state.can_save(true));
        }
    }

    #[test]
    fn static_general_help_is_read_only_independent_and_epoch_fenced() {
        let mut state = Settings::default();
        assert!(state.handle(Action::Open, 0, false).is_none());
        assert!(
            state
                .handle(Action::SelectPage(Page::General), 0, false)
                .is_none()
        );
        let operation = state.ticket();
        toggle(&mut state, Info::Details, 0);
        toggle(&mut state, Info::Shortcuts, 0);
        assert!(state.details_open && state.shortcuts_open);
        assert_eq!(state.ticket(), operation);
        assert!(state.metadata.is_none() && state.reading.is_none());
        toggle(&mut state, Info::Details, 1);
        assert!(state.details_open); // wrong runtime context cannot change the disclosure.
        toggle(&mut state, Info::ApiHelp, 0);
        assert!(!state.api_help_open); // hidden category cannot expand.
    }

    #[test]
    fn help_resets_and_stale_events_cannot_reopen_it() {
        let mut state = Settings::default();
        state.handle(Action::Open, 1, false);
        let old = state.info_ticket();
        toggle(&mut state, Info::ApiHelp, 1);
        assert!(state.api_help_open);
        state.handle(Action::SelectPage(Page::General), 1, false);
        assert!(!state.api_help_open);
        assert!(
            state
                .handle(
                    Action::ToggleInfo {
                        ticket: old,
                        info: Info::ApiHelp
                    },
                    1,
                    false
                )
                .is_none()
        );
        state.handle(Action::SelectPage(Page::ApiLogin), 1, false);
        assert!(
            state
                .handle(
                    Action::ToggleInfo {
                        ticket: old,
                        info: Info::ApiHelp
                    },
                    1,
                    false
                )
                .is_none()
        );
        assert!(!state.api_help_open); // leaving and returning also invalidates the old event.
        toggle(&mut state, Info::ApiHelp, 1);
        let before_epoch = state.info_ticket();
        state.adopt_epoch(2);
        assert!(!state.api_help_open);
        assert!(
            state
                .handle(
                    Action::ToggleInfo {
                        ticket: before_epoch,
                        info: Info::ApiHelp
                    },
                    2,
                    false
                )
                .is_none()
        );
        toggle(&mut state, Info::ApiHelp, 2);
        state.disconnect();
        assert!(!state.api_help_open);
        toggle(&mut state, Info::ApiHelp, 0);
        let before_close = state.info_ticket();
        state.close();
        assert!(!state.api_help_open);
        assert!(
            state
                .handle(
                    Action::ToggleInfo {
                        ticket: before_close,
                        info: Info::ApiHelp
                    },
                    0,
                    false
                )
                .is_none()
        );
        state.handle(Action::Open, 0, false);
        assert!(
            state
                .handle(
                    Action::ToggleInfo {
                        ticket: before_close,
                        info: Info::ApiHelp
                    },
                    0,
                    false
                )
                .is_none()
        );
        assert!(!state.api_help_open);
        assert!(state.reading.is_none() && state.metadata.is_none());
    }

    /// The context/tool-policy explanation is local help on the General page: it
    /// expands only for its own ticket while that page is open, and produces no
    /// effect, so it can never be mistaken for a policy change.
    #[test]
    fn context_policy_disclosure_is_local_help_on_the_general_page() {
        let mut state = Settings::default();
        assert!(state.handle(Action::Open, 1, true).is_some());
        let page = Info::ContextPolicy.page();
        assert_eq!(page, Page::General);
        state.handle(Action::SelectPage(Page::General), 1, true);
        let ticket = state.info_ticket();
        assert!(
            state
                .handle(
                    Action::ToggleInfo {
                        ticket,
                        info: Info::ContextPolicy,
                    },
                    1,
                    true,
                )
                .is_none()
        );
        assert!(state.context_policy_open);
        assert!(!state.api_help_open);
        // A stale ticket or the wrong page leaves it alone.
        let stale = Settings::default().info_ticket();
        assert!(
            state
                .handle(
                    Action::ToggleInfo {
                        ticket: stale,
                        info: Info::ContextPolicy,
                    },
                    1,
                    true,
                )
                .is_none()
        );
        assert!(state.context_policy_open);
        state.handle(Action::SelectPage(Page::ApiLogin), 1, true);
        assert!(!state.context_policy_open);
        // The page still renders with the disclosure expanded.
        state.handle(Action::SelectPage(Page::General), 1, true);
        state.handle(
            Action::ToggleInfo {
                ticket: state.info_ticket(),
                info: Info::ContextPolicy,
            },
            1,
            true,
        );
        assert!(state.context_policy_open);
        let _: Element<'_, Action> = state.view(true);
    }
}
