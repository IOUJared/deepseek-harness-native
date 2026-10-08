//! Explicit native key persistence. Secret edits are opaque, one-use and never serialized.
use crate::design;
use crate::ui::{ACCENT, DANGER, MUTED, SURFACE, TEXT, label};
use dsh_native_core::{OnboardingMetadata, SecretApiKey};
use iced::widget::{button, column, container, row, scrollable, text_input};
use iced::{Element, Length};
use std::sync::{
    Arc, Mutex,
    atomic::{Ordering, compiler_fence},
};
const MAX_DRAFT: usize = 16_384;
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
            Self::Confirmed => {
                "Host confirmed API-key persistence. No account sign-in or model validation performed."
            }
            Self::Refused => "Host refused persistence. No confirmed save; no automatic retry.",
            Self::Indeterminate => {
                "Save outcome indeterminate: the Host may already have persisted the key. No automatic retry; metadata cannot confirm this attempt."
            }
            Self::NotSent => {
                "Save was not sent: the worker, queue, permission metadata or lifecycle did not permit it. Draft cleared; re-enter only explicitly."
            }
        }
    }
}
// The native editor needs a local plaintext value; it is never an Event/String DTO.
struct Editor(String);
impl Default for Editor {
    fn default() -> Self {
        Self(String::new())
    }
}
impl Editor {
    fn wipe(&mut self) {
        // SAFETY: String owns its complete u8 allocation. Write only, including spare capacity;
        // zero bytes preserve UTF-8 and the allocation remains live until String drops.
        unsafe {
            let bytes = self.0.as_mut_vec();
            for offset in 0..bytes.capacity() {
                bytes.as_mut_ptr().add(offset).write_volatile(0);
            }
        }
        compiler_fence(Ordering::SeqCst);
    }
}
impl Drop for Editor {
    fn drop(&mut self) {
        self.wipe();
    }
}
impl std::fmt::Debug for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretEditor([REDACTED])")
    }
}
struct Edit {
    editor: Editor,
}
enum InputValue {
    Valid(Edit),
    Rejected,
}
/// Clone duplicates only a one-use holder, never plaintext or SecretApiKey.
#[derive(Clone)]
pub struct Input(Arc<Mutex<Option<InputValue>>>);
impl Input {
    pub fn new(raw: String) -> Self {
        let value = if raw.len() > MAX_DRAFT || !raw.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
            drop(Editor(raw));
            InputValue::Rejected
        } else {
            InputValue::Valid(Edit {
                editor: Editor(raw),
            })
        };
        Self(Arc::new(Mutex::new(Some(value))))
    }
    fn take(&self) -> Option<InputValue> {
        self.0.lock().ok()?.take()
    }
}
impl std::fmt::Debug for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretInput([REDACTED])")
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    General,
    #[default]
    ApiLogin,
    Codex,
    Plugins,
}
impl Page {
    const ALL: [Self; 4] = [Self::General, Self::ApiLogin, Self::Codex, Self::Plugins];
    fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::ApiLogin => "API login",
            Self::Codex => "Codex",
            Self::Plugins => "Plugins",
        }
    }
}
/// Static help only; these disclosures never read or change backend settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Info {
    Details,
    Shortcuts,
    ApiHelp,
}
impl Info {
    fn page(self) -> Page {
        match self {
            Self::Details | Self::Shortcuts => Page::General,
            Self::ApiHelp => Page::ApiLogin,
        }
    }
}
#[derive(Clone)]
pub enum Action {
    Open,
    Close,
    SelectPage(Page),
    ToggleInfo { ticket: Ticket, info: Info },
    Plugins(crate::plugins::Action),
    Codex(crate::codex::Action),
    Refresh(Ticket),
    Edit { ticket: Ticket, input: Input },
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
    api_help_open: bool,
    plugins: crate::plugins::Controller,
    codex: crate::codex::Controller,
    epoch: u64,
    panel: u64,
    serial: u64,
    editor_generation: u64,
    edit: Edit,
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
            api_help_open: false,
            plugins: crate::plugins::Controller::default(),
            codex: crate::codex::Controller::default(),
            epoch: 0,
            panel: 0,
            serial: 0,
            editor_generation: 0,
            edit: Edit {
                editor: Editor::default(),
            },
            metadata: None,
            reading: None,
            save: SavePhase::Idle,
            uncertain_save: None,
            error: None,
        }
    }
}
impl Settings {
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
        .style(design::button_style)
        .on_press(Action::ToggleInfo {
            ticket: self.info_ticket(),
            info,
        })
        .into()
    }
    fn plugin_context(&self, enabled: bool) -> crate::plugins::Context {
        crate::plugins::Context {
            epoch: self.epoch,
            visible: self.open && self.page == Page::Plugins,
            enabled: enabled && !self.pending() && !self.codex.pending(),
        }
    }
    fn codex_context(&self, enabled: bool) -> crate::codex::Context {
        crate::codex::Context {
            epoch: self.epoch,
            visible: self.open && self.page == Page::Codex,
            enabled: enabled && !self.pending() && !self.plugins.pending(),
        }
    }
    pub(crate) fn codex_completed(
        &mut self,
        ticket: crate::codex::Ticket,
        outcome: crate::codex::Outcome,
    ) {
        self.codex.completed(ticket, outcome);
    }
    pub(crate) fn codex_observed(&mut self, metadata: dsh_native_core::CodexMetadata) {
        self.codex.observed(metadata);
    }
    /// Continue only a connection explicitly started in this visible, enabled panel.
    pub(crate) fn codex_continue(&mut self, enabled: bool) -> Option<crate::codex::Effect> {
        let context = self.codex_context(enabled);
        self.codex.continue_connection(context)
    }
    #[allow(dead_code)]
    pub(crate) fn codex_ticket(&self) -> crate::codex::Ticket {
        self.codex.ticket()
    }
    #[allow(dead_code)]
    pub(crate) fn codex_draft_ticket(&self) -> Option<crate::codex::DraftTicket> {
        self.codex.draft_ticket()
    }
    /// Nonsecret view tickets; every action remains subject to panel and backend guards.
    #[allow(dead_code)]
    pub(crate) fn plugin_read_ticket(&self) -> crate::plugins::ReadTicket {
        self.plugins.read_ticket()
    }
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
        self.edit = Edit {
            editor: Editor::default(),
        };
    }
    pub fn close(&mut self) {
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
        self.error = Some("Owned backend unavailable; settings are read-only.");
    }
    fn can_save(&self, enabled: bool) -> bool {
        enabled
            && self.open
            && !self.pending()
            && self.uncertain_save.is_none()
            && !self.plugins.pending()
            && !self.codex.pending()
            && self.reading.is_none()
            && self.metadata.is_some_and(|m| m.writable)
            && !self.edit.editor.0.is_empty()
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
            if self.page == Page::Plugins {
                self.plugins.enter(epoch);
            }
            if self.page == Page::Codex {
                self.codex.enter(epoch);
            }
            if enabled
                && self.page == Page::ApiLogin
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
                if self.page == Page::Plugins {
                    self.plugins.leave();
                }
                if self.page == Page::Codex {
                    self.codex.leave();
                }
                self.page = page;
                if self.page == Page::Codex {
                    self.codex.enter(epoch);
                }
                if self.page == Page::Plugins {
                    self.plugins.enter(epoch);
                }
            }
            return None; // Navigation never reads, authenticates or writes.
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
                };
                *expanded = !*expanded;
            }
            return None;
        }
        if let Action::Codex(action) = action {
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
                Some(InputValue::Valid(edit)) => {
                    self.edit = edit;
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
                        "Use at most 16384 printable, non-whitespace ASCII bytes. Draft cleared; enter a new valid key.",
                    );
                }
                None => {}
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
                let raw = std::mem::take(&mut self.edit.editor.0);
                let secret = match SecretApiKey::new(raw) {
                    Ok(secret) => secret,
                    Err(_) => {
                        self.clear();
                        self.save = SavePhase::Idle;
                        self.error = Some("Invalid key input; draft cleared, no write attempted.");
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
                    self.metadata = None;
                    self.error =
                        Some("Metadata unavailable; Save disabled. Refresh only explicitly.");
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
    /// Compatibility entrypoint for embedders; the app uses the viewport-fitted variant.
    #[allow(dead_code)]
    pub fn view(&self, enabled: bool) -> Element<'_, Action> {
        self.view_sized(enabled, 960.0, 700.0)
    }
    pub fn view_sized(&self, enabled: bool, width: f32, height: f32) -> Element<'_, Action> {
        let navigation = Page::ALL
            .into_iter()
            .fold(column![].spacing(6), |nav, page| {
                nav.push(
                    button(page.title())
                        .padding([12, 14])
                        .width(Length::Fill)
                        .style(move |theme, status| {
                            design::navigation(theme, status, self.page == page)
                        })
                        .on_press(Action::SelectPage(page)),
                )
            });
        let body: Element<'_, Action> = match self.page {
            Page::ApiLogin => {
                self.api_view(enabled && !self.plugins.pending() && !self.codex.pending())
            }
            Page::General => self.general_view(),
            Page::Codex => self
                .codex
                .view(self.codex_context(enabled))
                .map(Action::Codex),
            Page::Plugins => self
                .plugins
                .view(self.plugin_context(enabled))
                .map(Action::Plugins),
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
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill);
        let sidebar = container(navigation)
            .padding(12)
            .width(180)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(design::BASE.into()),
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
        let footer = if self.pending() {
            "API-key write submitted; closing or switching pages cannot undo it. Awaiting acknowledgment."
        } else if self.plugins.pending() {
            "Plugin setting write submitted; closing or switching pages cannot undo it. Awaiting acknowledgment."
        } else if self.codex.pending() {
            "Codex sign-in or configuration pending; return to Codex to cancel. Closing clears drafts but does not undo admitted credential writes."
        } else {
            "Closing or switching sections clears unsaved drafts."
        };
        container(column![
            container(
                row![
                    label("Settings", TEXT).size(26).width(Length::Fill),
                    button("Close  ×")
                        .padding([9, 12])
                        .style(design::button_style)
                        .on_press(Action::Close)
                ]
                .align_y(iced::Alignment::Center)
            )
            .padding([16, 20]),
            sections,
            container(label(footer, MUTED).size(11)).padding([12, 20])
        ])
        .width(width.max(1.0))
        .height(height.max(1.0))
        .style(|_| design::card(SURFACE, 20.0))
        .clip(true)
        .into()
    }
    fn general_view(&self) -> Element<'_, Action> {
        let mut body = column![
            label("General", TEXT).size(24),
            label("Appearance", ACCENT).size(16),
            label("Dark Rosé Pine · DejaVu Sans 14 · read-only", TEXT),
            self.info_button(Info::Details, "Details", self.details_open),
        ]
        .spacing(14);
        if self.details_open {
            body = body
                .push(label("Native Iced / wgpu interface; no embedded browser or webview. Appearance is fixed, not an editable preference.", MUTED))
                .push(label("API login saves a key after Review and Confirm. Codex connects ChatGPT in one click; Advanced provides manual recovery. Plugins use explicit Refresh and schema-defined settings with Review and Confirm.", MUTED));
        }
        body = body.push(self.info_button(Info::Shortcuts, "Shortcuts", self.shortcuts_open));
        if self.shortcuts_open {
            body = body.push(label("Ctrl+B · navigation\nCtrl+Enter · send in the focused composer\nEnter · new line\nEscape · close settings and clear unsaved drafts", TEXT));
        }
        body.into()
    }
    fn api_view(&self, enabled: bool) -> Element<'_, Action> {
        let ticket = self.ticket();
        let can_save = self.can_save(enabled);
        let editable = enabled
            && !self.pending()
            && self.uncertain_save.is_none()
            && !matches!(self.save, SavePhase::Confirm(_));
        let mut body = column![
            label("API login", TEXT).size(24),
            label("API key", ACCENT).size(16),
            label(
                "Saving may replace credentials; no sign-in or key validation.",
                MUTED
            )
        ]
        .spacing(18);
        if self.uncertain_save.is_some() {
            body = body.push(label(
                "An earlier API-key save is still uncertain. Further API-key writes are disabled for this app lifetime. Metadata cannot confirm that receipt. Restart only after the owned backend has fully stopped; restarting does not undo or verify the write.",
                DANGER,
            ));
        }
        if let Some(meta) = self.metadata {
            body = body.push(label(
                format!(
                    "Read-only metadata: logged in={} · key stored={} · settings writable={}",
                    meta.logged_in, meta.has_api_key, meta.writable
                ),
                TEXT,
            ));
        } else {
            body = body.push(label(
                if self.reading.is_some() {
                    "Reading nonsecret onboarding metadata…"
                } else {
                    "Metadata unknown; Save disabled until explicit Refresh."
                },
                MUTED,
            ));
        }
        body = body.push(
            button("Refresh nonsecret metadata")
                .padding([10, 14])
                .style(design::button_style)
                .on_press_maybe((enabled && !self.pending()).then_some(Action::Refresh(ticket))),
        );
        let input_ticket = self.input_ticket();
        let input = text_input(
            "API key (masked, maximum 16384 ASCII bytes)",
            &self.edit.editor.0,
        )
        .padding(14)
        .size(15)
        .style(design::input_style)
        .secure(true)
        .id("native-api-key-draft");
        body = body.push(if editable {
            input.on_input(move |raw| Action::Edit {
                ticket: input_ticket,
                input: Input::new(raw),
            })
        } else {
            input
        });
        body = body.push(
            label(
                "Draft clears on submission or close. Masking is not memory protection.",
                MUTED,
            )
            .size(12),
        );
        match self.save {
            SavePhase::Confirm(t) => {
                body=body.push(label("Confirm persistence to this owned Host's configured credential provider. Default isolated profile uses native-home credentials; trusted custom providers may write elsewhere. Existing credentials may be replaced. This does not validate the key or sign in.",DANGER)).push(column![button("Confirm Save API key").padding([11, 18]).style(design::primary_button).on_press_maybe(can_save.then_some(Action::Confirm(t))),button("Cancel write").padding([10, 14]).style(design::button_style).on_press(Action::CancelReview(t))].spacing(10));
            }
            SavePhase::Pending(_) => {
                body=body.push(label("Write submitted; awaiting actual persistence acknowledgment. Draft cleared. Closing does not undo a write already submitted.",MUTED));
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
                        button("Review explicit Save")
                            .padding([11, 18])
                            .style(design::primary_button)
                            .on_press_maybe(can_save.then_some(Action::Review(ticket))),
                    );
            }
            SavePhase::EndedElsewhere => {
                body=body.push(label("An earlier closed-panel attempt returned; its outcome is not applied here. It may have persisted; this draft was not submitted. Refresh explicitly; no automatic retry.",DANGER)).push(button("Review explicit Save").padding([11, 18]).style(design::primary_button).on_press_maybe(can_save.then_some(Action::Review(ticket))));
            }
            SavePhase::Idle => {
                body = body.push(
                    button("Review explicit Save")
                        .padding([11, 18])
                        .style(design::primary_button)
                        .on_press_maybe(can_save.then_some(Action::Review(ticket))),
                );
            }
        }
        if !enabled {
            body=body.push(label("Read-only: unavailable/stopped/closing backend or real keyless smoke. No key save allowed.",DANGER));
        }
        if let Some(error) = self.error {
            body = body.push(label(error, DANGER));
        }
        body = body.push(self.info_button(Info::ApiHelp, "Help & privacy", self.api_help_open));
        if self.api_help_open {
            body = body
                .push(label("API login only persists a key to the owned Host. No browser sign-in, provider catalog, account probe or model validation. Metadata is not proof that a key works.", MUTED))
                .push(label("No key is read back from Core. The owned draft is consumed on submission and wiped on close. Do not paste real secrets into test fixtures or artifacts.", MUTED))
                .push(label("Privacy limit: this app wipes its owned draft/holder and Core wipes its owned input/frame. Iced editor/render copies, clipboard, allocator/kernel/Node copies and swap cannot be guaranteed wiped; masking is not memory protection.", MUTED));
        }
        body.into()
    }
}

#[cfg(test)]
mod disclosure_tests {
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
                input: Input::new("PUBLIC_FAKE_KEY".into()),
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
            assert_eq!(state.edit.editor.0, "PUBLIC_FAKE_KEY");
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
}
