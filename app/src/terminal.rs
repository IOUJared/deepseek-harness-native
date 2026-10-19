//! Native terminal panel: a client-only view of Host-owned shells.
//!
//! The app never spawns a shell or child process. Opening the panel only reads the
//! Host's terminal list, shell profiles and limits. Creating, closing and writing
//! are explicit operator actions: every admission is fenced to the panel's current
//! transport epoch, session generation and serial; no state is predicted before the
//! Host ACK, no write is retried, and an unknown outcome stays indeterminate.
use crate::management::{Ticket, View};
use dsh_native_transport::dto::SessionId;
use dsh_native_transport::terminal::{
    MAX_INPUT_BYTES, TerminalAttachmentId, TerminalEnvironment, TerminalFrame, TerminalShell,
    WebTerminalId, WebTerminalInfo,
};

/// Default PTY dimensions requested for a new Host terminal.
pub const DEFAULT_COLS: u64 = 80;
pub const DEFAULT_ROWS: u64 = 24;
/// Bounded panel output: at most this many retained chunks and bytes.
pub const OUTPUT_CHUNKS: usize = 400;
pub const OUTPUT_BYTES: usize = 128 * 1024;
/// One sanitized output chunk is truncated to this many bytes.
pub const CHUNK_BYTES: usize = 16 * 1024;
/// Newest output bytes rendered in the panel; the retained buffer may be larger.
pub const DISPLAY_BYTES: usize = 32 * 1024;

/// Host-observed data for one panel open. Missing/oversized values are already
/// refused by the transport, so this is only assembled from confirmed reads.
/// `environment` is absent when the Session has no registered Agent: the list is
/// still Host-confirmed history, but create and write are unavailable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loaded {
    pub list: Vec<WebTerminalInfo>,
    pub shells: Vec<TerminalShell>,
    pub environment: Option<TerminalEnvironment>,
}
/// Redacted read failure carried by the worker; no remote message is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadFailure {
    Unavailable,
    Rejected,
}
/// Redacted create failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateFailure {
    Unavailable,
    LimitReached,
    NotSent,
    Unknown,
}
/// Redacted close failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseFailure {
    Unavailable,
    NotSent,
    Unknown,
}
/// Redacted write failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteFailure {
    ControlUnavailable,
    Unavailable,
    NotSent,
    Unknown,
}

/// One panel read identity, fenced to the open view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadTicket {
    pub epoch: u64,
    pub generation: u64,
    pub serial: u64,
    pub target: SessionId,
}
/// Who the Host named as the input controller of the selected terminal.
///
/// `follow` sets `info.controllerId` to the attaching client's own id before it yields the
/// baseline (`api/terminal-controller/src/terminal.ts:77-84`), so a frame naming this panel's
/// attachment is the Host's confirmation — the panel never asserts it alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// The Host's latest frame names this panel's attachment.
    Held,
    /// Another attachment holds input control; this panel is read-only until it reattaches.
    Elsewhere,
    /// No frame has said yet.
    Unconfirmed,
}
/// One live follow identity: the exact Host attachment this panel owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowTicket {
    pub epoch: u64,
    pub generation: u64,
    pub serial: u64,
    pub target: SessionId,
    pub id: WebTerminalId,
    pub attachment: TerminalAttachmentId,
}
/// One reviewed create attempt, fenced to the panel and the exact new identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateReceipt {
    pub ticket: Ticket,
    pub id: WebTerminalId,
    pub attempt: u64,
}
/// One reviewed close attempt, fenced to the panel and the exact terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseReceipt {
    pub ticket: Ticket,
    pub id: WebTerminalId,
    pub attempt: u64,
}
/// One admitted input write, fenced to the panel attachment and the input revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteReceipt {
    pub ticket: Ticket,
    pub id: WebTerminalId,
    pub attachment: TerminalAttachmentId,
    pub attempt: u64,
    pub editor: u64,
}
/// The reviewed-but-not-submitted create. No shell path or identity is retained here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Review {
    pub ticket: Ticket,
}
/// The reviewed close target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseReview {
    pub ticket: Ticket,
    pub id: WebTerminalId,
}
/// One consuming reviewed create. The requested shell and dimensions are captured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateSubmission {
    pub receipt: CreateReceipt,
    pub shell_path: Option<String>,
    pub cols: u64,
    pub rows: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseSubmission {
    pub receipt: CloseReceipt,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteSubmission {
    pub receipt: WriteReceipt,
    data: String,
}
impl WriteSubmission {
    pub fn data(&self) -> &str {
        &self.data
    }
}
/// Outcome reported by the worker for one create attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreateOutcome {
    /// Boxed: the Host's terminal facts dwarf every other variant, and this
    /// outcome is carried in results alongside them.
    Created(Box<WebTerminalInfo>),
    Unavailable,
    LimitReached,
    NotSent,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloseOutcome {
    Closed,
    Unavailable,
    NotSent,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    Sent,
    Refused,
    NotSent,
    Unknown,
}
/// The acknowledged new terminal plus the follow this panel now wants to open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateApplied {
    pub info: WebTerminalInfo,
    pub follow: FollowTicket,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    None,
    Loading,
    ReadFailed,
    /// Host limits are shown so the operator sees the actual input bound.
    Limits {
        max_input_bytes: u64,
        max_cols: u64,
        max_rows: u64,
    },
    /// The Session has no registered Agent: history lists, but create/write cannot run.
    LimitsUnavailable,
    /// Another terminal operation is outstanding, so a drawn control's press was not admitted.
    Busy,
    /// The Host attachment this panel held has ended; nothing can be written until it reattaches.
    Detached,
    /// Attached, and waiting for the Host's frame to name this panel the controller.
    FollowPending,
    /// Another attachment holds input control; input here is refused until the panel reattaches.
    ControlElsewhere,
    /// The output stream skipped a frame, so the screen no longer matches the Host's
    /// (`ui-sidebar-terminal/src/client/model.ts:294`, `locales.ts:42`).
    OutputGap,
    CreateReview,
    Creating,
    CreateFailed,
    CreateLimit,
    CreatingIndeterminate,
    CreateNotSent,
    CloseReview,
    Closing,
    Closed,
    CloseFailed,
    ClosingIndeterminate,
    CloseNotSent,
    WritePending,
    WriteRefused,
    WriteNotSent,
    WriteIndeterminate,
    InputRefused,
    FollowAttached,
    FollowEnded,
    FollowFailed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Read(ReadTicket),
    Follow(FollowTicket),
    /// Stop the follow this panel owned. Dismissing the panel must not leave the Host attachment
    /// and its stream running behind a surface that no longer shows them.
    Unfollow,
    Create(CreateSubmission),
    Close(CloseSubmission),
    Write(WriteSubmission),
}

/// A bounded, sanitized plain-text view of Host terminal output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    chunks: Vec<String>,
    bytes: usize,
    dropped: usize,
    /// Bytes a single oversized chunk lost at [`CHUNK_BYTES`]. Counted separately from whole-chunk
    /// eviction so the panel's "never silently hidden" copy stays true.
    truncated: usize,
}
impl Output {
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.bytes = 0;
        self.dropped = 0;
        self.truncated = 0;
    }
    /// Append one sanitized chunk, evicting the oldest chunks under both bounds.
    pub fn push(&mut self, data: &str) {
        let mut text = sanitize(data);
        if text.len() > CHUNK_BYTES {
            let mut cut = CHUNK_BYTES;
            while cut > 0 && !text.is_char_boundary(cut) {
                cut -= 1;
            }
            self.truncated = self.truncated.saturating_add(text.len() - cut);
            text.truncate(cut);
        }
        if text.is_empty() {
            return;
        }
        self.bytes = self.bytes.saturating_add(text.len());
        self.chunks.push(text);
        while self.chunks.len() > OUTPUT_CHUNKS || self.bytes > OUTPUT_BYTES {
            if self.chunks.len() <= 1 {
                break;
            }
            let removed = self.chunks.remove(0);
            self.bytes = self.bytes.saturating_sub(removed.len());
            self.dropped = self.dropped.saturating_add(1);
        }
    }
    /// Whole retained output, unbounded. The panel renders the byte-bounded
    /// [`Self::tail`]; only the retention-bound tests read this.
    #[cfg(test)]
    pub fn text(&self) -> String {
        self.chunks.concat()
    }
    /// Newest retained output bounded to `max_bytes`, for the rendered panel only.
    pub fn tail(&self, max_bytes: usize) -> String {
        let mut out = String::new();
        for chunk in self.chunks.iter().rev() {
            if out.len() + chunk.len() > max_bytes {
                break;
            }
            out.insert_str(0, chunk);
        }
        out
    }
    /// Raw retained chunks, for the tests that assert the retention bounds.
    #[cfg(test)]
    pub fn chunks(&self) -> &[String] {
        &self.chunks
    }
    pub fn dropped(&self) -> usize {
        self.dropped
    }
    /// Bytes discarded because one chunk exceeded [`CHUNK_BYTES`].
    pub fn truncated(&self) -> usize {
        self.truncated
    }
    /// Retained byte total, for the tests that assert the retention bounds.
    #[cfg(test)]
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

