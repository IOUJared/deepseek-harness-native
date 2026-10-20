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
/// Every terminal action that mutates Host state, in the order the panel's copy names them. The
/// sentence below is generated from this list, so adding an action cannot leave the copy listing
/// only the older ones.
pub const MUTATING_ACTIONS: [&str; 5] = ["create", "write", "rename", "resize", "close"];
/// The panel's limits-unavailable sentence, generated so it never lists a stale subset.
pub fn limits_unavailable_copy() -> String {
    let names = MUTATING_ACTIONS;
    let (last, rest) = names.split_last().expect("a non-empty action list");
    format!(
        "Host limits unavailable: this Session has no registered Agent. History is listed below, but {} and {last} cannot run.",
        rest.join(", ")
    )
}
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
    /// Whether the shell-profile read failed. Distinct from an empty list: the Host serves no
    /// profiles on some builds, and a failed read is the operator's cue to re-read
    /// (`TerminalGuide.tsx:53-64`).
    pub shells_failed: bool,
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
/// Redacted resize outcome: only the Host's ACK states the applied dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeOutcome {
    Resized,
    Unavailable,
    NotSent,
    Unknown,
}

/// Redacted rename outcome: only the Host's ACK is a title authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameOutcome {
    Renamed,
    Unavailable,
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
/// One admitted resize, fenced to the panel's writable attachment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResizeSubmission {
    pub receipt: ResizeReceipt,
}
/// A resize the panel reviewed and admitted. The Host applies it to the attached PTY; the panel
/// does not resize anything itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResizeReceipt {
    pub ticket: Ticket,
    pub id: WebTerminalId,
    pub attachment: TerminalAttachmentId,
    pub cols: u64,
    pub rows: u64,
}
/// A rename the panel reviewed and admitted: the Host's ACK is the only title authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameReceipt {
    pub ticket: Ticket,
    pub id: WebTerminalId,
    title: String,
}
impl RenameReceipt {
    #[cfg(test)]
    pub fn title(&self) -> &str {
        &self.title
    }
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
/// One reviewed resize: the terminal it targets and the dimensions being requested.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResizeReview {
    pub ticket: Ticket,
    pub id: WebTerminalId,
    pub cols: u64,
    pub rows: u64,
}
/// One reviewed rename: the terminal it targets and the title being edited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameReview {
    pub ticket: Ticket,
    pub id: WebTerminalId,
    pub title: String,
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
/// One admitted rename, carrying the title the operator reviewed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameSubmission {
    pub receipt: RenameReceipt,
    data: String,
}
impl RenameSubmission {
    pub fn title(&self) -> &str {
        &self.data
    }
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
    RenameReview,
    Renaming,
    Renamed,
    RenameFailed,
    RenamingIndeterminate,
    RenameNotSent,
    ResizeReview,
    Resizing,
    Resized,
    ResizeFailed,
    ResizingIndeterminate,
    ResizeNotSent,
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
    Rename(RenameSubmission),
    Resize(ResizeSubmission),
    Write(WriteSubmission),
}

/// How many lines the screen keeps before it drops the oldest; a screen has no scrollback of its
/// own, so this is the panel's own bound and the drop is counted the way a chunk drop is.
const SCREEN_LINES: usize = 2_000;

/// The most rows one paint renders: a rich-text row per line, so a 2,000-line scrollback must not
/// cost 2,000 of them. The newest rows are the ones an operator is reading.
const RENDERED_ROWS: usize = 400;

/// One screen line, written left to right by a cursor that `\r`, `\b` and the erase sequences move.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Screen {
    /// Lines that scrolled off the grid, oldest first: a terminal's scrollback.
    history: Vec<Vec<Cell>>,
    /// The visible grid, exactly `rows` lines once a size is known (one until then).
    grid: Vec<Vec<Cell>>,
    row: usize,
    column: usize,
    rows: usize,
    /// The Host's column count: a glyph written past it wraps to the next row, the way a shell's own
    /// line does, instead of drawing one endless row.
    cols: usize,
    /// The scrolling region, 0-based and inclusive: `CSI t;b r` keeps a status row fixed while the
    /// body scrolls, so a newline at the region's bottom scrolls only the region.
    top: usize,
    bottom: usize,
    /// Autowrap (`DECAWM`, `CSI ? 7 h`/`l`): on by default, so a glyph past the last column wraps; a
    /// program turns it off to overwrite the last column instead.
    wrap: bool,
    dropped: usize,
    /// The SGR colour the next written cell takes: an ANSI index 0–15, or none for the panel's
    /// default ink.
    ink: Option<u8>,
    /// SGR `4` for the cells written next.
    underline: bool,
    /// SGR `40`–`47`/`100`–`107` for the cells written next.
    paper: Option<u8>,
    /// `ESC 7` saves the cursor and its attributes; `ESC 8` restores them (DECSC/DECRC), which a large
    /// share of full-screen programs use around a redraw.
    saved_cursor: Option<(usize, usize, Option<u8>, bool, Option<u8>)>,
    /// The screen a full-screen program switched away from, restored when it leaves the alternate
    /// screen (`CSI ? 1049 h`/`l`): without it a `vim` session would scribble over the shell history
    /// and the history would not come back.
    alternate: Option<Box<Saved>>,
}

/// One saved screen: everything a full-screen program's exit restores.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Saved {
    history: Vec<Vec<Cell>>,
    grid: Vec<Vec<Cell>>,
    row: usize,
    column: usize,
    ink: Option<u8>,
    underline: bool,
    paper: Option<u8>,
}

/// One screen cell: a character and the colour it was written with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    character: char,
    ink: Option<u8>,
    /// SGR `4`: drawn with an underline.
    underline: bool,
    /// SGR `40`–`47`/`100`–`107`: the cell's background, or none for the panel's own.
    background: Option<u8>,
    /// The second column of a wide character: it draws nothing of its own, so a CJK glyph keeps its
    /// two-column width without the line collecting a phantom space.
    continuation: bool,
}

impl Cell {
    fn blank() -> Self {
        Self {
            character: ' ',
            ink: None,
            underline: false,
            background: None,
            continuation: false,
        }
    }
}

