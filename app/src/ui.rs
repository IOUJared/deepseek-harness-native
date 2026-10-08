use crate::{
    config::Options,
    design,
    reducer::{self, DisplayRow, Transcript},
    worker::{self, Command, Event, Feed, Handle},
};
use dsh_native_core::{Inspection, PublicEvent, StopResult};
use dsh_native_transport::dto::*;
use iced::widget::{
    Column, Space, button, column, container, pick_list, row, scrollable, text, text_editor,
    text_input,
};
use iced::{Color, Element, Length, Subscription, Task, Theme, keyboard, window};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
const ROW_HEIGHT: f32 = 160.0;
#[cfg(test)]
const ACTIVITY_HEIGHT: f32 = 40.0;
pub(crate) const BASE: Color = Color::from_rgb8(0x19, 0x17, 0x24);
pub(crate) const SURFACE: Color = Color::from_rgb8(0x1f, 0x1d, 0x2e);
pub(crate) const OVERLAY: Color = Color::from_rgb8(0x26, 0x23, 0x3a);
pub(crate) const TEXT: Color = Color::from_rgb8(0xe0, 0xde, 0xf4);
pub(crate) const MUTED: Color = Color::from_rgb8(0x90, 0x8c, 0xaa);
pub(crate) const ACCENT: Color = Color::from_rgb8(0xc4, 0xa7, 0xe7);
pub(crate) const DANGER: Color = Color::from_rgb8(0xeb, 0x6f, 0x92);
pub const FONT: iced::Font = iced::Font::with_name("DejaVu Sans");
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effort {
    id: String,
    name: String,
}
impl std::fmt::Display for Effort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    provider: String,
    id: String,
    label: String,
    efforts: Vec<Effort>,
    default_effort: Option<String>,
}
impl std::fmt::Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}
#[derive(Clone, Copy)]
pub enum InfoSection {
    Features,
    Diagnostics,
}
impl InfoSection {
    fn bit(self) -> u8 {
        match self {
            Self::Features => 1,
            Self::Diagnostics => 2,
        }
    }
}
#[path = "attachment_draft.rs"]
mod attachment_draft;
#[cfg(test)]
#[path = "attachment_ui_tests.rs"]
mod attachment_ui_tests;
#[path = "detail_panel.rs"]
mod detail_panel;
#[cfg(test)]
#[path = "detail_ui_tests.rs"]
mod detail_ui_tests;
#[derive(Clone)]
pub enum Message {
    FileOpen,
    FilePath(attachment_draft::EditorStamp, String),
    FileUpload(attachment_draft::EditorStamp),
    FileCancel(attachment_draft::EditorStamp),
    FileRemove(worker::attachments::Ticket),
    ToggleSidebar,
    ToggleConversationMenu,
    ToggleInfoDetails(InfoSection),
    ToggleRecords,
    DismissPanel,
    #[cfg(test)]
    ModalBlocked,
    Resized(iced::Size),
    WindowActive(bool),
    MotionFrame(Instant),
    Settings(crate::settings::Action),
    Management(crate::management::Action),
    ExportSession(crate::management::Ticket),
    Export(crate::exporter::Action),
    ArchivedView,
    Interaction(crate::interactions::Action),
    Worker(Event),
    Editor(text_editor::Action),
    Send,
    Stop,
    Select(SessionId),
    Workspace(Option<WorkspaceId>),
    NewSession,
    WorkspacePath(String),
    WorkspaceControls,
    OpenWorkspace,
    Filter(String),
    LoadModels,
    Model(Model),
    Effort(Effort),
    Older,
    Reload,
    Scrolled {
        generation: u64,
        revision: Option<u64>,
        viewport: scrollable::Viewport,
    },
    ScrollApplied {
        generation: u64,
        revision: Option<u64>,
        feedback: Option<u64>,
        y: f32,
        viewport: f32,
    },
    Bottom,
    Details(detail_panel::Opening),
    DetailMode(detail_panel::Stamp, detail_panel::Mode),
    HideDetails(detail_panel::Stamp),
    CopyDetailSource(detail_panel::Stamp),
    Capabilities,
    Close,
    Stay,
    ConfirmClose,
    ExitAnyway,
    SmokeDeadline,
    CaptureOwnWindow(window::Id),
    OwnScreenshot(window::Screenshot),
    ScreenshotSaved {
        success: bool,
        size: [u32; 2],
    },
    NativeOpened(window::Id),
    NativeInfo {
        backend: &'static str,
        size: iced::Size,
        native_scale: f32,
    },
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeUiMessage(redacted)")
    }
}
enum Close {
    None,
    Inspecting,
    Confirm(String),
    Stopping,
    Failed(String),
}
struct Pending {
    session: SessionId,
    text: String,
    file_request_id: Option<SessionRequestId>,
}
pub struct App {
    pub options: Options,
    handle: Handle,
    feed: Feed,
    ready: bool,
    root_ready: bool,
    stopped: bool,
    status: String,
    account: String,
    sessions: BTreeMap<SessionId, SessionSummary>,
    roster_pending: bool,
    early_root: Vec<RemoteEventFrame>,
    workspaces: Vec<WorkspaceView>,
    workspace: Option<WorkspaceId>,
    management: crate::management::Management,
    exporter: crate::exporter::Exporter,
    files: attachment_draft::Draft,
    show_archived: bool,
    workspace_path: String,
    workspace_controls: bool,
    filter: String,
    selected: Option<SessionId>,
    generation: u64,
    transcript: Transcript,
    follow_ready: bool,
    editor: text_editor::Content,
    drafts: BTreeMap<SessionId, String>,
    pending: BTreeMap<u64, Pending>,
    prompt_serial: u64,
    models: Vec<Model>,
    chosen_model: Option<Model>,
    chosen_effort: Option<Effort>,
    model_pending: bool,
    catalog_pending: bool,
    model_selections: BTreeMap<SessionId, ModelSelection>,
    new_pending: bool,
    workspace_pending: bool,
    page_pending: bool,
    cancel_pending: bool,
    offset: f32,
    viewport: f32,
    follow_bottom: bool,
    detail: Option<detail_panel::Detail>,
    detail_opening: Option<u64>,
    capabilities: bool,
    conversation_menu: bool,
    info_expanded: u8,
    settings: crate::settings::Settings,
    decisions: crate::interactions::Decisions,
    transport_epoch: u64,
    projections: BTreeMap<String, serde_json::Value>,
    title_updates: BTreeMap<SessionId, (i64, serde_json::Value)>,
    close: Close,
    last_stop: Option<Result<StopResult, String>>,
    smoke_evidence: crate::smoke::Evidence,
    own_window: Option<window::Id>,
    window_size: iced::Size,
    window_active: bool,
    sidebar_expanded: bool,
    sidebar_sheet: bool,
    records_expanded: bool,
    sidebar_motion: design::SidebarMotion,
    chat_geometry: chat_geometry::Cache,
    scroll_revision: Option<u64>,
    scroll_feedback: Option<u64>,
    scroll_target: Option<f32>,
}
impl App {
    pub fn boot(options: Options, handle: Handle, feed: Feed) -> (Self, Task<Message>) {
        let workspace_path = options
            .backend
            .working_directory
            .to_string_lossy()
            .into_owned();
        let deadline = options.smoke.as_ref().map(|smoke| smoke.exit_after);
        let screenshot_requested = options
            .smoke
            .as_ref()
            .is_some_and(|smoke| smoke.screenshot.is_some());
        let task = deadline.map_or_else(Task::none, |duration| {
            Task::perform(
                async move {
                    tokio::time::sleep(duration).await;
                },
                |()| Message::SmokeDeadline,
            )
        });
        (
            Self {
                options,
                handle,
                feed,
                ready: false,
                root_ready: false,
                stopped: false,
                status: "Starting isolated owned Core…".into(),
                account: "Account metadata not loaded".into(),
                sessions: BTreeMap::new(),
                roster_pending: true,
                early_root: vec![],
                workspaces: vec![],
                workspace: None,
                management: crate::management::Management::default(),
                exporter: crate::exporter::Exporter::default(),
                files: attachment_draft::Draft::default(),
                show_archived: false,
                workspace_path,
                workspace_controls: false,
                filter: String::new(),
                selected: None,
                generation: 0,
                transcript: Transcript::new(0),
                follow_ready: false,
                editor: text_editor::Content::new(),
                drafts: BTreeMap::new(),
                pending: BTreeMap::new(),
                prompt_serial: 0,
                models: vec![],
                chosen_model: None,
                chosen_effort: None,
                model_pending: false,
                catalog_pending: false,
                model_selections: BTreeMap::new(),
                new_pending: false,
                workspace_pending: false,
                page_pending: false,
                cancel_pending: false,
                offset: 0.0,
                viewport: 480.0,
                follow_bottom: true,
                detail: None,
                detail_opening: Some(0),
                capabilities: false,
                conversation_menu: false,
                info_expanded: 0,
                settings: Default::default(),
                decisions: Default::default(),
                transport_epoch: 0,
                projections: BTreeMap::new(),
                title_updates: BTreeMap::new(),
                close: Close::None,
                last_stop: None,
                smoke_evidence: crate::smoke::Evidence {
                    screenshot_requested,
                    ..Default::default()
                },
                own_window: None,
                window_size: iced::Size::new(1280.0, 820.0),
                window_active: false,
                sidebar_expanded: true,
                sidebar_sheet: false,
                records_expanded: false,
                sidebar_motion: design::SidebarMotion::new(design::SIDEBAR),
                chat_geometry: Default::default(),
                scroll_revision: Some(0),
                scroll_feedback: Some(0),
                scroll_target: None,
            },
            task,
        )
    }
    pub fn subscription(&self) -> Subscription<Message> {
        let animation = if self.animation_active() {
            iced::time::every(Duration::from_millis(16)).map(Message::MotionFrame)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            Subscription::run_with(self.feed.clone(), worker::stream).map(Message::Worker),
            window::events().filter_map(|(id, event)| match event {
                window::Event::CloseRequested => Some(Message::Close),
                window::Event::Opened { .. } => Some(Message::NativeOpened(id)),
                window::Event::Resized(size) => Some(Message::Resized(size)),
                window::Event::Focused => Some(Message::WindowActive(true)),
                window::Event::Unfocused | window::Event::Closed => {
                    Some(Message::WindowActive(false))
                }
                _ => None,
            }),
            keyboard::listen().with(self.detail_escape_ticket()).filter_map(|(ticket,event)| match event {
                keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    repeat: false,
                    ..
                } => panel_shortcut(&key, modifiers, ticket),
                _ => None,
            }),
            animation,
        ])
    }
    fn layout_size(&self) -> iced::Size {
        // Iced's Resized and get_size already include native * application scale.
        self.window_size
    }
    fn compact(&self) -> bool {
        self.layout_size().width < design::BREAKPOINT
    }
    fn sidebar_target(&self) -> f32 {
        if !self.compact() && self.sidebar_expanded {
            design::SIDEBAR
        } else {
            design::RAIL
        }
    }
    fn resize_layout(&mut self, size: iced::Size) {
        if !size.width.is_finite()
            || !size.height.is_finite()
            || size.width <= 0.0
            || size.height <= 0.0
        {
            return;
        }
        self.window_size = size;
        self.conversation_menu = false;
        self.sidebar_motion.snap(self.sidebar_target());
        if !self.compact() {
            self.sidebar_sheet = false;
        }
    }
    fn layout_controls_enabled(&self) -> bool {
        matches!(self.close, Close::None)
            && !self.settings.is_open()
            && !self.decisions.is_open()
            && !self.management.is_open()
            && !self.exporter.is_open()
    }
    fn animation_active(&self) -> bool {
        self.sidebar_motion
            .running(self.window_active && self.layout_controls_enabled())
    }
    fn composer_visible(&self) -> bool {
        self.layout_controls_enabled()
            && !self.capabilities
            && !self.conversation_menu
            && self.detail.is_none()
            && !self.sidebar_sheet
    }
    fn detail_visible(&self) -> bool {
        self.layout_controls_enabled() && !self.capabilities && !self.conversation_menu
            && !self.sidebar_sheet && self.detail.is_some()
    }
    fn detail_request(&self, key:u64) -> Option<Message> {
        self.detail_opening.filter(|opening|*opening<u64::MAX).map(|prior_opening|Message::Details(detail_panel::Opening {
            generation:self.generation, prior_opening, key,
        }))
    }
    fn detail_back(&self) -> Option<Message> {
        self.detail.as_ref().and_then(|detail|detail.ticket).map(Message::HideDetails)
    }
    fn detail_copy_source(&self, ticket: detail_panel::Stamp) -> Option<String> {
        if !self.detail_visible() || ticket.generation != self.generation {
            return None;
        }
        self.detail.as_ref().filter(|detail| detail.ticket == Some(ticket) && detail.copy_available())
            .map(|detail| detail.source.clone())
    }
    fn detail_escape_ticket(&self) -> Option<detail_panel::Stamp> {
        self.detail_visible().then(||self.detail.as_ref().and_then(|detail|detail.ticket)).flatten()
    }
    fn open_detail(&mut self,heading:String,source:String,metadata:Option<String>,assistant:bool) {
        let Some(opening)=self.detail_opening.and_then(|n|n.checked_add(1)) else {
            // Never mint a reusable ticket after exhaustion. Keep any readable old
            // snapshot and its valid Back ticket, but reject all new openings.
            self.detail_opening=None;
            return;
        };
        self.detail_opening=Some(opening);
        let ticket=Some(detail_panel::Stamp{generation:self.generation,opening});
        self.detail=Some(detail_panel::Detail::new(heading,source,metadata,assistant,ticket));
    }
    fn command(&mut self, command: Command) {
        if matches!(
            &command,
            Command::Prompt { .. } | Command::PromptFile { .. }
        ) {
            self.smoke_evidence.model_prompts += 1;
        }
        if matches!(&command, Command::Catalog) {
            self.smoke_evidence.catalog_requested = true;
        }
        if let Err(command) = self.handle.send(command) {
            self.status = "Native worker queue full or closed; operation not sent".into();
            match command {
                Command::StageFile(submission) => {
                    let context = self.file_context();
                    self.files.complete(
                        &submission.ticket,
                        worker::attachments::Outcome::NotSent,
                        &context,
                    );
                }
                Command::PromptFile { ticket, request } => {
                    let context = self.file_context();
                    self.files.prompt_complete(
                        &ticket,
                        &request.request_id,
                        worker::attachments::PromptOutcome::NotSent,
                        &context,
                    );
                    self.pending.remove(&ticket.generation);
                }
                Command::Codex(effect) => {
                    use crate::codex::Effect;
                    let ticket = match &effect {
                        Effect::StatusRead { ticket }
                        | Effect::StartLogin { ticket }
                        | Effect::OpenBrowser { ticket, .. }
                        | Effect::SubmitCallback { ticket, .. }
                        | Effect::CancelLogin { ticket, .. }
                        | Effect::EnableModels { ticket, .. } => *ticket,
                    };
                    drop(effect);
                    self.settings
                        .codex_completed(ticket, crate::codex::Outcome::NotSent);
                }
                Command::PluginsRead(ticket) => self
                    .settings
                    .plugins_loaded(ticket, Err(crate::plugins::ReadFailure::Unavailable)),
                Command::PluginSave { ticket, .. } => self
                    .settings
                    .plugin_saved(ticket, crate::plugins::SaveOutcome::NotSent),
                Command::Manage(submission) => self.management.finish(&submission, Err(()), true),
                Command::Export(submission) => {
                    self.exporter
                        .finish(&submission.receipt, crate::exporter::Outcome::NotSent);
                }
                Command::KeyMetadata(ticket) => self.settings.metadata(ticket, Err(())),
                Command::SaveKey { ticket, secret } => {
                    drop(secret);
                    self.settings
                        .saved(ticket, crate::settings::SaveResult::NotSent);
                }
                Command::Decision(submission) => self.decisions.fail(
                    &submission.key,
                    submission.attempt,
                    "Native queue full or closed; reply not sent".into(),
                ),
                Command::Inspect => {
                    self.close = Close::Confirm(
                        "Inspection unavailable; idle state unknown. Close only the owned backend?"
                            .into(),
                    )
                }
                Command::Shutdown => {
                    self.close = Close::Failed("Owned shutdown signal unavailable".into())
                }
                Command::Create { .. } => self.new_pending = false,
                Command::OpenWorkspace(_) => self.workspace_pending = false,
                Command::Catalog => self.catalog_pending = false,
                Command::Prompt { generation, .. } => {
                    self.pending.remove(&generation);
                }
                Command::SelectModel { generation, .. } if generation == self.generation => {
                    self.model_pending = false
                }
                Command::Page { generation, .. } if generation == self.generation => {
                    self.page_pending = false
                }
                Command::Cancel { generation, .. } if generation == self.generation => {
                    self.cancel_pending = false
                }
                _ => {}
            }
        }
    }
    fn choose(&mut self, session: SessionId) {
        self.files.invalidate();
        if let Some(old) = &self.selected {
            self.drafts.insert(old.clone(), self.editor.text());
        }
        self.editor =
            text_editor::Content::with_text(self.drafts.get(&session).map_or("", String::as_str));
        self.management.dismiss();
        self.exporter.dismiss();
        self.selected = Some(session.clone());
        self.generation += 1;
        self.transcript = Transcript::new(self.generation);
        self.chat_geometry = Default::default();
        self.invalidate_scroll();
        self.follow_ready = false;
        self.page_pending = false;
        self.cancel_pending = false;
        self.model_pending = false;
        self.detail = None;
        self.conversation_menu = false;
        self.sidebar_sheet = false;
        self.records_expanded = false;
        self.offset = 0.0;
        self.follow_bottom = true;
        self.projections.clear();
        self.chosen_model = self
            .model_selections
            .get(&session)
            .and_then(|s| {
                self.models
                    .iter()
                    .find(|m| m.provider == s.provider && m.id == s.model)
            })
            .cloned();
        self.chosen_effort = self
            .chosen_model
            .as_ref()
            .and_then(|m| {
                self.model_selections.get(&session).and_then(|s| {
                    m.efforts
                        .iter()
                        .find(|e| Some(&e.id) == s.reasoning_effort.as_ref())
                })
            })
            .cloned();
        self.status = "Opening authoritative session snapshot…".into();
        let address = self.address(&session);
        self.command(Command::Select {
            generation: self.generation,
            address,
        });
    }
    fn address(&self, session: &SessionId) -> SessionAddress {
        match self.sessions.get(session) {
            Some(summary) if summary.origin.is_some() => SessionAddress::Subagent {
                parent_session_id: summary
                    .parent_session_id
                    .clone()
                    .unwrap_or_else(|| session.clone()),
                child_session_id: session.clone(),
                mode: SubagentMode::Unknown,
            },
            _ => SessionAddress::Session {
                session_id: session.clone(),
            },
        }
    }
    fn file_context(&self) -> attachment_draft::Context {
        attachment_draft::Context {
            epoch: self.transport_epoch,
            generation: self.generation,
            target: self.selected.clone(),
            allowed: self.options.smoke.is_none()
                && self.ready
                && self.root_ready
                && self.follow_ready
                && self.transcript.safe_to_send()
                && self.selected.as_ref().is_some_and(|id| {
                    self.sessions.get(id).is_none_or(|s| s.origin.is_none())
                        && !self.management.registry.archived(id)
                        && self.management.registry.ready()
                        && !self.pending.values().any(|p| &p.session == id)
                })
                && !self.model_pending
                && matches!(self.close, Close::None)
                && self.composer_visible(),
        }
    }
    fn file_local_context(&self) -> attachment_draft::Context {
        let mut context = self.file_context();
        context.allowed = self.options.smoke.is_none()
            && matches!(self.close, Close::None)
            && self.composer_visible()
            && !self.files.prompt_pending();
        context
    }
    fn send_allowed(&self) -> bool {
        self.options.smoke.is_none()
            && self.ready
            && self.root_ready
            && self.follow_ready
            && self.transcript.safe_to_send()
            && self.selected.as_ref().is_some_and(|id| {
                self.sessions.get(id).is_none_or(|s| s.origin.is_none())
                    && !self.management.registry.archived(id)
                    && self.management.registry.ready()
                    && !self.pending.values().any(|p| &p.session == id)
            })
            && !self.model_pending
            && self.files.can_send(&self.file_context())
            && (!self.editor.text().trim().is_empty() || self.files.has_file(&self.file_context()))
            && self.editor.text().len() <= 32 * 1024
            && matches!(self.close, Close::None)
            && self.composer_visible()
    }
    fn stop_allowed(&self) -> bool {
        self.ready
            && self.selected.is_some()
            && !self.cancel_pending
            && matches!(self.close, Close::None)
    }
    fn shutdown(&mut self) {
        self.files.invalidate();
        self.management.disconnect();
        self.exporter.disconnect();
        self.settings.disconnect();
        self.settings.close();
        self.close = Close::Stopping;
        self.ready = false;
        self.command(Command::Shutdown);
    }
    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Fence stale widget messages too: the dimmed shell is presentation-only.
        if self.settings.is_open() && background_message(&message) {
            return Task::none();
        }
        if self.exporter.is_open()
            && !self.settings.is_open()
            && ((background_message(&message) && !matches!(&message, Message::Export(_)))
                || matches!(&message, Message::Settings(_)))
        {
            return Task::none();
        }
        if self.management.is_open()
            && !self.settings.is_open()
            && !self.exporter.is_open()
            && ((background_message(&message)
                && !matches!(&message, Message::Management(_) | Message::ExportSession(_)))
                || matches!(&message, Message::Settings(_)))
        {
            return Task::none();
        }
        let was_active = self.transcript_active();
        let geometry_change = matches!(
            &message,
            Message::Resized(_)
                | Message::MotionFrame(_)
                | Message::WindowActive(_)
                | Message::ToggleSidebar
                | Message::ToggleRecords
        );
        let reader_anchor = if geometry_change {
            self.reader_anchor()
        } else {
            None
        };
        if matches!(
            &message,
            Message::Settings(_)
                | Message::Management(_)
                | Message::ExportSession(_)
                | Message::Export(_)
                | Message::Capabilities
                | Message::Close
                | Message::Select(_)
                | Message::Workspace(_)
                | Message::ToggleSidebar
        ) {
            self.conversation_menu = false;
        }
        match message {
            Message::FileOpen => {
                let context = self.file_context();
                self.files.open(&context);
            }
            Message::FilePath(stamp, path) => {
                let context = self.file_context();
                self.files.edit(stamp, path, &context);
            }
            Message::FileUpload(stamp) => {
                let context = self.file_context();
                if let Some(submission) = self.files.stage(stamp, &context) {
                    self.command(Command::StageFile(submission));
                }
            }
            Message::FileCancel(stamp) => {
                let context = self.file_local_context();
                self.files.cancel_editor(stamp, &context);
            }
            Message::FileRemove(ticket) => {
                let context = self.file_local_context();
                if context.allowed && self.files.ticket() == Some(&ticket) {
                    if self.handle.send(Command::DiscardFile(ticket)).is_ok() {
                        self.files.remove(&context);
                    } else {
                        self.status =
                            "Removal not delivered; attachment remains owned. Try Remove again."
                                .into();
                    }
                }
            }
            Message::ToggleConversationMenu
                if self.layout_controls_enabled() && !self.sidebar_sheet =>
            {
                self.conversation_menu = !self.conversation_menu;
            }
            Message::ToggleInfoDetails(section)
                if self.layout_controls_enabled()
                    && self.capabilities
                    && !self.conversation_menu
                    && !self.sidebar_sheet =>
            {
                self.info_expanded ^= section.bit();
            }
            Message::ArchivedView if self.layout_controls_enabled() => {
                self.show_archived = !self.show_archived;
            }
            Message::DismissPanel
                if self.management.is_open()
                    && !self.exporter.is_open()
                    && !self.settings_modal_visible() =>
            {
                self.management.dismiss()
            }
            Message::DismissPanel if self.exporter.is_open() && !self.settings_modal_visible() => {
                self.exporter.dismiss();
            }
            Message::ExportSession(ticket) => {
                let context = self.management_context();
                if context.allowed
                    && self.management.ticket().as_ref() == Some(&ticket)
                    && context.view.as_ref().is_some_and(|view| {
                        view.epoch == ticket.epoch
                            && view.generation == ticket.generation
                            && view.target == ticket.target
                    })
                    && !self.management.pending()
                    && self.exporter.pending().is_none()
                {
                    if let Some(view) = context.view {
                        self.management.dismiss();
                        let context = self.export_context();
                        self.exporter
                            .apply(crate::exporter::Action::Open(view), &context);
                    }
                }
            }
            Message::Export(action) => {
                let context = self.export_context();
                if let Some(submission) = self.exporter.apply(action, &context) {
                    self.command(Command::Export(submission));
                }
            }
            Message::Management(action) => {
                let context = self.management_context();
                if let Some(submission) = self.management.update(action, &context) {
                    self.command(Command::Manage(submission));
                }
            }
            #[cfg(test)]
            Message::ModalBlocked => {}
            Message::DismissPanel if self.settings_modal_visible() => {
                // Explicit Escape has the same draft clearing/indeterminate-write
                // semantics as the settings Close button; it never submits a key.
                return self.update(Message::Settings(crate::settings::Action::Close));
            }
            Message::Resized(size) => self.resize_layout(size),
            Message::WindowActive(active) => {
                self.window_active = active;
                if !active {
                    self.sidebar_motion.snap(self.sidebar_target());
                }
            }
            Message::MotionFrame(now) => self.sidebar_motion.advance(now),
            Message::ToggleSidebar if self.layout_controls_enabled() => {
                if self.compact() {
                    self.sidebar_sheet = !self.sidebar_sheet;
                } else {
                    self.sidebar_expanded = !self.sidebar_expanded;
                    self.sidebar_motion.retarget(
                        self.sidebar_target(),
                        Instant::now(),
                        self.window_active && self.transcript.rows().len() <= 128,
                    );
                }
            }
            Message::ToggleRecords if self.layout_controls_enabled() => {
                self.records_expanded = !self.records_expanded;
            }
            Message::DismissPanel if self.layout_controls_enabled() && self.conversation_menu => {
                self.conversation_menu = false;
            }
            Message::DismissPanel if self.layout_controls_enabled() => {
                self.sidebar_sheet = false;
                self.capabilities = false;
                // Stateless composer/old subscription dismissals never close Details.
                // The visible Details Escape subscription carries its opening ticket.
            }
            Message::Settings(action) => {
                let enabled = self.settings_enabled();
                if let Some(effect) = self.settings.handle(action, self.transport_epoch, enabled) {
                    self.command(match effect {
                        crate::settings::Effect::Codex(effect) => Command::Codex(effect),
                        crate::settings::Effect::Plugins(effect) => match effect {
                            crate::plugins::Effect::Read(ticket) => Command::PluginsRead(ticket),
                            crate::plugins::Effect::Save {
                                ticket,
                                namespace,
                                field,
                                value,
                            } => Command::PluginSave {
                                ticket,
                                namespace,
                                field,
                                value,
                            },
                        },
                        crate::settings::Effect::Read(ticket) => Command::KeyMetadata(ticket),
                        crate::settings::Effect::Save { ticket, secret } => {
                            Command::SaveKey { ticket, secret }
                        }
                    });
                }
            }
            Message::Interaction(action) => {
                let enabled = self.decision_replies_enabled();
                if let Some(submission) =
                    self.decisions.handle(action, self.transport_epoch, enabled)
                {
                    self.command(Command::Decision(submission));
                }
            }
            Message::NativeOpened(id) => {
                self.own_window = Some(id);
                return window::run(id, |window| {
                    match window.display_handle().map(|h| h.as_raw()) {
                        Ok(raw_window_handle::RawDisplayHandle::Wayland(_)) => "wayland",
                        Ok(
                            raw_window_handle::RawDisplayHandle::Xlib(_)
                            | raw_window_handle::RawDisplayHandle::Xcb(_),
                        ) => "x11",
                        _ => "other_or_unavailable",
                    }
                })
                .then(move |backend| {
                    window::scale_factor(id).then(move |native_scale| {
                        window::size(id).map(move |size| Message::NativeInfo {
                            backend,
                            size,
                            native_scale,
                        })
                    })
                });
            }
            Message::NativeInfo {
                backend,
                size,
                native_scale,
            } => {
                self.resize_layout(size);
                self.smoke_evidence.backend = Some(backend);
                self.smoke_evidence.logical_size = Some([size.width, size.height]);
                self.smoke_evidence.native_scale = Some(native_scale);
                return self.maybe_screenshot();
            }
            Message::CaptureOwnWindow(id)
                if self.smoke_evidence.screenshot_scheduled
                    && !self.smoke_evidence.screenshot_started
                    && self.smoke_evidence.snapshot_valid
                    && self.follow_ready
                    && self.own_window == Some(id)
                    && matches!(self.close, Close::None) =>
            {
                self.smoke_evidence.screenshot_started = true;
                return window::screenshot(id).map(Message::OwnScreenshot);
            }
            Message::OwnScreenshot(screenshot) => {
                if let Some(path) = self
                    .options
                    .smoke
                    .as_ref()
                    .and_then(|s| s.screenshot.clone())
                {
                    let size = [screenshot.size.width, screenshot.size.height];
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                crate::smoke::save_screenshot(&path, &screenshot).is_ok()
                            })
                            .await
                            .unwrap_or(false)
                        },
                        move |success| Message::ScreenshotSaved { success, size },
                    );
                }
            }
            Message::ScreenshotSaved { success, size } => {
                self.smoke_evidence.screenshot_saved = Some(success);
                self.smoke_evidence.screenshot_size = Some(size);
            }
            Message::SmokeDeadline if self.options.smoke.is_some() => {
                self.smoke_evidence.deadline_reached = true;
                self.shutdown();
            }
            Message::Worker(event) => {
                let task = self.receive(event);
                if !self.layout_controls_enabled() {
                    self.conversation_menu = false;
                }
                return task;
            }
            Message::Editor(action) => {
                self.editor.perform(action);
            }
            Message::Send if self.send_allowed() => {
                let session = self.selected.clone().unwrap();
                let value = self.editor.text();
                let Some(serial) = self.prompt_serial.checked_add(1) else {
                    self.status =
                        "Native prompt authority exhausted; restart before sending".into();
                    return Task::none();
                };
                self.prompt_serial = serial;
                let request_id = SessionRequestId::new(format!(
                    "native-{}-{}-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_nanos()),
                    self.generation,
                    serial
                ))
                .unwrap();
                let context = self.file_context();
                let file_ticket = if self.files.has_file(&context) {
                    match self.files.begin_prompt(&context, request_id.clone()) {
                        Some(ticket) => Some(ticket),
                        None => {
                            self.status = "Attachment authority changed; prompt not sent".into();
                            return Task::none();
                        }
                    }
                } else {
                    None
                };
                let request = SessionPromptRequest {
                    request_id: request_id.clone(),
                    session_id: session.clone(),
                    mode: PromptMode::Queue,
                    content: if value.is_empty() {
                        Vec::new()
                    } else {
                        vec![PromptContentPart::Text {
                            text: value.clone(),
                        }]
                    },
                    client_time_zone: Some("UTC".into()),
                };
                self.pending.insert(
                    self.generation,
                    Pending {
                        session,
                        text: value,
                        file_request_id: file_ticket.as_ref().map(|_| request_id),
                    },
                );
                self.status = "Sending prompt; awaiting Host inbox acceptance…".into();
                if let Some(ticket) = file_ticket {
                    self.command(Command::PromptFile { ticket, request });
                } else {
                    self.command(Command::Prompt {
                        generation: self.generation,
                        request,
                    });
                }
            }
            Message::Stop if self.stop_allowed() => {
                self.cancel_pending = true;
                self.status = "Requesting cancellation…".into();
                self.command(Command::Cancel {
                    generation: self.generation,
                    session: self.selected.clone().unwrap(),
                });
            }
            Message::Select(id) if self.ready => self.choose(id),
            Message::Workspace(id) => {
                self.workspace = id;
            }
            Message::NewSession
                if self.options.smoke.is_none() && self.ready && !self.new_pending =>
            {
                self.new_pending = true;
                self.status = "Creating real session…".into();
                let cwd = self.workspace.is_none().then(|| {
                    self.options
                        .backend
                        .working_directory
                        .to_string_lossy()
                        .into_owned()
                });
                self.command(Command::Create {
                    workspace: self.workspace.clone(),
                    cwd,
                });
            }
            Message::WorkspaceControls => self.workspace_controls = !self.workspace_controls,
            Message::WorkspacePath(value) => self.workspace_path = value,
            Message::OpenWorkspace if self.ready && !self.workspace_pending => {
                let path = std::path::Path::new(&self.workspace_path);
                if path.is_absolute()
                    && path.is_dir()
                    && !path
                        .components()
                        .any(|p| matches!(p, std::path::Component::ParentDir))
                {
                    self.workspace_pending = true;
                    self.command(Command::OpenWorkspace(self.workspace_path.clone()));
                } else {
                    self.status = "Choose an existing absolute workspace directory".into();
                }
            }
            Message::Filter(value) => self.filter = value,
            Message::LoadModels
                if self.options.smoke.is_none() && self.ready && !self.catalog_pending =>
            {
                self.catalog_pending = true;
                self.status = "Loading real provider catalog (explicit request)…".into();
                self.command(Command::Catalog);
            }
            Message::Model(model)
                if self.options.smoke.is_none()
                    && self.ready
                    && self.selected.is_some()
                    && !self.model_pending =>
            {
                self.chosen_effort = model
                    .default_effort
                    .as_ref()
                    .and_then(|id| model.efforts.iter().find(|e| &e.id == id))
                    .cloned();
                self.chosen_model = Some(model);
                self.select_model();
            }
            Message::Effort(effort)
                if self.options.smoke.is_none() && self.ready && !self.model_pending =>
            {
                self.chosen_effort = Some(effort);
                self.select_model();
            }
            Message::Older
                if self.ready
                    && self.follow_ready
                    && self.transcript.has_more()
                    && !self.page_pending =>
            {
                if let Some(id) = self.selected.clone() {
                    self.page_pending = true;
                    self.follow_bottom = false;
                    self.command(Command::Page {
                        generation: self.generation,
                        request: SessionPageRequest {
                            address: self.address(&id),
                            through_seq: self.transcript.opening_cursor(),
                            before_seq: self.transcript.before(),
                            max_messages: Some(50),
                            turn_window: None,
                        },
                    });
                }
            }
            Message::Reload if self.ready => {
                if let Some(id) = self.selected.clone() {
                    self.choose(id);
                }
            }
            Message::Scrolled {
                generation,
                revision,
                viewport,
            } if generation == self.generation
                && revision.is_some()
                && revision == self.scroll_revision
                && self.transcript_active() =>
            {
                self.observe_scroll(
                    generation,
                    revision,
                    viewport.absolute_offset().y,
                    viewport.bounds().height,
                    viewport.relative_offset().y,
                    viewport.content_bounds().height,
                );
            }
            Message::ScrollApplied {
                generation,
                revision,
                feedback,
                y,
                viewport,
            } if generation == self.generation
                && revision.is_some()
                && revision == self.scroll_revision
                && feedback.is_some()
                && feedback == self.scroll_feedback
                && self.transcript_active() =>
            {
                if y.is_finite() && y >= 0.0 && viewport.is_finite() && viewport >= 0.0 {
                    self.offset = y;
                    self.viewport = viewport;
                    self.scroll_target = Some(y); // Feedback hint, never a required notification/ACK.
                }
            }
            Message::Bottom => {
                self.follow_bottom = true;
                self.invalidate_scroll();
                return self.position_transcript();
            }
            Message::Details(request) if self.composer_visible()
                && request.generation==self.generation
                && self.detail_opening==Some(request.prior_opening) => {
                let key=request.key;
                if key == u64::MAX {
                    if let Some(source)=self.transcript.partial() {
                        self.open_detail("Live assistant snapshot".into(),source,
                            Some("Frozen live display · not a durable record".into()),true);
                    }
                } else if let Some(row) = self.transcript.rows().iter().find(|r| r.key == key) {
                    let heading = self.transcript.activity(key).map_or_else(||row.role.clone(),|a|a.title);
                    let source=row.text.clone();
                    let metadata=Some(format!("Record #{} · timestamp {} · frozen display",row.key,row.time));
                    let assistant=self.transcript.assistant_record(key);
                    self.open_detail(heading,source,metadata,assistant);
                }
            }
            Message::CopyDetailSource(ticket) => {
                if let Some(source) = self.detail_copy_source(ticket) {
                    // Iced writes Standard clipboard without a success acknowledgement.
                    // Do not claim completion or change backend/reader/draft authority.
                    return iced::clipboard::write(source);
                }
            }
            Message::DetailMode(ticket,mode) if self.detail_visible() && ticket.generation==self.generation => {
                if let Some(detail)=&mut self.detail {detail.set_mode(ticket,mode);}
            }
            Message::HideDetails(ticket) if self.detail_visible()
                && ticket.generation==self.generation
                && self.detail.as_ref().and_then(|detail|detail.ticket)==Some(ticket) => self.detail = None,
            Message::Capabilities => {
                self.capabilities = !self.capabilities;
                self.info_expanded = 0;
            }
            Message::Close => {
                self.settings.close();
                self.exporter.dismiss();
                if self.stopped {
                    if self.last_stop.as_ref().is_some_and(|r|matches!(r,Ok(s) if s.exited&&!s.containment_unknown&&s.observed_descendants_remaining==0)){return iced::exit();}
                    self.close = Close::Failed(
                        "Backend stop result is uncertain; review before exiting".into(),
                    );
                } else if !self.ready {
                    self.close=Close::Confirm("Core startup is not ready; activity is unknown. Close cancels only this owned startup/backend.".into());
                } else if matches!(self.close, Close::None) {
                    self.close = Close::Inspecting;
                    self.command(Command::Inspect);
                }
            }
            Message::Stay => self.close = Close::None,
            Message::ConfirmClose => self.shutdown(),
            Message::ExitAnyway if self.stopped => return iced::exit(),
            _ => {}
        }
        if geometry_change {
            self.reflow_transcript(reader_anchor)
        } else if was_active != self.transcript_active() {
            self.invalidate_scroll();
            self.position_transcript()
        } else {
            Task::none()
        }
    }
    fn select_model(&mut self) {
        if let (Some(session), Some(model)) = (self.selected.clone(), self.chosen_model.clone()) {
            self.model_pending = true;
            self.command(Command::SelectModel {
                generation: self.generation,
                request: SelectModelRequest {
                    session_id: session,
                    selection: ModelSelection {
                        provider: model.provider,
                        model: model.id,
                        reasoning_effort: self.chosen_effort.as_ref().map(|e| e.id.clone()),
                    },
                },
            });
        }
    }
    fn receive(&mut self, event: Event) -> Task<Message> {
        let was_active = self.transcript_active();
        let projection_change = matches!(&event, Event::Selected { generation, .. } | Event::Page { generation, .. } if *generation == self.generation);
        let reader_anchor = if projection_change {
            self.reader_anchor()
        } else {
            None
        };
        let mut changed = false;
        match event {
            Event::Ready(version) => {
                self.ready = true;
                self.transport_epoch = worker::TRANSPORT_EPOCH;
                self.settings.adopt_epoch(self.transport_epoch);
                self.smoke_evidence.core_ready = true;
                if self.options.smoke.is_some() {
                    self.workspace_pending = true;
                    self.command(Command::OpenWorkspace(
                        self.options
                            .backend
                            .working_directory
                            .to_string_lossy()
                            .into_owned(),
                    ));
                }
                self.status = format!("Owned Core {version}; authenticated native transport ready");
            }
            Event::Core(PublicEvent::Codex(metadata)) => {
                self.settings.codex_observed(metadata);
                self.continue_codex_connection();
            }
            Event::Core(PublicEvent::Account(metadata))
                if self.ready && !self.stopped && !matches!(self.close, Close::Stopping) =>
            {
                self.account = format!(
                    "Account: {:?}{}",
                    metadata.status,
                    metadata
                        .phase
                        .map_or(String::new(), |phase| format!(" · {phase:?}"))
                )
            }
            Event::Core(PublicEvent::Warning(warning)) => {
                if self.ready
                    && matches!(
                        warning,
                        dsh_native_core::Warning::AccountSubscriptionUnavailable
                    )
                {
                    self.account = "Account metadata unavailable".into();
                }
                self.status = format!("Core warning: {warning:?}")
            }
            Event::Core(PublicEvent::Exited(result)) => {
                self.files.disconnect();
                self.management.disconnect();
                self.exporter.disconnect();
                self.settings.disconnect();
                self.account = "Account metadata unavailable (disconnected)".into();
                self.ready = false;
                self.root_ready = false;
                self.decisions.disconnect();
                self.status = format!(
                    "Owned Core exited; graceful={}, uncertain={}",
                    result.graceful, result.containment_unknown
                );
            }
            Event::Core(PublicEvent::Ready(_) | PublicEvent::Account(_)) => {}
            Event::FileStaged { ticket, outcome } => {
                let context = self.file_context();
                self.files.complete(&ticket, outcome, &context);
            }
            Event::FilePrompt {
                ticket,
                request_id,
                outcome,
            } => {
                let context = self.file_context();
                self.files
                    .prompt_complete(&ticket, &request_id, outcome, &context);
                if self.pending.get(&ticket.generation).is_some_and(|pending| {
                    pending.session == ticket.target
                        && pending.file_request_id.as_ref() == Some(&request_id)
                }) {
                    let pending = self.pending.remove(&ticket.generation).unwrap();
                    if outcome == worker::attachments::PromptOutcome::Accepted {
                        if ticket.generation == self.generation
                            && self.editor.text() == pending.text
                        {
                            self.editor = text_editor::Content::new();
                        }
                        if self.drafts.get(&pending.session) == Some(&pending.text) {
                            self.drafts.remove(&pending.session);
                        }
                    }
                    if ticket.generation == self.generation {
                        self.status=match outcome {worker::attachments::PromptOutcome::Accepted=>"Attachment prompt accepted into Host inbox; generation/history still come from Host events",worker::attachments::PromptOutcome::NotSent=>"Attachment prompt not admitted; file remains ready",worker::attachments::PromptOutcome::Unknown=>"Attachment Send outcome unknown; do not retry automatically"}.into();
                    }
                }
            }
            Event::Exported { receipt, outcome } => {
                self.exporter.finish(&receipt, outcome);
            }
            Event::Managed { submission, result } => {
                self.management.finish(&submission, result, false);
            }
            Event::Roster(result) => {
                self.roster_pending = false;
                match result {
                    Ok(roster) => {
                        if roster.items.len() > 4096 {
                            self.status = "Session list exceeds native 4096-row limit".into();
                            self.ready = false;
                        } else {
                            self.sessions = roster
                                .items
                                .into_iter()
                                .map(|s| (s.session_id.clone(), s))
                                .collect();
                            for frame in std::mem::take(&mut self.early_root) {
                                self.root(frame, None);
                            }
                            self.status =
                                "Real sessions loaded; select one or create a new session".into();
                        }
                    }
                    Err(error) => self.status = error,
                }
            }
            Event::Workspace(frame) => match frame {
                WorkspaceFollowFrame::Baseline { value } => {
                    self.workspaces = value.items;
                    if !self
                        .management
                        .baseline(value.archived_session_ids, value.pinned_session_ids)
                    {
                        self.status = "Registry snapshot malformed or exceeds native bound; management disabled".into();
                    }
                }
                WorkspaceFollowFrame::Upsert { workspace } => {
                    if let Some(existing) = self
                        .workspaces
                        .iter_mut()
                        .find(|w| w.workspace_id == workspace.workspace_id)
                    {
                        *existing = workspace;
                    } else if self.workspaces.len() < 2048 {
                        self.workspaces.push(workspace);
                    } else {
                        self.status = "Workspace list exceeds native 2048-row limit".into();
                    }
                }
                WorkspaceFollowFrame::Remove { workspace_id } => {
                    self.workspaces.retain(|w| w.workspace_id != workspace_id);
                    if self.workspace.as_ref() == Some(&workspace_id) {
                        self.workspace = None;
                    }
                }
                WorkspaceFollowFrame::Order { workspace_ids } => self.workspaces.sort_by_key(|w| {
                    workspace_ids
                        .iter()
                        .position(|id| id == &w.workspace_id)
                        .unwrap_or(usize::MAX)
                }),
                WorkspaceFollowFrame::Archived {
                    archived_session_ids,
                } => {
                    if !self.management.archives(archived_session_ids) {
                        self.status =
                            "Registry archive feed not authoritative; management disabled".into();
                    }
                }
                WorkspaceFollowFrame::Pinned { pinned_session_ids } => {
                    if !self.management.pins(pinned_session_ids) {
                        self.status =
                            "Registry pin feed not authoritative; management disabled".into();
                    }
                }
            },
            Event::Root {
                frame,
                decision_key,
            } => {
                if self.roster_pending && matches!(frame, RemoteEventFrame::Emit { .. }) {
                    if self.early_root.len() < 256 {
                        self.early_root.push(frame);
                    } else {
                        self.status =
                            "Roster event buffer overflow; close and restart native app".into();
                        self.ready = false;
                    }
                } else {
                    self.root(frame, decision_key);
                }
            }
            Event::Selected { generation, frame } if generation == self.generation => {
                let snapshot = matches!(&frame, SessionFollowFrame::Snapshot { .. });
                let title_change = match &frame {
                    SessionFollowFrame::Snapshot { projections, .. } => projections.values.get("title").map(|value| (projections.as_of_seq, value.clone())),
                    SessionFollowFrame::Event { event } if event.event_type == "session/title" => event.data.get("title").and_then(|value| i64::try_from(event.seq).ok().map(|seq| (seq, value.clone()))),
                    _ => None,
                };
                if let SessionFollowFrame::Snapshot {
                    projections,
                    cursor,
                    records,
                    ..
                } = &frame
                {
                    if self.options.smoke.is_some() {
                        self.smoke_evidence.snapshot_cursor = Some(*cursor);
                        self.smoke_evidence.record_count = records.len();
                        self.smoke_evidence.record_kinds = records
                            .iter()
                            .map(|record| match record {
                                HistoryRecord::Event { event } => {
                                    crate::smoke::kind(&event.event_type)
                                }
                            })
                            .collect();
                    }
                    self.projections = projections.values.clone();
                    if let (Some(id), Some(value)) = (
                        self.selected.clone(),
                        projections
                            .values
                            .get("modelSelection")
                            .and_then(|v| v.get("next")),
                    ) {
                        if let Ok(selection) =
                            serde_json::from_value::<ModelSelection>(value.clone())
                        {
                            self.model_selections.insert(id, selection);
                        }
                    }
                    self.follow_ready = true;
                    self.status = "Live real session; UTC timestamps; no generation started".into();
                }
                match self.transcript.apply(generation, frame) {
                    Ok(()) => {
                        if let Some((seq, value)) = title_change {
                            self.observe_title(seq, value);
                        }
                        changed = true;
                        if snapshot {
                            self.smoke_evidence.snapshot_valid = true;
                        }
                    }
                    Err(error) => {
                        self.files.invalidate();
                        self.follow_ready = false;
                        if snapshot {
                            self.smoke_evidence.snapshot_valid = false;
                        }
                        self.status = error.to_string();
                    }
                }
            }
            Event::Created(result) => {
                self.new_pending = false;
                match result {
                    Ok(value) => {
                        self.smoke_evidence.session_created = true;
                        self.choose(value.session_id);
                    }
                    Err(error) => self.status = error,
                }
            }
            Event::WorkspaceOpened(result) => {
                self.workspace_pending = false;
                match result {
                    Ok(value) => {
                        self.smoke_evidence.workspace_opened = true;
                        self.workspace = Some(value.workspace.workspace_id.clone());
                        if self.options.smoke.is_some() {
                            self.new_pending = true;
                            self.command(Command::Create {
                                workspace: self.workspace.clone(),
                                cwd: None,
                            });
                        }
                        if !self
                            .workspaces
                            .iter()
                            .any(|w| w.workspace_id == value.workspace.workspace_id)
                        {
                            self.workspaces.push(value.workspace);
                        }
                        self.status = "Workspace opened; no prompt submitted".into();
                    }
                    Err(error) => self.status = error,
                }
            }
            Event::Catalog(result) => {
                self.catalog_pending = false;
                match result {
                    Ok(catalog) => {
                        self.models = catalog
                            .groups
                            .into_iter()
                            .flat_map(|group| {
                                let provider = group.id;
                                let group_name = group.name;
                                group.models.into_iter().map(move |model| {
                                    let reasoning = model.reasoning;
                                    Model {
                                        provider: provider.clone(),
                                        id: model.id,
                                        label: format!("{} / {}", group_name, model.name),
                                        default_effort: reasoning
                                            .as_ref()
                                            .and_then(|r| r.default_effort.clone()),
                                        efforts: reasoning.map_or(vec![], |r| {
                                            r.efforts
                                                .into_iter()
                                                .map(|e| Effort {
                                                    id: e.id,
                                                    name: e.name,
                                                })
                                                .collect()
                                        }),
                                    }
                                })
                            })
                            .take(4096)
                            .collect();
                        self.status = format!(
                            "{} routable models; {} provider failures (details redacted); existing session model unchanged",
                            self.models.len(),
                            catalog.failures.len()
                        );
                    }
                    Err(error) => self.status = error,
                }
            }
            Event::Model { generation, result } if generation == self.generation => {
                self.model_pending = false;
                match result {
                    Ok(value) => {
                        if let Some(id) = &self.selected {
                            self.model_selections.insert(id.clone(), value.selected);
                        }
                        self.status = "Model selection accepted by owned Host".into();
                    }
                    Err(error) => {
                        self.chosen_model = None;
                        self.chosen_effort = None;
                        self.status = error;
                    }
                }
            }
            Event::Prompt { generation, result }
                if self
                    .pending
                    .get(&generation)
                    .is_some_and(|pending| pending.file_request_id.is_none()) =>
            {
                if let Some(pending) = self.pending.remove(&generation) {
                    match result {
                        Ok(accepted) if accepted.accepted => {
                            if generation == self.generation && self.editor.text() == pending.text {
                                self.editor = text_editor::Content::new();
                            }
                            if self.drafts.get(&pending.session) == Some(&pending.text) {
                                self.drafts.remove(&pending.session);
                            }
                            self.status="Prompt accepted; transcript and running state come from Host events".into();
                        }
                        Ok(_) => self.status = "Host did not accept prompt; draft retained".into(),
                        Err(error) => self.status = error,
                    }
                }
            }
            Event::Cancel { generation, result } if generation == self.generation => {
                self.cancel_pending = false;
                self.status = match result {
                    Ok(value) if value.accepted => {
                        "Cancellation accepted; awaiting actual Host status".into()
                    }
                    Ok(_) => "No cancellable generation was accepted".into(),
                    Err(error) => error,
                };
            }
            Event::Page { generation, result } if generation == self.generation => {
                self.page_pending = false;
                match result {
                    Ok(page) => {
                        if let Err(error) = self.transcript.page(generation, page) {
                            self.status = error.to_string();
                        }
                        // Errors can still change the actual projection; restore from it below.
                    }
                    Err(error) => self.status = error,
                }
            }
            Event::FollowError { generation, error } if generation == self.generation => {
                self.files.invalidate();
                self.files.invalidate();
                self.follow_ready = false;
                self.status = error;
            }
            Event::Inspection(result) if matches!(self.close, Close::Inspecting) => match result {
                Ok(inspection) if inspection.is_known_idle() => self.shutdown(),
                Ok(Inspection {
                    active_tasks,
                    scheduled_tasks,
                    unknown,
                }) => {
                    self.close = Close::Confirm(format!(
                        "Owned backend: {active_tasks} active tasks, {scheduled_tasks} scheduled tasks; inspection unknown={unknown}. Closing cancels/tears down only this owned backend."
                    ))
                }
                Err(error) => {
                    self.close = Close::Confirm(format!(
                        "{error}. Idle state is unknown. Close and stop only the owned backend?"
                    ))
                }
            },
            Event::DecisionResult { submission, result }
                if submission.key.epoch == self.transport_epoch =>
            {
                let acknowledged = result.is_ok();
                if self
                    .decisions
                    .acknowledge(&submission.key, submission.attempt, result)
                {
                    self.status=if acknowledged{"Reply RPC acknowledged; no tool execution or approval outcome inferred."}else{"Reply not acknowledged; request retained. Any submitted outcome may be indeterminate; retry only explicitly."}.into();
                }
            }
            Event::CodexCompleted { ticket, outcome } => {
                self.settings.codex_completed(ticket, outcome);
                self.continue_codex_connection();
            }
            Event::PluginsLoaded { ticket, result } => self.settings.plugins_loaded(ticket, result),
            Event::PluginSaved { ticket, result } => self.settings.plugin_saved(ticket, result),
            Event::KeyMetadata { ticket, result } => self.settings.metadata(ticket, result),
            Event::KeySaved { ticket, result } => self.settings.saved(ticket, result),
            Event::Fault(error) => {
                self.files.disconnect();
                self.management.disconnect();
                self.exporter.disconnect();
                self.settings.disconnect();
                self.account = "Account metadata unavailable (disconnected)".into();
                self.ready = false;
                self.root_ready = false;
                self.decisions.disconnect();
                self.status = error;
            }
            Event::NoBackendStarted => {
                self.files.disconnect();
                self.management.disconnect();
                self.exporter.disconnect();
                self.settings.disconnect();
                self.account = "Account metadata unavailable (disconnected)".into();
                self.ready = false;
                self.stopped = true;
                self.status = "No owned backend was started".into();
                if matches!(self.close, Close::Stopping) {
                    return iced::exit();
                }
            }
            Event::Stopped(result) => {
                self.files.disconnect();
                self.management.disconnect();
                self.exporter.disconnect();
                self.settings.disconnect();
                self.account = "Account metadata unavailable (disconnected)".into();
                self.transport_epoch += 1;
                self.decisions.disconnect();
                if let Some(smoke) = &self.options.smoke {
                    if self
                        .smoke_evidence
                        .write(
                            &smoke.report,
                            self.sessions.len(),
                            self.workspaces.len(),
                            self.options.scale,
                            &result,
                        )
                        .is_err()
                    {
                        eprintln!("Native smoke report unavailable");
                    }
                    return iced::exit();
                }
                self.ready = false;
                self.root_ready = false;
                self.stopped = true;
                self.decisions.disconnect();
                self.last_stop = Some(result.clone());
                if matches!(self.close, Close::Stopping) {
                    match result {
                        Ok(stop)
                            if stop.exited
                                && !stop.containment_unknown
                                && stop.observed_descendants_remaining == 0 =>
                        {
                            return iced::exit();
                        }
                        Ok(stop) => {
                            self.close = Close::Failed(format!(
                                "Stop returned: exited={}, graceful={}, uncertain={}, observed remaining={}. No unowned processes will be signalled.",
                                stop.exited,
                                stop.graceful,
                                stop.containment_unknown,
                                stop.observed_descendants_remaining
                            ))
                        }
                        Err(error) => self.close = Close::Failed(error),
                    }
                } else {
                    self.status = match result {
                        Ok(stop) => format!(
                            "Owned backend stopped; graceful={}, uncertain={}",
                            stop.graceful, stop.containment_unknown
                        ),
                        Err(error) => error,
                    };
                }
            }
            _ => {}
        }
        let screenshot = self.maybe_screenshot();
        if projection_change {
            Task::batch([self.reflow_transcript(reader_anchor), screenshot])
        } else if was_active != self.transcript_active() {
            self.invalidate_scroll();
            Task::batch([self.position_transcript(), screenshot])
        } else if changed && self.follow_bottom {
            Task::batch([self.position_transcript(), screenshot])
        } else {
            screenshot
        }
    }
    fn maybe_screenshot(&mut self) -> Task<Message> {
        if !self.smoke_evidence.screenshot_requested
            || self.smoke_evidence.screenshot_scheduled
            || !self.smoke_evidence.snapshot_valid
            || !self.follow_ready
            || !matches!(self.close, Close::None)
        {
            return Task::none();
        }
        let Some(id) = self.own_window else {
            return Task::none();
        };
        self.smoke_evidence.screenshot_scheduled = true;
        Task::perform(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(750)).await;
            },
            move |()| Message::CaptureOwnWindow(id),
        )
    }
    fn root(&mut self, frame: RemoteEventFrame, decision_key: Option<crate::interactions::Key>) {
        match frame {
            RemoteEventFrame::Ready { .. } => self.root_ready = true,
            RemoteEventFrame::Emit { event, args } => match event.as_str() {
                "api-session/added" => {
                    if let Some(value) = args.first() {
                        match serde_json::from_value::<SessionSummary>(value.clone()) {
                            Ok(summary) => {
                                if self.sessions.len() < 4096
                                    || self.sessions.contains_key(&summary.session_id)
                                {
                                    self.sessions.insert(summary.session_id.clone(), summary);
                                }
                            }
                            Err(_) => self.status = "Invalid typed session-added event".into(),
                        }
                    }
                }
                "api-session/removed" => {
                    if let Some(id) = args
                        .first()
                        .and_then(|v| v.as_str())
                        .and_then(|v| SessionId::new(v).ok())
                    {
                        self.sessions.remove(&id);
                        self.title_updates.remove(&id);
                        if self.selected.as_ref() == Some(&id) {
                            self.files.invalidate();
                            self.follow_ready = false;
                            self.status = "Selected session removed by Host".into();
                        }
                    }
                }
                "api-session/status" => {
                    if let (Some(id), Some(running)) = (
                        args.first()
                            .and_then(|v| v.as_str())
                            .and_then(|v| SessionId::new(v).ok()),
                        args.get(1).and_then(|v| v.as_bool()),
                    ) {
                        if let Some(session) = self.sessions.get_mut(&id) {
                            session.running = running;
                        }
                    }
                }
                "api-session/activity" => {
                    if let (Some(id), Some(updated)) = (
                        args.first()
                            .and_then(|v| v.as_str())
                            .and_then(|v| SessionId::new(v).ok()),
                        args.get(1).and_then(|v| v.as_i64()),
                    ) {
                        if let Some(session) = self.sessions.get_mut(&id) {
                            session.updated_at = updated;
                        }
                    }
                }
                "api-session/error" => {
                    self.status =
                        "Host reported session failure; view durable transcript for details".into()
                }
                _ => {}
            },
            RemoteEventFrame::Waterfall {
                event,
                event_id,
                agent_id,
                request,
            } => {
                let Some(key) = decision_key
                    .filter(|key| key.epoch == self.transport_epoch && key.event_id == event_id)
                else {
                    self.status =
                        "Decision has no matching native worker ticket; replies disabled".into();
                    return;
                };
                if let Err(error) = self.decisions.add_owned(key, agent_id, event, request) {
                    self.status = error.into();
                }
            }
            RemoteEventFrame::Cancel { event_id } => {
                self.decisions.cancel(&event_id);
            }
        }
    }
    fn management_context(&self) -> crate::management::Context {
        let mut context = self.operator_context();
        context.allowed =
            context.allowed && !self.exporter.is_open() && self.exporter.pending().is_none();
        context
    }
    fn export_context(&self) -> crate::exporter::Context {
        let context = self.operator_context();
        crate::exporter::Context {
            view: context.view,
            allowed: context.allowed && !self.management.is_open() && !self.management.pending(),
        }
    }
    fn operator_context(&self) -> crate::management::Context {
        let observed = self.selected.as_ref().and_then(|id| self.sessions.get(id));
        crate::management::Context {
            view: self
                .selected
                .as_ref()
                .map(|target| crate::management::View {
                    epoch: self.transport_epoch,
                    generation: self.generation,
                    target: target.clone(),
                }),
            allowed: self.options.smoke.is_none()
                && self.ready
                && self.root_ready
                && !self.roster_pending
                && self.management.registry.ready()
                && observed.is_some_and(|s| s.origin.is_none())
                && !self.settings.is_open()
                && !self.decisions.is_open()
                && matches!(self.close, Close::None),
        }
    }
    fn decision_replies_enabled(&self) -> bool {
        self.options.smoke.is_none()
            && self.ready
            && self.root_ready
            && matches!(self.close, Close::None)
    }
    fn continue_codex_connection(&mut self) {
        let enabled = self.settings_enabled();
        if let Some(effect) = self.settings.codex_continue(enabled) {
            self.command(Command::Codex(effect));
        }
    }
    fn settings_enabled(&self) -> bool {
        self.exporter.pending().is_none()
            && !self.exporter.is_open()
            && self.options.smoke.is_none()
            && self.ready
            && !self.stopped
            && matches!(self.close, Close::None)
    }
    pub fn view(&self) -> Element<'_, Message> {
        // Modal priority and worker fences stay independent of presentation.
        if !matches!(self.close, Close::None) {
            return self.close_view();
        }
        if self.settings_modal_visible() {
            let size = settings_modal_size(self.layout_size());
            let card = self
                .settings
                .view_sized(self.settings_enabled(), size.width, size.height)
                .map(Message::Settings);
            return settings_overlay(
                self.presentation_shell(true),
                card,
                Message::Settings(crate::settings::Action::Close),
            );
        }
        if let Some(ticket) = self.exporter.ticket() {
            return settings_overlay(
                self.presentation_shell(false),
                self.export_view(),
                Message::Export(crate::exporter::Action::Close(ticket)),
            );
        }
        if let Some(ticket) = self.management.ticket() {
            return settings_overlay(
                self.presentation_shell(false),
                self.management_view(),
                Message::Management(crate::management::Action::Close(ticket)),
            );
        }
        if let Some(dialog) = self.decisions.dialog(self.decision_replies_enabled()) {
            return dialog.map(Message::Interaction);
        }
        self.presentation_shell(self.composer_visible())
    }
    fn export_view(&self) -> Element<'_, Message> {
        use crate::exporter::{Action, Notice};
        let ticket = self.exporter.ticket().expect("visible export panel");
        let target = self
            .sessions
            .get(&ticket.target)
            .map(|session| self.session_title(session))
            .unwrap_or_else(|| "Unavailable session".into());
        let enabled = self.export_context().allowed && self.exporter.pending().is_none();
        let mut body = column![
            row![label("Export session ZIP", TEXT), Space::new().width(Length::Fill),
                button("Close").on_press(Message::Export(Action::Close(ticket.clone())))
                    .style(design::button_style).padding([8, 12])].spacing(12),
            label(target, ACCENT),
            label("Sensitive, unredacted archive: conversation log plus referenced root images and files. It may contain credentials or private paths. Descendant sessions are excluded.", DANGER),
            label("Before downloading, the Host flushes the live log. This can write durable data; cancellation does not undo it. Trusted profile flush hooks may have additional effects.", TEXT),
            label("Enter an explicit absolute NEW .zip destination. Existing files and symlinks are refused. Download limit: 4 MiB. ZIP structure is checked, not CRC, decompressed content or session identity; nothing is extracted.", MUTED),
        ].spacing(16).padding(24);
        match self.exporter.notice() {
            Notice::None => {},
            Notice::ReviewChanged => body = body.push(label("Session or review changed. Review the current destination again.", MUTED)),
            Notice::InvalidPath => body = body.push(label("Use an absolute new .zip path, at most 4096 UTF-8 bytes, without control characters, dot components or repeated separators. Invalid input is cleared or refused; it is never normalized.", DANGER)),
            Notice::Pending => body = body.push(label("Export admitted. Close hides this panel; it does not cancel or undo the operation. No second export is admitted until its receipt settles.", ACCENT)),
            Notice::Saved { bytes } => body = body.push(label(format!("Saved {bytes} bytes with private mode 0600; file and parent directory sync succeeded. External pathname replacement is not ruled out."), ACCENT)),
            Notice::NotSent => body = body.push(label("Export not sent; no destination file was opened. Nothing was automatically retried.", DANGER)),
            Notice::DownloadFailed => body = body.push(label("Download failed or was refused. No destination file was opened, but the Host may already have flushed the live log. No automatic retry.", DANGER)),
            Notice::NotCreated => body = body.push(label("The destination was refused before creating a file. Check the reviewed location and existing files; no overwrite or automatic retry occurred.", DANGER)),
            Notice::MayRemain => body = body.push(label("Save outcome uncertain. A private empty, partial or complete file may remain at the reviewed location. Inspect it yourself before retrying; nothing is deleted or automatically retried.", DANGER)),
        }
        if let Some(review) = self.exporter.review() {
            body = body
                .push(label(
                    "Confirm the destination and both warnings above",
                    TEXT,
                ))
                .push(label(self.exporter.path(), ACCENT))
                .push(
                    row![
                        button("Back")
                            .on_press_maybe(enabled.then_some(Message::Export(Action::Edit {
                                ticket: ticket.clone(),
                                path: self.exporter.path().to_owned(),
                            })))
                            .style(design::button_style)
                            .padding([10, 14]),
                        button("Confirm export and save")
                            .on_press_maybe(
                                enabled.then_some(Message::Export(Action::Confirm(review.clone())))
                            )
                            .style(design::button_style)
                            .padding([10, 14]),
                    ]
                    .spacing(12),
                );
        } else {
            let edit_ticket = ticket.clone();
            body = body
                .push(
                    text_input("/absolute/directory/new-session.zip", self.exporter.path())
                        .on_input_maybe(enabled.then_some(move |path| {
                            Message::Export(Action::Edit {
                                ticket: edit_ticket.clone(),
                                path,
                            })
                        }))
                        .font(FONT)
                        .padding(12),
                )
                .push(
                    button("Review export")
                        .on_press_maybe(enabled.then_some(Message::Export(Action::Review(ticket))))
                        .style(design::button_style)
                        .padding([10, 14]),
                );
        }
        body = body.push(label("An admitted filesystem save is synchronous and cannot be cancelled. Closing the application waits for owned filesystem work; an unhealthy filesystem may delay shutdown. Close never promises rollback or secure erasure.", MUTED));
        let size = settings_modal_size(self.layout_size());
        container(scrollable(body).height(Length::Fill))
            .width(size.width)
            .height(size.height)
            .style(|_| panel(SURFACE))
            .into()
    }
    fn management_view(&self) -> Element<'_, Message> {
        use crate::management::{Action, Notice, Operation};
        let ticket = self.management.ticket().expect("visible management panel");
        let target = self
            .sessions
            .get(&ticket.target)
            .map(|session| self.session_title(session))
            .unwrap_or_else(|| "Unavailable session".into());
        let enabled = self.management_context().allowed;
        let archived = self.management.registry.archived(&ticket.target);
        let pinned = self.management.registry.pinned(&ticket.target);
        let mut body = column![
            label("Session management", TEXT).size(25),
            label(short(&target, 100), ACCENT).size(15),
            label(
                "Registry-global pin/archive metadata. Archive keeps history; it is not deletion.",
                MUTED
            ),
            button(label("Back · no automatic write", TEXT))
                .style(design::button_style)
                .on_press(Message::Management(Action::Close(ticket.clone()))),
            label(
                format!(
                    "Live registry: archived={archived} · pinned={pinned} · {} global pins",
                    self.management.registry.ordered_pins().len()
                ),
                TEXT
            ),
        ]
        .spacing(14);
        let notice = match self.management.notice() {
            Notice::None => "Choose an operation, then explicitly confirm.",
            Notice::ReviewChanged => "Live registry changed. Review again before confirming.",
            Notice::Pending => {
                "An owned operation is pending. Closing does not cancel or roll back its write."
            }
            Notice::Confirmed => {
                "Host acknowledged the operation. Membership/order below comes only from the live stream."
            }
            Notice::NotSent => "Worker queue rejected the operation: not sent. No automatic retry.",
            Notice::Indeterminate => {
                "Operation not confirmed: Host refusal/failure or uncertain write. Observe the live registry; never assume rollback. No automatic retry."
            }
        };
        body = body.push(label(notice, MUTED));
        if let Some(review) = self.management.review() {
            let warning = match review.operation {
                Operation::Archive => {
                    "Archive and unpin this session without force-stop? The Host refuses active work. History and workspace accounting remain; restore does not repin."
                }
                Operation::Restore => {
                    "Restore this session to recent conversations? Its former pin is not restored."
                }
                Operation::Pin => {
                    "Pin this session in the registry-global, most-recently-pinned-first order? Existing pins are not reordered."
                }
                Operation::Unpin => "Remove this global pin? Session history is unchanged.",
            };
            body = body.push(label(warning, DANGER)).push(
                button(text(format!("Confirm {}", review.operation.label())).font(FONT))
                    .on_press_maybe(
                        enabled
                            .then_some(Message::Management(Action::Confirm(review.ticket.clone()))),
                    )
                    .style(design::primary)
                    .padding([10, 16]),
            );
        } else {
            let operations: Vec<_> = if archived {
                vec![Operation::Restore]
            } else {
                vec![
                    if pinned {
                        Operation::Unpin
                    } else {
                        Operation::Pin
                    },
                    Operation::Archive,
                ]
            };
            for operation in operations {
                body = body.push(
                    button(text(format!("Review {}", operation.label())).font(FONT))
                        .on_press_maybe((enabled && self.management.allows(operation)).then_some(
                            Message::Management(Action::Review {
                                ticket: ticket.clone(),
                                operation,
                            }),
                        ))
                        .style(design::button_style)
                        .padding([10, 14]),
                );
            }
        }
        body = body.push(
            button(text("Export session ZIP…").font(FONT))
                .on_press_maybe(
                    (enabled && !self.management.pending() && self.exporter.pending().is_none())
                        .then_some(Message::ExportSession(ticket.clone())),
                )
                .style(design::button_style)
                .padding([10, 14]),
        );
        body = body.push(label("No forced stop, session deletion, rename, fork, provider/model call or automatic retry. A matching ACK does not replace newer stream state.", MUTED));
        let size = settings_modal_size(self.layout_size());
        container(scrollable(body).height(Length::Fill))
            .padding(22)
            .width(size.width)
            .height(size.height)
            .style(|_| panel(SURFACE))
            .into()
    }
    fn settings_modal_visible(&self) -> bool {
        matches!(self.close, Close::None) && self.settings.is_open()
    }
    fn presentation_shell(&self, include_composer: bool) -> Element<'_, Message> {
        let main: Element<'_, Message> = if self.sidebar_sheet && !include_composer {
            self.navigation_sheet()
        } else {
            let body = if self.conversation_menu {
                self.conversation_menu_view()
            } else if self.capabilities {
                self.capabilities_view()
            } else if let Some(detail) = &self.detail {
                self.detail_view(detail)
            } else {
                self.transcript_view()
            };
            let mut content = column![self.header(), body].spacing(4).height(Length::Fill);
            if include_composer {
                content = content.push(self.composer());
            }
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| panel(BASE))
                .into()
        };
        row![self.sidebar(), main].height(Length::Fill).into()
    }
    fn panel_stop_visible(&self) -> bool {
        self.layout_controls_enabled() && !self.composer_visible() && self.selected.is_some()
    }
    fn stop_button(&self) -> Element<'_, Message> {
        // Keep cancellation reachable even when the displayed running state lags the Host.
        container(
            button(
                label(
                    if self.cancel_pending {
                        "Stopping…"
                    } else {
                        "Stop"
                    },
                    DANGER,
                )
                .size(12),
            )
            .on_press_maybe(self.stop_allowed().then_some(Message::Stop))
            .style(design::danger_button)
            .padding([8, 12]),
        )
        .id("native-stop-control")
        .into()
    }
    fn settings_entry(&self, compact: bool) -> Element<'_, Message> {
        // The mutually exclusive sidebar/rail branches share exactly one entry.
        container(
            button(label("Settings", MUTED).size(if compact { 10 } else { 12 }))
                .on_press(Message::Settings(crate::settings::Action::Open))
                .style(design::button_style)
                .padding(if compact { [12, 2] } else { [10, 10] })
                .width(Length::Fill),
        )
        .id("native-settings-entry")
        .width(Length::Fill)
        .into()
    }
    fn conversation_menu_view(&self) -> Element<'_, Message> {
        let context = self.management_context();
        let manage = context
            .view
            .filter(|_| context.allowed)
            .map(|view| Message::Management(crate::management::Action::Open(view)));
        let content = column![
            label("More options", TEXT).size(22),
            label("CONVERSATION", MUTED).size(11),
            button(label("Manage & export…", TEXT))
                .on_press_maybe(manage)
                .style(design::button_style)
                .padding([12, 14])
                .width(Length::Fill),
            label("APPLICATION", MUTED).size(11),
            button(label("Information & diagnostics", TEXT))
                .on_press(Message::Capabilities)
                .style(design::button_style)
                .padding([12, 14])
                .width(Length::Fill),
            button(label("Close native Harness…", MUTED))
                .on_press(Message::Close)
                .style(design::button_style)
                .padding([12, 14])
                .width(Length::Fill)
        ]
        .spacing(14);
        container(
            scrollable(container(content).max_width(440).width(Length::Fill)).height(Length::Fill),
        )
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
    fn header(&self) -> Element<'_, Message> {
        let heading = self
            .selected
            .as_ref()
            .and_then(|id| self.sessions.get(id))
            .map(|session| self.session_title(session))
            .unwrap_or_else(|| "DeepSeek Harness".into());
        let running = self
            .selected
            .as_ref()
            .and_then(|id| self.sessions.get(id))
            .is_some_and(|s| s.running);
        let title_limit = if self.layout_size().width < 700.0 {
            18
        } else if self.compact() {
            30
        } else {
            64
        };
        let mut identity = row![
            label(short(&heading, title_limit), TEXT)
                .size(18)
                .width(Length::Fill)
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);
        if running {
            identity = identity.push(label("Running", design::FOAM).size(11));
        }
        let mut actions = row![identity].spacing(6).align_y(iced::Alignment::Center);
        if self.panel_stop_visible() {
            actions = actions.push(self.stop_button());
        }
        actions = actions.push(
            button(
                label(
                    if self.conversation_menu {
                        "Back"
                    } else {
                        "More ▾"
                    },
                    MUTED,
                )
                .size(12),
            )
            .on_press(Message::ToggleConversationMenu)
            .style(design::button_style)
            .padding([8, 10]),
        );
        let mut header = column![actions].spacing(6);
        if !routine_status(&self.status) {
            header = header.push(label(&self.status, MUTED).size(11));
        }
        if self.decisions.len() > 0 {
            header = header.push(
                scrollable(
                    self.decisions
                        .cards(self.decision_replies_enabled())
                        .map(Message::Interaction),
                )
                .height(94),
            );
        }
        container(header)
            .id("native-header")
            .padding([14, 20])
            .width(Length::Fill)
            .into()
    }
    fn file_controls(&self) -> Element<'_, Message> {
        let context = self.file_context();
        let mut body = Column::new().spacing(5);
        if let Some((stamp, path)) = self.files.editor() {
            body = body.push(
                row![
                    text_input("Absolute file path · up to 4 MiB", path)
                        .on_input(move |path| Message::FilePath(stamp, path))
                        .style(design::input_style)
                        .padding(7)
                        .size(12)
                        .width(Length::Fill),
                    button(label("Upload", TEXT).size(12))
                        .on_press_maybe(
                            (context.allowed && !path.is_empty())
                                .then_some(Message::FileUpload(stamp))
                        )
                        .style(design::button_style)
                        .padding([6, 9]),
                    button(label("Cancel", MUTED).size(12))
                        .on_press_maybe(
                            self.file_local_context()
                                .allowed
                                .then_some(Message::FileCancel(stamp))
                        )
                        .style(design::button_style)
                        .padding([6, 9]),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            );
            body = body.push(label(attachment_draft::UPLOAD_CONSENT, MUTED).size(10));
        } else if let Some(ticket) = self.files.ticket() {
            let title = if let Some(metadata) = self.files.metadata() {
                format!("File · {} · {} bytes", metadata.name, metadata.bytes)
            } else if self.files.reading() {
                "Reading / uploading file…".into()
            } else {
                "File was not staged".into()
            };
            body = body.push(
                row![
                    label(title, MUTED).size(12).width(Length::Fill),
                    button(
                        label(
                            if self.files.reading() {
                                "Cancel"
                            } else {
                                "Remove"
                            },
                            MUTED
                        )
                        .size(12)
                    )
                    .on_press_maybe(
                        self.file_local_context()
                            .allowed
                            .then_some(Message::FileRemove(ticket.clone()))
                    )
                    .style(design::button_style)
                    .padding([6, 9])
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            );
        } else if self.files.pending() || self.files.prompt_pending() {
            body = body.push(label("Previous file operation is finishing…", MUTED).size(11));
        }
        if let Some(notice) = self.files.notice() {
            body = body.push(label(notice, DANGER).size(11));
        }
        container(body)
            .id("native-file-draft")
            .width(Length::Fill)
            .into()
    }
    fn composer(&self) -> Element<'_, Message> {
        let allowed = self.send_allowed() && self.composer_visible();
        let small = self.layout_size().height < 620.0;
        let editor = text_editor(&self.editor)
            .placeholder("Message DeepSeek Harness…")
            .on_action(Message::Editor)
            .height(if small { 70 } else { 96 })
            .padding(12)
            .font(FONT)
            .size(15)
            .style(design::editor)
            .key_binding(move |event| composer_binding(event, allowed));
        let models: Element<'_, Message> = if self.models.is_empty() {
            row![
                label("Session model unchanged", MUTED)
                    .size(11)
                    .width(Length::Fill),
                button(
                    label(
                        if self.catalog_pending {
                            "Loading…"
                        } else {
                            "Load models"
                        },
                        MUTED
                    )
                    .size(12)
                )
                .on_press_maybe(
                    (self.options.smoke.is_none() && self.ready && !self.catalog_pending)
                        .then_some(Message::LoadModels)
                )
                .style(design::button_style)
                .padding([6, 10])
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            let picker = pick_list(
                self.models.as_slice(),
                self.chosen_model.as_ref(),
                Message::Model,
            )
            .placeholder("Existing session model")
            .width(Length::Fill)
            .padding(7)
            .text_size(12)
            .style(design::picker_style);
            let refresh = button(label("Reload models", MUTED).size(11))
                .on_press_maybe(
                    (self.options.smoke.is_none() && self.ready && !self.catalog_pending)
                        .then_some(Message::LoadModels),
                )
                .style(design::button_style)
                .padding([6, 9]);
            row![picker, refresh]
                .spacing(6)
                .align_y(iced::Alignment::Center)
                .into()
        };
        let models = container(models)
            .id("native-model-controls")
            .width(Length::Fill);
        let sending = self
            .pending
            .values()
            .any(|p| Some(&p.session) == self.selected.as_ref());
        let mut inner = column![editor].spacing(4);
        if self.files.editor_open()
            || self.files.ticket().is_some()
            || self.files.notice().is_some()
            || self.files.pending()
            || self.files.prompt_pending()
        {
            inner = inner.push(self.file_controls());
        }
        inner = inner.push(models);
        // A separate reasoning row cannot push Send/Stop outside a small viewport.
        if let Some(model) = &self.chosen_model {
            if !model.efforts.is_empty() {
                inner = inner.push(
                    container(
                        row![
                            label("Reasoning", MUTED).size(11),
                            pick_list(
                                model.efforts.as_slice(),
                                self.chosen_effort.as_ref(),
                                Message::Effort
                            )
                            .placeholder("Default")
                            .width(Length::Fill)
                            .padding(6)
                            .text_size(12)
                            .style(design::picker_style)
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                    )
                    .id("native-reasoning-control")
                    .width(Length::Fill),
                );
            }
        }
        inner = inner.push(
            row![
                label(
                    if self.options.smoke.is_some() {
                        "Keyless smoke · sending disabled"
                    } else if self.compact() {
                        "Ctrl+Enter · send"
                    } else {
                        "Ctrl+Enter to send · Enter for a new line"
                    },
                    MUTED
                )
                .size(10)
                .width(Length::Fill),
                container(
                    button(label("+ File path", MUTED).size(11))
                        .on_press_maybe(
                            (self.file_context().allowed
                                && !self.files.editor_open()
                                && self.files.ticket().is_none()
                                && !self.files.pending()
                                && !self.files.prompt_pending())
                            .then_some(Message::FileOpen)
                        )
                        .style(design::button_style)
                        .padding([6, 9])
                )
                .id("native-file-open"),
                self.stop_button(),
                container(
                    button(
                        text(if sending { "Sending…" } else { "Send ↑" })
                            .font(FONT)
                            .size(13)
                    )
                    .on_press_maybe(allowed.then_some(Message::Send))
                    .style(design::primary_button)
                    .padding([8, 14])
                )
                .id("native-send-control")
            ]
            .spacing(7)
            .align_y(iced::Alignment::Center),
        );
        if !self.transcript.safe_to_send() {
            inner = inner.push(
                label(
                    "Read-only: transcript needs resynchronization before sending.",
                    DANGER,
                )
                .size(11),
            );
        }
        let card = container(inner)
            .padding(10)
            .width(Length::Fill)
            .style(|_| design::card(SURFACE, 20.0));
        container(container(card).max_width(design::CHAT).width(Length::Fill))
            .center_x(Length::Fill)
            .padding([10, 20])
            .into()
    }
    fn sidebar(&self) -> Element<'_, Message> {
        let width = self.sidebar_motion.value();
        if self.compact() || width < 185.0 {
            let rail = column![
                brand(),
                button(label("Chats", MUTED).size(11))
                    .on_press(Message::ToggleSidebar)
                    .style(design::button_style)
                    .padding([12, 4])
                    .width(Length::Fill),
                button(label("+ New", TEXT).size(11))
                    .on_press_maybe(self.can_create().then_some(Message::NewSession))
                    .style(design::button_style)
                    .padding([12, 4])
                    .width(Length::Fill),
                Space::new().height(Length::Fill),
                self.settings_entry(true)
            ]
            .spacing(8)
            .align_x(iced::Alignment::Center);
            return container(rail)
                .padding([18, 8])
                .width(width)
                .height(Length::Fill)
                .clip(true)
                .style(|_| panel(SURFACE))
                .into();
        }
        let navigation = column![
            row![
                brand(),
                label("Harness", TEXT).size(17).width(Length::Fill),
                button(label("‹", MUTED).size(22))
                    .on_press(Message::ToggleSidebar)
                    .style(design::button_style)
                    .padding([3, 8])
            ]
            .spacing(9)
            .align_y(iced::Alignment::Center),
            button(
                row![
                    label("+", TEXT).size(19),
                    label(
                        if self.new_pending {
                            "Creating…"
                        } else {
                            "New conversation"
                        },
                        TEXT
                    )
                    .size(13)
                ]
                .spacing(12)
            )
            .on_press_maybe(self.can_create().then_some(Message::NewSession))
            .style(design::button_style)
            .padding([11, 10])
            .width(Length::Fill),
            self.session_navigation(),
            button(
                label(
                    if self.workspace_controls {
                        "⌄  Workspaces"
                    } else {
                        "›  Workspaces"
                    },
                    MUTED
                )
                .size(12)
            )
            .on_press(Message::WorkspaceControls)
            .style(design::button_style)
            .padding([8, 10])
            .width(Length::Fill),
            self.workspace_navigation(),
            self.settings_entry(false)
        ]
        .spacing(10);
        container(navigation)
            .padding([18, 14])
            .width(width)
            .height(Length::Fill)
            .clip(true)
            .style(|_| panel(SURFACE))
            .into()
    }
    fn can_create(&self) -> bool {
        self.options.smoke.is_none() && self.ready && !self.new_pending
    }
    fn observe_title(&mut self, seq: i64, value: serde_json::Value) {
        let Some(id) = self.selected.as_ref() else { return; };
        if seq < -1 || !self.sessions.contains_key(id)
            || (self.title_updates.len() >= 4096 && !self.title_updates.contains_key(id))
            || self.title_updates.get(id).is_some_and(|(old, _)| *old > seq) {
            return;
        }
        let value = match value {
            serde_json::Value::Null => serde_json::Value::Null,
            serde_json::Value::String(value) if !value.trim().is_empty() => serde_json::Value::String(reducer::bounded(value.trim(), 1024)),
            _ => return,
        };
        self.title_updates.insert(id.clone(), (seq, value));
    }
    fn session_title(&self, session: &SessionSummary) -> String {
        let value = self.title_updates.get(&session.session_id).filter(|(seq, _)| {
            session.projections.as_ref().is_none_or(|p| p.as_of_seq <= *seq)
        });
        value.map_or_else(|| title(session), |(_, value)| projected_title(session, Some(value)))
    }
    fn navigation_sessions(&self) -> Vec<&SessionSummary> {
        let filter = self.filter.to_lowercase();
        let ids = self
            .workspace
            .as_ref()
            .and_then(|id| self.workspaces.iter().find(|w| &w.workspace_id == id))
            .map(|w| &w.session_ids);
        let mut sessions: Vec<_> = self
            .sessions
            .values()
            .filter(|s| {
                self.management.registry.archived(&s.session_id) == self.show_archived
                    && ids.is_none_or(|ids| ids.contains(&s.session_id))
                    && (filter.is_empty()
                        || s.session_id.as_str().to_lowercase().contains(&filter)
                        || self.session_title(s).to_lowercase().contains(&filter))
            })
            .collect();
        sessions.sort_by(|a, b| {
            self.management
                .registry
                .pin_rank(&a.session_id)
                .unwrap_or(usize::MAX)
                .cmp(
                    &self
                        .management
                        .registry
                        .pin_rank(&b.session_id)
                        .unwrap_or(usize::MAX),
                )
                .then(b.updated_at.cmp(&a.updated_at))
                .then(a.session_id.cmp(&b.session_id))
        });
        sessions
    }
    fn session_navigation(&self) -> Element<'_, Message> {
        let sessions = self.navigation_sessions();
        let count = sessions.len();
        let mut list = Column::new().spacing(3);
        for session in sessions.into_iter().take(200) {
            let active = self.selected.as_ref() == Some(&session.session_id);
            let marker = if session.running {
                "●"
            } else if self.management.registry.pinned(&session.session_id) {
                "•"
            } else {
                ""
            };
            list = list.push(
                button(row![
                    label(
                        short(&self.session_title(session), 28),
                        if active { TEXT } else { MUTED }
                    )
                    .size(13)
                    .width(Length::Fill),
                    label(marker, design::FOAM).size(10)
                ])
                .on_press_maybe(
                    self.ready
                        .then_some(Message::Select(session.session_id.clone())),
                )
                .style(move |theme, status| design::navigation(theme, status, active))
                .padding([11, 10])
                .width(Length::Fill),
            );
        }
        if count == 0 {
            list = list.push(
                container(
                    label(
                        if self.roster_pending {
                            "Loading conversations…"
                        } else {
                            "No matching conversations"
                        },
                        MUTED,
                    )
                    .size(12),
                )
                .padding(10),
            );
        }
        column![
            text_input("Search conversations", &self.filter)
                .on_input(Message::Filter)
                .padding(10)
                .size(12)
                .font(FONT)
                .style(design::input_style),
            row![
                label(
                    if self.show_archived {
                        "ARCHIVED"
                    } else {
                        "RECENT"
                    },
                    MUTED
                )
                .size(10)
                .width(Length::Fill),
                button(
                    label(
                        if self.show_archived {
                            "View recent"
                        } else {
                            "View archived"
                        },
                        MUTED
                    )
                    .size(10)
                )
                .on_press(Message::ArchivedView)
                .style(design::button_style)
            ]
            .align_y(iced::Alignment::Center),
            scrollable(list).height(Length::Fill),
            label(
                format!(
                    "{} conversations{}",
                    count,
                    if count > 200 {
                        " · first 200 shown"
                    } else {
                        ""
                    }
                ),
                MUTED
            )
            .size(10)
        ]
        .spacing(9)
        .height(Length::Fill)
        .into()
    }
    fn workspace_navigation(&self) -> Element<'_, Message> {
        if !self.workspace_controls && !self.sidebar_sheet {
            return Space::new().height(0).into();
        }
        let mut list = column![
            button(label("All conversations", TEXT).size(12))
                .on_press(Message::Workspace(None))
                .style(design::button_style)
                .width(Length::Fill)
        ]
        .spacing(3);
        for workspace in self.workspaces.iter().take(128) {
            let active = self.workspace.as_ref() == Some(&workspace.workspace_id);
            list = list.push(
                button(
                    label(
                        short(&workspace.title, 28),
                        if active { TEXT } else { MUTED },
                    )
                    .size(12),
                )
                .on_press(Message::Workspace(Some(workspace.workspace_id.clone())))
                .style(move |theme, status| design::navigation(theme, status, active))
                .width(Length::Fill),
            );
        }
        column![
            scrollable(list).height(if self.sidebar_sheet { 110 } else { 80 }),
            text_input("Absolute workspace directory", &self.workspace_path)
                .on_input(Message::WorkspacePath)
                .padding(9)
                .size(11)
                .font(FONT)
                .style(design::input_style),
            button(
                label(
                    if self.workspace_pending {
                        "Opening…"
                    } else {
                        "Open workspace"
                    },
                    TEXT
                )
                .size(12)
            )
            .on_press_maybe(
                (self.ready && !self.workspace_pending).then_some(Message::OpenWorkspace)
            )
            .style(design::button_style)
            .width(Length::Fill)
        ]
        .spacing(5)
        .into()
    }
    fn navigation_sheet(&self) -> Element<'_, Message> {
        // Fixed toolbar: scrolling history must never scroll away Stop or Done.
        let toolbar = row![
            label("Conversations", TEXT).size(20).width(Length::Fill),
            self.stop_button(),
            button(label("Done", TEXT))
                .on_press(Message::DismissPanel)
                .style(design::button_style)
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        let sheet = column![
            button(label(
                if self.new_pending {
                    "Creating…"
                } else {
                    "+ New conversation"
                },
                TEXT
            ))
            .on_press_maybe(self.can_create().then_some(Message::NewSession))
            .style(design::button_style),
            container(self.session_navigation()).height(210),
            label("WORKSPACES", MUTED).size(11),
            self.workspace_navigation()
        ]
        .spacing(14);
        container(column![toolbar, scrollable(sheet).height(Length::Fill)].spacing(18))
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
    fn detail_view<'a>(&'a self, detail: &'a detail_panel::Detail) -> Element<'a, Message> {
        let mut toolbar=row![label(detail.heading.as_str(),TEXT).size(18).width(Length::Fill)];
        if detail.formatted_available() {
            for (mode,title) in [(detail_panel::Mode::Formatted,"Formatted"),(detail_panel::Mode::Source,"Source")] {
                let selected=detail.mode==mode;
                let control=button(label(title,if selected {ACCENT}else{MUTED}).size(12))
                    .on_press_maybe(detail.ticket.map(|ticket|Message::DetailMode(ticket,mode)))
                    .style(move |theme,status|design::navigation(theme,status,selected));
                #[cfg(feature="public-layout-fixture")]
                let control=container(control).id(match mode {detail_panel::Mode::Formatted=>"native-detail-formatted",detail_panel::Mode::Source=>"native-detail-source"});
                toolbar=toolbar.push(control);
            }
        }
        let copy=button(label("Copy source",MUTED).size(12))
            .on_press_maybe(detail.ticket.filter(|_|detail.copy_available()).map(Message::CopyDetailSource))
            .style(design::button_style);
        #[cfg(feature="public-layout-fixture")]
        let copy=container(copy).id("native-detail-copy-source");
        toolbar=toolbar.push(copy);
        let back=button(label("Back",MUTED)).on_press_maybe(self.detail_back()).style(design::button_style);
        #[cfg(feature="public-layout-fixture")]
        let back=container(back).id("native-detail-back");
        toolbar=toolbar.push(back).spacing(8).align_y(iced::Alignment::Center);
        let mut body=Column::new().spacing(12).width(Length::Fill);
        if let Some(metadata)=&detail.metadata {body=body.push(label(metadata.as_str(),MUTED).size(11));}
        let rendered=detail.body();
        #[cfg(feature="public-layout-fixture")]
        let rendered=container(rendered).id("native-detail-body");
        body=body.push(rendered);
        let content=column![toolbar,label(detail.notice(),MUTED).size(11),
            scrollable(container(body).padding(16)).height(Length::Fill),
            label("Bounded native display source (up to 32 KiB); raw records and unsupported content may not be fully represented.",MUTED).size(11)
        ].spacing(12).height(Length::Fill);
        container(
            container(content)
                .max_width(design::CHAT)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .padding(20)
        .center_x(Length::Fill)
        .height(Length::Fill)
        .into()
    }
    fn row_geometries(&self, rows: &[&DisplayRow]) -> Vec<chat_geometry::Geometry> {
        // Match the real sidebar animation and centered padded scroll lane.
        let lane =
            (self.layout_size().width - self.sidebar_motion.value() - 40.0).min(design::CHAT);
        let cache = &self.chat_geometry;
        // Keep the one cache-only live entry across durable anchor measurements.
        cache.retain_keys(
            rows.iter()
                .map(|row| row.key)
                .chain(std::iter::once(u64::MAX)),
        );
        rows.iter()
            .map(|row| cache.geometry(row.key, &row.role, &excerpt(&row.text), lane))
            .collect()
    }
    fn scroll_projection(&self) -> (Vec<u64>, Vec<f32>, f32) {
        let rows: Vec<_> = self
            .transcript
            .display_rows(self.records_expanded)
            .collect();
        let heights = self
            .row_geometries(&rows)
            .iter()
            .map(|g| g.height)
            .collect();
        let lane =
            (self.layout_size().width - self.sidebar_motion.value() - 40.0).min(design::CHAT);
        let trailing = self.transcript.partial().map_or(0.0, |text| {
            self.chat_geometry
                .geometry(u64::MAX, "Assistant · live", &excerpt(&text), lane)
                .height
        });
        (rows.iter().map(|r| r.key).collect(), heights, trailing)
    }
    fn reader_anchor(&self) -> Option<scroll_anchor::Anchor> {
        if self.follow_bottom {
            return None;
        }
        let rows: Vec<_> = self
            .transcript
            .display_rows(self.records_expanded)
            .collect();
        let keys: Vec<_> = rows.iter().map(|r| r.key).collect();
        let heights: Vec<_> = self
            .row_geometries(&rows)
            .iter()
            .map(|g| g.height)
            .collect();
        scroll_anchor::Anchor::capture(&keys, &heights, self.offset)
    }
    fn observe_scroll(
        &mut self,
        generation: u64,
        revision: Option<u64>,
        y: f32,
        height: f32,
        relative: f32,
        content_height: f32,
    ) {
        if generation != self.generation
            || revision.is_none()
            || revision != self.scroll_revision
            || !self.transcript_active()
            || !y.is_finite()
            || y < 0.0
            || !height.is_finite()
            || height < 0.0
            || !content_height.is_finite()
            || content_height < 0.0
        {
            return;
        }
        let maximum = (content_height - height).max(0.0);
        let own_target = self
            .scroll_target
            .is_some_and(|target| (target.clamp(0.0, maximum) - y).abs() <= 1.0);
        // Iced also emits viewport feedback on redraw after layout changes,
        // without input. Preserve intent for a height-only move at the prior legal
        // offset; a differing displacement still supersedes older operations.
        let geometry_feedback = (height - self.viewport).abs() > 1.0
            && (self.offset.clamp(0.0, maximum) - y).abs() <= 1.0;
        self.offset = y;
        self.viewport = height;
        if !own_target && !geometry_feedback {
            self.scroll_target = None;
            self.scroll_feedback = self.scroll_feedback.and_then(|r| r.checked_add(1));
            if relative.is_finite() {
                self.follow_bottom = relative >= 0.98;
            }
        }
    }
    fn invalidate_scroll(&mut self) {
        self.scroll_revision = self.scroll_revision.and_then(|r| r.checked_add(1));
        self.scroll_target = None;
    }
    fn scroll_id(&self) -> String {
        format!(
            "native-transcript-{}-{:?}-{:?}",
            self.generation, self.scroll_revision, self.scroll_feedback
        )
    }
    fn transcript_active(&self) -> bool {
        self.composer_visible()
            && (self.transcript.partial().is_some()
                || self
                    .transcript
                    .display_rows(self.records_expanded)
                    .next()
                    .is_some())
    }
    fn position_transcript(&mut self) -> Task<Message> {
        if !self.transcript_active()
            || self.scroll_revision.is_none()
            || self.scroll_feedback.is_none()
        {
            return Task::none();
        }
        if self.follow_bottom {
            iced::widget::operation::snap_to(self.scroll_id(), scrollable::RelativeOffset::END)
        } else {
            self.scroll_target = Some(self.offset);
            let generation = self.generation;
            let revision = self.scroll_revision;
            let feedback = self.scroll_feedback;
            scroll_operation::restore(self.scroll_id(), self.offset, move |y, viewport| {
                Message::ScrollApplied {
                    generation,
                    revision,
                    feedback,
                    y,
                    viewport,
                }
            })
        }
    }
    fn reflow_transcript(&mut self, anchor: Option<scroll_anchor::Anchor>) -> Task<Message> {
        self.invalidate_scroll();
        let (keys, heights, trailing) = self.scroll_projection();
        // Final viewport clamp belongs to the operation on the rebuilt native layout.
        let max = (heights.iter().sum::<f32>() + trailing).max(0.0);
        self.offset = anchor
            .and_then(|a| a.restore(&keys, &heights, 0.0, trailing))
            .unwrap_or_else(|| {
                if self.offset.is_finite() {
                    self.offset.clamp(0.0, max)
                } else {
                    0.0
                }
            });
        self.position_transcript()
    }
    fn transcript_view(&self) -> Element<'_, Message> {
        let partial = self.transcript.partial().map(|text| DisplayRow {
            key: u64::MAX,
            role: "Assistant · live".into(),
            text,
            time: 0,
            surface: false,
        });
        let len = self.transcript.display_rows(true).count() + usize::from(partial.is_some());
        let rendered: Vec<&DisplayRow> = self
            .transcript
            .display_rows(self.records_expanded)
            .chain(partial.as_ref())
            .collect();
        if rendered.is_empty() {
            self.chat_geometry.retain_keys(std::iter::empty());
            return self.welcome(len);
        }
        let geometries = self.row_geometries(&rendered);
        let heights: Vec<_> = geometries.iter().map(|g| g.height).collect();
        let range = height_range(&heights, self.offset, self.viewport);
        let mut contents = Column::new()
            .push(Space::new().height(heights[..range.start].iter().sum::<f32>()))
            .width(Length::Fill);
        for index in range.clone() {
            let record = rendered[index];
            let human = record.role == "You";
            let assistant = record.role.starts_with("Assistant")
                || record.role == "Interrupted assistant attempt";
            let bubble: Element<'_, Message> = if let Some(activity) =
                self.transcript.activity(record.key)
            {
                // Match Harness's quiet single-line disclosures, not full tool cards.
                // Raw arguments/output never enter the collapsed line.
                let title = if record.role == "Tool result" {
                    activity
                        .title
                        .strip_suffix(" · result")
                        .unwrap_or(&activity.title)
                } else {
                    &activity.title
                };
                let verb = if title.starts_with("Tool · ") {
                    title
                } else {
                    title.split(" · ").next().unwrap_or(title)
                };
                let icon = if verb == "Edit" || verb == "Write" {
                    "↳"
                } else if verb == "Bash" || verb == "Shell" {
                    ">_"
                } else {
                    "▤"
                };
                let target = activity.target.as_deref().unwrap_or("Details available");
                let mut line = row![
                    label(icon, MUTED).size(12),
                    label(short(verb, 24), MUTED).size(12),
                    label("·", MUTED).size(12),
                    label(
                        short(
                            target,
                            if self.layout_size().width < 700.0 {
                                28
                            } else if self.compact() {
                                42
                            } else {
                                80
                            }
                        ),
                        MUTED
                    )
                    .size(12)
                    .width(Length::Fill)
                ]
                .spacing(7)
                .align_y(iced::Alignment::Center);
                // Suffix stays outside the clipped label; output cannot look like a new call.
                if activity.failed {
                    line = line.push(label("Recorded error", DANGER).size(10));
                } else if record.role == "Tool result" {
                    line = line.push(label("Output", MUTED).size(10));
                }
                line = line.push(label("▸", MUTED).size(12));
                button(container(line).height(24).clip(true))
                    .on_press_maybe(self.detail_request(record.key))
                    .style(design::button_style)
                    .padding([0, 8])
                    .width(Length::Fill)
                    .into()
            } else if human || assistant {
                let geometry = geometries[index];
                let header = row![
                    label(
                        record.role.clone(),
                        if assistant { design::FOAM } else { ACCENT }
                    )
                    .size(12)
                    .width(Length::Fill),
                    button(
                        label(
                            if geometry.body_height >= 75.0 || excerpt(&record.text) != record.text
                            {
                                "More ▸"
                            } else {
                                "▸"
                            },
                            MUTED
                        )
                        .size(12)
                    )
                    .on_press_maybe(self.detail_request(record.key))
                    .style(design::button_style)
                    .padding([2, 4])
                ]
                .align_y(iced::Alignment::Center);
                let card = column![
                    container(header).height(24).clip(true),
                    container(
                        label(excerpt(&record.text), TEXT)
                            .size(15)
                            .width(geometry.width - 28.0)
                    )
                    .height(geometry.body_height)
                    .clip(true)
                ]
                .spacing(7);
                container(card)
                    .padding([10, 14])
                    .width(geometry.width)
                    .style(move |_| {
                        if human {
                            design::card(OVERLAY, 18.0)
                        } else {
                            panel(BASE)
                        }
                    })
                    .into()
            } else {
                let card = column![
                    label(
                        if self.transcript.inbox_canceled(record.key) {
                            "Canceled queued input".into()
                        } else {
                            record.role.clone()
                        },
                        if self.transcript.inbox_canceled(record.key) {
                            DANGER
                        } else if assistant {
                            design::FOAM
                        } else {
                            ACCENT
                        }
                    )
                    .size(12),
                    container(label(excerpt(&record.text), TEXT).size(15))
                        .height(75)
                        .clip(true),
                    button(label("Read more ▸", MUTED).size(10))
                        .on_press_maybe(self.detail_request(record.key))
                        .style(design::button_style)
                        .padding([3, 0])
                ]
                .spacing(7);
                container(card)
                    .padding([10, 14])
                    .width(Length::Fill)
                    .style(move |_| {
                        if human {
                            design::card(OVERLAY, 18.0)
                        } else if assistant {
                            panel(BASE)
                        } else {
                            design::card(SURFACE, 12.0)
                        }
                    })
                    .into()
            };
            let bubble = container(bubble).width(geometries[index].width);
            // View, spacers and anchors share the same projected rows and heights;
            // diagnostics remain retained for the Session records disclosure.
            let outer = container(bubble)
                .align_x(if human {
                    iced::alignment::Horizontal::Right
                } else {
                    iced::alignment::Horizontal::Left
                })
                .padding([8, 4])
                .height(geometries[index].height)
                .width(Length::Fill)
                .clip(true);
            // Fixture-only observation tags do not change production layout or metrics.
            #[cfg(feature = "public-layout-fixture")]
            let outer = outer.id(format!("native-public-transcript-row-{}", record.key));
            contents = contents.push(outer);
        }
        contents = contents.push(Space::new().height(heights[range.end..].iter().sum::<f32>()));
        let controls = row![
            button(
                label(
                    if self.page_pending {
                        "Loading…"
                    } else {
                        "Older messages"
                    },
                    MUTED
                )
                .size(11)
            )
            .on_press_maybe(
                (self.ready
                    && self.follow_ready
                    && self.transcript.has_more()
                    && !self.page_pending)
                    .then_some(Message::Older)
            )
            .style(design::button_style),
            button(label("Reload", MUTED).size(11))
                .on_press_maybe((self.ready && self.selected.is_some()).then_some(Message::Reload))
                .style(design::button_style),
            button(
                label(
                    if self.records_expanded {
                        "Conversation"
                    } else {
                        "Session records"
                    },
                    MUTED
                )
                .size(11)
            )
            .on_press(Message::ToggleRecords)
            .style(design::button_style),
            Space::new().width(Length::Fill),
            button(
                label(
                    if self.follow_bottom {
                        "Following live"
                    } else {
                        "Latest ↓"
                    },
                    MUTED
                )
                .size(11)
            )
            .on_press(Message::Bottom)
            .style(design::button_style)
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);
        let generation = self.generation;
        let revision = self.scroll_revision;
        let lane = container(contents)
            .max_width(design::CHAT)
            .width(Length::Fill);
        column![
            container(controls).padding([2, 20]),
            scrollable(container(lane).center_x(Length::Fill).padding([0, 20]))
                .id(self.scroll_id())
                .on_scroll(move |viewport| Message::Scrolled {
                    generation,
                    revision,
                    viewport
                })
                .height(Length::Fill)
        ]
        .height(Length::Fill)
        .into()
    }
    fn welcome(&self, records: usize) -> Element<'_, Message> {
        let selected = self.selected.is_some();
        let mut hero = column![
            brand(),
            label(
                if selected {
                    "What are we working on?"
                } else {
                    "A quiet space for your work"
                },
                TEXT
            )
            .size(if self.compact() { 25 } else { 30 }),
            label(
                if selected {
                    "Write a message. Send when ready."
                } else {
                    "Choose or create a conversation in the sidebar."
                },
                MUTED
            )
            .size(14)
        ]
        .spacing(18)
        .align_x(iced::Alignment::Center);
        if records > 0 {
            hero = hero.push(
                button(label(format!("Show {records} session records"), MUTED).size(12))
                    .on_press(Message::ToggleRecords)
                    .style(design::button_style),
            );
        }
        if !self.transcript.safe_to_send() {
            hero = hero.push(
                label(
                    "Read-only: this transcript needs resynchronization.",
                    DANGER,
                )
                .size(12),
            );
        }
        let hero = container(hero)
            .padding(30)
            .max_width(620)
            .width(Length::Fill);
        scrollable(
            container(hero)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .height(Length::Fill)
        .into()
    }
    fn capabilities_view(&self) -> Element<'_, Message> {
        let heading = |name: &'static str, section: InfoSection| {
            row![
                label(name, TEXT).size(16).width(Length::Fill),
                button(label(
                    if self.info_expanded & section.bit() != 0 {
                        "▾"
                    } else {
                        "▸"
                    },
                    MUTED
                ))
                .on_press(Message::ToggleInfoDetails(section))
                .style(design::button_style)
                .padding([8, 12])
            ]
            .align_y(iced::Alignment::Center)
        };
        let mut details = column![
            label("About Harness", TEXT).size(22),
            heading("Features & limits", InfoSection::Features)
        ]
        .spacing(16);
        if self.info_expanded & InfoSection::Features.bit() != 0 {
            details = details.push(label("Native conversations and workspaces · explicit models and Send/Stop · API login and Connect ChatGPT · supported plugin settings · reviewed pin/archive and export · supported approval/question replies.", TEXT))
                .push(label("Rich attachments, terminal/document previews, timed/continued question claims, rename/fork and subagent steering are not available in this UI. Full desktop parity remains unfinished.", MUTED));
        }
        details = details.push(heading("Diagnostics", InfoSection::Diagnostics));
        if self.info_expanded & InfoSection::Diagnostics.bit() != 0 {
            details = details
                .push(label(&self.status, MUTED))
                .push(label(
                    format!("Optional account metadata: {}", self.account),
                    MUTED,
                ))
                .push(label(
                    "ChatGPT connection status is separate: Settings → Codex.",
                    MUTED,
                ));
            if let Some(id) = &self.selected {
                if let Some(session) = self.sessions.get(id) {
                    details =
                        details.push(label(format!("Conversation: {}", self.session_title(session)), TEXT));
                }
                if let Some(model) = self.model_selections.get(id) {
                    details = details.push(label(
                        format!("Session model: {} / {}", model.provider, model.model),
                        TEXT,
                    ));
                }
            }
            if !self.projections.is_empty() {
                details = details
                    .push(label("Read-only session projections", MUTED).size(13))
                    .push(label(
                        reducer::safe_json(
                            &serde_json::to_value(&self.projections).unwrap_or_default(),
                        ),
                        TEXT,
                    ));
            }
        }
        details = details.push(
            button(label("Back to conversation", TEXT))
                .on_press(Message::Capabilities)
                .style(design::button_style),
        );
        scrollable(
            container(
                container(details)
                    .max_width(design::CHAT)
                    .width(Length::Fill),
            )
            .center_x(Length::Fill)
            .padding(24),
        )
        .height(Length::Fill)
        .into()
    }
    fn close_view(&self) -> Element<'_, Message> {
        let mut content = column![label("Close native Harness", ACCENT)].spacing(16);
        match &self.close {Close::Inspecting=>content=content.push(label("Inspecting owned Core outside the GUI thread…",TEXT)),Close::Confirm(reason)=>content=content.push(label(reason,TEXT)).push(row![button("Stay").on_press(Message::Stay),button("Close and stop own backend").on_press(Message::ConfirmClose)].spacing(12)),Close::Stopping=>content=content.push(label("Closing native subscriptions and transport; waiting for owned Core exit, containment and any admitted filesystem save (not cancellable; partial files may remain)…",TEXT)),Close::Failed(reason)=>content=content.push(label(reason,DANGER)).push(row![button("Stay").on_press(Message::Stay),button("Exit anyway (result uncertain)").on_press_maybe(self.stopped.then_some(Message::ExitAnyway))].spacing(12)),Close::None=>{}}
        container(
            container(content)
                .padding(28)
                .max_width(760)
                .style(|_| design::card(SURFACE, 20.0)),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    }
}
#[cfg(test)]
fn chat_role(role: &str) -> bool {
    reducer::presentation::conversation_role(role)
}
#[cfg(test)]
fn chat_row_count(rows: &[DisplayRow]) -> usize {
    rows.iter().filter(|row| chat_role(&row.role)).count()
}
#[cfg(test)]
fn chat_rows<'a>(rows: &'a [DisplayRow], partial: Option<&'a DisplayRow>) -> Vec<&'a DisplayRow> {
    rows.iter()
        .filter(|row| chat_role(&row.role))
        .chain(partial)
        .collect()
}
pub(crate) fn panel(color: Color) -> container::Style {
    container::Style {
        background: Some(color.into()),
        text_color: Some(TEXT),
        ..Default::default()
    }
}
pub(crate) fn label<'a>(
    value: impl text::IntoFragment<'a>,
    color: Color,
) -> iced::widget::Text<'a> {
    text(value)
        .size(14)
        .color(color)
        .font(FONT)
        .shaping(text::Shaping::Advanced)
}
// Focus-local layout shortcuts only; never register or alter desktop bindings.
fn layout_shortcut(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Message> {
    if modifiers.control() && !modifiers.alt() && !modifiers.logo() && !modifiers.shift() {
        if let keyboard::Key::Character(value) = key {
            if value.eq_ignore_ascii_case("b") {
                return Some(Message::ToggleSidebar);
            }
        }
    }
    if modifiers.is_empty() && matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape)) {
        return Some(Message::DismissPanel);
    }
    None
}
fn panel_shortcut(key:&keyboard::Key, modifiers:keyboard::Modifiers, ticket:Option<detail_panel::Stamp>) -> Option<Message> {
    match (layout_shortcut(key,modifiers)?,ticket) {
        (Message::DismissPanel,Some(ticket)) => Some(Message::HideDetails(ticket)),
        (message,_) => Some(message),
    }
}
fn routine_status(status: &str) -> bool {
    // Exact controlled successes only. Never hide errors by substring/prefix or infer account validity.
    matches!(
        status,
        "Live real session; UTC timestamps; no generation started"
            | "Real sessions loaded; select one or create a new session"
            | "Workspace opened; no prompt submitted"
            | "Model selection accepted by owned Host"
    )
}
fn short(value: &str, count: usize) -> String {
    let mut chars = value.chars();
    let result: String = chars.by_ref().take(count).collect();
    if chars.next().is_some() {
        format!("{result}…")
    } else {
        result
    }
}
fn excerpt(value: &str) -> String {
    let mut lines = value.lines();
    let first = lines.by_ref().take(4).collect::<Vec<_>>().join("\n");
    let mut result = short(&first, 240);
    if lines.next().is_some() && !result.ends_with('…') {
        result.push('…');
    }
    result
}
fn brand<'a>() -> Element<'a, Message> {
    container(label("H", ACCENT).size(19))
        .width(36)
        .height(36)
        .center_x(36)
        .center_y(36)
        .style(|_| design::card(OVERLAY, 12.0))
        .into()
}
/// A settings dialog over the existing native shell, never another OS window.
/// Inert background widgets are drawn, but receive no events, focus operations or overlays.
/// The foreground receives input normally, then captures any otherwise-unhandled input.
pub(crate) fn settings_overlay<'a, M: Clone + 'a>(
    background: Element<'a, M>,
    dialog: Element<'a, M>,
    on_escape: M,
) -> Element<'a, M> {
    let scrim = container(Space::new().width(Length::Fill).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.58).into()),
            ..Default::default()
        });
    let foreground = container(iced::widget::opaque(dialog))
        .padding(24)
        .center_x(Length::Fill)
        .center_y(Length::Fill);
    let layer = iced::widget::stack![iced::widget::opaque(scrim), foreground].into();
    iced::widget::stack![
        Element::new(ModalFence {
            content: background,
            on_escape: None
        }),
        Element::new(ModalFence {
            content: layer,
            on_escape: Some(on_escape)
        })
    ]
    .into()
}