/// Strip terminal control sequences so a plain-text panel can render Host output.
fn sanitize(input: &str) -> String {
    let mut out = String::with_capacity(input.len().min(CHUNK_BYTES));
    let mut chars = input.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\u{1b}' => match chars.peek() {
                Some('[') => {
                    chars.next();
                    for c in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' {
                            break;
                        }
                        if c == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {
                    chars.next();
                }
            },
            '\n' => out.push('\n'),
            '\t' => out.push('\t'),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[derive(Clone)]
pub enum Action {
    /// Open the panel for one session view. Read-only: it never creates a terminal.
    Open { view: View },
    /// Dismiss the panel. Hides controls; never cancels or rolls back a write.
    Dismiss(Ticket),
    /// Re-read the Host list, shells and limits.
    Refresh(Ticket),
    /// Choose one Host terminal; the returned effect attaches a new input controller.
    Select { ticket: Ticket, id: WebTerminalId },
    /// Choose one discovered shell profile for the next create.
    ChooseShell { ticket: Ticket, index: usize },
    /// Replace the typed input line. Never sends.
    EditInput { ticket: Ticket, text: String },
    /// Explicit consent to send the input line to the selected Host terminal.
    Write(Ticket),
    /// One keystroke, sent as the bytes the key stands for. The reference pipes the terminal's own
    /// key handler to the Host (`ui-sidebar-terminal/src/client/terminal.tsx:101`), so Enter, arrows,
    /// Ctrl-C and Escape reach the shell instead of being typed into a line that never ends.
    Key { ticket: Ticket, data: String },
    /// Enter the create review.
    ReviewCreate(Ticket),
    /// Leave the create review without writing.
    BackCreate(Ticket),
    /// Confirm the reviewed create: one owned Host allocation, never retried.
    ConfirmCreate(Review),
    /// Enter the close review for one listed terminal.
    ReviewClose { ticket: Ticket, id: WebTerminalId },
    /// Leave the close review without writing.
    BackClose(Ticket),
    /// Confirm the reviewed close: one owned Host termination, never retried.
    ConfirmClose(CloseReview),
}
pub struct Context {
    pub view: Option<View>,
    pub allowed: bool,
}

struct Panel {
    ticket: Ticket,
}
pub struct Terminal {
    panel: Option<Panel>,
    serial: u64,
    read_serial: u64,
    follow_serial: u64,
    list: Vec<WebTerminalInfo>,
    list_loaded: bool,
    shells: Vec<TerminalShell>,
    shell: Option<usize>,
    environment: Option<TerminalEnvironment>,
    selected: Option<WebTerminalId>,
    attachment: Option<TerminalAttachmentId>,
    follow: Option<FollowTicket>,
    /// The last output frame's sequence, so a skipped one is noticed instead of shortening the
    /// screen (`ui-sidebar-terminal/src/client/model.ts:294`).
    last_sequence: Option<i64>,
    output: Output,
    input: String,
    editor: u64,
    max_input_bytes: u64,
    create_review: Option<Review>,
    create_pending: Option<CreateReceipt>,
    close_review: Option<CloseReview>,
    close_pending: Option<CloseReceipt>,
    write_pending: Option<WriteReceipt>,
    notice: Notice,
    attempt: u64,
    exhausted: bool,
}
impl Default for Terminal {
    fn default() -> Self {
        Self {
            panel: None,
            serial: 0,
            read_serial: 0,
            follow_serial: 0,
            list: vec![],
            list_loaded: false,
            shells: vec![],
            shell: None,
            environment: None,
            selected: None,
            attachment: None,
            follow: None,
            last_sequence: None,
            output: Output::default(),
            input: String::new(),
            editor: 0,
            max_input_bytes: MAX_INPUT_BYTES as u64,
            create_review: None,
            create_pending: None,
            close_review: None,
            close_pending: None,
            write_pending: None,
            notice: Notice::None,
            attempt: 0,
            exhausted: false,
        }
    }
}
impl Terminal {
    pub fn is_open(&self) -> bool {
        self.panel.is_some()
    }
    pub fn ticket(&self) -> Option<Ticket> {
        self.panel.as_ref().map(|panel| panel.ticket.clone())
    }
    pub fn target(&self) -> Option<&SessionId> {
        self.panel.as_ref().map(|panel| &panel.ticket.target)
    }
    pub fn list(&self) -> &[WebTerminalInfo] {
        &self.list
    }
    pub fn list_loaded(&self) -> bool {
        self.list_loaded
    }
    /// Whether the panel may claim a read is still outstanding. A refused read leaves
    /// `list_loaded` false, and the notice already states the failure — claiming "loading" beside
    /// it, forever, was the bug.
    pub fn loading_claim(&self) -> bool {
        !self.list_loaded && matches!(self.notice, Notice::Loading)
    }
    pub fn shells(&self) -> &[TerminalShell] {
        &self.shells
    }
    pub fn chosen_shell(&self) -> Option<&TerminalShell> {
        self.shell.and_then(|index| self.shells.get(index))
    }
    pub fn environment(&self) -> Option<&TerminalEnvironment> {
        self.environment.as_ref()
    }
    /// Whether this panel holds the live attachment a write needs. A follow that ended clears it,
    /// so the Write control can be disabled instead of refusing with unrelated copy.
    pub fn attached(&self) -> bool {
        self.attachment.is_some()
    }
    /// What the Host last said about input control for the selected terminal.
    pub fn control(&self) -> Control {
        let Some(selected) = self.selected.as_ref() else {
            return Control::Unconfirmed;
        };
        let Some(info) = self.list.iter().find(|info| &info.id == selected) else {
            return Control::Unconfirmed;
        };
        match &info.controller_id {
            Some(controller) if Some(controller) == self.attachment.as_ref() => Control::Held,
            Some(_) => Control::Elsewhere,
            None => Control::Unconfirmed,
        }
    }
    pub fn selected(&self) -> Option<&WebTerminalId> {
        self.selected.as_ref()
    }
    /// Detail lookup for the selected terminal. The panel resolves the row it
    /// already matched, so only this module's tests read it.
    #[cfg(test)]
    pub fn selected_info(&self) -> Option<&WebTerminalInfo> {
        let id = self.selected.as_ref()?;
        self.list.iter().find(|info| &info.id == id)
    }
    /// Selected terminal's live-follow ticket, for the tests that read it and for the layout fixture
    /// that feeds it recorded frames.
    #[cfg(any(test, feature = "public-layout-fixture"))]
    pub fn follow(&self) -> Option<&FollowTicket> {
        self.follow.as_ref()
    }
    pub fn output(&self) -> &Output {
        &self.output
    }
    pub fn input(&self) -> &str {
        &self.input
    }
    pub fn max_input_bytes(&self) -> u64 {
        self.max_input_bytes
    }
    pub fn review_create(&self) -> bool {
        self.create_review.is_some()
    }
    pub fn review_close(&self) -> Option<&WebTerminalId> {
        self.close_review.as_ref().map(|review| &review.id)
    }
    /// Whole-panel pending probes. The composer gates on the specific action, so
    /// only this module's tests read the aggregate slots.
    #[cfg(test)]
    pub fn create_pending(&self) -> bool {
        self.create_pending.is_some()
    }
    #[cfg(test)]
    pub fn close_pending(&self) -> bool {
        self.close_pending.is_some()
    }
    pub fn write_pending(&self) -> bool {
        self.write_pending.is_some()
    }
    pub fn busy(&self) -> bool {
        self.create_pending.is_some() || self.close_pending.is_some()
    }
    pub fn notice(&self) -> &Notice {
        &self.notice
    }
    /// Hide the panel and retire local reviews. It never cancels an admitted write.
    /// Returns whether a Host follow was live, so the caller can retire the stream too.
    pub fn dismiss(&mut self) -> bool {
        self.panel = None;
        self.create_review = None;
        self.close_review = None;
        self.notice = Notice::None;
        let following = self.follow.is_some();
        self.stop_follow();
        following
    }
    /// Fence late receipts after the transport generation is gone.
    pub fn disconnect(&mut self) {
        let admitted = self.create_pending.take().is_some() || self.close_pending.take().is_some();
        let writing = self.write_pending.take().is_some();
        self.dismiss();
        if admitted {
            self.notice = Notice::CreatingIndeterminate;
        } else if writing {
            self.notice = Notice::WriteIndeterminate;
        }
    }
    fn stop_follow(&mut self) {
        self.follow = None;
        self.attachment = None;
    }
    fn exhausted(&mut self) {
        self.exhausted = true;
        self.disconnect();
    }
    fn current(&self, ticket: &Ticket, context: &Context) -> bool {
        !self.exhausted
            && context.allowed
            && self
                .panel
                .as_ref()
                .is_some_and(|panel| &panel.ticket == ticket)
            && context.view.as_ref().is_some_and(|view| {
                view.epoch == ticket.epoch
                    && view.generation == ticket.generation
                    && view.target == ticket.target
            })
    }
    fn bump(&mut self) -> bool {
        match self.serial.checked_add(1) {
            Some(serial) => {
                self.serial = serial;
                if let Some(panel) = self.panel.as_mut() {
                    panel.ticket.serial = serial;
                }
                true
            }
            None => {
                self.exhausted();
                false
            }
        }
    }
    fn upsert(&mut self, info: WebTerminalInfo) {
        match self.list.iter_mut().find(|known| known.id == info.id) {
            Some(known) => *known = info,
            None => self.list.push(info),
        }
    }
    fn select(&mut self, id: WebTerminalId) -> Option<FollowTicket> {
        let ticket = self.ticket()?;
        let follow_serial = self.follow_serial.checked_add(1)?;
        self.follow_serial = follow_serial;
        let attachment =
            TerminalAttachmentId::new(format!("native-{}-{}", ticket.generation, follow_serial))
                .ok()?;
        self.selected = Some(id.clone());
        self.attachment = Some(attachment.clone());
        self.output.clear();
        let follow = FollowTicket {
            epoch: ticket.epoch,
            generation: ticket.generation,
            serial: follow_serial,
            target: ticket.target.clone(),
            id,
            attachment,
        };
        self.follow = Some(follow.clone());
        // The Host names the controller in the frame that follows; until then this is only an
        // attachment, and the notice must not claim exclusivity it has not been given.
        self.notice = Notice::FollowPending;
        Some(follow)
    }