/// The columns one character occupies: two for the East Asian wide and fullwidth ranges and for
/// emoji, one for everything else. A single-column terminal would misalign a CJK prompt against the
/// text below it, which is what a `CSI H` layout notices first.
fn columns(character: char) -> usize {
    // A combining mark has no column of its own: the grid keeps one character per cell and cannot
    // compose it onto the base, so it is dropped rather than drawn beside it as a stray box.
    let mark = matches!(character as u32,
        0x0300..=0x036F
            | 0x0483..=0x0489
            | 0x0591..=0x05BD
            | 0x0610..=0x061A
            | 0x064B..=0x065F
            | 0x0670
            | 0x06D6..=0x06DC
            | 0x0900..=0x0903
            | 0x093E..=0x094F
            | 0x0E31
            | 0x0E34..=0x0E3A
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20F0
            | 0xFE20..=0xFE2F
    );
    if mark {
        return 0;
    }
    let c = character as u32;
    let wide = matches!(c,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1F64F
            | 0x1F900..=0x1F9FF
            | 0x20000..=0x3FFFD);
    if wide { 2 } else { 1 }
}
impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}
impl Screen {
    fn new() -> Self {
        Self {
            history: Vec::new(),
            grid: vec![Vec::new()],
            row: 0,
            column: 0,
            // No size yet: the grid grows instead of scrolling, so text written before the Host
            // reports its rows is not scrolled into scrollback and later "cleared".
            rows: 0,
            cols: 0,
            top: 0,
            // `usize::MAX` is "the last row": the region is the whole screen until a program sets
            // one, and every use clamps it to the grid.
            bottom: usize::MAX,
            wrap: true,
            dropped: 0,
            ink: None,
            underline: false,
            paper: None,
            saved_cursor: None,
            alternate: None,
        }
    }
    /// Adopt the Host's size: the grid becomes that tall and that wide.
    fn set_size(&mut self, cols: u64, rows: u64) {
        // Zero means "no column bound known yet": the screen grows instead of wrapping.
        self.cols = if cols == 0 {
            0
        } else {
            usize::try_from(cols).unwrap_or(usize::MAX).clamp(1, 500)
        };
        self.set_rows(rows);
    }
    /// Adopt the Host's row count: the grid becomes that tall, keeping the newest lines.
    fn set_rows(&mut self, rows: u64) {
        let rows = usize::try_from(rows).unwrap_or(usize::MAX).clamp(1, 500);
        // A fresh screen (one empty line) simply becomes `rows` empty lines: nothing above it to
        // keep, and the cursor stays at the top.
        if self.grid.len() == 1 && self.grid[0].is_empty() {
            self.grid = vec![Vec::new(); rows];
            self.row = 0;
            self.rows = rows;
            return;
        }
        while self.grid.len() < rows {
            self.grid.insert(0, Vec::new());
            // Existing rows keep their text; the cursor follows its own line down.
            self.row = self.row.saturating_add(1);
        }
        while self.grid.len() > rows {
            self.history.push(self.grid.remove(0));
            self.row = self.row.saturating_sub(1);
        }
        self.row = self.row.min(self.grid.len().saturating_sub(1));
        self.rows = rows;
    }
    fn line(&mut self) -> &mut Vec<Cell> {
        if self.grid.is_empty() {
            self.grid.push(Vec::new());
            self.row = 0;
        }
        let row = self.row.min(self.grid.len() - 1);
        &mut self.grid[row]
    }
    fn newline(&mut self) {
        if self.rows == 0 {
            // Unbounded until a size arrives; the history bound still applies.
            self.grid.push(Vec::new());
            self.row = self.grid.len() - 1;
            while self.grid.len() > SCREEN_LINES {
                self.grid.remove(0);
                self.dropped = self.dropped.saturating_add(1);
                self.row = self.row.saturating_sub(1);
            }
            self.column = 0;
            return;
        }
        if self.row + 1 < self.grid.len() {
            self.row += 1;
        } else {
            // The bottom row scrolls: the oldest grid line becomes scrollback.
            self.history.push(self.grid.remove(0));
            self.grid.push(Vec::new());
            while self.history.len() > SCREEN_LINES {
                self.history.remove(0);
                self.dropped = self.dropped.saturating_add(1);
            }
        }
        self.column = 0;
    }
    /// Write one character at the cursor, overwriting the column it lands on.
    fn put(&mut self, character: char) {
        // A glyph that would end past the last column wraps first: the shell's own line discipline
        // starts the next row rather than letting the screen grow sideways.
        let width = columns(character);
        if width == 0 {
            return;
        }
        let paper = self.paper;
        if self.wrap && self.cols > 0 && self.column + width > self.cols {
            self.newline();
        }
        // With autowrap off the cursor parks on the last column and every further glyph overwrites
        // it, which is what a progress bar at the screen's edge relies on.
        let column = if !self.wrap && self.cols > 0 {
            self.column.min(self.cols - 1)
        } else {
            self.column
        };
        let ink = self.ink;
        let cell = Cell {
            character,
            ink,
            underline: self.underline,
            background: paper,
            continuation: false,
        };
        let line = self.line();
        while line.len() <= column + width - 1 {
            line.push(Cell::blank());
        }
        line[column] = cell;
        if width == 2 {
            line[column + 1] = Cell {
                character: ' ',
                ink,
                underline: cell.underline,
                background: paper,
                continuation: true,
            };
        }
        self.column = column + width;
    }
    /// Erase in line, relative to the cursor row.
    fn erase_line(&mut self, mode: u32) {
        let column = self.column;
        let line = self.line();
        match mode {
            0 => line.truncate(column),
            1 => {
                let end = column.min(line.len().saturating_sub(1));
                for cell in line.iter_mut().take(end + 1) {
                    *cell = Cell::blank();
                }
            }
            _ => line.clear(),
        }
    }
    /// Erase in display: 0 blanks from the cursor to the end of the grid, 1 blanks up to the cursor,
    /// 2 clears the grid. Scrollback survives every mode, as a terminal's does.
    fn erase_display(&mut self, mode: u32) {
        match mode {
            // `CSI 3 J` is the only mode that clears the scrollback; a shell's `clear -x` uses it.
            3 => self.history.clear(),
            0 => {
                let column = self.column;
                let row = self.row;
                for line in self.grid.iter_mut().skip(row + 1) {
                    line.clear();
                }
                if let Some(line) = self.grid.get_mut(row) {
                    line.truncate(column);
                }
            }
            1 => {
                // `CSI 1 J` blanks from the start of the screen to the cursor and leaves everything
                // after the cursor alone; truncating the cursor's row would delete the rest of it.
                let column = self.column;
                let row = self.row;
                for index in 0..row {
                    if let Some(line) = self.grid.get_mut(index) {
                        line.clear();
                    }
                }
                if let Some(line) = self.grid.get_mut(row) {
                    let end = column.min(line.len().saturating_sub(1));
                    for cell in line.iter_mut().take(end + 1) {
                        *cell = Cell::blank();
                    }
                }
            }
            _ => {
                // `CSI 2 J` clears the display and leaves the cursor where it was; programs that want
                // the cursor home send `CSI H` as well. Resetting it here moved every following glyph.
                for line in self.grid.iter_mut() {
                    line.clear();
                }
            }
        }
    }
    /// Switch to (or back from) the alternate screen, saving or restoring the whole screen.
    fn alternate_screen(&mut self, enter: bool) {
        if enter {
            if self.alternate.is_some() {
                return;
            }
            self.alternate = Some(Box::new(Saved {
                history: std::mem::take(&mut self.history),
                grid: std::mem::replace(&mut self.grid, vec![Vec::new(); self.rows.max(1)]),
                row: self.row,
                column: self.column,
                ink: self.ink,
                underline: self.underline,
                paper: self.paper,
            }));
            self.row = 0;
            self.column = 0;
            self.ink = None;
            self.underline = false;
            self.paper = None;
        } else if let Some(saved) = self.alternate.take() {
            self.history = saved.history;
            self.grid = saved.grid;
            self.row = saved.row;
            self.column = saved.column;
            self.ink = saved.ink;
            self.underline = saved.underline;
            self.paper = saved.paper;
        }
    }
    /// Insert `count` blank rows at the cursor, pushing the rows below down; the grid keeps its
    /// height, so the bottom rows fall out, as a terminal's do.
    fn insert_lines(&mut self, count: usize) {
        // A parsed count is bounded by the grid: `CSI 999999999 L` must not insert a billion rows
        // before the oldest fall off. Real terminals clamp to the screen; so does this.
        let count = count.min(self.grid.len());
        let row = self.row.min(self.grid.len().saturating_sub(1));
        let bottom = self.bottom.min(self.grid.len().saturating_sub(1));
        for _ in 0..count {
            if row <= bottom {
                self.grid.insert(row, Vec::new());
                // The grid keeps its height only once the Host states one; before that it grows, so
                // an insert must not drop the last row.
                if self.rows > 0 {
                    self.grid.remove(bottom + 1);
                }
            }
        }
        self.column = 0;
    }
    /// Delete `count` rows at the cursor, pulling the rows below up and blanking the bottom.
    fn delete_lines(&mut self, count: usize) {
        let count = count.min(self.grid.len());
        let row = self.row.min(self.grid.len().saturating_sub(1));
        let bottom = self.bottom.min(self.grid.len().saturating_sub(1));
        for _ in 0..count {
            if row <= bottom {
                self.grid.remove(row);
                // A bounded grid refills from the bottom; an unbounded one simply shrinks.
                if self.rows > 0 {
                    self.grid.insert(bottom, Vec::new());
                }
            }
        }
        self.column = 0;
    }
    /// Delete `count` characters at the cursor, shifting the rest of the row left and padding the end.
    /// Erase `count` characters at the cursor without shifting the rest of the row (`CSI N X`).
    fn erase_chars(&mut self, count: usize) {
        let count = count.min(self.cols.max(self.line().len()).max(1));
        let column = self.column;
        let line = self.line();
        let end = (column + count).min(line.len());
        for cell in line.iter_mut().take(end).skip(column) {
            *cell = Cell::blank();
        }
    }
    fn delete_chars(&mut self, count: usize) {
        // The same bound for characters: a row cannot hold more than its own length (or the width).
        let count = count.min(self.cols.max(self.line().len()).max(1));
        let column = self.column;
        let line = self.line();
        let end = (column + count).min(line.len());
        line.drain(column..end);
    }
    /// Insert `count` blanks at the cursor, shifting the rest of the row right and dropping overflow.
    fn insert_chars(&mut self, count: usize) {
        // Bounded by the width when it is known; a screen with no width yet still refuses an absurd
        // count rather than allocating for it.
        let count = if self.cols > 0 {
            count.min(self.cols)
        } else {
            count.min(1024)
        };
        let column = self.column;
        let cols = self.cols;
        let line = self.line();
        for _ in 0..count {
            line.insert(column.min(line.len()), Cell::blank());
        }
        if cols > 0 {
            line.truncate(cols);
        }
    }
    /// Scroll the region's contents up (`CSI N S`): lines move toward the top and blanks join at the
    /// bottom. The cursor does not move, unlike a newline at the bottom of the region.
    fn scroll_up(&mut self, count: usize) {
        let top = self.top.min(self.grid.len().saturating_sub(1));
        let bottom = self.bottom.min(self.grid.len().saturating_sub(1));
        let count = count.min(bottom.saturating_sub(top) + 1);
        for _ in 0..count {
            self.grid.remove(top);
            self.grid.insert(bottom, Vec::new());
        }
    }
    /// Scroll the region's contents down (`CSI N T`): blanks join at the top.
    fn scroll_down(&mut self, count: usize) {
        let top = self.top.min(self.grid.len().saturating_sub(1));
        let bottom = self.bottom.min(self.grid.len().saturating_sub(1));
        let count = count.min(bottom.saturating_sub(top) + 1);
        for _ in 0..count {
            self.grid.remove(bottom);
            self.grid.insert(top, Vec::new());
        }
    }
    /// Move the cursor within the grid, clamped to it.
    fn place(&mut self, row: usize, column: usize) {
        self.row = row.min(self.grid.len().saturating_sub(1));
        self.column = column;
        self.clamp_column();
    }
    /// Keep the cursor's column inside the screen. A parsed column is untrusted: `CSI 1;999999999 H`
    /// followed by a glyph would otherwise pad a line with a billion blanks before writing — the same
    /// class as the tab loop (round 589) and the insert counts (round 592).
    fn clamp_column(&mut self) {
        let limit = if self.cols > 0 { self.cols - 1 } else { 10_000 };
        self.column = self.column.min(limit);
    }
    /// Consume one chunk of raw Host bytes, applying the redraw, erase and addressing rules.
    fn feed(&mut self, data: &str) {
        let mut chars = data.chars().peekable();
        while let Some(character) = chars.next() {
            match character {
                '\u{1b}' => {
                    if chars.peek() == Some(&'[') {
                        chars.next();
                        let mut params = String::new();
                        let mut final_byte = None;
                        for c in chars.by_ref() {
                            if ('\u{40}'..='\u{7e}').contains(&c) {
                                final_byte = Some(c);
                                break;
                            }
                            params.push(c);
                        }
                        let private = params.starts_with('?');
                        let numbers: Vec<u32> = params
                            .trim_start_matches('?')
                            .split(';')
                            .filter(|part| !part.is_empty())
                            .map(|part| part.parse().unwrap_or(0))
                            .collect();
                        let first = numbers.first().copied().unwrap_or(0).max(1) as usize;
                        let second = numbers.get(1).copied().unwrap_or(0);
                        match final_byte {
                            // The alternate screen: a full-screen program's own buffer. Entering
                            // saves the shell's screen and starts a cleared grid; leaving restores
                            // it. `47`/`1047` are the same switch without the cursor save, which
                            // this screen does not distinguish.
                            Some('h') | Some('l')
                                if private
                                    && numbers.iter().any(|n| [1047, 1049, 47].contains(n)) =>
                            {
                                let enter = final_byte == Some('h');
                                self.alternate_screen(enter)
                            }
                            // `CSI ? 1048 h`/`l` save and restore the cursor, the mode form of `ESC 7`/`ESC 8`.
                            Some('h') if private && numbers.contains(&1048) => {
                                self.saved_cursor = Some((
                                    self.row,
                                    self.column,
                                    self.ink,
                                    self.underline,
                                    self.paper,
                                ));
                            }
                            Some('l') if private && numbers.contains(&1048) => {
                                if let Some((row, column, ink, underline, paper)) =
                                    self.saved_cursor
                                {
                                    self.place(row, column);
                                    self.ink = ink;
                                    self.underline = underline;
                                    self.paper = paper;
                                }
                            }
                            // `DECAWM`: autowrap on or off.
                            Some('h') if private && numbers.contains(&7) => self.wrap = true,
                            Some('l') if private && numbers.contains(&7) => self.wrap = false,
                            Some('r') if !private => {
                                // `CSI t;b r`: 1-based, both defaulting to the full screen. A
                                // region homes to its own first row, not the screen's.
                                let top = numbers.first().copied().unwrap_or(1).max(1) as usize - 1;
                                let bottom = numbers
                                    .get(1)
                                    .copied()
                                    .unwrap_or(self.grid.len() as u32)
                                    .max(1) as usize
                                    - 1;
                                if top < bottom && bottom < self.grid.len() {
                                    self.top = top;
                                    self.bottom = bottom;
                                    self.place(top, 0);
                                }
                            }
                            Some('X') if !private => self.erase_chars(first),
                            Some('S') if !private => self.scroll_up(first),
                            Some('T') if !private => self.scroll_down(first),
                            Some('L') if !private => self.insert_lines(first),
                            Some('M') if !private => self.delete_lines(first),
                            Some('P') if !private => self.delete_chars(first),
                            Some('@') if !private => self.insert_chars(first),
                            Some('K') if !private => {
                                self.erase_line(numbers.first().copied().unwrap_or(0))
                            }
                            Some('J') if !private => {
                                self.erase_display(numbers.first().copied().unwrap_or(0))
                            }
                            Some('m') if !private => {
                                for code in numbers.iter().copied() {
                                    match code {
                                        0 => {
                                            self.ink = None;
                                            self.underline = false;
                                            self.paper = None;
                                        }
                                        // Bold brightens the base colour, the way a terminal's
                                        // bold attribute is drawn.
                                        1 => self.ink = self.ink.map(|ink| (ink % 8) + 8),
                                        // `39` restores the default foreground; `22` cancels bold,
                                        // which the panel does not draw separately.
                                        // Underline on, double underline (drawn as on), and off.
                                        4 | 21 => self.underline = true,
                                        24 => self.underline = false,
                                        40..=47 => self.paper = Some((code - 40) as u8),
                                        100..=107 => self.paper = Some((code - 100 + 8) as u8),
                                        49 => self.paper = None,
                                        39 => self.ink = None,
                                        // Bold brightened the base colour, so cancelling bold returns
                                        // it to the base half rather than leaving it bright.
                                        22 => self.ink = self.ink.map(|ink| ink % 8),
                                        30..=37 => self.ink = Some((code - 30) as u8),
                                        90..=97 => self.ink = Some((code - 90 + 8) as u8),
                                        // Background and every other attribute are not drawn.
                                        _ => {}
                                    }
                                }
                            }
                            Some('G') if !private => {
                                self.column = first - 1;
                                self.clamp_column();
                            }
                            Some('d') if !private => self.place(first - 1, self.column),
                            Some('H') | Some('f') if !private => {
                                let column = second.max(1) as usize - 1;
                                self.place(first - 1, column);
                            }
                            Some('A') if !private => {
                                self.row = self.row.saturating_sub(first).max(self.top);
                            }
                            Some('B') if !private => {
                                let bottom = self.bottom.min(self.grid.len().saturating_sub(1));
                                self.row = (self.row + first).min(bottom);
                            }
                            Some('C') if !private => {
                                self.column = self.column.saturating_add(first);
                                self.clamp_column();
                            }
                            Some('D') if !private => {
                                self.column = self.column.saturating_sub(first)
                            }
                            // Every other sequence is consumed silently: the panel draws text, not
                            // a complete terminal, and a guess would corrupt what it does draw. A
                            // private `?` mode (alternate screen, cursor visibility) is not text.
                            _ => {}
                        }
                    } else if chars.peek() == Some(&']') {
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
                    } else if chars.peek() == Some(&'7') {
                        chars.next();
                        self.saved_cursor =
                            Some((self.row, self.column, self.ink, self.underline, self.paper));
                    } else if chars.peek() == Some(&'8') {
                        chars.next();
                        if let Some((row, column, ink, underline, paper)) = self.saved_cursor {
                            self.place(row, column);
                            self.ink = ink;
                            self.underline = underline;
                            self.paper = paper;
                        }
                    } else {
                        chars.next();
                    }
                }
                '\r' => self.column = 0,
                '\n' => self.newline(),
                '\u{8}' => self.column = self.column.saturating_sub(1),
                '\t' => {
                    // Clamped to the last column: with autowrap on, `put` would wrap at the margin
                    // and a tab stop past it would never be reached — an infinite loop on a byte the
                    // Host can send (found by the adversarial test in round 588).
                    let mut next = (self.column / 8 + 1) * 8;
                    if self.cols > 0 {
                        next = next.min(self.cols - 1);
                    }
                    while self.column < next {
                        self.put(' ');
                    }
                }
                c if c.is_control() => {}
                c => self.put(c),
            }
        }
    }
    #[cfg(test)]
    fn render(&self) -> String {
        // The whole screen: what scrolled off, then the grid. Trailing blank rows are the cleared
        // space below the cursor, so they collapse to one newline rather than a run of them.
        let mut blocks: Vec<String> = Vec::new();
        for line in self.history.iter().chain(self.grid.iter()) {
            blocks.push(
                line.iter()
                    .filter(|cell| !cell.continuation)
                    .map(|cell| cell.character)
                    .collect(),
            );
        }
        let mut trailing = 0;
        while blocks.last().is_some_and(|line| line.is_empty()) {
            blocks.pop();
            trailing += 1;
        }
        // A trailing newline is the cursor standing on a fresh line, not merely a cleared row
        // below the output, so the kept grid rows decide it.
        let kept_grid = blocks.len().saturating_sub(self.history.len());
        let mut out = blocks.join("\n");
        if trailing > 0 && !out.is_empty() && self.row >= kept_grid {
            out.push('\n');
        }
        out
    }
}