fn settings_modal_size(viewport: iced::Size) -> iced::Size {
    // Fit the preferred 960×700 card even below the application's minimum size.
    iced::Size::new(
        (viewport.width - 48.0).max(1.0).min(960.0),
        (viewport.height - 48.0).max(1.0).min(700.0),
    )
}

fn background_message(message: &Message) -> bool {
    matches!(
        message,
        Message::FileOpen
            | Message::FilePath(..)
            | Message::FileUpload(_)
            | Message::FileCancel(_)
            | Message::FileRemove(_)
            | Message::ToggleSidebar
            | Message::ToggleConversationMenu
            | Message::ToggleInfoDetails(_)
            | Message::ToggleRecords
            | Message::Editor(_)
            | Message::Send
            | Message::Stop
            | Message::Select(_)
            | Message::Workspace(_)
            | Message::NewSession
            | Message::WorkspacePath(_)
            | Message::WorkspaceControls
            | Message::OpenWorkspace
            | Message::Filter(_)
            | Message::LoadModels
            | Message::Model(_)
            | Message::Effort(_)
            | Message::Older
            | Message::Reload
            | Message::Scrolled { .. }
            | Message::ScrollApplied { .. }
            | Message::Bottom
            | Message::Details(_)
            | Message::DetailMode(..)
            | Message::CopyDetailSource(_)
            | Message::HideDetails(_)
            | Message::Capabilities
            | Message::Interaction(_)
            | Message::Management(_)
            | Message::ExportSession(_)
            | Message::Export(_)
            | Message::ArchivedView
    )
}