    pub fn apply(&mut self, action: Action, context: &Context) -> Option<Effect> {
        match action {
            Action::Dismiss(ticket) if self.ticket().as_ref() == Some(&ticket) => {
                return self.dismiss().then_some(Effect::Unfollow);
            }
            Action::Open { view }
                if !self.exhausted && context.allowed && context.view.as_ref() == Some(&view) =>
            {
                if !self.bump() {
                    return None;
                }
                let serial = self.serial;
                let read_serial = match self.read_serial.checked_add(1) {
                    Some(value) => value,
                    None => {
                        self.exhausted();
                        return None;
                    }
                };
                self.read_serial = read_serial;
                self.dismiss();
                self.exhausted = false;
                self.panel = Some(Panel {
                    ticket: Ticket {
                        epoch: view.epoch,
                        generation: view.generation,
                        serial,
                        target: view.target.clone(),
                    },
                });
                self.list.clear();
                self.list_loaded = false;
                self.shells.clear();
                self.shell = None;
                self.environment = None;
                self.selected = None;
                self.input.clear();
                self.output.clear();
                self.notice = Notice::Loading;
                return Some(Effect::Read(ReadTicket {
                    epoch: view.epoch,
                    generation: view.generation,
                    serial: read_serial,
                    target: view.target,
                }));
            }
            Action::Refresh(ticket) if self.current(&ticket, context) && !self.busy() => {
                let read_serial = match self.read_serial.checked_add(1) {
                    Some(value) => value,
                    None => {
                        self.exhausted();
                        return None;
                    }
                };
                self.read_serial = read_serial;
                self.notice = Notice::Loading;
                return Some(Effect::Read(ReadTicket {
                    epoch: ticket.epoch,
                    generation: ticket.generation,
                    serial: read_serial,
                    target: ticket.target,
                }));
            }
            Action::Select { ticket, id }
                if self.current(&ticket, context)
                    && !self.busy()
                    && self.list.iter().any(|info| info.id == id) =>
            {
                if let Some(follow) = self.select(id) {
                    return Some(Effect::Follow(follow));
                }
                self.exhausted();
            }
            Action::ChooseShell { ticket, index }
                if self.current(&ticket, context) && index < self.shells.len() =>
            {
                self.shell = Some(index);
            }
            Action::EditInput { ticket, text } if self.current(&ticket, context) => {
                let editor = match self.editor.checked_add(1) {
                    Some(value) => value,
                    None => {
                        self.exhausted();
                        return None;
                    }
                };
                self.editor = editor;
                self.input = text;
                if self.write_pending.is_some() {
                    self.notice = Notice::WritePending;
                } else if self.input.len() as u64 > self.max_input_bytes {
                    self.notice = Notice::InputRefused;
                } else if matches!(self.notice, Notice::InputRefused | Notice::WriteRefused) {
                    // Typing is how the operator fixes those two; every other notice states an
                    // event (a detached follow, a failed read) that a keystroke must not erase.
                    self.notice = Notice::None;
                }
            }
            Action::Write(ticket)
                if self.current(&ticket, context)
                    && self.environment.is_some()
                    && self.write_pending.is_none()
                    && self.create_pending.is_none()
                    && self.close_pending.is_none() =>
            {
                if self.control() == Control::Elsewhere {
                    self.notice = Notice::ControlElsewhere;
                    return None;
                }
                let (Some(id), Some(attachment)) = (self.selected.clone(), self.attachment.clone())
                else {
                    // A detached follow is not an empty field or a missing selection: say what
                    // actually happened so the operator reattaches instead of retyping.
                    self.notice = if self.selected.is_some() {
                        Notice::Detached
                    } else {
                        Notice::InputRefused
                    };
                    return None;
                };
                if self.input.is_empty() || self.input.len() as u64 > self.max_input_bytes {
                    self.notice = Notice::InputRefused;
                    return None;
                }
                let attempt = match self.attempt.checked_add(1) {
                    Some(value) => value,
                    None => {
                        self.exhausted();
                        return None;
                    }
                };
                self.attempt = attempt;
                let receipt = WriteReceipt {
                    ticket,
                    id,
                    attachment,
                    attempt,
                    editor: self.editor,
                };
                let data = self.input.clone();
                self.write_pending = Some(receipt.clone());
                self.notice = Notice::WritePending;
                return Some(Effect::Write(WriteSubmission { receipt, data }));
            }
            Action::Key { ticket, data }
                if self.current(&ticket, context)
                    && self.environment.is_some()
                    && self.write_pending.is_none()
                    && self.create_pending.is_none()
                    && self.close_pending.is_none()
                    && !data.is_empty()
                    && data.len() as u64 <= self.max_input_bytes =>
            {
                if self.control() == Control::Elsewhere {
                    self.notice = Notice::ControlElsewhere;
                    return None;
                }
                let (Some(id), Some(attachment)) = (self.selected.clone(), self.attachment.clone())
                else {
                    self.notice = if self.selected.is_some() {
                        Notice::Detached
                    } else {
                        Notice::InputRefused
                    };
                    return None;
                };
                let attempt = match self.attempt.checked_add(1) {
                    Some(value) => value,
                    None => {
                        self.exhausted();
                        return None;
                    }
                };
                self.attempt = attempt;
                let receipt = WriteReceipt {
                    ticket,
                    id,
                    attachment,
                    attempt,
                    editor: self.editor,
                };
                // A keystroke never edits the typed line: the bytes go straight to the Host, the way
                // the reference's terminal has no line at all.
                self.write_pending = Some(receipt.clone());
                self.notice = Notice::WritePending;
                return Some(Effect::Write(WriteSubmission { receipt, data }));
            }
            Action::ReviewCreate(ticket)
                if self.current(&ticket, context)
                    && self.environment.is_some()
                    && !self.busy()
                    && self.write_pending.is_none()
                    && self.list.len() < dsh_native_transport::terminal::MAX_TERMINALS =>
            {
                self.create_review = Some(Review {
                    ticket: ticket.clone(),
                });
                self.close_review = None;
                self.notice = Notice::CreateReview;
            }
            Action::BackCreate(ticket) if self.current(&ticket, context) => {
                self.create_review = None;
                self.notice = Notice::None;
            }
            Action::ConfirmCreate(review)
                if self.current(&review.ticket, context)
                    && !self.busy()
                    && self.write_pending.is_none()
                    && self.create_review.as_ref() == Some(&review) =>
            {
                let attempt = match self.attempt.checked_add(1) {
                    Some(value) => value,
                    None => {
                        self.exhausted();
                        return None;
                    }
                };
                let Some(id) =
                    WebTerminalId::new(format!("native-{}-{}", review.ticket.generation, attempt))
                        .ok()
                        .filter(|id| !self.list.iter().any(|info| &info.id == id))
                else {
                    self.notice = Notice::CreateFailed;
                    return None;
                };
                self.attempt = attempt;
                self.create_review = None;
                let receipt = CreateReceipt {
                    ticket: review.ticket,
                    id,
                    attempt,
                };
                let shell_path = self.chosen_shell().map(|shell| shell.path.clone());
                self.create_pending = Some(receipt.clone());
                self.notice = Notice::Creating;
                // 80×24 is the request, bounded by the limits the Host advertised: creating at a
                // size the environment refuses would be a request the panel already knows fails.
                let (cols, rows) = match &self.environment {
                    Some(environment) => (
                        environment.max_cols.clamp(1, DEFAULT_COLS),
                        environment.max_rows.clamp(1, DEFAULT_ROWS),
                    ),
                    None => (DEFAULT_COLS, DEFAULT_ROWS),
                };
                return Some(Effect::Create(CreateSubmission {
                    receipt,
                    shell_path,
                    cols,
                    rows,
                }));
            }
            Action::ReviewClose { ticket, id }
                if self.current(&ticket, context)
                    && !self.busy()
                    && self.write_pending.is_none()
                    && self.list.iter().any(|info| info.id == id) =>
            {
                self.close_review = Some(CloseReview {
                    ticket: ticket.clone(),
                    id,
                });
                self.create_review = None;
                self.notice = Notice::CloseReview;
            }
            Action::BackClose(ticket) if self.current(&ticket, context) => {
                self.close_review = None;
                self.notice = Notice::None;
            }
            Action::ConfirmClose(review)
                if self.current(&review.ticket, context)
                    && !self.busy()
                    && self.write_pending.is_none()
                    && self.close_review.as_ref() == Some(&review) =>
            {
                let attempt = match self.attempt.checked_add(1) {
                    Some(value) => value,
                    None => {
                        self.exhausted();
                        return None;
                    }
                };
                self.attempt = attempt;
                self.close_review = None;
                let receipt = CloseReceipt {
                    ticket: review.ticket,
                    id: review.id,
                    attempt,
                };
                self.close_pending = Some(receipt.clone());
                self.notice = Notice::Closing;
                return Some(Effect::Close(CloseSubmission { receipt }));
            }
            // A press that an admission guard refuses is still a user action, and the control stays
            // drawn. Name the blocking condition here instead of letting the wildcard swallow it
            // silently; the guarded arms above already matched every admitted press.
            Action::ReviewCreate(ticket) if self.current(&ticket, context) => {
                self.notice = if self.environment.is_none() {
                    Notice::LimitsUnavailable
                } else if self.busy() || self.write_pending.is_some() {
                    Notice::Busy
                } else {
                    Notice::CreateLimit
                };
            }
            Action::ConfirmCreate(review)
                if self.current(&review.ticket, context)
                    && self.create_review.as_ref() == Some(&review) =>
            {
                self.notice = Notice::Busy;
            }
            Action::ReviewClose { ticket, id }
                if self.current(&ticket, context) && self.list.iter().any(|info| info.id == id) =>
            {
                self.notice = Notice::Busy;
            }
            Action::ConfirmClose(review)
                if self.current(&review.ticket, context)
                    && self.close_review.as_ref() == Some(&review) =>
            {
                self.notice = Notice::Busy;
            }
            _ => {}
        }
        None
    }