/// A bounded, sanitized plain-text view of Host terminal output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    chunks: Vec<String>,
    bytes: usize,
    dropped: usize,
    /// The screen the panel draws: lines with a cursor, fed from the same raw bytes as the chunks.
    screen: Screen,
    /// Bytes a single oversized chunk lost at [`CHUNK_BYTES`]. Counted separately from whole-chunk
    /// eviction so the panel's "never silently hidden" copy stays true.
    truncated: usize,
}
impl Output {
    /// Rows only, for the tests whose subject is scrolling rather than wrapping.
    #[cfg(test)]
    pub fn set_rows(&mut self, rows: u64) {
        self.screen.set_size(0, rows);
    }
    /// Adopt the Host's size: the row count bounds the grid, the column count wraps a long line.
    pub fn set_size(&mut self, cols: u64, rows: u64) {
        self.screen.set_size(cols, rows);
    }
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.bytes = 0;
        self.dropped = 0;
        self.truncated = 0;
        self.screen = Screen::new();
    }
    /// Append one sanitized chunk, evicting the oldest chunks under both bounds.
    pub fn push(&mut self, data: &str) {
        self.screen.feed(data);
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
    /// The screen as coloured runs, one per line: `(text, ANSI colour)` where `None` is the panel's
    /// own ink. This is what the panel draws; [`Self::display`] is the same text without colour.
    pub fn display_styled(
        &self,
        max_bytes: usize,
    ) -> Vec<Vec<(String, Option<u8>, bool, Option<u8>)>> {
        let mut lines: Vec<Vec<(String, Option<u8>, bool, Option<u8>)>> = Vec::new();
        for line in self.screen.history.iter().chain(self.screen.grid.iter()) {
            let mut runs: Vec<(String, Option<u8>, bool, Option<u8>)> = Vec::new();
            for cell in line.iter().filter(|cell| !cell.continuation) {
                match runs.last_mut() {
                    Some((text, ink, underline, background))
                        if *ink == cell.ink
                            && *underline == cell.underline
                            && *background == cell.background =>
                    {
                        text.push(cell.character)
                    }
                    _ => runs.push((
                        cell.character.to_string(),
                        cell.ink,
                        cell.underline,
                        cell.background,
                    )),
                }
            }
            lines.push(runs);
        }
        // Collapse the cleared rows below the cursor the same way `display` does.
        while lines
            .last()
            .is_some_and(|runs| runs.iter().all(|(text, ..)| text.trim().is_empty()))
        {
            lines.pop();
        }
        let single = lines.len() == 1;
        lines.retain(|runs| !runs.is_empty() || single);
        let mut used = 0usize;
        let mut kept: Vec<Vec<(String, Option<u8>, bool, Option<u8>)>> = Vec::new();
        for runs in lines.into_iter().rev() {
            let size: usize = runs.iter().map(|(text, ..)| text.len()).sum::<usize>() + 1;
            if used + size > max_bytes && !kept.is_empty() {
                break;
            }
            used += size;
            kept.push(runs);
        }
        kept.reverse();
        // A paint costs one rich-text row per line, so the panel draws the newest rows a screen can
        // plausibly show rather than the whole scrollback; the byte bound above usually trims first.
        if kept.len() > RENDERED_ROWS {
            kept.drain(..kept.len() - RENDERED_ROWS);
        }
        kept
    }
    /// The screen as plain text, newest lines kept: the tests read this, the panel draws the
    /// styled runs from [`Self::display_styled`].
    #[cfg(test)]
    ///
    /// The screen applies the redraw and erase rules a real terminal applies — `\r` overwrites in
    /// place, `ESC [ K` erases part of a line, `ESC [ 2 J` clears the display — while the retained
    /// chunks stay untouched, so the bounds and the dropped/truncated accounting still describe the
    /// Host's bytes.
    pub fn display(&self, max_bytes: usize) -> String {
        let mut text = self.screen.render();
        if text.len() <= max_bytes {
            return text;
        }
        let mut cut = text.len() - max_bytes;
        while cut < text.len() && !text.is_char_boundary(cut) {
            cut += 1;
        }
        text.drain(..cut);
        text
    }
    /// Newest retained output bounded to `max_bytes`: the raw bound the retention tests read.
    #[cfg(test)]
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

/// Strip terminal control sequences for the plain-text copy; the screen applies them instead.
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
            // The carriage return is kept: the display applies it as an in-place redraw, so a
            // progress bar does not become one line per update.
            '\r' => out.push('\r'),
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
    /// Enter the rename review for one listed terminal.
    ReviewRename { ticket: Ticket, id: WebTerminalId },
    /// Leave the rename review without writing.
    BackRename(Ticket),
    /// Edit the reviewed title.
    EditRename { ticket: Ticket, title: String },
    /// Confirm the reviewed rename: one Host write, never retried.
    ConfirmRename(RenameReview),
    /// Enter the resize review for one listed terminal.
    ReviewResize { ticket: Ticket, id: WebTerminalId },
    /// Leave the resize review without writing.
    BackResize(Ticket),
    /// Edit the reviewed dimensions.
    EditResize {
        ticket: Ticket,
        cols: u64,
        rows: u64,
    },
    /// Confirm the reviewed resize: one Host PTY update, never retried.
    ConfirmResize(ResizeReview),
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
    /// Whether the last shell-profile read itself failed, rather than answering with no profiles.
    shells_failed: bool,
    shell: Option<usize>,
    environment: Option<TerminalEnvironment>,
    selected: Option<WebTerminalId>,
    attachment: Option<TerminalAttachmentId>,
    follow: Option<FollowTicket>,
    /// The last output frame's sequence, so a skipped one is noticed instead of shortening the
    /// screen (`ui-sidebar-terminal/src/client/model.ts:294`).
    last_sequence: Option<i64>,
    /// The shell the user last chose, by path: the reference remembers it
    /// (`api/terminal-controller/src/client/shell-preference.ts:2`, `model.ts:125-129`), so opening the
    /// panel again offers the same shell instead of the first one discovered.
    preferred_shell: Option<String>,
    output: Output,
    input: String,
    editor: u64,
    max_input_bytes: u64,
    create_review: Option<Review>,
    create_pending: Option<CreateReceipt>,
    close_review: Option<CloseReview>,
    rename_review: Option<RenameReview>,
    rename_pending: Option<RenameReceipt>,
    resize_review: Option<ResizeReview>,
    resize_pending: Option<ResizeReceipt>,
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
            shells_failed: false,
            shell: None,
            environment: None,
            selected: None,
            attachment: None,
            follow: None,
            last_sequence: None,
            preferred_shell: None,
            output: Output::default(),
            input: String::new(),
            editor: 0,
            max_input_bytes: MAX_INPUT_BYTES as u64,
            create_review: None,
            create_pending: None,
            close_review: None,
            rename_review: None,
            rename_pending: None,
            resize_review: None,
            resize_pending: None,
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
    pub fn shells_failed(&self) -> bool {
        self.shells_failed
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
    /// The shell the user last chose, for the layout preference that remembers it across launches.
    pub fn preferred_shell(&self) -> Option<&str> {
        self.preferred_shell.as_deref()
    }
    /// Adopt the shell a previous launch stored, before any Host list arrives.
    pub fn adopt_preferred_shell(&mut self, path: Option<String>) {
        if self.preferred_shell.is_none() {
            self.preferred_shell = path;
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
    /// The rename review, when one is open: its target and the title being edited.
    pub fn review_rename(&self) -> Option<&RenameReview> {
        self.rename_review.as_ref()
    }
    /// The resize review, when one is open.
    pub fn review_resize(&self) -> Option<&ResizeReview> {
        self.resize_review.as_ref()
    }
    #[cfg(test)]
    pub fn resize_pending(&self) -> bool {
        self.resize_pending.is_some()
    }
    /// The dimensions the Host advertised for this Session, when it advertised any.
    pub fn dimension_bounds(&self) -> Option<(u64, u64)> {
        self.environment
            .as_ref()
            .map(|environment| (environment.max_cols, environment.max_rows))
    }
    #[cfg(test)]
    pub fn rename_pending(&self) -> bool {
        self.rename_pending.is_some()
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
        self.create_pending.is_some()
            || self.close_pending.is_some()
            || self.rename_pending.is_some()
            || self.resize_pending.is_some()
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
        self.rename_review = None;
        self.resize_review = None;
        self.notice = Notice::None;
        let following = self.follow.is_some();
        self.stop_follow();
        following
    }
    /// Fence late receipts after the transport generation is gone.
    pub fn disconnect(&mut self) {
        let resizing = self.resize_pending.take().is_some();
        let renaming = self.rename_pending.take().is_some();
        let admitted = self.create_pending.take().is_some() || self.close_pending.take().is_some();
        let writing = self.write_pending.take().is_some();
        self.dismiss();
        if admitted {
            self.notice = Notice::CreatingIndeterminate;
        } else if resizing {
            self.notice = Notice::ResizingIndeterminate;
        } else if renaming {
            self.notice = Notice::RenamingIndeterminate;
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
        // The Host's own size drives the screen for this terminal: rows bound the grid so a
        // full-screen program scrolls into scrollback, and columns wrap a long line.
        if let Some((cols, rows)) = self
            .list
            .iter()
            .find(|info| info.id == id)
            .map(|info| (info.cols, info.rows))
        {
            self.output.set_size(cols, rows);
        }
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
                // `preferred_shell` survives: the reference remembers the shell across opens, so a
                // user who chose one is not returned to the first discovered.
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
                self.preferred_shell = Some(self.shells[index].path.clone());
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
            Action::ReviewRename { ticket, id }
                if self.current(&ticket, context)
                    && !self.busy()
                    && self.write_pending.is_none()
                    && self.list.iter().any(|info| info.id == id) =>
            {
                // The review opens on the terminal's current title, so a rename that keeps the
                // name is a no-op the operator can see before confirming.
                let title = self
                    .list
                    .iter()
                    .find(|info| info.id == id)
                    .map(|info| info.title.clone())
                    .unwrap_or_default();
                self.rename_review = Some(RenameReview {
                    ticket: ticket.clone(),
                    id,
                    title,
                });
                self.create_review = None;
                self.close_review = None;
                self.notice = Notice::RenameReview;
            }
            Action::BackRename(ticket) if self.current(&ticket, context) => {
                self.rename_review = None;
                self.notice = Notice::None;
            }
            Action::EditRename { ticket, title }
                if self.current(&ticket, context)
                    && self
                        .rename_review
                        .as_ref()
                        .is_some_and(|review| review.ticket == ticket) =>
            {
                if let Some(review) = self.rename_review.as_mut() {
                    review.title = title;
                }
            }
            Action::ReviewResize { ticket, id }
                if self.current(&ticket, context)
                    && !self.busy()
                    && self.write_pending.is_none()
                    && self.attached()
                    && self.control() != Control::Elsewhere
                    && self.list.iter().any(|info| info.id == id) =>
            {
                // The review opens on the dimensions the panel last asked for, so a resize that
                // changes nothing is visible before Confirm.
                let (cols, rows) = self
                    .list
                    .iter()
                    .find(|info| info.id == id)
                    .map(|info| (info.cols, info.rows))
                    .unwrap_or((DEFAULT_COLS, DEFAULT_ROWS));
                self.resize_review = Some(ResizeReview {
                    ticket: ticket.clone(),
                    id,
                    cols,
                    rows,
                });
                self.create_review = None;
                self.close_review = None;
                self.rename_review = None;
                self.notice = Notice::ResizeReview;
            }
            Action::BackResize(ticket) if self.current(&ticket, context) => {
                self.resize_review = None;
                self.notice = Notice::None;
            }
            Action::EditResize { ticket, cols, rows }
                if self.current(&ticket, context)
                    && self
                        .resize_review
                        .as_ref()
                        .is_some_and(|review| review.ticket == ticket) =>
            {
                if let Some(review) = self.resize_review.as_mut() {
                    review.cols = cols;
                    review.rows = rows;
                }
            }
            Action::ConfirmResize(review)
                if self.current(&review.ticket, context)
                    && !self.busy()
                    && self.write_pending.is_none()
                    && self.resize_review.as_ref() == Some(&review) =>
            {
                let (Some(attachment), Some(info)) = (
                    self.attachment.clone(),
                    self.list.iter().find(|info| info.id == review.id),
                ) else {
                    self.notice = Notice::ResizeFailed;
                    return None;
                };
                // The Host bounds the dimensions it advertised; a request outside them would be
                // refused after the round trip, so it is refused here instead.
                let (max_cols, max_rows) = self
                    .environment
                    .as_ref()
                    .map(|environment| (environment.max_cols, environment.max_rows))
                    .unwrap_or((DEFAULT_COLS, DEFAULT_ROWS));
                if review.cols == 0
                    || review.rows == 0
                    || review.cols > max_cols
                    || review.rows > max_rows
                    || (review.cols == info.cols && review.rows == info.rows)
                {
                    self.notice = Notice::ResizeFailed;
                    return None;
                }
                self.resize_review = None;
                let receipt = ResizeReceipt {
                    ticket: review.ticket.clone(),
                    id: review.id.clone(),
                    attachment,
                    cols: review.cols,
                    rows: review.rows,
                };
                self.resize_pending = Some(receipt.clone());
                self.notice = Notice::Resizing;
                return Some(Effect::Resize(ResizeSubmission { receipt }));
            }
            Action::ConfirmRename(review)
                if self.current(&review.ticket, context)
                    && !self.busy()
                    && self.write_pending.is_none()
                    && self.rename_review.as_ref() == Some(&review) =>
            {
                let title = review.title.trim().to_owned();
                // The Host refuses an empty or over-long title; refusing here keeps the panel from
                // sending a request it already knows fails (`terminal-controller/src/index.ts`).
                if title.is_empty()
                    || title.chars().count() > 120
                    || title.chars().any(char::is_control)
                    || self
                        .list
                        .iter()
                        .any(|info| info.id == review.id && info.title == title)
                {
                    self.notice = Notice::RenameFailed;
                    return None;
                }
                self.rename_review = None;
                let receipt = RenameReceipt {
                    ticket: review.ticket,
                    id: review.id,
                    title: title.clone(),
                };
                self.rename_pending = Some(receipt.clone());
                self.notice = Notice::Renaming;
                return Some(Effect::Rename(RenameSubmission {
                    receipt,
                    data: title,
                }));
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
                self.shells_failed = loaded.shells_failed;
                // The remembered shell wins when the Host still offers it; otherwise the first one
                // stands in, which is the reference's own fallback (`model.ts:125-129`).
                let remembered = self
                    .preferred_shell
                    .as_ref()
                    .and_then(|path| self.shells.iter().position(|shell| &shell.path == path));
                if let Some(index) = remembered {
                    self.shell = Some(index);
                } else if self.shell.is_none_or(|index| index >= self.shells.len()) {
                    self.shell = (!self.shells.is_empty()).then_some(0);
                    self.preferred_shell = self
                        .shell
                        .and_then(|index| self.shells.get(index))
                        .map(|shell| shell.path.clone());
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
                    if self.selected.as_ref() == Some(&info.id) {
                        self.output.set_size(info.cols, info.rows);
                    }
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
    /// Retire the exact rename receipt; install the Host-confirmed title only for the reviewed
    /// terminal, and only when the ACK names the same identity.
    pub fn finish_rename(&mut self, receipt: &RenameReceipt, outcome: RenameOutcome) {
        if self.rename_pending.as_ref() != Some(receipt) {
            return;
        }
        self.rename_pending = None;
        let notice = match outcome {
            RenameOutcome::Renamed => {
                if let Some(info) = self
                    .list
                    .iter_mut()
                    .find(|info| info.id == receipt.id && info.title != receipt.title)
                {
                    info.title = receipt.title.clone();
                }
                Notice::Renamed
            }
            RenameOutcome::Unavailable => Notice::RenameFailed,
            RenameOutcome::NotSent => Notice::RenameNotSent,
            RenameOutcome::Unknown => Notice::RenamingIndeterminate,
        };
        if self
            .panel
            .as_ref()
            .is_some_and(|panel| panel.ticket == receipt.ticket)
        {
            self.notice = notice;
        } else if matches!(self.notice, Notice::Renaming) {
            self.notice = Notice::None;
        }
    }
    /// Retire the exact resize receipt; adopt the dimensions only from the Host's acknowledgment.
    pub fn finish_resize(&mut self, receipt: &ResizeReceipt, outcome: ResizeOutcome) {
        if self.resize_pending.as_ref() != Some(receipt) {
            return;
        }
        self.resize_pending = None;
        let notice = match outcome {
            ResizeOutcome::Resized => {
                if let Some(info) = self.list.iter_mut().find(|info| info.id == receipt.id) {
                    info.cols = receipt.cols;
                    info.rows = receipt.rows;
                }
                Notice::Resized
            }
            ResizeOutcome::Unavailable => Notice::ResizeFailed,
            ResizeOutcome::NotSent => Notice::ResizeNotSent,
            ResizeOutcome::Unknown => Notice::ResizingIndeterminate,
        };
        if self
            .panel
            .as_ref()
            .is_some_and(|panel| panel.ticket == receipt.ticket)
        {
            self.notice = notice;
        } else if matches!(self.notice, Notice::Resizing) {
            self.notice = Notice::None;
        }
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
                shells_failed: false,
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
                shells_failed: false,
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
                shells_failed: false,
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
                shells_failed: false,
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
                shells_failed: false,
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
        // Control sequences are stripped from the text copy; the screen applies them.
        let mut control = Output::default();
        control.push("\u{1b}[31mPUBLIC\u{1b}[0m\r\n");
        // The escape sequences are gone; the carriage return survives for the display to apply, so
        // the rendered text is one line and the retained bytes keep the redraw that made it so.
        assert_eq!(control.text(), "PUBLIC\r\n");
        assert_eq!(control.display(DISPLAY_BYTES), "PUBLIC\n");
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
    fn a_failed_shell_read_is_not_an_empty_discovery() {
        // An empty list with the failure flag: the Host could not be asked, which the panel
        // reports as a read failure rather than as "no profiles" (`TerminalGuide.tsx:53-64`).
        let mut terminal = opened();
        let read = terminal
            .apply(Action::Refresh(ticket(&terminal)), &context())
            .and_then(|effect| match effect {
                Effect::Read(read) => Some(read),
                _ => None,
            })
            .expect("refresh reads");
        terminal.loaded(
            &read,
            Ok(Loaded {
                list: vec![info("PUBLIC-terminal-1")],
                shells: vec![],
                shells_failed: true,
                environment: None,
            }),
        );
        assert!(terminal.shells().is_empty());
        assert!(
            terminal.shells_failed(),
            "the failure survives the empty list"
        );
        // A later read that answers with no profiles clears it.
        let read = terminal
            .apply(Action::Refresh(ticket(&terminal)), &context())
            .and_then(|effect| match effect {
                Effect::Read(read) => Some(read),
                _ => None,
            })
            .expect("refresh reads again");
        terminal.loaded(
            &read,
            Ok(Loaded {
                list: vec![info("PUBLIC-terminal-1")],
                shells: vec![],
                shells_failed: false,
                environment: None,
            }),
        );
        assert!(
            !terminal.shells_failed(),
            "an answered read clears the flag"
        );
    }

    #[test]
    fn one_paint_renders_only_the_newest_rows_it_can_show() {
        let mut output = Output::default();
        // A thousand lines: more than any screen shows, fewer than the scrollback bound.
        for index in 0..1_000 {
            output.push(&format!("line {index}\n"));
        }
        let rows = output.display_styled(DISPLAY_BYTES);
        assert!(
            rows.len() <= RENDERED_ROWS,
            "{} rows is more than one paint renders",
            rows.len()
        );
        // The newest line is last and the oldest are the ones dropped.
        let last: String = rows
            .last()
            .expect("rows")
            .iter()
            .map(|(text, ..)| text.as_str())
            .collect();
        assert_eq!(last, "line 999");
        assert!(!rows.iter().flatten().any(|(text, ..)| text == "line 0"));
    }

    #[test]
    fn a_background_colour_is_painted_and_reset() {
        let mut output = Output::default();
        output.push("plain \u{1b}[41mred paper\u{1b}[49m plain\n");
        let runs = output.display_styled(DISPLAY_BYTES);
        assert_eq!(
            runs[0],
            vec![
                ("plain ".to_owned(), None, false, None),
                ("red paper".to_owned(), None, false, Some(1)),
                (" plain".to_owned(), None, false, None),
            ]
        );
        // A bright background is the second eight, and `0` clears it with the rest.
        let mut output = Output::default();
        output.push("\u{1b}[104;4mblue\u{1b}[0m plain\n");
        let runs = output.display_styled(DISPLAY_BYTES);
        assert_eq!(
            runs[0],
            vec![
                ("blue".to_owned(), None, true, Some(12)),
                (" plain".to_owned(), None, false, None),
            ]
        );
    }

    #[test]
    fn underline_is_a_paintable_attribute_and_resets_with_the_rest() {
        let mut output = Output::default();
        output.push("plain \u{1b}[4munder\u{1b}[24m plain\n");
        let runs = output.display_styled(DISPLAY_BYTES);
        assert_eq!(
            runs[0],
            vec![
                ("plain ".to_owned(), None, false, None),
                ("under".to_owned(), None, true, None),
                (" plain".to_owned(), None, false, None),
            ]
        );
        // `0` clears it with the colour, and a double underline draws as one.
        let mut output = Output::default();
        output.push("\u{1b}[4;31mred\u{1b}[0m plain \u{1b}[21mtwo\n");
        let runs = output.display_styled(DISPLAY_BYTES);
        assert_eq!(
            runs[0],
            vec![
                ("red".to_owned(), Some(1), true, None),
                (" plain ".to_owned(), None, false, None),
                ("two".to_owned(), None, true, None),
            ]
        );
    }

    #[test]
    fn the_cursor_mode_saves_and_restores_like_escape_seven() {
        // `CSI ? 1048 h`/`l` is the private-mode spelling of `ESC 7`/`ESC 8`, used by programs that
        // bracket a redraw with one pair of sequences.
        let mut output = Output::default();
        output.set_size(0, 3);
        output.push("ab\u{1b}[?1048hcd\u{1b}[?1048lX");
        assert_eq!(output.display(DISPLAY_BYTES), "abXd");
    }

    #[test]
    fn erasing_the_scrollback_is_the_only_mode_that_clears_it() {
        // `CSI 3 J` clears the lines that scrolled off; `CSI 2 J` deliberately leaves them, as a
        // terminal's buffer does.
        let mut output = Output::default();
        output.set_rows(2);
        output.push("old\nnew\nthird\n\u{1b}[3J\u{1b}[2J");
        assert_eq!(output.display(DISPLAY_BYTES).trim_end(), "");
        // Without the `3 J` the scrolled line survives a `2 J`.
        let mut output = Output::default();
        output.set_rows(2);
        output.push("old\nnew\nthird\n\u{1b}[2J");
        assert_eq!(output.display(DISPLAY_BYTES).trim_end(), "old\nnew");
    }

    #[test]
    fn cancelling_bold_returns_the_colour_to_its_base_half() {
        // `1` brightens the current colour and `22` cancels the attribute, so the ink goes back to the
        // base eight instead of staying bright.
        let mut output = Output::default();
        output.push("\u{1b}[32;1mbold\u{1b}[22mbase\n");
        let runs = output.display_styled(DISPLAY_BYTES);
        assert_eq!(
            runs[0],
            vec![
                ("bold".to_owned(), Some(10), false, None),
                ("base".to_owned(), Some(2), false, None),
            ]
        );
    }

    #[test]
    fn saving_and_restoring_the_cursor_returns_to_the_same_cell() {
        // `ESC 7` / `ESC 8` are how a program redraws around a position it wants back; the attributes
        // travel with the cursor.
        let mut output = Output::default();
        output.set_size(0, 3);
        output.push("ab\u{1b}7cd\u{1b}8X");
        // The cursor returns to the saved cell and the glyph overwrites it; the text it wrote after
        // saving stays where it was written.
        assert_eq!(output.display(DISPLAY_BYTES), "abXd");
        // An `ESC 8` with nothing saved is a no-op rather than a jump to the origin.
        let mut output = Output::default();
        output.set_size(0, 3);
        output.push("ab\u{1b}8X");
        assert_eq!(output.display(DISPLAY_BYTES), "abX");
    }

    #[test]
    fn a_scroll_up_moves_the_region_toward_the_top() {
        // `CSI S` scrolls the region's contents up by one line and joins a blank at the bottom; the
        // cursor stays where it was, unlike a newline at the region's bottom.
        let mut output = Output::default();
        output.set_size(0, 3);
        output.push("one\ntwo\nthree\u{1b}[S");
        // The cursor stays on the (now blank) last row, so the render carries its trailing newline.
        assert_eq!(output.display(DISPLAY_BYTES), "two\nthree\n");
        // `CSI T` scrolls the other way, joining a blank at the top.
        let mut output = Output::default();
        output.set_size(0, 3);
        output.push("one\ntwo\nthree\u{1b}[T");
        assert_eq!(output.display(DISPLAY_BYTES), "\none\ntwo");
    }

    #[test]
    fn erasing_characters_blanks_them_without_shifting_the_row() {
        // `CSI 2 X` blanks two cells at the cursor and leaves the rest of the row where it is, which
        // is how a TUI clears a field without moving what follows it.
        let mut output = Output::default();
        output.set_size(0, 2);
        output.push("abcdef\u{1b}[1;2H\u{1b}[2X");
        assert_eq!(output.display(DISPLAY_BYTES), "a  def");
    }

    #[test]
    fn clearing_the_display_leaves_the_cursor_where_it_was() {
        // `CSI 2 J` empties the screen but does not home the cursor; text next lands at the same
        // column, which is what a program that clears and then redraws in place expects.
        let mut output = Output::default();
        output.set_size(0, 3);
        output.push("abc\u{1b}[2JX");
        assert_eq!(output.display(DISPLAY_BYTES), "   X");
    }

    #[test]
    fn erasing_to_the_cursor_keeps_the_text_after_it() {
        // `CSI 1 J` blanks from the top of the screen to the cursor; the row's tail is untouched.
        let mut output = Output::default();
        output.set_size(0, 3);
        output.push("first\nabcdef\u{1b}[2;3H\u{1b}[1J");
        // Erase-to-cursor includes the cursor's own cell, so `c` is blanked: three spaces, then `def`.
        assert_eq!(output.display(DISPLAY_BYTES), "\n   def");
    }

    #[test]
    fn adversarial_escape_input_terminates_and_renders() {
        // The screen is fed bytes from the Host, so every sequence is untrusted. Each case runs in its
        // own thread with a bounded join: a non-terminating input fails this test instead of hanging
        // the suite — which is exactly how round 588's tab loop was found.
        let cases: [&str; 13] = [
            "\u{1b}[",
            "\u{1b}",
            "\u{1b}[?",
            "\u{1b}[0m\u{1b}[0;0HX",
            "\u{1b}[999999999H\u{1b}[999999999;999999999r",
            "\u{1b}[3;1r\u{1b}[L\u{1b}[M\u{1b}[P\u{1b}[@",
            "\u{1b}[?1049h\u{1b}[?1049l\u{1b}[?1049l",
            "\u{1b}]0;title\u{7}text",
            "\u{1b}[38;5;200mcolour\u{1b}[0m",
            "\r\u{8}\t\u{7f}ok",
            "\u{1b}[1;999999999HX\u{1b}[999999999GX\u{1b}[999999999CX",
            "\u{1b}[999999999L\u{1b}[999999999M",
            "\u{1b}[999999999P\u{1b}[999999999@",
        ];
        for (index, case) in cases.iter().enumerate() {
            let (sender, receiver) = std::sync::mpsc::channel();
            let owned = (*case).to_owned();
            std::thread::spawn(move || {
                let mut output = Output::default();
                output.set_size(5, 3);
                output.push(&owned);
                let _ = output.display_styled(DISPLAY_BYTES);
                let _ = sender.send(());
            });
            assert!(
                receiver
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .is_ok(),
                "case {index} did not terminate: {case:?}"
            );
        }
    }

    #[test]
    fn a_tab_past_the_last_column_does_not_spin() {
        // A tab advances to the next eight-column stop. On a five-column screen that stop is past the
        // margin, and with autowrap on the cursor wrapped to column 0 on every space, so the stop was
        // never reached: an infinite loop on `\t`, a byte a shell prints. The stop is clamped to the
        // last column. Found by the round-588 adversarial test, which hung.
        let mut output = Output::default();
        output.set_size(5, 3);
        output.push("\rab\tX");
        assert_eq!(output.display(DISPLAY_BYTES), "ab  X");
        // The same bytes at a wide screen still reach the ordinary eight-column stop.
        let mut output = Output::default();
        output.set_size(80, 3);
        output.push("ab\tX");
        assert_eq!(output.display(DISPLAY_BYTES), "ab      X");
    }

    #[test]
    fn a_combining_mark_takes_no_column_of_its_own() {
        // `e` plus U+0301: the grid keeps one character per cell, so the mark is dropped rather than
        // drawn beside its base as a stray box, and the column does not advance.
        let mut output = Output::default();
        output.set_size(6, 2);
        output.push("e\u{301}X");
        assert_eq!(output.display(DISPLAY_BYTES), "eX");
        // A column address after a marked glyph still lands one column past the base.
        let mut output = Output::default();
        output.set_size(6, 2);
        output.push("e\u{301}\u{1b}[1;3HY");
        assert_eq!(output.display(DISPLAY_BYTES), "e Y");
    }

    #[test]
    fn autowrap_off_overwrites_the_last_column_instead_of_wrapping() {
        let mut output = Output::default();
        output.set_size(5, 3);
        output.push("abcdefg");
        assert_eq!(output.display(DISPLAY_BYTES), "abcde\nfg");
        // With autowrap off the sixth character and everything after it stay in the last column.
        let mut output = Output::default();
        output.set_size(5, 3);
        output.push("\u{1b}[?7labcdefg");
        assert_eq!(output.display(DISPLAY_BYTES), "abcdg");
        // `?7h` turns it back on.
        let mut output = Output::default();
        output.set_size(5, 3);
        output.push("\u{1b}[?7l\u{1b}[?7habcdefg");
        assert_eq!(output.display(DISPLAY_BYTES), "abcde\nfg");
    }

    #[test]
    fn a_scroll_region_scrolls_only_its_own_rows() {
        let mut output = Output::default();
        output.set_size(0, 5);
        // A status row at the top and one at the bottom, with three body rows between them.
        output.push("STATUS\n\u{1b}[2;4rbody one\nbody two\nbody three\nbody four\nFOOT\n");
        let text = output.display(DISPLAY_BYTES);
        assert!(text.starts_with("STATUS\n"), "{text}");
        assert!(text.contains("body two\nbody three\nbody four"), "{text}");
        // A newline at the region's bottom scrolls the body only: the top status row and the row
        // below the region do not move.
        assert!(text.contains("FOOT"), "{text}");
        // Setting a region homes the cursor to its first row, not the screen's, so the next text
        // lands below the status row above it.
        let mut output = Output::default();
        output.set_size(0, 4);
        output.push("top\u{1b}[2;3rX");
        assert_eq!(output.display(DISPLAY_BYTES), "top\nX");
        // `CSI r` with no parameters restores the whole screen.
        let mut output = Output::default();
        output.set_size(0, 3);
        output.push("a\nb\u{1b}[1;2r\u{1b}[r\u{1b}[3;1Hc");
        assert_eq!(output.display(DISPLAY_BYTES), "a\nb\nc");
    }

    #[test]
    fn inserting_a_line_before_a_size_arrives_keeps_every_row() {
        // The grid is unbounded until the Host states its height, so an insert must not truncate it to
        // one row: `truncate(rows.max(1))` did exactly that before this test.
        let mut output = Output::default();
        output.push("one\ntwo\nthree\u{1b}[2;1H\u{1b}[LX");
        assert_eq!(output.display(DISPLAY_BYTES), "one\nX\ntwo\nthree");
    }

    #[test]
    fn lines_and_characters_are_inserted_and_deleted_at_the_cursor() {
        // Insert two rows at the cursor: the rows below move down and the bottom falls away.
        let mut output = Output::default();
        output.set_size(0, 4);
        output.push("one\ntwo\u{1b}[2;1H\u{1b}[2L");
        assert_eq!(output.display(DISPLAY_BYTES), "one\n\n\ntwo");
        // Delete one row: the rows below move up, so the deleted line is gone.
        let mut output = Output::default();
        output.set_size(0, 4);
        output.push("one\ntwo\nthree\u{1b}[2;1H\u{1b}[1M");
        assert_eq!(output.display(DISPLAY_BYTES), "one\nthree");
        // Delete three characters at column 1 of `abcXYZ`.
        let mut output = Output::default();
        output.set_size(0, 2);
        output.push("abcXYZ\u{1b}[1;4H\u{1b}[3P");
        assert_eq!(output.display(DISPLAY_BYTES), "abc");
        // Insert two blanks at column 1, pushing the rest right.
        let mut output = Output::default();
        output.set_size(0, 2);
        output.push("abc\u{1b}[1;2H\u{1b}[2@");
        assert_eq!(output.display(DISPLAY_BYTES), "a  bc");
    }

    #[test]
    fn a_line_that_reaches_the_last_column_wraps_to_the_next_row() {
        let mut output = Output::default();
        output.set_size(10, 4);
        output.push("abcdefghijkl\n");
        // Twelve columns into a ten-column screen: two rows, and the trailing newline ends a third.
        assert_eq!(output.display(DISPLAY_BYTES), "abcdefghij\nkl\n");
        // Wrapping counts columns, not characters: a wide glyph that would straddle the edge wraps
        // rather than losing its second half.
        let mut output = Output::default();
        output.set_size(3, 4);
        output.push("ab\u{4f60}");
        assert_eq!(output.display(DISPLAY_BYTES), "ab\n\u{4f60}");
        // A space at the last column wraps like any other glyph, so the shell's own padding does
        // not hide the first character of the next row.
        let mut output = Output::default();
        output.set_size(4, 4);
        output.push("abcd e");
        assert_eq!(output.display(DISPLAY_BYTES), "abcd\n e");
    }

    #[test]
    fn the_alternate_screen_restores_the_shell_after_a_full_screen_program() {
        let mut output = Output::default();
        output.set_rows(4);
        output.push("PUBLIC $ vim notes.txt\n");
        assert_eq!(output.display(DISPLAY_BYTES), "PUBLIC $ vim notes.txt\n");
        // The program switches to its own screen and draws there.
        output.push("\u{1b}[?1049h~\n~\n");
        assert_eq!(output.display(DISPLAY_BYTES), "~\n~\n");
        // Leaving restores the shell exactly, cursor and all, so the next prompt continues below.
        output.push("\u{1b}[?1049lPUBLIC $ ");
        assert_eq!(
            output.display(DISPLAY_BYTES),
            "PUBLIC $ vim notes.txt\nPUBLIC $ "
        );
        // A second enter/leave pair is idempotent: an unmatched enter saves nothing new.
        output.push("\u{1b}[?1049h\u{1b}[?1049h\u{1b}[?1049l\u{1b}[?1049l");
        assert_eq!(
            output.display(DISPLAY_BYTES),
            "PUBLIC $ vim notes.txt\nPUBLIC $ "
        );
    }

    #[test]
    fn a_wide_character_takes_two_columns_and_keeps_a_layout_aligned() {
        // A CJK prompt is two columns per glyph, so text addressed to column 8 lands where the
        // glyphs end, not one column early.
        let mut output = Output::default();
        output.set_rows(4);
        // Three columns of glyphs plus `: ` put column 7 (index 6) right after the space; a
        // single-column model would have written there one column early.
        output.push("\u{4f60}\u{597d}: \u{1b}[1;7Hok\n");
        assert_eq!(output.display(DISPLAY_BYTES), "\u{4f60}\u{597d}: ok\n");
        // The second column of a wide glyph draws nothing of its own.
        let mut output = Output::default();
        output.push("a\u{4f60}b");
        let runs = output.display_styled(DISPLAY_BYTES);
        let text: String = runs[0].iter().map(|(text, ..)| text.as_str()).collect();
        assert_eq!(text, "a\u{4f60}b");
        // A wide glyph at the end of a written line still occupies its two columns.
        let mut output = Output::default();
        output.push("x\u{4f60}");
        assert_eq!(output.display(DISPLAY_BYTES), "x\u{4f60}");
    }

    #[test]
    fn sgr_colours_a_run_of_cells_and_the_bright_half_is_the_second_eight() {
        let mut output = Output::default();
        output.push("plain \u{1b}[32mPASS\u{1b}[0m plain\n");
        let runs = output.display_styled(DISPLAY_BYTES);
        assert_eq!(runs.len(), 1);
        assert_eq!(
            runs[0],
            vec![
                ("plain ".to_owned(), None, false, None),
                ("PASS".to_owned(), Some(2), false, None),
                (" plain".to_owned(), None, false, None),
            ]
        );
        // A bright colour is the second eight: `90`+ is index 8+, and bold brightens a base colour.
        let mut output = Output::default();
        output.push("\u{1b}[91mbright\u{1b}[32m\u{1b}[1mbold\u{1b}[0m\n");
        let runs = output.display_styled(DISPLAY_BYTES);
        assert_eq!(
            runs[0],
            vec![
                ("bright".to_owned(), Some(9), false, None),
                ("bold".to_owned(), Some(10), false, None),
            ]
        );
        // An unwritten tail keeps the default ink, and a reset returns to it.
        let mut output = Output::default();
        output.push("\u{1b}[31mred\u{1b}[39m back\n");
        let runs = output.display_styled(DISPLAY_BYTES);
        assert_eq!(
            runs[0],
            vec![
                ("red".to_owned(), Some(1), false, None),
                (" back".to_owned(), None, false, None),
            ]
        );
    }

    #[test]
    fn the_cursor_addresses_the_grid_and_a_full_screen_scrolls_into_history() {
        // Four output lines into a three-row grid: the oldest scrolls into scrollback, and the
        // rendered text still shows everything, in order.
        let mut output = Output::default();
        output.set_rows(3);
        output.push("PUBLIC $ printf 'a\\nb\\nc\\nd\\n'\n");
        output.push("a\nb\nc\nd\n");
        let text = output.display(DISPLAY_BYTES);
        assert!(text.starts_with("PUBLIC $ printf"), "{text}");
        assert!(text.contains("a\nb\nc\nd"), "{text}");
        assert!(text.ends_with("d\n"), "{text}");
        // Home then write: the cursor addresses the top of the grid, not the end of the output.
        let mut output = Output::default();
        output.set_rows(4);
        output.push("one\ntwo\nthree\n");
        output.push("\u{1b}[HPUBLIC");
        assert_eq!(output.display(DISPLAY_BYTES), "PUBLIC\ntwo\nthree");
        // Up and right move within the grid; the write lands on that column of that row.
        let mut output = Output::default();
        output.set_rows(4);
        output.push("abc\ndef\n");
        output.push("\u{1b}[1A\u{1b}[2GR");
        assert_eq!(output.display(DISPLAY_BYTES), "abc\ndRf");
        // `2 J` clears the grid but keeps the scrollback, as a terminal's buffer does.
        let mut output = Output::default();
        output.set_rows(2);
        output.push("old\nnew\nthird\n");
        assert!(output.display(DISPLAY_BYTES).contains("old"));
        output.push("\u{1b}[2J");
        // Both scrolled lines survive and `third` — written into the grid — is gone.
        assert_eq!(output.display(DISPLAY_BYTES).trim_end(), "old\nnew");
    }

    #[test]
    fn the_screen_applies_the_erase_sequences_a_shell_uses() {
        // `clear` is `ESC [ 2 J` (plus a home): the display empties instead of keeping the history.
        let mut output = Output::default();
        output.push("one\ntwo\nthree\n");
        // `clear` sends home as well: `CSI 2 J` empties the display and leaves the cursor, so the
        // prompt only returns to the top-left with `CSI H` (round 599).
        output.push("\u{1b}[2J\u{1b}[HPUBLIC $ ");
        assert_eq!(output.display(DISPLAY_BYTES), "PUBLIC $ ");
        // `ESC [ K` erases from the cursor to the end of its line, leaving the prefix.
        let mut output = Output::default();
        output.push("PUBLIC progress 45%\u{1b}[K done");
        assert_eq!(output.display(DISPLAY_BYTES), "PUBLIC progress 45% done");
        // `ESC [ 2 K` clears the whole line and leaves the cursor where it was, so the next
        // character lands at that column rather than at the start.
        let mut output = Output::default();
        output.push("PUBLIC junk\u{1b}[2KA");
        assert_eq!(output.display(DISPLAY_BYTES), "           A");
        // A backspace steps the cursor back without erasing, so the next character overwrites it.
        let mut output = Output::default();
        output.push("abc\u{8}X");
        assert_eq!(output.display(DISPLAY_BYTES), "abX");
        // An unsupported sequence is consumed rather than drawn.
        let mut output = Output::default();
        output.push("\u{1b}[1;31mPUBLIC\u{1b}[0m");
        assert_eq!(output.display(DISPLAY_BYTES), "PUBLIC");
    }

    #[test]
    fn a_carriage_return_redraws_its_line_in_place() {
        // A progress bar redraws with `\r`: one line, the newest text.
        let mut output = Output::default();
        output.push("Downloading 10%\rDownloading 100%\n");
        assert_eq!(output.display(DISPLAY_BYTES), "Downloading 100%\n");
        // Only the columns the new text reaches change; the rest of the line survives, as it
        // does in a terminal.
        let mut output = Output::default();
        output.push("abcdef\rXY");
        assert_eq!(output.display(DISPLAY_BYTES), "XYcdef");
        // A carriage return at column 0 changes nothing.
        let mut output = Output::default();
        output.push("\rabc");
        assert_eq!(output.display(DISPLAY_BYTES), "abc");
        // A newline after a redraw still ends the line.
        let mut output = Output::default();
        output.push("one\rtwo\nthree");
        assert_eq!(output.display(DISPLAY_BYTES), "two\nthree");
        // The retained bytes are unchanged: the transform is display-only.
        let mut output = Output::default();
        output.push("a\rb");
        assert_eq!(output.tail(DISPLAY_BYTES), "a\rb");
    }

    #[test]
    fn the_limits_sentence_is_generated_from_the_actions_it_names() {
        let copy = limits_unavailable_copy();
        assert!(copy.starts_with("Host limits unavailable"), "{copy}");
        for action in MUTATING_ACTIONS {
            assert!(copy.contains(action), "{action} missing from {copy}");
        }
        // Commas between, `and` before the last: `create, write, rename, resize and close`.
        assert!(copy.contains("create, write, rename, resize and close cannot run"));
        assert!(!copy.contains(", and"), "no Oxford comma: {copy}");
        assert_eq!(MUTATING_ACTIONS.len(), 5, "one entry per mutating action");
    }

    #[test]
    fn a_rename_is_reviewed_edited_admitted_and_installed_only_from_the_ack() {
        let mut terminal = opened();
        let read = terminal
            .apply(Action::Refresh(ticket(&terminal)), &context())
            .and_then(|effect| match effect {
                Effect::Read(read) => Some(read),
                _ => None,
            })
            .expect("refresh reads");
        terminal.loaded(
            &read,
            Ok(Loaded {
                list: vec![info("PUBLIC-terminal-1")],
                shells: vec![shell()],
                shells_failed: false,
                environment: Some(environment()),
            }),
        );
        let t = ticket(&terminal);
        let id = WebTerminalId::new("PUBLIC-terminal-1").unwrap();
        // The review opens on the current title.
        assert!(
            terminal
                .apply(
                    Action::ReviewRename {
                        ticket: t.clone(),
                        id: id.clone()
                    },
                    &context()
                )
                .is_none()
        );
        let review = terminal
            .review_rename()
            .cloned()
            .expect("the review opened");
        assert_eq!(review.title, info("PUBLIC-terminal-1").title);
        // Editing the title is local until Confirm.
        terminal.apply(
            Action::EditRename {
                ticket: t.clone(),
                title: "PUBLIC renamed".into(),
            },
            &context(),
        );
        assert_eq!(
            terminal.review_rename().map(|review| review.title.as_str()),
            Some("PUBLIC renamed")
        );
        let review = terminal.review_rename().cloned().expect("still open");
        let Some(Effect::Rename(submission)) =
            terminal.apply(Action::ConfirmRename(review), &context())
        else {
            panic!("confirming a rename is one Host write");
        };
        assert_eq!(submission.title(), "PUBLIC renamed");
        assert!(terminal.rename_pending());
        // The list changes only from the ACK, which must carry the reviewed title.
        terminal.finish_rename(&submission.receipt, RenameOutcome::Renamed);
        assert_eq!(
            terminal
                .list()
                .iter()
                .find(|info| info.id == id)
                .map(|info| info.title.as_str()),
            Some("PUBLIC renamed")
        );
        assert_eq!(terminal.notice(), &Notice::Renamed);
        assert!(!terminal.rename_pending());
    }

    #[test]
    fn a_rename_that_changes_nothing_or_breaks_the_title_rule_is_refused() {
        let mut terminal = opened();
        let read = terminal
            .apply(Action::Refresh(ticket(&terminal)), &context())
            .and_then(|effect| match effect {
                Effect::Read(read) => Some(read),
                _ => None,
            })
            .expect("refresh reads");
        terminal.loaded(
            &read,
            Ok(Loaded {
                list: vec![info("PUBLIC-terminal-1")],
                shells: vec![shell()],
                shells_failed: false,
                environment: Some(environment()),
            }),
        );
        let t = ticket(&terminal);
        let id = WebTerminalId::new("PUBLIC-terminal-1").unwrap();
        // Opening the review and confirming the unchanged title is refused.
        terminal.apply(
            Action::ReviewRename {
                ticket: t.clone(),
                id: id.clone(),
            },
            &context(),
        );
        let unchanged = terminal.review_rename().cloned().expect("open");
        assert!(
            terminal
                .apply(Action::ConfirmRename(unchanged), &context())
                .is_none()
        );
        assert_eq!(terminal.notice(), &Notice::RenameFailed);
        // An empty or over-long title is refused without a Host round trip: the review is edited
        // first, because Confirm admits only the review that is actually open.
        for title in ["", "   ", &"x".repeat(121)] {
            terminal.apply(
                Action::ReviewRename {
                    ticket: t.clone(),
                    id: id.clone(),
                },
                &context(),
            );
            terminal.apply(
                Action::EditRename {
                    ticket: t.clone(),
                    title: (*title).to_owned(),
                },
                &context(),
            );
            let review = terminal.review_rename().cloned().expect("open");
            assert!(
                terminal
                    .apply(Action::ConfirmRename(review), &context())
                    .is_none()
            );
            assert_eq!(terminal.notice(), &Notice::RenameFailed, "{title:?}");
        }
    }

    #[test]
    fn a_resize_is_reviewed_bounded_and_installed_only_from_the_ack() {
        let mut terminal = opened();
        let attach = select(&mut terminal);
        terminal.frame(
            &attach,
            TerminalFrame::State {
                info: info("PUBLIC-terminal-1"),
            },
        );
        let id = WebTerminalId::new("PUBLIC-terminal-1").unwrap();
        let t = ticket(&terminal);
        terminal.apply(
            Action::ReviewResize {
                ticket: t.clone(),
                id: id.clone(),
            },
            &context(),
        );
        let review = terminal
            .review_resize()
            .cloned()
            .expect("attached, so it reviews");
        assert_eq!(
            (review.cols, review.rows),
            (80, 24),
            "the review opens on the reported size"
        );
        // The dimensions are edited locally until Confirm.
        terminal.apply(
            Action::EditResize {
                ticket: t.clone(),
                cols: 120,
                rows: 40,
            },
            &context(),
        );
        let review = terminal.review_resize().cloned().expect("still open");
        let Some(Effect::Resize(submission)) =
            terminal.apply(Action::ConfirmResize(review), &context())
        else {
            panic!("confirming a resize is one Host PTY update");
        };
        assert_eq!(
            (submission.receipt.cols, submission.receipt.rows),
            (120, 40)
        );
        assert!(terminal.resize_pending());
        // The reported size changes only from the ACK.
        terminal.finish_resize(&submission.receipt, ResizeOutcome::Resized);
        assert_eq!(
            terminal
                .list()
                .iter()
                .find(|info| info.id == id)
                .map(|info| (info.cols, info.rows)),
            Some((120, 40))
        );
        assert_eq!(terminal.notice(), &Notice::Resized);
        assert!(!terminal.resize_pending());
    }

    #[test]
    fn a_resize_of_zero_an_unchanged_size_or_beyond_the_limits_is_refused() {
        let mut terminal = opened();
        let attach = select(&mut terminal);
        terminal.frame(
            &attach,
            TerminalFrame::State {
                info: info("PUBLIC-terminal-1"),
            },
        );
        let id = WebTerminalId::new("PUBLIC-terminal-1").unwrap();
        let t = ticket(&terminal);
        for (cols, rows) in [(0, 24), (80, 0), (80, 24), (501, 24), (80, 1001)] {
            terminal.apply(
                Action::ReviewResize {
                    ticket: t.clone(),
                    id: id.clone(),
                },
                &context(),
            );
            terminal.apply(
                Action::EditResize {
                    ticket: t.clone(),
                    cols,
                    rows,
                },
                &context(),
            );
            let review = terminal.review_resize().cloned().expect("open");
            assert!(
                terminal
                    .apply(Action::ConfirmResize(review), &context())
                    .is_none(),
                "{cols}x{rows} was admitted"
            );
            assert_eq!(terminal.notice(), &Notice::ResizeFailed, "{cols}x{rows}");
            assert!(!terminal.resize_pending());
        }
    }

    #[test]
    fn the_chosen_shell_is_remembered_across_opens() {
        // Two shells, so a remembered choice cannot be confused with the first one.
        let second = TerminalShell {
            path: "/bin/PUBLIC-other".into(),
            args: Vec::new(),
            name: "PUBLIC other shell".into(),
        };
        let mut terminal = opened();
        let read = terminal
            .apply(Action::Refresh(ticket(&terminal)), &context())
            .and_then(|effect| match effect {
                Effect::Read(read) => Some(read),
                _ => None,
            })
            .expect("refresh reads");
        let first = shell();
        terminal.loaded(
            &read,
            Ok(Loaded {
                list: vec![info("PUBLIC-terminal-1")],
                shells: vec![first.clone(), second.clone()],
                shells_failed: false,
                environment: Some(environment()),
            }),
        );
        let t = ticket(&terminal);
        terminal.apply(
            Action::ChooseShell {
                ticket: t,
                index: 1,
            },
            &context(),
        );
        assert_eq!(terminal.shell, Some(1));
        // Opening again clears the list; the remembered path puts the same shell back when the
        // Host offers it, rather than falling to the first.
        let mut reopened = terminal;
        let effect = reopened
            .apply(
                Action::Open {
                    view: context().view.unwrap(),
                },
                &context(),
            )
            .expect("reopening reads");
        let Effect::Read(read) = effect else {
            panic!("reopening must not create a shell");
        };
        reopened.loaded(
            &read,
            Ok(Loaded {
                list: vec![info("PUBLIC-terminal-1")],
                // The Host now lists them in the other order: the remembered path still wins.
                shells: vec![second, first],
                shells_failed: false,
                environment: Some(environment()),
            }),
        );
        assert_eq!(reopened.shell, Some(0), "the remembered shell, now first");
        assert_eq!(
            reopened.preferred_shell.as_deref(),
            Some("/bin/PUBLIC-other"),
            "the choice is the path, not the index"
        );
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