fn modal_escape(event: &iced::Event) -> bool {
    matches!(event, iced::Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(keyboard::key::Named::Escape), modifiers,
        repeat: false, ..
    }) if modifiers.is_empty())
}

struct ModalFence<'a, M, R = iced::Renderer> {
    content: Element<'a, M, Theme, R>,
    // None is the inert background; Some is the interactive, input-capturing dialog layer.
    on_escape: Option<M>,
}
impl<M: Clone, R: iced::advanced::Renderer> iced::advanced::Widget<M, Theme, R>
    for ModalFence<'_, M, R>
{
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        self.content.as_widget().tag()
    }
    fn state(&self) -> iced::advanced::widget::tree::State {
        self.content.as_widget().state()
    }
    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        self.content.as_widget().children()
    }
    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        self.content.as_widget().diff(tree);
    }
    fn size(&self) -> iced::Size<Length> {
        self.content.as_widget().size()
    }
    fn size_hint(&self) -> iced::Size<Length> {
        self.content.as_widget().size_hint()
    }
    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &R,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }
    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut R,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        self.content.as_widget().draw(
            tree,
            renderer,
            theme,
            style,
            layout,
            if self.on_escape.is_some() {
                cursor
            } else {
                iced::advanced::mouse::Cursor::Unavailable
            },
            viewport,
        );
    }
    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &R,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        if self.on_escape.is_some() {
            self.content
                .as_widget_mut()
                .operate(tree, layout, renderer, operation);
        }
    }
    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &R,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, M>,
        viewport: &iced::Rectangle,
    ) {
        let Some(on_escape) = &self.on_escape else {
            return;
        };
        if modal_escape(event) {
            shell.publish(on_escape.clone());
        } else {
            self.content.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            );
        }
        if matches!(
            event,
            iced::Event::Mouse(_)
                | iced::Event::Keyboard(_)
                | iced::Event::Touch(_)
                | iced::Event::InputMethod(_)
        ) {
            shell.capture_event();
        }
    }
    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &R,
    ) -> iced::advanced::mouse::Interaction {
        if self.on_escape.is_none() {
            return iced::advanced::mouse::Interaction::None;
        }
        let interaction = self
            .content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer);
        if interaction == iced::advanced::mouse::Interaction::None
            && cursor.is_over(layout.bounds())
        {
            iced::advanced::mouse::Interaction::Idle
        } else {
            interaction
        }
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &R,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, M, Theme, R>> {
        if self.on_escape.is_some() {
            self.content
                .as_widget_mut()
                .overlay(tree, layout, renderer, viewport, translation)
        } else {
            None
        }
    }
}

