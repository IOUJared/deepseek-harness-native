use crate::{
    config::Options,
    design,
    reducer::{self, DisplayRow, FileCard, Transcript},
    worker::{self, Command, Event, Feed, Handle},
};
use dsh_native_core::{Inspection, PublicEvent, StopResult};
use dsh_native_transport::dto::*;
use iced::widget::{
    Button, Column, Row, Space, button, column, container, rich_text, row, scrollable, span, text,
    text_editor, text_input,
};
use iced::{Color, Element, Length, Subscription, Task, Theme, keyboard, window};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
#[path = "accessibility.rs"]
mod accessibility;
#[path = "workspace_membership.rs"]
mod workspace_membership;
// Owned here so `ui::work_panel_ui` resolves in both the binary and the
// examples that compile this module by path.
#[path = "work_panel_ui.rs"]
mod work_panel_ui;
const ROW_HEIGHT: f32 = 160.0;
#[cfg(test)]
const ACTIVITY_HEIGHT: f32 = 40.0;
/// The palette is declared in [`crate::design`]; re-exported under this path
/// because sibling modules import their colours from `crate::ui`.
pub(crate) use crate::design::{ACCENT, BASE, DANGER, MUTED, OVERLAY, SURFACE, TEXT};
pub const FONT: iced::Font = iced::Font::with_name("DejaVu Sans");
/// The code font the web gives tool-body text and the diff totals
/// (`ToolRow.module.css:64-69` names `--ds-font-family-code`).
const MONO: iced::Font = iced::Font::with_name("DejaVu Sans Mono");
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
    name: String,
    label: String,
    efforts: Vec<Effort>,
    default_effort: Option<String>,
    service_tiers: Vec<model_controls::Choice>,
    verbosity_levels: Vec<model_controls::Choice>,
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
#[path = "anchored_popup.rs"]
mod anchored_popup;
#[path = "attachment_draft.rs"]
mod attachment_draft;
#[path = "attachment_picker.rs"]
mod attachment_picker;
#[path = "attachment_preview.rs"]
mod attachment_preview;
#[path = "attachment_ui.rs"]
mod attachment_ui;
#[cfg(test)]
#[path = "attachment_ui_tests.rs"]
mod attachment_ui_tests;
#[cfg(test)]
#[path = "chat_flow_ui_tests.rs"]
mod chat_flow_ui_tests;
#[path = "chat_usage.rs"]
pub(crate) mod chat_usage;
#[path = "command_menu.rs"]
mod command_menu;
#[cfg(test)]
#[path = "conversation_actions_ui_tests.rs"]
mod conversation_actions_ui_tests;
#[path = "conversation_content.rs"]
mod conversation_content;
#[cfg(test)]
#[path = "conversation_recovery_ui_tests.rs"]
mod conversation_recovery_ui_tests;
#[path = "conversation_search.rs"]
mod conversation_search;
#[path = "detail_panel.rs"]
mod detail_panel;
#[cfg(test)]
#[path = "detail_ui_tests.rs"]
mod detail_ui_tests;
#[cfg(test)]
#[path = "draft_lifecycle_ui_tests.rs"]
mod draft_lifecycle_ui_tests;
#[path = "draft_recovery.rs"]
mod draft_recovery;
#[path = "draft_recovery_ui.rs"]
mod draft_recovery_ui;
#[path = "draft_store.rs"]
mod draft_store;
#[path = "editor_menu.rs"]
mod editor_menu;
#[cfg(test)]
#[path = "mcp_ui_tests.rs"]
mod mcp_ui_tests;
#[path = "model_controls.rs"]
mod model_controls;
#[cfg(test)]
#[path = "model_metadata_ui_tests.rs"]
mod model_metadata_ui_tests;
#[path = "notifications.rs"]
mod notifications;
#[path = "plan_control.rs"]
mod plan_control;
#[path = "plan_mode.rs"]
mod plan_mode;
#[cfg(test)]
#[path = "prompt_recovery_ui_tests.rs"]
mod prompt_recovery_ui_tests;
#[path = "run_watch.rs"]
mod run_watch;
#[path = "sidebar_organization.rs"]
mod sidebar_organization;
#[path = "sidebar_ui.rs"]
mod sidebar_ui;
#[cfg(test)]
#[path = "sidebar_ui_tests.rs"]
mod sidebar_ui_tests;
#[path = "subagent_catalog.rs"]
mod subagent_catalog;
#[cfg(test)]
#[path = "terminal_ui_tests.rs"]
mod terminal_ui_tests;
#[cfg(test)]
#[path = "work_panel_ui_tests.rs"]
mod work_panel_ui_tests;
#[path = "working_indicator.rs"]
mod working_indicator;
#[path = "working_status.rs"]
mod working_status;
#[cfg(test)]
#[path = "working_ui_tests.rs"]
mod working_ui_tests;
#[derive(Clone)]
pub enum Message {
    /// An explicit file-open action. No native control publishes it: the composer
    /// offers [`Self::FileChoose`], and drops route through the picker. The
    /// composed file example and the tests drive the handler.
    #[allow(dead_code)]
    FileOpen,
    FileChoose,
    FilePicker(attachment_picker::Action),
    FilePickerRead(std::sync::Arc<attachment_picker::Completion>),
    FileDropped(window::Id, std::path::PathBuf),
    FileDropCancelled(window::Id),
    FilePreview(attachment_draft::EditorStamp),
    FilePreviewRead(
        attachment_ui::PreviewTicket,
        std::sync::Arc<attachment_preview::Outcome>,
    ),
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
    WorkingTick,
    Settings(crate::settings::Action),
    Management(crate::management::Action),
    Rename(crate::rename::Action),
    Fork(crate::fork::Action),
    ExportSession(crate::management::Ticket),
    Export(crate::exporter::Action),
    ArchivedView,
    Sidebar(sidebar_ui::Action),
    SidebarLoaded {
        ticket: u64,
        result: Result<draft_store::Drafts, draft_store::Failure>,
    },
    SidebarSaved {
        ticket: u64,
        result: Result<(), draft_store::Failure>,
    },
    Interaction(crate::interactions::Action),
    Worker(Event),
    CheckSubmission(prompt_recovery::Ticket),
    ReviewSubmission(prompt_recovery::Ticket),
    StopSubmission(prompt_recovery::Ticket),
    RejoinObservations,
    Editor(text_editor::Action),
    /// Opens or dismisses the editable-field context menu the shell installs
    /// (`apps/desktop/src/main.ts:276-300`).
    EditorMenu(bool),
    EditorMenuAction(editor_menu::Action),
    /// The hero preset chip's own press: a user action, and the roster read's only trigger.
    AgentPresetSeat,
    /// Opens or dismisses the jobs panel.
    ToggleJobs,
    /// The reference's `shortcuts.open` trigger (`primary + Slash`).
    ShortcutsOpen,
    /// Opens or closes the header's subagent catalog dropdown.
    ToggleSubagentMenu(bool),
    /// Opens one child session from that catalog.
    SubagentOpen(String),
    /// Expands or collapses one catalog row's own children.
    ToggleSubagentBranch(String),
    /// Expands or collapses the queue dock's multi-item list.
    ToggleQueue,
    /// Expands or collapses one turn-trigger notice.
    ToggleTrigger(u64),
    /// A key press as **data** for the live chain, with the state at the moment it arrives: an
    /// armed capture consumes it, a focused field's links own it, or a stored binding invokes a
    /// command — so a rebound key replaces the hardcoded default instead of shadowing it.
    KeyPressed(iced::keyboard::Key, iced::keyboard::Modifiers),
    /// Stops one job, after the reference's two-click confirmation.
    KillJob(String),
    /// Expands or collapses one job's live output.
    ToggleJobOutput(String),
    /// Chooses one advertised preset from the hero seat's list.
    AgentPresetPick(String),
    /// Opens or dismisses the permission chip's menu.
    TogglePermissionMenu,
    /// Sends one permission preset as the command line the reference's picker submits.
    PermissionPick(String),
    /// Persists one new-session permission default from the settings row.
    PermissionDefaultPick(String),
    /// Ticks or clears the risk acknowledgement.
    PermissionConfirmAcknowledge(bool),
    /// Cancels a pending full-access confirmation.
    PermissionConfirmCancel,
    /// Commits the confirmed preset.
    PermissionConfirmEnable,
    Send,
    KeyboardSend,
    /// Wayland composer paste: read the clipboard off the UI thread.
    PasteComposer,
    PasteComposerText(Option<String>),
    ModelControls(model_controls::Action),
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
    ToggleThoughts(conversation_content::Ticket),
    /// Expand or collapse one tool row's recorded body inside the conversation.
    ToggleActivity {
        generation: u64,
        key: u64,
    },
    CopyCode(conversation_content::Ticket, usize),
    CopyReply {
        generation: u64,
        key: u64,
    },
    EditReply {
        generation: u64,
        key: u64,
    },
    ConfirmEdit {
        generation: u64,
        key: u64,
        draft_serial: u64,
    },
    CancelEdit,
    DraftFlush,
    DraftReload,
    ReviewRecovered {
        generation: u64,
        serial: u64,
    },
    DraftOutcome(draft_recovery::Outcome),
    SearchOpen,
    SearchInput {
        generation: u64,
        serial: u64,
        query: String,
    },
    SearchNext {
        generation: u64,
        serial: u64,
        backwards: bool,
    },
    SearchClose {
        generation: u64,
        serial: u64,
    },
    Capabilities,
    /// Open (from the More menu) or dismiss the read-only work records panel.
    Work(work_panel_ui::Action),
    /// Expand or collapse the composer's context-management explanation.
    ToggleContextPolicy,
    /// Enter or leave plan mode through the Host's command surface.
    TogglePlanMode,
    /// Move the completion highlight by one row, wrapping.
    CommandMenuMove(i32),
    /// Accept the highlighted completion into the draft.
    CommandMenuAccept,
    /// Accept one named completion, which is how a pointer picks a row.
    CommandMenuPick(String),
    /// Close the completion menu without changing the draft.
    CommandMenuDismiss,
    /// A press inside the composer that owns no action of its own: the card and
    /// its metadata strip. It publishes nothing, and exists because iced's text
    /// editor drops focus on any press outside its own bounds; the press is the
    /// user staying in the composer, so the caret stays there too.
    ComposerFocus,
    /// Client-only view of Host-owned terminals. Opening spawns no local shell.
    Terminal(crate::terminal::Action),
    /// App-owned MCP composition: list, add, edit and disable. Only one reviewed
    /// Confirm writes, and it writes only inside the private native home.
    Mcp(crate::mcp::Action),
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
    /// One AT-SPI request: activation, deactivation or an action on a node.
    Accessibility(accessibility::Request),
    /// One accessibility measurement: widget bounds and the focused control.
    AccessibilityMeasured(accessibility::Measured),
    /// Poll the exposed state while a client is connected.
    AccessibilityTick,
    /// Copy the focused tagged text field, or paste into it. Iced's single-line
    /// text input has no copy or paste binding, so the app answers these itself.
    ClipboardCopyField,
    ClipboardPasteField,
    /// The focused tagged field, resolved by asking the widget tree.
    ClipboardFocusedField(Option<String>),
    /// A clipboard write finished; nothing to do, and deliberately so.
    ClipboardCopied,
    /// Clipboard text ready to insert into the focused tagged field.
    ClipboardPasteText(Option<String>),
    /// The focused tagged field and the text to insert into it.
    ClipboardPasteInto {
        id: Option<String>,
        text: String,
    },
    /// A named field (or the page's target) and the clipboard text for it.
    ClipboardPasteIntoField {
        id: Option<String>,
        text: Option<String>,
    },
    /// A conversation-width handle was pressed.
    /// Jump the transcript back to its live tail, as the reference's floating
    /// control does (`ChatView.tsx:288-292`).
    ScrollToBottom,
    WidthHandleHover(design::WidthSide, bool),
    WidthDragStart(design::WidthSide),
    /// A cursor sample while a width drag is in flight.
    WidthDragMove(f32),
    /// The drag finished: persist it, unless the pointer never actually moved.
    WidthDragEnd,
    /// The drag was abandoned: restore the pre-drag width and persist nothing.
    WidthDragCancel,
    /// A later launch of this profile asked the running window to come forward.
    FocusRequested,
    /// The transcript content size changed. Out-of-range values become the
    /// default, so a step can never leave the offered range.
    ContentSize(f32),
    /// The pointer entered a tool row; the web reveals that row's chevron here.
    ActivityHover(u64),
    /// The pointer left a tool row, restoring the tool's own variant glyph.
    ActivityHoverEnd(u64),
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
/// The panels that write through the worker's single management flight. Every one
/// of them refuses while any *other* flight is open or pending, which is the same
/// exclusivity `worker::Admission` enforces before it dispatches.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Flight {
    Management,
    Rename,
    Fork,
    Export,
    Terminal,
    Mcp,
}
impl Flight {
    /// Every flight, in the order the panels appear in the UI.
    const ALL: [Self; 6] = [
        Self::Management,
        Self::Rename,
        Self::Fork,
        Self::Export,
        Self::Terminal,
        Self::Mcp,
    ];
}
use crate::worker::prompt_recovery::{self, Pending};
/// Width-handle hit area, and its offset outside the conversation lane. The web
/// places a `10px` handle `24px` beyond the content edge, and renders neither
/// when the pane has no room for both.
const WIDTH_HANDLE_WIDTH: f32 = 10.0;
const WIDTH_HANDLE_OFFSET: f32 = 24.0;
/// The gap between a message and its action row, the action row's own height,
/// and the total a row must reserve. The web's user row is a right-aligned column
/// with a `6px` gap (`MessageItem.module.css` `.userRow`), and the geometry and
/// the view read these together so a reservation cannot drift from what is drawn.
const ACTION_GAP: f32 = 6.0;
/// The action row's own height. The web's `.actions` and `.action` are both
/// `calc(28px + var(--dsh-content-font-delta))` (`MessageIconActions.module.css`),
/// where native used 18.
const ACTION_HEIGHT: f32 = 28.0;
/// The gap between the actions themselves, which the web sets to 8px.
const ACTION_SPACING: f32 = 8.0;
/// The local wall-clock `HH:MM` the reference's `.timeStart` shows beside a user
/// message's actions (`MessageIconActions.module.css:13-19`). `None` when the
/// platform cannot resolve the zone, so the label is simply omitted.
fn local_hh_mm(millis: i64) -> Option<String> {
    let seconds = millis.div_euclid(1000);
    let time = libc::time_t::try_from(seconds).ok()?;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: `localtime_r` writes into the `tm` this frame owns and reads only
    // `time`; it is the thread-safe form and retains no pointer to either.
    let filled = unsafe { libc::localtime_r(&time, tm.as_mut_ptr()) };
    if filled.is_null() {
        return None;
    }
    let tm = unsafe { tm.assume_init() };
    Some(format!("{:02}:{:02}", tm.tm_hour, tm.tm_min))
}
/// Action targets are icon buttons, as the reference draws them (a copy mark and
/// a pencil), not words. `❐` and `✎` are the shapes DejaVu Sans actually has;
/// the web's own outline icons live in `ui-primitives/icons`.
const ACTION_COPY: &str = "❐";
const ACTION_EDIT: &str = "✎";
/// The web's `.action { padding: 6px }` around a 15px glyph.
const ACTION_PAD: f32 = 6.0;
/// The reference's action glyphs are 15px inside the 28px target, and the
/// assistant's completion row gives them two extra pixels
/// (`MessageIconActions.module.css:79-87`).
const ACTION_GLYPH: u32 = 15;
const ACTION_GLYPH_END: u32 = 17;
/// The reference's file card leads with a 28px `FileTypeIcon`. Drawn icons are
/// blocked (docs/NATIVE-UI-PARITY.md), so the `▤` glyph the tool rows already
/// use for a file stands in at the card's own icon box.
const ATTACHMENT_GLYPH: &str = "▤";
/// What a message row reserves for the gap plus the action row.
fn action_footer(delta: f32) -> f32 {
    ACTION_GAP + ACTION_HEIGHT + delta
}
/// True for the row the web draws as an Assistant response part, which takes
/// the wider flow gap around it (`ChatView.module.css:73-85`).
fn response_row(row: &DisplayRow) -> bool {
    row.role.starts_with("Assistant") || row.role == "Interrupted assistant attempt"
}
/// The web's leading gap for `rows[index]` (`ChatView.module.css:60-95`), read
/// by both the reserved height and the drawn padding so the two cannot drift.
/// The first row has none: the scroller's own `design::FLOW_INSET` is the top
/// inset.
fn flow_gap_before(rows: &[&DisplayRow], index: usize) -> f32 {
    if index == 0 {
        return 0.0;
    }
    if response_row(rows[index]) || response_row(rows[index - 1]) {
        design::RESPONSE_GAP
    } else {
        design::FLOW_GAP
    }
}
/// The right-aligned attachment cards a user message stacks above its bubble,
/// 8px apart. One fixed `240x64` card per file, as the reference's `.fileCard`
/// is fixed (`MessageItem.module.css:320-332`): its name and meta lines are
/// literals there, so no card measurement follows the content-size preference
/// and `design::attachment_height` reserves exactly `64` per card.
/// The composer's attachment rail. The reference lays staged attachments in a
/// horizontal, 10px-gapped row of 64px-tall items inside `padding: 2px 10px 0`
/// with a `-6px` bottom margin (`ComposerAttachments.module.css:1-8`,
/// `AttachmentRail.module.css:8-30`), each file a 240x64 `FileCard` whose 18px
/// round remove control sits 6px from its top-right corner
/// (`FileCard.module.css:1-40, 77-96`). The control is always visible here: Iced
/// would need per-card hover state to reveal it the way the reference does.
fn composer_rail(items: Vec<(FileCard, Message)>) -> Option<Element<'static, Message>> {
    if items.is_empty() {
        return None;
    }
    let mut cards = Row::new()
        .spacing(design::ATTACH_RAIL_GAP)
        .align_y(iced::Alignment::Center);
    for (file, remove) in items {
        let icon = container(label(ATTACHMENT_GLYPH, TEXT).size(18))
            .width(28)
            .height(28)
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center);
        let name = container(
            label(file.name, TEXT)
                .size(14)
                .line_height(iced::Pixels(22.0))
                .wrapping(iced::advanced::text::Wrapping::None),
        )
        .height(22)
        .clip(true);
        let meta = container(
            label(file.meta, MUTED)
                .size(12)
                .line_height(iced::Pixels(15.0))
                .wrapping(iced::advanced::text::Wrapping::None),
        )
        .height(15)
        .clip(true);
        // The card's 178px text column keeps the 28px icon and 12px paddings
        // inside the reference's 240px width.
        let body = container(row![icon, Column::new().push(name).push(meta)].spacing(10))
            .padding([8, 12])
            .align_y(iced::Alignment::Center)
            .width(design::ATTACHMENT_CARD_WIDTH)
            .height(design::ATTACHMENT_CARD_HEIGHT)
            .style(design::elevated_surface);
        let remove = container(
            button(label("×", TEXT).size(12))
                .on_press(remove)
                .style(design::fill_solid)
                .padding([0.0, 0.0])
                .width(design::ATTACH_RAIL_REMOVE_SIZE)
                .height(design::ATTACH_RAIL_REMOVE_SIZE),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::Alignment::End)
        .align_y(iced::Alignment::Start)
        .padding(design::ATTACH_RAIL_REMOVE_INSET);
        let mut card = iced::widget::Stack::new().push(body);
        card = card.push(remove);
        cards = cards.push(card);
    }
    Some(
        container(cards)
            .id("native-composer-rail")
            .padding(design::ATTACH_RAIL_PAD)
            .into(),
    )
}
fn attachment_cards(files: Vec<FileCard>) -> Element<'static, Message> {
    let mut cards = Column::new()
        .spacing(design::ATTACHMENT_CARD_GAP)
        .align_x(iced::Alignment::End);
    for file in files {
        let icon = container(label(ATTACHMENT_GLYPH, TEXT).size(18))
            .width(28)
            .height(28)
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center);
        // The reference ellipsizes both lines inside their 178px column; Iced
        // has no ellipsis, so the one-line boxes clip instead.
        let name = container(
            label(file.name, TEXT)
                .size(14)
                .wrapping(iced::advanced::text::Wrapping::None),
        )
        .height(22)
        .clip(true);
        let meta = container(
            label(file.meta, MUTED)
                .size(12)
                .wrapping(iced::advanced::text::Wrapping::None),
        )
        .height(15)
        .clip(true);
        cards = cards.push(
            container(
                row![icon, Column::new().push(name).push(meta)]
                    .spacing(10)
                    .align_y(iced::Alignment::Center),
            )
            .padding([8, 12])
            .align_y(iced::Alignment::Center)
            .width(design::ATTACHMENT_CARD_WIDTH)
            .height(design::ATTACHMENT_CARD_HEIGHT)
            .style(design::elevated_surface),
        );
    }
    cards.into()
}
/// One in-flight conversation-width drag.
struct WidthDrag {
    side: design::WidthSide,
    /// The resolved width the gesture started from.
    base: f32,
    /// The preference to restore when the gesture is cancelled or never moves: a
    /// press alone must not overwrite a wider preference with its window-clamped
    /// display width.
    saved: Option<f32>,
    /// First cursor sample after the press. A press carries no position, and the
    /// handle's own `on_move` is local to a 10px box, so the drag reads the
    /// window cursor instead.
    origin_x: Option<f32>,
    moved: bool,
}
pub struct App {
    pub options: Options,
    handle: Handle,
    feed: Feed,
    /// AT-SPI bridge. `None` in every run that must not touch the desktop's
    /// accessibility bus: fixtures, smoke runs and tests.
    accessibility: Option<accessibility::Bridge>,
    accessibility_source: accessibility::Source,
    /// Fingerprint of the last published tree, so an unchanged frame publishes
    /// nothing and a client's bus is not filled with duplicate updates.
    accessibility_published: Option<u64>,
    accessibility_measuring: bool,
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
    rename: crate::rename::Rename,
    fork: crate::fork::Fork,
    exporter: crate::exporter::Exporter,
    files: attachment_draft::Attachments,
    local_files: attachment_ui::LocalFiles,
    show_archived: bool,
    show_trash: bool,
    sidebar_organization: sidebar_organization::State,
    sidebar_store: Option<std::sync::Arc<std::sync::Mutex<draft_store::Store>>>,
    /// User-set conversation content width, or `None` for the adaptive clamp.
    /// The web persists this under `dsh.conversation.contentWidth`; native keeps
    /// it in the layout namespace and resolves it through `design::chat_width_with`.
    /// The store itself is opened per read/write rather than held, so there is no
    /// idle descriptor for a preference that changes rarely.
    chat_width_preference: Option<f32>,
    /// The home the layout preference is persisted under, or `None` in a smoke
    /// run, which must not touch the user's profile.
    native_home: Option<std::path::PathBuf>,
    /// In-flight width drag; `None` when no handle is held.
    width_drag: Option<WidthDrag>,
    /// Which width handle the pointer is over, for the reference's glow.
    width_hover: Option<design::WidthSide>,
    /// Focus requests from a later launch of this profile. Set by `main` after
    /// boot, because the instance claim is taken before the App exists.
    pub(crate) focus_source: crate::single_instance::Source,
    /// The tool row under the pointer, which decides whether its leading box
    /// shows the tool's variant glyph or the disclosure chevron.
    hovered_activity: Option<u64>,
    /// Forces every tool row to render as hovered. The public layout fixture sets
    /// this to capture the pointer-only glyph swap, which no keyboard or state
    /// path otherwise reaches.
    #[cfg(feature = "public-layout-fixture")]
    hover_all_activities: bool,
    /// Render the width handles' glow, so a capture shows an affordance that
    /// otherwise needs a pointer.
    width_handle_glow: bool,
    /// The transcript content size, defaulting to the web's own default.
    content_font_size: f32,
    /// What Enter/Send do while the agent is running: queue the next turn (default) or steer
    /// the running one. Persisted in the layout namespace.
    busy_enter: crate::settings::BusyEnter,
    /// The queue dock's multi-item list is open.
    queue_expanded: bool,
    /// Expanded turn-trigger notices, by record key.
    trigger_expanded: std::collections::BTreeSet<u64>,
    workspace_path: String,
    workspace_controls: bool,
    filter: String,
    selected: Option<SessionId>,
    generation: u64,
    transcript: Transcript,
    follow_ready: bool,
    editor: text_editor::Content,
    /// The shell's editable context menu, and the editor-owned history its Undo and
    /// Redo entries drive (`apps/desktop/src/main.ts:1018`).
    editor_menu: bool,
    /// The header's subagent catalog dropdown is open.
    subagent_menu: bool,
    /// Expanded catalog branches, by child session id; reset when the dropdown closes.
    subagent_expanded: std::collections::BTreeSet<String>,
    /// The permission chip's menu, opened by its own press.
    permission_menu: bool,
    /// The preset awaiting the reference's risk confirmation, and whether its
    /// acknowledgement is ticked. Full access never commits on one press.
    permission_confirm: Option<String>,
    permission_acknowledged: bool,
    /// The Host's advertised presets, read when the menu first opens; the design set
    /// stands in until the reply arrives.
    permission_catalog: Option<dsh_native_core::PermissionCatalog>,
    /// The deployment's new-session permission default, for the settings row. `None`
    /// until the descriptor answers, which is also how the row stays hidden when the
    /// deployment exposes no permission settings.
    permission_settings: Option<dsh_native_core::PermissionSettingsState>,
    /// The Host's agent-preset roster, once its read answers. `None` keeps every surface
    /// that reads it hidden rather than showing a guessed name.
    agent_roster: Option<dsh_native_core::AgentPresetRoster>,
    /// The hero preset seat's selection list is open.
    agent_seat_open: bool,
    /// The Host's refusal of the last pick, shown on the seat rather than dropped: a
    /// `agentPreset/locked` message means the session had already started.
    agent_seat_error: Option<String>,
    /// The session's background jobs, replaced wholesale by each list frame and updated per job
    /// by the follow stream. Empty means the Host reported none, which is a valid set.
    jobs: Vec<dsh_native_transport::dto::JobView>,
    /// The jobs panel is open behind the counted header action.
    jobs_open: bool,
    /// The job whose stop is armed: the reference confirms a kill on a second click
    /// (`ui-jobs/src/client/locales.ts:61-64`).
    jobs_armed: Option<String>,
    /// The Host's answer to the last kill, shown with the reference's own copy.
    jobs_kill_error: Option<String>,
    /// The job whose live output is expanded, and the buffer that output accumulates into.
    jobs_expanded: Option<String>,
    jobs_output: std::collections::BTreeMap<String, String>,
    /// A write is in flight; the row is busy until its acknowledgement lands.
    permission_default_saving: bool,
    editor_undo: Vec<String>,
    editor_redo: Vec<String>,
    draft_serial: u64,
    drafts: BTreeMap<SessionId, String>,
    pending: prompt_recovery::Controller,
    prompt_serial: u64,
    models: Vec<Model>,
    chosen_model: Option<Model>,
    chosen_effort: Option<Effort>,
    chosen_service_tier: Option<String>,
    chosen_text_verbosity: Option<String>,
    model_pending: bool,
    catalog_read: worker::catalog_read::State,
    model_controls: model_controls::State,
    model_selections: BTreeMap<SessionId, ModelSelection>,
    new_pending: bool,
    workspace_pending: bool,
    page_pending: bool,
    cancel_pending: bool,
    cancel_generation: Option<u64>,
    observations_lost: bool,
    rejoin_pending: bool,
    offset: f32,
    viewport: f32,
    follow_bottom: bool,
    detail: Option<detail_panel::Detail>,
    detail_opening: Option<u64>,
    capabilities: bool,
    conversation_menu: bool,
    /// The composer's context-management disclosure is expanded. Presentation
    /// only: it explains the harness's staged policy and starts no request.
    context_policy_open: bool,
    /// Whether the Host has published the `plan` projection: `None` until the
    /// selected session reports, so an offer is never assumed before it answers.
    plan_published: Option<bool>,
    /// The Host's advertised commands for the selected session, or `None` until
    /// the composer asks. Only ever the Host's own list.
    commands: Option<Vec<command_menu::Candidate>>,
    /// A completion read is in flight, so the composer asks once, not per key.
    commands_pending: bool,
    /// Highlighted completion row.
    command_selected: usize,
    /// Tracks the selected session's run state so a settle is announced once.
    run_watch: run_watch::Watch,
    /// A plan-mode toggle is in flight. Inert until its reply settles.
    plan_toggle_pending: bool,
    /// The read-only More → Work records panel is open.
    work_open: bool,
    /// The More → Terminal panel is open. The Host owns every shell; the app is a client.
    terminal: crate::terminal::Terminal,
    /// The More → MCP servers panel is open. It is app-global (the composition
    /// file belongs to the private home), not session-scoped.
    mcp: crate::mcp::Mcp,
    info_expanded: u8,
    settings: crate::settings::Settings,
    decisions: crate::interactions::Decisions,
    transport_epoch: u64,

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
    working_phase: u8,
    /// The first Escape of the reference's two-press stop sequence: its deadline, the session and
    /// the generation the press addressed. `None` when no first press is pending.
    stop_sequence: Option<(Instant, SessionId, u64)>,
    chat_geometry: chat_geometry::Cache,
    reply_content: conversation_content::Cache,
    thoughts_expanded: std::collections::BTreeSet<u64>,
    activity_expanded: std::collections::BTreeSet<u64>,
    edit_review: Option<(u64, u64, u64)>,
    search_open: bool,
    search_query: String,
    search_serial: u64,
    search_cursor: Option<u64>,
    draft_state: draft_recovery::State,
    draft_store: Option<std::sync::Arc<std::sync::Mutex<draft_store::Store>>>,
    draft_exit_requested: bool,
    draft_discard_on_exit: bool,
    draft_recovered: std::collections::BTreeSet<SessionId>,
    draft_review: Option<(u64, u64)>,
    draft_versions: std::collections::BTreeMap<SessionId, u64>,
    scroll_revision: Option<u64>,
    scroll_feedback: Option<u64>,
    scroll_target: Option<f32>,
}
impl App {
    pub fn boot(options: Options, handle: Handle, feed: Feed) -> (Self, Task<Message>) {
        let draft_store = if options.smoke.is_none() {
            draft_store::Store::new(options.backend.native_home.clone())
                .ok()
                .map(|store| std::sync::Arc::new(std::sync::Mutex::new(store)))
        } else {
            None
        };
        let sidebar_store = if options.smoke.is_none() {
            draft_store::Store::sidebar(options.backend.native_home.clone())
                .ok()
                .map(|store| std::sync::Arc::new(std::sync::Mutex::new(store)))
        } else {
            None
        };
        // Layout preferences are read once at boot and the store is released
        // again: an absent, unreadable or malformed document leaves the adaptive
        // width in place rather than failing startup, because a missing
        // preference has a correct default.
        let layout_preferences = if options.smoke.is_none() {
            draft_store::Store::layout(options.backend.native_home.clone())
                .ok()
                .and_then(|mut store| store.load().ok())
        } else {
            None
        };
        // One read, two preferences: each falls back to its own default when the
        // key is absent or unreadable.
        let stored = |key: &str| -> Option<&str> {
            layout_preferences
                .as_ref()?
                .iter()
                .find(|(entry, _)| entry.as_str() == key)
                .map(|(_, raw)| raw.as_str())
        };
        let chat_width_preference =
            stored(draft_store::LAYOUT_CONTENT_WIDTH).and_then(draft_store::parse_layout_width);
        let content_font_size = stored(draft_store::LAYOUT_CONTENT_SIZE)
            .and_then(draft_store::parse_layout_size)
            .map_or(design::CONTENT_FONT_DEFAULT, design::content_size);
        let busy_enter = stored(draft_store::LAYOUT_BUSY_ENTER)
            .and_then(crate::settings::BusyEnter::parse)
            .unwrap_or_default();
        // Captured here, before `options` is consumed: a later field initializer
        // cannot borrow it. Smoke runs keep `None` so they never write a profile.
        let native_home = options
            .smoke
            .is_none()
            .then(|| options.backend.native_home.clone());
        let workspace_path = options
            .backend
            .working_directory
            .to_string_lossy()
            .into_owned();
        let (accessibility, accessibility_source) = if options.accessibility {
            let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
            (
                Some(accessibility::Bridge::new(sender)),
                accessibility::Source::new(receiver),
            )
        } else {
            (None, accessibility::Source::disabled())
        };
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
                accessibility,
                accessibility_source,
                accessibility_published: None,
                accessibility_measuring: false,
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
                rename: crate::rename::Rename::default(),
                fork: crate::fork::Fork::default(),
                exporter: crate::exporter::Exporter::default(),
                files: attachment_draft::Attachments::default(),
                local_files: Default::default(),
                show_archived: false,
                show_trash: false,
                sidebar_organization: Default::default(),
                sidebar_store,
                chat_width_preference,
                native_home,
                width_drag: None,
                width_hover: None,
                focus_source: crate::single_instance::Source::disabled(),
                hovered_activity: None,
                #[cfg(feature = "public-layout-fixture")]
                hover_all_activities: false,
                width_handle_glow: false,
                content_font_size,
                busy_enter,
                queue_expanded: false,
                trigger_expanded: Default::default(),
                workspace_path,
                workspace_controls: false,
                filter: String::new(),
                selected: None,
                generation: 0,
                transcript: Transcript::new(0),
                follow_ready: false,
                editor: text_editor::Content::new(),
                editor_menu: false,
                subagent_menu: false,
                subagent_expanded: Default::default(),
                permission_menu: false,
                permission_confirm: None,
                permission_acknowledged: false,
                permission_catalog: None,
                permission_settings: None,
                agent_roster: None,
                agent_seat_open: false,
                agent_seat_error: None,
                jobs: Vec::new(),
                jobs_open: false,
                jobs_armed: None,
                jobs_kill_error: None,
                jobs_expanded: None,
                jobs_output: std::collections::BTreeMap::new(),
                permission_default_saving: false,
                editor_undo: Vec::new(),
                editor_redo: Vec::new(),
                draft_serial: 0,
                drafts: BTreeMap::new(),
                pending: prompt_recovery::Controller::default(),
                prompt_serial: 0,
                models: vec![],
                chosen_model: None,
                chosen_effort: None,
                chosen_service_tier: None,
                chosen_text_verbosity: None,
                model_pending: false,
                catalog_read: worker::catalog_read::State::default(),
                model_controls: Default::default(),
                model_selections: BTreeMap::new(),
                new_pending: false,
                workspace_pending: false,
                page_pending: false,
                cancel_pending: false,
                cancel_generation: None,
                observations_lost: false,
                rejoin_pending: false,
                offset: 0.0,
                viewport: 480.0,
                follow_bottom: true,
                detail: None,
                detail_opening: Some(0),
                capabilities: false,
                conversation_menu: false,
                work_open: false,
                context_policy_open: false,
                plan_published: None,
                plan_toggle_pending: false,
                commands: None,
                commands_pending: false,
                command_selected: 0,
                run_watch: run_watch::Watch::default(),
                terminal: crate::terminal::Terminal::default(),
                mcp: crate::mcp::Mcp::default(),
                info_expanded: 0,
                settings: Default::default(),
                decisions: Default::default(),
                transport_epoch: 0,

                title_updates: BTreeMap::new(),
                close: Close::None,
                last_stop: None,
                smoke_evidence: crate::smoke::Evidence {
                    screenshot_requested,
                    ..Default::default()
                },
                own_window: None,
                window_size: iced::Size::new(design::WINDOW_WIDTH, design::WINDOW_HEIGHT),
                window_active: false,
                sidebar_expanded: true,
                sidebar_sheet: false,
                records_expanded: false,
                sidebar_motion: design::SidebarMotion::new(design::SIDEBAR),
                working_phase: 0,
                stop_sequence: None,
                chat_geometry: Default::default(),
                reply_content: Default::default(),
                thoughts_expanded: Default::default(),
                activity_expanded: Default::default(),
                edit_review: None,
                search_open: false,
                search_query: String::new(),
                search_serial: 0,
                search_cursor: None,
                draft_state: Default::default(),
                draft_store,
                draft_exit_requested: false,
                draft_discard_on_exit: false,
                draft_recovered: Default::default(),
                draft_review: None,
                draft_versions: Default::default(),
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
            Subscription::run_with(
                self.accessibility_source.clone(),
                accessibility::Source::stream,
            )
            .map(Message::Accessibility),
            // While a client is connected, re-examine the exposed state at a
            // bounded rate instead of making every update path remember to
            // publish. Publication itself still only happens on a real change.
            if self
                .accessibility
                .as_ref()
                .is_some_and(accessibility::Bridge::live)
            {
                iced::time::every(Duration::from_millis(200)).map(|_| Message::AccessibilityTick)
            } else {
                Subscription::none()
            },
            window::events().filter_map(|(id, event)| match event {
                window::Event::CloseRequested => Some(Message::Close),
                window::Event::Opened { .. } => Some(Message::NativeOpened(id)),
                window::Event::Resized(size) => Some(Message::Resized(size)),
                window::Event::FileDropped(path) => Some(Message::FileDropped(id, path)),
                window::Event::FilesHoveredLeft => Some(Message::FileDropCancelled(id)),
                window::Event::Focused => Some(Message::WindowActive(true)),
                window::Event::Unfocused | window::Event::Closed => {
                    Some(Message::WindowActive(false))
                }
                _ => None,
            }),
            // A width drag follows the window cursor, not the handle: a handle's
            // own `on_move` is local to its 10px box and stops reporting the
            // moment the pointer leaves it, which a drag always does. The stream
            // is gated on an in-flight drag so idle cost stays at nothing, and
            // `with` keeps the subscription in step as the gesture starts and ends.
            iced::event::listen()
                .with(self.width_drag.is_some())
                .filter_map(|(dragging, event)| {
                    if !dragging {
                        return None;
                    }
                    match event {
                        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                            Some(Message::WidthDragMove(position.x))
                        }
                        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                            iced::mouse::Button::Left,
                        )) => Some(Message::WidthDragEnd),
                        // Escape abandons the gesture and persists nothing.
                        iced::Event::Keyboard(keyboard::Event::KeyPressed {
                            key: keyboard::Key::Named(keyboard::key::Named::Escape),
                            ..
                        }) => Some(Message::WidthDragCancel),
                        _ => None,
                    }
                }),
            // A later launch of this profile could not take the lock and asked us
            // to come forward instead, which is what the desktop shell does.
            Subscription::run_with(
                self.focus_source.clone(),
                crate::single_instance::Source::stream,
            )
            .map(|_| Message::FocusRequested),
            // Every press no widget claimed is carried as data to `update`, the one place that
            // reads live state: an armed capture, a focused field's paste/copy, the
            // `shortcuts.open` trigger and the stored binding chain all decide there. Deciding
            // in this closure would capture state that goes stale the moment a panel opens.
            keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    repeat: false,
                    ..
                } => Some(Message::KeyPressed(key, modifiers)),
                _ => None,
            }),
            animation,
            if self.draft_state.ready()
                && self.draft_state.dirty()
                && !self.draft_state.busy()
                && self.draft_state.failure().is_none()
                && matches!(self.close, Close::None)
            {
                iced::time::every(Duration::from_millis(750)).map(|_| Message::DraftFlush)
            } else {
                Subscription::none()
            },
            if self.working_animation_active() {
                iced::time::every(Duration::from_millis(100)).map(|_| Message::WorkingTick)
            } else {
                Subscription::none()
            },
        ])
    }
    /// The ticket that scopes clipboard routing to the page that owns the tagged
    /// fields, or `None` when that page is not the one on screen.
    fn clipboard_field_ticket(&self) -> Option<crate::settings::Ticket> {
        self.settings.cloudflare_field_ticket()
    }
    /// Ask the widget tree which tagged field has focus. The answer arrives as a
    /// message, so nothing here blocks the update loop.
    fn resolve_focused_field(
        &self,
        then: impl FnOnce(Option<String>) -> Message + Send + 'static,
    ) -> Task<Message> {
        let fields: Vec<&'static str> = crate::settings::cloudflare::Field::ALL
            .iter()
            .map(|field| field.id())
            .collect();
        iced::advanced::widget::operate(FocusedField {
            fields,
            found: None,
            then: std::sync::Mutex::new(Some(then)),
        })
    }
    fn copy_focused_field(&self) -> Task<Message> {
        self.resolve_focused_field(Message::ClipboardFocusedField)
    }
    /// Copying answers with the field's whole value: iced exposes no selection
    /// for a single-line input, so there is nothing finer to copy.
    fn copy_field_text(&self, id: Option<String>) -> Task<Message> {
        let Some(field) = id
            .as_deref()
            .and_then(crate::settings::cloudflare::Field::of)
        else {
            return Task::none();
        };
        let Some(cloudflare) = self.settings.cloudflare_fields() else {
            return Task::none();
        };
        self.write_clipboard(field.value(cloudflare).to_owned())
    }
    /// Pushes the editor's current text onto the undo stack and drops any redo
    /// branch, so the menu reflects what the editor actually did.
    fn record_editor_edit(&mut self) {
        const HISTORY: usize = 100;
        self.editor_redo.clear();
        self.editor_undo.push(self.editor.text());
        if self.editor_undo.len() > HISTORY {
            self.editor_undo.remove(0);
        }
    }
    /// Restores text into the editor. Iced's `Content` has no text-plus-cursor
    /// restore, so the caret lands where `with_text` puts it; the menu still unfocuses
    /// afterwards, and the next click or keystroke re-seats it.
    fn set_editor_text(&mut self, text: &str) {
        self.editor = text_editor::Content::with_text(text);
        self.draft_serial = self.draft_serial.saturating_add(1);
        self.edit_review = None;
        self.touch_draft();
    }
    /// Puts text on the clipboard, from whichever thread the seat-bound backend
    /// requires. Shared by the settings fields and the editor's Cut/Copy entries.
    fn write_clipboard(&self, text: String) -> Task<Message> {
        if wayland_session() {
            // The write happens on a worker thread, like the read, because the
            // seat-bound backend cannot serve it from this one.
            return Task::perform(
                async move {
                    std::thread::spawn(move || native_clipboard::write_text(text));
                },
                |()| Message::ClipboardCopied,
            );
        }
        iced::clipboard::write(text)
    }
    /// Read the clipboard for one named field, or for the page's own target.
    fn read_clipboard_into(
        &mut self,
        field: Option<crate::settings::cloudflare::Field>,
    ) -> Task<Message> {
        if self.clipboard_field_ticket().is_none() {
            self.settings.cloudflare_clipboard_closed();
            return Task::none();
        }
        let id = field.map(|field| field.id().to_owned());
        if wayland_session() {
            return Task::perform(
                async {
                    let (sender, receiver) = tokio::sync::oneshot::channel();
                    std::thread::spawn(move || {
                        let _ = sender.send(native_clipboard::read_text());
                    });
                    receiver.await.ok().flatten()
                },
                move |text| Message::ClipboardPasteIntoField { id, text },
            );
        }
        iced::clipboard::read().map(move |text| Message::ClipboardPasteIntoField {
            id: id.clone(),
            text,
        })
    }
    fn read_clipboard_for_field(&mut self) -> Task<Message> {
        if self.clipboard_field_ticket().is_none() {
            return Task::none();
        }
        if wayland_session() {
            // Iced's Wayland clipboard read needs a seat serial it does not have
            // here, so the read happens on a worker thread.
            return Task::perform(
                async {
                    let (sender, receiver) = tokio::sync::oneshot::channel();
                    std::thread::spawn(move || {
                        let _ = sender.send(native_clipboard::read_text());
                    });
                    receiver.await.ok().flatten()
                },
                Message::ClipboardPasteText,
            );
        }
        iced::clipboard::read().map(Message::ClipboardPasteText)
    }
    fn paste_text_into_field(&mut self, text: Option<String>) -> Task<Message> {
        // A copied ID usually arrives with a trailing newline, which every field
        // validator here refuses, so take the first line and trim it.
        let text = text.map(|text| text.trim().to_owned());
        if self.clipboard_field_ticket().is_none() {
            self.settings.cloudflare_clipboard_closed();
            return Task::none();
        }
        let Some(text) = text.filter(|text| !text.is_empty()) else {
            // Saying nothing here was the last silent failure: the read is what
            // failed, and the page now says so.
            self.settings.cloudflare_clipboard_empty();
            return Task::none();
        };
        // No focus lookup on this path: a lookup that never answers would drop
        // the paste silently, and the page's own target is deterministic.
        self.update(Message::ClipboardPasteInto { id: None, text })
    }
    /// Paste for a named field, or the page's target, and say what happened.
    fn paste_into_named_field(
        &mut self,
        id: Option<String>,
        text: Option<String>,
    ) -> Task<Message> {
        let Some(ticket) = self.clipboard_field_ticket() else {
            return Task::none();
        };
        let Some(text) = text
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
        else {
            self.settings.cloudflare_clipboard_empty();
            return Task::none();
        };
        let Some(cloudflare) = self.settings.cloudflare_fields() else {
            return Task::none();
        };
        let field = id
            .as_deref()
            .and_then(crate::settings::cloudflare::Field::of)
            .or_else(|| cloudflare.paste_target());
        let Some(field) = field else {
            self.settings.cloudflare_clipboard_empty();
            return Task::none();
        };
        let current = field.value(cloudflare).to_owned();
        let characters = text.chars().count();
        self.settings.cloudflare_clipboard_result(field, characters);
        self.update(Message::Settings(crate::settings::Action::Cloudflare(
            field.paste(ticket, &current, &text),
        )))
    }
    fn paste_into_focused_field(&mut self, id: Option<String>, text: String) -> Task<Message> {
        let Some(ticket) = self.clipboard_field_ticket() else {
            return Task::none();
        };
        let Some(cloudflare) = self.settings.cloudflare_fields() else {
            return Task::none();
        };
        // The caret decides first; when the tree cannot name it, the field the
        // user last edited does, and an untouched page starts at the client ID.
        let field = id
            .as_deref()
            .and_then(crate::settings::cloudflare::Field::of)
            .or_else(|| cloudflare.paste_target());
        let Some(field) = field else {
            return Task::none();
        };
        let current = field.value(cloudflare).to_owned();
        self.update(Message::Settings(crate::settings::Action::Cloudflare(
            field.paste(ticket, &current, &text),
        )))
    }
    /// One AT-SPI request. An action is answered with the very message a click
    /// sends, so consent, single-flight and panel guards are the same ones.
    fn accessibility_request(&mut self, request: accessibility::Request) -> Task<Message> {
        match request {
            accessibility::Request::Activate => {
                // The client has no tree yet; publish one even if the exposed
                // state happens to match the last fingerprint.
                self.accessibility_published = None;
                self.accessibility_tick()
            }
            accessibility::Request::Action(request) => match accessibility::intent(&request) {
                Some(accessibility::Intent::Send) => self.update(Message::Send),
                Some(accessibility::Intent::Stop) => self.update(Message::Stop),
                Some(accessibility::Intent::Attach) => self.update(Message::FileChoose),
                Some(accessibility::Intent::Settings) => {
                    self.update(Message::Settings(crate::settings::Action::Open))
                }
                Some(accessibility::Intent::Model) => {
                    self.update(Message::ModelControls(model_controls::Action::Toggle))
                }
                Some(accessibility::Intent::FocusComposer) => {
                    iced::widget::operation::focus(accessibility::WIDGET_COMPOSER)
                }
                Some(accessibility::Intent::SetComposer(text)) => {
                    self.editor = text_editor::Content::with_text(&text);
                    self.touch_draft();
                    Task::none()
                }
                None => Task::none(),
            },
        }
    }
    /// Measure once while a client is connected, so an unattended app pays nothing.
    fn accessibility_tick(&mut self) -> Task<Message> {
        let Some(bridge) = &self.accessibility else {
            return Task::none();
        };
        if !bridge.live() || self.accessibility_measuring {
            return Task::none();
        }
        self.accessibility_measuring = true;
        accessibility::measure(Message::AccessibilityMeasured)
    }
    /// Publish only what changed: a client is told about a new state, not about
    /// every frame. The bounds are part of the comparison, so a resize or a panel
    /// change republishes even when no exposed sentence changed.
    fn accessibility_measured(&mut self, measured: accessibility::Measured) -> Task<Message> {
        self.accessibility_measuring = false;
        let fingerprint = self.accessibility_fingerprint(&measured);
        if self.accessibility_published == Some(fingerprint) {
            return Task::none();
        }
        let snapshot = self.accessibility_snapshot(&measured);
        let tree = accessibility::tree(&snapshot);
        if let Some(bridge) = &mut self.accessibility {
            bridge.publish(tree);
        } else {
            return Task::none();
        }
        self.accessibility_published = Some(fingerprint);
        Task::none()
    }
    /// The state one published tree describes. Layout decides presence: the
    /// composer and its controls are exposed only while the composer is on
    /// screen, which is also what keeps the API-key draft out of the tree.
    fn accessibility_snapshot<'a>(
        &'a self,
        measured: &'a accessibility::Measured,
    ) -> accessibility::Snapshot<'a> {
        let composer = self.composer_visible().then(|| self.editor.text());
        let model = composer.as_ref().map(|_| self.model_caption());
        let status = (!routine_status(&self.status)).then(|| self.status.clone());
        accessibility::Snapshot {
            composer,
            send: self.send_allowed(),
            stop: self.stop_allowed(),
            attach: self.file_context().allowed,
            model,
            status,
            bounds: &measured.bounds,
            focus: measured.focus,
        }
    }
    fn accessibility_fingerprint(&self, measured: &accessibility::Measured) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.editor.text().hash(&mut hasher);
        self.send_allowed().hash(&mut hasher);
        self.stop_allowed().hash(&mut hasher);
        self.file_context().allowed.hash(&mut hasher);
        self.composer_visible().hash(&mut hasher);
        self.model_caption().hash(&mut hasher);
        self.status.hash(&mut hasher);
        measured.focus.hash(&mut hasher);
        for (node, rectangle) in &measured.bounds {
            node.hash(&mut hasher);
            for value in [rectangle.x, rectangle.y, rectangle.width, rectangle.height] {
                value.to_bits().hash(&mut hasher);
            }
        }
        hasher.finish()
    }
    fn layout_size(&self) -> iced::Size {
        // Iced's Resized and get_size already include native * application scale.
        self.window_size
    }
    fn compact(&self) -> bool {
        self.layout_size().width < design::BREAKPOINT
    }
    /// The conversation pane: the window minus the animated sidebar, before its
    /// horizontal padding.
    fn content_column(&self) -> f32 {
        (self.layout_size().width - self.sidebar_motion.value()).max(0.0)
    }
    /// Shared transcript/composer column width. The web clamps the content to
    /// `clamp(680, 64% of the pane, 920)` — or to a set preference — and never
    /// exceeds the pane's padded content box. Geometry and rendered `max_width`
    /// must use this one value.
    fn chat_lane(&self) -> f32 {
        let column = self.content_column();
        // The web's transcript column is capped at `--dsh-chat-content-width`
        // and inset by `TRANSCRIPT_INSET` per side inside the scroller.
        (column - 2.0 * design::TRANSCRIPT_INSET)
            .min(self.chat_cap())
            .max(0.0)
    }
    /// The web's `--dsh-chat-content-width`: the adaptive clamp or the stored
    /// preference, before any pane inset. The composer's card rides this value
    /// plus its own 32px overhang (`ConversationRoot.module.css:381-390`).
    fn chat_cap(&self) -> f32 {
        design::chat_width_with(self.content_column(), self.chat_width_preference)
    }
    /// Write the bounded crash report for a fatal backend fault.
    /// Silent by design, like the reference: the report is a file under
    /// `<home>/logs` and the in-window status keeps showing the cause. Smoke and
    /// keyless runs hold no native home and therefore write nothing.
    fn record_start_failure(&self, cause: &str) {
        let Some(home) = self.native_home.as_ref() else {
            return;
        };
        let _ = crate::crash_log::write(
            home,
            &crate::crash_log::Record {
                source: "host-start",
                summary: cause,
                detail: cause,
            },
        );
    }
    /// Persist the layout preference. Reached only from an explicit gesture
    /// commit, never per frame, so the small synchronous write stays bounded.
    fn save_layout_preference(&self) {
        let Some(home) = self.native_home.clone() else {
            return;
        };
        let Ok(mut store) = draft_store::Store::layout(home) else {
            return;
        };
        let mut entries = store.load().unwrap_or_default();
        // Each preference writes or clears its own reserved key, so clearing one
        // never disturbs the other.
        for (key, raw) in [
            (
                draft_store::LAYOUT_CONTENT_WIDTH,
                self.chat_width_preference
                    .and_then(draft_store::format_layout_width),
            ),
            (
                draft_store::LAYOUT_CONTENT_SIZE,
                if self.content_font_size == design::CONTENT_FONT_DEFAULT {
                    None
                } else {
                    draft_store::format_layout_size(self.content_font_size)
                },
            ),
            (
                draft_store::LAYOUT_BUSY_ENTER,
                self.busy_enter.stored().map(str::to_owned),
            ),
        ] {
            let Ok(entry) = SessionId::new(key) else {
                continue;
            };
            match raw {
                Some(value) => {
                    entries.insert(entry, value);
                }
                None => {
                    entries.remove(&entry);
                }
            }
        }
        let _ = store.save(&entries);
    }
    /// The bottom fade that dissolves scrolled content before the composer. It is
    /// an overlay rather than a sibling strip, so it costs no layout height and
    /// cannot shorten the lane or change what the virtualizer sees.
    fn composer_fade(&self) -> Element<'_, Message> {
        container(
            container(
                Space::new()
                    .width(Length::Fill)
                    .height(design::COMPOSER_FADE_HEIGHT),
            )
            .style(design::composer_fade)
            .width(Length::Fill)
            .height(design::COMPOSER_FADE_HEIGHT),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(iced::Alignment::End)
        .into()
    }
    /// The two width handles, overlaid on the transcript so neither the lane nor
    /// the scroll model changes. Only the handles are interactive; the spacers
    /// between them are not, so the transcript underneath still receives input.
    fn width_handles(&self, lane: f32) -> Element<'_, Message> {
        // The reference's glow is a 2px bar 4px in from the strip's inner edge,
        // drawn only while hovering or dragging, and positioned at the pointer's
        // Y with a 50% fallback (`ConversationRoot.module.css:289-317`). Native
        // draws the solid core (Iced has no gradient) at that fallback.
        let handle = |side| {
            let lit = self.width_hover == Some(side)
                || self
                    .width_drag
                    .as_ref()
                    .is_some_and(|drag| drag.side == side)
                || self.width_handle_fixture_glow();
            // Only the 2px bar is painted; the strip around it stays transparent.
            let bar = container(Space::new().height(design::HANDLE_GLOW_CORE))
                .width(design::HANDLE_GLOW_WIDTH)
                .style(move |_| {
                    if lit {
                        design::bar(MUTED)
                    } else {
                        container::Style::default()
                    }
                });
            let glow = container(bar)
                .width(WIDTH_HANDLE_WIDTH)
                .height(Length::Fill)
                .align_y(iced::Alignment::Center)
                .align_x(match side {
                    // The bar sits on the strip's inner edge: toward the
                    // transcript for the left handle, away for the right one.
                    design::WidthSide::Left => iced::Alignment::End,
                    design::WidthSide::Right => iced::Alignment::Start,
                })
                .padding(match side {
                    design::WidthSide::Left => iced::Padding {
                        right: design::HANDLE_GLOW_INSET,
                        ..iced::Padding::ZERO
                    },
                    design::WidthSide::Right => iced::Padding {
                        left: design::HANDLE_GLOW_INSET,
                        ..iced::Padding::ZERO
                    },
                });
            let strip = container(
                iced::widget::MouseArea::new(glow)
                    .interaction(iced::mouse::Interaction::ResizingHorizontally)
                    .on_press(Message::WidthDragStart(side))
                    .on_release(Message::WidthDragEnd)
                    .on_enter(Message::WidthHandleHover(side, true))
                    .on_exit(Message::WidthHandleHover(side, false)),
            )
            .width(WIDTH_HANDLE_WIDTH)
            .height(Length::Fill);
            match side {
                design::WidthSide::Left => strip.id("native-width-handle-left"),
                design::WidthSide::Right => strip.id("native-width-handle-right"),
            }
        };
        row![
            Space::new().width(Length::Fill),
            handle(design::WidthSide::Left),
            Space::new().width(WIDTH_HANDLE_OFFSET),
            Space::new().width(lane),
            Space::new().width(WIDTH_HANDLE_OFFSET),
            handle(design::WidthSide::Right),
            Space::new().width(Length::Fill),
        ]
        .height(Length::Fill)
        .into()
    }
    /// Composer editor height. The web InputBar grows its draft up to 14 lines
    /// (`--dsh-composer-text-max-height: 336px`) over a 36px minimum. Native
    /// counts logical lines at the body line box plus the editor's 24px vertical
    /// padding, keeps the existing resting size, and uses a shorter cap in a
    /// short window. Wrapped visual lines are not counted; beyond the cap the
    /// editor scrolls internally.
    fn composer_editor_height(&self) -> f32 {
        // The composer holds body text, so it takes the body line box rather than
        // Iced's relative default, plus the input's own chrome: one line is then
        // exactly the web's `min-height: 36px` (`InputBar.module.css:169-176`)
        // and the fourteen-line cap keeps the text box at the web's 336px.
        // Reserving and drawing the same box is what keeps both figures honest.
        // A short window keeps native's smaller cap so the transcript stays
        // visible; the web has no such variant.
        let line = design::LINE_BODY + self.content_delta();
        let max_rows = if self.layout_size().height < 620.0 {
            8.0
        } else {
            14.0
        };
        let rows = (self.editor.text().lines().count().max(1) as f32).min(max_rows);
        // The hero's floor is the reference's two-line 52px box; the docked
        // floor is its one-line 36px.
        let floor = if self.hero_active() {
            design::HERO_INPUT_MIN
        } else {
            design::COMPOSER_INPUT_MIN
        };
        (rows * line + design::COMPOSER_INPUT_CHROME)
            .max(floor)
            .round()
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
            && !self.sidebar_organization.is_open()
            && !self.exporter.is_open()
    }
    fn animation_active(&self) -> bool {
        self.sidebar_motion
            .running(self.window_active && self.layout_controls_enabled())
    }
    fn composer_visible(&self) -> bool {
        self.composer_base_visible() && !self.local_files.picker.is_open()
    }
    fn composer_base_visible(&self) -> bool {
        self.layout_controls_enabled()
            && !self.capabilities
            && !self.conversation_menu
            && !self.work_open
            && !self.terminal.is_open()
            && !self.mcp.is_open()
            && self.detail.is_none()
            && !self.sidebar_sheet
            && !self
                .selected
                .as_ref()
                .is_some_and(|id| self.sidebar_organization.trashed(id))
    }
    fn detail_visible(&self) -> bool {
        self.layout_controls_enabled()
            && !self.capabilities
            && !self.conversation_menu
            && !self.work_open
            && !self.terminal.is_open()
            && !self.sidebar_sheet
            && self.detail.is_some()
    }
    fn detail_request(&self, key: u64) -> Option<Message> {
        self.detail_opening
            .filter(|opening| *opening < u64::MAX)
            .map(|prior_opening| {
                Message::Details(detail_panel::Opening {
                    generation: self.generation,
                    prior_opening,
                    key,
                })
            })
    }
    fn detail_back(&self) -> Option<Message> {
        self.detail
            .as_ref()
            .and_then(|detail| detail.ticket)
            .map(Message::HideDetails)
    }
    fn detail_copy_source(&self, ticket: detail_panel::Stamp) -> Option<String> {
        if !self.detail_visible() || ticket.generation != self.generation {
            return None;
        }
        self.detail
            .as_ref()
            .filter(|detail| detail.ticket == Some(ticket) && detail.copy_available())
            .map(|detail| detail.source.clone())
    }
    fn detail_escape_ticket(&self) -> Option<detail_panel::Stamp> {
        self.detail_visible()
            .then(|| self.detail.as_ref().and_then(|detail| detail.ticket))
            .flatten()
    }
    fn open_detail(
        &mut self,
        heading: String,
        source: String,
        metadata: Option<String>,
        assistant: bool,
    ) {
        let Some(opening) = self.detail_opening.and_then(|n| n.checked_add(1)) else {
            // Never mint a reusable ticket after exhaustion. Keep any readable old
            // snapshot and its valid Back ticket, but reject all new openings.
            self.detail_opening = None;
            return;
        };
        self.detail_opening = Some(opening);
        let ticket = Some(detail_panel::Stamp {
            generation: self.generation,
            opening,
        });
        self.detail = Some(detail_panel::Detail::new(
            heading, source, metadata, assistant, ticket,
        ));
    }
    fn request_catalog(&mut self) {
        if self.options.smoke.is_some()
            || !self.ready
            || self.stopped
            || self.observations_lost
            || !matches!(self.close, Close::None)
            || self.catalog_read.pending()
        {
            return;
        }
        let Some(ticket) = self.catalog_read.begin(self.transport_epoch) else {
            self.model_controls.catalog_failed(true);
            self.status = "Catalog read identity unavailable or exhausted; no request sent".into();
            return;
        };
        self.model_controls.catalog_failed(false);
        self.command(Command::Catalog(ticket));
    }
    fn command(&mut self, command: Command) {
        if let Command::Catalog(ticket) = &command
            && !self.catalog_read.owns(*ticket, self.transport_epoch)
        {
            return;
        }
        if matches!(
            &command,
            Command::Prompt { .. } | Command::PromptFile { .. }
        ) {
            self.smoke_evidence.model_prompts += 1;
        }
        if matches!(&command, Command::Catalog(_)) {
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
                Command::PromptFile { tickets, request } => {
                    let context = self.file_context();
                    for ticket in &tickets {
                        self.files.prompt_complete(
                            ticket,
                            &request.request_id,
                            worker::attachments::PromptOutcome::NotSent,
                            &context,
                        );
                    }
                    if let Some(ticket) = tickets.first() {
                        self.pending.remove(&ticket.generation);
                    }
                }
                Command::Codex(effect) => {
                    use crate::codex::Effect;
                    let ticket = match &effect {
                        Effect::RuntimeRead { ticket }
                        | Effect::RuntimeSave { ticket, .. }
                        | Effect::UsageRead { ticket }
                        | Effect::UsageSave { ticket, .. }
                        | Effect::AccountsRead { ticket }
                        | Effect::AddAccount { ticket, .. }
                        | Effect::SelectAccount { ticket, .. }
                        | Effect::SignOutAccount { ticket, .. }
                        | Effect::SignOutAll { ticket, .. }
                        | Effect::StatusRead { ticket }
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
                Command::Rename(submission) => {
                    self.rename
                        .finish(&submission.receipt, crate::rename::Outcome::NotSent);
                }
                Command::Fork(submission) => {
                    self.fork
                        .finish(&submission.receipt, crate::fork::Outcome::NotSent);
                }
                Command::TerminalRead(ticket) => {
                    self.terminal
                        .loaded(&ticket, Err(crate::terminal::ReadFailure::Rejected));
                }
                Command::TerminalFollow(ticket) => {
                    self.terminal
                        .follow_ended(&ticket, Some(crate::terminal::ReadFailure::Rejected));
                }
                Command::TerminalCreate(submission) => {
                    self.terminal.finish_create(
                        &submission.receipt,
                        crate::terminal::CreateOutcome::NotSent,
                    );
                }
                Command::TerminalClose(submission) => {
                    self.terminal
                        .finish_close(&submission.receipt, crate::terminal::CloseOutcome::NotSent);
                }
                Command::TerminalWrite(submission) => {
                    self.terminal
                        .finish_write(&submission.receipt, crate::terminal::WriteOutcome::NotSent);
                }
                Command::Export(submission) => {
                    self.exporter
                        .finish(&submission.receipt, crate::exporter::Outcome::NotSent);
                }
                Command::Account(effect) => {
                    let ticket = effect.ticket();
                    drop(effect);
                    self.settings
                        .account_completed(ticket, crate::settings::account::Outcome::NotSent);
                }
                Command::Cloudflare(effect) => {
                    let ticket = effect.ticket();
                    drop(effect);
                    self.settings.cloudflare_completed(
                        ticket,
                        crate::settings::cloudflare::Outcome::NotSent,
                    );
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
                Command::Create { .. } => {
                    self.new_pending = false;
                    self.model_controls.new_selection = None;
                    self.reconcile_model_selection();
                }
                Command::OpenWorkspace(_) => self.workspace_pending = false,
                Command::Catalog(ticket) => {
                    if self.catalog_read.complete(ticket, self.transport_epoch) {
                        self.model_controls.catalog_failed(true);
                    }
                }
                Command::Prompt {
                    generation,
                    request,
                } => {
                    let ticket = prompt_recovery::Ticket {
                        epoch: self.transport_epoch,
                        generation,
                        session: request.session_id,
                        request_id: request.request_id,
                    };
                    self.pending
                        .settle(&ticket, prompt_recovery::Receipt::NotSent);
                }
                Command::CheckSubmission(ticket) => {
                    self.pending.checked(&ticket, None);
                }
                Command::RejoinObservations { .. } => {
                    self.rejoin_pending = false;
                }
                Command::SelectModel { generation, .. } if generation == self.generation => {
                    self.model_pending = false;
                    self.reconcile_model_selection();
                }
                Command::Page { generation, .. } if generation == self.generation => {
                    self.page_pending = false
                }
                Command::Cancel { generation, .. } if generation == self.generation => {
                    self.cancel_pending = false
                }
                Command::McpRead(ticket) => {
                    self.mcp.loaded_snapshot(
                        ticket,
                        crate::mcp::Loaded::refused(
                            "The native queue, worker or lifecycle refused the MCP read.",
                        ),
                    );
                }
                Command::McpWrite(submission) => {
                    self.mcp
                        .written(&submission, crate::mcp::WriteOutcome::NotSent);
                }
                _ => {}
            }
        }
    }
    fn choose(&mut self, session: SessionId) {
        self.capture_draft();
        self.files.invalidate();
        if let Some(old) = &self.selected {
            self.drafts.insert(old.clone(), self.editor.text());
        }
        self.editor =
            text_editor::Content::with_text(self.drafts.get(&session).map_or("", String::as_str));
        self.management.dismiss();
        self.sidebar_organization.dismiss();
        self.rename.dismiss();
        self.terminal.dismiss();
        self.mcp.dismiss();
        self.exporter.dismiss();
        self.selected = Some(session.clone());
        self.generation += 1;
        self.transcript = Transcript::new(self.generation);
        self.chat_geometry = Default::default();
        self.reply_content = Default::default();
        self.thoughts_expanded.clear();
        self.activity_expanded.clear();
        self.edit_review = None;
        self.draft_review = None;
        self.search_open = false;
        self.search_query.clear();
        self.search_serial = 0;
        self.search_cursor = None;
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
        // A projection or advertisement belongs to one session, so a new
        // selection has none yet.
        self.plan_published = None;
        self.plan_toggle_pending = false;
        self.commands = None;
        self.commands_pending = false;
        self.command_selected = 0;
        // A new subject: its run state is a snapshot, not a settle.
        self.run_watch = run_watch::Watch::default();
        self.model_controls.dismiss();
        self.reconcile_model_selection();
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
                && !self.observations_lost
                && self.pending.is_empty()
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
                && !self.new_pending
                && matches!(self.close, Close::None)
                && self.composer_base_visible(),
        }
    }
    fn file_local_context(&self) -> attachment_draft::Context {
        let mut context = self.file_context();
        context.allowed = self.pending.is_empty()
            && self.options.smoke.is_none()
            && matches!(self.close, Close::None)
            && self.composer_visible()
            && !self.files.prompt_pending();
        context
    }
    fn send_allowed(&self) -> bool {
        self.pending.is_empty()
            && self.draft_serial != u64::MAX
            && !self.sidebar_organization.busy()
            && self.sidebar_organization.failure().is_none()
            && !self.local_files.picker.is_open()
            && !self.model_controls.is_open()
            && self.options.smoke.is_none()
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
            && !self.new_pending
            && self.files.can_send(&self.file_context())
            && self.draft_load_allows_send()
            && (!self.editor.text().trim().is_empty() || self.files.has_file(&self.file_context()))
            && self.editor.text().len() <= 32 * 1024
            && matches!(self.close, Close::None)
            && self.composer_visible()
    }
    fn stop_allowed(&self) -> bool {
        self.options.smoke.is_none()
            && self.ready
            && self.selected.is_some()
            && !self.cancel_pending
            && matches!(self.close, Close::None)
    }
    fn shutdown(&mut self) {
        self.capture_draft();
        self.draft_exit_requested = true;
        self.pending.disconnect();
        self.files.invalidate();
        self.management.disconnect();
        self.sidebar_organization.dismiss();
        self.exporter.disconnect();
        self.rename.disconnect();
        self.fork.disconnect();
        self.terminal.disconnect();
        self.mcp.disconnect();
        self.settings.disconnect();
        self.settings.close();
        self.close = Close::Stopping;
        self.local_files.invalidate();
        self.catalog_read.retire();
        self.ready = false;
        self.command(Command::Shutdown);
    }
    pub fn update(&mut self, message: Message) -> Task<Message> {
        self.reconcile_local_files();
        // Joined local callbacks must settle even behind another modal or during shutdown.
        match message {
            Message::ClipboardCopyField => return self.copy_focused_field(),
            Message::ClipboardPasteField => return self.read_clipboard_for_field(),
            Message::ClipboardPasteText(text) => return self.paste_text_into_field(text),
            Message::ClipboardPasteInto { id, text } => {
                return self.paste_into_focused_field(id, text);
            }
            Message::ClipboardPasteIntoField { id, text } => {
                return self.paste_into_named_field(id, text);
            }
            Message::ClipboardFocusedField(id) => return self.copy_field_text(id),
            Message::ClipboardCopied => return Task::none(),
            Message::Accessibility(request) => return self.accessibility_request(request),
            Message::AccessibilityMeasured(measured) => {
                return self.accessibility_measured(measured);
            }
            Message::AccessibilityTick => return self.accessibility_tick(),
            Message::FilePickerRead(result) => return self.picker_completed(result),
            Message::FilePreviewRead(stamp, result) => {
                return self.complete_file_preview(stamp, result);
            }
            Message::SidebarLoaded { ticket, result } => {
                self.sidebar_organization.loaded(ticket, result);
                return if self.draft_exit_requested {
                    self.finish_draft_exit()
                } else {
                    Task::none()
                };
            }
            Message::SidebarSaved { ticket, result } => {
                self.sidebar_organization.saved(ticket, result);
                return if self.draft_exit_requested {
                    self.finish_draft_exit()
                } else {
                    Task::none()
                };
            }
            _ => {}
        }
        if self.local_files.picker.is_open()
            && (background_message(&message) || matches!(&message, Message::Settings(_)))
            && !matches!(&message, Message::FilePicker(_))
        {
            return Task::none();
        }
        if (self.observations_lost || !self.pending.is_empty())
            && matches!(
                &message,
                Message::Send
                    | Message::KeyboardSend
                    | Message::NewSession
                    | Message::OpenWorkspace
                    | Message::Model(_)
                    | Message::Effort(_)
                    | Message::ModelControls(_)
                    | Message::Reload
                    | Message::Select(_)
                    | Message::FileUpload(_)
                    | Message::FileRemove(_)
                    | Message::ExportSession(_)
            )
        {
            return Task::none();
        }
        if ((self.sidebar_organization.busy() || self.sidebar_organization.failure().is_some())
            && matches!(&message, Message::Select(_)))
            || matches!(&message,Message::Select(id) if self.sidebar_organization.trashed(id))
        {
            return Task::none();
        }
        if matches!(&message, Message::WorkingTick) {
            if self.working_animation_active() {
                self.working_phase = (self.working_phase + 1) % working_indicator::PHASES;
            }
            return Task::none();
        }
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
        if self.rename.is_open()
            && !self.settings.is_open()
            && ((background_message(&message) && !matches!(&message, Message::Rename(_)))
                || matches!(&message, Message::Settings(_)))
        {
            return Task::none();
        }
        if self.fork.is_open()
            && !self.settings.is_open()
            && ((background_message(&message) && !matches!(&message, Message::Fork(_)))
                || matches!(&message, Message::Settings(_)))
        {
            return Task::none();
        }
        if self.sidebar_organization.is_open()
            && ((background_message(&message) && !matches!(&message, Message::Sidebar(_)))
                || matches!(&message, Message::Settings(_)))
        {
            return Task::none();
        }
        if self.terminal.is_open()
            && !self.settings.is_open()
            && ((background_message(&message) && !matches!(&message, Message::Terminal(_)))
                || matches!(&message, Message::Settings(_)))
        {
            return Task::none();
        }
        // The MCP panel owns the whole surface while it is open: a background
        // frame cannot change the file, the draft or the review under the user.
        if self.mcp.is_open()
            && !self.settings.is_open()
            && ((background_message(&message) && !matches!(&message, Message::Mcp(_)))
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
                | Message::ToggleThoughts(_)
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
                | Message::Rename(_)
                | Message::Fork(_)
                | Message::ExportSession(_)
                | Message::Export(_)
                | Message::Capabilities
                | Message::Work(_)
                | Message::Terminal(_)
                | Message::Close
                | Message::Select(_)
                | Message::Workspace(_)
                | Message::ToggleSidebar
        ) {
            self.conversation_menu = false;
            self.model_controls.dismiss();
        }
        if matches!(
            &message,
            Message::ToggleConversationMenu
                | Message::FileOpen
                | Message::Details(_)
                | Message::Resized(_)
                | Message::WindowActive(false)
        ) {
            self.model_controls.dismiss();
        }
        match message {
            Message::Sidebar(action) => return self.sidebar_action(action),
            Message::DismissPanel if self.sidebar_organization.is_open() => {
                self.sidebar_organization.dismiss();
            }
            Message::DismissPanel if self.search_open && self.composer_visible() => {
                return self.update(Message::SearchClose {
                    generation: self.generation,
                    serial: self.search_serial,
                });
            }
            // Every model-popup control lives in the composer's own strip, so the
            // caret comes back with the choice. The picker and the panels render
            // outside this match and take their own focus.
            Message::ModelControls(action) => {
                let visible = self.composer_visible();
                return Self::composer_focus_kept(visible, self.model_control_action(action));
            }
            Message::FileChoose => return self.file_choose(),
            Message::FilePicker(action) => return self.file_picker_action(action),
            Message::FilePreview(stamp) => return self.start_file_preview(stamp),
            Message::FileDropped(id, path) if self.own_window == Some(id) => {
                return self.choose_local_file(path);
            }
            Message::FileDropCancelled(id)
                if self.own_window == Some(id) && self.file_context().allowed =>
            {
                self.status =
                    "Drop canceled or unsupported. Attach up to 4 local files (4 MiB each).".into();
            }
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
                    self.preview_uploaded(stamp, submission.ticket.clone());
                    self.command(Command::StageFile(submission));
                }
            }
            Message::FileCancel(stamp) => {
                let context = self.file_local_context();
                self.files.cancel_editor(stamp, &context);
            }
            Message::FileRemove(ticket) => {
                let context = self.file_local_context();
                if context.allowed && self.files.owns(&ticket) {
                    if self
                        .handle
                        .send(Command::DiscardFile(ticket.clone()))
                        .is_ok()
                    {
                        self.files.remove(&ticket, &context);
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
                self.work_open = false;
                self.terminal.dismiss();
                self.mcp.dismiss();
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
                self.show_trash = false;
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
            Message::DismissPanel if self.rename.is_open() && !self.settings_modal_visible() => {
                self.rename.dismiss();
            }
            Message::DismissPanel if self.fork.is_open() && !self.settings_modal_visible() => {
                self.fork.dismiss();
            }
            Message::DismissPanel if self.mcp.is_open() && !self.settings_modal_visible() => {
                self.mcp.dismiss();
            }
            Message::DismissPanel if self.terminal.is_open() && !self.settings_modal_visible() => {
                self.terminal.dismiss();
                self.mcp.dismiss();
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
                    && let Some(view) = context.view
                {
                    self.management.dismiss();
                    let context = self.export_context_for(Some(&view.target));
                    self.exporter
                        .apply(crate::exporter::Action::Open(view), &context);
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
            Message::Rename(action) => {
                let context = self.rename_context();
                if let Some(submission) = self.rename.apply(action, &context) {
                    self.command(Command::Rename(submission));
                }
            }
            Message::Fork(action) => {
                let context = self.fork_context();
                if let Some(submission) = self.fork.apply(action, &context) {
                    self.command(Command::Fork(submission));
                }
            }
            Message::Terminal(action) => {
                // Opening from the More menu replaces the menu and any other panel.
                if matches!(action, crate::terminal::Action::Open { .. }) {
                    self.conversation_menu = false;
                    self.capabilities = false;
                    self.work_open = false;
                    self.detail = None;
                    self.mcp.dismiss();
                }
                let context = self.terminal_context();
                if let Some(effect) = self.terminal.apply(action, &context) {
                    self.dispatch_terminal(effect);
                }
            }
            Message::Mcp(action) => {
                // Opening from the More menu replaces the menu and any other
                // panel; the panel itself owns everything after that.
                if matches!(action, crate::mcp::Action::Open) && !self.mcp.is_open() {
                    self.conversation_menu = false;
                    self.capabilities = false;
                    self.work_open = false;
                    self.detail = None;
                    self.terminal.dismiss();
                    let effect = self.mcp.enter(self.transport_epoch);
                    self.dispatch_mcp(effect);
                } else {
                    let context = self.mcp_context();
                    if let Some(effect) = self.mcp.apply(action, context) {
                        self.dispatch_mcp(effect);
                    }
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
            Message::ScrollToBottom => {
                // Following the tail is the same state the reader anchor is
                // captured against, so one flag both re-arms the snap and hides
                // the control.
                self.follow_bottom = true;
                return self.position_transcript();
            }
            Message::WidthHandleHover(side, inside) => {
                // Enter and exit both arrive; only the side the pointer actually
                // left clears, so crossing the transcript cannot drop a live hover.
                if inside {
                    self.width_hover = Some(side);
                } else if self.width_hover == Some(side) {
                    self.width_hover = None;
                }
                return Task::none();
            }
            Message::WidthDragStart(side) => {
                // The web's `onStart` returns the width currently displayed, and
                // the drag moves from there rather than from the stored value.
                let base =
                    design::chat_width_with(self.content_column(), self.chat_width_preference);
                self.width_drag = Some(WidthDrag {
                    side,
                    base,
                    saved: self.chat_width_preference,
                    origin_x: None,
                    moved: false,
                });
                return Task::none();
            }
            Message::WidthDragMove(x) => {
                let mut next = None;
                if let Some(drag) = self.width_drag.as_mut() {
                    match drag.origin_x {
                        // The first sample after the press sets the origin, so a
                        // press that never moves cannot change the width.
                        None => drag.origin_x = Some(x),
                        Some(origin) => {
                            let raw = design::chat_width_drag(drag.base, origin, x, drag.side);
                            drag.moved |= (raw - drag.base).abs() > f32::EPSILON;
                            next = Some(raw);
                        }
                    }
                }
                if let Some(raw) = next {
                    self.chat_width_preference = Some(raw);
                }
                return Task::none();
            }
            Message::WidthDragEnd => {
                if let Some(drag) = self.width_drag.take() {
                    if drag.moved {
                        self.save_layout_preference();
                    } else {
                        // A press is not a gesture: put the wider preference back
                        // rather than persisting the clamped display width.
                        self.chat_width_preference = drag.saved;
                    }
                }
                return Task::none();
            }
            Message::WidthDragCancel => {
                if let Some(drag) = self.width_drag.take() {
                    self.chat_width_preference = drag.saved;
                }
                return Task::none();
            }
            Message::FocusRequested => {
                return match self.own_window {
                    Some(id) => window::gain_focus(id),
                    None => Task::none(),
                };
            }
            Message::ContentSize(size) => {
                let size = design::content_size(size);
                if size != self.content_font_size {
                    self.content_font_size = size;
                    self.save_layout_preference();
                }
                return Task::none();
            }
            Message::ActivityHover(key) => {
                self.hovered_activity = Some(key);
                return Task::none();
            }
            Message::ActivityHoverEnd(key) => {
                // Clear only the row this exit belongs to: a later enter may have
                // already replaced it, and clearing that would strand the pointer.
                if self.hovered_activity == Some(key) {
                    self.hovered_activity = None;
                }
                return Task::none();
            }
            Message::WindowActive(active) => {
                self.window_active = active;
                if let Some(bridge) = &mut self.accessibility {
                    bridge.window_focus(active);
                }
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
            Message::DismissPanel if self.subagent_menu => {
                self.subagent_menu = false;
            }
            Message::DismissPanel if self.layout_controls_enabled() && self.conversation_menu => {
                self.conversation_menu = false;
            }
            Message::DismissPanel if self.layout_controls_enabled() && self.work_open => {
                self.work_open = false;
            }
            Message::DismissPanel if self.layout_controls_enabled() => {
                self.sidebar_sheet = false;
                self.capabilities = false;
                // Stateless composer/old subscription dismissals never close Details.
                // The visible Details Escape subscription carries its opening ticket.
            }
            Message::Settings(crate::settings::Action::Paste { ticket })
                if self.settings.paste_allowed(
                    ticket,
                    self.transport_epoch,
                    self.settings_enabled(),
                ) =>
            {
                if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                    // Iced's Wayland clipboard backend selects a seat only from
                    // key/pointer serials, so a plain Paste click reads nothing.
                    // Read on a detached thread; the UI thread never blocks and
                    // the text enters a one-use holder, never the message.
                    return Task::perform(
                        async {
                            let (sender, receiver) = tokio::sync::oneshot::channel();
                            std::thread::spawn(move || {
                                let _ = sender.send(native_clipboard::read_text());
                            });
                            receiver.await.ok().flatten()
                        },
                        move |text| match text {
                            Some(text) if !text.trim().is_empty() => {
                                Message::Settings(crate::settings::Action::Pasted {
                                    ticket,
                                    input: crate::settings::pasted_key_input(text),
                                })
                            }
                            _ => Message::Settings(crate::settings::Action::PasteUnavailable {
                                ticket,
                            }),
                        },
                    );
                }
                // X11 has no Wayland seat; Iced's own clipboard read is usable.
                return iced::clipboard::read().map(move |text| {
                    Message::Settings(crate::settings::Action::Pasted {
                        ticket,
                        input: crate::settings::pasted_key_input(text.unwrap_or_default()),
                    })
                });
            }
            Message::Settings(action) => {
                // The editor's own actions are local store edits, and the permission default
                // reuses the same gated pick. They must be handled *before* `Settings::handle`,
                // whose wildcard would otherwise absorb them: a button whose arm sits after a
                // catch-all compiles and does nothing, which is how this surface was once inert.
                match &action {
                    crate::settings::Action::BusyEnter(value) => {
                        if self.busy_enter != *value {
                            self.busy_enter = *value;
                            self.save_layout_preference();
                        }
                        return Task::none();
                    }
                    crate::settings::Action::CaptureShortcut(id) => {
                        self.settings.capture_for = Some(id.clone());
                        self.settings.capture_error = None;
                        return Task::none();
                    }
                    crate::settings::Action::ResetAllDefaults => {
                        self.settings.reset_all_armed = true;
                        self.settings.capture_error = None;
                        return Task::none();
                    }
                    crate::settings::Action::ConfirmResetAll(confirmed) => {
                        self.settings.reset_all_armed = false;
                        if *confirmed {
                            let _ = self
                                .settings
                                .keybindings
                                .reset_all(self.settings.keybindings.revision());
                        }
                        return Task::none();
                    }
                    crate::settings::Action::ResetShortcut(id) => {
                        self.settings.capture_error = None;
                        let _ = self.settings.keybindings.apply(
                            crate::keybindings::Edit::Reset { id: id.clone() },
                            self.settings.keybindings.revision(),
                        );
                        return Task::none();
                    }
                    crate::settings::Action::ClearShortcut(id) => {
                        self.settings.capture_error = None;
                        // Synchronous and local: the store lives on Settings, so the edit
                        // needs no message and cannot race the revision it just read.
                        let _ = self.settings.keybindings.apply(
                            crate::keybindings::Edit::Set {
                                id: id.clone(),
                                binding: None,
                            },
                            self.settings.keybindings.revision(),
                        );
                        return Task::none();
                    }
                    crate::settings::Action::PermissionDefault(name) => {
                        return self.update(Message::PermissionDefaultPick(name.clone()));
                    }
                    _ => {}
                }
                // A documentation link is the one settings action that reaches no
                // Host state: the destination is a constant this app ships, so it
                // is copied for the user to open rather than launched.
                if let crate::settings::Action::Cloudflare(
                    crate::settings::cloudflare::Action::CopyGuide(_, guide),
                ) = &action
                {
                    return iced::clipboard::write(guide.url().to_owned());
                }
                if let crate::settings::Action::Cloudflare(
                    crate::settings::cloudflare::Action::RequestPaste(field),
                ) = &action
                {
                    return self.read_clipboard_into(Some(*field));
                }
                let enabled = self.settings_enabled();
                // Asking to sign in subscribes to the Host's account stream
                // first, because the stream is what arms the attempt's browser.
                if matches!(
                    action,
                    crate::settings::Action::Account(crate::settings::account::Action::Start)
                ) && let Some(effect) = self.settings.subscribe_account()
                {
                    self.command(Command::Account(effect));
                }
                if let Some(effect) = self.settings.handle(action, self.transport_epoch, enabled) {
                    self.command(match effect {
                        crate::settings::Effect::Account(effect) => Command::Account(effect),
                        crate::settings::Effect::Cloudflare(effect) => Command::Cloudflare(effect),
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
                // Allow a draw after layout rebuild; capture directly, with no intervening
                // App message. A bounded scheduling assumption, not an atomic draw barrier.
                return Task::future(async {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                })
                .then(move |()| window::screenshot(id))
                .map(Message::OwnScreenshot);
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
                if !self.composer_visible() || !self.ready {
                    self.model_controls.dismiss();
                }
                // Every Host report can settle the selected session, so the
                // announcement is decided after the state it describes is applied.
                self.announce_settle();
                return task;
            }
            Message::ReviewRecovered { generation, serial }
                if generation == self.generation
                    && serial == self.draft_serial
                    && self.composer_visible()
                    && self.pending.is_empty()
                    && self.recovered_draft_needs_review() =>
            {
                self.draft_review = Some((generation, serial));
            }
            Message::DraftFlush => return self.flush_drafts(),
            Message::DraftOutcome(outcome) => return self.drafts_completed(outcome),
            Message::DraftReload
                if self.composer_visible() && self.draft_state.failure().is_some() =>
            {
                return self.load_drafts(true);
            }
            Message::PermissionConfirmAcknowledge(acknowledged)
                if self.permission_confirm.is_some() =>
            {
                self.permission_acknowledged = acknowledged;
            }
            Message::PermissionConfirmCancel if self.permission_confirm.is_some() => {
                self.permission_confirm = None;
                self.permission_acknowledged = false;
            }
            Message::PermissionConfirmEnable
                if self.permission_confirm.is_some() && self.permission_acknowledged =>
            {
                let preset = self.permission_confirm.take().unwrap_or_default();
                self.permission_acknowledged = false;
                self.send_permission_preset(&preset);
            }
            Message::AgentPresetSeat => {
                // The read rides the press rather than a frame or a selection, which is the
                // discipline the suites enforce: no read is queued without a user action.
                if self.agent_roster.is_none() {
                    self.command(Command::AgentPresetsRead);
                }
                self.agent_seat_open = !self.agent_seat_open;
            }
            Message::AgentPresetPick(id) => {
                self.agent_seat_open = false;
                // A new attempt clears the previous refusal, as the reference's store does.
                self.agent_seat_error = None;
                if let Some(session) = self.selected.clone() {
                    self.command(Command::AgentPresetSelect {
                        session,
                        preset: id,
                    });
                }
            }
            Message::KeyPressed(key, modifiers) => {
                return self.key_pressed(key, modifiers);
            }
            Message::ShortcutsOpen => {
                // The reference's `shortcuts.open` shows its own overlay; the equivalent here is
                // the Settings modal on the General page with the Shortcuts disclosure expanded.
                // Opening alone lands on the account page, which is why this selects General:
                // the disclosure existed but was hidden behind the default page.
                let opened = self.update(Message::Settings(crate::settings::Action::Open));
                let page = self.update(Message::Settings(crate::settings::Action::SelectPage(
                    crate::settings::Page::General,
                )));
                // Set the disclosure *after* open and page selection: both reset the info
                // disclosures, so setting it first is silently undone.
                self.settings.shortcuts_open = true;
                // The reference focuses its search field when the overlay opens
                // (`Reference.tsx:137-139`).
                let focus = iced::widget::operation::focus("native-shortcuts-search");
                return Task::batch([opened, page, focus]);
            }
            Message::ToggleJobs => {
                self.jobs_open = !self.jobs_open;
            }
            Message::KillJob(id) => {
                // Two clicks, as the reference's own copy requires: the second one asks the Host.
                if self.jobs_armed.as_deref() == Some(id.as_str()) {
                    self.jobs_armed = None;
                    if let Some(session) = self.selected.clone() {
                        self.command(Command::JobKill { session, job: id });
                    }
                } else {
                    self.jobs_armed = Some(id);
                    self.jobs_kill_error = None;
                }
            }
            Message::ToggleJobOutput(id) => {
                let opening = self.jobs_expanded.as_deref() != Some(id.as_str());
                self.jobs_expanded = if opening { Some(id.clone()) } else { None };
                // The stream is per job, so expanding asks the worker for that job's output.
                if opening && let Some(session) = self.selected.clone() {
                    self.command(Command::JobOutput { session, job: id });
                }
            }
            Message::TogglePermissionMenu if self.composer_visible() => {
                self.permission_menu = !self.permission_menu;
                // The reference reads the catalog when the picker opens; native does the
                // same and keeps the design set until the reply lands.
                if self.permission_menu && self.permission_catalog.is_none() {
                    self.command(Command::PermissionCatalogRead);
                }
            }
            Message::PermissionDefaultPick(name) => {
                if let Some(settings) = &self.permission_settings
                    && settings.writable
                    && !self.permission_default_saving
                {
                    self.permission_default_saving = true;
                    self.command(Command::PermissionDefaultSet {
                        name,
                        expected_revision: settings.revision,
                    });
                }
            }
            Message::PermissionPick(preset) if self.composer_visible() => {
                self.permission_menu = false;
                // `PermissionSelect.tsx:198`: full access passes through a risk
                // confirmation with an explicit acknowledgement before it commits.
                if preset == "full-access" {
                    self.permission_confirm = Some(preset);
                    self.permission_acknowledged = false;
                    return Task::none();
                }
                self.send_permission_preset(&preset);
            }
            Message::ToggleSubagentMenu(open) if self.layout_controls_enabled() => {
                self.subagent_menu = open;
                if !open {
                    self.subagent_expanded.clear();
                }
            }
            Message::ToggleQueue if self.layout_controls_enabled() => {
                self.queue_expanded = !self.queue_expanded;
            }
            Message::ToggleTrigger(key) if self.layout_controls_enabled() => {
                if !self.trigger_expanded.remove(&key) {
                    self.trigger_expanded.insert(key);
                }
            }
            Message::ToggleSubagentBranch(id) if self.layout_controls_enabled() => {
                if !self.subagent_expanded.remove(&id) {
                    self.subagent_expanded.insert(id);
                }
            }
            Message::SubagentOpen(id) if self.layout_controls_enabled() => {
                self.subagent_menu = false;
                if let Ok(id) = SessionId::new(&id) {
                    return self.update(Message::Select(id));
                }
                return Task::none();
            }
            Message::EditorMenu(open) if self.composer_visible() => {
                self.editor_menu = open;
            }
            // The shell's own entries: Undo/Redo drive the editor-owned history, the
            // clipboard entries act on the selection, and Paste reuses the composer's
            // existing asynchronous read.
            Message::EditorMenuAction(action) if self.composer_visible() => {
                self.editor_menu = false;
                match action {
                    editor_menu::Action::Undo => {
                        if let Some(previous) = self.editor_undo.pop() {
                            self.editor_redo.push(self.editor.text());
                            self.set_editor_text(&previous);
                        }
                    }
                    editor_menu::Action::Redo => {
                        if let Some(next) = self.editor_redo.pop() {
                            self.editor_undo.push(self.editor.text());
                            self.set_editor_text(&next);
                        }
                    }
                    editor_menu::Action::Cut => {
                        if let Some(selection) = self.editor.selection() {
                            let copy = self.write_clipboard(selection);
                            self.editor
                                .perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                            self.record_editor_edit();
                            return copy;
                        }
                    }
                    editor_menu::Action::Copy => {
                        if let Some(selection) = self.editor.selection() {
                            return self.write_clipboard(selection);
                        }
                    }
                    editor_menu::Action::Paste => {
                        return self.update(Message::PasteComposer);
                    }
                    editor_menu::Action::SelectAll => {
                        self.editor.perform(text_editor::Action::SelectAll);
                    }
                }
            }
            Message::Editor(action)
                if !self.model_controls.is_open() && self.composer_visible() =>
            {
                let changed = matches!(&action, text_editor::Action::Edit(_));
                if changed {
                    // One history step per edit, cleared of redo branches, exactly
                    // like the web editor the shell dispatches its keys to.
                    self.record_editor_edit();
                    self.draft_serial = self.draft_serial.saturating_add(1);
                    self.edit_review = None;
                }
                self.editor.perform(action);
                if changed {
                    self.touch_draft();
                    // Typing a leading slash is the user asking for completions;
                    // the read is lazy, once, and never on a frame.
                    if command_menu::query(&self.editor.text()).is_some() {
                        self.request_commands();
                    } else {
                        // Leaving the command position resets the highlight, so a
                        // later command line starts from the top.
                        self.command_selected = 0;
                    }
                }
            }
            // The menu consumes these keys only while it is open, so nothing
            // about ordinary composing changes when no command line is typed.
            Message::CommandMenuMove(delta) if self.completion().is_some() => {
                let len = self.completion().map_or(0, |menu| menu.matches.len());
                self.command_selected = command_menu::advance(self.command_selected, len, delta);
            }
            Message::CommandMenuAccept if self.completion().is_some() => {
                if let Some(candidate) = self
                    .completion()
                    .and_then(|menu| menu.candidate().map(|c| c.name.clone()))
                {
                    return self.accept_command(&candidate);
                }
            }
            // A pointer picks the row it named, not the highlighted one, so a
            // click can never complete a command the user did not point at.
            Message::CommandMenuPick(name) if self.completion().is_some() => {
                return self.accept_command(&name);
            }
            Message::CommandMenuDismiss if self.completion().is_some() => {
                // Dismissal is not a draft edit: clear the menu by leaving the
                // command position, and keep every typed character.
                self.commands = Some(Vec::new());
                self.command_selected = 0;
                return Self::focus_composer();
            }
            // A press that belongs to the composer but not to a control in it.
            // Nothing to change, and focus is the whole point.
            Message::ComposerFocus => return Self::focus_composer(),
            // A command line that still has a menu open consumes Enter as
            // "accept", so the first Enter never sends a half-typed command.
            Message::KeyboardSend
                if self.completion().is_some() && !self.editor.text().trim().ends_with(' ') =>
            {
                return self.update(Message::CommandMenuAccept);
            }
            Message::KeyboardSend if !self.editor.text().trim().is_empty() => {
                return self.update(Message::Send);
            }
            Message::PasteComposer if !self.model_controls.is_open() && self.composer_visible() => {
                return Task::perform(
                    async {
                        let (sender, receiver) = tokio::sync::oneshot::channel();
                        std::thread::spawn(move || {
                            let _ = sender.send(native_clipboard::read_text());
                        });
                        receiver.await.ok().flatten()
                    },
                    Message::PasteComposerText,
                );
            }
            Message::PasteComposerText(Some(text))
                if !self.model_controls.is_open()
                    && self.composer_visible()
                    && !text.is_empty() =>
            {
                self.draft_serial = self.draft_serial.saturating_add(1);
                self.edit_review = None;
                self.editor
                    .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                        std::sync::Arc::new(text),
                    )));
                self.touch_draft();
            }
            Message::StopSubmission(ticket)
                if self.ready
                    && !self.stopped
                    && !self.cancel_pending
                    && ticket.epoch == self.transport_epoch
                    && self.pending.ticket().as_ref() == Some(&ticket)
                    && self.composer_visible() =>
            {
                self.cancel_pending = true;
                self.cancel_generation = Some(ticket.generation);
                self.command(Command::Cancel {
                    generation: ticket.generation,
                    session: ticket.session,
                });
            }
            Message::RejoinObservations
                if self.ready
                    && self.observations_lost
                    && !self.rejoin_pending
                    && !self.stopped =>
            {
                self.rejoin_pending = true;
                self.command(Command::RejoinObservations {
                    epoch: self.transport_epoch,
                });
            }
            Message::CheckSubmission(ticket)
                if self.ready && !self.stopped && self.composer_visible() =>
            {
                if let Some(check) = self.pending.begin_check(&ticket, self.transport_epoch) {
                    self.command(Command::CheckSubmission(check));
                }
            }
            Message::ReviewSubmission(ticket)
                if self.pending.ticket().as_ref() == Some(&ticket) && self.composer_visible() =>
            {
                self.pending.expanded = !self.pending.expanded;
            }
            Message::Send if self.send_allowed() => self.send_prompt(),
            Message::Stop if self.stop_allowed() => {
                self.cancel_pending = true;
                self.status = "Requesting cancellation…".into();
                self.command(Command::Cancel {
                    generation: self.generation,
                    session: self.selected.clone().unwrap(),
                });
            }
            Message::Select(id) if self.ready && self.pending.is_empty() => self.choose(id),
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
                if self.options.smoke.is_none()
                    && self.ready
                    && !self.catalog_read.pending()
                    && !self.observations_lost
                    && !self.stopped
                    && matches!(self.close, Close::None) =>
            {
                self.status = "Loading available models…".into();
                self.request_catalog();
            }
            Message::Model(model)
                if self.options.smoke.is_none()
                    && self.ready
                    && !self.new_pending
                    && !self.model_pending =>
            {
                self.chosen_effort = model
                    .default_effort
                    .as_ref()
                    .and_then(|id| model.efforts.iter().find(|e| &e.id == id))
                    .cloned();
                if self.chosen_model.as_ref().is_none_or(|previous| {
                    previous.provider != model.provider || previous.id != model.id
                }) {
                    self.chosen_service_tier = None;
                    self.chosen_text_verbosity = None;
                }
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
            Message::Details(request)
                if self.composer_visible()
                    && request.generation == self.generation
                    && self.detail_opening == Some(request.prior_opening) =>
            {
                let key = request.key;
                if key == u64::MAX {
                    if let Some(source) = self.transcript.partial() {
                        self.open_detail(
                            "Live assistant snapshot".into(),
                            source,
                            Some("Frozen live display · not a durable record".into()),
                            true,
                        );
                    }
                } else if let Some(row) = self.transcript.rows().iter().find(|r| r.key == key) {
                    let heading = self
                        .transcript
                        .activity(key)
                        .map_or_else(|| row.role.clone(), |a| a.title);
                    let source = row.text.clone();
                    let metadata = Some(format!(
                        "Record #{} · timestamp {} · frozen display",
                        row.key, row.time
                    ));
                    let assistant = self.transcript.assistant_record(key);
                    self.open_detail(heading, source, metadata, assistant);
                }
            }
            Message::ToggleThoughts(ticket)
                if ticket.generation == self.generation && self.composer_visible() =>
            {
                if self.reply_parts(ticket.key).is_some_and(|reply| {
                    reply.ticket == Some(ticket) && !reply.reasoning.is_empty()
                }) && !self.thoughts_expanded.remove(&ticket.key)
                {
                    self.thoughts_expanded.insert(ticket.key);
                }
            }
            // One tool row expands in place: the conversation never leaves for a
            // separate page, and no request, read or write is started. Like a
            // thought disclosure, the row heights refresh on the next view.
            Message::ToggleActivity { generation, key }
                if generation == self.generation && self.composer_visible() =>
            {
                let has_body = self
                    .transcript
                    .rows()
                    .iter()
                    .find(|row| row.key == key)
                    .is_some_and(|row| !activity_body_source(row).is_empty());
                if has_body && !self.activity_expanded.remove(&key) {
                    self.activity_expanded.insert(key);
                } else if !has_body {
                    self.activity_expanded.remove(&key);
                }
            }
            Message::CopyCode(ticket, index)
                if ticket.generation == self.generation && self.composer_visible() =>
            {
                if let Some(reply) = self
                    .reply_parts(ticket.key)
                    .filter(|reply| reply.ticket == Some(ticket))
                    && let Some(source) = reply.code(index).filter(|source| !source.is_empty())
                {
                    self.status = "Code copy requested.".into();
                    return iced::clipboard::write(source.to_owned());
                }
            }
            Message::CopyReply { generation, key }
                if generation == self.generation && self.composer_visible() =>
            {
                if let Some(source) = self.row_copy_source(key) {
                    self.status = "Message copy requested.".into();
                    return iced::clipboard::write(source);
                }
            }
            Message::EditReply { generation, key }
                if self.edit_message_allowed(generation, key) =>
            {
                if self.editor.text().is_empty() {
                    self.populate_message_draft(key);
                } else {
                    self.edit_review = Some((generation, key, self.draft_serial));
                }
            }
            Message::ConfirmEdit {
                generation,
                key,
                draft_serial,
            } if self.edit_review == Some((generation, key, draft_serial))
                && self.draft_serial == draft_serial
                && self.edit_message_allowed(generation, key) =>
            {
                self.populate_message_draft(key);
            }
            Message::SearchOpen
                if self.composer_visible()
                    && self.selected.is_some()
                    && self.search_serial != u64::MAX =>
            {
                if !self.search_open {
                    self.search_serial += 1;
                    self.search_open = true;
                }
                return iced::widget::operation::focus("native-chat-search");
            }
            Message::SearchInput {
                generation,
                serial,
                query,
            } if generation == self.generation
                && serial == self.search_serial
                && self.search_open
                && self.composer_visible() =>
            {
                self.search_query = if query.len() <= conversation_search::MAX_QUERY_BYTES {
                    query
                } else {
                    self.status = "Search is limited to 256 bytes.".into();
                    String::new()
                };
                self.search_cursor = None;
                return self.jump_search(false, true);
            }
            Message::SearchNext {
                generation,
                serial,
                backwards,
            } if generation == self.generation
                && serial == self.search_serial
                && self.search_open
                && self.composer_visible() =>
            {
                return self.jump_search(backwards, false);
            }
            Message::SearchClose { generation, serial }
                if generation == self.generation
                    && serial == self.search_serial
                    && self.search_open
                    && self.composer_visible() =>
            {
                let anchor = self.reader_anchor();
                self.search_open = false;
                self.search_cursor = None;
                // Closing search returns the user to writing; leaving the caret
                // behind in a control that is gone is how a composer goes dead.
                let visible = self.composer_visible();
                return Self::composer_focus_kept(visible, self.reflow_transcript(anchor));
            }
            Message::CancelEdit => {
                self.edit_review = None;
            }
            Message::CopyDetailSource(ticket) => {
                if let Some(source) = self.detail_copy_source(ticket) {
                    // Iced writes Standard clipboard without a success acknowledgement.
                    // Do not claim completion or change backend/reader/draft authority.
                    return iced::clipboard::write(source);
                }
            }
            Message::DetailMode(ticket, mode)
                if self.detail_visible() && ticket.generation == self.generation =>
            {
                if let Some(detail) = &mut self.detail {
                    detail.set_mode(ticket, mode);
                }
            }
            Message::HideDetails(ticket)
                if self.detail_visible()
                    && ticket.generation == self.generation
                    && self.detail.as_ref().and_then(|detail| detail.ticket) == Some(ticket) =>
            {
                self.detail = None
            }
            Message::Capabilities => {
                self.capabilities = !self.capabilities;
                self.work_open = false;
                self.terminal.dismiss();
                self.mcp.dismiss();
                self.info_expanded = 0;
            }
            // Read-only view of retained records: opening it produces no command,
            // effect, prompt, cancellation or schedule edit.
            Message::Work(work_panel_ui::Action::Open)
                if self.layout_controls_enabled() && !self.sidebar_sheet =>
            {
                self.work_open = true;
                self.conversation_menu = false;
                self.capabilities = false;
                self.terminal.dismiss();
                self.mcp.dismiss();
            }
            Message::Work(work_panel_ui::Action::Close) if self.work_open => {
                self.work_open = false;
            }
            // A disclosure with no visible composer has nothing to expand, and an
            // invisible one is collapsed again rather than kept as hidden state.
            Message::ToggleContextPolicy if self.composer_visible() => {
                self.context_policy_open = !self.context_policy_open;
                return Self::focus_composer();
            }
            Message::ToggleContextPolicy => {
                self.context_policy_open = false;
            }
            // The control is built from the same state the chip renders, so a
            // press cannot disagree with what the user just read. `press` refuses
            // an unavailable command, and the pending flag makes it inert.
            Message::TogglePlanMode => {
                if let Some(line) = self.plan_control().press()
                    && let Some(session_id) = self.selected.clone()
                {
                    self.plan_toggle_pending = true;
                    self.command(Command::CommandsExecute {
                        session_id,
                        line: line.into(),
                    });
                }
            }
            Message::Close => {
                self.local_files.invalidate();
                self.settings.close();
                self.exporter.dismiss();
                self.rename.dismiss();
                self.fork.dismiss();
                self.terminal.dismiss();
                self.mcp.dismiss();
                if self.stopped {
                    if self.last_stop.as_ref().is_some_and(|r|matches!(r,Ok(s) if s.exited&&!s.containment_unknown&&s.observed_descendants_remaining==0)){self.capture_draft();self.draft_exit_requested=true;self.close=Close::Stopping;return self.finish_draft_exit();}
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
            Message::Stay => {
                self.close = Close::None;
                self.draft_exit_requested = false;
                self.draft_discard_on_exit = false;
                if self.ready && !self.stopped {
                    return Task::batch([self.load_drafts(false), self.position_transcript()]);
                }
            }
            Message::ConfirmClose => self.shutdown(),
            Message::ExitAnyway
                if self.stopped && !self.draft_state.busy() && !self.local_files.busy() =>
            {
                self.draft_discard_on_exit = true;
                return iced::exit();
            }
            _ => {}
        }
        self.reconcile_local_files();
        if geometry_change {
            self.reflow_transcript(reader_anchor)
        } else if was_active != self.transcript_active() {
            self.invalidate_scroll();
            self.position_transcript()
        } else {
            Task::none()
        }
    }

    /// Capture one reviewed prompt - the composer text and, when one is staged,
    /// the exact file set - and dispatch it. Each refusal states why and leaves
    /// the transcript unchanged; the send button already checked `send_allowed`.
    fn send_prompt(&mut self) {
        let session = self.selected.clone().unwrap();
        let value = self.editor.text();
        let Some(serial) = self.prompt_serial.checked_add(1) else {
            self.status = "Native prompt authority exhausted; restart before sending".into();
            return;
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
        // The attachment admission is Queue-only (its own Host command), so a staged file
        // keeps the busy preference on Queue; steering is the text-only path for now.
        let has_files = self.files.has_file(&context);
        let file_tickets = if has_files {
            match self.files.begin_prompt(&context, request_id.clone()) {
                Some(tickets) => Some(tickets),
                None => {
                    self.status = "Attachment authority changed; prompt not sent".into();
                    return;
                }
            }
        } else {
            None
        };
        let running = self
            .selected
            .as_ref()
            .and_then(|id| self.sessions.get(id))
            .is_some_and(|summary| summary.running);
        let mode = if running && !has_files && self.busy_enter == crate::settings::BusyEnter::Steer
        {
            PromptMode::Steer
        } else {
            PromptMode::Queue
        };
        let request = SessionPromptRequest {
            request_id: request_id.clone(),
            session_id: session.clone(),
            mode,
            content: if value.is_empty() {
                Vec::new()
            } else {
                vec![PromptContentPart::Text {
                    text: value.clone(),
                }]
            },
            client_time_zone: Some("UTC".into()),
        };
        let captured = self.pending.insert(
            self.generation,
            Pending {
                session,
                text: value,
                draft_serial: self.draft_serial,
                file_request_id: file_tickets.as_ref().map(|_| request_id),
                epoch: self.transport_epoch,
                file: self.files.metadata_all(),
                file_ticket: file_tickets.clone().unwrap_or_default(),
                request: request.clone(),
            },
        );
        if !captured {
            if let Some(tickets) = &file_tickets {
                let context = self.file_context();
                for ticket in tickets {
                    self.files.prompt_complete(
                        ticket,
                        &request.request_id,
                        worker::attachments::PromptOutcome::NotSent,
                        &context,
                    );
                }
            }
            self.status = "Original intent could not be captured; no prompt dispatched.".into();
            return;
        }
        self.touch_draft(); // Scrub this owned original from the next private snapshot.
        self.status = "Sending prompt; awaiting Host inbox acceptance…".into();
        if let Some(tickets) = file_tickets {
            self.command(Command::PromptFile { tickets, request });
        } else {
            self.command(Command::Prompt {
                generation: self.generation,
                request,
            });
        }
    }
    fn select_model(&mut self) {
        if self.selected.is_none() {
            if let Some(model) = self
                .chosen_model
                .as_ref()
                .filter(|_| !self.new_pending && self.ready && self.options.smoke.is_none())
            {
                self.model_controls.new_selection = Some((
                    self.transport_epoch,
                    self.generation,
                    ModelSelection {
                        provider: model.provider.clone(),
                        model: model.id.clone(),
                        reasoning_effort: self.chosen_effort.as_ref().map(|e| e.id.clone()),
                        service_tier: self.chosen_service_tier.clone(),
                        text_verbosity: self.chosen_text_verbosity.clone(),
                    },
                ));
                self.new_pending = true;
                self.command(Command::Create {
                    workspace: self.workspace.clone(),
                    cwd: self.workspace.is_none().then(|| {
                        self.options
                            .backend
                            .working_directory
                            .to_string_lossy()
                            .into_owned()
                    }),
                });
            }
            return;
        }
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
                        service_tier: self.chosen_service_tier.clone(),
                        text_verbosity: self.chosen_text_verbosity.clone(),
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
                if self.transport_epoch != worker::TRANSPORT_EPOCH {
                    self.catalog_read.retire();
                    self.model_controls.dismiss();
                }
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
                self.request_catalog();
                if !self.stopped && matches!(self.close, Close::None) {
                    return Task::batch([self.load_drafts(false), self.load_sidebar(false)]);
                }
            }
            Event::Core(PublicEvent::CloudflareAuth(status)) => {
                self.settings.cloudflare_observed(status);
                self.continue_cloudflare_connection();
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
                );
                // The panel's own state, so the sign-in and sign-out controls
                // follow the Host rather than a sentence.
                self.settings.account_observed(metadata);
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
                self.pending.disconnect();
                self.files.disconnect();
                self.management.disconnect();
                self.sidebar_organization.dismiss();
                self.exporter.disconnect();
                self.rename.disconnect();
                self.fork.disconnect();
                self.terminal.disconnect();
                self.mcp.disconnect();
                self.settings.disconnect();
                self.account = "Account metadata unavailable (disconnected)".into();
                self.catalog_read.retire();
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
                tickets,
                request_id,
                outcome,
            } => {
                let context = self.file_context();
                for ticket in &tickets {
                    self.files
                        .prompt_complete(ticket, &request_id, outcome, &context);
                }
                if let Some(ticket) = tickets.first()
                    && self.pending.get(&ticket.generation).is_some_and(|pending| {
                        pending.session == ticket.target
                            && pending.file_request_id.as_ref() == Some(&request_id)
                    })
                {
                    let owner = prompt_recovery::Ticket {
                        epoch: ticket.epoch,
                        generation: ticket.generation,
                        session: ticket.target.clone(),
                        request_id: request_id.clone(),
                    };
                    let receipt = match outcome {
                        worker::attachments::PromptOutcome::Accepted => {
                            prompt_recovery::Receipt::Accepted
                        }
                        worker::attachments::PromptOutcome::NotSent => {
                            prompt_recovery::Receipt::NotSent
                        }
                        worker::attachments::PromptOutcome::Unknown => {
                            prompt_recovery::Receipt::Unknown
                        }
                    };
                    if let Some(pending) = self.pending.settle(&owner, receipt) {
                        self.prompt_resolved(pending, ticket.generation, receipt);
                    }
                    if ticket.generation == self.generation {
                        self.status=match outcome {worker::attachments::PromptOutcome::Accepted=>"Attachment prompt accepted into Host inbox; generation/history still come from Host events",worker::attachments::PromptOutcome::NotSent=>"Attachment prompt not admitted; files remain ready",worker::attachments::PromptOutcome::Unknown=>"Attachment Send outcome unknown; do not retry automatically"}.into();
                    }
                }
            }
            Event::Exported { receipt, outcome } => {
                self.exporter.finish(&receipt, outcome);
            }
            Event::Managed { submission, result } => {
                self.management.finish(&submission, result, false);
            }
            Event::CommandsListed { session_id, result } => {
                self.commands_pending = false;
                // An advertisement belongs to the session that answered; a reply
                // for a session the user has left changes nothing.
                if self.selected.as_ref() == Some(&session_id) {
                    // The shortcuts disclosure lists the same advertisement as pairs: the
                    // slash menu's candidate type is private to the UI module tree. A failed
                    // read advertises nothing rather than keeping a stale list.
                    self.settings.commands = match &result {
                        Ok(commands) => commands
                            .iter()
                            .map(|command| (command.name.clone(), command.name.clone()))
                            .collect(),
                        Err(_) => Vec::new(),
                    };
                    match result {
                        Ok(commands) if commands.is_empty() => {
                            self.commands = Some(Vec::new());
                            self.status =
                                "This Host advertises no commands; slash completion is unavailable."
                                    .into();
                        }
                        Ok(commands) => {
                            self.commands = Some(
                                commands
                                    .into_iter()
                                    .map(|command| command_menu::Candidate {
                                        name: command.name,
                                        description: command.description,
                                    })
                                    .collect(),
                            );
                        }
                        // A failed read offers nothing and is not remembered as an
                        // answer. Clearing the in-flight flag is what makes the next
                        // keystroke retry: a session whose context the gateway could
                        // not resolve yet is readable once the session has activity,
                        // and latching here would leave the menu permanently shut. The
                        // failure is also reported, because a silent dead end leaves
                        // the user nothing to act on.
                        Err(failure) => {
                            self.commands = None;
                            self.commands_pending = false;
                            self.status = failure.message();
                        }
                    }
                }
            }
            Event::CommandsExecuted {
                session_id,
                line,
                result,
            } => {
                // Only this session's reply for the command still in flight may
                // clear the pending state; anything else is stale or foreign.
                let expected = self
                    .plan_control()
                    .mode
                    .map(plan_control::toggle_line)
                    .is_some_and(|expected| expected == line);
                if self.selected.as_ref() == Some(&session_id) && expected {
                    self.plan_toggle_pending = false;
                    // The command's own outcome is reported, never interpreted: the
                    // mode the user sees still comes from the `plan` projection, so
                    // an accepted command that changed nothing reads as unchanged.
                    match result {
                        Ok(Some(executed)) => match executed.result {
                            CommandResult::Success { text } => {
                                self.status = text
                                    .filter(|text| !text.trim().is_empty())
                                    .unwrap_or_else(|| "Command accepted".into());
                            }
                            CommandResult::Error { text } => self.status = text,
                        },
                        Ok(None) => self.status = "Command settled without a result".into(),
                        Err(()) => {
                            self.status =
                                "Command outcome unknown; the mode shown is the last one the Host reported."
                                    .into();
                        }
                    }
                }
            }
            Event::Renamed { submission, result } => {
                let outcome = match result {
                    Ok(value) => crate::rename::Outcome::Renamed {
                        title: value.title,
                        seq: value.seq,
                    },
                    Err(()) => crate::rename::Outcome::Unknown,
                };
                // Apply only the acknowledged normalized title, and only for this receipt.
                if let Some(applied) = self.rename.finish(&submission.receipt, outcome) {
                    self.observe_title_for(
                        &applied.target,
                        applied.seq,
                        serde_json::Value::String(applied.title),
                    );
                }
            }
            Event::Forked { submission, result } => {
                let outcome = match result {
                    Ok(value) => crate::fork::Outcome::Forked {
                        child: value.session_id,
                    },
                    Err(crate::fork::Failure::Unavailable) => crate::fork::Outcome::Unavailable,
                    Err(crate::fork::Failure::Unattached) => crate::fork::Outcome::Unattached,
                    Err(crate::fork::Failure::Unknown) => crate::fork::Outcome::Unknown,
                };
                // The acknowledged child id becomes visible state only through this App's
                // own roster or follow frames; an ACK alone never creates a roster entry.
                if let Some(applied) = self.fork.finish(&submission.receipt, outcome) {
                    let visible = self.fork_child_visible(&applied.child);
                    if visible {
                        self.choose(applied.child);
                    }
                    self.fork.reveal(visible);
                }
            }
            Event::TerminalLoaded { ticket, result } => {
                self.terminal.loaded(&ticket, result);
            }
            Event::TerminalFrame { ticket, frame } => {
                self.terminal.frame(&ticket, frame);
            }
            Event::TerminalFollowEnded { ticket, failure } => {
                self.terminal.follow_ended(&ticket, failure);
            }
            Event::TerminalCreated { submission, result } => {
                use crate::terminal::{CreateFailure, CreateOutcome};
                let outcome = match result {
                    Ok(info) => CreateOutcome::Created(Box::new(info)),
                    Err(CreateFailure::Unavailable) => CreateOutcome::Unavailable,
                    Err(CreateFailure::LimitReached) => CreateOutcome::LimitReached,
                    Err(CreateFailure::NotSent) => CreateOutcome::NotSent,
                    Err(CreateFailure::Unknown) => CreateOutcome::Unknown,
                };
                // Only the Host ACK may add and follow the new terminal.
                if let Some(applied) = self.terminal.finish_create(&submission.receipt, outcome) {
                    self.command(Command::TerminalFollow(applied.follow));
                }
            }
            Event::TerminalClosed { submission, result } => {
                use crate::terminal::{CloseFailure, CloseOutcome};
                let outcome = match result {
                    Ok(()) => CloseOutcome::Closed,
                    Err(CloseFailure::Unavailable) => CloseOutcome::Unavailable,
                    Err(CloseFailure::NotSent) => CloseOutcome::NotSent,
                    Err(CloseFailure::Unknown) => CloseOutcome::Unknown,
                };
                self.terminal.finish_close(&submission.receipt, outcome);
            }
            Event::TerminalWritten { submission, result } => {
                use crate::terminal::{WriteFailure, WriteOutcome};
                let outcome = match result {
                    Ok(()) => WriteOutcome::Sent,
                    // A control/unavailable refusal never delivered the input.
                    Err(WriteFailure::ControlUnavailable | WriteFailure::Unavailable) => {
                        WriteOutcome::Refused
                    }
                    Err(WriteFailure::NotSent) => WriteOutcome::NotSent,
                    Err(WriteFailure::Unknown) => WriteOutcome::Unknown,
                };
                self.terminal.finish_write(&submission.receipt, outcome);
            }
            Event::McpLoaded { ticket, loaded } => {
                self.mcp.loaded_snapshot(ticket, loaded);
            }
            // The receipt is the only thing that may change the panel's view of
            // the file; a refused or indeterminate write is never replayed.
            Event::McpWritten {
                submission,
                outcome,
            } => {
                self.mcp.written(&submission, outcome);
            }
            Event::Roster(result) => {
                self.roster_pending = false;
                match result {
                    Ok(roster) => {
                        if roster.items.len() > 4096 {
                            self.status = "Session list exceeds native 4096-row limit".into();
                            self.catalog_read.retire();
                            self.ready = false;
                        } else {
                            self.sessions = roster
                                .items
                                .into_iter()
                                .map(|s| (s.session_id.clone(), s))
                                .collect();
                            self.title_updates
                                .retain(|id, _| self.sessions.contains_key(id));
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
                    self.sidebar_organization.dismiss();
                    self.workspaces = value.items;
                    if !self
                        .management
                        .baseline(value.archived_session_ids, value.pinned_session_ids)
                    {
                        self.status = "Registry snapshot malformed or exceeds native bound; management disabled".into();
                    }
                }
                WorkspaceFollowFrame::Upsert { workspace } => {
                    self.sidebar_organization.dismiss();
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
                    self.sidebar_organization.dismiss();
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
                    self.sidebar_organization.dismiss();
                    if !self.management.archives(archived_session_ids) {
                        self.status =
                            "Registry archive feed not authoritative; management disabled".into();
                    }
                }
                WorkspaceFollowFrame::Pinned { pinned_session_ids } => {
                    self.sidebar_organization.dismiss();
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
                        self.catalog_read.retire();
                        self.ready = false;
                    }
                } else {
                    self.root(frame, decision_key);
                }
            }
            Event::Selected { generation, frame } if generation == self.generation => {
                changed |= self.host_selected(generation, frame);
            }
            Event::Created(result) => {
                self.new_pending = false;
                let deferred =
                    self.model_controls
                        .new_selection
                        .take()
                        .filter(|(epoch, generation, _)| {
                            *epoch == self.transport_epoch
                                && *generation == self.generation
                                && self.selected.is_none()
                                && self.ready
                                && !self.stopped
                                && matches!(self.close, Close::None)
                        });
                match result {
                    Ok(value) => {
                        self.smoke_evidence.session_created = true;
                        self.choose(value.session_id);
                        if let Some((_, _, selection)) = deferred {
                            self.model_pending = true;
                            self.command(Command::SelectModel {
                                generation: self.generation,
                                request: SelectModelRequest {
                                    session_id: self.selected.clone().unwrap(),
                                    selection,
                                },
                            });
                        }
                    }
                    Err(error) => {
                        self.status = error;
                        self.reconcile_model_selection();
                    }
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
            Event::Catalog { ticket, result } => {
                if !self.catalog_read.complete(ticket, self.transport_epoch)
                    || !self.ready
                    || self.observations_lost
                    || self.stopped
                    || !matches!(self.close, Close::None)
                {
                    return Task::none();
                }
                self.model_controls.catalog_failed(result.is_err());
                match result {
                    Ok(catalog) => {
                        self.model_controls.catalog_default = Some(catalog.default.clone());
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
                                        name: model.name,
                                        service_tiers: model_controls::Choice::from_catalog(
                                            model.service_tier.map_or(vec![], |t| t.tiers),
                                            model_controls::Section::Speed,
                                        ),
                                        verbosity_levels: model_controls::Choice::from_catalog(
                                            model.text_verbosity.map_or(vec![], |v| v.levels),
                                            model_controls::Section::OutputDetail,
                                        ),
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
                        self.reconcile_model_selection();
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
                self.reconcile_model_selection();
            }
            Event::ObservationsLost => {
                self.catalog_read.retire();
                self.observations_lost = true;
                self.rejoin_pending = false;
                self.root_ready = false;
                self.follow_ready = false;
                self.pending.disconnect();
                self.files.invalidate();
                self.management.disconnect();
                self.sidebar_organization.dismiss();
                self.decisions.disconnect();
                self.model_controls.dismiss();
                self.terminal.disconnect();
                self.mcp.disconnect();
                self.status="Observation connection lost; original Host identity not re-verified. Check/reconnect explicitly; no automatic restart or session resume.".into();
            }
            Event::ObservationsRejoined { epoch, result }
                if epoch == self.transport_epoch && self.ready =>
            {
                self.rejoin_pending = false;
                match result {
                    Ok(()) => {
                        self.observations_lost = false;
                        self.status="Same owned Host verified; root observations rejoined. Check original submission, then select a session explicitly to resume.".into();
                    }
                    Err(error) => self.status = error,
                }
            }
            Event::Prompt {
                generation,
                request_id,
                result,
            } => {
                if let Some(ticket) = self.pending.ticket().filter(|t| {
                    t.generation == generation
                        && t.request_id == request_id
                        && t.epoch == self.transport_epoch
                        && self
                            .pending
                            .get(&generation)
                            .is_some_and(|p| p.file_request_id.is_none())
                }) {
                    let receipt = if result.as_ref().is_ok_and(|accepted| accepted.accepted) {
                        prompt_recovery::Receipt::Accepted
                    } else {
                        prompt_recovery::Receipt::Unknown
                    };
                    if let Some(pending) = self.pending.settle(&ticket, receipt) {
                        self.prompt_resolved(pending, generation, receipt);
                    } else {
                        self.status="Submission outcome unknown. Original intent retained; do not resend. Check submission explicitly.".into();
                    }
                }
            }
            Event::PromptNotSent { ticket } => {
                if let Some(pending) = self
                    .pending
                    .settle(&ticket, prompt_recovery::Receipt::NotSent)
                {
                    self.prompt_resolved(
                        pending,
                        ticket.generation,
                        prompt_recovery::Receipt::NotSent,
                    );
                }
            }
            Event::SubmissionChecked { ticket, value } => {
                if let Some((pending, receipt)) = self.pending.checked(&ticket, value) {
                    if !pending.file_ticket.is_empty() {
                        let context = self.file_context();
                        let accepted = receipt == prompt_recovery::Receipt::Accepted;
                        for file in &pending.file_ticket {
                            self.files.reconcile_prompt(
                                file,
                                &pending.request.request_id,
                                accepted,
                                &context,
                            );
                        }
                    }
                    self.prompt_resolved(pending, ticket.submission.generation, receipt);
                } else if self.pending.ticket().as_ref() == Some(&ticket.submission) {
                    self.status="No verified admission decision. Original submission still owned; no resend permission. Check again only explicitly.".into();
                }
            }
            Event::Cancel { generation, result }
                if generation == self.generation || self.cancel_generation == Some(generation) =>
            {
                self.cancel_pending = false;
                self.cancel_generation = None;
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
                Ok(inspection) if inspection.is_known_idle() && self.pending.is_empty() => {
                    self.shutdown()
                }
                Ok(_) if !self.pending.is_empty() => {
                    self.close=Close::Confirm("Original submission is unresolved. Closing stops only this owned Host and discards local recovery state; it is not rollback or permission to resend.".into());
                }
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
            Event::AccountCompleted { ticket, outcome } => {
                self.settings.account_completed(ticket, outcome);
            }
            Event::CloudflareCompleted { ticket, outcome } => {
                self.settings.cloudflare_completed(ticket, outcome);
                self.continue_cloudflare_connection();
            }
            Event::KeyMetadata { ticket, result } => self.settings.metadata(ticket, result),
            Event::KeySaved { ticket, result } => self.settings.saved(ticket, result),
            Event::Fault(error) => {
                // A Core start failure arrives here as `Core start: <cause>`
                // (worker.rs publishes it before `Event::Stopped`), and the shell
                // keeps a bounded report for exactly that class of failure
                // (`main.js:7647-7742`). Written before the status is replaced.
                self.record_start_failure(&error);
                self.pending.disconnect();
                self.files.disconnect();
                self.management.disconnect();
                self.sidebar_organization.dismiss();
                self.exporter.disconnect();
                self.rename.disconnect();
                self.fork.disconnect();
                self.terminal.disconnect();
                self.mcp.disconnect();
                self.settings.disconnect();
                self.account = "Account metadata unavailable (disconnected)".into();
                self.catalog_read.retire();
                self.ready = false;
                self.root_ready = false;
                self.decisions.disconnect();
                self.status = error;
            }
            Event::NoBackendStarted => {
                self.files.disconnect();
                self.management.disconnect();
                self.sidebar_organization.dismiss();
                self.exporter.disconnect();
                self.rename.disconnect();
                self.fork.disconnect();
                self.terminal.disconnect();
                self.mcp.disconnect();
                self.settings.disconnect();
                self.account = "Account metadata unavailable (disconnected)".into();
                self.catalog_read.retire();
                self.ready = false;
                self.stopped = true;
                self.status = "No owned backend was started".into();
                if matches!(self.close, Close::Stopping) {
                    return self.finish_draft_exit();
                }
            }
            Event::Stopped(result) => {
                self.pending.disconnect();
                self.files.disconnect();
                self.management.disconnect();
                self.sidebar_organization.dismiss();
                self.exporter.disconnect();
                self.rename.disconnect();
                self.fork.disconnect();
                self.terminal.disconnect();
                self.mcp.disconnect();
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
                self.catalog_read.retire();
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
                            return self.finish_draft_exit();
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
            Event::JobKilled(result) => {
                // A refusal keeps the panel as it was and says so; a receipt needs no words.
                self.jobs_kill_error = result.err();
            }
            Event::JobsRows(rows) => {
                // Whole-set replacement: a reconnect's first frame is already the truth.
                self.jobs = rows;
            }
            Event::JobsFollow {
                job: followed,
                frame,
            } => {
                use dsh_native_transport::dto::JobFollowFrame;
                match frame {
                    // A job opening or changing state updates that row; output is not consumed
                    // until the panel's expansion exists, and saying so beats pretending.
                    JobFollowFrame::Opened { job, .. } | JobFollowFrame::Status { job } => {
                        match self.jobs.iter_mut().find(|row| row.id == job.id) {
                            Some(row) => *row = job,
                            None => self.jobs.push(job),
                        }
                    }
                    JobFollowFrame::Output { chunks, lossy, .. } => {
                        // The event carries the job id, because the frame itself does not: the
                        // stream is per job, so the worker is the only party that knows which row
                        // these bytes belong to. The gap copy is the reference's own
                        // (`locales.ts:75-76`).
                        let buffer = self.jobs_output.entry(followed.clone()).or_default();
                        if lossy == Some(true) && !buffer.is_empty() {
                            buffer.push_str("… earlier output dropped …\n");
                        }
                        for chunk in chunks {
                            if chunk.gap_before == Some(true) && !buffer.is_empty() {
                                buffer.push_str("… earlier output dropped …\n");
                            }
                            buffer.push_str(&chunk.text);
                        }
                    }
                }
            }
            Event::AgentPresetSelected(result) => {
                match result {
                    // The Host returns the committed identity, so the seat's own value follows
                    // the Host rather than the click.
                    Ok(_) => self.agent_seat_error = None,
                    Err(message) => self.agent_seat_error = Some(message),
                }
            }
            Event::PermissionCatalog(result) => {
                if let Ok(catalog) = result {
                    self.permission_catalog = Some(catalog);
                }
            }
            Event::AgentPresets(result) => {
                if let Ok(roster) = result {
                    self.agent_roster = Some(roster);
                }
            }
            Event::PermissionSettings(result) => {
                if let Ok(settings) = result {
                    self.permission_settings = Some(settings);
                }
            }
            Event::PermissionDefaultSet(result) => {
                self.permission_default_saving = false;
                // A successful write changes the descriptor, so the row re-reads it rather
                // than assuming the new value; a refusal leaves the old one in place.
                if result.is_ok() {
                    self.command(Command::PermissionSettingsRead);
                }
            }
            _ => {}
        }
        self.reconcile_local_files();
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

    /// Fold one follow frame for the selected session and report whether the
    /// transcript changed, so the caller can refresh the scroll geometry. The
    /// caller has already matched the frame to this App generation.
    fn host_selected(&mut self, generation: u64, frame: SessionFollowFrame) -> bool {
        let mut changed = false;
        if matches!(&frame, SessionFollowFrame::Snapshot { .. }) {
            self.reply_content.clear();
            self.thoughts_expanded.clear();
            self.activity_expanded.clear();
        }
        if matches!(&frame, SessionFollowFrame::AssistantStream { .. }) {
            self.reply_content.clear_live();
        }
        let snapshot = matches!(&frame, SessionFollowFrame::Snapshot { .. });
        let title_change = match &frame {
            SessionFollowFrame::Snapshot { projections, .. } => projections
                .values
                .get("title")
                .map(|value| (projections.as_of_seq, value.clone())),
            SessionFollowFrame::Event { event } if event.event_type == "session/title" => {
                event.data.get("title").and_then(|value| {
                    i64::try_from(event.seq)
                        .ok()
                        .filter(|seq| *seq > self.transcript.cursor())
                        .map(|seq| (seq, value.clone()))
                })
            }
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
                        HistoryRecord::Event { event } => crate::smoke::kind(&event.event_type),
                    })
                    .collect();
            }
            if let (Some(id), Some(value)) = (
                self.selected.clone(),
                projections
                    .values
                    .get("modelSelection")
                    .and_then(|v| v.get("next")),
            ) && let Ok(selection) = serde_json::from_value::<ModelSelection>(value.clone())
            {
                self.model_selections.insert(id, selection);
                self.reconcile_model_selection();
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
                // Follow delivery dispatches nothing. The offer is read from
                // the projection the frame carries, so no round trip is needed
                // to know whether this Host mounts plan mode.
                self.plan_published = Some(plan_mode::published(self.transcript.projections()));
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
        changed
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
                    ) && let Some(session) = self.sessions.get_mut(&id)
                    {
                        session.running = running;
                    }
                }
                "api-session/activity" => {
                    if let (Some(id), Some(updated)) = (
                        args.first()
                            .and_then(|v| v.as_str())
                            .and_then(|v| SessionId::new(v).ok()),
                        args.get(1).and_then(|v| v.as_i64()),
                    ) && let Some(session) = self.sessions.get_mut(&id)
                    {
                        session.updated_at = updated;
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
        self.management_context_for(self.management.target().or(self.selected.as_ref()))
    }
    /// True when every management flight except `own` is open or pending. Each
    /// panel passes itself, so a panel never blocks on its own in-flight write —
    /// the worker's own guard decides that.
    fn other_flights_idle(&self, own: Flight) -> bool {
        let open = |flight: Flight| match flight {
            Flight::Management => self.management.is_open() || self.management.pending(),
            Flight::Rename => self.rename.is_open() || self.rename.pending().is_some(),
            Flight::Fork => self.fork.is_open() || self.fork.pending().is_some(),
            Flight::Export => self.exporter.is_open() || self.exporter.pending().is_some(),
            Flight::Terminal => self.terminal.busy(),
            Flight::Mcp => self.mcp.pending(),
        };
        Flight::ALL
            .into_iter()
            .all(|flight| flight == own || !open(flight))
    }
    fn management_context_for(&self, target: Option<&SessionId>) -> crate::management::Context {
        let mut context = self.operator_context_for(target);
        context.allowed = context.allowed && self.other_flights_idle(Flight::Management);
        context
    }
    fn rename_context(&self) -> crate::rename::Context {
        let ticket = self.rename.ticket();
        let target = ticket
            .as_ref()
            .map(|ticket| &ticket.target)
            .or(self.selected.as_ref());
        self.rename_context_for(target)
    }
    fn rename_context_for(&self, target: Option<&SessionId>) -> crate::rename::Context {
        let context = self.operator_context_for(target);
        crate::rename::Context {
            view: context.view,
            allowed: context.allowed && self.other_flights_idle(Flight::Rename),
        }
    }
    fn fork_context(&self) -> crate::fork::Context {
        let ticket = self.fork.ticket();
        let target = ticket
            .as_ref()
            .map(|ticket| &ticket.target)
            .or(self.selected.as_ref());
        self.fork_context_for(target)
    }
    /// A forked child may be revealed only when this App already holds it in its own
    /// authoritative roster or a workspace follow frame. An ACK id alone proves the
    /// child exists on the Host, not that this App has a row for it.
    fn fork_child_visible(&self, child: &SessionId) -> bool {
        self.sessions.contains_key(child)
            || self
                .workspaces
                .iter()
                .any(|workspace| workspace.session_ids.contains(child))
    }
    fn fork_context_for(&self, target: Option<&SessionId>) -> crate::fork::Context {
        let context = self.operator_context_for(target);
        crate::fork::Context {
            view: context.view,
            allowed: context.allowed && self.other_flights_idle(Flight::Fork),
        }
    }
    fn export_context(&self) -> crate::exporter::Context {
        let ticket = self.exporter.ticket();
        self.export_context_for(
            ticket
                .as_ref()
                .map(|ticket| &ticket.target)
                .or(self.selected.as_ref()),
        )
    }
    fn export_context_for(&self, target: Option<&SessionId>) -> crate::exporter::Context {
        let context = self.operator_context_for(target);
        crate::exporter::Context {
            view: context.view,
            allowed: context.allowed && self.other_flights_idle(Flight::Export),
        }
    }
    /// The terminal panel reads and mutates only the selected Session's Host terminals.
    /// Create/close share the app's single management flight with rename/fork/export.
    fn terminal_context(&self) -> crate::terminal::Context {
        let target = self
            .terminal
            .target()
            .cloned()
            .or_else(|| self.selected.clone());
        self.terminal_context_for(target.as_ref())
    }
    fn terminal_context_for(&self, target: Option<&SessionId>) -> crate::terminal::Context {
        let context = self.operator_context_for(target);
        crate::terminal::Context {
            view: context.view,
            allowed: context.allowed && self.other_flights_idle(Flight::Terminal),
        }
    }
    /// The MCP panel is app-global: it manages the private home's composition
    /// file, not one session. It stays available while the Host is live and no
    /// other management flight is open or pending.
    fn mcp_context(&self) -> crate::mcp::Context {
        crate::mcp::Context {
            epoch: self.transport_epoch,
            visible: self.mcp.is_open(),
            enabled: self.options.smoke.is_none()
                && self.ready
                && !self.stopped
                && matches!(self.close, Close::None)
                && !self.settings.is_open()
                && self.other_flights_idle(Flight::Mcp)
                && !self.mcp.pending(),
        }
    }
    /// Map one MCP model effect onto the worker. A read reserves nothing; the
    /// reviewed write is the only command that touches the composition file.
    fn dispatch_mcp(&mut self, effect: crate::mcp::Effect) {
        match effect {
            crate::mcp::Effect::Read(ticket) => self.command(Command::McpRead(ticket)),
            crate::mcp::Effect::Write(submission) => self.command(Command::McpWrite(submission)),
        }
    }
    /// Map one terminal model effect onto the worker. Only mutating effects reserve
    /// the management flight; a read or follow never blocks another operation.
    fn dispatch_terminal(&mut self, effect: crate::terminal::Effect) {
        match effect {
            crate::terminal::Effect::Read(ticket) => self.command(Command::TerminalRead(ticket)),
            crate::terminal::Effect::Follow(ticket) => {
                self.command(Command::TerminalFollow(ticket))
            }
            crate::terminal::Effect::Create(submission) => {
                self.command(Command::TerminalCreate(submission))
            }
            crate::terminal::Effect::Close(submission) => {
                self.command(Command::TerminalClose(submission))
            }
            crate::terminal::Effect::Write(submission) => {
                self.command(Command::TerminalWrite(submission))
            }
        }
    }
    /// Read-only context for the selected session. The mutation path assembles its
    /// own context from a ticket, so only the export-policy tests read this.
    #[cfg(test)]
    fn operator_context(&self) -> crate::management::Context {
        self.operator_context_for(self.selected.as_ref())
    }
    fn operator_context_for(&self, target: Option<&SessionId>) -> crate::management::Context {
        let observed = target.and_then(|id| self.sessions.get(id));
        crate::management::Context {
            view: target.map(|target| crate::management::View {
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
    fn continue_cloudflare_connection(&mut self) {
        let enabled = self.settings_enabled();
        if let Some(effect) = self.settings.cloudflare_continue(enabled) {
            self.command(Command::Cloudflare(effect));
        }
    }
    fn continue_codex_connection(&mut self) {
        let enabled = self.settings_enabled();
        if let Some(effect) = self.settings.codex_continue(enabled) {
            self.command(Command::Codex(effect));
        }
    }
    fn settings_enabled(&self) -> bool {
        self.pending.is_empty()
            && !self.observations_lost
            && self.exporter.pending().is_none()
            && !self.exporter.is_open()
            && self.options.smoke.is_none()
            && self.ready
            && !self.stopped
            && matches!(self.close, Close::None)
    }
    /// Panel ticket for a Wayland Ctrl+V on the Models key field, when the live
    /// draft accepts a paste. `None` keeps the shortcut out of the subscription.
    fn settings_paste_ticket(&self) -> Option<crate::settings::Ticket> {
        let ticket = self.settings.input_ticket();
        self.settings
            .paste_allowed(ticket, self.transport_epoch, self.settings_enabled())
            .then_some(ticket)
    }
    /// Whether a bare Escape belongs to the running turn's stop sequence instead of the close
    /// command: a live selected turn, the composer surface visible, and no rename/fork overlay.
    /// That is the reference's eligible region (`stop-shortcut.ts`), minus what native cannot
    /// observe (the focused DOM target).
    fn stop_escape_eligible(&self) -> bool {
        self.stop_allowed()
            && self.composer_visible()
            && !self.rename.is_open()
            && !self.fork.is_open()
            && self
                .selected
                .as_ref()
                .and_then(|id| self.sessions.get(id))
                .is_some_and(|session| session.running)
    }
    /// Two independent Escape presses addressed to the same live turn stop it; one only arms.
    /// A mismatch or expiry starts a new first press, as the reference's `StopSequence` does.
    fn stop_sequence_press(
        &mut self,
        key: &keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Option<Task<Message>> {
        if !modifiers.is_empty()
            || !matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape))
            || !self.stop_escape_eligible()
        {
            self.stop_sequence = None;
            return None;
        }
        let now = Instant::now();
        if let Some((deadline, session, generation)) = self.stop_sequence.take()
            && now <= deadline
            && self.selected.as_ref() == Some(&session)
            && generation == self.generation
        {
            return Some(self.update(Message::Stop));
        }
        if let Some(session) = self.selected.clone() {
            self.stop_sequence = Some((now + STOP_SEQUENCE_WINDOW, session, self.generation));
        }
        Some(Task::none())
    }
    /// The live key chain. Order is the contract: an armed capture changes what the next press
    /// means; a focused field's own links keep copy and paste; the global trigger is fixed; then
    /// the stored bindings decide, and a key no current binding holds does nothing rather than
    /// falling back to the default it was moved away from.
    fn key_pressed(&mut self, key: keyboard::Key, modifiers: keyboard::Modifiers) -> Task<Message> {
        use crate::keybindings::Invoke;
        // A capture wins over routing: the press *becomes* a binding rather than invoking
        // anything, bounded to the armed row (`Editor.tsx:59-71`).
        if let Some(id) = self.settings.capture_for.clone() {
            use keyboard::key::Named;
            // A modifier press is not a candidate: the reference ignores it rather than calling
            // it unsupported, so the arming stays ready for the character that follows.
            if matches!(
                key,
                keyboard::Key::Named(
                    Named::Alt
                        | Named::AltGraph
                        | Named::Control
                        | Named::Shift
                        | Named::Meta
                        | Named::Hyper
                        | Named::Super
                )
            ) {
                return Task::none();
            }
            let Some(binding) = crate::keybindings::binding_from(&key, modifiers) else {
                self.settings.capture_error =
                    Some(crate::keybindings::COPY_UNSUPPORTED_KEY.to_owned());
                return Task::none();
            };
            if crate::keybindings::needs_modifier(&binding) {
                self.settings.capture_error =
                    Some(crate::keybindings::COPY_MODIFIER_REQUIRED.to_owned());
                return Task::none();
            }
            if let Some(other) = self.settings.keybindings.conflict(&id, &binding) {
                let label = crate::keybindings::native_catalog()
                    .into_iter()
                    .chain(crate::keybindings::catalog_rows(&self.settings.commands))
                    .find(|(known, _, _)| known == other)
                    .map(|(_, label, _)| label)
                    .unwrap_or_else(|| other.to_owned());
                self.settings.capture_error =
                    Some(crate::keybindings::COPY_CONFLICT.replace("{commands}", &label));
                return Task::none();
            }
            let _ = self.settings.keybindings.apply(
                crate::keybindings::Edit::Set {
                    id,
                    binding: Some(binding),
                },
                self.settings.keybindings.revision(),
            );
            self.settings.capture_for = None;
            self.settings.capture_error = None;
            return Task::none();
        }
        // A running turn's two-press Escape wins over the close command while the conversation
        // surface is visible; the first press is consumed exactly as the reference consumes it.
        if let Some(task) = self.stop_sequence_press(&key, modifiers) {
            return task;
        }
        // The field-level links own these keys before any command is consulted, so a stored
        // binding cannot swallow Ctrl+C or Ctrl+V from the field that needs it.
        // Each routed message is applied through `update` now, not returned as a `Task` the
        // runtime would apply on the next pass: that keeps the live chain synchronous, and the
        // gates that own a modal see the message exactly as they would from a direct press.
        if let Some(message) = settings_paste_message(
            wayland_session(),
            self.settings_paste_ticket(),
            &key,
            modifiers,
        ) {
            return self.update(message);
        }
        if let Some(message) = clipboard_field_shortcut(&key, modifiers) {
            return self.update(message);
        }
        // `shortcuts.open` is fixed (`primary + Slash`); the reference does not expose it as an
        // editable command either.
        if crate::keybindings::is_trigger(&key, modifiers) {
            return self.update(Message::ShortcutsOpen);
        }
        if let Some(id) = crate::keybindings::resolve(&self.settings.keybindings, &key, modifiers) {
            let invoke = crate::keybindings::invoke_for(id);
            if invoke == Some(Invoke::Close) {
                // The visible Details sheet is the topmost dismissal, as the tagged route always
                // was; the command decides the key, the surface decides what Escape means here.
                return match self.detail_escape_ticket() {
                    Some(ticket) => self.update(Message::HideDetails(ticket)),
                    None => self.update(Message::DismissPanel),
                };
            }
            if let Some(message) = invoke_message(invoke) {
                return self.update(message);
            }
        }
        // A press whose command was cleared or moved is dead, not a second chance for the
        // hardcoded handler: this is what makes a stored binding *replace* a default.
        Task::none()
    }
    /// DeepSeek route facts the Settings page renders. Read-only projection of
    /// the real catalog read; never invents a model or an endpoint.
    fn settings_route_facts(&self) -> crate::settings::RouteFacts {
        let active = self.effective_model_selection();
        let mut models: Vec<(String, String, bool)> = self
            .models
            .iter()
            .filter(|model| deepseek_route(model))
            .map(|model| {
                let is_active = active.is_some_and(|selection| {
                    selection.provider == model.provider && selection.model == model.id
                });
                (model.name.clone(), model.id.clone(), is_active)
            })
            .collect();
        models.sort();
        // The General page's permission row reads the descriptor and the catalog's own
        // advertised new-session presets, so it never invents an option.
        let permission = self.permission_settings.as_ref().map(|settings| {
            let options: Vec<(String, String)> = self
                .permission_catalog
                .as_ref()
                .map(|catalog| {
                    let advertised = if catalog.default_options.is_empty() {
                        &catalog.options
                    } else {
                        &catalog.default_options
                    };
                    advertised
                        .iter()
                        .map(|option| (option.value.clone(), option.name.clone()))
                        .collect()
                })
                .unwrap_or_default();
            crate::settings::PermissionDefault {
                current: settings.current_value.clone(),
                revision: settings.revision,
                writable: settings.writable,
                options,
                saving: self.permission_default_saving,
            }
        });
        crate::settings::RouteFacts {
            permission,
            models,
            loading: self.catalog_read.pending(),
            failed: self.model_controls.catalog_is_failed(),
            busy_enter: self.busy_enter,
        }
    }
    /// The content-size delta for the body tier, and for the secondary tier that
    /// carries the de-emphasized text. Both are zero at the default size, so
    /// nothing about the shipped layout moves until a preference is set.
    fn content_delta(&self) -> f32 {
        design::content_delta(self.content_font_size)
    }
    /// The height the referenced-session summary adds to a row: the user stack's gap
    /// plus the sheet's `18px + delta_secondary` line, or zero when the row cites
    /// nothing (`ui-chat/src/client/chat/MessageItem.module.css:42-45`).
    fn reference_summary_height(&self, row: &DisplayRow, lane: f32) -> f32 {
        chat_geometry::reference_summary_height(
            &row.references,
            lane,
            self.content_delta_secondary(),
        )
    }
    fn content_delta_secondary(&self) -> f32 {
        design::content_delta_secondary(self.content_font_size)
    }
    /// Whether a tool row's leading box should show the disclosure chevron
    /// because the pointer is over it.
    fn activity_hovered(&self, key: u64) -> bool {
        if self.hovered_activity == Some(key) {
            return true;
        }
        #[cfg(feature = "public-layout-fixture")]
        if self.hover_all_activities {
            return true;
        }
        false
    }
    /// Render every tool row as hovered, for the public layout fixture.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_hover_all_activities(&mut self, hover: bool) {
        self.hover_all_activities = hover;
    }
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_width_handle_glow(&mut self, glow: bool) {
        self.width_handle_glow = glow;
    }
    /// Fixture-only: hold a history page in flight, so a capture shows the
    /// history control's disabled loading form.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_page_pending(&mut self, pending: bool) {
        self.page_pending = pending;
    }
    /// Fixture-only: render the transcript as scrolled away from its tail, so a
    /// capture shows the jump-to-latest control that otherwise needs a scroll.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_scrolled_away(&mut self, away: bool) {
        if away {
            self.follow_bottom = false;
        }
    }
    /// Fixture-only: open the permission chip's menu, which a real run opens by pressing
    /// the chip itself.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_permission_menu(&mut self, open: bool) {
        self.permission_menu = open;
    }
    /// Fixture-only: hold the full-access confirmation open, which a real run reaches by
    /// picking Full access from the chip's menu.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_permission_confirm(&mut self, open: bool) {
        self.permission_confirm = open.then(|| "full-access".to_owned());
        self.permission_acknowledged = false;
    }
    /// Fixture-only: open the settings modal, so a capture can show the General page's
    /// rows without a real Settings press.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_settings_open(&mut self) {
        // A fixture renders state; it dispatches no Host effects. The production open path is
        // covered by App tests, and the disabled handles keep every capture at zero commands.
        let _ = self
            .settings
            .handle(crate::settings::Action::Open, self.transport_epoch, false);
        // The modal opens on the account page; the General rows live behind this select.
        let _ = self.settings.handle(
            crate::settings::Action::SelectPage(crate::settings::Page::General),
            self.transport_epoch,
            false,
        );
    }
    /// Fixture-only: hold the seat's refusal state open, which a real run reaches by picking a
    /// preset for a session that has already started.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_agent_seat_error(&mut self, message: &str) {
        self.agent_seat_error = Some(message.to_owned());
    }
    /// Fixture-only: clear one binding through the store, so the count line is visible.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn customise_one_shortcut(&mut self) {
        // A *clear*, not a reset: `Reset` restores the default, which is not a modification.
        let _ = self.settings.keybindings.apply(
            crate::keybindings::Edit::Set {
                id: "native.find".to_owned(),
                binding: None,
            },
            0,
        );
    }

    /// Fixture-only: expand one turn-trigger notice, which a real run expands by press.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn expand_trigger(&mut self, key: u64) {
        self.trigger_expanded.insert(key);
    }

    /// Fixture-only: open the queue dock's multi-item list, which a real run expands by press.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn expand_queue(&mut self) {
        self.queue_expanded = true;
    }

    /// Fixture-only: set the busy-state Enter preference, which a real run changes in Settings.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_busy_enter(&mut self, value: crate::settings::BusyEnter) {
        self.busy_enter = value;
    }

    /// Fixture-only: register one pending approval for the selected session, so the sidebar's
    /// "Waiting for approval" status is capturable without a Host request.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn seed_pending_approval(&mut self) {
        self.decisions.seed_approval(self.transport_epoch);
    }

    /// Fixture-only: open the header's subagent catalog dropdown, which a real run opens by
    /// pressing the count action.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_subagent_menu(&mut self, open: bool) {
        self.subagent_menu = open;
    }

    /// Fixture-only: pre-expand one catalog branch, which a real run opens by pressing its
    /// chevron.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn expand_subagent_branch(&mut self, id: &str) {
        self.subagent_expanded.insert(id.to_owned());
    }

    /// Fixture-only: open the shortcuts disclosure, so its rows are capturable.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_shortcuts_open(&mut self, open: bool) {
        self.settings.shortcuts_open = open;
    }

    /// Fixture-only: prefill the shortcuts search, so its filtered and empty states are
    /// capturable without typing.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_shortcuts_query(&mut self, query: &str) {
        self.settings.shortcuts_query = query.to_owned();
    }

    /// Fixture-only: stand in for the General page's session-log namespace reply, through the
    /// same admitted read a Host reply passes. The value is the one boolean the row edits.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn seed_session_log(&mut self) {
        self.settings
            .seed_plugin_snapshot(Self::session_log_snapshot(true, 4));
    }

    /// Fixture-only: seed the namespace and complete one real write, so the reference's
    /// "Preference saved" state is capturable.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn seed_session_log_saved(&mut self) {
        self.seed_session_log();
        let Some(crate::settings::Effect::Plugins(crate::plugins::Effect::Save { ticket, .. })) =
            self.settings.handle(
                crate::settings::Action::SessionLog(false),
                self.transport_epoch,
                true,
            )
        else {
            return;
        };
        self.settings.plugin_saved(
            ticket,
            crate::plugins::SaveOutcome::Confirmed(Self::session_log_namespace(false, 5)),
        );
    }

    /// Fixture-only: arm the restore-all confirmation, so its copy is capturable.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn arm_reset_all(&mut self) {
        self.settings.reset_all_armed = true;
    }

    /// Fixture-only: arm the capture branch for one row, so the reference's recorder copy is
    /// capturable without a pointer press.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn arm_shortcut_capture(&mut self) {
        self.settings.capture_for = Some("native.find".to_owned());
    }

    /// Fixture-only: the one namespace the General page's session-log row renders.
    #[cfg(feature = "public-layout-fixture")]
    fn session_log_snapshot(enabled: bool, revision: u64) -> crate::plugins::Snapshot {
        crate::plugins::Snapshot {
            inventory: dsh_native_transport::plugin::PluginInventorySnapshot {
                entries: Vec::new(),
                management_available: false,
            },
            settings: dsh_native_transport::plugin::SettingsDescribeValue {
                writable: true,
                namespaces: vec![Self::session_log_namespace(enabled, revision)],
            },
        }
    }
    /// Fixture-only: the namespace view a confirmed write returns at the next revision.
    #[cfg(feature = "public-layout-fixture")]
    fn session_log_namespace(
        enabled: bool,
        revision: u64,
    ) -> dsh_native_transport::plugin::SettingsNamespaceView {
        use dsh_native_transport::plugin::{
            SettingsField, SettingsFieldKind, SettingsNamespaceView, SettingsScalar,
        };
        SettingsNamespaceView {
            ns: "session-log-deepseek".into(),
            revision,
            auto_generate: true,
            unsupported_fields: 0,
            secret_fields: 0,
            fields: vec![SettingsField {
                path: vec!["enabled".into()],
                label: "Upload session logs".into(),
                kind: SettingsFieldKind::Bool,
                value: Some(SettingsScalar::Bool(enabled)),
                overridden: false,
            }],
        }
    }

    /// Fixture-only: seed the Host's advertised commands as the pairs the disclosure reads.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn seed_commands(&mut self) {
        self.settings.commands = vec![
            ("session.compact".to_owned(), "session.compact".to_owned()),
            ("session.fork".to_owned(), "session.fork".to_owned()),
        ];
    }

    /// Fixture-only: seed background jobs, so the counted header action and its panel are
    /// capturable without a running Host.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_jobs(&mut self) {
        let row =
            |id: &str, kind: &str, label: &str, status: &str| dsh_native_transport::dto::JobView {
                id: id.to_owned(),
                kind: kind.to_owned(),
                label: label.to_owned(),
                owner: None,
                status: status.to_owned(),
            };
        // The panel is opened in the fixture, and the first row is armed: the closed action state
        // and the unarmed rows both have their own captures.
        self.jobs_open = true;
        self.jobs_armed = Some("shell-1".to_owned());
        self.jobs_expanded = Some("shell-1".to_owned());
        self.jobs_output.insert(
            "shell-1".to_owned(),
            "… earlier output dropped …\nrunning 4 tests\ntest widget::layout ... ok\ntest widget::paint ... ok"
                .to_owned(),
        );
        self.jobs = vec![
            row("shell-1", "shell", "Run the test suite", "running"),
            row("build-2", "build", "Build the release binary", "stopping"),
            row("grep-3", "search", "Search for parity notes", "completed"),
            row("lint-4", "shell", "Lint the app crate", "failed"),
        ];
    }
    /// Fixture-only: open the hero preset seat's list, so its rows are capturable.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_agent_seat_open(&mut self) {
        self.agent_seat_open = true;
    }
    /// Fixture-only: stand in for the Host's roster reply, so the label's display name is
    /// capturable without a running Host.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_agent_roster(&mut self) {
        self.agent_roster = Some(dsh_native_core::AgentPresetRoster {
            presets: vec![
                dsh_native_core::AgentPresetRow {
                    id: "implementer".to_owned(),
                    is_default: true,
                    name: Some("Implementer".to_owned()),
                    description: None,
                    broken: None,
                },
                dsh_native_core::AgentPresetRow {
                    id: "reviewer".to_owned(),
                    is_default: false,
                    name: Some("Reviewer".to_owned()),
                    description: None,
                    broken: None,
                },
                // A declared preset whose activation failed: visible, and unchoosable.
                dsh_native_core::AgentPresetRow {
                    id: "scout".to_owned(),
                    is_default: false,
                    name: Some("Scout".to_owned()),
                    description: None,
                    broken: Some("activation failed".to_owned()),
                },
            ],
        });
    }
    /// Fixture-only: stand in for the settings descriptor reply, so the permission row is
    /// capturable without a running Host.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_permission_settings(&mut self, writable: bool) {
        self.permission_settings = Some(dsh_native_core::PermissionSettingsState {
            current_value: "workspace-write".to_owned(),
            revision: 4,
            writable,
        });
    }
    /// Fixture-only: stand in for the Host's catalog reply, so the menu's Host-driven list
    /// is capturable without a running Host. The names are deliberately not the design set's.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_permission_catalog(&mut self) {
        // The row needs the descriptor as well as the catalog, so the fixture stands in for
        // both sides of the same Host read.
        self.set_permission_settings(true);
        self.permission_catalog = Some(dsh_native_core::PermissionCatalog {
            options: vec![
                dsh_native_core::PresetOption {
                    value: "ask".to_owned(),
                    name: "Ask every time".to_owned(),
                    description: Some("Confirm each change".to_owned()),
                },
                dsh_native_core::PresetOption {
                    value: "workspace-write".to_owned(),
                    name: "Workspace write".to_owned(),
                    description: None,
                },
                dsh_native_core::PresetOption {
                    value: "full-access".to_owned(),
                    name: "Full access".to_owned(),
                    description: None,
                },
                dsh_native_core::PresetOption {
                    value: "auto".to_owned(),
                    name: "Auto".to_owned(),
                    description: None,
                },
            ],
            default_options: Vec::new(),
            default_preset: "ask".to_owned(),
        });
    }
    /// Fixture-only: tick the risk acknowledgement, so a capture shows the enabled form of
    /// the confirmation's commit button.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_permission_acknowledged(&mut self, acknowledged: bool) {
        self.permission_acknowledged = acknowledged;
    }
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_editor_menu(&mut self, open: bool) {
        self.editor_menu = open;
    }
    /// Fixture-only: hold a plan toggle in flight, so a capture can show the
    /// chip's disabled state and the strip's in-flight line.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_plan_pending(&mut self, pending: bool) {
        self.plan_toggle_pending = pending;
    }
    /// The fixture's glow override; a real run reads only the pointer.
    fn width_handle_fixture_glow(&self) -> bool {
        #[cfg(feature = "public-layout-fixture")]
        if self.width_handle_glow {
            return true;
        }
        false
    }
    /// Set the transcript content size directly, without touching a preference
    /// file. The public layout fixture uses this to render a size the control
    /// offers, so a scaled capture needs no profile.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn set_content_font_size(&mut self, size: f32) {
        self.content_font_size = design::content_size(size);
    }
    /// The task that puts focus in the composer. The public layout fixture
    /// dispatches it to capture the focused editor, which no fixture state
    /// otherwise reaches.
    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn focus_composer_task() -> Task<Message> {
        Self::focus_composer()
    }
    pub fn view(&self) -> Element<'_, Message> {
        // Both caches hold measurements taken at one text size, so they adopt the
        // current delta before anything is measured or drawn; each clears itself
        // when the delta actually changes.
        let delta = self.content_delta();
        self.chat_geometry.set_content_delta(delta);
        self.reply_content
            .set_content_delta(delta, self.content_delta_secondary());
        // Modal priority and worker fences stay independent of presentation.
        if !matches!(self.close, Close::None) {
            return self.close_view();
        }
        if self.settings_modal_visible() {
            let size = settings_modal_size(self.layout_size());
            let route = self.settings_route_facts();
            let card = self
                .settings
                .view_sized(self.settings_enabled(), size.width, size.height, &route)
                .map(Message::Settings);
            return settings_overlay(
                self.presentation_shell(true),
                card,
                Message::Settings(crate::settings::Action::Close),
            );
        }
        // The reference shows full access's risk confirmation as a modal over the
        // conversation (`PermissionSelect.tsx:198`), so native uses the same overlay the
        // settings and exporter dialogs use rather than a panel anchored to the chip.
        if self.permission_confirm.is_some() {
            return settings_overlay(
                self.presentation_shell(false),
                self.permission_confirm_view(),
                Message::PermissionConfirmCancel,
            );
        }
        if let Some(ticket) = self.exporter.ticket() {
            return settings_overlay(
                self.presentation_shell(false),
                self.export_view(),
                Message::Export(crate::exporter::Action::Close(ticket)),
            );
        }
        if let Some(ticket) = self.rename.ticket() {
            return settings_overlay(
                self.presentation_shell(false),
                self.rename_view(),
                Message::Rename(crate::rename::Action::Close(ticket)),
            );
        }
        if let Some(ticket) = self.fork.ticket() {
            return settings_overlay(
                self.presentation_shell(false),
                self.fork_view(),
                Message::Fork(crate::fork::Action::Close(ticket)),
            );
        }
        if let Some(ticket) = self.management.ticket() {
            return settings_overlay(
                self.presentation_shell(false),
                self.management_view(),
                Message::Management(crate::management::Action::Close(ticket)),
            );
        }
        if let Some(stamp) = self.sidebar_organization.stamp() {
            return settings_overlay(
                self.presentation_shell(false),
                self.sidebar_panel(),
                Message::Sidebar(sidebar_ui::Action::Close(stamp)),
            );
        }
        if let Some(dialog) = self.decisions.dialog(self.decision_replies_enabled()) {
            return dialog.map(Message::Interaction);
        }
        if let Some(ticket) = self.local_files.picker.ticket() {
            return settings_overlay(
                self.presentation_shell(false),
                self.picker_view(ticket),
                Message::FilePicker(attachment_picker::Action::Close(ticket)),
            );
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
                    .style(design::ghost).padding([8, 12])].spacing(12),
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
                            .style(design::ghost)
                            .padding([10, 14]),
                        button("Confirm export and save")
                            .on_press_maybe(
                                enabled.then_some(Message::Export(Action::Confirm(review.clone())))
                            )
                            .style(design::ghost)
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
                        .style(design::ghost)
                        .padding([10, 14]),
                );
        }
        body = body.push(label("An admitted filesystem save is synchronous and cannot be cancelled. Closing the application waits for owned filesystem work; an unhealthy filesystem may delay shutdown. Close never promises rollback or secure erasure.", MUTED));
        let size = settings_modal_size(self.layout_size());
        container(
            scrollable(body)
                .direction(design::scrollbar())
                .style(design::scrollable_style)
                .height(Length::Fill),
        )
        .width(size.width)
        .height(size.height)
        .style(|_| panel(SURFACE))
        .into()
    }
    fn rename_view(&self) -> Element<'_, Message> {
        use crate::rename::{Action, Notice};
        let ticket = self.rename.ticket().expect("visible rename editor");
        let current = self
            .sessions
            .get(&ticket.target)
            .map(|session| self.session_title(session))
            .unwrap_or_else(|| "Unavailable conversation".into());
        let enabled = self.rename_context().allowed && self.rename.pending().is_none();
        let mut body =
            column![
            row![
                label("Rename conversation", TEXT).size(22).width(Length::Fill),
                button("Close")
                    .style(design::ghost)
                    .on_press(Message::Rename(Action::Close(ticket.clone())))
            ]
            .spacing(10),
            label(format!("Shown as: {}", short(&current, 60)), MUTED).size(11),
            label(
                "Changes only the Host-owned title. It never sends a chat message or stops work.",
                MUTED
            )
            .size(11),
        ]
            .spacing(10);
        if let Some(review) = self.rename.review() {
            body = body
                .push(label("Confirm this exact title", TEXT).size(13))
                .push(label(self.rename.title(), ACCENT).size(14))
                .push(
                    row![
                        button("Back").style(design::ghost).on_press_maybe(
                            enabled.then_some(Message::Rename(Action::Back(ticket.clone())))
                        ),
                        button("Confirm rename")
                            .style(design::primary)
                            .on_press_maybe(
                                enabled.then_some(Message::Rename(Action::Confirm(review.clone())))
                            ),
                    ]
                    .spacing(8),
                );
        } else {
            let edit_ticket = ticket.clone();
            body = body
                .push(
                    text_input("Session title", self.rename.title())
                        .id("native-rename-title")
                        .on_input_maybe(enabled.then_some(move |title| {
                            Message::Rename(Action::Edit {
                                ticket: edit_ticket.clone(),
                                title,
                            })
                        }))
                        .style(design::field)
                        .padding(8)
                        .size(13),
                )
                .push(
                    label(
                        format!("Up to {} UTF-8 bytes; blank titles and control or invisible formatting characters are refused.", crate::rename::MAX_TITLE_BYTES),
                        MUTED
                    )
                    .size(10),
                )
                .push(
                    button("Review rename")
                        .style(design::primary)
                        .on_press_maybe(
                            enabled.then_some(Message::Rename(Action::Review(ticket.clone())))
                        ),
                );
        }
        body = body.push(match self.rename.notice() {
            Notice::None => label("Type a title, then review before the single write.", MUTED).size(11),
            Notice::Invalid(reason) => {
                label(format!("{} Nothing was sent; your typed title is kept.", reason.text()), DANGER).size(11)
            }
            Notice::Pending => label(
                "Rename admitted. Closing does not cancel or undo it; the title changes only from the Host acknowledgment.",
                ACCENT,
            )
            .size(11),
            Notice::Renamed { title } => {
                label(format!("Host accepted \"{}\".", short(title, 80)), ACCENT).size(11)
            }
            Notice::NotSent => label(
                "Worker queue rejected the rename: not sent. No automatic retry.",
                DANGER,
            )
            .size(11),
            Notice::Indeterminate => label(
                "Rename not confirmed: Host refusal, failure or uncertain write. The displayed title still comes only from the Host; reconcile explicitly and never retry automatically.",
                DANGER,
            )
            .size(11),
        });
        let size = settings_modal_size(self.layout_size());
        container(
            scrollable(body)
                .direction(design::scrollbar())
                .style(design::scrollable_style)
                .height(Length::Fill),
        )
        .width(size.width.min(560.0))
        .height(size.height.min(360.0))
        .style(|_| panel(SURFACE))
        .into()
    }
    fn fork_view(&self) -> Element<'_, Message> {
        use crate::fork::{Action, Notice};
        let ticket = self.fork.ticket().expect("visible fork review");
        let source = self
            .sessions
            .get(&ticket.target)
            .map(|session| self.session_title(session))
            .unwrap_or_else(|| "Unavailable conversation".into());
        // Actions stay enabled only while the panel's fence still matches the current
        // App state; a successful reveal advances the generation and retires them.
        let context = self.fork_context();
        let enabled = self.fork.pending().is_none()
            && context.allowed
            && context.view.as_ref().is_some_and(|view| {
                view.epoch == ticket.epoch
                    && view.generation == ticket.generation
                    && view.target == ticket.target
            });
        let cut = self.fork.cut();
        let mut body = column![
            row![
                label("Fork conversation", TEXT).size(22).width(Length::Fill),
                button("Close")
                    .style(design::ghost)
                    .on_press(Message::Fork(Action::Close(ticket.clone())))
            ]
            .spacing(10),
            label(format!("From: {}", short(&source, 60)), MUTED).size(11),
            label(
                if cut.is_some() {
                    "Creates a new child session ending at the selected reply, inclusive. It never sends a chat message, renames the source, stops work or deletes history."
                } else {
                    "Creates a new child session from the latest completed turn. It never sends a chat message, renames the source, stops work or deletes history."
                },
                MUTED
            )
            .size(11),
        ]
        .spacing(10);
        if let Some(cut) = cut {
            body = body.push(
                label(
                    format!("Branch point: inclusive through seq {cut}."),
                    ACCENT,
                )
                .size(11),
            );
        }
        if let Some(review) = self.fork.review() {
            body = body
                .push(label("Confirm this fork", TEXT).size(13))
                .push(label(format!("Source: {}", short(&source, 80)), ACCENT).size(14))
                .push(
                    row![
                        button("Back").style(design::ghost).on_press_maybe(
                            enabled.then_some(Message::Fork(Action::Back(ticket.clone())))
                        ),
                        button("Confirm fork")
                            .style(design::primary)
                            .on_press_maybe(
                                enabled.then_some(Message::Fork(Action::Confirm(review.clone())))
                            ),
                    ]
                    .spacing(8),
                );
        } else {
            body =
                body.push(button("Review fork").style(design::primary).on_press_maybe(
                    enabled.then_some(Message::Fork(Action::Review(ticket.clone()))),
                ));
        }
        body = body.push(match self.fork.notice() {
            Notice::None => label(
                "Review before the single write. Nothing is sent until Confirm.",
                MUTED,
            )
            .size(11),
            Notice::Pending => label(
                "Fork admitted. Closing does not cancel or undo it; the child id comes only from the Host acknowledgment.",
                ACCENT,
            )
            .size(11),
            Notice::Forked { child, selected } => label(
                if *selected {
                    format!(
                        "Host created child {}. It was already in this App's frames and was selected.",
                        short(child.as_str(), 80)
                    )
                } else {
                    format!(
                        "Host created child {}. It is not in this App's roster yet, so no sidebar row was invented and nothing was selected; it appears only after an authoritative roster or follow update.",
                        short(child.as_str(), 80)
                    )
                },
                ACCENT,
            )
            .size(11),
            Notice::Unavailable => label(
                "The Host refused the fork before creating a child: the source has no completed turn to fork from. Nothing was created and there is no automatic retry.",
                DANGER,
            )
            .size(11),
            Notice::Unattached => label(
                "The Host created a child but could not attach it to its workspace. The outcome is indeterminate and no child id was acknowledged; do not retry automatically and do not assume a rollback.",
                DANGER,
            )
            .size(11),
            Notice::NotSent => label(
                "Worker queue rejected the fork: not sent. No automatic retry.",
                DANGER,
            )
            .size(11),
            Notice::Indeterminate => label(
                "Fork not confirmed: refusal, failure, timeout or uncertain write. A child may exist; reconcile explicitly and never retry automatically.",
                DANGER,
            )
            .size(11),
        });
        let size = settings_modal_size(self.layout_size());
        container(
            scrollable(body)
                .direction(design::scrollbar())
                .style(design::scrollable_style)
                .height(Length::Fill),
        )
        .width(size.width.min(560.0))
        .height(size.height.min(360.0))
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
                .style(design::ghost)
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
                        .style(design::ghost)
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
                .style(design::ghost)
                .padding([10, 14]),
        );
        body = body.push(label("No forced stop, session deletion, rename, fork, provider/model call or automatic retry. A matching ACK does not replace newer stream state.", MUTED));
        let size = settings_modal_size(self.layout_size());
        container(
            scrollable(body)
                .direction(design::scrollbar())
                .style(design::scrollable_style)
                .height(Length::Fill),
        )
        .padding(22)
        .width(size.width)
        .height(size.height)
        .style(|_| panel(SURFACE))
        .into()
    }
    fn settings_modal_visible(&self) -> bool {
        matches!(self.close, Close::None) && self.settings.is_open()
    }
    /// Chat is the clean conversation; Trajectory is the complete recorded log,
    /// which is where the system prompt, harness context injects, session logs
    /// and unknown event kinds live.
    fn transcript_tabs(&self) -> Element<'_, Message> {
        let expanded = self.records_expanded;
        // The web's `.tab` (`ConversationRoot.module.css:203-231`): a `13px/16px`
        // label with no horizontal padding, a 9px under-pad, and a 2px
        // business-blue bar that ends flush with the header's seam. The strip
        // gaps its tabs 36px and starts 8px inside the header's own inset
        // (`:193-201`). Native keeps `design::ghost`'s hover surface, which the
        // web's plain transparent tab does not draw.
        let tab = move |text: &'static str, active: bool| -> Element<'_, Message> {
            let bar = container(Space::new().height(design::TAB_BAR).width(Length::Fill))
                .width(Length::Fill)
                .style(move |_| {
                    if active {
                        design::bar(ACCENT)
                    } else {
                        container::Style::default()
                    }
                });
            // The bar spans the tab's own text width, which only the stack's base
            // layer knows: a `Fill` width inside a `Stack` resolves against the
            // base's size, so the tab stays text-wide.
            iced::widget::Stack::new()
                .push(
                    column![
                        button(
                            label(text, if active { ACCENT } else { MUTED })
                                .size(13)
                                .line_height(iced::Pixels(design::TAB_LINE)),
                        )
                        .padding([0.0, 0.0])
                        .style(design::ghost)
                        .on_press_maybe(
                            (self.layout_controls_enabled() && !active)
                                .then_some(Message::ToggleRecords),
                        ),
                        Space::new().height(design::TAB_PAD_BOTTOM),
                    ]
                    .align_x(iced::Alignment::Center),
                )
                .push(
                    container(bar)
                        .height(Length::Fill)
                        .align_y(iced::Alignment::End),
                )
                .into()
        };
        row![tab("Chat", !expanded), tab("Trajectory", expanded)]
            .spacing(design::TAB_GAP)
            .padding([0.0, design::HEADER_TABS_INSET])
            .into()
    }
    fn transcript_tabs_visible(&self) -> bool {
        self.selected.is_some()
            && !self.conversation_menu
            && !self.capabilities
            && !self.work_open
            && !self.terminal.is_open()
            && !self.mcp.is_open()
            && self.detail.is_none()
    }
    /// True when the pane shows the empty-state hero instead of a conversation:
    /// the transcript is the active view and has no row to render, live partial
    /// included. The reference centers its hero strip and the composer card as
    /// one stack in that state (`ConversationContent.tsx:164-171`,
    /// `HeroShell.module.css:5-21`).
    fn hero_active(&self) -> bool {
        self.transcript_tabs_visible()
            && self.transcript.partial().is_none()
            && self
                .transcript
                .display_rows(self.records_expanded)
                .next()
                .is_none()
    }
    fn presentation_shell(&self, include_composer: bool) -> Element<'_, Message> {
        let main: Element<'_, Message> = if self.sidebar_sheet && !include_composer {
            self.navigation_sheet()
        } else {
            // The hero and the composer are one centered stack in the
            // reference, so the empty state owns its own pane instead of a
            // centered body above a docked composer.
            let hero_pane = include_composer && self.hero_active();
            let (body, composer_inline) = if self.conversation_menu {
                (self.conversation_menu_view(), false)
            } else if self.capabilities {
                (self.capabilities_view(), false)
            } else if self.work_open {
                (self.work_panel_view(), false)
            } else if self.terminal.is_open() {
                (self.terminal_view(), false)
            } else if self.mcp.is_open() {
                (self.mcp_view(), false)
            } else if let Some(detail) = &self.detail {
                (self.detail_view(detail), false)
            } else if hero_pane {
                (self.hero_pane(), true)
            } else {
                (self.transcript_view(), false)
            };
            // The tabs live inside the header band, as the reference's one
            // header region holds both, so nothing is pushed between them.
            let mut content = column![self.header()].height(Length::Fill);
            content = content.push(body);
            if include_composer && !composer_inline {
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
    fn stop_color(&self) -> Color {
        if self.cancel_pending || self.stop_allowed() {
            DANGER
        } else {
            MUTED
        }
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
                    self.stop_color(),
                )
                .size(12),
            )
            .on_press_maybe(self.stop_allowed().then_some(Message::Stop))
            .style(design::danger)
            .padding([8, 12])
            // The web draws this control in the same 34px box as Send
            // (`InputBar.module.css:369-395`).
            .height(design::COMPOSER_CONTROL_HEIGHT),
        )
        .id("native-stop-control")
        .into()
    }
    fn settings_entry(&self, compact: bool) -> Element<'_, Message> {
        // The mutually exclusive sidebar/rail branches share exactly one entry.
        // Expanded it is a web `.panelRow` — `min-height: 36px`, `padding: 7px 8px`,
        // `radius-md` (`SidebarRoot.module.css:493-505`); the rail keeps the
        // compact control it already had.
        let entry = if compact {
            // "Settings" cannot fit the reference's 36px rail box (single words
            // clip rather than wrap), so the rail shows the gear the web's own
            // icon button draws; the accessibility node keeps the name.
            button(label("⚙", MUTED).size(15))
                .on_press(Message::Settings(crate::settings::Action::Open))
                .style(design::ghost)
                .width(Length::Fill)
                .height(design::RAIL_CONTROL)
        } else {
            button(label("Settings", MUTED).size(14))
                .on_press(Message::Settings(crate::settings::Action::Open))
                .style(|theme, status| design::navigation_row(theme, status, false))
                .padding(design::PANEL_ROW_PAD)
                .height(design::PANEL_ROW_HEIGHT)
                .width(Length::Fill)
        };
        container(entry)
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
            label("CONTENT", MUTED).size(11),
            row![
                label("Text size", TEXT),
                Space::new().width(Length::Fill),
                button(label("−", TEXT))
                    .on_press_maybe(
                        (self.content_font_size > design::CONTENT_FONT_MIN)
                            .then_some(Message::ContentSize(self.content_font_size - 1.0))
                    )
                    .style(design::ghost)
                    .padding([6, 10]),
                label(format!("{:.0}", self.content_font_size), MUTED),
                button(label("+", TEXT))
                    .on_press_maybe(
                        (self.content_font_size < design::CONTENT_FONT_MAX)
                            .then_some(Message::ContentSize(self.content_font_size + 1.0))
                    )
                    .style(design::ghost)
                    .padding([6, 10]),
            ]
            .align_y(iced::Alignment::Center)
            .padding([6, 14]),
            label("CONVERSATION", MUTED).size(11),
            button(label("Manage & export…", TEXT))
                .on_press_maybe(manage)
                .style(design::ghost)
                .padding([12, 14])
                .width(Length::Fill),
            label("APPLICATION", MUTED).size(11),
            button(label("Information & diagnostics", TEXT))
                .on_press(Message::Capabilities)
                .style(design::ghost)
                .padding([12, 14])
                .width(Length::Fill),
            button(label(
                format!(
                    "Work records · read-only ({})",
                    self.transcript.work().total()
                ),
                TEXT
            ))
            .on_press(Message::Work(work_panel_ui::Action::Open))
            .style(design::ghost)
            .padding([12, 14])
            .width(Length::Fill),
            button(label("Terminal · Host shell", TEXT))
                .on_press_maybe(
                    self.terminal_context()
                        .allowed
                        .then(|| self.terminal_context().view)
                        .flatten()
                        .map(|view| Message::Terminal(crate::terminal::Action::Open { view })),
                )
                .style(design::ghost)
                .padding([12, 14])
                .width(Length::Fill),
            button(label(
                format!("MCP servers · app-owned ({})", self.mcp.entries().len()),
                TEXT,
            ))
            .on_press_maybe(
                self.layout_controls_enabled()
                    .then_some(Message::Mcp(crate::mcp::Action::Open)),
            )
            .style(design::ghost)
            .padding([12, 14])
            .width(Length::Fill),
            button(label("Close native Harness…", MUTED))
                .on_press(Message::Close)
                .style(design::ghost)
                .padding([12, 14])
                .width(Length::Fill)
        ]
        .spacing(14);
        container(
            scrollable(container(content).max_width(440).width(Length::Fill))
                .direction(design::scrollbar())
                .style(design::scrollable_style)
                .height(Length::Fill),
        )
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
    /// The direct children the Host's `subagentCatalog` projection publishes for the selected
    /// session. Empty for a child session or an unready catalog, which hides the header action.
    fn subagent_entries(&self) -> Vec<subagent_catalog::Entry> {
        let root = self
            .selected
            .as_ref()
            .and_then(|id| self.sessions.get(id))
            .is_some_and(|session| session.parent_session_id.is_none());
        if !root {
            return Vec::new();
        }
        subagent_catalog::entries(self.transcript.projections())
    }
    /// Why the selected catalog-addressed child cannot accept input, or `None` when the normal
    /// composer applies. Only the one-shot case is decided here: it is the child's own durable
    /// descriptor, while the parent-offline and unknown cases need state native does not hold.
    fn subagent_read_only_copy(&self) -> Option<(&'static str, &'static str)> {
        let child = self
            .selected
            .as_ref()
            .and_then(|id| self.sessions.get(id))
            .is_some_and(|session| {
                matches!(
                    session.origin,
                    Some(dsh_native_transport::dto::SubagentOrigin::Subagent)
                )
            });
        if !child {
            return None;
        }
        let mode = subagent_catalog::descriptor_mode(
            self.transcript
                .records()
                .map(|event| (event.event_type.as_str(), &event.data)),
        );
        matches!(mode, Some(subagent_catalog::Mode::OneShot)).then_some((
            "One-shot subagent record",
            "One-shot tasks do not accept follow-ups; review the full execution record here.",
        ))
    }
    /// The parent's catalog when a child session is selected: the reference's sibling list
    /// (`SubagentHeaderLineage.tsx:767-800`). `None` on a root, or when the parent summary
    /// carries no catalog, so the header keeps a plain current crumb.
    /// One session's own catalog, for a nested branch. Absent when the summary carries no
    /// projection or no rows.
    fn catalog_for(&self, id: &str) -> Vec<subagent_catalog::Entry> {
        let Some(summary) = SessionId::new(id)
            .ok()
            .and_then(|id| self.sessions.get(&id))
        else {
            return Vec::new();
        };
        let Some(projections) = summary.projections.as_ref() else {
            return Vec::new();
        };
        subagent_catalog::entries(&projections.values)
    }
    fn parent_catalog(&self) -> Option<Vec<subagent_catalog::Entry>> {
        let selected = self.selected.as_ref()?;
        let summary = self.sessions.get(selected)?;
        if summary.origin.is_none() {
            return None;
        }
        let parent = summary.parent_session_id.as_ref()?;
        let projections = self.sessions.get(parent)?.projections.as_ref()?;
        let entries = subagent_catalog::entries(&projections.values);
        (!entries.is_empty()).then_some(entries)
    }
    fn subagent_title(&self, id: &str) -> Option<String> {
        SessionId::new(id)
            .ok()
            .and_then(|id| self.sessions.get(&id))
            .map(|summary| self.session_title(summary))
            .filter(|title| !title.is_empty())
    }
    /// The reference's per-row metrics from a child summary's `tokenUsage`/`subagentTiming`
    /// projections (`SubagentHeaderLineage.tsx:250-270`), if the list carries them.
    fn subagent_metrics(&self, id: &str) -> subagent_catalog::Metrics {
        let Some(projections) = SessionId::new(id)
            .ok()
            .and_then(|id| self.sessions.get(&id))
            .and_then(|summary| summary.projections.as_ref())
        else {
            return subagent_catalog::Metrics::default();
        };
        subagent_catalog::metrics(&projections.values, self.subagent_running(id), now_millis())
    }
    fn subagent_running(&self, id: &str) -> bool {
        SessionId::new(id)
            .ok()
            .and_then(|id| self.sessions.get(&id))
            .is_some_and(|session| session.running)
    }
    /// The reference's activity dot: foam while running, the hairline colour otherwise.
    fn subagent_dot(running: bool) -> Element<'static, Message> {
        container(Space::new().width(6).height(6))
            .style(move |_| container::Style {
                background: Some(if running { design::FOAM } else { design::LINE }.into()),
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }
    fn subagent_menu_view(&self, entries: &[subagent_catalog::Entry]) -> Element<'_, Message> {
        let rows = subagent_catalog::tree_rows(entries, &self.subagent_expanded, |id| {
            self.catalog_for(id)
        });
        let mut list = column![label("Subagent sessions", MUTED).size(11)].spacing(4);
        for row in rows {
            let entry = &row.entry;
            let running = self.subagent_running(&entry.id);
            // A branch toggle keeps its column even when absent, so sibling labels align.
            let toggle: Element<'_, Message> = if row.has_children {
                button(label(if row.expanded { "▾" } else { "▸" }, MUTED).size(11))
                    .on_press(Message::ToggleSubagentBranch(entry.id.clone()))
                    .style(design::ghost)
                    .padding([0, 2])
                    .into()
            } else {
                container(Space::new().width(15)).into()
            };
            let label_ink = if self
                .selected
                .as_ref()
                .is_some_and(|id| id.as_str() == entry.id)
            {
                TEXT
            } else {
                MUTED
            };
            // The reference's secondary line: the child's title (when it differs from the
            // catalog label), its mode and its activity.
            let activity = if running { "running" } else { "inactive" };
            let mut secondary = Vec::new();
            if let Some(title) = self.subagent_title(&entry.id)
                && title != entry.label
            {
                secondary.push(title);
            }
            secondary.push(subagent_catalog::mode_label(entry.mode).to_owned());
            secondary.push(activity.to_owned());
            let content = column![
                label(entry.label.clone(), label_ink).size(12),
                label(secondary.join(" · "), MUTED).size(11),
            ]
            .spacing(1)
            .width(Length::Fill);
            let metrics = self.subagent_metrics(&entry.id);
            let mut right = column![].spacing(1).align_x(iced::Alignment::End);
            if let Some(tokens) = metrics.tokens {
                right = right.push(
                    label(
                        format!("{} tok", subagent_catalog::format_tokens(tokens)),
                        MUTED,
                    )
                    .size(11),
                );
            }
            if let Some(duration) = metrics.duration_ms {
                right =
                    right.push(label(subagent_catalog::format_duration(duration), MUTED).size(11));
            }
            let body = button(
                row![Self::subagent_dot(running), content, right,]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
            )
            .on_press(Message::SubagentOpen(entry.id.clone()))
            .style(design::ghost)
            .padding([6, 7])
            .width(Length::Fill);
            list = list.push(
                row![Space::new().width(row.level as f32 * 12.0), toggle, body,]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
            );
        }
        container(list)
            .id("native-subagent-menu")
            .width(336)
            .padding(6)
            .style(|_| design::card(SURFACE, 10.0))
            .into()
    }
    /// The header count action, visible only on a root session whose catalog has rows. Its
    /// dropdown lists direct children; nested catalogs belong to the child session that owns
    /// them, which native has not opened yet.
    fn subagent_action(&self) -> Option<Element<'_, Message>> {
        let entries = self.subagent_entries();
        if entries.is_empty() {
            return None;
        }
        let running = entries
            .iter()
            .filter(|entry| self.subagent_running(&entry.id))
            .count();
        let mut content = row![].spacing(5).align_y(iced::Alignment::Center);
        if running > 0 {
            content = content.push(Self::subagent_dot(true));
        }
        content = content
            .push(label(subagent_catalog::count_label(entries.len(), running), MUTED).size(12));
        let trigger = button(content)
            .on_press(Message::ToggleSubagentMenu(!self.subagent_menu))
            .style(design::ghost)
            .padding([5, 8])
            .into();
        Some(anchored_popup::anchored_below(
            trigger,
            self.subagent_menu
                .then(|| self.subagent_menu_view(&entries)),
            Message::ToggleSubagentMenu(false),
        ))
    }
    fn header(&self) -> Element<'_, Message> {
        let heading = self
            .selected
            .as_ref()
            .and_then(|id| self.sessions.get(id))
            .map(|session| self.session_title(session))
            .unwrap_or_else(|| "DeepSeek Harness".into());
        let observed = self.selected.as_ref().and_then(|id| self.sessions.get(id));
        let title_limit = if self.layout_size().width < 700.0 {
            18
        } else if self.compact() {
            30
        } else {
            64
        };
        let mut identity = row![
            container(self.header_crumbs(&heading, title_limit))
                .width(Length::Fill)
                .clip(true)
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);
        if let Some(session) = observed.filter(|session| session.running) {
            identity = identity.push(self.working_badge(session));
        }
        let mut actions = row![identity]
            .spacing(6)
            .height(design::HEADER_TITLE_ROW)
            .align_y(iced::Alignment::Center);
        if let Some(subagents) = self.subagent_action() {
            actions = actions.push(subagents);
        }
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
            .style(design::ghost)
            // The web's title row is 30px tall (`ConversationRoot.module.css:72-79`),
            // so this control fits that row rather than setting its own.
            .padding([7, 10]),
        );
        let mut header = column![actions].spacing(6);
        if !routine_status(&self.status) {
            header = header.push(
                container(label(&self.status, MUTED).size(11)).id(accessibility::WIDGET_STATUS),
            );
        }
        if let Some(id) = self
            .selected
            .as_ref()
            .filter(|id| self.sidebar_organization.trashed(id))
        {
            header = header.push(
                row![
                    label(
                        "In Trash · history retained; background work may continue",
                        DANGER
                    )
                    .size(11)
                    .width(Length::Fill),
                    button(label("Restore…", TEXT).size(11))
                        .style(design::ghost)
                        .on_press_maybe(
                            self.sidebar_target_allowed(id)
                                .then(|| self.sidebar_open_message(id))
                        )
                ]
                .spacing(6),
            );
        }
        if self.decisions.len() > 0 {
            header = header.push(
                scrollable(
                    self.decisions
                        .cards(self.decision_replies_enabled())
                        .map(Message::Interaction),
                )
                .direction(design::scrollbar())
                .style(design::scrollable_style)
                .height(94),
            );
        }
        let mut band = column![header];
        if self.transcript_tabs_visible() {
            // The web's tab strip carries its own 10px top margin
            // (`ConversationRoot.module.css:199`).
            band = band
                .push(Space::new().height(design::HEADER_TABS_GAP))
                .push(self.transcript_tabs());
        }
        container(column![
            container(band)
                .id("native-header")
                // `ConversationRoot.module.css:15-23`: one header region holding
                // the title and the tabs, `padding: 10px 28px 0 20px`. Without
                // tabs the same sheet restores a 10px bottom padding and drops
                // the min-height (`:27-30`), which is what keeps a blank session's
                // title off the seam.
                .padding(if self.transcript_tabs_visible() {
                    iced::Padding {
                        top: 10.0,
                        right: 28.0,
                        bottom: 0.0,
                        left: 20.0,
                    }
                } else {
                    iced::Padding {
                        top: 10.0,
                        right: 28.0,
                        bottom: 10.0,
                        left: 20.0,
                    }
                })
                .width(Length::Fill),
            // Conversation header seam: the web's 0.5px `border-l3` rule
            // drawn at one native device pixel (docs/NATIVE-UI-PARITY.md).
            container(Space::new().height(1).width(Length::Fill)).style(|_| panel(design::LINE))
        ])
        .width(Length::Fill)
        .into()
    }
    /// The rail's items: every staged or in-flight attachment, with the ticket
    /// its remove control must carry. The reference's rail shows the same set —
    /// uploading, ready and failed cards alike.
    fn composer_rail_items(&self) -> Vec<(FileCard, Message)> {
        let mut items: Vec<(FileCard, Message)> = Vec::new();
        for (ticket, metadata) in self.files.attachments() {
            items.push((
                FileCard {
                    name: metadata.name.clone(),
                    meta: crate::reducer::attachment_meta(&metadata.name, metadata.bytes),
                },
                Message::FileRemove(ticket.clone()),
            ));
        }
        for (ticket, metadata, reading) in self.files.active() {
            let (name, meta) = match (reading, metadata) {
                (true, _) => ("Uploading…".to_owned(), "waiting for the Host".to_owned()),
                (false, Some(metadata)) => (
                    metadata.name.clone(),
                    crate::reducer::attachment_meta(&metadata.name, metadata.bytes),
                ),
                (false, None) => ("Not staged".to_owned(), "no Host metadata".to_owned()),
            };
            items.push((FileCard { name, meta }, Message::FileRemove(ticket.clone())));
        }
        items
    }
    fn file_controls(&self) -> Element<'_, Message> {
        let context = self.file_context();
        // The fixture records this container's box, so a run whose rows have all
        // moved to the rail must not emit a zero-height one: an empty body is no
        // control at all.
        let finishing = self.files.attachments().is_empty()
            && self.files.active().is_empty()
            && !self.files.editor_open()
            && (self.files.pending() || self.files.prompt_pending() || self.local_files.busy());
        if self.files.editor().is_none()
            && !self.preview_current()
            && self.files.notice().is_none()
            && !finishing
        {
            return Space::new().height(0).into();
        }
        let mut body = Column::new().spacing(5);
        if let Some((stamp, path)) = self.files.editor() {
            body = body.push(
                row![
                    text_input("Absolute file path · up to 4 MiB", path)
                        .on_input(move |path| Message::FilePath(stamp, path))
                        .style(design::field)
                        .padding(7)
                        .size(12)
                        .width(Length::Fill),
                    button(label("Browse…", MUTED).size(12))
                        .on_press_maybe(
                            (context.allowed && !self.local_files.busy()).then_some(
                                Message::FilePicker(attachment_picker::Action::Open(stamp))
                            )
                        )
                        .style(design::ghost)
                        .padding([6, 9]),
                    button(label("Upload", TEXT).size(12))
                        .on_press_maybe(
                            (context.allowed && !path.is_empty())
                                .then_some(Message::FileUpload(stamp))
                        )
                        .style(design::ghost)
                        .padding([6, 9]),
                    button(label("Cancel", MUTED).size(12))
                        .on_press_maybe(
                            self.file_local_context()
                                .allowed
                                .then_some(Message::FileCancel(stamp))
                        )
                        .style(design::ghost)
                        .padding([6, 9]),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            );
            body = body.push(label(attachment_draft::UPLOAD_CONSENT, MUTED).size(10));
            body = body.push(
                button(label("Preview image", MUTED).size(10))
                    .style(design::ghost)
                    .on_press_maybe(
                        (context.allowed && !path.is_empty() && !self.local_files.busy())
                            .then_some(Message::FilePreview(stamp)),
                    ),
            );
        }
        // Staged and in-flight attachments moved to the card's rail above the
        // draft, exactly where the reference draws them.
        if finishing {
            body = body.push(label("Previous file operation is finishing…", MUTED).size(11));
        }
        body = body.push(self.local_file_preview());
        if let Some(notice) = self.files.notice() {
            body = body.push(label(notice, DANGER).size(11));
        }
        container(body)
            .id("native-file-draft")
            .width(Length::Fill)
            .into()
    }
    /// One compact Rosé Pine row per attachment, carrying exactly that slot's ticket.
    fn attachment_row(
        &self,
        ticket: &worker::attachments::Ticket,
        title: String,
        action: &'static str,
    ) -> Element<'_, Message> {
        row![
            label(title, MUTED).size(12).width(Length::Fill),
            button(label(action, MUTED).size(12))
                .on_press_maybe(
                    self.file_local_context()
                        .allowed
                        .then_some(Message::FileRemove(ticket.clone()))
                )
                .style(design::ghost)
                .padding([6, 9])
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center)
        .into()
    }
    fn prompt_resolved(
        &mut self,
        pending: Pending,
        generation: u64,
        receipt: prompt_recovery::Receipt,
    ) {
        self.draft_state.touch(&pending.session); // Includes definitive NotSent/queue refusal.
        if receipt == prompt_recovery::Receipt::Accepted {
            if generation == self.generation
                && self.draft_serial == pending.draft_serial
                && self.editor.text() == pending.text
            {
                self.editor = text_editor::Content::new();
                self.touch_draft();
            }
            if self.draft_versions.get(&pending.session) == Some(&pending.draft_serial)
                && self.drafts.get(&pending.session) == Some(&pending.text)
            {
                self.drafts.remove(&pending.session);
                self.draft_versions.remove(&pending.session);
                self.draft_recovered.remove(&pending.session);
                self.draft_state.touch(&pending.session);
            }
            self.status="Submission admission verified; recorded is not provider completion. Transcript/running state come from Host events.".into();
        } else {
            self.status="Original submission was definitely not dispatched; original draft retained. No automatic resend.".into();
        }
    }
    /// The composer's completion menu, if the draft is a command token and the
    /// Host has advertised commands. The draft is the only trigger: the menu
    /// disappears the moment the line stops being a command position.
    fn completion(&self) -> Option<command_menu::Completion<'_>> {
        let draft = self.editor.text();
        let query = command_menu::query(&draft)?;
        let candidates = self.commands.as_deref().unwrap_or_default();
        let matches = command_menu::matches(candidates, query);
        if matches.is_empty() {
            return None;
        }
        Some(command_menu::Completion {
            matches,
            selected: self
                .command_selected
                .min(matches_len(&draft, candidates) - 1),
            query: query.to_owned(),
        })
    }
    /// Announce a turn that finished while the window was not focused.
    ///
    /// The decision is split from delivery so the falling edge can be tested
    /// without spawning anything: `run_watch` owns the transition, `notifications`
    /// owns the wording, and this only connects them to the Host's own run flag.
    fn announce_settle(&mut self) {
        if self.options.smoke.is_some() {
            return;
        }
        let selected = self.selected.clone();
        let running = selected
            .as_ref()
            .and_then(|id| self.sessions.get(id))
            .is_some_and(|session| session.running && self.running_status_live());
        if self
            .run_watch
            .observe(selected.as_ref().map(|id| id.as_str()), running)
            .is_none()
        {
            return;
        }
        // The title the sidebar already shows for this session, so the notice
        // names the conversation the way the user sees it.
        let title = self
            .selected
            .as_ref()
            .and_then(|id| self.sessions.get(id))
            .map(|session| self.session_title(session))
            .unwrap_or_default();
        // A broken update stream is the other settle worth interrupting for: the
        // conversation may have stopped receiving records off-screen.
        let (kind, detail) = if self.transcript.interrupted() {
            (
                notifications::Kind::Interrupted,
                "The update stream stopped. Open the conversation to check it.",
            )
        } else {
            (
                notifications::Kind::Answered,
                "The turn finished. Open the conversation to read it.",
            )
        };
        let notice = notifications::notice(kind, self.window_active, &title, detail);
        if let Some(notice) = notice {
            notifications::deliver(&notice);
        }
    }
    /// Ask the Host for this session's commands, once per composer session. The
    /// trigger is a user action -- a leading slash -- so no frame dispatch and no
    /// selection does this on its own.
    fn request_commands(&mut self) {
        if self.commands.is_some()
            || self.commands_pending
            || self.options.smoke.is_some()
            || !self.ready
            || self.stopped
            || self.observations_lost
            || !matches!(self.close, Close::None)
        {
            return;
        }
        let Some(session_id) = self.selected.clone() else {
            return;
        };
        self.commands_pending = true;
        self.command(Command::CommandsList(session_id));
    }
    /// Put keyboard focus back in the composer.
    ///
    /// Iced's text editor clears its own focus when any left press lands outside
    /// its bounds (see `iced_widget::text_editor::Update::from_event`), so every
    /// press that means "I am still writing" -- a completion row, the model
    /// menu, the card's own padding -- would otherwise leave a visible caret in
    /// a dead field and swallow the next keystroke. This is the one place that
    /// asks for focus back; nothing here takes focus away from another field.
    fn focus_composer() -> Task<Message> {
        iced::widget::operation::focus(accessibility::WIDGET_COMPOSER)
    }
    /// Run `task`, then put focus back in the composer when `keep` says the
    /// composer is on screen. Refocusing an invisible composer would target a
    /// widget the tree does not publish, so the caller passes the same guard the
    /// view uses; it is a value, not `&self`, because the task often comes from a
    /// mutable handler.
    fn composer_focus_kept(keep: bool, task: Task<Message>) -> Task<Message> {
        if keep {
            Task::batch([task, Self::focus_composer()])
        } else {
            task
        }
    }
    /// Write one accepted command into the draft and keep the caret there.
    ///
    /// Acceptance is an edit, never a submission: the command line stays
    /// editable and the Host only ever sees it when the user sends it.
    fn accept_command(&mut self, name: &str) -> Task<Message> {
        let accepted = command_menu::accepted(&self.editor.text(), name);
        self.editor = text_editor::Content::with_text(&accepted);
        self.command_selected = 0;
        self.touch_draft();
        Self::focus_composer()
    }
    /// The reference's permission chip, in the form it takes while its control is locked:
    /// a 28px chip with a glyph and the current preset's name, drawn in the dimmed colour
    /// with no menu, which is exactly the state `PermissionSelect` renders for
    /// `disabled={locked || leaving}` (`PermissionSelect.module.css:1-40`). The preset
    /// comes from the Host's own `permission/preset` record; the menu that switches it will
    /// submit `/permission <preset>`, the command line the package's header names
    /// (`ui-permission-presets/src/client/index.ts:3-11`).
    fn permission_chip(&self) -> Option<Element<'_, Message>> {
        let preset = self.transcript.permission_preset()?.to_owned();
        let ink = if self.permission_menu { TEXT } else { MUTED };
        let chip = button(
            row![
                label("◈", ink)
                    .size(12)
                    .width(design::PERMISSION_CHIP_GLYPH),
                label(permission_label(&preset).to_owned(), ink).size(13),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::TogglePermissionMenu)
        .style(design::ghost)
        .padding(design::COMPOSER_CHIP_PAD)
        .height(design::COMPOSER_CHIP_HEIGHT);
        // The reference shows the risk confirmation as a modal over the conversation
        // (`PermissionSelect.tsx:198`); native swaps the chip's own panel for it, keeping
        // the guard and its copy identical while staying inside the popup pattern this app
        // already uses for menus.
        Some(
            container(anchored_popup::anchored(
                chip.into(),
                self.permission_menu
                    .then(|| self.permission_menu_view(&preset)),
                Message::TogglePermissionMenu,
            ))
            .id("native-permission-control")
            .into(),
        )
    }
    /// The reference's full-access risk confirmation, copy and all: title, description, an
    /// explicit acknowledgement, Cancel and Enable
    /// (`ui-permission-presets/src/client/locales.ts:34-38`). Enable stays disabled until
    /// the acknowledgement is ticked, which is what makes the guard meaningful.
    fn permission_confirm_view(&self) -> Element<'_, Message> {
        // An inert button that still looks enabled is a false signal on a safety control,
        // so the tick is what gives it the primary style as well as its action.
        let acknowledged = self.permission_acknowledged;
        let enable = button(
            label(
                "Enable Full access",
                if acknowledged { TEXT } else { MUTED },
            )
            .size(13),
        )
        .style(move |theme, status| {
            if acknowledged {
                design::primary(theme, status)
            } else {
                design::ghost(theme, status)
            }
        })
        .padding([4, 10])
        .on_press_maybe(
            self.permission_acknowledged
                .then_some(Message::PermissionConfirmEnable),
        );
        container(
            Column::new()
                .spacing(8)
                .push(label("Enable Full access?", TEXT).size(14))
                .push(label(FULL_ACCESS_RISK, MUTED).size(12))
                .push(
                    row![
                        iced::widget::checkbox(self.permission_acknowledged)
                            .on_toggle(Message::PermissionConfirmAcknowledge)
                            .size(14),
                        // Iced's checkbox carries no label, so the acknowledgement text
                        // sits beside it and answers a press itself: a checkbox's label
                        // toggles its box, and the reference's acknowledgement behaves that
                        // way (`locales.ts:36`).
                        iced::widget::mouse_area(
                            label("I understand the risks and want to continue", MUTED).size(12),
                        )
                        .on_press(Message::PermissionConfirmAcknowledge(
                            !self.permission_acknowledged,
                        )),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                )
                .push(
                    row![
                        Space::new().width(Length::Fill),
                        button(label("Cancel", MUTED).size(13))
                            .style(design::ghost)
                            .padding([4, 10])
                            .on_press(Message::PermissionConfirmCancel),
                        enable,
                    ]
                    .spacing(6),
                ),
        )
        .id("native-permission-confirm")
        .width(360)
        .padding(10)
        .style(|_| design::card(SURFACE, 10.0))
        .into()
    }
    /// Sends one preset as the command line the reference's picker submits.
    fn send_permission_preset(&mut self, preset: &str) {
        if let Some(session_id) = self.selected.clone() {
            self.command(Command::CommandsExecute {
                session_id,
                line: format!("/permission {preset}"),
            });
        }
    }
    /// The jobs panel: the live jobs under the reference's own "Running" heading and the rest
    /// under "Finished {count}" (`ui-jobs/src/client/locales.ts:56-58`), each row carrying the
    /// producer's label and its kind and status words. The reference's state dot and its Clear
    /// action are both deliberately absent — the dot's form and Clear's semantics are unread, and
    /// omitting them is the alternative to inventing them.
    fn jobs_panel_view(&self) -> Element<'_, Message> {
        let live: Vec<&dsh_native_transport::dto::JobView> = self
            .jobs
            .iter()
            .filter(|job| job.status == "running" || job.status == "stopping")
            .collect();
        let settled = self.jobs.len() - live.len();
        let mut panel = Column::new().spacing(4);
        if !live.is_empty() {
            panel = panel.push(label("Running", MUTED).size(11));
            for job in live {
                panel = panel.push(job_row(
                    job,
                    self.jobs_expanded.as_deref(),
                    self.jobs_output
                        .get(&job.id)
                        .map(String::as_str)
                        .unwrap_or_default(),
                ));
            }
        }
        if settled > 0 {
            panel = panel.push(label(format!("Finished {settled}"), MUTED).size(11));
            for job in self
                .jobs
                .iter()
                .filter(|job| job.status != "running" && job.status != "stopping")
            {
                panel = panel.push(job_row(
                    job,
                    self.jobs_expanded.as_deref(),
                    self.jobs_output
                        .get(&job.id)
                        .map(String::as_str)
                        .unwrap_or_default(),
                ));
            }
        }
        if let Some(armed) = &self.jobs_armed {
            let job_label = self
                .jobs
                .iter()
                .find(|job| &job.id == armed)
                .map(|job| job.label.clone())
                .unwrap_or_default();
            panel = panel
                .push(label(format!("Stop task {job_label}"), MUTED).size(11))
                .push(label("Click again to confirm", design::GOLD).size(11));
        }
        if let Some(error) = &self.jobs_kill_error {
            panel = panel.push(label(format!("Stop failed: {error}"), DANGER).size(11));
        }
        container(panel)
            .id("native-jobs-panel")
            .width(260)
            .padding(6)
            .style(|_| design::card(SURFACE, 10.0))
            .into()
    }
    /// The presets the menu offers: the reference's design set, with the current one in
    /// full ink. The Host's own catalog would extend this list once the catalog read lands.
    fn permission_menu_view(&self, current: &str) -> Element<'_, Message> {
        let mut items = Column::new().spacing(2);
        // The Host's catalog when it has answered, the design set until then, so the menu
        // never shows an invented preset once the deployment has spoken.
        let offered: Vec<(String, String)> = match &self.permission_catalog {
            Some(catalog) => catalog
                .options
                .iter()
                .map(|option| (option.value.clone(), option.name.clone()))
                .collect(),
            None => [
                ("read-only", "Read only"),
                ("workspace-write", "Workspace write"),
                ("full-access", "Full access"),
            ]
            .into_iter()
            .map(|(value, name)| (value.to_owned(), name.to_owned()))
            .collect(),
        };
        for (value, name) in offered {
            let ink = if value == current { TEXT } else { MUTED };
            // `locales.ts:54`: the deployment's registered Auto preset carries an `EXP`
            // badge in the picker, so it is never mistaken for a user-defined preset.
            let mut content = row![label(name, ink).size(13)].spacing(6);
            if value == "auto" {
                content = content.push(label(AUTO_BADGE, MUTED).size(10));
            }
            items = items.push(
                button(content)
                    .on_press(Message::PermissionPick(value.clone()))
                    .style(design::ghost)
                    .padding([4, 8])
                    .width(Length::Fill),
            );
        }
        container(items)
            .id("native-permission-menu")
            .width(200)
            .padding(6)
            .style(|_| design::card(SURFACE, 10.0))
            .into()
    }
    /// The plan-mode control for the selected session, from the Host's advertised
    /// commands and the Host's own `plan` projection.
    /// The plan-mode chip the reference seats in the control row's leading group.
    /// It exists only while the Host's `plan` projection says the mode is on — the
    /// reference's chip likewise only offers to turn it off — and it is inert
    /// while a toggle is in flight, drawing that state dimmed.
    fn plan_chip(&self) -> Option<Element<'_, Message>> {
        let control = self.plan_control();
        control.mode?;
        let actionable = control.visible() && !control.pending && self.composer_visible();
        // The reference disables the chip while a toggle is in flight
        // (`disabled={locked || leaving}` under `opacity: 0.6`).
        let (ink, fill) = if control.pending {
            (MUTED, design::PLAN_CHIP_FILL_DIM)
        } else {
            (ACCENT, design::PLAN_CHIP_FILL)
        };
        let chip = container(
            button(
                row![
                    // DejaVu carries the ballot-with-check glyph; the reference
                    // draws its own plan outline in this 14px slot.
                    label("☑", ink).size(12).width(design::PLAN_CHIP_GLYPH),
                    label("Plan", ink).size(13),
                ]
                .spacing(design::PLAN_CHIP_GAP)
                .align_y(iced::Alignment::Center),
            )
            .on_press_maybe(actionable.then_some(Message::TogglePlanMode))
            .style(move |theme, status| {
                let mut style = design::ghost(theme, status);
                style.background = Some(
                    if matches!(status, button::Status::Hovered) {
                        design::PLAN_CHIP_HOVER
                    } else {
                        fill
                    }
                    .into(),
                );
                style.border.radius = design::RADIUS_SM.into();
                style
            })
            .padding(design::PLAN_CHIP_PAD)
            .height(design::COMPOSER_CHIP_HEIGHT),
        )
        .id("native-plan-control")
        .into();
        Some(chip)
    }
    fn plan_control(&self) -> plan_control::Control {
        plan_control::Control {
            offer: plan_control::Offer::from_published(self.plan_published),
            mode: plan_mode::mode(self.transcript.projections()),
            pending: self.plan_toggle_pending,
        }
    }
    /// The read-only queue strip above the composer, built from the same inbox splices the
    /// transcript retains. One item renders directly; several collapse under a count header,
    /// as the reference's dock does.
    fn queue_dock(&self) -> Option<Element<'_, Message>> {
        let items = self.transcript.queued();
        if items.is_empty() {
            return None;
        }
        let mut list = column![].spacing(4);
        if items.len() > 1 {
            list = list.push(
                button(
                    row![
                        label(format!("{} queued messages", items.len()), MUTED).size(11),
                        Space::new().width(Length::Fill),
                        label(if self.queue_expanded { "▾" } else { "▸" }, MUTED).size(11),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::ToggleQueue)
                .style(design::ghost)
                .padding([3, 6])
                .width(Length::Fill),
            );
        }
        if items.len() == 1 || self.queue_expanded {
            for item in &items {
                list = list.push(
                    row![
                        label("Queue", MUTED).size(10),
                        label(short(&item.preview, 160), TEXT)
                            .size(12)
                            .width(Length::Fill),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                );
            }
        }
        Some(
            container(list)
                .id("native-queue-dock")
                .width(Length::Fill)
                .padding([6, 8])
                .style(|_| design::card(SURFACE, 10.0))
                .into(),
        )
    }

    fn composer(&self) -> Element<'_, Message> {
        if let Some((title, body)) = self.subagent_read_only_copy() {
            // The reference replaces the whole composer for a one-shot child
            // (`SubagentReadOnlyComposer.tsx:31-38`): a centred frame, no input, no controls.
            let frame = container(
                row![label(title, TEXT).size(13), label(body, MUTED).size(13),]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .max_width(self.chat_cap() + design::COMPOSER_CARD_OVERHANG)
            .align_y(iced::Alignment::Center)
            .center_x(Length::Fill)
            // 20px line box + 17px above/below = the reference's 54px minimum, with padding
            // inside that box rather than added outside it (Iced adds container padding).
            .padding([17, 16])
            .style(|_| design::card(SURFACE, design::RADIUS_LG));
            // The reference seats the frame with `margin: 0 24px 20px`; the bottom margin is
            // what keeps its rounded border off the window edge.
            return container(frame)
                .width(Length::Fill)
                .center_x(Length::Fill)
                .padding(iced::Padding {
                    top: 0.0,
                    right: 24.0,
                    bottom: 20.0,
                    left: 24.0,
                })
                .into();
        }
        let allowed = self.send_allowed() && self.composer_visible();
        let keyboard_allowed = allowed && !self.editor.text().trim().is_empty();
        // The menu claims navigation keys only while it is showing.
        let menu_open = self.completion().is_some();
        // The reference's placeholder is state-owned: the empty-state hero asks
        // for a build, the docked bar offers the default line
        // (`locales.ts:20, 396-399`). Native's blocked session-less state keeps
        // the hero line, whose copy names the next step.
        let placeholder = if self.hero_active() && self.selected.is_some() {
            "Describe what you want to build, / commands, @ files or sessions"
        } else {
            "Message or run a task, / commands, @ files or sessions"
        };
        let editor = text_editor(&self.editor)
            .id(accessibility::WIDGET_COMPOSER)
            .placeholder(placeholder)
            .on_action(Message::Editor)
            .height(self.composer_editor_height())
            .padding(design::COMPOSER_INPUT_PAD)
            .font(FONT)
            .size(design::BODY_SIZE as u32)
            .line_height(design::body_line(self.content_delta()))
            .style(design::editor)
            .key_binding(move |event| {
                composer_binding(
                    event,
                    keyboard_allowed,
                    menu_open,
                    &self.settings.keybindings,
                )
            });
        // The shell's editable context menu: a right press opens it, and the anchoring
        // wrapper keeps the anchor's own flow size and dismisses on an outside press
        // (`apps/desktop/src/main.ts:276-300`).
        let editor = iced::widget::mouse_area(editor).on_right_press(Message::EditorMenu(true));
        let editor = anchored_popup::anchored(
            editor.into(),
            self.editor_menu.then(|| {
                editor_menu::view(
                    !self.editor_undo.is_empty(),
                    !self.editor_redo.is_empty(),
                    self.editor.selection().is_some(),
                )
            }),
            Message::EditorMenu(false),
        );
        let sending = self
            .pending
            .values()
            .any(|p| Some(&p.session) == self.selected.as_ref());
        // The reference renders the attachment rail as the card's first child,
        // above the draft (`ConversationContent.tsx` composer bar).
        let mut inner = Column::new().spacing(design::COMPOSER_CARD_GAP);
        if let Some(rail) = composer_rail(self.composer_rail_items()) {
            inner = inner.push(rail);
        }
        inner = inner.push(editor);
        if self.sidebar_organization.failure().is_some() {
            inner = inner.push(row![
                label(
                    "Sidebar organization not confirmed · sending paused",
                    DANGER
                )
                .size(11)
                .width(Length::Fill),
                button(label("Reload", MUTED).size(11))
                    .style(design::ghost)
                    .on_press_maybe(
                        (!self.sidebar_organization.busy())
                            .then_some(Message::Sidebar(sidebar_ui::Action::Reload))
                    )
            ]);
        }
        if self.recovered_draft_needs_review() {
            inner=inner.push(column![label("Recovered text may already have been submitted. Check the conversation first.",DANGER).size(11),
                button(label("Reviewed · enable a new Send",TEXT).size(11)).style(design::ghost)
                    .on_press_maybe(self.pending.is_empty().then_some(Message::ReviewRecovered {generation:self.generation,serial:self.draft_serial}))].spacing(3));
        }
        if self.options.smoke.is_none()
            && (self.draft_state.is_loading() || self.draft_state.failure().is_some())
        {
            let mut recovery = row![
                label(
                    self.draft_state.status(),
                    if self.draft_state.failure().is_some() {
                        DANGER
                    } else {
                        MUTED
                    }
                )
                .size(10)
                .width(Length::Fill)
            ];
            if self.draft_state.failure().is_some() {
                recovery = recovery.push(
                    button(label("Reload drafts", MUTED).size(10))
                        .style(design::ghost)
                        .on_press_maybe((!self.draft_state.busy()).then_some(Message::DraftReload)),
                );
            }
            inner = inner.push(recovery);
        }
        if let Some((generation, key, draft_serial)) = self.edit_review {
            inner = inner.push(
                row![
                    label("Replace your unsent draft?", MUTED)
                        .size(11)
                        .width(Length::Fill),
                    button(label("Replace draft", TEXT).size(11))
                        .style(design::ghost)
                        .on_press_maybe(
                            (self.draft_serial == draft_serial
                                && self.edit_message_allowed(generation, key))
                            .then_some(Message::ConfirmEdit {
                                generation,
                                key,
                                draft_serial
                            })
                        ),
                    button(label("Cancel", MUTED).size(11))
                        .style(design::ghost)
                        .on_press(Message::CancelEdit)
                ]
                .spacing(6),
            );
        }
        if self.observations_lost {
            inner = inner.push(
                button("Reconnect observations · no session resume")
                    .style(design::ghost)
                    .padding([6, 9])
                    .on_press_maybe(
                        (self.ready && !self.rejoin_pending && !self.stopped)
                            .then_some(Message::RejoinObservations),
                    ),
            );
        }
        if let Some(ticket) = self.pending.ticket() {
            let pending = self
                .pending
                .get(&ticket.generation)
                .expect("owned submission");
            let unknown = self.pending.uncertain();
            inner=inner.push(label(if unknown {"Submission unresolved · original retained; current draft is separate. No resend."} else {"Awaiting admission · original request is frozen."},if unknown {DANGER} else {MUTED}).size(11))
                .push(row![button("Check submission").style(design::ghost).padding([6,9]).on_press_maybe((unknown && !self.pending.checking() && self.ready && !self.stopped).then_some(Message::CheckSubmission(ticket.clone()))),button(if self.pending.expanded {"Hide original"} else {"Review original"}).style(design::ghost).padding([6,9]).on_press(Message::ReviewSubmission(ticket.clone()))].spacing(6));
            if self.selected.as_ref() != Some(&ticket.session)
                || self.generation != ticket.generation
            {
                inner = inner.push(
                    button("Stop original session")
                        .style(design::ghost)
                        .padding([6, 9])
                        .on_press_maybe(
                            (self.ready && !self.cancel_pending && !self.stopped)
                                .then_some(Message::StopSubmission(ticket.clone())),
                        ),
                );
            }
            if !pending.file.is_empty() {
                let bytes: u64 = pending.file.iter().map(|file| file.bytes).sum();
                let description = if pending.file.len() == 1 {
                    format!(
                        "Original file: {} · {} bytes · receipt is never replayed",
                        pending.file[0].name, bytes
                    )
                } else {
                    format!(
                        "Original files: {} · {} bytes total · receipts are never replayed",
                        pending.file.len(),
                        bytes
                    )
                };
                inner = inner.push(label(description, MUTED).size(11));
            }
            if self.pending.expanded {
                inner = inner.push(
                    scrollable(label(pending.text.clone(), TEXT).size(12))
                        .direction(design::scrollbar())
                        .style(design::scrollable_style)
                        .height(100),
                );
            }
        }
        if self.files.editor_open()
            || self.files.ticket().is_some()
            || self.files.notice().is_some()
            || self.files.pending()
            || self.files.prompt_pending()
            || self.local_files.busy()
        {
            inner = inner.push(self.file_controls());
        }
        // One reading: the tier colours the line, and only a real window with a
        // plausible occupancy can raise one (see `chat_usage::Severity`).
        let context = chat_usage::state(
            self.transcript.projections(),
            self.follow_ready
                && self.transcript.safe_to_send()
                && !self.model_pending
                && !self.observations_lost
                && self.transcript.cursor() == self.transcript.opening_cursor(),
            self.transcript.records(),
        );
        // The web's `.row` pads itself 2/8/6 and gaps its groups by 12px
        // (`InputBar.module.css:251-260`); the card contributes no side padding.
        // The web's left group holds its own command control; native's attach
        // affordance has no counterpart box there, so it keeps its label and takes
        // the chip height. The plan chip follows it, and only exists while the
        // mode is on, so the row's 12px gap cannot leave a phantom slot.
        let mut controls = row![
            container(
                button(
                    label(
                        if self.files.count() == 0 {
                            "+ Attach".to_owned()
                        } else {
                            format!(
                                "+ Attach · {}/{}",
                                self.files.count(),
                                attachment_draft::MAX_ATTACHMENTS
                            )
                        },
                        MUTED,
                    )
                    .size(11)
                )
                .on_press_maybe(
                    (self.file_context().allowed
                        && !self.files.editor_open()
                        && !self.files.at_capacity()
                        && !self.files.pending()
                        && !self.files.prompt_pending()
                        && !self.local_files.busy())
                    .then_some(Message::FileChoose)
                )
                .style(design::ghost)
                .padding([6, 9])
                .height(design::COMPOSER_CHIP_HEIGHT)
            )
            .id("native-file-open"),
        ];
        if let Some(chip) = self.plan_chip() {
            controls = controls.push(chip);
        }
        if let Some(chip) = self.permission_chip() {
            controls = controls.push(chip);
        }
        controls = controls
            .push(Space::new().width(Length::Fill))
            // The web puts the model select in the trailing group beside the send
            // control (`InputBar.tsx:450-486`).
            .push(self.model_control())
            .push(self.stop_button())
            .push(
                container(
                    button(
                        text(if sending {
                            "Sending…"
                        } else if self.busy_enter == crate::settings::BusyEnter::Steer
                            && !self.files.has_file(&self.file_context())
                            && self
                                .selected
                                .as_ref()
                                .and_then(|id| self.sessions.get(id))
                                .is_some_and(|summary| summary.running)
                        {
                            "Steer message ↑"
                        } else {
                            "Send ↑"
                        })
                        .font(FONT)
                        .size(13),
                    )
                    .on_press_maybe(allowed.then_some(Message::Send))
                    .style(design::primary)
                    .padding([8, 14])
                    .height(design::COMPOSER_CONTROL_HEIGHT),
                )
                .id("native-send-control"),
            );
        inner = inner.push(
            container(
                controls
                    .spacing(design::COMPOSER_ROW_GAP)
                    .align_y(iced::Alignment::Center),
            )
            .padding(design::COMPOSER_ROW_PAD)
            .width(Length::Fill),
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
        // The paper's strongest single effect is that a context which overflows
        // ends the run early, so reaching the window is stated, not tinted only.
        if context.severity == chat_usage::Severity::Critical {
            inner = inner.push(
                label(
                    "Context at the window limit — the next turns risk an early stop.",
                    DANGER,
                )
                .size(11),
            );
        }
        // The web's card is the elevated input surface (the same token as the
        // attachment card) with an 8px top inset only; the text area and the
        // control row own their insets (`InputBar.module.css:60-72`).
        let card = container(inner)
            .padding(iced::Padding {
                top: design::COMPOSER_CARD_TOP,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            })
            .width(Length::Fill)
            .style(design::elevated_surface);
        // The card owns the space around its controls: a press there is the user
        // reaching for the composer, not leaving it. Inner presses are consumed by
        // the widget that handles them, so this only fires on the card itself.
        let card = Element::new(FocusKeeper {
            content: card.into(),
            on_press: Message::ComposerFocus,
        });
        let steering = self.busy_enter == crate::settings::BusyEnter::Steer
            && !self.files.has_file(&self.file_context())
            && self
                .selected
                .as_ref()
                .and_then(|id| self.sessions.get(id))
                .is_some_and(|summary| summary.running);
        let hint = if self.options.smoke.is_some() {
            "Keyless preview · sending disabled"
        } else if self.compact() {
            "Shift+Enter · new line"
        } else if steering {
            "Enter to steer · Shift+Enter for a new line"
        } else {
            "Enter to send · Shift+Enter for a new line"
        };
        let severity_color = match context.severity {
            chat_usage::Severity::Normal => MUTED,
            chat_usage::Severity::Tight => TEXT,
            chat_usage::Severity::Critical => DANGER,
        };
        let plan = self
            .transcript
            .work()
            .plan()
            .map(|(done, total, next)| match next {
                Some(next) => format!("Plan · {done}/{total} · next: {next}"),
                None if done == total => format!("Plan · {done}/{total} · complete"),
                None => format!("Plan · {done}/{total}"),
            });
        let mut metadata = row![];
        // Plan mode's actionable control now rides the card's own control row as
        // the reference draws it (`PlanModeControl.module.css`), so the strip
        // keeps only the in-flight status it can add: the projection still owns
        // the mode, and a pending toggle stays visible while it settles.
        let control = self.plan_control();
        if let Some(mode) = control.mode {
            if control.pending {
                metadata = metadata.push(label(mode.label(), ACCENT).size(10));
            }
        }
        if let Some(plan) = plan {
            metadata = metadata.push(label(short(&plan, 60), MUTED).size(10));
        }
        metadata = metadata.push(label(context.line.clone(), severity_color).size(10));
        if !context.detail.is_empty() {
            metadata = metadata.push(
                button(
                    label(
                        format!(
                            "{} Context policy",
                            if self.context_policy_open {
                                "▾"
                            } else {
                                "▸"
                            }
                        ),
                        MUTED,
                    )
                    .size(10),
                )
                .padding([2, 4])
                .style(design::ghost)
                .on_press(Message::ToggleContextPolicy),
            );
        }
        metadata = metadata
            .push(Space::new().width(Length::Fill))
            .push(label(hint, MUTED).size(10));
        let mut shell = column![];
        if let Some(dock) = self.queue_dock() {
            shell = shell.push(dock);
        }
        // The completion menu sits directly above the card, so it reads as an
        // extension of the composer rather than a floating panel.
        if let Some(menu) = self.completion() {
            let mut rows = column![].spacing(1);
            for (index, candidate) in menu.matches.iter().enumerate() {
                let selected = index == menu.selected;
                let row = container(
                    row![
                        label(
                            format!("/{}", candidate.name),
                            if selected { ACCENT } else { MUTED },
                        )
                        .size(12)
                        .width(Length::Shrink),
                        label(short(&candidate.description, 70), MUTED)
                            .size(10)
                            .width(Length::Fill),
                        // A caret marks the highlighted row even where a
                        // background is hard to read at 1x.
                        label(if selected { "▸" } else { "" }, ACCENT).size(10),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                )
                .padding([4, 7])
                .width(Length::Fill)
                .style(move |_| {
                    if selected {
                        design::card(OVERLAY, design::RADIUS_SM)
                    } else {
                        container::Style::default()
                    }
                });
                // The row completes the command it names, and the draft keeps the
                // caret: a pointer and the arrow keys then Tab all mean the same
                // thing, and the next keystroke continues the line.
                rows = rows.push(Element::new(FocusKeeper {
                    content: row.into(),
                    on_press: Message::CommandMenuPick(candidate.name.clone()),
                }));
            }
            shell = shell.push(
                container(rows)
                    .padding(4)
                    .width(Length::Fill)
                    .style(|_| design::card(SURFACE, design::RADIUS_SM)),
            );
        }
        if self.context_policy_open {
            let mut detail = column![];
            for line in &context.detail {
                // Wrapped, because a one-line paragraph is clipped by the lane
                // and a clipped warning is not a warning.
                detail = detail.push(
                    label(line.clone(), MUTED)
                        .size(10)
                        .width(Length::Fill)
                        .wrapping(iced::widget::text::Wrapping::Word),
                );
            }
            shell = shell.push(container(detail).padding([0, 8]).width(Length::Fill));
        }
        // The strip between the card and the lane edge belongs to the composer
        // too; only the controls inside it publish, so a press on the gap keeps
        // the caret where the user left it.
        let metadata = Element::new(FocusKeeper {
            content: container(metadata.spacing(8).align_y(iced::Alignment::Center))
                .padding([0, 8])
                .into(),
            on_press: Message::ComposerFocus,
        });
        shell = shell.push(card).push(metadata);
        // The web caps the composer card at the content width + 32px inside the
        // seat's own 16px side padding, so the card overhangs the transcript by
        // 16px per side and its dock strip sits 4px under it
        // (`ConversationRoot.module.css:388`, `InputBar.module.css:9-20`).
        container(
            container(shell.spacing(design::COMPOSER_DOCK_GAP))
                .max_width(self.chat_cap() + design::COMPOSER_CARD_OVERHANG)
                .width(Length::Fill),
        )
        .center_x(Length::Fill)
        .padding(if self.hero_active() {
            design::COMPOSER_HERO_SEAT_PAD
        } else {
            design::COMPOSER_SEAT_PAD
        })
        .into()
    }
    fn sidebar(&self) -> Element<'_, Message> {
        let width = self.sidebar_motion.value();
        if self.compact() || width < 185.0 {
            // The reference's rail spec is 36x36 control boxes centered in the
            // 56px rail (10px side padding), an 18px top seat and a 12px
            // vertical rhythm (`SidebarRoot.module.css:34-37`). Native's labels
            // stand in for the web's icon buttons and take those boxes.
            let rail = column![
                brand(),
                // A 36px box cannot hold "+ New"; the reference's rail is
                // icon-only anyway, so the new-session control is its plus and
                // the word survives in the expanded sidebar. `+` and the gear
                // below are glyphs DejaVu Sans carries.
                button(label("Chats", MUTED).size(11))
                    .on_press(Message::ToggleSidebar)
                    .style(design::ghost)
                    .width(Length::Fill)
                    .height(design::RAIL_CONTROL),
                button(label("+", TEXT).size(16))
                    .on_press_maybe(self.can_create().then_some(Message::NewSession))
                    .style(design::ghost)
                    .width(Length::Fill)
                    .height(design::RAIL_CONTROL),
                Space::new().height(Length::Fill),
                self.settings_entry(true)
            ]
            .spacing(design::RAIL_RHYTHM)
            .align_x(iced::Alignment::Center);
            return container(rail)
                .padding(design::ATTACH_RAIL_PAD)
                .width(width)
                .height(Length::Fill)
                .clip(true)
                .style(|_| panel(SURFACE))
                .into();
        }
        // The web's New Session bar: a filled, hairline-edged `radius-md` bar
        // with its own 2px inset and 12px of clear space below
        // (`SidebarRoot.module.css:397-409`).
        let new_session = container(
            button(
                // The web centers the icon and label in the bar
                // (`SidebarRoot.module.css:470`).
                container(
                    row![
                        label("+", TEXT).size(14),
                        label(
                            if self.new_pending {
                                "Creating…"
                            } else {
                                "New conversation"
                            },
                            TEXT
                        )
                        .size(14),
                    ]
                    .spacing(6),
                )
                .center_x(Length::Fill),
            )
            .on_press_maybe(self.can_create().then_some(Message::NewSession))
            .style(design::elevated)
            .padding(design::NEW_SESSION_PAD)
            .height(design::NEW_SESSION_HEIGHT)
            .width(Length::Fill),
        )
        .padding([0.0, design::NEW_SESSION_INSET]);
        let workspaces = button(
            label(
                if self.workspace_controls {
                    "⌄  Workspaces"
                } else {
                    "›  Workspaces"
                },
                MUTED,
            )
            .size(14),
        )
        .on_press(Message::WorkspaceControls)
        .style(|theme, status| design::navigation_row(theme, status, false))
        .padding(design::PANEL_ROW_PAD)
        .height(design::PANEL_ROW_HEIGHT)
        .width(Length::Fill);
        // Native keeps its own sidebar order (sessions, then the workspace
        // seat, then Settings); only the web's part metrics and its own gaps are
        // taken: `.logoRow`'s 4px, `.newSession`'s 12px, and `.panelList`'s 4px
        // gap with its 8px trailing margin.
        let navigation = column![
            row![
                brand(),
                label("Harness", TEXT).size(17).width(Length::Fill),
                button(label("‹", MUTED).size(22))
                    .on_press(Message::ToggleSidebar)
                    .style(design::ghost)
                    .padding([3, 8])
            ]
            .spacing(9)
            .align_y(iced::Alignment::Center),
            Space::new().height(design::SIDEBAR_LOGO_GAP),
            new_session,
            Space::new().height(design::NEW_SESSION_GAP),
            self.session_navigation(),
            Space::new().height(design::PANEL_LIST_GAP),
            workspaces,
            Space::new().height(design::PANEL_GAP),
            self.workspace_navigation(),
            Space::new().height(design::PANEL_GAP),
            self.settings_entry(false)
        ];
        container(navigation)
            .padding(design::SIDEBAR_PAD)
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
        let Some(id) = self.selected.clone() else {
            return;
        };
        self.observe_title_for(&id, seq, value);
    }
    /// Record one title projection for an explicit Session, including a rename ACK.
    /// Nothing is displayed until it is durable; a malformed value is ignored.
    fn observe_title_for(&mut self, id: &SessionId, seq: i64, value: serde_json::Value) {
        if seq < -1
            || !self.sessions.contains_key(id)
            || (self.title_updates.len() >= 4096 && !self.title_updates.contains_key(id))
            || self
                .title_updates
                .get(id)
                .is_some_and(|(old, _)| *old > seq)
        {
            return;
        }
        let value = match value {
            serde_json::Value::Null => serde_json::Value::Null,
            serde_json::Value::String(value) if !value.trim().is_empty() => {
                serde_json::Value::String(reducer::bounded(value.trim(), 1024))
            }
            _ => return,
        };
        self.title_updates.insert(id.clone(), (seq, value));
    }
    /// The header's breadcrumb trail: the selected session and, while it is a
    /// subagent, its ancestors. The reference walks `parentId` while
    /// `origin === 'subagent'` and renders every crumb in one `4px`-gapped row —
    /// ancestors as `4px 8px`-padded links that select that session, the current
    /// one as plain primary text in the same box, and a `14px` caption `/`
    /// between them (`ConversationSession.tsx:25-43, 73-104`,
    /// `ConversationRoot.module.css:102-146`). The heading arrives already
    /// shortened, because Iced has no ellipsis and a wrapped crumb would grow
    /// the header.
    fn header_crumbs(&self, heading: &str, title_limit: usize) -> Element<'_, Message> {
        let mut trail: Vec<(SessionId, String, bool)> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut cursor = self.selected.clone();
        while let Some(id) = cursor {
            if !seen.insert(id.clone()) {
                break;
            }
            let Some(summary) = self.sessions.get(&id) else {
                break;
            };
            let subagent = summary.origin.is_some();
            let title = if Some(&id) == self.selected.as_ref() {
                heading.to_owned()
            } else {
                short(&self.session_title(summary), title_limit)
            };
            trail.push((id.clone(), title, subagent));
            if !subagent {
                break;
            }
            cursor = summary.parent_session_id.clone();
        }
        trail.reverse();
        // No selected session is the session-less header: one plain current crumb.
        if trail.is_empty() {
            return container(
                container(
                    label(heading.to_owned(), TEXT)
                        .size(design::CRUMB_SIZE)
                        .line_height(iced::Pixels(design::CRUMB_LINE))
                        .wrapping(iced::advanced::text::Wrapping::None),
                )
                .padding(design::CRUMB_PAD)
                .max_width(design::CRUMB_MAX),
            )
            .id("native-header-crumbs")
            .into();
        }
        let mut crumbs = row![]
            .spacing(design::CRUMB_GAP)
            .align_y(iced::Alignment::Center);
        let last = trail.len().saturating_sub(1);
        for (index, (id, title, subagent)) in trail.into_iter().enumerate() {
            if index > 0 {
                crumbs = crumbs.push(
                    label("/", MUTED)
                        .size(design::CRUMB_SIZE)
                        .line_height(iced::Pixels(design::CRUMB_LINE)),
                );
            }
            let current = index == last;
            let size = if subagent {
                design::CRUMB_SUBAGENT_SIZE
            } else {
                design::CRUMB_SIZE
            };
            let line = if subagent {
                design::CRUMB_SUBAGENT_LINE
            } else {
                design::CRUMB_LINE
            };
            let ink = if current { TEXT } else { MUTED };
            // A selected child's current crumb is the reference's sibling switcher: the title
            // in primary ink with a chevron, opening the parent's catalog. Roots and parents
            // whose catalog is absent keep the plain crumb.
            let crumb: Element<'_, Message> = if current
                && subagent
                && let Some(siblings) = self.parent_catalog()
            {
                let trigger = button(
                    row![
                        label(title, TEXT)
                            .size(size)
                            .line_height(iced::Pixels(line))
                            .wrapping(iced::advanced::text::Wrapping::None),
                        label("▾", MUTED).size(design::CRUMB_SIZE),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::ToggleSubagentMenu(!self.subagent_menu))
                .style(design::ghost)
                .padding(design::CRUMB_PAD)
                .into();
                anchored_popup::anchored_below(
                    trigger,
                    self.subagent_menu
                        .then(|| self.subagent_menu_view(&siblings)),
                    Message::ToggleSubagentMenu(false),
                )
            } else {
                let text = label(title, ink)
                    .size(size)
                    .line_height(iced::Pixels(line))
                    .wrapping(iced::advanced::text::Wrapping::None);
                let boxed = container(text)
                    .padding(design::CRUMB_PAD)
                    .max_width(design::CRUMB_MAX);
                if current {
                    boxed.into()
                } else {
                    button(boxed)
                        .on_press(Message::Select(id))
                        .style(design::ghost)
                        .padding([0.0, 0.0])
                        .into()
                }
            };
            crumbs = crumbs.push(crumb);
        }
        // `ui-subagent/src/client/index.ts:83-92` fixes the band's order in its own
        // comment: subagent lineage "leads the band, directly after the title crumbs",
        // "ahead of Team navigation (-20) and the preset label (-10)". Native keeps that
        // lineage inside the trail itself, as the title's continuation, so the preset label
        // belongs *after* the trail rather than before it -- which is where it started, and
        // wrong. The projection carries no display name, so the key stands in, as the
        // permission chip does without its catalog.
        if let Some(preset) = self.transcript.agent_preset() {
            // The projection carries the key only; the roster carries the deployment's own
            // name for it, and the key stands in until the roster answers -- the same
            // fallback rule as the permission chip without its catalog.
            let shown = self
                .agent_roster
                .as_ref()
                .and_then(|roster| {
                    roster
                        .presets
                        .iter()
                        .find(|row| row.id == preset)
                        .map(|row| row.name.clone().unwrap_or_else(|| row.id.clone()))
                })
                .unwrap_or_else(|| preset.to_owned());
            crumbs = crumbs.push(
                container(
                    label(shown, MUTED)
                        .size(design::CRUMB_SIZE)
                        .line_height(iced::Pixels(design::CRUMB_LINE))
                        .wrapping(iced::advanced::text::Wrapping::None),
                )
                .id("native-agent-preset")
                .padding(design::CRUMB_PAD)
                .max_width(design::CRUMB_MAX),
            );
        }
        // `ui-jobs/src/client/index.ts:36-42`: background work follows the preset label at
        // order 20, and its label is the live count (`locales.ts:8-11`).
        if !self.jobs.is_empty() {
            let running = self
                .jobs
                .iter()
                .filter(|job| job.status == "running" || job.status == "stopping")
                .count();
            // The reference's own English copy distinguishes live from idle **and** singular
            // from plural (`ui-jobs/src/client/locales.ts:51-54`): "{count} background job
            // running" / "{count} background jobs running" / "{count} background job" /
            // "{count} background jobs". An earlier revision of this arm translated the
            // package's Chinese dictionary instead of reading the English one, which is the
            // same class of mistake as an invented command line.
            let count = if running > 0 {
                running
            } else {
                self.jobs.len()
            };
            let noun = if count == 1 {
                "background job"
            } else {
                "background jobs"
            };
            let text = if running > 0 {
                format!("{count} {noun} running")
            } else {
                format!("{count} {noun}")
            };
            let action = button(label(text, MUTED).size(design::CRUMB_SIZE))
                .on_press(Message::ToggleJobs)
                .style(design::ghost)
                .padding(design::CRUMB_PAD);
            // The panel is anchored to the action, the way the preset seat's list is anchored to
            // its chip. Its sections and status words come from the reference's own English copy
            // (`locales.ts:56-69`); a row's state dot is not drawn because `StateDot`'s form is
            // unread, and the status word is the part the copy names.
            crumbs = crumbs.push(
                container(anchored_popup::anchored_below(
                    action.into(),
                    self.jobs_open.then(|| self.jobs_panel_view()),
                    Message::ToggleJobs,
                ))
                .id("native-jobs-action"),
            );
        }
        // The row's own box is the trail's intrinsic width, which is what a
        // capture can measure: one crumb, or the ancestors plus separators.
        container(crumbs).id("native-header-crumbs").into()
    }
    fn session_title(&self, session: &SessionSummary) -> String {
        let value = self
            .title_updates
            .get(&session.session_id)
            .filter(|(seq, _)| {
                session
                    .projections
                    .as_ref()
                    .is_none_or(|p| p.as_of_seq <= *seq)
            });
        value.map_or_else(
            || title(session),
            |(_, value)| projected_title(session, Some(value)),
        )
    }
    fn navigation_workspace_ids(&self) -> Option<&[SessionId]> {
        self.workspace
            .as_ref()
            .and_then(|id| self.workspaces.iter().find(|w| &w.workspace_id == id))
            .map(|w| w.session_ids.as_slice())
    }
    fn navigation_candidates(&self) -> impl Iterator<Item = &SessionSummary> {
        self.navigation_candidates_for(self.navigation_workspace_ids())
    }
    fn navigation_candidates_for<'a>(
        &'a self,
        ids: Option<&'a [SessionId]>,
    ) -> impl Iterator<Item = &'a SessionSummary> {
        let filter = self.filter.to_lowercase();
        let mut membership = workspace_membership::Membership::new(ids);
        self.sessions.values().filter(move |s| {
            // The reference keeps subagent children out of the session list; they are reached
            // through their parent's header catalog (`ui-workspace/src/client/tree.ts:240-244`).
            s.origin.is_none()
                && self.sidebar_organization.trashed(&s.session_id) == self.show_trash
                && (self.show_trash
                    || self.management.registry.archived(&s.session_id) == self.show_archived)
                && (self.show_trash
                    || match self.sidebar_organization.location(&s.session_id) {
                        sidebar_organization::Location::Original => {
                            membership.contains(&s.session_id)
                        }
                        sidebar_organization::Location::Workspace(id) => {
                            ids.is_none() || self.workspace.as_ref() == Some(&id)
                        }
                        sidebar_organization::Location::Ungrouped => ids.is_none(),
                    })
                && (filter.is_empty()
                    || s.session_id.as_str().to_lowercase().contains(&filter)
                    || self.session_title(s).to_lowercase().contains(&filter))
        })
    }
    /// Direct children of a parent that are running, read from the parent summary's own
    /// `subagentCatalog` projection and each child's session state — the reference's
    /// `runningChildCount` (`ui-workspace/src/client/tree.ts:382-386`).
    fn running_subagent_count(&self, parent: &SessionId) -> usize {
        let Some(projections) = self
            .sessions
            .get(parent)
            .and_then(|summary| summary.projections.as_ref())
        else {
            return 0;
        };
        subagent_catalog::entries(&projections.values)
            .iter()
            .filter(|entry| self.subagent_running(&entry.id))
            .count()
    }
    fn navigation_sessions(&self) -> Vec<&SessionSummary> {
        let ids = self.navigation_workspace_ids();
        if ids.is_some_and(|ids| ids.is_empty())
            && !self.show_trash
            && self.sidebar_organization.data.is_empty()
        {
            return Vec::new();
        }
        let capacity = if self.filter.is_empty() {
            ids.map_or(self.sessions.len(), |ids| {
                ids.len().min(self.sessions.len())
            })
        } else {
            0
        };
        let mut sessions = Vec::with_capacity(capacity);
        sessions.extend(self.navigation_candidates_for(ids));
        sessions.sort_by(|a, b| self.navigation_order(a, b));
        sessions
    }
    fn session_navigation(&self) -> Element<'_, Message> {
        let sessions = self.navigation_sessions();
        let count = sessions.len();
        // The web's session cells are 32px tall and stacked with no gap between
        // them; a native row with a status line grows instead of clipping.
        let mut list = Column::new();
        for session in sessions.into_iter().take(200) {
            let active = self.selected.as_ref() == Some(&session.session_id);
            let mut identity = column![
                // The web's `.sessionRow .title` is one clipped line
                // (`Rows.module.css:162-166`, `text-overflow: ellipsis`); Iced has
                // no ellipsis, so the 20px box clips instead of wrapping the row
                // taller than the web's 32px cell.
                container(
                    label(
                        short(&self.session_title(session), 28),
                        if active || session.running {
                            TEXT
                        } else {
                            MUTED
                        },
                    )
                    .size(14)
                    .line_height(design::session_title_line())
                    .wrapping(iced::advanced::text::Wrapping::None),
                )
                .width(Length::Fill)
                .height(design::SESSION_TITLE_LINE)
                .clip(true)
            ]
            .spacing(3)
            .width(Length::Fill);
            if !session.agent_available {
                identity = identity.push(label("Resume · queued input may run", MUTED).size(11));
            }
            let pending = active.then_some(self.decisions.pending_label()).flatten();
            if let Some(status) = pending {
                identity = identity.push(label(status, design::GOLD).size(11));
            }
            let running_children = self.running_subagent_count(&session.session_id);
            if running_children > 0 {
                identity = identity.push(
                    label(
                        subagent_catalog::count_label(running_children, running_children),
                        design::FOAM,
                    )
                    .size(11),
                );
            }
            if let Some(status) = self.working_label(session).filter(|_| pending.is_none()) {
                identity = identity.push(
                    label(
                        status,
                        if self.running_status_live() {
                            design::FOAM
                        } else {
                            MUTED
                        },
                    )
                    .size(11),
                );
            }
            let mut content = row![].spacing(8).align_y(iced::Alignment::Center);
            if session.running && self.running_status_live() {
                content = content.push(working_indicator::view(self.working_phase));
            }
            content = content.push(identity);
            if self.management.registry.pinned(&session.session_id) {
                content = content.push(label("•", design::FOAM).size(10));
            }
            list = list.push(
                row![
                    button(content)
                        .on_press_maybe(
                            (self.ready
                                && !self.observations_lost
                                && self.pending.is_empty()
                                && !self.show_trash
                                && !self.sidebar_organization.busy()
                                && self.sidebar_organization.failure().is_none())
                            .then_some(Message::Select(session.session_id.clone())),
                        )
                        .style(move |theme, status| design::navigation_row(theme, status, active))
                        .padding(design::SESSION_ROW_PAD)
                        .width(Length::Fill),
                    button(
                        label("⋯", MUTED)
                            .size(14)
                            .line_height(design::session_title_line()),
                    )
                    .style(design::ghost)
                    .padding([0.0, 5.0])
                    .height(design::ROW_ACTION_HEIGHT)
                    .on_press_maybe(
                        self.sidebar_target_allowed(&session.session_id)
                            .then(|| self.sidebar_open_message(&session.session_id))
                    )
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            );
        }
        if self.sidebar_organization.failure().is_some() {
            list = list
                .push(label("Organization not confirmed", DANGER).size(11))
                .push(
                    button(label("Reload organization", MUTED).size(11))
                        .style(design::ghost)
                        .on_press_maybe(
                            (!self.sidebar_organization.busy())
                                .then_some(Message::Sidebar(sidebar_ui::Action::Reload)),
                        ),
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
            // The web's expanded search control: a 30px box with a `0.5px`
            // hairline over a transparent fill and `13px/18px` text
            // (`WorkspaceBrowser.module.css:160-171, 214-222`). Native keeps the
            // field mounted where the web expands it from a 28px icon control.
            text_input("Search conversations", &self.filter)
                .on_input(Message::Filter)
                .padding(design::SEARCH_PAD)
                .size(design::SEARCH_FONT)
                // Iced's `line_height(f32)` is a *factor* of the font size;
                // the web's `18px` box needs `Pixels` for an absolute height.
                .line_height(iced::Pixels(design::SEARCH_LINE))
                .font(FONT)
                .style(design::search_field),
            Space::new().height(design::SECTION_HEADER_GAP),
            // The web's `.sectionHeader` is a 36px box with a 4px gap and a 4px
            // trailing margin (`WorkspaceBrowser.module.css:53-62`). Its label is
            // 14px there and its trailing controls are 28px icon squares; native's
            // denser `RECENT`/view controls keep the hint tier and text stand-ins,
            // so only the box and the control height are taken.
            row![
                label(
                    if self.show_trash {
                        "TRASH"
                    } else if self.show_archived {
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
                .style(design::ghost)
                .height(design::SECTION_ACTION_HEIGHT),
                button(label(if self.show_trash { "Recent" } else { "Trash" }, MUTED).size(10))
                    .on_press(Message::Sidebar(sidebar_ui::Action::TrashView))
                    .style(design::ghost)
                    .height(design::SECTION_ACTION_HEIGHT)
            ]
            .spacing(design::SECTION_HEADER_GAP)
            .height(design::SECTION_HEADER_HEIGHT)
            .align_y(iced::Alignment::Center),
            Space::new().height(design::SECTION_HEADER_GAP),
            scrollable(list)
                .direction(design::scrollbar())
                .style(design::scrollable_style)
                .height(Length::Fill),
            // Native's own footer line, set off by the panel rhythm.
            Space::new().height(design::PANEL_GAP),
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
        .spacing(0)
        .height(Length::Fill)
        .into()
    }
    fn workspace_navigation(&self) -> Element<'_, Message> {
        if !self.workspace_controls && !self.sidebar_sheet {
            return Space::new().height(0).into();
        }
        let mut list = column![
            button(label("All conversations", TEXT).size(14))
                .on_press(Message::Workspace(None))
                .style(|theme, status| design::navigation_row(theme, status, false))
                .padding(design::SESSION_ROW_PAD)
                .width(Length::Fill)
        ];
        for workspace in self.workspaces.iter().take(128) {
            let active = self.workspace.as_ref() == Some(&workspace.workspace_id);
            list = list.push(
                button(
                    label(
                        short(&workspace.title, 28),
                        if active { TEXT } else { MUTED },
                    )
                    .size(14),
                )
                .on_press(Message::Workspace(Some(workspace.workspace_id.clone())))
                .style(move |theme, status| design::navigation_row(theme, status, active))
                .padding(design::SESSION_ROW_PAD)
                .width(Length::Fill),
            );
        }
        column![
            scrollable(list)
                .direction(design::scrollbar())
                .style(design::scrollable_style)
                .height(if self.sidebar_sheet { 110 } else { 80 }),
            text_input("Absolute workspace directory", &self.workspace_path)
                .on_input(Message::WorkspacePath)
                .padding(9)
                .size(11)
                .font(FONT)
                .style(design::field),
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
            .style(design::ghost)
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
                .style(design::ghost)
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
            .style(design::ghost),
            container(self.session_navigation()).height(210),
            label("WORKSPACES", MUTED).size(11),
            self.workspace_navigation()
        ]
        .spacing(14);
        container(
            column![
                toolbar,
                scrollable(sheet)
                    .direction(design::scrollbar())
                    .style(design::scrollable_style)
                    .height(Length::Fill)
            ]
            .spacing(18),
        )
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
    fn detail_view<'a>(&'a self, detail: &'a detail_panel::Detail) -> Element<'a, Message> {
        let mut toolbar = row![
            label(detail.heading.as_str(), TEXT)
                .size(18)
                .width(Length::Fill)
        ];
        if detail.formatted_available() {
            for (mode, title) in [
                (detail_panel::Mode::Formatted, "Formatted"),
                (detail_panel::Mode::Source, "Source"),
            ] {
                let selected = detail.mode == mode;
                let control = button(label(title, if selected { ACCENT } else { MUTED }).size(12))
                    .on_press_maybe(
                        detail
                            .ticket
                            .map(|ticket| Message::DetailMode(ticket, mode)),
                    )
                    .style(move |theme, status| design::navigation(theme, status, selected));
                #[cfg(feature = "public-layout-fixture")]
                let control = container(control).id(match mode {
                    detail_panel::Mode::Formatted => "native-detail-formatted",
                    detail_panel::Mode::Source => "native-detail-source",
                });
                toolbar = toolbar.push(control);
            }
        }
        let copy = button(label("Copy source", MUTED).size(12))
            .on_press_maybe(
                detail
                    .ticket
                    .filter(|_| detail.copy_available())
                    .map(Message::CopyDetailSource),
            )
            .style(design::ghost);
        #[cfg(feature = "public-layout-fixture")]
        let copy = container(copy).id("native-detail-copy-source");
        toolbar = toolbar.push(copy);
        let back = button(label("Back", MUTED))
            .on_press_maybe(self.detail_back())
            .style(design::ghost);
        #[cfg(feature = "public-layout-fixture")]
        let back = container(back).id("native-detail-back");
        toolbar = toolbar
            .push(back)
            .spacing(8)
            .align_y(iced::Alignment::Center);
        let mut body = Column::new().spacing(12).width(Length::Fill);
        if let Some(metadata) = &detail.metadata {
            body = body.push(label(metadata.as_str(), MUTED).size(11));
        }
        let rendered = detail.body();
        #[cfg(feature = "public-layout-fixture")]
        let rendered = container(rendered).id("native-detail-body");
        body = body.push(rendered);
        let content=column![toolbar,label(detail.notice(),MUTED).size(11),
            scrollable(container(body).padding(16))
                .direction(design::scrollbar())
                .style(design::scrollable_style).height(Length::Fill),
            label("Bounded native display source (up to 32 KiB); raw records and unsupported content may not be fully represented.",MUTED).size(11)
        ].spacing(12).height(Length::Fill);
        container(
            container(content)
                .max_width(self.chat_lane())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .padding(20)
        .center_x(Length::Fill)
        .height(Length::Fill)
        .into()
    }
    fn reply_parts(&self, key: u64) -> Option<std::rc::Rc<conversation_content::Prepared>> {
        if let Some(prepared) = self.reply_content.get(self.generation, key) {
            return Some(prepared);
        }
        let parts = if key == u64::MAX {
            self.transcript.partial_parts()
        } else {
            self.transcript.assistant_parts(key)
        }?;
        Some(
            self.reply_content
                .prepare(self.generation, key, parts.reasoning, parts.answer),
        )
    }
    fn row_copy_source(&self, key: u64) -> Option<String> {
        if key == u64::MAX {
            return None;
        } // Only a frozen durable source is copied.
        let row = self
            .transcript
            .display_rows(self.records_expanded)
            .find(|row| row.key == key)?;
        if let Some(parts) = self.transcript.assistant_parts(key) {
            return (!parts.answer.is_empty()).then_some(parts.answer);
        }
        ((row.role == "You" || self.transcript.assistant_record(key)) && !row.text.is_empty())
            .then(|| row.text.clone())
    }
    fn edit_message_allowed(&self, generation: u64, key: u64) -> bool {
        generation == self.generation
            && self.composer_visible()
            && self.layout_controls_enabled()
            && self.pending.is_empty()
            && !self.observations_lost
            && self.files.ticket().is_none()
            && self.draft_serial != u64::MAX
            && self.transcript.editable_text(key).is_some()
    }
    fn populate_message_draft(&mut self, key: u64) {
        if let Some(text) = self.transcript.editable_text(key) {
            self.draft_serial = self.draft_serial.saturating_add(1);
            self.editor = text_editor::Content::with_text(&text);
            self.touch_draft();
            self.edit_review = None;
            self.status = "Copied into the composer. Edit, then Send explicitly; existing conversation is unchanged.".into();
        }
    }
    fn active_search(&self) -> &str {
        if self.search_open {
            &self.search_query
        } else {
            ""
        }
    }
    fn search_hits(&self) -> Vec<u64> {
        let query = self.active_search();
        if query.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<_> = self
            .transcript
            .display_rows(self.records_expanded)
            .filter(|row| {
                if let Some(reply) = self.reply_parts(row.key) {
                    reply.matches(query)
                } else if row.role == "You" {
                    conversation_search::contains(&row.text, query)
                } else {
                    false
                }
            })
            .map(|row| row.key)
            .collect();
        if self.transcript.partial().is_some()
            && self
                .reply_parts(u64::MAX)
                .is_some_and(|reply| reply.matches(query))
        {
            hits.push(u64::MAX);
        }
        hits
    }
    fn search_expands_user(&self, row: &DisplayRow) -> bool {
        row.role == "You" && !self.active_search().is_empty() && self.search_cursor == Some(row.key)
    }
    fn jump_search(&mut self, backwards: bool, first: bool) -> Task<Message> {
        let hits = self.search_hits();
        if hits.is_empty() {
            self.search_cursor = None;
            return self.reflow_transcript(None);
        }
        let current = self
            .search_cursor
            .and_then(|key| hits.iter().position(|candidate| *candidate == key));
        let index = if first {
            0
        } else if backwards {
            current.map_or(hits.len() - 1, |index| {
                (index + hits.len() - 1) % hits.len()
            })
        } else {
            current.map_or(0, |index| (index + 1) % hits.len())
        };
        let key = hits[index];
        self.search_cursor = Some(key);
        if self.reply_parts(key).is_some_and(|reply| {
            conversation_search::contains(&reply.reasoning, self.active_search())
        }) {
            self.thoughts_expanded.insert(key);
        }
        let partial = self.transcript.partial().map(|text| DisplayRow {
            key: u64::MAX,
            role: "Assistant · live".into(),
            kind: "assistant/message".into(),
            chat: true,
            text,
            files: Vec::new(),
            references: Vec::new(),
            trigger: None,
            time: 0,
            surface: false,
        });
        let rows: Vec<_> = self
            .transcript
            .display_rows(self.records_expanded)
            .chain(partial.as_ref())
            .collect();
        let geometries = self.row_geometries(&rows);
        if let Some(index) = rows.iter().position(|row| row.key == key) {
            let width = (geometries[index].width - chat_geometry::BODY_INSET).max(1.0);
            let inside = if let Some(reply) = self.reply_parts(key) {
                reply.first_match_offset(
                    width,
                    self.thoughts_expanded.contains(&key),
                    self.active_search(),
                )
            } else {
                conversation_content::plain_first_match_offset(
                    &rows[index].text,
                    width,
                    design::BODY_SIZE + self.content_delta(),
                    design::body_line(self.content_delta()),
                    self.active_search(),
                )
            };
            self.offset = (geometries[..index]
                .iter()
                .map(|geometry| geometry.height)
                .sum::<f32>()
                + chat_geometry::body_top(&rows[index].role)
                + inside
                - 12.0)
                .max(0.0);
            self.follow_bottom = false;
        }
        self.invalidate_scroll();
        self.position_transcript()
    }
    fn search_bar(&self) -> Element<'_, Message> {
        let generation = self.generation;
        let serial = self.search_serial;
        let hits = self.search_hits();
        let current = self
            .search_cursor
            .and_then(|key| hits.iter().position(|candidate| *candidate == key))
            .map_or(0, |index| index + 1);
        container(
            row![
                text_input("Find in loaded messages…", &self.search_query)
                    .id("native-chat-search")
                    .on_input(move |query| Message::SearchInput {
                        generation,
                        serial,
                        query
                    })
                    .style(design::field)
                    .size(13)
                    .width(Length::Fill),
                label(format!("{current}/{} messages", hits.len()), MUTED).size(11),
                button(label("↑", MUTED))
                    .on_press_maybe((!hits.is_empty()).then_some(Message::SearchNext {
                        generation,
                        serial,
                        backwards: true
                    }))
                    .style(design::ghost),
                button(label("↓", MUTED))
                    .on_press_maybe((!hits.is_empty()).then_some(Message::SearchNext {
                        generation,
                        serial,
                        backwards: false
                    }))
                    .style(design::ghost),
                button(label("Close", MUTED).size(11))
                    .on_press(Message::SearchClose { generation, serial })
                    .style(design::ghost)
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 20])
        .into()
    }
    /// True only while this tool row is expanded and actually has a recorded body.
    fn activity_is_expanded(&self, row: &DisplayRow) -> bool {
        self.activity_expanded.contains(&row.key) && !activity_body_source(row).is_empty()
    }
    fn row_geometry(&self, row: &DisplayRow, lane: f32) -> chat_geometry::Geometry {
        if let Some(reply) = self.reply_parts(row.key) {
            // Formatted assistant replies share the content column, never wider
            // than the web's 920 cap (the lane never exceeds it).
            let width = (lane - 8.0).clamp(37.0, design::MAX_CHAT);
            let body_height = reply.body_height(
                width - chat_geometry::BODY_INSET,
                self.thoughts_expanded.contains(&row.key),
            );
            // An answerless reply keeps only its thoughts disclosure, so it has
            // no action row and must not reserve one.
            let actions = if reply.has_answer() {
                action_footer(self.content_delta())
            } else {
                0.0
            };
            return chat_geometry::Geometry {
                width,
                body_height,
                height: chat_geometry::chrome_height(&row.role) + body_height + actions,
            };
        }
        let mut geometry = self.chat_geometry.geometry(
            row.key,
            &row.role,
            &excerpt(&row.text),
            self.reference_summary_height(row, lane),
            lane,
        );
        if self.search_expands_user(row) {
            geometry.body_height = conversation_content::plain_height(
                &row.text,
                geometry.width - chat_geometry::BODY_INSET,
                design::BODY_SIZE + self.content_delta(),
                design::body_line(self.content_delta()),
                FONT,
                false,
            )
            .max(20.0);
            geometry.height = chat_geometry::chrome_height(&row.role) + geometry.body_height;
        }
        // A file-only user message draws no bubble (`MessageItem.tsx:187`), so
        // its cards are the whole body: neither the text measurement nor the
        // bubble's 20px of vertical padding is reserved. Chosen once here and
        // read by the height, the wrapper width and the view.
        let attachments = !row.files.is_empty();
        let has_bubble = !row.text.is_empty();
        if attachments && !has_bubble {
            geometry.body_height = 0.0;
            geometry.height = chat_geometry::ROW_CHROME;
        }
        if row.role == "You"
            || row.role.starts_with("Assistant")
            || row.role == "Interrupted assistant attempt"
        {
            geometry.height += action_footer(self.content_delta());
        }
        if attachments {
            // The cards stack above the bubble inside `.userStack`, so they add
            // to the row's height and not to the body the bubble sizes
            // (`MessageItem.module.css:10-21`).
            geometry.height += design::attachment_height(row.files.len(), has_bubble);
        }
        if self.activity_is_expanded(row) {
            // The compact line stays; the recorded body sits beneath it in the
            // same conversation row, so expanding never leaves this chat.
            geometry.body_height = self.chat_geometry.tool_line()
                + chat_geometry::TOOL_BODY_GAP
                + conversation_content::tool_body_height(
                    activity_body_source(row),
                    geometry.width - chat_geometry::BODY_INSET,
                    self.content_delta_secondary(),
                    design::body_line(self.content_delta()),
                );
            geometry.height = geometry.body_height + chat_geometry::ROW_CHROME;
        }
        geometry
    }
    fn row_geometries(&self, rows: &[&DisplayRow]) -> Vec<chat_geometry::Geometry> {
        // Match the real sidebar animation and centered padded scroll lane.
        let lane = self.chat_lane();
        let cache = &self.chat_geometry;
        // Keep the one cache-only live entry across durable anchor measurements.
        cache.retain_keys(
            rows.iter()
                .map(|row| row.key)
                .chain(std::iter::once(u64::MAX)),
        );
        self.reply_content.retain(
            rows.iter()
                .map(|row| row.key)
                .chain(std::iter::once(u64::MAX)),
        );
        rows.iter()
            .enumerate()
            .map(|(index, row)| {
                let mut geometry = self.row_geometry(row, lane);
                // A row's height arrives carrying its own 12px of symmetric
                // outer padding (`chat_geometry::ROW_CHROME`); the web instead
                // puts one leading margin on each row, from the flow ladder.
                // Reserve that gap here and draw it as the row's top padding,
                // so the two read the same value.
                geometry.height += flow_gap_before(rows, index) - chat_geometry::ROW_CHROME;
                geometry
            })
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
        let lane = self.chat_lane();
        let trailing = self.transcript.partial().map_or(0.0, |text| {
            self.row_geometry(
                &DisplayRow {
                    key: u64::MAX,
                    role: "Assistant · live".into(),
                    kind: "assistant/message".into(),
                    chat: true,
                    text,
                    files: Vec::new(),
                    references: Vec::new(),
                    trigger: None,
                    time: 0,
                    surface: false,
                },
                lane,
            )
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
            kind: "assistant/message".into(),
            chat: true,
            text,
            files: Vec::new(),
            references: Vec::new(),
            trigger: None,
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
            return if self.search_open {
                column![self.search_bar(), self.welcome(len)]
                    .height(Length::Fill)
                    .into()
            } else {
                self.welcome(len)
            };
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
                } else if verb == "Context elided" || verb == "Context summarized" {
                    // Both of the paper's context-management mechanisms replace
                    // retained content, so they share the replacement glyph.
                    "↳"
                } else if verb == "Loop guard" {
                    "⟳"
                } else {
                    "▤"
                };
                let target = activity.target.as_deref().unwrap_or("Details available");
                let target_limit = if self.layout_size().width < 700.0 {
                    28
                } else if self.compact() {
                    42
                } else {
                    80
                };
                // The web gives tool-row file links a grey dotted affordance;
                // Iced draws the same resting affordance as a solid MUTED
                // underline. Commands and generic labels stay plain.
                let target_element: Element<'_, Message> =
                    if matches!(verb, "Read" | "Edit" | "Write") {
                        rich_text([span::<(), iced::Font>(short(target, target_limit))
                            .color(MUTED)
                            .underline(true)])
                        .font(FONT)
                        .size(12)
                        .width(Length::Fill)
                        .into()
                    } else {
                        label(short(target, target_limit), MUTED)
                            .size(12)
                            .width(Length::Fill)
                            .into()
                    };
                // The disclosure lives in the leading box, as the web's
                // `DisclosureRow` does: the chevron when the row is open, the
                // chevron on hover for a row that can open, and the tool's own
                // variant glyph at rest. A fixed box keeps the verb from shifting
                // when the glyph swaps.
                let activity_body = activity_body_source(record);
                let expandable = !activity_body.is_empty();
                let expanded = self.activity_is_expanded(record);
                let hovered = self.activity_hovered(record.key);
                let leading = if expandable && (expanded || hovered) {
                    if expanded { "▾" } else { "▸" }
                } else {
                    icon
                };
                let mut line = row![
                    container(label(leading, MUTED).size(12))
                        .width(design::LEADING_BOX + self.content_delta())
                        .align_x(iced::Alignment::Center),
                    label(short(verb, 24), MUTED).size(12),
                    label("·", MUTED).size(12),
                    target_element
                ]
                .spacing(7)
                .align_y(iced::Alignment::Center);
                // The web's diff totals ride after the summary: 10px to their
                // left, the code font at `secondary - 2` = 11px, an ASCII hyphen,
                // and the success/error pair only while the row is hovered or
                // expanded — at rest they stay muted. A failed call drops them
                // (`ToolRow.tsx:185-190, 248-250`, `ToolRow.module.css:64-77`).
                if let Some((added, removed)) = activity.edits
                    && !activity.failed
                {
                    let cue = hovered || expanded;
                    line = line.push(
                        container(
                            row![
                                label(format!("+{added}"), if cue { design::FOAM } else { MUTED })
                                    .size(design::DIFF_STAT_SIZE)
                                    .font(MONO),
                                // The web writes a literal space between the two
                                // counts; at this size that is the same width as
                                // this gap.
                                label(format!("-{removed}"), if cue { DANGER } else { MUTED })
                                    .size(design::DIFF_STAT_SIZE)
                                    .font(MONO),
                            ]
                            .spacing(3),
                        )
                        .padding(iced::Padding {
                            top: 0.0,
                            right: 0.0,
                            bottom: 0.0,
                            left: design::DIFF_STAT_LEFT,
                        }),
                    );
                }
                // Suffix stays outside the clipped label; output cannot look like a new call.
                if activity.failed {
                    line = line.push(label("Recorded error", DANGER).size(10));
                } else if record.role == "Tool result" {
                    line = line.push(label("Output", MUTED).size(10));
                }
                // The recorded body expands in place; the leading box above is the
                // only affordance, so a row with nothing recorded keeps the plain
                // variant glyph and starts no request.
                let compact = iced::widget::MouseArea::new(
                    button(
                        container(line)
                            .height(self.chat_geometry.tool_line())
                            .clip(true),
                    )
                    .on_press_maybe((expandable && self.composer_visible()).then_some(
                        Message::ToggleActivity {
                            generation: self.generation,
                            key: record.key,
                        },
                    ))
                    .style(design::ghost)
                    .padding([0, 8])
                    .width(Length::Fill),
                )
                // `MouseArea` hands the event to its content first and returns when
                // the button captured it, so the press still toggles the row.
                .on_enter(Message::ActivityHover(record.key))
                .on_exit(Message::ActivityHoverEnd(record.key));
                let mut activity_column = Column::new().push(compact);
                if expanded {
                    // Measure and render at the inset the geometry reserved, and
                    // insert the gap constant it reserved for. Rendering at a
                    // narrower inset than the reservation made the two disagree.
                    let body_width = geometries[index].width - chat_geometry::BODY_INSET;
                    activity_column = activity_column
                        .push(Space::new().height(chat_geometry::TOOL_BODY_GAP))
                        .push(conversation_content::tool_body_view(
                            activity_body,
                            conversation_content::tool_body_height(
                                activity_body,
                                body_width,
                                self.content_delta_secondary(),
                                design::body_line(self.content_delta()),
                            ),
                            self.content_delta_secondary(),
                            design::body_line(self.content_delta()),
                        ));
                }
                activity_column.into()
            } else if let Some(trigger) = record.trigger {
                // The reference's turn-trigger notice (`TurnTriggerNodeView.tsx`): an
                // unboxed, expandable line naming the non-human producer, above the turn
                // it began. Its body is the recorded notification, not model output.
                let expanded = self.trigger_expanded.contains(&record.key);
                let clock = local_hh_mm(record.time);
                let mut header = row![
                    label(if expanded { "▾" } else { "▸" }, MUTED).size(10),
                    label(trigger.title, MUTED).size(11),
                    Space::new().width(Length::Fill),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center);
                if let Some(clock) = clock {
                    header = header.push(label(clock, MUTED).size(10));
                }
                let mut notice = column![
                    button(header)
                        .on_press(Message::ToggleTrigger(record.key))
                        .style(design::ghost)
                        .padding([4, 0])
                        .width(Length::Fill),
                ]
                .spacing(2);
                if expanded {
                    notice = notice
                        .push(label("This notification triggered this response.", MUTED).size(10))
                        .push(label(short(&record.text, 600), TEXT).size(12));
                }
                container(notice)
                    .padding([8, 14])
                    .width(geometries[index].width)
                    .style(|_| panel(BASE))
                    .into()
            } else if human || assistant {
                let geometry = geometries[index];
                let reply = self.reply_parts(record.key);
                let answerless =
                    assistant && reply.as_ref().is_some_and(|reply| !reply.has_answer());
                if let (true, Some(reply)) = (answerless, reply) {
                    // Thinking or a recorded tool call with no answer keeps its
                    // disclosure alone: the empty body and the Copy/Details row
                    // used to be left behind underneath it.
                    container(reply.view_search(
                        geometry.width - chat_geometry::BODY_INSET,
                        self.thoughts_expanded.contains(&record.key),
                        self.active_search(),
                    ))
                    .padding([10, 14])
                    .width(geometry.width)
                    .style(|_| panel(BASE))
                    .into()
                } else {
                    let body: Element<'_, Message> =
                        if let Some(reply) = self.reply_parts(record.key) {
                            reply.view_search(
                                geometry.width - chat_geometry::BODY_INSET,
                                self.thoughts_expanded.contains(&record.key),
                                self.active_search(),
                            )
                        } else {
                            let source = if self.search_expands_user(record) {
                                record.text.clone()
                            } else {
                                excerpt(&record.text)
                            };
                            conversation_content::plain_view(
                                &source,
                                self.active_search(),
                                (design::BODY_SIZE + self.content_delta()).round() as u32,
                                design::body_line(self.content_delta()),
                                TEXT,
                            )
                        };
                    // The assistant's completion row gives its glyph two extra
                    // pixels, as the reference's data-clock='end' row does.
                    let glyph = if human {
                        ACTION_GLYPH
                    } else {
                        ACTION_GLYPH_END
                    };
                    let mut footer = row![]
                        .spacing(ACTION_SPACING)
                        .height(ACTION_HEIGHT + self.content_delta());
                    // Only a user message carries the clock, as the web's
                    // `.timeStart` does; the assistant tail keeps `.timeEnd`.
                    if human && let Some(clock) = local_hh_mm(record.time) {
                        footer =
                            footer.push(
                                container(label(clock, MUTED).size(
                                    design::content_secondary(self.content_font_size).round()
                                        as u32,
                                ))
                                .padding(iced::Padding {
                                    top: 0.0,
                                    right: 12.0,
                                    bottom: 0.0,
                                    left: 0.0,
                                }),
                            );
                    }
                    // The reference's turn tail carries a branch control beside copy
                    // (`TurnTailNodeView.tsx:69` forks at `data.seq`). Native's reply
                    // record key is its inclusive cut.
                    if !human && let Ok(cut) = i64::try_from(record.key) {
                        let fork = self.fork_context();
                        if let Some(view) = fork.view.filter(|_| fork.allowed) {
                            footer = footer.push(
                                button(label("Branch", MUTED).size(10))
                                    .on_press(Message::Fork(crate::fork::Action::Open {
                                        view,
                                        cut: Some(cut),
                                    }))
                                    .style(design::ghost)
                                    .padding([1, 2]),
                            );
                        }
                    }
                    footer = footer.push(
                        button(label(ACTION_COPY, MUTED).size(glyph))
                            .on_press_maybe(self.row_copy_source(record.key).is_some().then_some(
                                Message::CopyReply {
                                    generation: self.generation,
                                    key: record.key,
                                },
                            ))
                            .style(design::ghost)
                            .padding([ACTION_PAD, ACTION_PAD]),
                    );
                    if human && self.transcript.editable_text(record.key).is_some() {
                        footer = footer.push(
                            button(label(ACTION_EDIT, MUTED).size(ACTION_GLYPH))
                                .on_press_maybe(
                                    self.edit_message_allowed(self.generation, record.key)
                                        .then_some(Message::EditReply {
                                            generation: self.generation,
                                            key: record.key,
                                        }),
                                )
                                .style(design::ghost)
                                .padding([ACTION_PAD, ACTION_PAD]),
                        );
                    }
                    if assistant || excerpt(&record.text) != record.text {
                        footer = footer.push(
                            button(
                                label(
                                    if human {
                                        "Read more ▸"
                                    } else {
                                        "Details ▸"
                                    },
                                    MUTED,
                                )
                                .size(10),
                            )
                            .on_press_maybe(self.detail_request(record.key))
                            .style(design::ghost)
                            .padding([1, 2]),
                        );
                    }
                    // The assistant tail closes with the clock, as the reference's
                    // `.timeEnd` does: one pixel under the secondary size, with an
                    // 8px left margin (`MessageIconActions.module.css:23-37`).
                    if !human && let Some(clock) = local_hh_mm(record.time) {
                        footer = footer.push(
                            container(
                                label(clock, MUTED).size(
                                    (design::content_secondary(self.content_font_size) - 1.0)
                                        .max(1.0)
                                        .round() as u32,
                                ),
                            )
                            .padding(iced::Padding {
                                top: 0.0,
                                right: 0.0,
                                bottom: 0.0,
                                left: 8.0,
                            }),
                        );
                    }
                    let mut card = Column::new();
                    if chat_geometry::status_header(&record.role) {
                        card = card
                            .push(
                                container(
                                    label(
                                        record.role.clone(),
                                        if record.role == "Interrupted assistant attempt" {
                                            DANGER
                                        } else {
                                            MUTED
                                        },
                                    )
                                    .size(12),
                                )
                                .height(24)
                                .clip(true),
                            )
                            .push(Space::new().height(7));
                    }
                    let body = container(body).height(geometry.body_height).clip(true);
                    if human {
                        // The web keeps the attachment row, the bubble and the
                        // action row as a right-aligned stack: `.userStack`
                        // holds the cards and the bubble with an 8px gap, and
                        // `.userRow` puts the actions 6px below it
                        // (`MessageItem.module.css`, `MessageItem.tsx:194-221`).
                        let mut stack = Column::new()
                            .spacing(design::ATTACHMENT_CARD_GAP)
                            .align_x(iced::Alignment::End);
                        if !record.files.is_empty() {
                            stack = stack.push(attachment_cards(record.files.clone()));
                        }
                        // A file-only message shows no bubble, exactly as the
                        // reference's `showBubble` decides.
                        if !record.text.is_empty() || record.files.is_empty() {
                            stack = stack.push(
                                container(card.push(body))
                                    .padding(design::USER_BUBBLE_PAD)
                                    .width(geometry.width)
                                    .style(|_| design::card(OVERLAY, design::USER_BUBBLE_RADIUS)),
                            );
                        }
                        // The reference's referenced-session summary: the labels the
                        // following `session-reference` record cited, one 13px/18px
                        // tertiary line under the bubble inside the same stack
                        // (`MessageItem.tsx:225-229`, `MessageItem.module.css:42-45`).
                        if !record.references.is_empty() {
                            stack = stack.push(
                                container(
                                    label(
                                        crate::reducer::reference_summary_text(&record.references),
                                        MUTED,
                                    )
                                    .size(13)
                                    .line_height(design::reference_line(
                                        self.content_delta_secondary(),
                                    ))
                                    .width(Length::Fill),
                                )
                                .id("native-reference-summary"),
                            );
                        }
                        column![stack, Space::new().height(ACTION_GAP), footer]
                            .align_x(iced::Alignment::End)
                            .into()
                    } else {
                        container(
                            card.push(body)
                                .push(Space::new().height(ACTION_GAP))
                                .push(footer),
                        )
                        .padding([10, 14])
                        .width(geometry.width)
                        .style(|_| panel(BASE))
                        .into()
                    }
                }
            } else {
                let card = column![
                    label(
                        if self.transcript.inbox_canceled(record.key) {
                            "Canceled queued input".into()
                        } else {
                            record.role.clone()
                        },
                        if self.transcript.inbox_canceled(record.key)
                            || self.transcript.turn_notice(record.key)
                        {
                            DANGER
                        } else if assistant {
                            design::FOAM
                        } else {
                            ACCENT
                        }
                    )
                    .size(12),
                    container(
                        label(
                            excerpt(&record.text),
                            if self.transcript.turn_notice(record.key) {
                                DANGER
                            } else {
                                TEXT
                            }
                        )
                        .size(15)
                    )
                    .height(75)
                    .clip(true),
                    button(label("Read more ▸", MUTED).size(10))
                        .on_press_maybe(self.detail_request(record.key))
                        .style(design::ghost)
                        .padding([3, 0])
                ]
                .spacing(7);
                container(card)
                    .padding([10, 14])
                    .width(Length::Fill)
                    .style(move |_| {
                        if human {
                            design::card(OVERLAY, design::RADIUS_LG)
                        } else if assistant {
                            panel(BASE)
                        } else {
                            design::card(SURFACE, design::RADIUS_MD)
                        }
                    })
                    .into()
            };
            // The row is as wide as its widest child. A bubble narrower than an
            // attachment card would otherwise clip the card; the bubble keeps
            // its own measured width and both align right inside the wider row,
            // as `.userStack { align-items: flex-end }` does
            // (`MessageItem.module.css:10-21`).
            let width = if human && !record.files.is_empty() {
                geometries[index].width.max(design::ATTACHMENT_CARD_WIDTH)
            } else {
                geometries[index].width
            };
            let bubble = container(bubble).width(width);
            // View, spacers and anchors share the same projected rows and heights;
            // diagnostics remain retained for the Session records disclosure.
            // The leading gap is the web's flow margin, drawn as this row's top
            // padding and reserved by `row_geometries` through the same helper.
            let outer = container(bubble)
                .align_x(if human {
                    iced::alignment::Horizontal::Right
                } else {
                    iced::alignment::Horizontal::Left
                })
                .padding([flow_gap_before(&rendered, index), 4.0])
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
            button(label("Reload", MUTED).size(11))
                .on_press_maybe((self.ready && self.selected.is_some()).then_some(Message::Reload))
                .style(design::ghost),
            button(label("Find", MUTED).size(11))
                .on_press(Message::SearchOpen)
                .style(design::ghost),
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
            .style(design::ghost)
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);
        let generation = self.generation;
        let revision = self.scroll_revision;
        // The reference centers its history control at the top of the column:
        // `.older { display: flex; justify-content: center }` around a button of
        // `padding: 4px 12px`, `radius-sm`, `12px` secondary text on the
        // interactive hover-solid fill, disabled while a page is in flight
        // (`ChatView.module.css:201-218`, `ChatView.tsx:249-256`).
        let mut lane_body = Column::new().width(Length::Fill);
        if self.transcript.has_more() {
            // The scroller's own `[16, 32]` padding supplies the edge inset, so
            // the control adds none of its own.
            lane_body = lane_body.push(
                container(
                    container(
                        Button::new(
                            label(
                                if self.page_pending {
                                    "Loading…"
                                } else {
                                    "Load earlier"
                                },
                                MUTED,
                            )
                            .size(12),
                        )
                        .on_press_maybe(
                            (self.ready && self.follow_ready && !self.page_pending)
                                .then_some(Message::Older),
                        )
                        .style(design::fill_solid)
                        .padding([4.0, 12.0]),
                    )
                    .id("native-load-older"),
                )
                .width(Length::Fill)
                .center_x(Length::Fill),
            );
        }
        lane_body = lane_body.push(contents);
        let lane = container(lane_body)
            .max_width(self.chat_lane())
            .width(Length::Fill);
        let mut chat = column![container(controls).padding([2, 20])];
        if self.search_open {
            chat = chat.push(self.search_bar());
        }
        // The web's `.scroll` carries a 16px vertical inset of its own
        // (`ChatView.module.css:26-29`): the rows below carry leading margins
        // only, so the first row's and last row's clearance lives here.
        let scroll = scrollable(
            container(lane)
                .center_x(Length::Fill)
                .padding([design::FLOW_INSET, design::TRANSCRIPT_INSET]),
        )
        .direction(design::scrollbar())
        .style(design::scrollable_style)
        .id(self.scroll_id())
        .on_scroll(move |viewport| Message::Scrolled {
            generation,
            revision,
            viewport,
        })
        .height(Length::Fill);
        // Both the width handles and the composer fade overlay the transcript
        // instead of taking layout space, so the lane and the scroll model are
        // untouched. The handles exist only when the pane can hold both, which is
        // the web's own rule; the fade belongs to the active phase, which is this
        // branch, and to a composer that is actually on screen.
        let lane_width = self.chat_lane();
        let room =
            self.content_column() >= lane_width + 2.0 * (WIDTH_HANDLE_OFFSET + WIDTH_HANDLE_WIDTH);
        let mut overlaid = iced::widget::Stack::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .push(scroll);
        if room {
            overlaid = overlaid.push(self.width_handles(lane_width));
        }
        if self.composer_visible() {
            overlaid = overlaid.push(self.composer_fade());
        }
        if let Some(control) = self.to_bottom_control() {
            overlaid = overlaid.push(control);
        }
        chat.push(overlaid).height(Length::Fill).into()
    }
    /// The reference's jump-to-latest control: a 34px round floating button in a
    /// slot 16px above the composer whose right edge rides the content column's
    /// right edge, or 32px in from the pane when the column is narrower
    /// (`ChatView.module.css:223-273`). It exists only while the transcript is
    /// not following the live tail — the same flag the reader anchor uses.
    fn to_bottom_control(&self) -> Option<Element<'_, Message>> {
        if self.follow_bottom
            || self
                .transcript
                .display_rows(self.records_expanded)
                .next()
                .is_none()
        {
            return None;
        }
        let lane = self.chat_lane();
        let inset = ((self.content_column() - lane) / 2.0).max(design::TO_BOTTOM_EDGE);
        let button = container(
            button(label("↓", TEXT).size(16))
                .on_press(Message::ScrollToBottom)
                .style(|theme, status| {
                    let mut style = design::ghost(theme, status);
                    // The reference hovers one rung above its floating fill; this
                    // palette's next rung over OVERLAY is LINE (Rosé Pine
                    // highlight-med), the same pair the rest of the shell uses.
                    style.background = Some(
                        if matches!(status, button::Status::Hovered) {
                            design::LINE
                        } else {
                            OVERLAY
                        }
                        .into(),
                    );
                    style.border.radius = (design::TO_BOTTOM_SIZE / 2.0).into();
                    style
                })
                .padding([0.0, 0.0])
                .width(design::TO_BOTTOM_SIZE)
                .height(design::TO_BOTTOM_SIZE),
        )
        .id("native-to-bottom");
        Some(
            container(button)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::Alignment::End)
                .align_y(iced::Alignment::End)
                .padding(iced::Padding {
                    right: inset,
                    bottom: design::TO_BOTTOM_INSET,
                    ..iced::Padding::ZERO
                })
                .into(),
        )
    }
    /// The hero's workspace chip row. The reference seats it between the
    /// headline and the composer card, left-aligned inside the card's width with
    /// `padding: 0 16px 0 20px` and a `4px` top margin
    /// (`ConversationRoot.module.css:520-529`), and the chip itself is a 28px
    /// transparent control — a 16px folder glyph, the workspace title (or the
    /// `Choose workspace` string), and a 12px chevron — that opens the workspace
    /// picker (`HeroShell.module.css:152-186`, `EmptyHero.tsx:38-62`). Native
    /// opens its sidebar workspace list instead of a popup, and DejaVu carries no
    /// folder outline, so the glyph is the house mark it does carry.
    fn hero_workspace_row(&self) -> Element<'_, Message> {
        let selected = self.workspace.as_ref().and_then(|id| {
            self.workspaces
                .iter()
                .find(|workspace| &workspace.workspace_id == id)
        });
        let label_text = selected
            .map(|workspace| short(&workspace.title, 34))
            .unwrap_or_else(|| "Choose workspace".into());
        let chip = button(
            row![
                container(label("⌂", TEXT).size(13))
                    .width(design::HERO_CHIP_GLYPH)
                    .align_x(iced::Alignment::Center),
                label(label_text, TEXT)
                    .size(13)
                    .line_height(iced::Pixels(design::CRUMB_LINE)),
                label("⌄", MUTED).size(12).width(design::HERO_CHIP_CHEVRON),
            ]
            .spacing(design::HERO_CHIP_GAP)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::WorkspaceControls)
        .style(design::ghost)
        .padding(design::HERO_CHIP_PAD)
        .height(design::COMPOSER_CHIP_HEIGHT);
        let chip = container(chip).id("native-hero-workspace");
        // `conversation.hero.agentPreset` (`ui-agent-preset/src/client/index.ts:210-214`): the
        // new-session preset, which a session-less window cannot read from a projection. The
        // press is also the read's trigger, because the app queues no read without a user
        // action (the suite enforces that), and the selection list follows the roster's own
        // default.
        let preset = self.agent_roster.as_ref().and_then(|roster| {
            roster
                .presets
                .iter()
                .find(|row| row.is_default)
                .map(|row| row.name.clone().unwrap_or_else(|| row.id.clone()))
        });
        let preset_chip = button(
            row![
                container(label("◈", MUTED).size(13))
                    .width(design::HERO_CHIP_GLYPH)
                    .align_x(iced::Alignment::Center),
                label(
                    self.agent_seat_error
                        .clone()
                        .unwrap_or_else(|| preset.unwrap_or_else(|| "Preset".to_owned())),
                    if self.agent_seat_error.is_some() {
                        DANGER
                    } else {
                        MUTED
                    },
                )
                .size(13)
                .line_height(iced::Pixels(design::CRUMB_LINE)),
                label("⌄", MUTED).size(12).width(design::HERO_CHIP_CHEVRON),
            ]
            .spacing(design::HERO_CHIP_GAP)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::AgentPresetSeat)
        .style(design::ghost)
        .padding(design::HERO_CHIP_PAD)
        .height(design::COMPOSER_CHIP_HEIGHT);
        let preset_chip = if self.agent_seat_open {
            let mut items = Column::new().spacing(2);
            for row in self
                .agent_roster
                .as_ref()
                .map(|roster| roster.presets.as_slice())
                .unwrap_or_default()
            {
                let name = row.name.clone().unwrap_or_else(|| row.id.clone());
                let ink = match (row.broken.is_some(), row.is_default) {
                    // A declared preset whose activation failed stays visible but unchoosable.
                    (true, _) => DANGER,
                    (false, true) => TEXT,
                    (false, false) => MUTED,
                };
                items = items.push(
                    button(
                        label(name, ink)
                            .size(13)
                            .wrapping(iced::advanced::text::Wrapping::None),
                    )
                    .on_press_maybe(
                        row.broken
                            .is_none()
                            .then(|| Message::AgentPresetPick(row.id.clone())),
                    )
                    .style(design::ghost)
                    .padding([4, 8])
                    .width(Length::Fill),
                );
            }
            container(anchored_popup::anchored(
                preset_chip.into(),
                Some(
                    container(items)
                        .id("native-hero-preset-menu")
                        .width(220)
                        .padding(6)
                        .style(|_| design::card(SURFACE, 10.0))
                        .into(),
                ),
                Message::AgentPresetSeat,
            ))
            .id("native-hero-preset")
        } else {
            container(preset_chip).id("native-hero-preset")
        };
        container(
            container(
                row![chip, preset_chip]
                    .spacing(design::HERO_CHIP_GAP)
                    .align_y(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .padding(design::HERO_ROW_PAD),
        )
        .max_width(self.chat_cap() + design::COMPOSER_CARD_OVERHANG)
        .width(Length::Fill)
        .padding(iced::Padding {
            top: design::HERO_ROW_TOP,
            ..iced::Padding::ZERO
        })
        .into()
    }
    /// The centered empty-state body used where no composer follows (the
    /// sidebar sheet); with a composer the hero and the card share `hero_pane`.
    fn welcome(&self, records: usize) -> Element<'_, Message> {
        scrollable(
            container(self.hero_strip(records))
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .direction(design::scrollbar())
        .style(design::scrollable_style)
        .height(Length::Fill)
        .into()
    }
    fn hero_strip(&self, records: usize) -> Element<'_, Message> {
        let selected = self.selected.is_some();
        // The reference's headline is the brand mark beside the title, one
        // centered row at a 10px gap, `26/32` weight 500
        // (`HeroShell.module.css:33-60`), and its copy is the hero headline
        // string (`locales.ts:453`). A session-less native window has no
        // counterpart state, so it keeps its own line and the guidance the
        // blocked composer needs.
        let mut hero = column![
            row![
                brand(),
                label(
                    if selected {
                        "Into the Unknown"
                    } else {
                        "A quiet space for your work"
                    },
                    TEXT
                )
                .size(design::HERO_TITLE_SIZE)
                .line_height(iced::Pixels(design::HERO_TITLE_LINE)),
            ]
            .spacing(design::HERO_HEADLINE_GAP)
            .align_y(iced::Alignment::Center)
        ]
        .spacing(18)
        .align_x(iced::Alignment::Center);
        if !selected {
            hero =
                hero.push(label("Choose or create a conversation in the sidebar.", MUTED).size(14));
        }
        if selected && self.transcript.has_more() {
            hero = hero.push(
                row![
                    button(
                        label(
                            if self.page_pending {
                                "Loading older messages…"
                            } else {
                                "Older messages"
                            },
                            MUTED
                        )
                        .size(12)
                    )
                    .on_press_maybe(
                        (self.ready && self.follow_ready && !self.page_pending)
                            .then_some(Message::Older)
                    )
                    .style(design::ghost),
                    button(label("Reload", MUTED).size(12))
                        .on_press_maybe(self.ready.then_some(Message::Reload))
                        .style(design::ghost),
                ]
                .spacing(8),
            );
        }
        if records > 0 {
            hero = hero.push(
                button(label(format!("Trajectory · {records} recorded events"), MUTED).size(12))
                    .on_press(Message::ToggleRecords)
                    .style(design::ghost),
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
        container(hero)
            .padding(30)
            .max_width(620)
            .width(Length::Fill)
            .into()
    }
    /// The empty-state pane: the hero strip and the composer card as one
    /// vertically centered stack, which is how the reference lays its hero out
    /// (`ConversationContent.tsx:164-171`, `HeroShell.module.css:5-21`).
    fn hero_pane(&self) -> Element<'_, Message> {
        let records = self.transcript.display_rows(true).count();
        container(
            column![
                self.hero_strip(records),
                self.hero_workspace_row(),
                self.composer()
            ]
            .spacing(design::HERO_GAP)
            .align_x(iced::Alignment::Center)
            .width(Length::Fill),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
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
                .style(design::ghost)
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
                    details = details.push(label(
                        format!("Conversation: {}", self.session_title(session)),
                        TEXT,
                    ));
                }
                if let Some(model) = self.model_selections.get(id) {
                    details = details.push(label(
                        format!("Session model: {} / {}", model.provider, model.model),
                        TEXT,
                    ));
                }
            }
            if !self.transcript.projections().is_empty() {
                details = details
                    .push(label("Read-only session projections", MUTED).size(13))
                    .push(label(
                        reducer::safe_json(
                            &serde_json::to_value(self.transcript.projections())
                                .unwrap_or_default(),
                        ),
                        TEXT,
                    ));
            }
        }
        details = details.push(
            button(label("Back to conversation", TEXT))
                .on_press(Message::Capabilities)
                .style(design::ghost),
        );
        scrollable(
            container(
                container(details)
                    .max_width(self.chat_lane())
                    .width(Length::Fill),
            )
            .center_x(Length::Fill)
            .padding(24),
        )
        .direction(design::scrollbar())
        .style(design::scrollable_style)
        .height(Length::Fill)
        .into()
    }
    fn work_panel_view(&self) -> Element<'_, Message> {
        let work = self.transcript.work().clone();
        let mut details = column![
            label("Work records", TEXT).size(22),
            label(
                "Read-only view of Goals, Todos, Subagents and Schedules recorded in this conversation. Nothing here can be changed, cancelled, retried or rescheduled.",
                MUTED
            )
            .size(11),
            label(
                format!(
                    "Keeping the newest {} records per section, {} in total; {} older record(s) omitted.",
                    crate::work_panel::SECTION_CAP,
                    crate::work_panel::TOTAL_CAP,
                    work.dropped_total()
                ),
                MUTED
            )
            .size(11),
        ]
        .spacing(12);
        if work.is_empty() {
            details = details.push(
                label(
                    "No goal, todo, subagent or schedule records in this conversation.",
                    MUTED,
                )
                .size(12),
            );
        }
        for section in crate::work_panel::Section::ALL {
            let entries = work.entries(section);
            if entries.is_empty() {
                continue;
            }
            details = details.push(label(section.name(), TEXT).size(14));
            if work.dropped(section) > 0 {
                details = details.push(
                    label(
                        format!("{} older records omitted", work.dropped(section)),
                        MUTED,
                    )
                    .size(10),
                );
            }
            for entry in entries {
                let mut lines = entry.lines.iter();
                if let Some(first) = lines.next() {
                    details = details.push(label(first.clone(), TEXT).size(12));
                }
                for line in lines {
                    details = details.push(
                        container(label(line.clone(), MUTED).size(11))
                            .padding([0, 10])
                            .width(Length::Fill),
                    );
                }
            }
        }
        details = details.push(
            button(label("Back to conversation", TEXT))
                .on_press(Message::Work(work_panel_ui::Action::Close))
                .style(design::ghost),
        );
        scrollable(
            container(
                container(details)
                    .max_width(self.chat_lane())
                    .width(Length::Fill),
            )
            .center_x(Length::Fill)
            .padding(24),
        )
        .direction(design::scrollbar())
        .style(design::scrollable_style)
        .height(Length::Fill)
        .into()
    }
    /// Client-only terminal panel. It renders exactly what the Host confirmed: the
    /// list, shell profiles and limits from read ACKs, the ACK-created terminal, and
    /// bounded frames from the owned follow. Opening it spawns no local shell.
    fn terminal_view(&self) -> Element<'_, Message> {
        use crate::terminal::{Action, CloseReview, DISPLAY_BYTES, Notice, Review};
        let ticket = self.terminal.ticket().expect("visible terminal panel");
        let context = self.terminal_context();
        let current = context.view.as_ref().is_some_and(|view| {
            view.epoch == ticket.epoch
                && view.generation == ticket.generation
                && view.target == ticket.target
        });
        let enabled = context.allowed && current && !self.terminal.busy();
        let mut body = column![
            row![
                label("Terminal", TEXT).size(22).width(Length::Fill),
                button("Close")
                    .style(design::ghost)
                    .on_press(Message::Terminal(Action::Dismiss(ticket.clone())))
            ]
            .spacing(10),
            label(
                "The shell runs on the Host with the Host's authority. This app is a client: it never spawns a shell or child process. Input is sent as-is, without interpretation; acceptance is not execution confirmation.",
                MUTED
            )
            .size(11),
            label(
                format!(
                    "Output is bounded to {} chunks / {} KiB; discarded output is counted, never silently hidden. The panel renders only the newest {} KiB.",
                    crate::terminal::OUTPUT_CHUNKS,
                    crate::terminal::OUTPUT_BYTES / 1024,
                    DISPLAY_BYTES / 1024
                ),
                MUTED
            )
            .size(11),
        ]
        .spacing(10);
        match self.terminal.environment() {
            Some(environment) => {
                body = body.push(
                    label(
                        format!(
                            "Host limits: input ≤ {} UTF-8 bytes · {}×{} max · scrollback {} rows · cwd {}",
                            environment.max_input_bytes,
                            environment.max_cols,
                            environment.max_rows,
                            environment.scrollback,
                            short(&environment.cwd, 60)
                        ),
                        MUTED,
                    )
                    .size(10),
                );
            }
            None => {
                body = body.push(
                    label(
                        "Host limits unavailable: this Session has no registered Agent. History is listed below, but create, write and close cannot run.",
                        MUTED,
                    )
                    .size(10),
                );
            }
        }
        body = body.push(label("SHELL", MUTED).size(10));
        if self.terminal.shells().is_empty() {
            body = body.push(
                label(
                    "No shell profile was discovered; create uses the Host default shell.",
                    MUTED,
                )
                .size(11),
            );
        } else {
            let mut choices = row![].spacing(6);
            for (index, profile) in self.terminal.shells().iter().enumerate() {
                let chosen = self
                    .terminal
                    .chosen_shell()
                    .is_some_and(|shell| shell.path == profile.path);
                let choose = ticket.clone();
                choices = choices.push(
                    button(
                        label(
                            format!(
                                "{}{}",
                                if chosen { "● " } else { "" },
                                short(&profile.name, 24)
                            ),
                            if chosen { ACCENT } else { MUTED },
                        )
                        .size(11),
                    )
                    .on_press_maybe(enabled.then_some(Message::Terminal(Action::ChooseShell {
                        ticket: choose,
                        index,
                    })))
                    .style(design::ghost)
                    .padding([6, 8]),
                );
            }
            body = body.push(choices);
        }
        if self.terminal.review_create() {
            let shell = self
                .terminal
                .chosen_shell()
                .map(|shell| short(&shell.name, 24).to_owned())
                .unwrap_or_else(|| "Host default".into());
            body = body
                .push(
                    label(
                        format!(
                            "Confirm create: shell {} · {}×{}. The Host allocates and owns the process.",
                            shell,
                            crate::terminal::DEFAULT_COLS,
                            crate::terminal::DEFAULT_ROWS
                        ),
                        ACCENT,
                    )
                    .size(12),
                )
                .push(
                    row![
                        button("Back")
                            .style(design::ghost)
                            .on_press_maybe(
                                enabled.then_some(Message::Terminal(Action::BackCreate(
                                    ticket.clone()
                                )))
                            ),
                        button("Confirm create")
                            .style(design::primary)
                            .on_press_maybe(
                                enabled.then_some(Message::Terminal(Action::ConfirmCreate(
                                    Review {
                                        ticket: ticket.clone()
                                    }
                                )))
                            ),
                    ]
                    .spacing(8),
                );
        } else {
            body = body.push(
                button("New terminal…")
                    .style(design::primary)
                    .on_press_maybe(
                        enabled.then_some(Message::Terminal(Action::ReviewCreate(ticket.clone()))),
                    ),
            );
        }
        body = body.push(
            row![
                label("TERMINALS", MUTED).size(10).width(Length::Fill),
                button("Refresh").style(design::ghost).on_press_maybe(
                    enabled.then_some(Message::Terminal(Action::Refresh(ticket.clone())))
                ),
            ]
            .spacing(8),
        );
        if !self.terminal.list_loaded() {
            body = body.push(label("Loading the Host terminal list…", MUTED).size(11));
        } else if self.terminal.list().is_empty() {
            body = body.push(label("No Host terminals exist for this Session.", MUTED).size(11));
        }
        for info in self.terminal.list() {
            let selected = self.terminal.selected() == Some(&info.id);
            let select = ticket.clone();
            let close = ticket.clone();
            let state = match info.state {
                dsh_native_transport::terminal::TerminalState::Running => "running",
                dsh_native_transport::terminal::TerminalState::Exited => "exited",
                dsh_native_transport::terminal::TerminalState::Failed => "failed",
            };
            body = body.push(
                row![
                    button(
                        label(
                            format!(
                                "{} {}",
                                if selected { "▸" } else { " " },
                                short(&info.title, 24)
                            ),
                            if selected { ACCENT } else { TEXT },
                        )
                        .size(11)
                    )
                    .on_press_maybe(enabled.then_some(Message::Terminal(Action::Select {
                        ticket: select,
                        id: info.id.clone()
                    })))
                    .style(design::ghost)
                    .padding([6, 8]),
                    label(
                        format!(
                            "{} · {}×{} · {}",
                            short(info.id.as_str(), 18),
                            info.cols,
                            info.rows,
                            state
                        ),
                        MUTED,
                    )
                    .size(10)
                    .width(Length::Fill),
                    button("Close…")
                        .on_press_maybe(enabled.then_some(Message::Terminal(Action::ReviewClose {
                            ticket: close,
                            id: info.id.clone()
                        })))
                        .style(design::danger)
                        .padding([6, 8]),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            );
        }
        if let Some(id) = self.terminal.review_close() {
            body = body
                .push(
                    label(
                        format!(
                            "Confirm close of {}: the Host terminates the whole process range. Repeated closes succeed; a failure leaves the terminal for explicit retry.",
                            short(id.as_str(), 24)
                        ),
                        DANGER,
                    )
                    .size(11),
                )
                .push(
                    row![
                        button("Back")
                            .style(design::ghost)
                            .on_press_maybe(
                                enabled.then_some(Message::Terminal(Action::BackClose(
                                    ticket.clone()
                                )))
                            ),
                        button("Confirm close")
                            .style(design::danger)
                            .on_press_maybe(
                                enabled.then_some(Message::Terminal(Action::ConfirmClose(
                                    CloseReview {
                                        ticket: ticket.clone(),
                                        id: id.clone()
                                    }
                                )))
                            ),
                    ]
                    .spacing(8),
                );
        }
        body = body.push(label("OUTPUT", MUTED).size(10));
        if self.terminal.selected().is_none() {
            body = body.push(
                label(
                    "Select a listed terminal to attach an input controller and stream its bounded output.",
                    MUTED,
                )
                .size(11),
            );
        } else {
            if self.terminal.output().dropped() > 0 {
                body = body.push(
                    label(
                        format!(
                            "{} older output chunk(s) were discarded by the panel bound.",
                            self.terminal.output().dropped()
                        ),
                        MUTED,
                    )
                    .size(10),
                );
            }
            let text = self.terminal.output().tail(DISPLAY_BYTES);
            body = body.push(
                container(
                    scrollable(
                        container(
                            label(
                                if text.is_empty() {
                                    "(no output yet)".to_owned()
                                } else {
                                    text
                                },
                                TEXT,
                            )
                            .size(11),
                        )
                        .width(Length::Fill),
                    )
                    .direction(design::scrollbar())
                    .style(design::scrollable_style),
                )
                .height(Length::Fixed(240.0))
                .width(Length::Fill)
                .padding(8)
                .style(|_| design::card(OVERLAY, design::RADIUS_MD)),
            );
            let write_enabled = enabled
                && self.terminal.environment().is_some()
                && !self.terminal.write_pending()
                && !self.terminal.input().is_empty()
                && self.terminal.input().len() as u64 <= self.terminal.max_input_bytes();
            let edit = ticket.clone();
            body = body
                .push(
                    text_input("Type input, then Write", self.terminal.input())
                        .id("native-terminal-input")
                        .on_input_maybe(enabled.then_some(move |text| {
                            Message::Terminal(Action::EditInput {
                                ticket: edit.clone(),
                                text,
                            })
                        }))
                        .style(design::field)
                        .padding(8)
                        .size(12),
                )
                .push(
                    row![
                        button("Write").style(design::primary).on_press_maybe(
                            write_enabled
                                .then_some(Message::Terminal(Action::Write(ticket.clone())))
                        ),
                        label(
                            format!(
                                "{} / {} UTF-8 bytes",
                                self.terminal.input().len(),
                                self.terminal.max_input_bytes()
                            ),
                            MUTED,
                        )
                        .size(10)
                        .width(Length::Fill),
                    ]
                    .spacing(8),
                );
        }
        body = body.push(match self.terminal.notice() {
            Notice::None => label("", MUTED).size(1),
            Notice::Loading => label("Reading the Host terminal list, shells and limits…", MUTED).size(11),
            Notice::ReadFailed => label(
                "The Host terminal list could not be read. Nothing was created; reconcile explicitly.",
                DANGER,
            )
            .size(11),
            Notice::Limits { .. } => {
                label("Host limits loaded. Select a terminal or create one explicitly.", MUTED)
                    .size(11)
            }
            Notice::LimitsUnavailable => label(
                "This Session has no registered Agent, so Host limits are unavailable; history is read-only here.",
                MUTED,
            )
            .size(11),
            Notice::CreateReview => label("Review the shell and dimensions, then Confirm.", MUTED).size(11),
            Notice::Creating => label(
                "Create admitted. Closing does not cancel or undo it; the terminal appears only from the Host acknowledgment.",
                ACCENT,
            )
            .size(11),
            Notice::CreateFailed => label(
                "The Host refused the create (unavailable or invalid). No terminal was added and there is no automatic retry.",
                DANGER,
            )
            .size(11),
            Notice::CreateLimit => label(
                "The Session's Host terminal limit was reached. Close an existing terminal explicitly before creating another.",
                DANGER,
            )
            .size(11),
            Notice::CreatingIndeterminate => label(
                "Create outcome unknown: the Host may or may not have allocated a terminal. No retry was attempted; reconcile with Refresh.",
                DANGER,
            )
            .size(11),
            Notice::CreateNotSent => label(
                "Worker queue rejected the create: not sent. No automatic retry.",
                DANGER,
            )
            .size(11),
            Notice::CloseReview => label("Review the exact terminal, then Confirm close.", MUTED).size(11),
            Notice::Closing => label(
                "Close admitted. Closing this panel does not cancel it; the list changes only from the Host acknowledgment.",
                ACCENT,
            )
            .size(11),
            Notice::Closed => label("Host confirmed the terminal closed.", ACCENT).size(11),
            Notice::CloseFailed => label(
                "The Host refused the close. The terminal may still be running; no automatic retry.",
                DANGER,
            )
            .size(11),
            Notice::ClosingIndeterminate => label(
                "Close outcome unknown: the Host process may still be running. Inspect and retry explicitly if needed.",
                DANGER,
            )
            .size(11),
            Notice::CloseNotSent => label("Worker queue rejected the close: not sent.", DANGER).size(11),
            Notice::WritePending => label(
                "Input admitted to the Host attachment. Acceptance is not shell execution; the line clears only on the Host acknowledgment.",
                ACCENT,
            )
            .size(11),
            Notice::WriteRefused => label(
                "The Host refused this input (attachment read-only, not running, or unavailable). The typed line is kept; no automatic retry.",
                DANGER,
            )
            .size(11),
            Notice::WriteNotSent => label("Worker queue rejected the input: not sent.", DANGER).size(11),
            Notice::WriteIndeterminate => label(
                "Input outcome unknown: the Host may or may not have delivered it. Do not resend blindly; check the terminal output.",
                DANGER,
            )
            .size(11),
            Notice::InputRefused => label(
                "Nothing was written: select a terminal, keep the input non-empty, and stay within the Host limit.",
                DANGER,
            )
            .size(11),
            Notice::FollowAttached => label(
                "Attached with exclusive input control; an older attachment became read-only.",
                MUTED,
            )
            .size(11),
            Notice::FollowEnded => label(
                "The Host ended this output attachment. The process is unaffected; select the terminal again to reattach.",
                MUTED,
            )
            .size(11),
            Notice::FollowFailed => label(
                "The output attachment ended with an error. The Host process is unaffected; reattach explicitly.",
                DANGER,
            )
            .size(11),
        });
        scrollable(
            container(
                container(body)
                    .max_width(self.chat_lane())
                    .width(Length::Fill),
            )
            .center_x(Length::Fill)
            .padding(24),
        )
        .direction(design::scrollbar())
        .style(design::scrollable_style)
        .height(Length::Fill)
        .into()
    }
    /// App-owned MCP composition panel. It lists the rows this app inserted, the
    /// Host's own mount state for them, and the single reviewed write that may
    /// replace the composition file. No secret value is ever rendered.
    fn mcp_view(&self) -> Element<'_, Message> {
        use crate::mcp::{Action, Notice};

        let ticket = self.mcp.read_ticket();
        let context = self.mcp_context();
        let allowed = context.enabled && context.visible;
        let writable = allowed && self.mcp.writable();
        let token = self.mcp.token();

        let mut body = column![
            row![
                label("MCP servers", TEXT).size(22).width(Length::Fill),
                button("Back to conversation")
                    .style(design::ghost)
                    .on_press(Message::Mcp(Action::Dismiss(ticket))),
            ]
            .spacing(10),
            label(
                "Each entry below inserts one @deepseek-ai/dsh-mcp-client row into the Host's home-level patch layer. This app owns that file: it is inside the private native home, rewritten atomically, and backed up first. Content the app cannot preserve is never rewritten.",
                MUTED
            )
            .size(11),
        ]
        .spacing(10);

        match self.mcp.loaded() {
            Some(loaded) if !loaded.path.is_empty() => {
                body = body.push(
                    label(
                        format!(
                            "File · {} · {} · {}",
                            short(&loaded.path, 90),
                            if loaded.revision.is_some() {
                                "present"
                            } else {
                                "absent"
                            },
                            if loaded.inventory_read {
                                "inventory read"
                            } else {
                                "inventory unread"
                            }
                        ),
                        MUTED,
                    )
                    .size(10),
                );
            }
            _ => {}
        }

        body = body.push(
            label(
                self.mcp.notice().text(),
                if matches!(
                    self.mcp.notice(),
                    Notice::Conflict
                        | Notice::Indeterminate
                        | Notice::Invalid
                        | Notice::ReadFailed
                        | Notice::Refused
                        | Notice::NotSent
                        | Notice::Exhausted
                ) {
                    DANGER
                } else {
                    MUTED
                },
            )
            .size(11),
        );
        if let Some(reason) = self.mcp.foreign_reason() {
            body = body.push(label(format!("Refused: {reason}"), DANGER).size(11));
        }
        if let Some(defect) = self.mcp.defect() {
            body = body.push(label(format!("Refused: {}", defect.text()), DANGER).size(11));
        }
        if self.mcp.pending() {
            body = body.push(label(
                "Write submitted; the reviewed document is not installed until the actual outcome returns. Closing the panel cannot undo it.",
                DANGER,
            ).size(11));
        }
        if let Some(report) = self.mcp.report() {
            body = body.push(
                label(
                    format!(
                        "Saved {} bytes to {}.{}",
                        report.bytes,
                        short(&report.path, 90),
                        report
                            .backup
                            .as_ref()
                            .map(|name| format!(" Previous file backed up as {name}."))
                            .unwrap_or_default()
                    ),
                    ACCENT,
                )
                .size(11),
            );
            body = body.push(label(crate::mcp::NEXT_STEP, DANGER).size(11));
            // The only restart this build has is closing the app, and that is
            // the app's own confirmed shutdown flow: nothing restarts silently.
            body = body.push(
                button("Close native Harness…")
                    .padding([8, 12])
                    .style(design::ghost)
                    .on_press(Message::Close),
            );
        }
        if !allowed {
            body = body.push(label(
                "Read-only: the Host lifecycle, smoke mode or another open management flight does not permit requests.",
                DANGER,
            ).size(11));
        }

        body = body.push(
            button(if self.mcp.loaded().is_none() {
                "Reading…"
            } else {
                "Refresh file & inventory"
            })
            .padding([10, 14])
            .style(design::ghost)
            // A press while the first read is outstanding is refused by the
            // panel itself; a press with no snapshot answers a lost read.
            .on_press_maybe(
                (allowed && !self.mcp.pending()).then_some(Message::Mcp(Action::Refresh(ticket))),
            ),
        );

        if self.mcp.mode() == "review" {
            body = body.push(self.mcp_review_view());
        } else if self.mcp.mode() == "form" {
            body = body.push(self.mcp_form_view(token, allowed));
        } else {
            body = body.push(self.mcp_list_view(ticket, writable));
        }

        scrollable(
            container(
                container(body)
                    .max_width(self.chat_lane())
                    .width(Length::Fill),
            )
            .center_x(Length::Fill)
            .padding(24),
        )
        .direction(design::scrollbar())
        .style(design::scrollable_style)
        .height(Length::Fill)
        .into()
    }
    /// One managed row and the Host's own mount state for it.
    fn mcp_row_label(&self, entry: &crate::mcp::ServerEntry) -> (String, String, Color) {
        let mount = match self.mcp.loaded().and_then(|loaded| loaded.mount(entry)) {
            Some(row) => format!(
                "{} · {}{}",
                crate::mcp::phase_label(row.phase),
                if row.enabled { "enabled" } else { "disabled" },
                if row.phase == dsh_native_transport::plugin::FiberPhase::Failed {
                    " · the Host reported a mount error"
                } else {
                    ""
                }
            ),
            None if self
                .mcp
                .loaded()
                .is_some_and(|loaded| loaded.inventory_read) =>
            {
                "not mounted in this Host session".to_owned()
            }
            None => "mount state unknown: inventory unread".to_owned(),
        };
        let colour = if self
            .mcp
            .loaded()
            .and_then(|l| l.mount(entry))
            .is_some_and(|row| row.phase == dsh_native_transport::plugin::FiberPhase::Failed)
        {
            DANGER
        } else {
            MUTED
        };
        (mount, entry.target().to_owned(), colour)
    }
    fn mcp_list_view(
        &self,
        ticket: crate::mcp::ReadTicket,
        writable: bool,
    ) -> Element<'_, Message> {
        use crate::mcp::Action;
        let mut body = column![].spacing(10);
        body = body.push(
            button("Add server…")
                .padding([10, 14])
                .style(design::primary)
                .on_press_maybe(writable.then_some(Message::Mcp(Action::BeginAdd(ticket)))),
        );
        if self.mcp.entries().is_empty() {
            body = body.push(
                label(
                    "No managed MCP server. The composition file carries an empty patch list.",
                    MUTED,
                )
                .size(11),
            );
        }
        for (index, entry) in self.mcp.entries().iter().enumerate() {
            let (mount, target, colour) = self.mcp_row_label(entry);
            let mut card = column![
                row![
                    label(short(&entry.server_name, 40), TEXT).size(14),
                    label(entry.transport.label(), ACCENT).size(11),
                    label(
                        if entry.enabled { "enabled" } else { "disabled" },
                        if entry.enabled { MUTED } else { DANGER }
                    )
                    .size(11),
                ]
                .spacing(8),
                label(short(&target, 90), MUTED).size(11),
                label(mount, colour).size(10),
            ]
            .spacing(6);
            card = card.push(
                row![
                    button("Edit")
                        .padding([7, 11])
                        .style(design::ghost)
                        .on_press_maybe(
                            writable.then_some(Message::Mcp(Action::BeginEdit { ticket, index }))
                        ),
                    button(if entry.enabled {
                        "Remove (disable)"
                    } else {
                        "Already disabled"
                    })
                    .padding([7, 11])
                    .style(design::danger)
                    .on_press_maybe(
                        (writable && entry.enabled)
                            .then_some(Message::Mcp(Action::BeginDisable { ticket, index }))
                    ),
                ]
                .spacing(8),
            );
            body = body.push(
                container(card)
                    .padding(12)
                    .width(Length::Fill)
                    .style(|_| design::card(design::OVERLAY, 12.0)),
            );
        }
        if let Some(loaded) = self.mcp.loaded() {
            body = body.push(label("HOST INVENTORY · mcp-client rows", MUTED).size(10));
            if !loaded.inventory_read {
                body = body.push(label(
                    "The Host inventory could not be read, so mount state is unknown. The file above is still the app-owned composition.",
                    DANGER,
                ).size(11));
            } else if loaded.inventory.is_empty() {
                body = body.push(label(
                    "The Host reports no live @deepseek-ai/dsh-mcp-client row. A just-written entry mounts on the next Host start.",
                    MUTED,
                ).size(11));
            }
            for row in &loaded.inventory {
                body = body.push(
                    label(
                        format!(
                            "{} · {} · {}{}",
                            short(&row.entry_id, 40),
                            crate::mcp::phase_label(row.phase),
                            if row.enabled { "enabled" } else { "disabled" },
                            if row.entry_id.starts_with(crate::mcp::ROW_PREFIX) {
                                ""
                            } else {
                                " · not managed by this app"
                            }
                        ),
                        if row.phase == dsh_native_transport::plugin::FiberPhase::Failed {
                            DANGER
                        } else {
                            MUTED
                        },
                    )
                    .size(10),
                );
            }
        }
        body.into()
    }
    fn mcp_form_view(&self, token: crate::mcp::Token, allowed: bool) -> Element<'_, Message> {
        use crate::mcp::{Action, BindingKind, Field, Transport};
        let Some(draft) = self.mcp.draft() else {
            return label("No draft is open.", MUTED).into();
        };
        let input = allowed && !self.mcp.pending();
        let mut body = column![
            label(
                format!("{} · {}", draft.kind().label(), draft.transport().label()),
                ACCENT
            )
            .size(14),
        ]
        .spacing(10);

        let mut transport_choice = row![].spacing(8);
        for transport in [Transport::Stdio, Transport::StreamableHttp] {
            let chosen = draft.transport() == transport;
            transport_choice = transport_choice.push(
                button(label(
                    format!("{}{}", if chosen { "● " } else { "" }, transport.label()),
                    if chosen { ACCENT } else { MUTED },
                ))
                .padding([8, 12])
                .style(design::ghost)
                .on_press_maybe(
                    input.then_some(Message::Mcp(Action::SetTransport { token, transport })),
                ),
            );
        }
        body = body.push(label("TRANSPORT", MUTED).size(10));
        body = body.push(transport_choice);
        body = body.push(self.mcp_text_field(
            "Server name · [A-Za-z0-9_-]{1,32}",
            draft.server_name(),
            Field::ServerName,
            token,
            input,
        ));
        match draft.transport() {
            Transport::Stdio => {
                body = body.push(self.mcp_text_field(
                    "Command · executable, no shell",
                    draft.command(),
                    Field::Command,
                    token,
                    input,
                ));
                body = body.push(label("ARGUMENTS", MUTED).size(10));
                for (index, arg) in draft.args().iter().enumerate() {
                    body = body.push(
                        row![
                            text_input("argument", arg)
                                .padding(10)
                                .style(design::field)
                                .width(Length::Fill)
                                .on_input_maybe(input.then_some(move |value| Message::Mcp(
                                    Action::SetArgument {
                                        token,
                                        index,
                                        value
                                    }
                                ))),
                            button("Remove")
                                .padding([8, 12])
                                .style(design::ghost)
                                .on_press_maybe(input.then_some(Message::Mcp(
                                    Action::RemoveArgument { token, index }
                                ))),
                        ]
                        .spacing(8),
                    );
                }
                body = body.push(
                    button("Add argument")
                        .padding([8, 12])
                        .style(design::ghost)
                        .on_press_maybe(input.then_some(Message::Mcp(Action::AddArgument(token)))),
                );
                body = body.push(self.mcp_bindings_view(
                    "ENVIRONMENT · names use [A-Za-z_][A-Za-z0-9_]*",
                    BindingKind::Environment,
                    token,
                    input,
                ));
                body = body.push(self.mcp_text_field(
                    "Working directory · empty inherits the Host process directory",
                    draft.cwd(),
                    Field::Cwd,
                    token,
                    input,
                ));
            }
            Transport::StreamableHttp => {
                body = body.push(self.mcp_text_field(
                    "URL · absolute http:// or https:// endpoint",
                    draft.url(),
                    Field::Url,
                    token,
                    input,
                ));
                body = body.push(self.mcp_bindings_view(
                    "HEADERS · HTTP token names",
                    BindingKind::Header,
                    token,
                    input,
                ));
            }
        }
        body = body.push(self.mcp_text_field(
            "Tool-call timeout ms · 1..3600000",
            draft.timeout(),
            Field::Timeout,
            token,
            input,
        ));
        body = body.push(
            row![
                button(label(
                    format!("Enabled: {}", if draft.enabled() { "yes" } else { "no" }),
                    if draft.enabled() { ACCENT } else { DANGER }
                ))
                .padding([8, 12])
                .style(design::ghost)
                .on_press_maybe(input.then_some(Message::Mcp(Action::SetEnabled {
                    token,
                    enabled: !draft.enabled()
                }))),
                button(label(
                    format!(
                        "Fail on startup error: {}",
                        if draft.fail_on_startup_error() {
                            "yes"
                        } else {
                            "no"
                        }
                    ),
                    MUTED
                ))
                .padding([8, 12])
                .style(design::ghost)
                .on_press_maybe(input.then_some(Message::Mcp(Action::SetFailOnStartup {
                    token,
                    value: !draft.fail_on_startup_error()
                }))),
            ]
            .spacing(8),
        );
        body = body.push(
            row![
                button("Review write")
                    .padding([10, 14])
                    .style(design::primary)
                    .on_press_maybe(input.then_some(Message::Mcp(Action::Review(token)))),
                button("Cancel")
                    .padding([8, 12])
                    .style(design::ghost)
                    .on_press_maybe(input.then_some(Message::Mcp(Action::Cancel(token)))),
            ]
            .spacing(8),
        );
        body.into()
    }
    fn mcp_text_field(
        &self,
        placeholder: &'static str,
        value: &str,
        field: crate::mcp::Field,
        token: crate::mcp::Token,
        enabled: bool,
    ) -> Element<'_, Message> {
        let input = text_input(placeholder, value)
            .padding(10)
            .style(design::field);
        if enabled {
            input
                .on_input(move |value| {
                    Message::Mcp(crate::mcp::Action::Field {
                        token,
                        field,
                        value,
                    })
                })
                .into()
        } else {
            input.into()
        }
    }
    fn mcp_bindings_view(
        &self,
        heading: &'static str,
        binding: crate::mcp::BindingKind,
        token: crate::mcp::Token,
        enabled: bool,
    ) -> Element<'_, Message> {
        use crate::mcp::Action;
        let Some(draft) = self.mcp.draft() else {
            return Space::new().into();
        };
        let mut body = column![label(heading, MUTED).size(10)].spacing(8);
        for (index, (key, label_text)) in draft.bindings(binding).into_iter().enumerate() {
            let mut line = row![
                text_input("name", key)
                    .padding(10)
                    .style(design::field)
                    .width(Length::Fill)
                    .on_input_maybe(enabled.then_some(move |value| Message::Mcp(
                        Action::SetBindingKey {
                            token,
                            binding,
                            index,
                            value
                        }
                    ))),
                label(label_text, MUTED).size(10),
            ]
            .spacing(8);
            if let Some(value) = draft.binding_input(binding, index) {
                line = line.push(
                    text_input("new value", value)
                        .padding(10)
                        .style(design::field)
                        .width(Length::Fill)
                        .on_input(move |value| {
                            Message::Mcp(Action::SetBindingSecret {
                                token,
                                binding,
                                index,
                                value: crate::mcp::secret(value),
                            })
                        }),
                );
            } else {
                line = line.push(
                    button("Replace value")
                        .padding([8, 12])
                        .style(design::ghost)
                        .on_press_maybe(enabled.then_some(Message::Mcp(
                            Action::ReplaceBindingValue {
                                token,
                                binding,
                                index,
                            },
                        ))),
                );
            }
            line = line.push(
                button("Remove")
                    .padding([8, 12])
                    .style(design::ghost)
                    .on_press_maybe(enabled.then_some(Message::Mcp(Action::RemoveBinding {
                        token,
                        binding,
                        index,
                    }))),
            );
            body = body.push(line);
        }
        body = body.push(
            button("Add entry")
                .padding([8, 12])
                .style(design::ghost)
                .on_press_maybe(
                    enabled.then_some(Message::Mcp(Action::AddBinding { token, binding })),
                ),
        );
        body.into()
    }
    fn mcp_review_view(&self) -> Element<'_, Message> {
        use crate::mcp::Action;
        let token = self.mcp.token();
        let Some(kind) = self.mcp.review_kind() else {
            return label("No write is under review.", MUTED).into();
        };
        let Some(entries) = self.mcp.review_entries() else {
            return label("No write is under review.", MUTED).into();
        };
        let row = kind.row(entries);
        let mut body = column![
            label(format!("REVIEW · {}", kind.label()), ACCENT).size(14),
            label(kind.text(), MUTED).size(11),
            label(
                format!(
                    "This writes {} managed entr{} in one complete document. Every other entry is preserved exactly as the app read it.",
                    entries.len(),
                    if entries.len() == 1 { "y" } else { "ies" }
                ),
                MUTED,
            )
            .size(11),
        ]
        .spacing(10);
        if let Some(row) = row {
            body =
                body.push(label(format!("row id: {}", short(&row.row_id(), 60)), MUTED).size(10));
            body = body.push(
                label(
                    format!(
                        "{} · {} · {} · timeout {} ms · fail on startup error {} · {}",
                        short(&row.server_name, 40),
                        row.transport.label(),
                        short(row.target(), 70),
                        row.tool_call_timeout_ms,
                        if row.fail_on_startup_error {
                            "yes"
                        } else {
                            "no"
                        },
                        if row.enabled { "enabled" } else { "disabled" }
                    ),
                    TEXT,
                )
                .size(11),
            );
            body = body.push(
                label(
                    format!(
                        "arguments {} · environment {} · headers {} · a restated value is written byte-for-byte and never displayed",
                        row.args.len(),
                        binding_names(&row.env),
                        binding_names(&row.headers)
                    ),
                    MUTED,
                )
                .size(10),
            );
        }
        body = body.push(label(
            "Confirm is the only control that writes. Cancel changes nothing. A refusal, a conflict or an indeterminate outcome is reported and never retried automatically.",
            DANGER,
        ).size(11));
        body = body.push(
            row![
                button("Confirm write")
                    .padding([10, 14])
                    .style(design::primary)
                    .on_press_maybe(
                        (self.mcp.writable() && self.mcp_context().enabled)
                            .then_some(Message::Mcp(Action::Confirm(token)))
                    ),
                button("Cancel review")
                    .padding([8, 12])
                    .style(design::ghost)
                    .on_press_maybe(Some(Message::Mcp(Action::CancelReview(token)))),
            ]
            .spacing(8),
        );
        body.into()
    }
    fn close_view(&self) -> Element<'_, Message> {
        let mut content = column![label("Close native Harness", ACCENT)].spacing(16);
        match &self.close {Close::Inspecting=>content=content.push(label("Inspecting owned Core outside the GUI thread…",TEXT)),Close::Confirm(reason)=>content=content.push(label(reason,TEXT)).push(row![button("Stay").on_press(Message::Stay),button("Close and stop own backend").on_press(Message::ConfirmClose)].spacing(12)),Close::Stopping=>content=content.push(label("Closing native subscriptions and transport; waiting for owned Core exit, containment and any admitted filesystem save (not cancellable; partial files may remain)…",TEXT)),Close::Failed(reason)=>content=content.push(label(reason,DANGER)).push(row![button("Stay").on_press(Message::Stay),button("Exit anyway (result uncertain)").on_press_maybe((self.stopped && !self.draft_state.busy() && !self.local_files.busy()).then_some(Message::ExitAnyway))].spacing(12)),Close::None=>{}}
        container(
            container(content)
                .padding(28)
                .max_width(760)
                .style(|_| design::card(SURFACE, design::RADIUS_XL)),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    }
}
#[cfg(test)]
fn chat_role(kind: &str, role: &str) -> bool {
    reducer::presentation::chat_visible(kind, role)
}
#[cfg(test)]
fn chat_row_count(rows: &[DisplayRow]) -> usize {
    rows.iter().filter(|row| row.chat).count()
}
#[cfg(test)]
fn chat_rows<'a>(rows: &'a [DisplayRow], partial: Option<&'a DisplayRow>) -> Vec<&'a DisplayRow> {
    rows.iter().filter(|row| row.chat).chain(partial).collect()
}
pub(crate) fn panel(color: Color) -> container::Style {
    container::Style {
        background: Some(color.into()),
        text_color: Some(TEXT),
        ..Default::default()
    }
}
/// Wayland clipboard text reads for paste.
///
/// Iced's bundled `window_clipboard`/`smithay-clipboard` backend selects a seat
/// only from key/pointer serials and otherwise fails with "no events received on
/// any seat", so an ordinary Paste click reads nothing. `wl-clipboard-rs` owns
/// its own connection and needs no focused seat. Defined here (not as a crate
/// module) because every example compiles this file directly.
mod native_clipboard {
    use std::io::Read;
    use std::time::Duration;

    /// Longest clipboard text this module returns; the API-key draft cap is far
    /// smaller and this only stops a pathological clipboard owning memory.
    pub const MAX_TEXT_BYTES: usize = 256 * 1024;
    /// Bound on how long a clipboard owner may take before paste reports nothing.
    pub const READ_TIMEOUT: Duration = Duration::from_millis(1500);

    /// Copies text, bounded by [`READ_TIMEOUT`]. Iced's own write goes through
    /// the same seat-bound backend its read does, so the app owns this too.
    /// Returns whether the clipboard accepted the text.
    pub fn write_text(text: String) -> bool {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(write_wayland_text(&text));
        });
        receiver.recv_timeout(READ_TIMEOUT).ok().unwrap_or(false)
    }

    fn write_wayland_text(text: &str) -> bool {
        wl_clipboard_rs::copy::Options::new()
            .copy(
                wl_clipboard_rs::copy::Source::Bytes(text.as_bytes().to_vec().into_boxed_slice()),
                wl_clipboard_rs::copy::MimeType::Text,
            )
            .is_ok()
    }

    /// Reads clipboard text, bounded by [`READ_TIMEOUT`] and [`MAX_TEXT_BYTES`].
    /// Blocks only its own thread; returns `None` when nothing usable arrives.
    pub fn read_text() -> Option<String> {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(read_wayland_text());
        });
        receiver.recv_timeout(READ_TIMEOUT).ok().flatten()
    }

    fn read_wayland_text() -> Option<String> {
        let (pipe, _mime) = wl_clipboard_rs::paste::get_contents(
            wl_clipboard_rs::paste::ClipboardType::Regular,
            wl_clipboard_rs::paste::Seat::Unspecified,
            wl_clipboard_rs::paste::MimeType::Text,
        )
        .ok()?;
        let mut text = String::new();
        pipe.take(MAX_TEXT_BYTES as u64 + 1)
            .read_to_string(&mut text)
            .ok()?;
        (text.len() <= MAX_TEXT_BYTES).then_some(text)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn clipboard_read_returns_within_its_bound_without_a_wayland_session() {
            // A session may or may not serve text; the contract under test is
            // that the call always returns inside its bound and never panics.
            let start = std::time::Instant::now();
            let observed = read_text();
            println!(
                "native_clipboard observed bytes = {:?}",
                observed.as_ref().map(String::len)
            );
            if let Some(text) = observed {
                assert!(text.len() <= MAX_TEXT_BYTES);
            }
            assert!(start.elapsed() < READ_TIMEOUT + Duration::from_secs(2));
        }

        #[test]
        fn clipboard_caps_are_bounded_and_cover_the_api_key_draft() {
            assert!(MAX_TEXT_BYTES >= 16_384);
            assert!(READ_TIMEOUT <= Duration::from_secs(3));
        }
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
/// The reference's default maximum interval between the two Escape presses
/// (`stopSequenceMs`, 500 ms; `shortcuts/README.md`).
const STOP_SEQUENCE_WINDOW: Duration = Duration::from_millis(500);

/// Wall-clock milliseconds for a running child's active duration. The projection's `since` and
/// `through` are epoch milliseconds; a pre-epoch clock yields 0, which formats as "0s".
/// The busy-state Enter preference parser, exposed for the public fixture's flag.
pub(crate) fn settings_busy_enter(raw: &str) -> Option<crate::settings::BusyEnter> {
    crate::settings::BusyEnter::parse(raw)
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// The message a stored binding should send. `Send` and `Newline` are absent on purpose: the
/// composer's text input owns those keys, so a rebound key cannot meaningfully stand in for them
/// and pretending otherwise would move focus.
fn invoke_message(invoke: Option<crate::keybindings::Invoke>) -> Option<Message> {
    use crate::keybindings::Invoke;
    match invoke? {
        Invoke::Navigation => Some(Message::ToggleSidebar),
        Invoke::Find => Some(Message::SearchOpen),
        Invoke::Close => Some(Message::DismissPanel),
        Invoke::Send | Invoke::Newline => None,
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
#[path = "text_excerpt.rs"]
mod text_excerpt;
fn excerpt(value: &str) -> String {
    text_excerpt::excerpt(value)
}
fn brand<'a>() -> Element<'a, Message> {
    container(label("H", ACCENT).size(19))
        .width(36)
        .height(36)
        .center_x(36)
        .center_y(36)
        .style(|_| design::card(OVERLAY, design::RADIUS_MD))
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
    // `max().min()` rather than `clamp` so a non-finite viewport answers a finite
    // 1.0 instead of propagating NaN into the modal's size.
    #[allow(clippy::manual_clamp)]
    iced::Size::new(
        (viewport.width - 48.0).max(1.0).min(960.0),
        (viewport.height - 48.0).max(1.0).min(700.0),
    )
}

/// One binding list as `NAME (N bytes)`; the value itself is never rendered.
fn binding_names(bindings: &[(String, crate::mcp::Secret)]) -> String {
    if bindings.is_empty() {
        return "none".to_owned();
    }
    bindings
        .iter()
        .map(|(key, value)| format!("{key} ({} bytes)", value.len()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn background_message(message: &Message) -> bool {
    matches!(
        message,
        Message::FileOpen
            | Message::FileChoose
            | Message::FilePicker(_)
            | Message::FileDropped(..)
            | Message::FileDropCancelled(..)
            | Message::FilePreview(_)
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
            | Message::KeyboardSend
            | Message::ModelControls(_)
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
            | Message::ToggleThoughts(_)
            | Message::CopyCode(..)
            | Message::CopyReply { .. }
            | Message::EditReply { .. }
            | Message::ConfirmEdit { .. }
            | Message::CancelEdit
            | Message::DraftReload
            | Message::ReviewRecovered { .. }
            | Message::SearchOpen
            | Message::SearchInput { .. }
            | Message::SearchNext { .. }
            | Message::SearchClose { .. }
            | Message::HideDetails(_)
            | Message::Capabilities
            | Message::Interaction(_)
            | Message::Management(_)
            | Message::Rename(_)
            | Message::Fork(_)
            | Message::Terminal(_)
            | Message::ExportSession(_)
            | Message::Export(_)
            | Message::ArchivedView
            | Message::Sidebar(_)
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

/// Content that hands an inside mouse press to a caret request.
///
/// Iced's text editor clears its own focus on any left press outside its bounds,
/// and it processes the press before the widget around it does, so a plain
/// The reference's own full-access warning, verbatim
/// (`ui-permission-presets/src/client/locales.ts:35`).
/// The reference's badge copy for the deployment's registered Auto preset
/// (`ui-permission-presets/src/client/locales.ts:54`).
const AUTO_BADGE: &str = "EXP";
const FULL_ACCESS_RISK: &str = "Full access lets new sessions reduce confirmation steps and perform more actions directly, including sensitive operations, file changes, or external commands. Only use it when you trust subsequent tasks.";
/// One job row: the producer's label, then its kind and status words. The label is primary and
/// the pair secondary, which is the reference's own row structure without its unread dot.
fn job_row(
    job: &dsh_native_transport::dto::JobView,
    expanded: Option<&str>,
    output: &str,
) -> Element<'static, Message> {
    let ink = match job.status.as_str() {
        "failed" => DANGER,
        "completed" | "killed" => MUTED,
        _ => TEXT,
    };
    // The reference's marker is one glyph in five colours, not five glyphs
    // (`ui-primitives/src/StateDot.module.css`: success/warn/error/idle), with
    // `running → ongoing` the one state it animates as a spinner — which a static capture cannot
    // show, so the half-filled dot stands in and is recorded as a deviation, not as the spinner
    // (`ui-jobs/src/client/JobListAction.tsx:86-96` for the mapping).
    let (glyph, dot_ink) = match job.status.as_str() {
        "running" => ("◐", ACCENT),
        "stopping" | "killed" => ("●", design::GOLD),
        "failed" => ("●", DANGER),
        "completed" => ("●", MUTED),
        _ => ("○", MUTED),
    };
    let expanded = expanded == Some(job.id.as_str());
    let head = row![
        // The reference's chevron control (`row.expandAria`: "Show live output of {label}").
        button(label(if expanded { "▾" } else { "▸" }, MUTED).size(10))
            .on_press(Message::ToggleJobOutput(job.id.clone()))
            .style(design::ghost)
            .padding([0, 2]),
        container(label(glyph, dot_ink).size(11))
            .width(design::CRUMB_SIZE)
            .align_x(iced::Alignment::Center),
        column![
            label(job.label.clone(), ink).size(13),
            label(format!("{} · {}", job.kind, job.status), MUTED).size(11),
        ]
        .spacing(1)
        .width(Length::Fill),
        // The reference names the control through a label ("Stop task {label}") and confirms
        // on a second click; the visible form of the control is unread, so a plain glyph
        // stands in and the confirmation copy is the reference's own.
        button(label("✕", MUTED).size(11))
            .on_press(Message::KillJob(job.id.clone()))
            .style(design::ghost)
            .padding([2, 4]),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    let mut body = column![head].spacing(2).width(Length::Fill);
    if expanded {
        // The reference renders the buffer in a code font; this app has one UI font, so the
        // output is the same font at a smaller size in muted ink, recorded as a deviation.
        body = body.push(
            container(text(output.to_owned()).size(10).color(MUTED))
                .width(Length::Fill)
                .padding([2, 0]),
        );
    }
    container(body).width(Length::Fill).into()
}
/// The display name for a permission preset. The reference takes it from the Host's
/// catalog and falls back to a locale key; native has no catalog here yet, so the three
/// design presets carry their own names and anything host-configured shows its own key,
/// which is what the Host's own `optionOf` does for a missing label.
fn permission_label(preset: &str) -> &str {
    match preset {
        "read-only" => "Read only",
        "workspace-write" => "Workspace write",
        "full-access" => "Full access",
        "auto" => "Auto",
        other => other,
    }
}
/// `mouse_area` cannot stop it: the field has already unfocused and captured the
/// event by the time the wrapper would run. This wrapper therefore lets the
/// content read the press first -- a control inside keeps its click -- and then,
/// when no child consumed the event, publishes the retention message. The press
/// is not captured, so it still reaches whatever else wants it.
struct FocusKeeper<'a, M, R = iced::Renderer> {
    content: Element<'a, M, Theme, R>,
    on_press: M,
}
impl<M: Clone, R: iced::advanced::Renderer> iced::advanced::Widget<M, Theme, R>
    for FocusKeeper<'_, M, R>
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
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport)
    }
    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &R,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
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
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
        if shell.is_event_captured() {
            return;
        }
        if let iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) =
            event
            && cursor.is_over(layout.bounds())
        {
            shell.publish(self.on_press.clone());
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
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &R,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, M, Theme, R>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
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

/// The number of rows the menu would show, so the selection can be clamped
/// against the same list the view renders.
fn matches_len(draft: &str, candidates: &[command_menu::Candidate]) -> usize {
    command_menu::query(draft)
        .map(|query| command_menu::matches(candidates, query).len())
        .unwrap_or(0)
}
fn composer_binding(
    event: text_editor::KeyPress,
    send_enabled: bool,
    menu_open: bool,
    bindings: &crate::keybindings::Store,
) -> Option<text_editor::Binding<Message>> {
    if matches!(event.status, text_editor::Status::Focused { .. }) {
        // While the menu is open these keys move the highlight or accept it, so
        // a half-typed command is never sent and the caret does not jump. They
        // are the reference's fixed menu keys, not editable commands.
        if menu_open && event.modifiers.is_empty() {
            match &event.key {
                keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                    return Some(text_editor::Binding::Custom(Message::CommandMenuMove(1)));
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                    return Some(text_editor::Binding::Custom(Message::CommandMenuMove(-1)));
                }
                keyboard::Key::Named(keyboard::key::Named::Tab) => {
                    return Some(text_editor::Binding::Custom(Message::CommandMenuAccept));
                }
                keyboard::Key::Named(keyboard::key::Named::Escape) => {
                    return Some(text_editor::Binding::Custom(Message::CommandMenuDismiss));
                }
                _ => {}
            }
        }
        // The field's own clipboard links keep their keys: a command bound to Ctrl+V or Ctrl+C
        // must not swallow the paste or copy the focused editor owns.
        if paste_shortcut(&event.key, event.modifiers) {
            return if wayland_session() {
                // Iced's own Ctrl+V goes through the unusable Wayland clipboard
                // backend; read on a worker thread instead (see native_clipboard).
                Some(text_editor::Binding::Custom(Message::PasteComposer))
            } else {
                text_editor::Binding::from_key_press(event)
            };
        }
        if clipboard_field_shortcut(&event.key, event.modifiers).is_some() {
            return text_editor::Binding::from_key_press(event);
        }
        // The store is read in the view, so its snapshot is the one this frame draws: a rebound
        // key invokes its command here, and a cleared or moved key falls through to the editor
        // instead of the default it no longer holds.
        if let Some(id) = crate::keybindings::resolve(bindings, &event.key, event.modifiers) {
            use crate::keybindings::Invoke;
            return match crate::keybindings::invoke_for(id) {
                Some(Invoke::Send) => {
                    send_enabled.then_some(text_editor::Binding::Custom(Message::KeyboardSend))
                }
                Some(Invoke::Newline) => Some(text_editor::Binding::Enter),
                Some(Invoke::Navigation) => {
                    Some(text_editor::Binding::Custom(Message::ToggleSidebar))
                }
                Some(Invoke::Find) => Some(text_editor::Binding::Custom(Message::SearchOpen)),
                // The surface decides what dismissal means (Details is topmost); carrying the
                // press as data keeps that decision in `key_pressed`, where live state lives.
                Some(Invoke::Close) => Some(text_editor::Binding::Custom(Message::KeyPressed(
                    event.key.clone(),
                    event.modifiers,
                ))),
                None => None,
            };
        }
    }
    text_editor::Binding::from_key_press(event)
}
/// Reports which of the tagged text fields currently holds focus. Iced's
/// `text_input` has no copy or paste binding, so the app answers Ctrl+C and
/// Ctrl+V itself, and this is how it learns where to send them.
struct FocusedField<F> {
    fields: Vec<&'static str>,
    found: Option<String>,
    then: std::sync::Mutex<Option<F>>,
}
impl<F> iced::advanced::widget::Operation<Message> for FocusedField<F>
where
    F: FnOnce(Option<String>) -> Message + Send,
{
    fn focusable(
        &mut self,
        id: Option<&iced::advanced::widget::Id>,
        _bounds: iced::Rectangle,
        state: &mut dyn iced::advanced::widget::operation::Focusable,
    ) {
        if !state.is_focused() {
            return;
        }
        if let Some(id) = id
            && let Some(field) = self
                .fields
                .iter()
                .find(|field| id == &iced::advanced::widget::Id::new(**field))
        {
            self.found = Some((*field).to_owned());
        }
    }
    fn traverse(
        &mut self,
        operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation<Message>),
    ) {
        operate(self);
    }
    fn finish(&self) -> iced::advanced::widget::operation::Outcome<Message> {
        let callback = self
            .then
            .lock()
            .ok()
            .and_then(|mut callback| callback.take());
        match callback {
            Some(callback) => {
                iced::advanced::widget::operation::Outcome::Some(callback(self.found.clone()))
            }
            None => iced::advanced::widget::operation::Outcome::None,
        }
    }
}

/// Ctrl/Command+C or Ctrl/Command+V without Alt/Shift, for a text field the
/// app answers itself. Iced's single-line input implements neither.
fn clipboard_field_shortcut(
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<Message> {
    if modifiers.alt() || modifiers.shift() || !modifiers.command() {
        return None;
    }
    let keyboard::Key::Character(value) = key else {
        return None;
    };
    if value.eq_ignore_ascii_case("c") {
        Some(Message::ClipboardCopyField)
    } else if value.eq_ignore_ascii_case("v") {
        Some(Message::ClipboardPasteField)
    } else {
        None
    }
}
/// True on a Wayland session, where Iced's bundled clipboard read fails.
fn wayland_session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}
/// Ctrl/Command+V without Alt/Shift, matching Iced's own paste binding scope.
fn paste_shortcut(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> bool {
    !modifiers.alt()
        && !modifiers.shift()
        && modifiers.command()
        && matches!(key, keyboard::Key::Character(value) if value.eq_ignore_ascii_case("v"))
}
/// Panel-level Ctrl+V for the Models key field on Wayland. Iced's secure
/// `text_input` pastes through the unusable Wayland clipboard backend, so the
/// panel reads instead and replaces the draft; other stores are unaffected.
fn settings_paste_message(
    wayland: bool,
    paste: Option<crate::settings::Ticket>,
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<Message> {
    if !wayland || !paste_shortcut(key, modifiers) {
        return None;
    }
    paste.map(|ticket| Message::Settings(crate::settings::Action::Paste { ticket }))
}
/// Recorded body shown when a tool row expands. The compact line already names
/// the tool, so a call drops that first line; a result is shown whole. Only the
/// text this app already retained for the row is ever surfaced.
fn activity_body_source(row: &DisplayRow) -> &str {
    let text = row.text.trim();
    if row.role == "Tool call" {
        text.split_once('\n').map_or("", |(_, body)| body.trim())
    } else {
        text
    }
}
/// True for catalog rows the backend reports for its DeepSeek route. Matched
/// case-insensitively so a renamed route or an id-only row still appears.
pub(crate) fn deepseek_route(model: &Model) -> bool {
    model.provider.to_ascii_lowercase().contains("deepseek")
        || model.id.to_ascii_lowercase().contains("deepseek")
}
fn title(session: &SessionSummary) -> String {
    projected_title(
        session,
        session
            .projections
            .as_ref()
            .and_then(|p| p.values.get("title")),
    )
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
            success: design::FOAM,
            warning: design::GOLD,
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
#[cfg(test)]
#[path = "navigation_efficiency_tests.rs"]
mod navigation_efficiency_tests;
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
#[path = "title_ui_tests.rs"]
mod title_ui_tests;
#[cfg(test)]
#[path = "transcript_layout_tests.rs"]
mod transcript_layout_tests;
#[cfg(test)]
#[path = "transcript_projection_tests.rs"]
mod transcript_projection_tests;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_file_attachment_adds_its_card_to_the_row_the_view_draws() {
        let (app, _) = app();
        let lane = app.chat_lane();
        let plain = display_row(1, "You");
        let mut attached = plain.clone();
        attached.files = vec![FileCard {
            name: "PUBLIC-note.txt".into(),
            meta: "TXT 12B".into(),
        }];
        // The reservation is the stack the view builds: the card plus the 8px
        // gap to the bubble.
        assert_eq!(
            app.row_geometry(&attached, lane).height - app.row_geometry(&plain, lane).height,
            design::attachment_height(1, true)
        );
        // A file-only message draws no bubble, so the card is the whole body and
        // no gap follows it (`MessageItem.tsx:187`): the row is the inter-row
        // chrome, the action row and the card, with no bubble padding.
        let mut file_only = attached.clone();
        file_only.key = 2;
        file_only.text.clear();
        assert_eq!(
            app.row_geometry(&file_only, lane).height,
            chat_geometry::ROW_CHROME
                + action_footer(app.content_delta())
                + design::ATTACHMENT_CARD_HEIGHT
        );
        // The text body still reserves only itself; the card never inflates it.
        assert_eq!(
            app.row_geometry(&attached, lane).body_height,
            app.row_geometry(&plain, lane).body_height
        );
    }
    #[test]
    fn conversation_flow_gaps_follow_the_web_ladder() {
        let tool = display_row(1, "Tool call");
        let tool2 = display_row(2, "Tool call");
        let assistant = display_row(3, "Assistant");
        let user = display_row(4, "You");
        let live = display_row(5, "Assistant · live");
        let rows: Vec<&DisplayRow> = vec![&tool, &tool2, &assistant, &user, &live];
        // The web: no leading margin on the first row, 6 between process
        // siblings, 12 around a response (`ChatView.module.css:60-95`).
        assert_eq!(flow_gap_before(&rows, 0), 0.0);
        assert_eq!(flow_gap_before(&rows, 1), design::FLOW_GAP);
        assert_eq!(flow_gap_before(&rows, 2), design::RESPONSE_GAP);
        assert_eq!(flow_gap_before(&rows, 3), design::RESPONSE_GAP);
        assert_eq!(flow_gap_before(&rows, 4), design::RESPONSE_GAP);
        // The reserved height carries that same gap and no longer the row's own
        // symmetric 12px padding, which is what the view draws as top padding.
        let (app, _) = app();
        let lane = app.chat_lane();
        let geometries = app.row_geometries(&rows);
        for (index, geometry) in geometries.iter().enumerate() {
            let bare = app.row_geometry(rows[index], lane).height;
            assert_eq!(
                geometry.height,
                bare - chat_geometry::ROW_CHROME + flow_gap_before(&rows, index),
                "row {index}"
            );
        }
        // Two process rows are 6px apart where the old uniform padding made them
        // 12; a response row keeps exactly the height the old 12px padding gave
        // it, so only the process rhythm moved.
        let bare_tool = app.row_geometry(&tool2, lane).height;
        assert_eq!(
            bare_tool - geometries[1].height,
            chat_geometry::ROW_CHROME - design::FLOW_GAP
        );
        let bare_response = app.row_geometry(&assistant, lane).height;
        assert_eq!(bare_response, geometries[2].height);
    }
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
            kind: role.into(),
            chat: crate::reducer::presentation::chat_visible(role, role),
            text: "fixture".into(),
            files: Vec::new(),
            references: Vec::new(),
            trigger: None,
            time: 1000,
            surface: false,
        }
    }
    #[test]
    fn chat_filter_hides_known_internals_and_keeps_conversation_and_unknown_rows() {
        let roles = [
            // Trajectory-only: the system prompt, developer injects, model-only
            // context and known bookkeeping/telemetry kinds.
            "permission/preset",
            "sandbox/mode",
            "approval/policy",
            "System",
            "Developer",
            "Model context update",
            "Injected context",
            "session-log-deepseek/delivery-accepted",
            // Chat keeps the conversation, and every unrecognized kind stays
            // visible so a future required diagnostic is never lost.
            "You",
            "Assistant",
            "Assistant · live",
            "Interrupted assistant attempt",
            "Tool call",
            "Tool result",
            "approval/policy-changed",
            "Permission/preset",
            "future/required",
        ];
        let hidden = 8;
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
            roles[hidden..]
        );
        assert_eq!(chat_row_count(&rows), roles.len() - hidden);
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
        let before_rows: Vec<_> = app.transcript.display_rows(false).collect();
        let before_geometry = app.row_geometries(&before_rows);
        assert_eq!(before_rows[0].key, 4);
        assert!(app.offset + app.viewport < before_geometry[0].height);
        let within = app.offset;
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
        let after_rows: Vec<_> = app.transcript.display_rows(false).collect();
        assert_eq!(
            after_rows.iter().map(|row| row.key).collect::<Vec<_>>(),
            [3, 4]
        );
        let after_geometry = app.row_geometries(&after_rows);
        let anchor_index = after_rows.iter().position(|row| row.key == 4).unwrap();
        let prefix = after_geometry[..anchor_index]
            .iter()
            .map(|row| row.height)
            .sum::<f32>();
        assert_eq!(app.offset - prefix, within);
        assert!(
            app.offset + app.viewport < after_geometry.iter().map(|row| row.height).sum::<f32>()
        );
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
    fn tool_and_unknown_roles_never_trigger_metadata_only_welcome() {
        for role in [
            "Tool result",
            "Tool call",
            "future/required",
            "Permission/preset",
        ] {
            assert!(chat_role(role, role));
        }
        for role in [
            "System",
            "Developer",
            "permission/preset",
            "sandbox/mode",
            "approval/policy",
            "session-log-deepseek/delivery-accepted",
        ] {
            assert!(!chat_role(role, role), "{role}");
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
    fn named(key: keyboard::key::Named) -> text_editor::KeyPress {
        text_editor::KeyPress {
            key: keyboard::Key::Named(key),
            modified_key: keyboard::Key::Named(key),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Tab),
            modifiers: keyboard::Modifiers::default(),
            text: None,
            status: text_editor::Status::Focused { is_hovered: false },
        }
    }
    /// While the menu is open the navigation keys must not reach the editor, and
    /// while it is closed they must behave exactly as they did before.
    #[test]
    fn the_completion_menu_claims_navigation_keys_only_while_open() {
        use keyboard::key::Named;
        for key in [Named::ArrowDown, Named::ArrowUp, Named::Tab, Named::Escape] {
            let open = composer_binding(named(key), true, true, &bindings());
            let claimed = matches!(
                open,
                Some(text_editor::Binding::Custom(
                    Message::CommandMenuMove(_)
                        | Message::CommandMenuAccept
                        | Message::CommandMenuDismiss
                ))
            );
            assert!(claimed, "{key:?} must be claimed while the menu is open");
            // The exact row movement, so a swapped direction cannot pass.
            if matches!(key, Named::ArrowDown | Named::ArrowUp) {
                let step = if matches!(key, Named::ArrowDown) {
                    1
                } else {
                    -1
                };
                let actual = composer_binding(named(key), true, true, &bindings());
                assert!(
                    matches!(
                        actual,
                        Some(text_editor::Binding::Custom(Message::CommandMenuMove(delta)))
                            if delta == step
                    ),
                    "{key:?} must move {step}"
                );
            }
            // Closed: the same keys fall through to the editor's own handling,
            // except Escape, which the app already binds elsewhere.
            let closed = composer_binding(named(key), true, false, &bindings());
            if !matches!(key, Named::Escape) {
                assert!(
                    !matches!(
                        closed,
                        Some(text_editor::Binding::Custom(
                            Message::CommandMenuMove(_)
                                | Message::CommandMenuAccept
                                | Message::CommandMenuDismiss
                        ))
                    ),
                    "{key:?} must not be claimed while the menu is closed"
                );
            }
        }
        // A modified navigation key stays the editor's: Ctrl+Home and friends.
        let mut modified = named(Named::ArrowDown);
        modified.modifiers = keyboard::Modifiers::CTRL;
        assert!(!matches!(
            composer_binding(modified, true, true, &bindings()),
            Some(text_editor::Binding::Custom(Message::CommandMenuMove(1)))
        ));
    }
    /// Accepting a command is an edit, and the caret stays in the draft. Iced's
    /// text editor drops focus on any press outside its own bounds, so a
    /// completion the user clicked would otherwise be the last thing they could
    /// type until they clicked back into the field.
    #[test]
    fn accepting_a_command_edits_the_draft_and_asks_for_focus_back() {
        let (mut app, _) = app();
        app.commands = Some(vec![
            command_menu::Candidate {
                name: "plan".into(),
                description: "Enter plan mode".into(),
            },
            command_menu::Candidate {
                name: "compact".into(),
                description: "Compact the conversation".into(),
            },
        ]);
        app.editor = text_editor::Content::with_text("/p");
        // "plan" leads on the prefix and "compact" matches by substring, so the
        // highlight rests on a row the pointer is not aiming at.
        assert_eq!(
            app.completion().map(|menu| menu.matches.len()),
            Some(2),
            "the fixture must offer a row the highlight is not on"
        );
        app.command_selected = 0;
        assert_eq!(
            app.completion().and_then(|menu| menu.candidate().cloned()),
            Some(command_menu::Candidate {
                name: "plan".into(),
                description: "Enter plan mode".into(),
            })
        );
        // A pointer names the row it hit; the highlight must not decide for it.
        let task = app.update(Message::CommandMenuPick("compact".into()));
        assert_eq!(app.editor.text(), "/compact ");
        assert_eq!(app.command_selected, 0);
        assert!(task.units() >= 1, "the pick must ask for the caret back");
        // The keyboard route accepts the highlighted row and does the same edit.
        app.editor = text_editor::Content::with_text("/co");
        app.command_selected = 0;
        let task = app.update(Message::CommandMenuAccept);
        assert_eq!(app.editor.text(), "/compact ");
        assert!(task.units() >= 1, "accept must ask for the caret back");
        // Dismissal keeps every typed character and still returns the caret.
        app.editor = text_editor::Content::with_text("/pla");
        let task = app.update(Message::CommandMenuDismiss);
        assert_eq!(app.editor.text(), "/pla");
        assert!(task.units() >= 1, "dismissal must ask for the caret back");
    }
    /// A press on the composer that owns no action publishes only the caret
    /// request, in the same message vocabulary the view uses.
    #[test]
    fn a_composer_press_that_owns_no_action_only_asks_for_focus() {
        let (mut app, _) = app();
        assert!(app.composer_visible());
        let task = app.update(Message::ComposerFocus);
        assert!(task.units() >= 1);
        // The composer's own disclosure is one of those presses.
        let task = app.update(Message::ToggleContextPolicy);
        assert!(app.context_policy_open);
        assert!(task.units() >= 1);
    }
    /// The card and the completion rows keep the caret through `FocusKeeper`. A
    /// rendered composer needs a real GPU renderer, so the rule is pinned here with
    /// a stand-in field; `NATIVE-COMPOSER-CONTROLS.md` records that limit.
    #[test]
    fn focus_keeper_hands_the_press_to_its_content_first_and_only_then_keeps_the_caret() {
        use iced::advanced::{self as core, Widget};
        #[derive(Clone)]
        struct Probe {
            pressed: std::rc::Rc<std::cell::Cell<usize>>,
            captures: bool,
        }
        impl Widget<Message, Theme, ()> for Probe {
            fn size(&self) -> iced::Size<Length> {
                iced::Size::new(Length::Fill, Length::Fill)
            }
            fn layout(
                &mut self,
                _: &mut core::widget::Tree,
                _: &(),
                _: &core::layout::Limits,
            ) -> core::layout::Node {
                core::layout::Node::new(iced::Size::new(200.0, 40.0))
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
                shell: &mut core::Shell<'_, Message>,
                _: &iced::Rectangle,
            ) {
                self.pressed.set(self.pressed.get() + 1);
                if self.captures {
                    shell.capture_event();
                }
            }
            fn mouse_interaction(
                &self,
                _: &core::widget::Tree,
                _: core::Layout<'_>,
                _: core::mouse::Cursor,
                _: &iced::Rectangle,
                _: &(),
            ) -> core::mouse::Interaction {
                core::mouse::Interaction::Text
            }
        }
        let bounds = iced::Rectangle::with_size(iced::Size::new(200.0, 40.0));
        let layout = core::layout::Node::with_children(
            bounds.size(),
            vec![core::layout::Node::new(bounds.size())],
        );
        let viewport = iced::Rectangle::with_size(iced::Size::new(900.0, 600.0));
        let click = |probe: Probe| -> (Probe, bool, Vec<Message>) {
            let mut wrapped: Element<'_, Message, Theme, ()> = Element::new(FocusKeeper {
                content: Element::new(probe.clone()),
                on_press: Message::ComposerFocus,
            });
            let mut tree = iced::advanced::widget::Tree::new(&wrapped);
            let mut messages = Vec::new();
            let mut shell = core::Shell::new(&mut messages);
            wrapped.as_widget_mut().update(
                &mut tree,
                &iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
                core::Layout::new(&layout),
                core::mouse::Cursor::Available(iced::Point::new(10.0, 10.0)),
                &(),
                &mut core::clipboard::Null,
                &mut shell,
                &viewport,
            );
            (probe, shell.is_event_captured(), messages)
        };
        let loose = Probe {
            pressed: std::rc::Rc::default(),
            captures: false,
        };
        // The content never claimed the press: the wrapper asks for the caret.
        let (loose, captured, messages) = click(loose.clone());
        assert_eq!(loose.pressed.get(), 1, "the content still reads the press");
        assert!(!captured, "another widget may still want this press");
        assert!(matches!(messages.as_slice(), [Message::ComposerFocus]));
        // A control that consumed the press keeps its click and no caret request.
        let (claimed, captured, messages) = click(Probe {
            pressed: std::rc::Rc::default(),
            captures: true,
        });
        assert_eq!(claimed.pressed.get(), 1);
        assert!(captured);
        assert!(messages.is_empty());
    }
    fn bindings() -> crate::keybindings::Store {
        crate::keybindings::Store::new(crate::keybindings::native_fixed())
    }
    fn press(character: &str, modifiers: keyboard::Modifiers) -> text_editor::KeyPress {
        text_editor::KeyPress {
            key: keyboard::Key::Character(character.into()),
            modified_key: keyboard::Key::Character(character.into()),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyJ),
            modifiers,
            text: None,
            status: text_editor::Status::Focused { is_hovered: false },
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
    fn enter_and_ctrl_enter_require_nonempty_send_permission_shift_enter_is_newline() {
        for modifiers in [keyboard::Modifiers::empty(), keyboard::Modifiers::CTRL] {
            assert!(composer_binding(enter(modifiers), false, false, &bindings()).is_none());
            assert!(matches!(
                composer_binding(enter(modifiers), true, false, &bindings()),
                Some(text_editor::Binding::Custom(Message::KeyboardSend))
            ));
        }
        for allowed in [false, true] {
            assert!(matches!(
                composer_binding(
                    enter(keyboard::Modifiers::SHIFT),
                    allowed,
                    false,
                    &bindings()
                ),
                Some(text_editor::Binding::Enter)
            ));
        }
        let mut unfocused = enter(keyboard::Modifiers::empty());
        unfocused.status = text_editor::Status::Active;
        assert!(!matches!(
            composer_binding(unfocused, true, false, &bindings()),
            Some(text_editor::Binding::Custom(Message::KeyboardSend))
        ));
    }
    #[test]
    fn a_rebound_command_replaces_its_default_in_the_live_chain() {
        use keyboard::{Key, Modifiers};
        let (mut app, _receiver) = app();
        let _ = app.update(Message::WindowActive(true));
        let before = app.sidebar_expanded;
        let _ = app.update(Message::KeyPressed(
            Key::Character("b".into()),
            Modifiers::CTRL,
        ));
        assert_ne!(app.sidebar_expanded, before, "the default still works");
        // Move Navigation onto Ctrl+L through the store the editor edits.
        let _ = app.settings.keybindings.apply(
            crate::keybindings::Edit::Set {
                id: "native.navigation".to_owned(),
                binding: Some(
                    crate::keybindings::binding_from(&Key::Character("l".into()), Modifiers::CTRL)
                        .unwrap(),
                ),
            },
            app.settings.keybindings.revision(),
        );
        let before = app.sidebar_expanded;
        let _ = app.update(Message::KeyPressed(
            Key::Character("b".into()),
            Modifiers::CTRL,
        ));
        assert_eq!(
            app.sidebar_expanded, before,
            "the replaced default must not still act"
        );
        let _ = app.update(Message::KeyPressed(
            Key::Character("l".into()),
            Modifiers::CTRL,
        ));
        assert_ne!(app.sidebar_expanded, before, "the new binding acts instead");
    }
    #[test]
    fn an_armed_capture_turns_the_next_press_into_the_rows_binding() {
        use keyboard::{Key, Modifiers};
        let (mut app, _receiver) = app();
        let _ = app.update(Message::Settings(crate::settings::Action::CaptureShortcut(
            "native.find".to_owned(),
        )));
        assert_eq!(app.settings.capture_for.as_deref(), Some("native.find"));
        let _ = app.update(Message::KeyPressed(
            Key::Character("g".into()),
            Modifiers::CTRL,
        ));
        assert!(app.settings.capture_for.is_none(), "a captured key disarms");
        let binding = app
            .settings
            .keybindings
            .bindings("native.find")
            .expect("the row keeps a binding");
        assert_eq!(binding.len(), 1);
        assert_eq!(crate::keybindings::display(&binding[0]), "Ctrl+G");
    }
    #[test]
    fn the_editors_row_actions_run_through_their_real_action_messages() {
        let (mut app, _receiver) = app();
        let _ = app.update(Message::Settings(crate::settings::Action::ClearShortcut(
            "native.find".to_owned(),
        )));
        assert!(
            app.settings
                .keybindings
                .bindings("native.find")
                .is_some_and(<[_]>::is_empty),
            "Remove clears the seeded binding"
        );
        let _ = app.update(Message::Settings(crate::settings::Action::ResetShortcut(
            "native.find".to_owned(),
        )));
        assert!(
            !app.settings
                .keybindings
                .bindings("native.find")
                .is_some_and(<[_]>::is_empty),
            "Restore default returns the seeded list"
        );
        let _ = app.update(Message::Settings(crate::settings::Action::ResetAllDefaults));
        assert!(app.settings.reset_all_armed);
        let _ = app.update(Message::Settings(crate::settings::Action::ConfirmResetAll(
            false,
        )));
        assert!(
            !app.settings.reset_all_armed,
            "Cancel disarms and changes nothing"
        );
    }
    #[test]
    fn the_permission_default_rows_action_reaches_its_gated_pick() {
        let (mut app, mut receiver) = app();
        app.permission_settings = Some(dsh_native_core::PermissionSettingsState {
            current_value: "ask".to_owned(),
            revision: 2,
            writable: true,
        });
        let _ = app.update(Message::Settings(
            crate::settings::Action::PermissionDefault("workspace-write".to_owned()),
        ));
        assert!(app.permission_default_saving);
        assert!(matches!(
            receiver.try_recv(),
            Ok(Command::PermissionDefaultSet {
                name,
                expected_revision: 2
            }) if name == "workspace-write"
        ));
    }
    #[test]
    fn the_shortcuts_trigger_opens_the_general_page_with_the_editor_showing() {
        let (mut app, _receiver) = app();
        let _ = app.update(Message::ShortcutsOpen);
        assert!(app.settings.is_open());
        assert_eq!(app.settings.page(), crate::settings::Page::General);
        assert!(app.settings.shortcuts_open);
    }
    #[test]
    fn a_refused_capture_keeps_the_arming_and_names_the_reason() {
        use keyboard::key::Named;
        use keyboard::{Key, Modifiers};
        let (mut app, _receiver) = app();
        let _ = app.update(Message::Settings(crate::settings::Action::CaptureShortcut(
            "native.find".to_owned(),
        )));
        // A modifier alone is ignored, not called unsupported.
        let _ = app.update(Message::KeyPressed(
            Key::Named(Named::Shift),
            Modifiers::SHIFT,
        ));
        assert_eq!(app.settings.capture_for.as_deref(), Some("native.find"));
        assert!(app.settings.capture_error.is_none());
        // Another command already holds Ctrl+B: refuse, stay armed, name the holder.
        let _ = app.update(Message::KeyPressed(
            Key::Character("b".into()),
            Modifiers::CTRL,
        ));
        assert_eq!(app.settings.capture_for.as_deref(), Some("native.find"));
        assert_eq!(
            app.settings.capture_error.as_deref(),
            Some("Already used by “Navigation”")
        );
        // A printable key without Ctrl/Alt is refused with the reference's copy.
        let _ = app.update(Message::KeyPressed(
            Key::Character("x".into()),
            Modifiers::empty(),
        ));
        assert_eq!(
            app.settings.capture_error.as_deref(),
            Some(crate::keybindings::COPY_MODIFIER_REQUIRED)
        );
        // A key the app cannot encode is named as unsupported.
        let _ = app.update(Message::KeyPressed(
            Key::Named(Named::F1),
            Modifiers::empty(),
        ));
        assert_eq!(
            app.settings.capture_error.as_deref(),
            Some(crate::keybindings::COPY_UNSUPPORTED_KEY)
        );
        // A valid, free combination saves, disarms and clears the message.
        let _ = app.update(Message::KeyPressed(
            Key::Character("g".into()),
            Modifiers::CTRL,
        ));
        assert!(app.settings.capture_for.is_none());
        assert!(app.settings.capture_error.is_none());
        let binding = app
            .settings
            .keybindings
            .bindings("native.find")
            .expect("the row keeps a binding");
        assert_eq!(crate::keybindings::display(&binding[0]), "Ctrl+G");
    }
    #[test]
    fn a_steer_preference_steers_a_send_while_the_session_runs() {
        for (preference, expected) in [
            (
                crate::settings::BusyEnter::Steer,
                dsh_native_transport::dto::PromptMode::Steer,
            ),
            (
                crate::settings::BusyEnter::Queue,
                dsh_native_transport::dto::PromptMode::Queue,
            ),
        ] {
            let (mut app, mut receiver) = managed_app();
            let session = app.selected.clone().unwrap();
            app.sessions.get_mut(&session).unwrap().running = true;
            app.follow_ready = true;
            app.busy_enter = preference;
            app.editor = text_editor::Content::with_text("PUBLIC queued message");
            let _ = app.update(Message::Send);
            let mut request = None;
            while let Ok(command) = receiver.try_recv() {
                if let Command::Prompt { request: sent, .. } = command {
                    request = Some(sent);
                    break;
                }
            }
            let request =
                request.unwrap_or_else(|| panic!("a running session accepts a second prompt"));
            assert!(
                matches!((request.mode, expected), (mode, expected) if std::mem::discriminant(&mode) == std::mem::discriminant(&expected)),
                "{preference:?}"
            );
        }
    }
    #[test]
    fn the_header_lists_direct_children_and_opens_one() {
        let (mut app, _receiver) = managed_app();
        let session = app.selected.clone().unwrap();
        let child = SessionId::new("public-child-reviewer").unwrap();
        let idle = SessionId::new("public-child-scout").unwrap();
        for (id, running, title) in [
            (child.clone(), true, "PUBLIC reviewer"),
            (idle.clone(), false, "PUBLIC scout"),
        ] {
            app.sessions.insert(
                id.clone(),
                SessionSummary {
                    agent_available: true,
                    session_id: id,
                    updated_at: 0,
                    running,
                    blank: false,
                    parent_session_id: Some(session.clone()),
                    origin: Some(dsh_native_transport::dto::SubagentOrigin::Subagent),
                    cwd: None,
                    projections: Some(
                        serde_json::from_value(serde_json::json!({
                            "kind":"cached","asOfSeq":-1,"values":{"title":title}
                        }))
                        .unwrap(),
                    ),
                },
            );
        }
        // The manual session above never went through `choose`, so bind the transcript to the
        // generation the frame will carry.
        app.transcript = crate::reducer::Transcript::new(app.generation);
        let frame: SessionFollowFrame = serde_json::from_value(serde_json::json!({
            "type":"snapshot","records":[
                {"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                    "surfaceOp":"append","data":{"id":"PUBLIC-user","role":"user",
                        "source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC"}]}}}
            ],"cursor":0,"hasMore":false,
            "header":{"version":4,"id":"public-managed-session","createdAt":1000,"isSeeded":false},
            "projections":{"asOfSeq":0,"values":{"subagentCatalog":[
                {"id":"public-child-reviewer","createdAt":1000,"mode":"continuable","label":"PUBLIC reviewer"},
                {"id":"public-child-scout","createdAt":1001,"mode":"one-shot","label":"PUBLIC scout"}
            ]}},"assistantStream":{"revision":0}
        }))
        .unwrap();
        let _ = app.update(Message::Worker(Event::Selected {
            generation: app.generation,
            frame,
        }));
        let entries = app.subagent_entries();
        assert_eq!(entries.len(), 2);
        assert!(app.subagent_running("public-child-reviewer"));
        assert!(!app.subagent_running("public-child-scout"));
        assert!(
            app.subagent_action().is_some(),
            "a root catalog shows its action"
        );
        let _ = app.update(Message::ToggleSubagentMenu(true));
        assert!(app.subagent_menu);
        let _ = app.update(Message::SubagentOpen("public-child-reviewer".into()));
        assert!(!app.subagent_menu, "opening a child closes the dropdown");
        assert_eq!(app.selected.as_ref(), Some(&child));
    }
    #[test]
    fn the_sidebar_hides_child_sessions_and_counts_running_ones() {
        let (mut app, _receiver) = managed_app();
        let parent = app.selected.clone().unwrap();
        let child = SessionId::new("public-child-reviewer").unwrap();
        app.sessions.insert(
            child.clone(),
            SessionSummary {
                agent_available: true,
                session_id: child.clone(),
                updated_at: 0,
                running: true,
                blank: false,
                parent_session_id: Some(parent.clone()),
                origin: Some(dsh_native_transport::dto::SubagentOrigin::Subagent),
                cwd: None,
                projections: None,
            },
        );
        app.sessions.get_mut(&parent).unwrap().projections = Some(
            serde_json::from_value(serde_json::json!({
                "kind":"cached","asOfSeq":-1,
                "values":{"subagentCatalog":[
                    {"id":"public-child-reviewer","createdAt":1,"mode":"continuable","label":"PUBLIC reviewer"}
                ]}
            }))
            .unwrap(),
        );
        let visible: Vec<_> = app
            .navigation_candidates()
            .map(|session| session.session_id.clone())
            .collect();
        assert!(visible.contains(&parent), "the parent keeps its row");
        assert!(
            !visible.contains(&child),
            "a child is reached through the parent catalog, not the list"
        );
        assert_eq!(app.running_subagent_count(&parent), 1);
        assert_eq!(subagent_catalog::count_label(1, 1), "1 subagent running");
    }
    #[test]
    fn a_catalog_branch_toggles_and_reads_grandchildren() {
        let (mut app, _receiver) = managed_app();
        let reviewer = SessionId::new("public-child-reviewer").unwrap();
        let helper = SessionId::new("public-grandchild-helper").unwrap();
        app.sessions.insert(
            reviewer.clone(),
            SessionSummary {
                agent_available: true,
                session_id: reviewer.clone(),
                updated_at: 0,
                running: false,
                blank: false,
                parent_session_id: app.selected.clone(),
                origin: Some(dsh_native_transport::dto::SubagentOrigin::Subagent),
                cwd: None,
                projections: Some(
                    serde_json::from_value(serde_json::json!({
                        "kind":"cached","asOfSeq":-1,
                        "values":{"subagentCatalog":[
                            {"id":"public-grandchild-helper","createdAt":1,"mode":"one-shot","label":"PUBLIC helper"}
                        ]}
                    }))
                    .unwrap(),
                ),
            },
        );
        app.sessions.insert(
            helper.clone(),
            SessionSummary {
                agent_available: true,
                session_id: helper,
                updated_at: 0,
                running: false,
                blank: false,
                parent_session_id: Some(reviewer.clone()),
                origin: Some(dsh_native_transport::dto::SubagentOrigin::Subagent),
                cwd: None,
                projections: None,
            },
        );
        let grandchildren = app.catalog_for("public-child-reviewer");
        assert_eq!(grandchildren.len(), 1);
        assert_eq!(grandchildren[0].label, "PUBLIC helper");
        assert!(app.catalog_for("public-grandchild-helper").is_empty());
        let _ = app.update(Message::ToggleSubagentMenu(true));
        let _ = app.update(Message::ToggleSubagentBranch(
            "public-child-reviewer".into(),
        ));
        assert!(app.subagent_expanded.contains("public-child-reviewer"));
        let _ = app.update(Message::ToggleSubagentBranch(
            "public-child-reviewer".into(),
        ));
        assert!(app.subagent_expanded.is_empty());
        // Closing the dropdown resets every expanded branch, as the reference does.
        let _ = app.update(Message::ToggleSubagentBranch(
            "public-child-reviewer".into(),
        ));
        let _ = app.update(Message::ToggleSubagentMenu(false));
        assert!(app.subagent_expanded.is_empty());
    }
    #[test]
    fn a_child_header_reads_the_parents_sibling_catalog() {
        let (mut app, _receiver) = managed_app();
        let parent = app.selected.clone().unwrap();
        app.sessions.get_mut(&parent).unwrap().projections = Some(
            serde_json::from_value(serde_json::json!({
                "kind":"cached","asOfSeq":-1,
                "values":{"subagentCatalog":[
                    {"id":"public-child-reviewer","createdAt":1,"mode":"continuable","label":"PUBLIC reviewer"},
                    {"id":"public-child-scout","createdAt":2,"mode":"one-shot","label":"PUBLIC scout"}
                ]}
            }))
            .unwrap(),
        );
        let child = SessionId::new("public-child-reviewer").unwrap();
        app.sessions.insert(
            child.clone(),
            SessionSummary {
                agent_available: true,
                session_id: child.clone(),
                updated_at: 0,
                running: false,
                blank: false,
                parent_session_id: Some(parent.clone()),
                origin: Some(dsh_native_transport::dto::SubagentOrigin::Subagent),
                cwd: None,
                projections: None,
            },
        );
        app.selected = Some(child);
        let siblings = app.parent_catalog().expect("the parent's catalog");
        assert_eq!(siblings.len(), 2);
        assert_eq!(siblings[0].label, "PUBLIC reviewer");
        // A parent session has no sibling switcher of its own.
        app.selected = Some(parent);
        assert!(app.parent_catalog().is_none());
    }
    #[test]
    fn a_one_shot_child_replaces_the_composer_and_a_continuable_child_does_not() {
        let (mut app, _receiver) = managed_app();
        let session = app.selected.clone().unwrap();
        app.sessions.get_mut(&session).unwrap().origin =
            Some(dsh_native_transport::dto::SubagentOrigin::Subagent);
        let frame = |mode: &str| -> SessionFollowFrame {
            serde_json::from_value(serde_json::json!({
                "type":"snapshot","records":[
                    {"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                        "surfaceOp":"append","data":{"id":"PUBLIC-user","role":"user",
                            "source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC"}]}}},
                    {"type":"event","event":{"type":"subagent/descriptor","seq":1,"time":1001,
                        "data":{"version":3,"provider":"PUBLIC","mode":mode,
                            "label":"PUBLIC child"}}}
                ],"cursor":1,"hasMore":false,
                "header":{"version":4,"id":"public-managed-session","createdAt":1000,"isSeeded":false},
                "projections":{"asOfSeq":1,"values":{}},"assistantStream":{"revision":0}
            }))
            .unwrap()
        };
        for (mode, read_only) in [("one-shot", true), ("continuable", false)] {
            app.transcript = crate::reducer::Transcript::new(app.generation);
            let _ = app.update(Message::Worker(Event::Selected {
                generation: app.generation,
                frame: frame(mode),
            }));
            assert_eq!(
                app.subagent_read_only_copy().is_some(),
                read_only,
                "{mode} child"
            );
        }
        // A parent session with the same descriptor is not a catalog-addressed child.
        app.sessions.get_mut(&session).unwrap().origin = None;
        assert!(app.subagent_read_only_copy().is_none());
    }
    #[test]
    fn two_escape_presses_stop_a_running_turn_and_one_only_arms() {
        use keyboard::key::Named;
        use keyboard::{Key, Modifiers};
        let escape = || Message::KeyPressed(Key::Named(Named::Escape), Modifiers::empty());
        let (mut app, mut receiver) = managed_app();
        let session = app.selected.clone().unwrap();
        app.sessions.get_mut(&session).unwrap().running = true;
        let _ = app.update(escape());
        assert!(
            receiver.try_recv().is_err(),
            "the first press only arms the sequence"
        );
        let _ = app.update(escape());
        assert!(matches!(
            receiver.try_recv(),
            Ok(Command::Cancel { session: cancelled, .. }) if cancelled == session
        ));
        assert!(app.stop_sequence.is_none());
        assert!(app.cancel_pending);
    }
    #[test]
    fn an_expired_first_escape_does_not_stop_and_starts_over() {
        use keyboard::key::Named;
        use keyboard::{Key, Modifiers};
        let (mut app, mut receiver) = managed_app();
        let session = app.selected.clone().unwrap();
        app.sessions.get_mut(&session).unwrap().running = true;
        app.stop_sequence = Some((
            Instant::now() - Duration::from_millis(1),
            session,
            app.generation,
        ));
        let _ = app.update(Message::KeyPressed(
            Key::Named(Named::Escape),
            Modifiers::empty(),
        ));
        assert!(receiver.try_recv().is_err(), "an expired press never stops");
        assert!(app.stop_sequence.is_some(), "it starts a new first press");
    }
    #[test]
    fn escape_outside_the_conversation_surface_still_dismisses() {
        use keyboard::key::Named;
        use keyboard::{Key, Modifiers};
        let (mut app, mut receiver) = managed_app();
        let session = app.selected.clone().unwrap();
        app.sessions.get_mut(&session).unwrap().running = true;
        app.work_open = true;
        let _ = app.update(Message::KeyPressed(
            Key::Named(Named::Escape),
            Modifiers::empty(),
        ));
        assert!(!app.work_open, "the cover's Escape still closes it");
        assert!(app.stop_sequence.is_none(), "no stop sequence was armed");
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn the_shortcuts_search_is_local_bounded_and_clears() {
        let (mut app, _receiver) = app();
        let _ = app.update(Message::Settings(crate::settings::Action::ShortcutsSearch(
            "send".to_owned(),
        )));
        assert_eq!(app.settings.shortcuts_query, "send");
        // The field is bounded like every other text surface; an oversized value is ignored
        // rather than truncated into a stale filter.
        let oversized = "x".repeat(129);
        let _ = app.update(Message::Settings(crate::settings::Action::ShortcutsSearch(
            oversized,
        )));
        assert_eq!(app.settings.shortcuts_query, "send");
        let _ = app.update(Message::Settings(crate::settings::Action::ShortcutsSearch(
            String::new(),
        )));
        assert!(app.settings.shortcuts_query.is_empty());
    }
    #[test]
    fn worker_job_events_reach_the_panel_state() {
        let (mut app, _receiver) = app();
        assert!(app.jobs.is_empty());
        let _ = app.update(Message::Worker(Event::JobsRows(vec![
            dsh_native_transport::dto::JobView {
                id: "shell-1".to_owned(),
                kind: "shell".to_owned(),
                label: "Run the test suite".to_owned(),
                owner: None,
                status: "running".to_owned(),
            },
        ])));
        assert_eq!(app.jobs.len(), 1, "the Host's list is the panel's list");
        let _ = app.update(Message::Worker(Event::JobsFollow {
            job: "shell-1".to_owned(),
            frame: dsh_native_transport::dto::JobFollowFrame::Output {
                chunks: vec![dsh_native_transport::dto::JobChunk {
                    at: 1,
                    text: "hello".to_owned(),
                    channel: None,
                    gap_before: None,
                }],
                next: 1,
                lossy: None,
            },
        }));
        assert_eq!(
            app.jobs_output.get("shell-1").map(String::as_str),
            Some("hello"),
            "a followed job's bytes reach its own buffer"
        );
        let _ = app.update(Message::Worker(Event::JobKilled(Err(
            "Could not stop the job".to_owned(),
        ))));
        assert_eq!(
            app.jobs_kill_error.as_deref(),
            Some("Could not stop the job")
        );
    }
    #[test]
    fn worker_permission_and_roster_events_reach_their_surfaces() {
        let (mut app, _receiver) = app();
        let _ = app.update(Message::Worker(Event::PermissionSettings(Ok(
            dsh_native_core::PermissionSettingsState {
                current_value: "ask".to_owned(),
                revision: 3,
                writable: true,
            },
        ))));
        assert!(app.permission_settings.is_some());
        app.permission_default_saving = true;
        let _ = app.update(Message::Worker(Event::PermissionDefaultSet(Err(
            "refused".to_owned()
        ))));
        assert!(!app.permission_default_saving, "a refusal settles the row");
        let _ = app.update(Message::Worker(Event::AgentPresets(Ok(
            dsh_native_core::AgentPresetRoster {
                presets: vec![dsh_native_core::AgentPresetRow {
                    id: "scout".to_owned(),
                    is_default: true,
                    name: Some("Scout".to_owned()),
                    description: None,
                    broken: None,
                }],
            },
        ))));
        assert!(app.agent_roster.is_some());
    }
    #[test]
    fn a_host_command_advertisement_reaches_the_editor_too() {
        let (mut app, _receiver) = app();
        let session = SessionId::new("PUBLIC-session").unwrap();
        app.selected = Some(session.clone());
        let _ = app.update(Message::Worker(Event::CommandsListed {
            session_id: session,
            result: Ok(vec![dsh_native_transport::dto::CommandDescriptor {
                definition_id: None,
                name: "session.compact".to_owned(),
                description: String::new(),
            }]),
        }));
        assert_eq!(
            app.settings.commands,
            vec![("session.compact".to_owned(), "session.compact".to_owned())],
            "the disclosure lists the same advertisement the slash menu completes"
        );
    }
    #[test]
    fn a_cleared_close_command_does_not_fall_back_to_escape() {
        use keyboard::{Key, Modifiers};
        let (mut app, _receiver) = app();
        app.work_open = true;
        let _ = app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert!(!app.work_open, "the default Escape still dismisses");
        app.work_open = true;
        let _ = app.settings.keybindings.apply(
            crate::keybindings::Edit::Set {
                id: "native.close".to_owned(),
                binding: None,
            },
            app.settings.keybindings.revision(),
        );
        let _ = app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert!(app.work_open, "a cleared command leaves its old key dead");
    }
    #[test]
    fn a_command_bound_to_ctrl_v_never_swallows_the_field_paste() {
        use keyboard::{Key, Modifiers};
        let (mut app, _receiver) = app();
        let _ = app.settings.keybindings.apply(
            crate::keybindings::Edit::Set {
                id: "native.navigation".to_owned(),
                binding: Some(
                    crate::keybindings::binding_from(&Key::Character("v".into()), Modifiers::CTRL)
                        .unwrap(),
                ),
            },
            app.settings.keybindings.revision(),
        );
        let before = app.sidebar_expanded;
        let _ = app.update(Message::KeyPressed(
            Key::Character("v".into()),
            Modifiers::CTRL,
        ));
        assert_eq!(
            app.sidebar_expanded, before,
            "Ctrl+V belongs to the field link, not to a binding"
        );
    }
    #[test]
    fn the_focused_composer_follows_the_live_send_binding() {
        let mut store = bindings();
        assert!(matches!(
            composer_binding(enter(keyboard::Modifiers::empty()), true, false, &store),
            Some(text_editor::Binding::Custom(Message::KeyboardSend))
        ));
        // Move Send onto Ctrl+J: Enter must stop sending, and the new key must send.
        let _ = store.apply(
            crate::keybindings::Edit::Set {
                id: "native.send".to_owned(),
                binding: Some(
                    crate::keybindings::binding_from(
                        &keyboard::Key::Character("j".into()),
                        keyboard::Modifiers::CTRL,
                    )
                    .unwrap(),
                ),
            },
            store.revision(),
        );
        assert!(!matches!(
            composer_binding(enter(keyboard::Modifiers::empty()), true, false, &store),
            Some(text_editor::Binding::Custom(Message::KeyboardSend))
        ));
        assert!(matches!(
            composer_binding(press("j", keyboard::Modifiers::CTRL), true, false, &store),
            Some(text_editor::Binding::Custom(Message::KeyboardSend))
        ));
    }
    #[test]
    fn clipboard_field_shortcut_answers_only_bare_ctrl_c_and_ctrl_v() {
        let c = keyboard::Key::Character("c".into());
        let v = keyboard::Key::Character("V".into());
        assert!(matches!(
            clipboard_field_shortcut(&c, keyboard::Modifiers::CTRL),
            Some(Message::ClipboardCopyField)
        ));
        assert!(matches!(
            clipboard_field_shortcut(&v, keyboard::Modifiers::CTRL),
            Some(Message::ClipboardPasteField)
        ));
        for modifiers in [
            keyboard::Modifiers::SHIFT,
            keyboard::Modifiers::ALT,
            keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT,
            keyboard::Modifiers::CTRL | keyboard::Modifiers::ALT,
            keyboard::Modifiers::empty(),
        ] {
            assert!(clipboard_field_shortcut(&c, modifiers).is_none());
            assert!(clipboard_field_shortcut(&v, modifiers).is_none());
        }
        assert!(
            clipboard_field_shortcut(
                &keyboard::Key::Named(keyboard::key::Named::Escape),
                keyboard::Modifiers::CTRL
            )
            .is_none()
        );
    }

    #[test]
    fn paste_shortcut_matches_only_unmodified_ctrl_or_command_v() {
        let lower = keyboard::Key::Character("v".into());
        let upper = keyboard::Key::Character("V".into());
        assert!(paste_shortcut(&lower, keyboard::Modifiers::CTRL));
        assert!(paste_shortcut(&upper, keyboard::Modifiers::CTRL));
        for modifiers in [
            keyboard::Modifiers::empty(),
            keyboard::Modifiers::SHIFT,
            keyboard::Modifiers::ALT,
            keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT,
        ] {
            assert!(!paste_shortcut(&lower, modifiers));
        }
        assert!(!paste_shortcut(
            &keyboard::Key::Character("c".into()),
            keyboard::Modifiers::CTRL
        ));
    }
    #[test]
    fn deepseek_route_matches_route_or_model_id_case_insensitively() {
        let model = |provider: &str, id: &str| Model {
            provider: provider.into(),
            id: id.into(),
            name: "PUBLIC".into(),
            label: "PUBLIC".into(),
            efforts: Vec::new(),
            default_effort: None,
            service_tiers: Vec::new(),
            verbosity_levels: Vec::new(),
        };
        assert!(deepseek_route(&model(
            "deepseek-official",
            "deepseek-flash"
        )));
        assert!(deepseek_route(&model("DEEPSEEK-OFFICIAL", "x")));
        assert!(deepseek_route(&model("other", "deepseek-v4-pro")));
        assert!(!deepseek_route(&model("openai-codex", "gpt-6")));
    }
    #[test]
    fn settings_paste_shortcut_needs_a_wayland_session_a_draft_and_ctrl_v() {
        let ticket = crate::settings::Ticket {
            epoch: 1,
            panel: 0,
            serial: 7,
        };
        let v = keyboard::Key::Character("v".into());
        // No live Models draft: the shortcut stays out of the subscription.
        assert!(settings_paste_message(true, None, &v, keyboard::Modifiers::CTRL).is_none());
        // Non-Wayland sessions keep Iced's own clipboard path.
        assert!(
            settings_paste_message(false, Some(ticket), &v, keyboard::Modifiers::CTRL).is_none()
        );
        for modifiers in [
            keyboard::Modifiers::empty(),
            keyboard::Modifiers::SHIFT,
            keyboard::Modifiers::ALT,
        ] {
            assert!(settings_paste_message(true, Some(ticket), &v, modifiers).is_none());
        }
        assert!(
            settings_paste_message(
                true,
                Some(ticket),
                &keyboard::Key::Character("c".into()),
                keyboard::Modifiers::CTRL
            )
            .is_none()
        );
        match settings_paste_message(true, Some(ticket), &v, keyboard::Modifiers::CTRL) {
            Some(Message::Settings(crate::settings::Action::Paste { ticket: got })) => {
                assert_eq!(got, ticket);
            }
            _ => panic!("expected a Models paste action"),
        }
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
        app.open_detail("Fixture".into(), "read-only".into(), None, false);
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
            input: crate::settings::key_input("PUBLIC_MODAL_FAKE_ONLY".into()),
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
    include!("catalog_ui_tests.rs");
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
        assert!(matches!(receiver.try_recv(), Ok(Command::Catalog(_))));
        let _ = window.view();
        assert!(window.ready);
        assert_eq!(window.account, "Account metadata not loaded");
        // The startup catalog is the only automatic read; rendering never requests account data.
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
                attempt: None,
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
        assert!(matches!(receiver.try_recv(), Ok(Command::Catalog(_))));
        assert!(receiver.try_recv().is_err());
        let account = PublicEvent::Account(dsh_native_core::AccountMetadata {
            status: dsh_native_core::AccountStatus::CredentialStored,
            phase: Some(dsh_native_core::AttemptPhase::Succeeded),
            attempt: None,
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
                attempt: None,
            },
        )));
        assert_eq!(window.account, "Account: CredentialStored · Succeeded");
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn general_reads_its_namespace_only_while_open_and_enabled() {
        use crate::settings::{Action, Page};
        let (mut app, mut receiver) = app();
        let _ = app.update(Message::Settings(Action::Open));
        let _ = app.update(Message::Settings(Action::SelectPage(Page::General)));
        assert!(
            receiver.try_recv().is_err(),
            "a stopped app requests no namespace read"
        );
        let _ = app.receive(Event::Ready("0.2.1-alpha.1".into()));
        assert!(matches!(receiver.try_recv(), Ok(Command::Catalog(_))));
        let _ = app.update(Message::Settings(Action::Close));
        let _ = app.update(Message::Settings(Action::Open));
        let Command::PluginsRead(read) = receiver.try_recv().expect("General reads its namespace")
        else {
            panic!("wrong command")
        };
        assert_eq!(read.epoch, worker::TRANSPORT_EPOCH);
        assert!(receiver.try_recv().is_err(), "one read per open, no retry");
    }
    #[test]
    fn plugins_and_general_read_only_from_their_own_visible_section() {
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
        assert!(matches!(receiver.try_recv(), Ok(Command::Catalog(_))));
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
        let Command::PluginsRead(general) =
            receiver.try_recv().expect("General reads its namespace")
        else {
            panic!("wrong command")
        };
        assert_eq!(general.epoch, worker::TRANSPORT_EPOCH);
        let _ = app.update(Message::Settings(Action::Plugins(
            crate::plugins::Action::Refresh(ticket),
        )));
        let _ = app.update(Message::Settings(Action::Close));
        let _ = app.update(Message::Settings(Action::Plugins(
            crate::plugins::Action::Refresh(ticket),
        )));
        assert!(receiver.try_recv().is_err());
        assert!(app.smoke_evidence.catalog_requested); // Only the drained startup catalog.
        assert_eq!(app.smoke_evidence.model_prompts, 0);
    }
    #[test]
    fn plugin_actual_ui_queue_rejection_settles_read_without_auto_retry() {
        use crate::settings::{Action, Page};
        let (mut app, mut receiver) = app();
        let _ = app.update(Message::Settings(Action::Open));
        let _ = app.update(Message::Settings(Action::SelectPage(Page::Plugins)));
        let _ = app.receive(Event::Ready("0.2.1-alpha.1".into()));
        assert!(matches!(receiver.try_recv(), Ok(Command::Catalog(_))));
        assert!(receiver.try_recv().is_err());
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
        assert!(matches!(receiver.try_recv(), Ok(Command::Catalog(_))));
        assert!(app.settings.is_open());
        assert_eq!(app.settings.input_ticket().epoch, worker::TRANSPORT_EPOCH);
        assert!(app.settings_enabled());
        // Startup catalog loading must not cause Settings/key/plugin reads.
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn width_handle_hover_lights_only_the_side_the_pointer_left() {
        let (mut app, _) = app();
        assert_eq!(app.width_hover, None);
        let _ = app.update(Message::WidthHandleHover(design::WidthSide::Left, true));
        assert_eq!(app.width_hover, Some(design::WidthSide::Left));
        // An exit for the other side cannot clear a live hover.
        let _ = app.update(Message::WidthHandleHover(design::WidthSide::Right, false));
        assert_eq!(app.width_hover, Some(design::WidthSide::Left));
        let _ = app.update(Message::WidthHandleHover(design::WidthSide::Left, false));
        assert_eq!(app.width_hover, None);
        // A drag lights its own side independent of hover, as the web's
        // `[data-dragging]` does.
        let _ = app.update(Message::WidthDragStart(design::WidthSide::Right));
        assert!(
            app.width_drag
                .as_ref()
                .is_some_and(|drag| drag.side == design::WidthSide::Right)
        );
    }
    #[test]
    fn failed_backend_start_leaves_a_bounded_crash_record() {
        let (mut app, _) = app();
        let home = std::env::temp_dir().join(format!("dsh-fault-record-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        app.native_home = Some(home.clone());
        // The production path: `worker::run` publishes this exact shape before it
        // publishes `Event::Stopped(Err(..))`, and `NoBackendStarted` is only the
        // already-closing branch.
        let _ = app.receive(Event::Fault("Core start: PUBLIC backend cause".into()));
        let _ = app.receive(Event::Stopped(Err(
            "Core did not start; inspect startup error".into(),
        )));
        let names: Vec<String> = std::fs::read_dir(home.join("logs"))
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
        let text = std::fs::read_to_string(home.join("logs").join(&names[0])).unwrap();
        assert!(text.contains("source: host-start"), "{text}");
        // The report carries the cause, and the status keeps showing it rather
        // than a path.
        assert!(text.contains("Core start: PUBLIC backend cause"), "{text}");
        assert_eq!(app.status, "Core did not start; inspect startup error");
        std::fs::remove_dir_all(&home).unwrap();
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
    fn send_is_typed_and_failed_acceptance_preserves_draft_and_identity() {
        let (mut app, mut receiver) = app();
        app.transport_epoch = worker::TRANSPORT_EPOCH;
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
        let request_id = app.pending.ticket().unwrap().request_id;
        let _ = app.receive(Event::Prompt {
            generation: 0,
            request_id,
            result: Err("session/model-unavailable".into()),
        });
        assert_eq!(app.editor.text(), "hello 世界");
        assert!(!app.pending.is_empty());
        assert!(!app.send_allowed());
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