    /// Install a confirmed read. A stale or foreign ticket changes nothing.
    pub fn loaded(&mut self, ticket: &ReadTicket, result: Result<Loaded, ReadFailure>) {
        let Some(panel) = self.panel.as_ref() else {
            return;
        };
        if ticket.serial != self.read_serial
            || ticket.epoch != panel.ticket.epoch
            || ticket.generation != panel.ticket.generation
            || ticket.target != panel.ticket.target
        {
            return;
        }
        match result {
            Ok(loaded) => {
                self.list = loaded.list;
                self.list_loaded = true;
                self.shells = loaded.shells;
                if self.shell.is_none_or(|index| index >= self.shells.len()) {
                    self.shell = (!self.shells.is_empty()).then_some(0);
                }
                self.environment = loaded.environment;
                self.notice = match &self.environment {
                    Some(environment) => {
                        self.max_input_bytes =
                            environment.max_input_bytes.min(MAX_INPUT_BYTES as u64);
                        Notice::Limits {
                            max_input_bytes: self.max_input_bytes,
                            max_cols: environment.max_cols,
                            max_rows: environment.max_rows,
                        }
                    }
                    None => Notice::LimitsUnavailable,
                };
                if self
                    .selected
                    .as_ref()
                    .is_some_and(|id| !self.list.iter().any(|info| &info.id == id))
                {
                    self.selected = None;
                    self.stop_follow();
                }
            }
            Err(_) => {
                self.list_loaded = false;
                self.notice = Notice::ReadFailed;
            }
        }
    }
    /// Apply one Host frame to the exact live follow. Foreign or retired frames are dropped.
    pub fn frame(&mut self, ticket: &FollowTicket, frame: TerminalFrame) {
        if self.follow.as_ref() != Some(ticket) {
            return;
        }
        match frame {
            TerminalFrame::Snapshot {
                sequence,
                screen,
                info,
            } => {
                // A malformed metadata frame is dropped, never merged into the list.
                if dsh_native_transport::terminal::validate_info(&info).is_err() {
                    return;
                }
                self.output.clear();
                self.output.push(&screen);
                self.last_sequence = Some(sequence);
                // A snapshot is the recovery a gap asks for, so it clears that notice.
                if matches!(self.notice, Notice::OutputGap) {
                    self.notice = Notice::None;
                }
                self.upsert(info);
                self.control_notice();
            }
            TerminalFrame::Output { sequence, data } => {
                // The reference compares every frame's sequence with the one before it and refuses
                // the screen when a frame is missing (`model.ts:294`): a silently short buffer would
                // read as the shell's own output. A snapshot re-establishes the base, so only the
                // incremental stream is checked.
                match self.last_sequence {
                    Some(previous) if sequence > previous + 1 => {
                        self.notice = Notice::OutputGap;
                        return;
                    }
                    _ => {}
                }
                self.last_sequence = Some(sequence);
                self.output.push(&data)
            }
            TerminalFrame::State { info } => {
                if dsh_native_transport::terminal::validate_info(&info).is_ok() {
                    self.upsert(info);
                    self.control_notice();
                }
            }
        }
    }
    /// Restate who the Host named as controller, without clobbering a notice about something else.
    fn control_notice(&mut self) {
        if self.follow.is_none() {
            return;
        }
        if !matches!(
            self.notice,
            Notice::FollowPending | Notice::FollowAttached | Notice::ControlElsewhere
        ) {
            return;
        }
        self.notice = match self.control() {
            Control::Held => Notice::FollowAttached,
            Control::Elsewhere => Notice::ControlElsewhere,
            Control::Unconfirmed => Notice::FollowPending,
        };
    }
    /// Retire the exact follow. A later frame for the same ticket changes nothing.
    pub fn follow_ended(&mut self, ticket: &FollowTicket, failure: Option<ReadFailure>) {
        if self.follow.as_ref() != Some(ticket) {
            return;
        }
        self.follow = None;
        self.attachment = None;
        self.notice = if failure.is_some() {
            Notice::FollowFailed
        } else {
            Notice::FollowEnded
        };
    }
    /// Retire the exact create receipt; install only a Host-confirmed matching terminal.
    pub fn finish_create(
        &mut self,
        receipt: &CreateReceipt,
        outcome: CreateOutcome,
    ) -> Option<CreateApplied> {
        if self.create_pending.as_ref() != Some(receipt) {
            return None;
        }
        self.create_pending = None;
        let mut applied = None;
        let notice = match outcome {
            CreateOutcome::Created(info) if info.id == receipt.id => {
                // A new terminal is selected and followed only from this Host ACK.
                let follow = self.select(info.id.clone());
                self.upsert((*info).clone());
                match follow {
                    Some(follow) => {
                        applied = Some(CreateApplied {
                            info: (*info).clone(),
                            follow,
                        });
                        Notice::FollowAttached
                    }
                    None => Notice::CreateFailed,
                }
            }
            CreateOutcome::Created(_) => Notice::CreatingIndeterminate,
            CreateOutcome::Unavailable => Notice::CreateFailed,
            CreateOutcome::LimitReached => Notice::CreateLimit,
            CreateOutcome::NotSent => Notice::CreateNotSent,
            CreateOutcome::Unknown => Notice::CreatingIndeterminate,
        };
        if self
            .panel
            .as_ref()
            .is_some_and(|panel| panel.ticket == receipt.ticket)
        {
            self.notice = notice;
        } else if matches!(self.notice, Notice::Creating) {
            self.notice = Notice::None;
        }
        applied
    }
    /// Retire the exact close receipt; remove the terminal only on a Host confirmation.
    pub fn finish_close(&mut self, receipt: &CloseReceipt, outcome: CloseOutcome) {
        if self.close_pending.as_ref() != Some(receipt) {
            return;
        }
        self.close_pending = None;
        let notice = match outcome {
            CloseOutcome::Closed => {
                self.list.retain(|info| info.id != receipt.id);
                if self.selected.as_ref() == Some(&receipt.id) {
                    self.selected = None;
                    self.stop_follow();
                    self.output.clear();
                }
                Notice::Closed
            }
            CloseOutcome::Unavailable => Notice::CloseFailed,
            CloseOutcome::NotSent => Notice::CloseNotSent,
            CloseOutcome::Unknown => Notice::ClosingIndeterminate,
        };
        if self
            .panel
            .as_ref()
            .is_some_and(|panel| panel.ticket == receipt.ticket)
        {
            self.notice = notice;
        } else if matches!(self.notice, Notice::Closing) {
            self.notice = Notice::None;
        }
    }
    /// Retire the exact write receipt. Only a Host acceptance clears the typed line.
    pub fn finish_write(&mut self, receipt: &WriteReceipt, outcome: WriteOutcome) {
        if self.write_pending.as_ref() != Some(receipt) {
            return;
        }
        self.write_pending = None;
        let notice = match outcome {
            WriteOutcome::Sent => {
                if self.editor == receipt.editor {
                    self.input.clear();
                }
                Notice::None
            }
            WriteOutcome::Refused => Notice::WriteRefused,
            WriteOutcome::NotSent => Notice::WriteNotSent,
            WriteOutcome::Unknown => Notice::WriteIndeterminate,
        };
        if self
            .panel
            .as_ref()
            .is_some_and(|panel| panel.ticket == receipt.ticket)
        {
            self.notice = notice;
        } else if matches!(self.notice, Notice::WritePending) {
            self.notice = Notice::None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid(value: &str) -> SessionId {
        SessionId::new(value).unwrap()
    }
    fn tid(value: &str) -> WebTerminalId {
        WebTerminalId::new(value).unwrap()
    }
    fn shell() -> TerminalShell {
        TerminalShell {
            path: "/bin/PUBLIC-shell".into(),
            args: vec!["-l".into()],
            name: "PUBLIC shell".into(),
        }
    }
    fn info(id: &str) -> WebTerminalInfo {
        WebTerminalInfo {
            id: tid(id),
            title: "PUBLIC terminal".into(),
            shell: shell(),
            cwd: "/PUBLIC/cwd".into(),
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
            state: dsh_native_transport::terminal::TerminalState::Running,
            exit_code: None,
            error: None,
            controller_id: None,
        }
    }
    fn environment() -> TerminalEnvironment {
        TerminalEnvironment {
            cwd: "/PUBLIC/cwd".into(),
            max_input_bytes: 512,
            max_cols: 500,
            max_rows: 200,
            scrollback: 1000,
        }
    }
    fn context() -> Context {
        Context {
            view: Some(View {
                epoch: 1,
                generation: 7,
                target: sid("PUBLIC-terminal-session"),
            }),
            allowed: true,
        }
    }
    fn ticket(terminal: &Terminal) -> Ticket {
        terminal.ticket().unwrap()
    }
    fn opened() -> Terminal {
        let mut terminal = Terminal::default();
        let effect = terminal
            .apply(
                Action::Open {
                    view: context().view.unwrap(),
                },
                &context(),
            )
            .expect("open reads the host list");
        let Effect::Read(read) = effect else {
            panic!("opening must not create a shell");
        };
        terminal.loaded(
            &read,
            Ok(Loaded {
                list: vec![info("PUBLIC-terminal-1")],
                shells: vec![shell()],
                environment: Some(environment()),
            }),
        );
        terminal
    }
    #[test]
    fn an_oversized_chunk_is_counted_as_truncated_and_never_silently_shortened() {
        let mut output = Output::default();
        output.push(&"x".repeat(CHUNK_BYTES));
        assert_eq!(
            output.truncated(),
            0,
            "an exactly-sized chunk loses nothing"
        );
        output.push(&"y".repeat(CHUNK_BYTES * 2));
        assert_eq!(
            output.truncated(),
            CHUNK_BYTES,
            "the tail a chunk loses is reported, not hidden"
        );
        assert!(
            output
                .chunks()
                .iter()
                .all(|chunk| chunk.len() <= CHUNK_BYTES)
        );
        // A chunk that ends mid-character backs the cut up to the boundary and loses exactly the
        // four bytes of the character the bound would have split.
        let before = output.truncated();
        output.push(&format!("{}🌹", "a".repeat(CHUNK_BYTES - 1)));
        assert_eq!(output.truncated() - before, 4);
        // A chunk far past the bound reports every byte it loses, not just the last character.
        let before = output.truncated();
        output.push(&"🌹".repeat(CHUNK_BYTES));
        assert!(output.truncated() - before > CHUNK_BYTES);
        output.clear();
        assert_eq!(output.truncated(), 0);
    }
    #[test]
    fn input_control_is_reported_only_from_the_hosts_own_frame() {
        let mut terminal = opened();
        let follow = select(&mut terminal);
        // The Host has not spoken yet: the panel says it is attached, not that it controls.
        assert_eq!(terminal.control(), Control::Unconfirmed);
        assert_eq!(terminal.notice(), &Notice::FollowPending);
        // The baseline frame names this panel's attachment as the controller.
        let mine = WebTerminalInfo {
            controller_id: Some(follow.attachment.clone()),
            ..info("PUBLIC-terminal-1")
        };
        terminal.frame(
            &follow,
            TerminalFrame::Snapshot {
                sequence: 1,
                screen: String::new(),
                info: mine.clone(),
            },
        );
        assert_eq!(terminal.control(), Control::Held);
        assert_eq!(terminal.notice(), &Notice::FollowAttached);
        // A later state frame from another client takes control away, and a write refuses.
        let foreign = WebTerminalInfo {
            controller_id: Some(TerminalAttachmentId::new("PUBLIC-other-client").unwrap()),
            ..info("PUBLIC-terminal-1")
        };
        terminal.frame(&follow, TerminalFrame::State { info: foreign });
        assert_eq!(terminal.control(), Control::Elsewhere);
        assert_eq!(terminal.notice(), &Notice::ControlElsewhere);
        let t = ticket(&terminal);
        let _ = terminal.apply(
            Action::EditInput {
                ticket: t.clone(),
                text: "PUBLIC input".into(),
            },
            &context(),
        );
        assert!(terminal.apply(Action::Write(t), &context()).is_none());
        assert_eq!(terminal.notice(), &Notice::ControlElsewhere);
        // The Host withdrawing the controller field is "unconfirmed", not "mine".
        terminal.frame(
            &follow,
            TerminalFrame::State {
                info: info("PUBLIC-terminal-1"),
            },
        );
        assert_eq!(terminal.control(), Control::Unconfirmed);
    }
    #[test]
    fn dismissing_a_panel_with_a_live_follow_asks_for_the_stream_to_stop() {
        let mut terminal = opened();
        let _follow = select(&mut terminal);
        let t = ticket(&terminal);
        assert!(matches!(
            terminal.apply(Action::Dismiss(t.clone()), &context()),
            Some(Effect::Unfollow)
        ));
        assert!(!terminal.is_open());
        // Without a follow there is no stream to retire, so no command is asked for.
        let mut terminal = opened();
        assert!(
            terminal
                .apply(Action::Dismiss(ticket(&terminal)), &context())
                .is_none()
        );
    }
    #[test]
    fn a_create_is_bounded_by_the_host_environment_it_was_told_about() {
        let mut terminal = opened();
        let submission = create(&mut terminal);
        assert_eq!(
            (submission.cols, submission.rows),
            (DEFAULT_COLS, DEFAULT_ROWS)
        );
        terminal.finish_create(&submission.receipt, CreateOutcome::Unavailable);
        // A Host that advertises smaller limits gets a request inside them.
        let mut terminal = opened();
        let effect = terminal
            .apply(Action::Refresh(ticket(&terminal)), &context())
            .expect("refresh reads again");
        let Effect::Read(read) = effect else {
            panic!("refresh must only read");
        };
        terminal.loaded(
            &read,
            Ok(Loaded {
                list: vec![info("PUBLIC-terminal-1")],
                shells: vec![shell()],
                environment: Some(TerminalEnvironment {
                    max_cols: 40,
                    max_rows: 10,
                    ..environment()
                }),
            }),
        );
        let submission = create(&mut terminal);
        assert_eq!((submission.cols, submission.rows), (40, 10));
    }
    #[test]
    fn a_detached_follow_refuses_a_write_by_name_and_keeps_its_warning() {
        let mut terminal = opened();
        let follow = select(&mut terminal);
        assert!(terminal.attached());
        let t = ticket(&terminal);
        let _ = terminal.apply(
            Action::EditInput {
                ticket: t.clone(),
                text: "PUBLIC input".into(),
            },
            &context(),
        );
        // The Host retires the attachment; the panel must say so and stop offering a write.
        terminal.follow_ended(&follow, None);
        assert!(!terminal.attached());
        assert_eq!(terminal.notice(), &Notice::FollowEnded);
        // Typing is not an event that erases the warning.
        let _ = terminal.apply(
            Action::EditInput {
                ticket: t.clone(),
                text: "PUBLIC input, retyped".into(),
            },
            &context(),
        );
        assert_eq!(
            terminal.notice(),
            &Notice::FollowEnded,
            "a keystroke must not hide a detached follow"
        );
        // And a write with text still in the field names the real cause, not an empty field.
        assert!(terminal.apply(Action::Write(t), &context()).is_none());
        assert_eq!(terminal.notice(), &Notice::Detached);
    }
    #[test]
    fn a_press_the_admission_guard_refuses_names_its_condition() {
        // No registered Agent: the create control is drawn but the write cannot run.
        let mut terminal = Terminal::default();
        let effect = terminal
            .apply(
                Action::Open {
                    view: context().view.unwrap(),
                },
                &context(),
            )
            .expect("open reads the host list");
        let Effect::Read(read) = effect else {
            panic!("opening must only read");
        };
        terminal.loaded(
            &read,
            Ok(Loaded {
                list: Vec::new(),
                shells: vec![shell()],
                environment: None,
            }),
        );
        let t = ticket(&terminal);
        assert!(
            terminal
                .apply(Action::ReviewCreate(t.clone()), &context())
                .is_none()
        );
        assert_eq!(terminal.notice(), &Notice::LimitsUnavailable);
        assert!(!terminal.review_create());
        // An outstanding write blocks a close review, and says so.
        let mut terminal = opened();
        let _follow = select(&mut terminal);
        let t = ticket(&terminal);
        let _ = terminal.apply(
            Action::EditInput {
                ticket: t.clone(),
                text: "PUBLIC input".into(),
            },
            &context(),
        );
        assert!(matches!(
            terminal.apply(Action::Write(t.clone()), &context()),
            Some(Effect::Write(_))
        ));
        assert!(terminal.write_pending());
        assert!(
            terminal
                .apply(
                    Action::ReviewClose {
                        ticket: t,
                        id: tid("PUBLIC-terminal-1"),
                    },
                    &context()
                )
                .is_none()
        );
        assert_eq!(terminal.notice(), &Notice::Busy);
        assert!(terminal.review_close().is_none());
    }
    #[test]
    fn a_refused_list_read_never_leaves_a_loading_claim() {
        let mut terminal = Terminal::default();
        let effect = terminal
            .apply(
                Action::Open {
                    view: context().view.unwrap(),
                },
                &context(),
            )
            .expect("open reads the host list");
        let Effect::Read(read) = effect else {
            panic!("opening must only read");
        };
        assert!(terminal.loading_claim(), "an outstanding read says so");
        terminal.loaded(&read, Err(ReadFailure::Rejected));
        assert!(!terminal.list_loaded());
        assert!(
            !terminal.loading_claim(),
            "a refusal is stated by the notice, not as loading"
        );
        assert_eq!(terminal.notice(), &Notice::ReadFailed);
        // A later successful read restores both the list and the settled claim.
        let effect = terminal
            .apply(Action::Refresh(ticket(&terminal)), &context())
            .expect("refresh reads again");
        let Effect::Read(read) = effect else {
            panic!("refresh must only read");
        };
        terminal.loaded(
            &read,
            Ok(Loaded {
                list: Vec::new(),
                shells: vec![shell()],
                environment: Some(environment()),
            }),
        );
        assert!(terminal.list_loaded());
        assert!(!terminal.loading_claim());
    }
    fn select(terminal: &mut Terminal) -> FollowTicket {
        let effect = terminal
            .apply(
                Action::Select {
                    ticket: ticket(terminal),
                    id: tid("PUBLIC-terminal-1"),
                },
                &context(),
            )
            .expect("selecting a listed terminal attaches a follow");
        match effect {
            Effect::Follow(follow) => follow,
            _ => panic!("select must follow"),
        }
    }
    fn create(terminal: &mut Terminal) -> CreateSubmission {
        let t = ticket(terminal);
        assert!(
            terminal
                .apply(Action::ReviewCreate(t.clone()), &context())
                .is_none()
        );
        assert!(terminal.review_create());
        let review = Review { ticket: t };
        match terminal
            .apply(Action::ConfirmCreate(review), &context())
            .expect("confirmed create")
        {
            Effect::Create(submission) => submission,
            _ => panic!("create effect"),
        }
    }

    #[test]
    fn opening_reads_the_host_and_creates_no_shell_or_terminal() {
        let mut terminal = Terminal::default();
        let effect = terminal
            .apply(
                Action::Open {
                    view: context().view.unwrap(),
                },
                &context(),
            )
            .unwrap();
        assert!(matches!(effect, Effect::Read(_)));
        assert!(!terminal.create_pending() && !terminal.close_pending());
        assert!(terminal.list().is_empty());
        let t = ticket(&terminal);
        assert!(terminal.apply(Action::Dismiss(t), &context()).is_none());
        assert!(!terminal.is_open());
    }

    #[test]
    fn create_requires_review_and_confirms_one_exact_request() {
        let mut terminal = opened();
        // A confirm without a review is refused and sends nothing.
        let forged = Review {
            ticket: ticket(&terminal),
        };
        assert!(
            terminal
                .apply(Action::ConfirmCreate(forged), &context())
                .is_none()
        );
        assert!(!terminal.create_pending());
        let submission = create(&mut terminal);
        assert_eq!(submission.cols, DEFAULT_COLS);
        assert_eq!(submission.shell_path.as_deref(), Some("/bin/PUBLIC-shell"));
        assert!(terminal.create_pending());
        assert!(!terminal.review_create());
        // A replay of the consumed review cannot become a second allocation.
        let t = ticket(&terminal);
        assert!(
            terminal
                .apply(Action::ConfirmCreate(Review { ticket: t }), &context())
                .is_none()
        );
        assert!(terminal.create_pending());
    }

    #[test]
    fn ack_installs_only_the_confirmed_terminal_and_follows_it() {
        let mut terminal = opened();
        let submission = create(&mut terminal);
        assert!(
            !terminal
                .list()
                .iter()
                .any(|i| i.id == submission.receipt.id)
        );
        let applied = terminal
            .finish_create(
                &submission.receipt,
                CreateOutcome::Created(Box::new(info(submission.receipt.id.as_str()))),
            )
            .expect("host-confirmed create");
        assert_eq!(applied.info.id, submission.receipt.id);
        assert_eq!(terminal.selected(), Some(&submission.receipt.id));
        assert_eq!(terminal.follow(), Some(&applied.follow));
        assert!(
            terminal
                .list()
                .iter()
                .any(|i| i.id == submission.receipt.id)
        );
        // A created identity that differs from the reviewed one is never installed.
        let mut terminal = opened();
        let submission = create(&mut terminal);
        assert!(
            terminal
                .finish_create(
                    &submission.receipt,
                    CreateOutcome::Created(Box::new(info("PUBLIC-other-terminal"))),
                )
                .is_none()
        );
        assert_eq!(terminal.notice(), &Notice::CreatingIndeterminate);
        assert!(
            !terminal
                .list()
                .iter()
                .any(|i| i.id == tid("PUBLIC-other-terminal"))
        );
    }

    #[test]
    fn refused_or_unsent_create_adds_no_terminal_and_reports_redacted_failure() {
        for (outcome, expected) in [
            (CreateOutcome::Unavailable, Notice::CreateFailed),
            (CreateOutcome::LimitReached, Notice::CreateLimit),
            (CreateOutcome::Unknown, Notice::CreatingIndeterminate),
            (CreateOutcome::NotSent, Notice::CreateNotSent),
        ] {
            let mut terminal = opened();
            let submission = create(&mut terminal);
            assert!(
                terminal
                    .finish_create(&submission.receipt, outcome)
                    .is_none()
            );
            assert_eq!(terminal.notice(), &expected);
            assert_eq!(terminal.list().len(), 1);
            assert!(!terminal.create_pending());
            // No automatic retry: the consumed review is not re-armed.
            let t = ticket(&terminal);
            assert!(
                terminal
                    .apply(Action::ConfirmCreate(Review { ticket: t }), &context())
                    .is_none()
            );
            assert!(!terminal.create_pending());
        }
    }

    #[test]
    fn a_session_without_an_agent_lists_history_but_refuses_create_and_write() {
        let mut terminal = Terminal::default();
        let effect = terminal
            .apply(
                Action::Open {
                    view: context().view.unwrap(),
                },
                &context(),
            )
            .unwrap();
        let Effect::Read(read) = effect else {
            panic!("read effect");
        };
        terminal.loaded(
            &read,
            Ok(Loaded {
                list: vec![info("PUBLIC-terminal-1")],
                shells: vec![],
                environment: None,
            }),
        );
        assert_eq!(terminal.notice(), &Notice::LimitsUnavailable);
        assert_eq!(terminal.list().len(), 1);
        let t = ticket(&terminal);
        assert!(
            terminal
                .apply(Action::ReviewCreate(t.clone()), &context())
                .is_none()
        );
        assert!(!terminal.review_create());
        assert!(
            terminal.apply(Action::Write(t), &context()).is_none(),
            "no Agent environment means no write"
        );
    }

    #[test]
    fn bounded_output_evicts_the_oldest_chunks_with_explicit_overflow() {
        let mut output = Output::default();
        for index in 0..(OUTPUT_CHUNKS + 20) {
            output.push(&format!("line {index}\n"));
        }
        assert!(output.dropped() > 0);
        assert!(output.bytes() <= OUTPUT_BYTES);
        assert!(output.chunks().len() <= OUTPUT_CHUNKS);
        // Control sequences are stripped so the plain-text panel cannot be corrupted.
        let mut control = Output::default();
        control.push("\u{1b}[31mPUBLIC\u{1b}[0m\r\n");
        assert_eq!(control.text(), "PUBLIC\n");
        // A single oversized chunk is truncated, never retained whole.
        let mut huge = Output::default();
        huge.push(&"x".repeat(CHUNK_BYTES * 2));
        assert!(huge.bytes() <= CHUNK_BYTES);
    }

    #[test]
    fn input_write_is_blocked_while_nothing_is_selected() {
        let mut terminal = opened();
        let t = ticket(&terminal);
        assert!(
            terminal
                .apply(
                    Action::EditInput {
                        ticket: t.clone(),
                        text: "PUBLIC input".into()
                    },
                    &context()
                )
                .is_none()
        );
        assert!(
            terminal.apply(Action::Write(t), &context()).is_none(),
            "no selection means no write"
        );
        assert_eq!(terminal.notice(), &Notice::InputRefused);
        assert!(terminal.selected().is_none());
    }

    #[test]
    fn write_is_single_flight_and_only_host_acceptance_clears_the_line() {
        let mut terminal = opened();
        let _follow = select(&mut terminal);
        let t = ticket(&terminal);
        let _ = terminal.apply(
            Action::EditInput {
                ticket: t.clone(),
                text: "PUBLIC input".into(),
            },
            &context(),
        );
        let submission = match terminal
            .apply(Action::Write(t.clone()), &context())
            .unwrap()
        {
            Effect::Write(submission) => submission,
            _ => panic!("write effect"),
        };
        assert_eq!(submission.data(), "PUBLIC input");
        assert!(terminal.write_pending());
        // A second write is refused while the first receipt is outstanding.
        assert!(terminal.apply(Action::Write(t), &context()).is_none());
        terminal.finish_write(&submission.receipt, WriteOutcome::Unknown);
        assert_eq!(terminal.notice(), &Notice::WriteIndeterminate);
        assert_eq!(terminal.input(), "PUBLIC input");
        // Only a Host acceptance clears the typed line.
        let _follow = select(&mut terminal);
        let t = ticket(&terminal);
        let _ = terminal.apply(
            Action::EditInput {
                ticket: t.clone(),
                text: "PUBLIC second".into(),
            },
            &context(),
        );
        let submission = match terminal.apply(Action::Write(t), &context()).unwrap() {
            Effect::Write(submission) => submission,
            _ => panic!("write effect"),
        };
        terminal.finish_write(&submission.receipt, WriteOutcome::Sent);
        assert_eq!(terminal.input(), "");
    }

    #[test]
    fn close_requires_confirmation_and_removes_only_a_host_confirmed_terminal() {
        let mut terminal = opened();
        let t = ticket(&terminal);
        // Confirming without a review is refused.
        assert!(
            terminal
                .apply(
                    Action::ConfirmClose(CloseReview {
                        ticket: t.clone(),
                        id: tid("PUBLIC-terminal-1"),
                    }),
                    &context(),
                )
                .is_none()
        );
        assert!(!terminal.close_pending());
        let _ = terminal.apply(
            Action::ReviewClose {
                ticket: t.clone(),
                id: tid("PUBLIC-terminal-1"),
            },
            &context(),
        );
        assert_eq!(terminal.review_close(), Some(&tid("PUBLIC-terminal-1")));
        let submission = match terminal
            .apply(
                Action::ConfirmClose(CloseReview {
                    ticket: t,
                    id: tid("PUBLIC-terminal-1"),
                }),
                &context(),
            )
            .unwrap()
        {
            Effect::Close(submission) => submission,
            _ => panic!("close effect"),
        };
        // Nothing is removed before the Host confirmation.
        assert_eq!(terminal.list().len(), 1);
        terminal.finish_close(&submission.receipt, CloseOutcome::Unknown);
        assert_eq!(terminal.notice(), &Notice::ClosingIndeterminate);
        assert_eq!(terminal.list().len(), 1);
    }

    #[test]
    fn stale_epoch_generation_or_target_refuses_without_writing() {
        let mutations: [fn(&mut View); 3] = [
            |view| view.epoch += 1,
            |view| view.generation += 1,
            |view| view.target = sid("PUBLIC-other-session"),
        ];
        for mutate in mutations {
            let mut terminal = opened();
            let mut stale = context();
            mutate(stale.view.as_mut().unwrap());
            let t = ticket(&terminal);
            // No review and no confirm can be admitted against a stale view.
            assert!(
                terminal
                    .apply(Action::ReviewCreate(t.clone()), &stale)
                    .is_none()
            );
            assert!(!terminal.review_create());
            assert!(
                terminal
                    .apply(Action::ConfirmCreate(Review { ticket: t.clone() }), &stale)
                    .is_none()
            );
            assert!(
                terminal
                    .apply(
                        Action::ReviewClose {
                            ticket: t.clone(),
                            id: tid("PUBLIC-terminal-1"),
                        },
                        &stale,
                    )
                    .is_none()
            );
            assert!(terminal.apply(Action::Write(t), &stale).is_none());
            assert!(!terminal.create_pending() && !terminal.close_pending());
        }
        let mut terminal = opened();
        let denied = Context {
            view: context().view,
            allowed: false,
        };
        let t = ticket(&terminal);
        assert!(
            terminal
                .apply(Action::ReviewCreate(t.clone()), &denied)
                .is_none()
        );
        assert!(!terminal.review_create());
    }

    #[test]
    fn a_missing_output_frame_is_reported_rather_than_swallowed() {
        let mut terminal = opened();
        let follow = select(&mut terminal);
        for (sequence, data) in [(1, "PUBLIC first"), (2, "PUBLIC second")] {
            terminal.frame(
                &follow,
                TerminalFrame::Output {
                    sequence,
                    data: data.into(),
                },
            );
        }
        assert_eq!(terminal.output().text(), "PUBLIC firstPUBLIC second");
        // Frame 3 never arrived: the screen can no longer be trusted to match the Host.
        terminal.frame(
            &follow,
            TerminalFrame::Output {
                sequence: 4,
                data: "PUBLIC after the gap".into(),
            },
        );
        assert_eq!(terminal.notice(), &Notice::OutputGap);
        assert!(
            !terminal.output().text().contains("after the gap"),
            "the untrustworthy tail is not merged into the screen"
        );
        // A snapshot re-establishes the base, so the stream continues cleanly from it.
        terminal.frame(
            &follow,
            TerminalFrame::Snapshot {
                sequence: 9,
                screen: "PUBLIC resynchronised".into(),
                info: info("PUBLIC-terminal-1"),
            },
        );
        assert_eq!(terminal.output().text(), "PUBLIC resynchronised");
        assert_ne!(terminal.notice(), &Notice::OutputGap);
        terminal.frame(
            &follow,
            TerminalFrame::Output {
                sequence: 10,
                data: "PUBLIC next".into(),
            },
        );
        assert_ne!(terminal.notice(), &Notice::OutputGap);
        assert_eq!(terminal.output().text(), "PUBLIC resynchronisedPUBLIC next");
    }

    #[test]
    fn a_key_sends_its_bytes_without_touching_the_typed_line() {
        let mut terminal = opened();
        let _follow = select(&mut terminal);
        let t = ticket(&terminal);
        let _ = terminal.apply(
            Action::EditInput {
                ticket: t.clone(),
                text: "PUBLIC typed".into(),
            },
            &context(),
        );
        let Some(Effect::Write(submission)) = terminal.apply(
            Action::Key {
                ticket: t.clone(),
                data: "\r".into(),
            },
            &context(),
        ) else {
            panic!("a keystroke is one write");
        };
        assert_eq!(submission.data, "\r");
        // The field is untouched: the reference's terminal has no line for a key to land in.
        assert_eq!(terminal.input(), "PUBLIC typed");
        // An empty keystroke is not a write at all.
        terminal.frame(
            &_follow,
            TerminalFrame::State {
                info: info("PUBLIC-terminal-1"),
            },
        );
        assert!(
            terminal
                .apply(
                    Action::Key {
                        ticket: t,
                        data: String::new(),
                    },
                    &context()
                )
                .is_none(),
            "an empty keystroke sends nothing"
        );
    }

    #[test]
    fn foreign_frames_and_receipts_are_dropped() {
        let mut terminal = opened();
        let follow = select(&mut terminal);
        terminal.frame(
            &follow,
            TerminalFrame::Output {
                sequence: 1,
                data: "PUBLIC first".into(),
            },
        );
        assert_eq!(terminal.output().text(), "PUBLIC first");
        // A frame for a retired follow ticket is ignored.
        let mut foreign = follow.clone();
        foreign.serial += 1;
        terminal.frame(
            &foreign,
            TerminalFrame::Output {
                sequence: 2,
                data: "PUBLIC injected".into(),
            },
        );
        assert_eq!(terminal.output().text(), "PUBLIC first");
        // A snapshot replaces the bounded screen.
        terminal.frame(
            &follow,
            TerminalFrame::Snapshot {
                sequence: 0,
                screen: "PUBLIC screen".into(),
                info: info("PUBLIC-terminal-1"),
            },
        );
        assert_eq!(terminal.output().text(), "PUBLIC screen");
        terminal.follow_ended(&foreign, None);
        assert!(terminal.follow().is_some(), "foreign end is ignored");
        terminal.follow_ended(&follow, Some(ReadFailure::Unavailable));
        assert!(terminal.follow().is_none());
        assert_eq!(terminal.notice(), &Notice::FollowFailed);
    }

    #[test]
    fn disconnect_fences_late_receipts_and_never_claims_rollback() {
        let mut terminal = opened();
        let submission = create(&mut terminal);
        terminal.disconnect();
        assert!(!terminal.is_open());
        assert!(!terminal.create_pending());
        assert_eq!(terminal.notice(), &Notice::CreatingIndeterminate);
        assert!(
            terminal
                .finish_create(
                    &submission.receipt,
                    CreateOutcome::Created(Box::new(info("PUBLIC-terminal-1")))
                )
                .is_none()
        );
    }

    #[test]
    fn a_malformed_frame_info_never_overwrites_confirmed_list_state() {
        let mut terminal = opened();
        let follow = select(&mut terminal);
        let mut malformed = info("PUBLIC-terminal-1");
        malformed.state = dsh_native_transport::terminal::TerminalState::Running;
        malformed.exit_code = Some(0);
        terminal.frame(&follow, TerminalFrame::State { info: malformed });
        assert_eq!(terminal.selected_info().unwrap().exit_code, None);
        terminal.frame(
            &follow,
            TerminalFrame::Snapshot {
                sequence: 0,
                screen: "PUBLIC".into(),
                info: {
                    let mut bad = info("PUBLIC-terminal-1");
                    bad.cols = 1;
                    bad
                },
            },
        );
        assert_ne!(terminal.selected_info().unwrap().cols, 1);
    }
}