#[cfg(all(test, debug_assertions))]
mod modal_widget_tests {
    use super::*;
    use iced::advanced::{self as core, Widget};
    use std::{cell::Cell, rc::Rc};
    #[derive(Clone, Default)]
    struct Counts {
        events: Rc<Cell<usize>>,
        operations: Rc<Cell<usize>>,
        overlays: Rc<Cell<usize>>,
    }
    struct Probe(Counts);
    impl Widget<&'static str, Theme, ()> for Probe {
        fn size(&self) -> iced::Size<Length> {
            iced::Size::new(Length::Fill, Length::Fill)
        }
        fn layout(
            &mut self,
            _: &mut core::widget::Tree,
            _: &(),
            _: &core::layout::Limits,
        ) -> core::layout::Node {
            core::layout::Node::new(iced::Size::new(300.0, 200.0))
        }
        fn draw(
            &self,
            _: &core::widget::Tree,
            _: &mut (),
            _: &Theme,
            _: &core::renderer::Style,
            _: core::Layout<'_>,
            _: core::mouse::Cursor,
            _: &iced::Rectangle,
        ) {
        }
        fn update(
            &mut self,
            _: &mut core::widget::Tree,
            _: &iced::Event,
            _: core::Layout<'_>,
            _: core::mouse::Cursor,
            _: &(),
            _: &mut dyn core::Clipboard,
            shell: &mut core::Shell<'_, &'static str>,
            _: &iced::Rectangle,
        ) {
            self.0.events.set(self.0.events.get() + 1);
            shell.publish("foreground");
        }
        fn operate(
            &mut self,
            _: &mut core::widget::Tree,
            _: core::Layout<'_>,
            _: &(),
            _: &mut dyn core::widget::Operation,
        ) {
            self.0.operations.set(self.0.operations.get() + 1);
        }
        fn mouse_interaction(
            &self,
            _: &core::widget::Tree,
            _: core::Layout<'_>,
            _: core::mouse::Cursor,
            _: &iced::Rectangle,
            _: &(),
        ) -> core::mouse::Interaction {
            core::mouse::Interaction::Pointer
        }
        fn overlay<'b>(
            &'b mut self,
            _: &'b mut core::widget::Tree,
            _: core::Layout<'b>,
            _: &(),
            _: &iced::Rectangle,
            _: iced::Vector,
        ) -> Option<core::overlay::Element<'b, &'static str, Theme, ()>> {
            self.0.overlays.set(self.0.overlays.get() + 1);
            None
        }
    }
    struct Traverse;
    impl core::widget::Operation for Traverse {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn core::widget::Operation)) {
            operate(self);
        }
    }
    fn key(key: keyboard::key::Named, modifiers: keyboard::Modifiers) -> iced::Event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(key),
            modified_key: keyboard::Key::Named(key),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Enter),
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        })
    }
    #[test]
    fn modal_layer_captures_mouse_wheel_keyboard_and_only_exposes_foreground_focus() {
        let background = Counts::default();
        let foreground = Counts::default();
        let elements: Vec<Element<'_, &'static str, Theme, ()>> = vec![
            Element::new(ModalFence {
                content: Element::new(Probe(background.clone())),
                on_escape: None,
            }),
            Element::new(ModalFence {
                content: Element::new(Probe(foreground.clone())),
                on_escape: Some("close"),
            }),
        ];
        let mut stack = iced::widget::Stack::with_children(elements);
        let mut tree = core::widget::Tree::new(&stack as &dyn Widget<&'static str, Theme, ()>);
        let node = stack.layout(
            &mut tree,
            &(),
            &core::layout::Limits::new(iced::Size::ZERO, iced::Size::new(300.0, 200.0)),
        );
        let layout = core::Layout::new(&node);
        let viewport = iced::Rectangle::with_size(iced::Size::new(300.0, 200.0));
        let cursor = core::mouse::Cursor::Available(iced::Point::new(10.0, 10.0));
        let events = [
            iced::Event::Mouse(core::mouse::Event::ButtonPressed(core::mouse::Button::Left)),
            iced::Event::Mouse(core::mouse::Event::WheelScrolled {
                delta: core::mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
            }),
            key(keyboard::key::Named::Enter, keyboard::Modifiers::CTRL),
            key(keyboard::key::Named::Escape, keyboard::Modifiers::empty()),
        ];
        let mut messages = vec![];
        for event in &events {
            let mut shell = core::Shell::new(&mut messages);
            stack.update(
                &mut tree,
                event,
                layout,
                cursor,
                &(),
                &mut core::clipboard::Null,
                &mut shell,
                &viewport,
            );
            assert!(shell.is_event_captured());
        }
        assert_eq!(
            messages,
            ["foreground", "foreground", "foreground", "close"]
        );
        assert_eq!(background.events.get(), 0);
        assert_eq!(foreground.events.get(), 3);
        stack.operate(&mut tree, layout, &(), &mut Traverse);
        assert_eq!(background.operations.get(), 0);
        assert_eq!(foreground.operations.get(), 1);
        let _ = stack.overlay(&mut tree, layout, &(), &viewport, iced::Vector::ZERO);
        assert_eq!(background.overlays.get(), 0);
        assert_eq!(foreground.overlays.get(), 1);
    }
}

fn composer_binding(
    event: text_editor::KeyPress,
    send_enabled: bool,
) -> Option<text_editor::Binding<Message>> {
    if matches!(event.status, text_editor::Status::Focused { .. }) {
        if event.modifiers.control()
            && !event.modifiers.alt()
            && !event.modifiers.logo()
            && !event.modifiers.shift()
            && matches!(event.key, keyboard::Key::Named(keyboard::key::Named::Enter))
        {
            return send_enabled.then_some(text_editor::Binding::Custom(Message::Send));
        }
        if let Some(message) = layout_shortcut(&event.key, event.modifiers) {
            return Some(text_editor::Binding::Custom(message));
        }
    }
    text_editor::Binding::from_key_press(event)
}
fn title(session: &SessionSummary) -> String {
    projected_title(session, session.projections.as_ref().and_then(|p| p.values.get("title")))
}
fn projected_title(session: &SessionSummary, value: Option<&serde_json::Value>) -> String {
    value
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != session.session_id.as_str())
        .map(str::to_owned)
        .or_else(|| {
            if session.blank {
                Some("New chat".into())
            } else {
                session.cwd.as_ref().and_then(|cwd| {
                    std::path::Path::new(cwd)
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
            }
        })
        .unwrap_or_else(|| "Untitled chat".into())
}
#[cfg(test)]
fn row_height(row: &DisplayRow) -> f32 {
    if matches!(row.role.as_str(), "Tool call" | "Tool result") {
        ACTIVITY_HEIGHT
    } else {
        ROW_HEIGHT
    }
}
#[cfg(test)]
fn transcript_height(rows: &[DisplayRow]) -> f32 {
    rows.iter().map(row_height).sum()
}
#[cfg(test)]
fn transcript_range(rows: &[&DisplayRow], offset: f32, viewport: f32) -> std::ops::Range<usize> {
    height_range(
        &rows.iter().map(|row| row_height(row)).collect::<Vec<_>>(),
        offset,
        viewport,
    )
}
fn height_range(heights: &[f32], offset: f32, viewport: f32) -> std::ops::Range<usize> {
    let offset = if offset.is_finite() {
        offset.max(0.0)
    } else {
        0.0
    };
    let viewport = if viewport.is_finite() {
        viewport.clamp(0.0, 4096.0)
    } else {
        0.0
    };
    let min = (offset - 2.0 * ROW_HEIGHT).max(0.0);
    let max = offset + viewport + 3.0 * ROW_HEIGHT;
    let mut height = 0.0;
    let mut start = heights.len();
    let mut end = heights.len();
    for (index, row_height) in heights.iter().enumerate() {
        let next = height + row_height;
        if start == heights.len() && next > min {
            start = index;
        }
        if height >= max {
            end = index;
            break;
        }
        height = next;
    }
    start..end.max(start)
}
#[cfg(test)]
pub fn visible_range(len: usize, offset: f32, viewport: f32) -> std::ops::Range<usize> {
    let start = ((offset.max(0.0) / ROW_HEIGHT).floor() as usize)
        .saturating_sub(2)
        .min(len);
    let end = (((offset.max(0.0) + viewport.max(0.0)) / ROW_HEIGHT).ceil() as usize)
        .saturating_add(3)
        .min(len);
    start..end.max(start)
}
pub fn theme(_: &App) -> Theme {
    native_theme()
}
pub(crate) fn native_theme() -> Theme {
    Theme::custom(
        "Rose Pine",
        iced::theme::Palette {
            background: BASE,
            text: TEXT,
            primary: ACCENT,
            success: Color::from_rgb8(0x9c, 0xcf, 0xd8),
            warning: Color::from_rgb8(0xf6, 0xc1, 0x77),
            danger: DANGER,
        },
    )
}
#[path = "chat_geometry.rs"]
mod chat_geometry;
#[cfg(test)]
#[path = "codex_ui_tests.rs"]
mod codex_ui_tests;
#[cfg(feature = "public-layout-fixture")]
#[path = "layout_fixture.rs"]
pub mod layout_fixture;
#[cfg(test)]
#[path = "layout_ui_tests.rs"]
mod layout_ui_tests;
#[path = "scroll_anchor.rs"]
mod scroll_anchor;
#[cfg(feature = "public-layout-fixture")]
#[path = "scroll_fixture.rs"]
pub mod scroll_fixture;
#[path = "scroll_operation.rs"]
mod scroll_operation;
#[cfg(test)]
#[path = "scroll_ui_tests.rs"]
mod scroll_ui_tests;
#[cfg(test)]
#[path = "transcript_layout_tests.rs"]
mod transcript_layout_tests;
#[cfg(test)]
#[path = "transcript_projection_tests.rs"]
mod transcript_projection_tests;
#[cfg(test)]
#[path = "title_ui_tests.rs"]
mod title_ui_tests;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_widget_tree_is_bounded_by_visible_range() {
        let range = visible_range(10_000, 500_000.0, 600.0);
        assert!(range.len() <= 10);
        assert!(range.start > 0);
    }
    fn display_row(key: u64, role: &str) -> DisplayRow {
        DisplayRow {
            key,
            role: role.into(),
            text: "fixture".into(),
            time: 1000,
            surface: false,
        }
    }
    #[test]
    fn chat_filter_hides_only_three_exact_metadata_roles() {
        let roles = [
            "permission/preset",
            "sandbox/mode",
            "approval/policy",
            "You",
            "Assistant",
            "Assistant · live",
            "Interrupted assistant attempt",
            "Tool call",
            "Tool result",
            "System",
            "Developer",
            "approval/policy-changed",
            "Permission/preset",
            "future/required",
        ];
        let rows: Vec<_> = roles
            .iter()
            .enumerate()
            .map(|(key, role)| display_row(key as u64, role))
            .collect();
        let visible = chat_rows(&rows, None);
        assert_eq!(
            visible
                .iter()
                .map(|row| row.role.as_str())
                .collect::<Vec<_>>(),
            roles[3..]
        );
        assert_eq!(chat_row_count(&rows), roles.len() - 3);
        assert_eq!(rows.len(), roles.len()); // Presentation does not discard durable records.
    }
    #[test]
    fn metadata_only_chat_is_empty_but_live_assistant_is_never_hidden() {
        let rows = [
            display_row(0, "permission/preset"),
            display_row(1, "sandbox/mode"),
            display_row(2, "approval/policy"),
        ];
        assert!(chat_rows(&rows, None).is_empty());
        let partial = display_row(u64::MAX, "Assistant · live");
        let visible = chat_rows(&rows, Some(&partial));
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].key, u64::MAX);
    }
    #[test]
    fn filtered_list_preserves_exact_160px_spacer_geometry_and_widget_bound() {
        let rows: Vec<_> = (0..4096)
            .map(|key| {
                display_row(
                    key,
                    if key < 3 {
                        "sandbox/mode"
                    } else {
                        "Tool result"
                    },
                )
            })
            .collect();
        let partial = display_row(u64::MAX, "Assistant · live");
        let rendered = chat_rows(&rows, Some(&partial));
        assert_eq!(rendered.len(), 4094);
        for offset in [0.0, 1000.0, 500_000.0, 700_000.0] {
            let range = visible_range(rendered.len(), offset, 600.0);
            assert!(range.len() <= 10);
            assert!(range.end <= rendered.len());
            let above = range.start as f32 * ROW_HEIGHT;
            let cards = range.len() as f32 * ROW_HEIGHT;
            let below = (rendered.len() - range.end) as f32 * ROW_HEIGHT;
            assert_eq!(above + cards + below, rendered.len() as f32 * 160.0);
        }
    }
    #[test]
    fn titles_use_readable_projection_then_workspace_without_session_uuid() {
        let mut session: SessionSummary = serde_json::from_value(serde_json::json!({
            "agentAvailable":true, "sessionId":"12345678-1234-1234-1234-123456789abc",
            "updatedAt":1000, "running":false, "blank":false, "cwd":"/home/example/project",
            "projections":{"kind":"cached", "asOfSeq":0, "values":{"title":"  Refine the native UI  "}}
        })).unwrap();
        assert_eq!(title(&session), "Refine the native UI");
        session.projections.as_mut().unwrap().values.insert(
            "title".into(),
            serde_json::json!(session.session_id.as_str()),
        );
        assert_eq!(title(&session), "project");
        session.projections = None;
        session.blank = true;
        assert_eq!(title(&session), "New chat");
        session.blank = false;
        session.cwd = None;
        assert_eq!(title(&session), "Untitled chat");
    }
    #[test]
    fn older_page_anchor_counts_every_rendered_durable_row() {
        let (mut app, _) = app();
        let user = |seq: u64, value: &str| {
            serde_json::json!({"type":"event", "event":{
                "type":"user/message", "seq":seq, "time":1000,
                "data":{"content":[{"type":"text", "text":value}]}, "surfaceOp":"append"
            }})
        };
        let snapshot: SessionFollowFrame = serde_json::from_value(serde_json::json!({
            "type":"snapshot", "records":[user(4, "tail")], "cursor":4, "hasMore":true,
            "header":{"version":4, "id":"s1", "createdAt":1000, "isSeeded":false},
            "projections":{"asOfSeq":4, "values":{}}, "assistantStream":{"revision":0}
        }))
        .unwrap();
        app.transcript.apply(0, snapshot).unwrap();
        app.offset = 40.0;
        app.viewport = 20.0;
        app.follow_bottom = false;
        let mut records: Vec<_> = ["permission/preset", "sandbox/mode", "approval/policy"]
            .iter()
            .enumerate()
            .map(|(seq, kind)| {
                serde_json::json!({"type":"event", "event":{
                    "type":kind, "seq":seq, "time":0, "data":{"value":"fixture"}
                }})
            })
            .collect();
        records.extend([user(3, "older"), user(4, "tail")]);
        let page = serde_json::from_value(serde_json::json!({"records":records, "hasMore":false}))
            .unwrap();
        let _ = app.receive(Event::Page {
            generation: 0,
            result: Ok(page),
        });
        assert_eq!(app.transcript.rows().len(), 5);
        assert_eq!(chat_row_count(app.transcript.rows()), 2);
        assert_eq!(app.offset, 127.0); // Same tail key/intra40 after one visible87px user row; diagnostics stay retained.
    }
    #[test]
    fn workspace_controls_toggle_is_view_only() {
        let (mut app, mut receiver) = app();
        let _ = app.update(Message::WorkspaceControls);
        assert!(app.workspace_controls);
        assert!(receiver.try_recv().is_err());
        let _ = app.update(Message::WorkspaceControls);
        assert!(!app.workspace_controls);
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn multilingual_excerpt_preserves_codepoints() {
        assert_eq!(short("你好世界🦀", 3), "你好世…");
        assert!(excerpt(&"x".repeat(10000)).chars().count() <= 241);
    }
    #[test]
    fn window_events_are_already_application_logical_at_scale125() {
        let (mut app, mut receiver) = app();
        app.options.scale = 1.25;
        let size = iced::Size::new(608.0, 448.0);
        let _ = app.update(Message::Resized(size));
        assert_eq!(app.layout_size(), size);
        assert!(app.compact());
        assert_eq!(app.sidebar_motion.value(), design::RAIL);
        assert!(!app.sidebar_motion.running(true));
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn resizing_and_retoggling_sidebar_never_send_backend_commands() {
        let (mut app, mut receiver) = app();
        let _ = app.update(Message::WindowActive(true));
        let _ = app.update(Message::ToggleSidebar);
        assert!(app.sidebar_motion.running(true));
        let _ = app.update(Message::MotionFrame(
            Instant::now() + Duration::from_secs(1),
        ));
        assert_eq!(app.sidebar_motion.value(), design::RAIL);
        assert!(!app.sidebar_motion.running(true));
        let _ = app.update(Message::Resized(iced::Size::new(760.0, 560.0)));
        let _ = app.update(Message::ToggleSidebar);
        assert!(app.sidebar_sheet);
        let _ = app.update(Message::DismissPanel);
        assert!(!app.sidebar_sheet);
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn tool_system_unknown_roles_never_trigger_metadata_only_welcome() {
        for role in [
            "Tool result",
            "Tool call",
            "System",
            "Developer",
            "future/required",
            "Permission/preset",
        ] {
            assert!(chat_role(role));
        }
        for role in ["permission/preset", "sandbox/mode", "approval/policy"] {
            assert!(!chat_role(role));
        }
    }
    #[test]
    fn visible_range_remains_bounded_for_nonfinite_offsets() {
        for (offset, viewport) in [
            (f32::INFINITY, 560.0),
            (f32::MAX, f32::MAX),
            (f32::NAN, 560.0),
            (-500.0, -10.0),
        ] {
            let range = visible_range(4096, offset, viewport);
            assert!(range.start <= range.end && range.end <= 4096);
        }
    }
    fn enter(modifiers: keyboard::Modifiers) -> text_editor::KeyPress {
        text_editor::KeyPress {
            key: keyboard::Key::Named(keyboard::key::Named::Enter),
            modified_key: keyboard::Key::Named(keyboard::key::Named::Enter),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Enter),
            modifiers,
            text: None,
            status: text_editor::Status::Focused { is_hovered: false },
        }
    }
    #[test]
    fn bare_enter_is_newline_and_ctrl_enter_requires_send_permission() {
        assert!(matches!(
            composer_binding(enter(keyboard::Modifiers::empty()), true),
            Some(text_editor::Binding::Enter)
        ));
        assert!(composer_binding(enter(keyboard::Modifiers::CTRL), false).is_none());
        assert!(matches!(
            composer_binding(enter(keyboard::Modifiers::CTRL), true),
            Some(text_editor::Binding::Custom(Message::Send))
        ));
    }
    #[test]
    fn modal_and_nonconversation_panels_cannot_submit_stale_editor() {
        let (mut app, mut receiver) = app();
        app.ready = true;
        app.root_ready = true;
        app.follow_ready = true;
        app.selected = Some(SessionId::new("fixture").unwrap());
        app.editor = text_editor::Content::with_text("must stay a draft");
        app.capabilities = true;
        let _ = app.update(Message::Send);
        assert!(receiver.try_recv().is_err());
        assert!(app.panel_stop_visible());
        app.capabilities = false;
        app.open_detail("Fixture".into(),"read-only".into(),None,false);
        assert!(app.panel_stop_visible());
        let _ = app.update(Message::Send);
        assert!(receiver.try_recv().is_err());
        app.detail = None;
        app.sidebar_sheet = true;
        assert!(app.panel_stop_visible());
        let _ = app.update(Message::Send);
        assert!(receiver.try_recv().is_err());
        app.sidebar_sheet = false;
        let _ = app.update(Message::Settings(crate::settings::Action::Open));
        // Opening settings explicitly may request metadata, never a prompt or save.
        while let Ok(command) = receiver.try_recv() {
            assert!(matches!(command, Command::KeyMetadata(_)));
        }
        assert!(!app.panel_stop_visible());
        let _ = app.update(Message::Send);
        assert!(receiver.try_recv().is_err());
        assert_eq!(app.editor.text(), "must stay a draft");
    }
    #[test]
    fn settings_modal_fits_logical_viewport_with_scrim_padding() {
        assert_eq!(
            settings_modal_size(iced::Size::new(1400.0, 1000.0)),
            iced::Size::new(960.0, 700.0)
        );
        assert_eq!(
            settings_modal_size(iced::Size::new(760.0, 560.0)),
            iced::Size::new(712.0, 512.0)
        );
        assert_eq!(
            settings_modal_size(iced::Size::new(608.0, 448.0)),
            iced::Size::new(560.0, 400.0)
        );
    }
    #[test]
    fn escape_settings_is_explicit_close_and_invalidates_old_editor_ticket() {
        let (mut app, mut receiver) = app();
        let _ = app.update(Message::Settings(crate::settings::Action::Open));
        let old = app.settings.input_ticket();
        assert!(app.settings_modal_visible());
        let _ = app.update(Message::DismissPanel);
        assert!(!app.settings.is_open());
        assert!(!app.settings_modal_visible());
        assert_ne!(app.settings.input_ticket(), old);
        let _ = app.update(Message::Settings(crate::settings::Action::Edit {
            ticket: old,
            input: crate::settings::Input::new("PUBLIC_MODAL_FAKE_ONLY".into()),
        }));
        assert!(!app.settings.is_open());
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn modal_scrim_and_queued_background_messages_never_act_or_dismiss() {
        let (mut app, mut receiver) = app();
        app.editor = text_editor::Content::with_text("keep composer draft");
        let _ = app.update(Message::Settings(crate::settings::Action::Open));
        let old = app.settings.input_ticket();
        for message in [
            Message::ModalBlocked,
            Message::Stop,
            Message::NewSession,
            Message::LoadModels,
            Message::ToggleSidebar,
            Message::ToggleRecords,
            Message::WorkspaceControls,
            Message::Filter("must not change".into()),
            Message::Send,
            Message::Editor(text_editor::Action::Edit(text_editor::Edit::Insert('x'))),
        ] {
            let _ = app.update(message);
        }
        assert!(app.settings_modal_visible());
        assert_eq!(app.settings.input_ticket(), old);
        assert_eq!(app.editor.text(), "keep composer draft");
        assert!(app.filter.is_empty());
        assert!(!app.workspace_controls);
        assert!(!app.composer_visible());
        assert!(!app.send_allowed());
        assert!(receiver.try_recv().is_err());
        // Close confirmation still has higher priority; Escape cannot dismiss it.
        app.close = Close::Confirm("public fixture".into());
        let _ = app.update(Message::DismissPanel);
        assert!(app.settings.is_open());
        assert!(!app.settings_modal_visible());
        assert!(matches!(app.close, Close::Confirm(_)));
    }
    #[test]
    fn modal_hidden_sidebar_has_no_animation_subscription() {
        let (mut app, _) = app();
        app.window_active = true;
        let _ = app.update(Message::ToggleSidebar);
        assert!(app.animation_active());
        let _ = app.update(Message::Settings(crate::settings::Action::Open));
        assert!(!app.animation_active());
    }
    #[test]
    fn raw_metadata_and_chat_share_one_exact_geometry() {
        let count = 4096;
        for offset in [0.0, 320.0, 15000.0, 500000.0] {
            let range = visible_range(count, offset, 560.0);
            assert!(range.len() <= 10);
            let above = range.start as f32 * ROW_HEIGHT;
            let inside = range.len() as f32 * ROW_HEIGHT;
            let below = (count - range.end) as f32 * ROW_HEIGHT;
            assert_eq!(above + inside + below, count as f32 * 160.0);
        }
    }
    #[test]
    fn excerpt_marks_line_limit_explicitly() {
        assert!(excerpt("one\ntwo\nthree\nfour\nfive").ends_with('…'));
    }
    fn managed_app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
        let (mut app, receiver) = app();
        let id = SessionId::new("public-managed-session").unwrap();
        app.ready = true;
        app.root_ready = true;
        app.roster_pending = false;
        app.transport_epoch = worker::TRANSPORT_EPOCH;
        app.sessions.insert(
            id.clone(),
            SessionSummary {
                agent_available: true,
                session_id: id.clone(),
                updated_at: 0,
                running: false,
                blank: true,
                parent_session_id: None,
                origin: None,
                cwd: None,
                projections: None,
            },
        );
        app.selected = Some(id);
        app.generation = 7;
        (app, receiver)
    }
    #[test]
    fn visible_settings_can_close_even_if_management_state_is_also_present() {
        let (mut app, mut receiver) = managed_app();
        assert!(app.management.baseline(vec![], vec![]));
        open_management(&mut app);
        assert!(app.management.is_open());
        let _ = app
            .settings
            .handle(crate::settings::Action::Open, app.transport_epoch, false);
        assert!(app.settings_modal_visible());
        let _ = app.update(Message::Settings(crate::settings::Action::Close));
        assert!(!app.settings.is_open());
        assert!(app.management.is_open());
        assert!(receiver.try_recv().is_err());
        let _ = app
            .settings
            .handle(crate::settings::Action::Open, app.transport_epoch, false);
        let _ = app.update(Message::DismissPanel);
        assert!(!app.settings.is_open());
        assert!(app.management.is_open());
        assert!(receiver.try_recv().is_err());
    }
    fn open_management(app: &mut App) {
        let view = app.management_context().view.unwrap();
        let _ = app.update(Message::Management(crate::management::Action::Open(view)));
    }
    #[test]
    fn management_stale_modal_messages_never_send_and_pin_ack_is_not_registry_state() {
        use crate::management::{Action, Operation};
        let (mut app, mut receiver) = managed_app();
        let stale = app.management_context().view.unwrap();
        app.generation += 1;
        let _ = app.update(Message::Management(Action::Open(stale)));
        assert!(!app.management.is_open());
        open_management(&mut app);
        let t = app.management.ticket().unwrap();
        let _ = app.update(Message::Management(Action::Review {
            ticket: t,
            operation: Operation::Pin,
        }));
        assert!(receiver.try_recv().is_err());
        let t = app.management.ticket().unwrap();
        let _ = app.update(Message::Management(Action::Confirm(t.clone())));
        let s = match receiver.try_recv().unwrap() {
            Command::Manage(s) => s,
            _ => panic!("unexpected command"),
        };
        let _ = app.update(Message::Management(Action::Confirm(t)));
        assert!(receiver.try_recv().is_err());
        let _ = app.receive(Event::Managed {
            submission: s,
            result: Ok(()),
        });
        assert!(
            !app.management
                .registry
                .pinned(app.selected.as_ref().unwrap())
        );
        let _ = app.receive(Event::Workspace(WorkspaceFollowFrame::Pinned {
            pinned_session_ids: vec![app.selected.clone().unwrap()],
        }));
        assert!(
            app.management
                .registry
                .pinned(app.selected.as_ref().unwrap())
        );
    }
    #[test]
    fn management_modal_and_archived_selection_cannot_submit_editor_or_force_stop() {
        use crate::management::{Action, Operation};
        let (mut app, mut receiver) = managed_app();
        app.follow_ready = true;
        app.editor = text_editor::Content::with_text("kept draft");
        open_management(&mut app);
        assert!(!app.animation_active());
        let _ = app.update(Message::Send);
        let _ = app.update(Message::Stop);
        let _ = app.update(Message::Settings(crate::settings::Action::Open));
        assert!(receiver.try_recv().is_err());
        assert!(!app.settings.is_open());
        app.management.dismiss();
        let id = app.selected.clone().unwrap();
        app.management.archives(vec![id]);
        assert!(!app.send_allowed());
        let _ = app.update(Message::ArchivedView);
        assert!(app.show_archived);
        open_management(&mut app);
        assert!(app.management.allows(Operation::Restore));
        let t = app.management.ticket().unwrap();
        let _ = app.update(Message::Management(Action::Review {
            ticket: t,
            operation: Operation::Restore,
        }));
        let _ = app.update(Message::Management(Action::Confirm(
            app.management.ticket().unwrap(),
        )));
        assert!(matches!(
            receiver.try_recv(),
            Ok(Command::Manage(crate::management::Submission {
                operation: Operation::Restore,
                ..
            }))
        ));
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn management_requires_real_roster_baseline_smoke_and_lifetime_authority() {
        use crate::management::Action;
        let (mut app, mut receiver) = managed_app();
        let view = app.management_context().view.unwrap();
        app.options.smoke = Some(crate::config::Smoke {
            exit_after: Duration::from_secs(1),
            report: "/unused-own-evidence".into(),
            screenshot: None,
        });
        let _ = app.update(Message::Management(Action::Open(view.clone())));
        assert!(!app.management.is_open());
        app.options.smoke = None;
        app.management.disconnect();
        let _ = app.update(Message::Management(Action::Open(view.clone())));
        assert!(!app.management.is_open());
        app.management.baseline(vec![], vec![]);
        app.sessions.clear();
        let _ = app.update(Message::Management(Action::Open(view)));
        assert!(!app.management.is_open());
        assert!(receiver.try_recv().is_err());
    }
    include!("export_ui_tests.rs");
    fn app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
        let (handle, feed, receiver, _events, _closing) = worker::test_channels();
        let options = crate::config::parse(
            [
                "--runtime",
                "/explicit-fork/apps/cli",
                "--expected-version",
                "0.2.1-alpha.1",
                "--home",
                "/isolated-native-home",
                "--user-home",
                "/user-home",
                "--cwd",
                "/tmp",
            ]
            .map(str::to_owned),
        )
        .unwrap()
        .unwrap();
        let (mut app, _) = App::boot(options, handle, feed);
        app.management.baseline(vec![], vec![]);
        (app, receiver)
    }
    #[test]
    fn account_observation_is_not_a_native_boot_or_render_dependency() {
        let (mut window, mut receiver) = app();
        assert_eq!(window.account, "Account metadata not loaded");
        let _ = window.receive(Event::Ready("0.2.1-alpha.1".into()));
        let _ = window.view();
        assert!(window.ready);
        assert_eq!(window.account, "Account metadata not loaded");
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn late_account_metadata_does_not_restore_disconnected_labels() {
        let stop = dsh_native_core::StopResult {
            exited: true,
            graceful: true,
            containment_unknown: false,
            observed_descendants_remaining: 0,
            exit_code: Some(0),
        };
        for terminal in [
            Event::Fault("PUBLIC fixture fault".into()),
            Event::Core(PublicEvent::Exited(stop)),
            Event::Stopped(Ok(stop)),
            Event::NoBackendStarted,
        ] {
            let (mut window, _) = app();
            let account = PublicEvent::Account(dsh_native_core::AccountMetadata {
                status: dsh_native_core::AccountStatus::CredentialStored,
                phase: None,
            });
            let _ = window.receive(Event::Core(account.clone()));
            assert_eq!(window.account, "Account metadata not loaded");
            let _ = window.receive(Event::Ready("0.2.1-alpha.1".into()));
            let _ = window.receive(Event::Core(account.clone()));
            assert_eq!(window.account, "Account: CredentialStored");
            let _ = window.receive(terminal);
            let _ = window.receive(Event::Core(account));
            assert_eq!(
                window.account,
                "Account metadata unavailable (disconnected)"
            );
        }
    }
    #[test]
    fn optional_account_loss_does_not_disconnect_or_request_recovery() {
        let (mut window, mut receiver) = app();
        let _ = window.receive(Event::Ready("0.2.1-alpha.1".into()));
        let account = PublicEvent::Account(dsh_native_core::AccountMetadata {
            status: dsh_native_core::AccountStatus::CredentialStored,
            phase: Some(dsh_native_core::AttemptPhase::Succeeded),
        });
        let _ = window.receive(Event::Core(account.clone()));
        assert_eq!(window.account, "Account: CredentialStored · Succeeded");
        let _ = window.receive(Event::Core(PublicEvent::Warning(
            dsh_native_core::Warning::AccountSubscriptionUnavailable,
        )));
        assert!(window.ready);
        assert_eq!(window.account, "Account metadata unavailable");
        assert!(receiver.try_recv().is_err());
        let _ = window.receive(Event::Core(account.clone()));
        assert_eq!(window.account, "Account: CredentialStored · Succeeded");
        window.close = Close::Stopping;
        let _ = window.receive(Event::Ready("0.2.1-alpha.1".into()));
        let _ = window.receive(Event::Core(PublicEvent::Account(
            dsh_native_core::AccountMetadata {
                status: dsh_native_core::AccountStatus::SignedOut,
                phase: None,
            },
        )));
        assert_eq!(window.account, "Account: CredentialStored · Succeeded");
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn plugins_actual_ui_routes_only_explicit_visible_ready_refresh() {
        use crate::settings::{Action, Page};
        let (mut app, mut receiver) = app();
        let _ = app.update(Message::Settings(Action::Open));
        let _ = app.update(Message::Settings(Action::SelectPage(Page::Plugins)));
        let old = app.settings.plugin_read_ticket();
        let _ = app.update(Message::Settings(Action::Plugins(
            crate::plugins::Action::Refresh(old),
        )));
        assert!(receiver.try_recv().is_err());
        let _ = app.receive(Event::Ready("0.2.1-alpha.1".into()));
        assert!(receiver.try_recv().is_err());
        let ticket = app.settings.plugin_read_ticket();
        let _ = app.update(Message::Settings(Action::Plugins(
            crate::plugins::Action::Refresh(ticket),
        )));
        let Command::PluginsRead(submitted) = receiver.try_recv().expect("explicit plugin read")
        else {
            panic!("wrong command")
        };
        assert_eq!(submitted.epoch, worker::TRANSPORT_EPOCH);
        let _ = app.receive(Event::PluginsLoaded {
            ticket: submitted,
            result: Err(crate::plugins::ReadFailure::Unavailable),
        });
        let _ = app.update(Message::Settings(Action::SelectPage(Page::General)));
        let _ = app.update(Message::Settings(Action::Plugins(
            crate::plugins::Action::Refresh(ticket),
        )));
        let _ = app.update(Message::Settings(Action::Close));
        let _ = app.update(Message::Settings(Action::Plugins(
            crate::plugins::Action::Refresh(ticket),
        )));
        assert!(receiver.try_recv().is_err());
        assert!(!app.smoke_evidence.catalog_requested);
        assert_eq!(app.smoke_evidence.model_prompts, 0);
    }
    #[test]
    fn plugin_actual_ui_queue_rejection_settles_read_without_auto_retry() {
        use crate::settings::{Action, Page};
        let (mut app, mut receiver) = app();
        let _ = app.update(Message::Settings(Action::Open));
        let _ = app.update(Message::Settings(Action::SelectPage(Page::Plugins)));
        let _ = app.receive(Event::Ready("0.2.1-alpha.1".into()));
        for _ in 0..32 {
            assert!(app.handle.send(Command::Inspect).is_ok());
        }
        let _ = app.update(Message::Settings(Action::Plugins(
            crate::plugins::Action::Refresh(app.settings.plugin_read_ticket()),
        )));
        for _ in 0..32 {
            assert!(matches!(receiver.try_recv(), Ok(Command::Inspect)));
        }
        assert!(receiver.try_recv().is_err());
        // A matching not-sent read failure releases the reducer; only another gesture retries.
        let _ = app.update(Message::Settings(Action::Plugins(
            crate::plugins::Action::Refresh(app.settings.plugin_read_ticket()),
        )));
        assert!(matches!(receiver.try_recv(), Ok(Command::PluginsRead(_))));
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn settings_opened_during_startup_rebinds_to_ready_epoch_without_auto_read() {
        let (mut app, mut receiver) = app();
        let _ = app.update(Message::Settings(crate::settings::Action::Open));
        assert!(app.settings.is_open());
        assert_eq!(app.settings.input_ticket().epoch, 0);
        assert!(receiver.try_recv().is_err());
        let _ = app.receive(Event::Ready("0.2.1-alpha.1".into()));
        assert!(app.settings.is_open());
        assert_eq!(app.settings.input_ticket().epoch, worker::TRANSPORT_EPOCH);
        assert!(app.settings_enabled());
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn fault_disables_settings_and_all_carrier_decisions() {
        let (mut app, _) = app();
        app.ready = true;
        app.root_ready = true;
        assert!(app.settings_enabled());
        let _ = app.receive(Event::Fault("Fixed fixture failure".into()));
        assert!(!app.settings_enabled());
        assert!(!app.decision_replies_enabled());
    }
    #[test]
    fn boot_never_requests_catalog_or_generation() {
        let (_app, mut receiver) = app();
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn unknown_inspection_requires_confirmation() {
        let (mut app, _) = app();
        app.close = Close::Inspecting;
        let _ = app.receive(Event::Inspection(Ok(Inspection {
            active_tasks: 0,
            scheduled_tasks: 0,
            unknown: true,
        })));
        assert!(matches!(app.close, Close::Confirm(_)));
    }
    #[test]
    fn send_is_typed_and_failed_acceptance_preserves_draft() {
        let (mut app, mut receiver) = app();
        app.ready = true;
        app.root_ready = true;
        app.follow_ready = true;
        app.selected = Some(SessionId::new("real-address").unwrap());
        assert!(app.management.baseline(vec![], vec![]));
        app.editor = text_editor::Content::with_text("hello 世界");
        let _ = app.update(Message::Send);
        assert!(
            matches!(receiver.try_recv(),Ok(Command::Prompt{request,..}) if request.client_time_zone.as_deref()==Some("UTC") && matches!(&request.content[0],PromptContentPart::Text{text} if text=="hello 世界"))
        );
        let _ = app.receive(Event::Prompt {
            generation: 0,
            result: Err("session/model-unavailable".into()),
        });
        assert_eq!(app.editor.text(), "hello 世界");
        assert!(app.pending.is_empty());
    }
    #[test]
    fn smoke_fences_user_generation_and_catalog_clicks() {
        let (mut app, mut receiver) = app();
        app.options.smoke = Some(crate::config::Smoke {
            exit_after: std::time::Duration::from_secs(8),
            report: std::path::PathBuf::from("/unused-own-evidence"),
            screenshot: None,
        });
        app.ready = true;
        app.root_ready = true;
        app.follow_ready = true;
        app.selected = Some(SessionId::new("smoke").unwrap());
        app.editor = text_editor::Content::with_text("must not send");
        let _ = app.update(Message::Send);
        let _ = app.update(Message::LoadModels);
        assert!(receiver.try_recv().is_err());
        assert_eq!(app.smoke_evidence.model_prompts, 0);
        assert!(!app.smoke_evidence.catalog_requested);
    }
    #[test]
    fn screenshot_schedules_only_once_after_real_fold_and_own_window() {
        let (mut app, _) = app();
        app.smoke_evidence.screenshot_requested = true;
        let _ = app.maybe_screenshot();
        assert!(!app.smoke_evidence.screenshot_scheduled);
        app.smoke_evidence.snapshot_valid = true;
        app.follow_ready = true;
        let _ = app.maybe_screenshot();
        assert!(!app.smoke_evidence.screenshot_scheduled);
        let id = window::Id::unique();
        app.own_window = Some(id);
        let _ = app.maybe_screenshot();
        assert!(app.smoke_evidence.screenshot_scheduled);
        let _ = app.update(Message::CaptureOwnWindow(id));
        assert!(app.smoke_evidence.screenshot_started);
        let _ = app.update(Message::CaptureOwnWindow(id));
        assert!(app.smoke_evidence.screenshot_saved.is_none());
    }
}
