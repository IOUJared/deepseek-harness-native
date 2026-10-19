//! Alpha wire fold. Durable cursor and transient dense indices are independent.
use dsh_native_transport::dto::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[path = "activity.rs"]
pub mod activity;
/// The reference's ask-user transcript card.
#[path = "ask_card.rs"]
pub mod ask_card;
#[path = "assistant_parts.rs"]
mod assistant_parts;
/// The reference's inline diff card for a file-mutation tool row. Declared here, beside
/// `activity`, because every fixture example that embeds `ui.rs` or `activity.rs` also embeds
/// this module root; a crate-root declaration would have to be repeated in each of them.
#[path = "inline_diff.rs"]
pub mod inline_diff;
/// The reference's line-numbered `read` card. Declared beside `inline_diff`, for the same reason:
/// every example that embeds this module root then sees it.
#[path = "read_card.rs"]
pub mod read_card;
/// The reference's search card for a `grep`/`glob` call.
#[path = "search_card.rs"]
pub mod search_card;
/// The reference's terminal card for a `bash`/`pwsh` call, declared beside the other cards for the
/// same reason.
#[path = "terminal_card.rs"]
pub mod terminal_card;
/// The reference's web card for a `web_search`/`web_fetch` call.
#[path = "web_card.rs"]
pub mod web_card;
pub use assistant_parts::AssistantParts;
use assistant_parts::{Block, Kind};
#[path = "chat_presentation.rs"]
mod chat_presentation;
#[cfg(test)]
#[path = "chat_visibility_tests.rs"]
mod chat_visibility_tests;
#[path = "transcript_presentation.rs"]
pub mod presentation;

const MAX_RECORDS: usize = 4096;
const MAX_RETAINED_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_DISPLAY_BYTES: usize = 32 * 1024;
/// One file attachment as the transcript draws it: the reference stacks a
/// `.fileCard` above the message bubble carrying the file name and an
/// `EXT SIZE` meta line (`MessageItem.module.css:320-364`,
/// `MessageItem.tsx:208-217`), so the two strings are bounded here and the view
/// never re-formats an attachment.
#[derive(Clone)]
pub struct FileCard {
    pub name: String,
    pub meta: String,
}
/// The referenced-session summary line's copy: the reference's
/// `Referenced session · {labels}` with `message.referenceSeparator` between labels
/// (`ui-chat/src/client/locale.ts:317-318`). One function so the measurement and the
/// view can never disagree.
pub fn reference_summary_text(labels: &[String]) -> String {
    format!("Referenced session · {}", labels.join(", "))
}
/// One member of a durable workflow run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowMember {
    pub seq: u64,
    pub label: String,
    pub phase: Option<String>,
    pub child_id: Option<String>,
    pub outcome: Option<String>,
}
/// One durable workflow run folded from `tool-workflow/*` events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRun {
    pub start_seq: u64,
    pub name: String,
    pub stop: Option<String>,
    pub members: Vec<WorkflowMember>,
}
#[derive(Clone)]
pub struct DisplayRow {
    pub key: u64,
    pub role: String,
    /// The retained event's own type, so Chat classification stays independent
    /// of any display label a branch above may choose. Read by
    /// [`presentation::chat_visible`], which the production filter reaches
    /// through the stored decision below; the field itself is read by the
    /// fixture and test builds that construct rows directly.
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub kind: String,
    /// Whether Chat presents this row. Decided once, where the event is folded.
    pub chat: bool,
    pub text: String,
    /// File attachments carried by this message, in content order. Kept out of
    /// [`DisplayRow::text`] so the view can draw the reference's attachment row
    /// as its own block above the bubble instead of one joined string.
    pub files: Vec<FileCard>,
    /// Labels cited by the `session-reference` context record that immediately follows
    /// this message: the reference draws them under the bubble as
    /// `Referenced session · labels` (`ui-chat/src/client/chat/MessageItem.tsx:225-229`,
    /// labels from `conversation-nodes/event-projection.ts:83-87`).
    pub references: Vec<String>,
    /// The paired `tool/result`'s recorded body, folded onto its call row. The reference draws
    /// one row per call with the result inside it (`ToolCallTree.tsx`); native keeps the result
    /// record as a Trajectory-only row and folds these two facts onto the call.
    pub output: Option<String>,
    /// The paired result's exact recorded failure marker.
    pub failed: bool,
    /// The turn this assistant record belongs to, when the event carries one.
    pub turn: Option<u64>,
    /// The derived assistant message identity, which feedback targets.
    pub message_id: Option<String>,
    /// A non-human source that began this turn, drawn as the reference's trigger
    /// notice instead of a user bubble; `None` for human history.
    pub trigger: Option<presentation::Trigger>,
    pub time: i64,
    pub surface: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReduceError {
    SequenceGap,
    InvalidSnapshot,
    StreamGap,
    UnknownRequiredEvent,
    RetentionLimit,
    InvalidSurface,
}
impl std::fmt::Display for ReduceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}; reload session before sending")
    }
}
struct Attempt {
    id: String,
    turn: u64,
    step: u64,
    next: u64,
    blocks: BTreeMap<u64, Block>,
    pending: BTreeSet<u64>,
}
pub struct Transcript {
    generation: u64,
    cursor: i64,
    opening: i64,
    revision: u64,
    before: Option<u64>,
    more: bool,
    records: BTreeMap<u64, SessionWireEvent>,
    /// The snapshot's plugin projections, owned beside the record window they
    /// describe: a projection is relative to the log prefix that produced it, so
    /// keeping the two together is what stops a stale projection describing a
    /// newer window.
    projections: BTreeMap<String, serde_json::Value>,
    /// The log seq each retained projection key is consistent with. A push frame
    /// wins only above the seq already held, so a replayed or stale frame cannot
    /// regress a value (`client/sessions/projection-store.ts:86-95`: sequenced rows
    /// merge under higher-seq-wins).
    projection_seqs: BTreeMap<String, i64>,
    retained_bytes: usize,
    rows: Vec<DisplayRow>,
    /// Call seq → its paired result seq, filled where the fold happens. `activity` reads it so a
    /// call's labels can reach its own result without scanning the tail on every frame.
    paired: BTreeMap<u64, u64>,
    attempt: Option<Attempt>,
    retained: Option<(String, u64, u64)>,
    error: Option<ReduceError>,
    gap: bool,
    /// Read-only projection of retained goal/todo/subagent/schedule records.
    work: crate::work_panel::Store,
}
impl Transcript {
    pub fn new(generation: u64) -> Self {
        Self {
            generation,
            cursor: -1,
            opening: -1,
            revision: 0,
            before: None,
            more: false,
            records: BTreeMap::new(),
            projections: BTreeMap::new(),
            projection_seqs: BTreeMap::new(),
            retained_bytes: 0,
            rows: vec![],
            paired: BTreeMap::new(),
            attempt: None,
            retained: None,
            error: None,
            gap: false,
            work: crate::work_panel::Store::default(),
        }
    }
    pub fn rows(&self) -> &[DisplayRow] {
        &self.rows
    }
    /// The projections that describe this retained window.
    /// One assistant record's compact turn usage, if its usage carries a positive count.
    pub fn turn_usage(&self, key: u64) -> Option<String> {
        crate::ui::chat_usage::turn_usage(self.records.get(&key)?)
    }
    /// One assistant record's accounting rows, when its usage carries any bucket.
    pub fn turn_usage_detail(&self, key: u64) -> Option<crate::ui::chat_usage::TurnUsageDetail> {
        crate::ui::chat_usage::turn_usage_detail(self.records.get(&key)?)
    }
    /// Fold the durable workflow events into one record per run, keyed by start seq.
    pub fn workflow_runs(&self) -> std::collections::BTreeMap<u64, WorkflowRun> {
        let mut runs: std::collections::BTreeMap<String, WorkflowRun> = Default::default();
        let mut events: Vec<_> = self.records.values().collect();
        events.sort_by_key(|event| event.seq);
        for event in events {
            let run_id = event.data["runId"].as_str().unwrap_or("");
            if run_id.is_empty() {
                continue;
            }
            match event.event_type.as_str() {
                "tool-workflow/run-start" => {
                    runs.insert(
                        run_id.to_owned(),
                        WorkflowRun {
                            start_seq: event.seq,
                            name: bounded(event.data["name"].as_str().unwrap_or("Workflow"), 120),
                            stop: None,
                            members: Vec::new(),
                        },
                    );
                }
                "tool-workflow/agent-start" => {
                    let Some(run) = runs.get_mut(run_id) else {
                        continue;
                    };
                    let Some(label) = event.data["label"].as_str() else {
                        continue;
                    };
                    run.members.push(WorkflowMember {
                        seq: event.data["seq"].as_u64().unwrap_or(0),
                        label: bounded(label, 120),
                        phase: event.data["phase"].as_str().map(|phase| bounded(phase, 80)),
                        child_id: event.data["childId"].as_str().map(str::to_owned),
                        outcome: None,
                    });
                }
                "tool-workflow/agent-end" => {
                    let Some(run) = runs.get_mut(run_id) else {
                        continue;
                    };
                    let seq = event.data["seq"].as_u64().unwrap_or(0);
                    if let Some(member) = run.members.iter_mut().find(|member| member.seq == seq) {
                        member.outcome = event.data["outcome"].as_str().map(str::to_owned);
                    }
                }
                "tool-workflow/run-end" => {
                    if let Some(run) = runs.get_mut(run_id) {
                        run.stop = event.data["stopReason"].as_str().map(str::to_owned);
                    }
                }
                _ => {}
            }
        }
        runs.into_values().map(|run| (run.start_seq, run)).collect()
    }
    /// Current feedback values derived from the log's log-only feedback events.
    pub fn feedback_state(&self) -> std::collections::BTreeMap<String, serde_json::Value> {
        let mut state = std::collections::BTreeMap::new();
        let mut events: Vec<_> = self.records.values().collect();
        events.sort_by_key(|event| event.seq);
        for event in events {
            match event.event_type.as_str() {
                "feedback/message-put" => {
                    let Some(item) = event.data.get("item") else {
                        continue;
                    };
                    let Some(message_id) =
                        item.get("messageId").and_then(serde_json::Value::as_str)
                    else {
                        continue;
                    };
                    state.insert(message_id.to_owned(), item.clone());
                }
                "feedback/message-delete" => {
                    if let Some(message_id) = event
                        .data
                        .get("messageId")
                        .and_then(serde_json::Value::as_str)
                    {
                        state.remove(message_id);
                    }
                }
                _ => {}
            }
        }
        state
    }
    /// Every `workspace/changes` record as `(turn, seq)`, in log order.
    pub fn changes_events(&self) -> Vec<(u64, u64)> {
        self.records
            .values()
            .filter(|event| event.event_type == "workspace/changes")
            .filter_map(|event| {
                event
                    .data
                    .get("turn")
                    .and_then(serde_json::Value::as_u64)
                    .map(|turn| (turn, event.seq))
            })
            .collect()
    }
    /// The announcing `workspace/changes` seq for one turn, if any.
    pub fn changes_seq(&self, turn: u64) -> Option<u64> {
        self.changes_events()
            .into_iter()
            .find(|(candidate, _)| *candidate == turn)
            .map(|(_, seq)| seq)
    }
    /// The whole-log decode speed from the retained assistant records.
    pub fn decode_speed(&self) -> Option<String> {
        crate::ui::chat_usage::decode_speed(self.records.values())
    }
    /// The next-turn queue as the composer's dock draws it, rebuilt from the retained inbox
    /// splices. Read-only; the reducer keeps the same records for the chat rows.
    pub fn queued(&self) -> Vec<crate::queue_dock::Item> {
        if let Some(rows) = self
            .projections
            .get("inbox")
            .and_then(|inbox| inbox.get("next-turn"))
            .and_then(serde_json::Value::as_array)
        {
            return crate::queue_dock::from_rows(rows);
        }
        crate::queue_dock::from_events(
            self.records
                .values()
                .filter(|event| event.event_type == "agent/inbox/spliced"),
        )
    }
    pub fn projections(&self) -> &BTreeMap<String, serde_json::Value> {
        &self.projections
    }
    /// Apply one Host-computed projection push to the retained window. Returns
    /// whether the value changed: a frame at or below the seq already held loses,
    /// so a replayed frame cannot regress one the snapshot or a newer frame set.
    pub fn apply_projection(&mut self, key: &str, value: serde_json::Value, seq: i64) -> bool {
        if self
            .projection_seqs
            .get(key)
            .is_some_and(|held| seq <= *held)
        {
            return false;
        }
        self.projections.insert(key.to_owned(), value);
        self.projection_seqs.insert(key.to_owned(), seq);
        true
    }
    /// Whether the retained stream is broken. Read-only: the app announces it but
    /// never repairs it here.
    pub fn interrupted(&self) -> bool {
        self.error.is_some()
    }
    /// Bounded read-only work records; rebuilt from the same retained events as rows.
    pub fn work(&self) -> &crate::work_panel::Store {
        &self.work
    }
    /// Project retained rows; cancellation, malformed/future records and Send gates are unchanged.
    /// The session's agent preset, from the Host's own `agentPreset` projection
    /// (`ui-agent-preset/src/client/seat-store.ts:253`). The projection may carry it as a
    /// string or as an object with an `id`, so both shapes are read; the display name is not
    /// in this projection, so a caller falls back to the key, exactly as the permission chip
    /// does when the catalog is absent.
    pub fn agent_preset(&self) -> Option<&str> {
        match self.projections.get("agentPreset")? {
            serde_json::Value::String(value) => (!value.is_empty()).then_some(value.as_str()),
            value => value
                .get("id")
                .and_then(serde_json::Value::as_str)
                .filter(|id| !id.is_empty()),
        }
    }
    /// The most recent `permission/preset` record's preset, if the session carried one.
    /// The record already reaches native (`fixtures/chat-visibility.json` opens with
    /// `workspace-write`, `known.rs` lists the kind), and this is the projection the
    /// reference's permission chip reads its current value from; the catalog it also
    /// needs is a remote call, not a record
    /// (`ui-permission-presets/src/client/catalog.ts:134-148`).
    pub fn permission_preset(&self) -> Option<&str> {
        self.records
            .values()
            .rev()
            .filter(|record| record.event_type == "permission/preset")
            // A malformed payload is skipped rather than erasing a known preset, the
            // same way every other record projection here treats one.
            .find_map(|record| record.data["preset"].as_str())
    }
    pub fn display_rows(&self, records: bool) -> impl Iterator<Item = &DisplayRow> {
        self.rows.iter().filter(move |row| {
            !self
                .records
                .get(&row.key)
                .is_some_and(|event| presentation::ui_control_event(&event.event_type))
                && (records
                    || (row.chat
                        && !(row.surface
                            && self.records.get(&row.key).is_some_and(|event| {
                                event.surface_op.as_ref().is_some_and(Value::is_object)
                            }))
                        // The role, not the kind, says this splice is not a
                        // routine queue outcome: the same kind carries a
                        // submitted message and a bare cancel.
                        && !(row.role == "agent/inbox/spliced"
                            && self
                                .records
                                .get(&row.key)
                                .is_some_and(|event| presentation::routine_inbox(&event.data)))))
        })
    }
    /// Display formatting eligibility comes from exact retained provenance, never a role label.
    pub fn assistant_record(&self, key: u64) -> bool {
        self.records.get(&key).is_some_and(|event| {
            matches!(
                event.event_type.as_str(),
                "assistant/message" | "assistant/attempt"
            )
        })
    }
    /// Published assistant stream only; context updates and undecodable/unknown blocks stay raw.
    pub fn assistant_parts(&self, key: u64) -> Option<AssistantParts> {
        let event = self.records.get(&key)?;
        if !self.assistant_record(key)
            || event.surface_op.as_ref().is_some_and(Value::is_object)
            || !self.rows.iter().any(|row| row.key == key)
        {
            return None;
        }
        assistant_parts::project(&stream_blocks(&event.data["stream"]).ok()?)
    }
    pub fn inbox_canceled(&self, key: u64) -> bool {
        self.records.get(&key).is_some_and(|event| {
            event.event_type == "agent/inbox/spliced" && event.data["outcome"] == "canceled"
        })
    }
    pub fn turn_notice(&self, key: u64) -> bool {
        self.records.get(&key).is_some_and(|event| {
            event.event_type == "turn/end" && chat_presentation::turn_notice(&event.data).is_some()
        })
    }
    /// Read-only ledger check used by the composed file-admission example.
    #[allow(dead_code)]
    pub fn submitted_input(&self, key: u64) -> bool {
        // A splice that carried her message is labelled `You` by the fold; one
        // that only reported a queue outcome keeps the kind as its label. The
        // kind alone cannot tell them apart, so the label is the signal here.
        self.records
            .get(&key)
            .is_some_and(|event| event.event_type == "agent/inbox/spliced")
            && self
                .rows
                .iter()
                .any(|row| row.key == key && row.role == "You")
    }
    pub fn records(&self) -> impl Iterator<Item = &SessionWireEvent> {
        self.records.values()
    }
    /// Populate a local text draft, never replay file receipts, queue input or model context.
    pub fn editable_text(&self, key: u64) -> Option<String> {
        let event = self.records.get(&key)?;
        if event.event_type != "user/message"
            || event.surface_op.as_ref().is_some_and(Value::is_object)
        {
            return None;
        }
        let parts = event.data["content"].as_array()?;
        if parts.is_empty()
            || !parts
                .iter()
                .all(|part| part["type"] == "text" && part["text"].is_string())
        {
            return None;
        }
        let text = parts
            .iter()
            .map(|part| part["text"].as_str().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        (!text.trim().is_empty() && text.len() <= MAX_DISPLAY_BYTES).then_some(text)
    }
    /// Derive a compact tool label from retained metadata, without changing the fold.
    /// `roots` is the display shortening a file path gets, so the label is the
    /// one the view draws rather than an unshortened path the view would redo.
    pub fn activity(&self, key: u64, roots: activity::PathRoots<'_>) -> Option<activity::Summary> {
        let paired = self.paired.get(&key).and_then(|seq| self.records.get(seq));
        activity::summarize_paired(
            self.records.get(&key)?,
            self.records.range(..key).rev().map(|(_, event)| event),
            paired,
            roots,
        )
    }
    pub fn before(&self) -> Option<u64> {
        self.before
    }
    pub fn opening_cursor(&self) -> i64 {
        self.opening
    }
    pub fn cursor(&self) -> i64 {
        self.cursor
    }
    pub fn has_more(&self) -> bool {
        self.more
    }
    pub fn safe_to_send(&self) -> bool {
        self.error.is_none() && self.cursor >= -1 && !self.gap
    }
    fn visible_attempt(&self) -> Option<&Attempt> {
        self.attempt.as_ref().filter(|a| {
            !self.records.values().any(|event| {
                !a.pending.contains(&event.seq)
                    && matches!(
                        event.event_type.as_str(),
                        "assistant/message" | "assistant/attempt"
                    )
                    && event.data.get("turn").and_then(Value::as_u64) == Some(a.turn)
                    && event.data.get("step").and_then(Value::as_u64) == Some(a.step)
            })
        })
    }
    pub fn partial(&self) -> Option<String> {
        self.visible_attempt()
            .map(|a| raw_blocks(&a.blocks))
            .filter(|s| !s.is_empty())
    }
    /// Uses the same pending settlement and retained-record suppression as `partial`.
    pub fn partial_parts(&self) -> Option<AssistantParts> {
        let attempt = self.visible_attempt()?;
        if attempt.blocks.len() <= 1 && attempt.blocks.values().all(|b| b.text.is_empty()) {
            return None;
        }
        assistant_parts::project(&attempt.blocks)
    }
    pub fn apply(&mut self, generation: u64, frame: SessionFollowFrame) -> Result<(), ReduceError> {
        if generation != self.generation {
            return Ok(());
        }
        if !matches!(&frame, SessionFollowFrame::Snapshot { .. })
            && let Some(error) = self.error
        {
            return Err(error);
        }
        let result = self.apply_inner(frame);
        if let Err(error) = result {
            self.error = Some(error);
        }
        result
    }
    fn apply_inner(&mut self, frame: SessionFollowFrame) -> Result<(), ReduceError> {
        match frame {
            SessionFollowFrame::Snapshot {
                records,
                cursor,
                has_more,
                assistant_stream,
                projections,
                ..
            } => {
                self.records.clear();
                self.retained_bytes = 0;
                // A new snapshot replaces the window, so its projections replace
                // the previous ones rather than being merged onto them. Every value
                // is consistent with the baseline's own cut, which is the seq a
                // later push has to beat.
                self.projections = projections.values.clone();
                self.projection_seqs = projections
                    .values
                    .keys()
                    .map(|key| (key.clone(), projections.as_of_seq))
                    .collect();
                self.rows.clear();
                self.attempt = None;
                self.retained = None;
                self.error = None;
                self.gap = false;
                self.cursor = cursor;
                self.opening = cursor;
                self.revision = assistant_stream.as_ref().map_or(0, |s| s.revision);
                self.more = has_more;
                self.insert_records(records)?;
                if self
                    .records
                    .keys()
                    .next_back()
                    .is_some_and(|s| *s as i64 > cursor)
                {
                    return Err(ReduceError::InvalidSnapshot);
                }
                if let Some(baseline) = assistant_stream.and_then(|s| s.active_attempt) {
                    if baseline.started_after_seq > cursor {
                        return Err(ReduceError::StreamGap);
                    }
                    let mut attempt = Attempt {
                        id: baseline.attempt_id,
                        turn: baseline.turn,
                        step: baseline.step,
                        next: baseline.next_index,
                        blocks: BTreeMap::new(),
                        pending: BTreeSet::new(),
                    };
                    let expanded = compact_chunks(&Value::Array(baseline.stream))?;
                    if expanded.len() as u64 != attempt.next {
                        return Err(ReduceError::StreamGap);
                    }
                    for chunk in expanded {
                        fold_chunk(&mut attempt.blocks, &chunk)?;
                    }
                    self.attempt = Some(attempt);
                }
                self.rebuild()?;
            }
            SessionFollowFrame::Event { event } => {
                if event.seq as i64 <= self.cursor {
                    return Ok(());
                }
                if event.seq as i64 != self.cursor + 1 {
                    return Err(ReduceError::SequenceGap);
                }
                self.cursor = event.seq as i64;
                if let Some(attempt) = &mut self.attempt
                    && matches!(
                        event.event_type.as_str(),
                        "assistant/message" | "assistant/attempt"
                    )
                    && event.data.get("turn").and_then(Value::as_u64) == Some(attempt.turn)
                    && event.data.get("step").and_then(Value::as_u64) == Some(attempt.step)
                {
                    attempt.pending.insert(event.seq);
                }
                if event.event_type == "step/end"
                    && self.retained.as_ref().is_some_and(|(_, t, s)| {
                        event.data.get("turn").and_then(Value::as_u64) == Some(*t)
                            && event.data.get("step").and_then(Value::as_u64) == Some(*s)
                    })
                {
                    self.retained = None;
                }
                self.insert_event(event)?;
                self.rebuild()?;
            }
            SessionFollowFrame::AssistantStream { frame } => {
                let revision = match &frame {
                    AssistantFrame::Start { revision, .. }
                    | AssistantFrame::Chunk { revision, .. }
                    | AssistantFrame::End { revision, .. } => *revision,
                };
                if revision != self.revision + 1 {
                    return Err(ReduceError::StreamGap);
                }
                self.revision = revision;
                match frame {
                    AssistantFrame::Start {
                        attempt_id,
                        started_after_seq,
                        turn,
                        step,
                        ..
                    } => {
                        if self.attempt.is_some()
                            || self.retained.is_some()
                            || started_after_seq > self.cursor
                        {
                            return Err(ReduceError::StreamGap);
                        }
                        self.attempt = Some(Attempt {
                            id: attempt_id,
                            turn,
                            step,
                            next: 0,
                            blocks: BTreeMap::new(),
                            pending: BTreeSet::new(),
                        });
                    }
                    AssistantFrame::Chunk {
                        attempt_id,
                        index,
                        chunk,
                        ..
                    } => {
                        if let Some(attempt) = &mut self.attempt {
                            if attempt.id != attempt_id {
                                return Ok(());
                            }
                            if index != attempt.next {
                                return Err(ReduceError::StreamGap);
                            }
                            attempt.next += 1;
                            fold_chunk(&mut attempt.blocks, &chunk)?;
                        } // A mount lacking start ignores an unknown transient suffix, as alpha does.
                    }
                    AssistantFrame::End {
                        attempt_id,
                        index,
                        outcome,
                        ..
                    } => {
                        let Some(attempt) = self.attempt.take() else {
                            return Ok(());
                        };
                        if attempt.id != attempt_id {
                            self.attempt = Some(attempt);
                            return Ok(());
                        }
                        if index != attempt.next {
                            return Err(ReduceError::StreamGap);
                        }
                        match outcome {
                            AssistantOutcome::Abandoned => {
                                if !attempt.pending.is_empty() {
                                    return Err(ReduceError::StreamGap);
                                }
                            }
                            AssistantOutcome::Committed { seq, event_type } => {
                                let Some(event) = self.records.get(&seq) else {
                                    return Err(ReduceError::StreamGap);
                                };
                                if event.data.get("turn").and_then(Value::as_u64)
                                    != Some(attempt.turn)
                                    || event.data.get("step").and_then(Value::as_u64)
                                        != Some(attempt.step)
                                    || event.event_type
                                        != match event_type {
                                            AssistantSettlement::Message => "assistant/message",
                                            AssistantSettlement::Attempt => "assistant/attempt",
                                        }
                                    || (!attempt.pending.is_empty()
                                        && (attempt.pending.len() != 1
                                            || !attempt.pending.contains(&seq)))
                                {
                                    return Err(ReduceError::StreamGap);
                                }
                                if event.event_type == "assistant/message"
                                    && event.data.get("interrupted") != Some(&Value::Bool(true))
                                {
                                    self.retained = Some((attempt.id, attempt.turn, attempt.step));
                                }
                            }
                        }
                        self.rebuild()?;
                    }
                }
            }
        }
        Ok(())
    }
    pub fn page(&mut self, generation: u64, page: SessionPage) -> Result<(), ReduceError> {
        if generation != self.generation {
            return Ok(());
        }
        if page.records.iter().any(|r| match r {
            HistoryRecord::Event { event } => event.seq as i64 > self.opening,
        }) {
            self.error = Some(ReduceError::SequenceGap);
            return Err(ReduceError::SequenceGap);
        }
        self.more = page.has_more;
        let result = self
            .insert_records(page.records)
            .and_then(|()| self.rebuild());
        if let Err(e) = result {
            self.error = Some(e);
        }
        result
    }
    fn insert_records(&mut self, records: Vec<HistoryRecord>) -> Result<(), ReduceError> {
        for record in records {
            match record {
                HistoryRecord::Event { event } => {
                    self.insert_event(event)?;
                }
            }
        }
        self.before = self.records.keys().next().copied();
        Ok(())
    }
    fn insert_event(&mut self, event: SessionWireEvent) -> Result<(), ReduceError> {
        if let Some(old) = self.records.get(&event.seq) {
            if old.event_type != event.event_type
                || old.time != event.time
                || old.data != event.data
                || old.surface_op != event.surface_op
                || old.source_event_seqs != event.source_event_seqs
                || old.ignorable != event.ignorable
            {
                return Err(ReduceError::InvalidSnapshot);
            }
            return Ok(());
        }
        let size = serde_json::to_vec(&event)
            .map_err(|_| ReduceError::InvalidSnapshot)?
            .len();
        if self.records.len() >= MAX_RECORDS
            || size > MAX_RETAINED_BYTES.saturating_sub(self.retained_bytes)
        {
            return Err(ReduceError::RetentionLimit);
        }
        self.retained_bytes += size;
        self.records.insert(event.seq, event);
        Ok(())
    }
    fn rebuild(&mut self) -> Result<(), ReduceError> {
        let mut rows: Vec<DisplayRow> = vec![];
        let mut surface_nodes = Vec::new();
        let complete = self.records.contains_key(&0)
            && self
                .records
                .keys()
                .enumerate()
                .all(|(index, seq)| *seq == index as u64);
        let mut invalid = None;
        let recorded: BTreeSet<&str> = self
            .records
            .values()
            .filter(|event| {
                event.event_type == "user/message"
                    && !event.surface_op.as_ref().is_some_and(Value::is_object)
            })
            .filter_map(|event| event.data["id"].as_str())
            .collect();
        let mut unknown = false;
        let mut work = crate::work_panel::Store::default();
        // One row per call, as the reference draws it: a `tool/call` is indexed here so its
        // `tool/result` can fold onto it instead of adding a second activity line to Chat.
        let mut calls: std::collections::BTreeMap<(String, u64, u64), u64> = Default::default();
        let mut paired_calls: BTreeMap<u64, u64> = BTreeMap::new();
        for event in self.records.values() {
            if self
                .attempt
                .as_ref()
                .is_some_and(|a| a.pending.contains(&event.seq))
            {
                continue;
            }
            let kind = event.event_type.as_str();
            work.ingest(kind, &event.data);
            let submitted = (kind == "agent/inbox/spliced")
                .then(|| chat_presentation::submitted_text(&event.data, &recorded))
                .flatten();
            let mut files = Vec::new();
            let text = match kind {
                "user/message" => {
                    let (text, cards) = user_content(&event.data["content"]);
                    files = cards;
                    text
                }
                "system/message" | "developer/message" | "tool/result" => {
                    content(&event.data["message"]["content"])
                }
                "assistant/message" | "assistant/attempt" => {
                    match stream_text(&event.data["stream"]) {
                        Ok(text) => text,
                        Err(error) => {
                            invalid.get_or_insert(error);
                            "Assistant output could not be decoded. Reload this conversation."
                                .into()
                        }
                    }
                }
                "tool/call" => format!(
                    "{}\n{}",
                    event.data["name"].as_str().unwrap_or("Tool"),
                    bounded(
                        event.data["arguments"].as_str().unwrap_or(""),
                        MAX_DISPLAY_BYTES
                    )
                ),
                "agent/inbox/spliced" => {
                    submitted.clone().unwrap_or_else(|| safe_json(&event.data))
                }
                "turn/end" => match chat_presentation::turn_notice(&event.data) {
                    Some(notice) => notice,
                    None => continue,
                },
                // A compaction's open and close markers are transaction brackets:
                // they carry no information the prune and summary lines below do
                // not, so they get no row rather than an unboxed JSON fallback.
                "compaction/start" | "compaction/end" => continue,
                kind if crate::known::LOG_EVENTS.contains(&kind) => safe_json(&event.data),
                "turn/start" | "step/start" | "step/end" | "request/header" | "request/context"
                | "session/end-seed" | "image/offload" => continue,
                _ => {
                    if event.ignorable != Some(true) {
                        unknown = true;
                    }
                    safe_json(&event.data)
                }
            };
            let surface = matches!(
                kind,
                "system/message"
                    | "developer/message"
                    | "user/message"
                    | "assistant/message"
                    | "tool/result"
            );
            let role = if surface && event.surface_op.as_ref().is_some_and(Value::is_object) {
                "Model context update"
            } else {
                match kind {
                    "agent/inbox/spliced" if submitted.is_some() => "You",
                    "turn/end" if event.data["reason"]["kind"] == "error" => "Agent error",
                    "turn/end" => "Agent notice",
                    // Harness context injects share the user role on the wire; only the
                    // `user` producer is human history. Trajectory keeps the rest, and
                    // the loop guard's own reminder gets its producer named instead of
                    // joining the general inject label.
                    "user/message"
                        if event.data["source"]["kind"].as_str()
                            == Some("repeat-tool-reminder") =>
                    {
                        "Loop guard"
                    }
                    "user/message" if !presentation::human_message(&event.data["source"]) => {
                        "Injected context"
                    }
                    "user/message" => "You",
                    "assistant/message" => "Assistant",
                    "assistant/attempt" => "Interrupted assistant attempt",
                    "tool/result" => "Tool result",
                    "tool/call" => "Tool call",
                    "tool-workflow/run-start" => "Workflow",
                    "system/message" => "System",
                    "developer/message" => "Developer",
                    // Context management is log-only bookkeeping: named for
                    // Trajectory, still absent from the clean conversation. The
                    // open and close markers never reach this match.
                    "compaction/prune" => "Context elided",
                    "compaction/summary" => "Context summarized",
                    _ => kind,
                }
            };
            let trigger = (kind == "user/message")
                .then(|| presentation::turn_trigger(&event.data["source"]))
                .flatten();
            let mut row = DisplayRow {
                key: event.seq,
                role: trigger.map_or_else(|| bounded(role, 128), |trigger| trigger.title.into()),
                kind: bounded(kind, 128),
                // A recognized trigger is clean conversation content, drawn as a notice;
                // every other non-human inject stays in Trajectory only.
                chat: trigger.is_some()
                    || kind == "tool-workflow/run-start"
                    || presentation::chat_visible(kind, role),
                text: bounded(&text, MAX_DISPLAY_BYTES),
                files,
                references: Vec::new(),
                turn: event.data["turn"].as_u64(),
                message_id: event.data["message"]["id"].as_str().map(str::to_owned),
                trigger,
                time: event.time,
                surface,
                output: None,
                failed: false,
            };
            // Pair a result with its call by the wire's own identifiers, exactly as the
            // activity fold pairs them. A result that names no call keeps its own row: an
            // unpaired result is information, not a duplicate.
            if kind == "tool/call"
                && let (Some(id), Some(turn), Some(step)) = (
                    event.data["callId"].as_str(),
                    event.data["turn"].as_u64(),
                    event.data["step"].as_u64(),
                )
            {
                calls.insert((id.to_owned(), turn, step), event.seq);
            }
            if kind == "tool/result" {
                let message = &event.data["message"];
                let paired = (|| {
                    let id = message["toolCallId"].as_str().filter(|id| !id.is_empty())?;
                    if message["role"].as_str() != Some("tool")
                        || message["source"]["kind"].as_str() != Some("tool")
                        || message["source"]["callId"].as_str() != Some(id)
                    {
                        return None;
                    }
                    let turn = event.data["turn"].as_u64()?;
                    let step = event.data["step"].as_u64()?;
                    calls.get(&(id.to_owned(), turn, step)).copied()
                })();
                if let Some(call) = paired
                    && let Some(target) = rows.iter_mut().find(|row| row.key == call)
                {
                    target.output = Some(row.text.clone());
                    target.failed = message["isError"].as_bool() == Some(true);
                }
                if let Some(call) = paired {
                    paired_calls.insert(call, event.seq);
                    // The record stays in Trajectory, where every record is listed; Chat draws
                    // the call once, with this result folded into it.
                    row.chat = false;
                }
            }
            // A `session-reference` context record carries the labels cited by the
            // message it follows, which the reference draws under that bubble
            // (`conversation-nodes/event-projection.ts:83-87`); it is attributed before
            // the record's own hidden row is pushed.
            if event.data["source"]["kind"].as_str() == Some("session-reference") {
                let labels: Vec<String> = event.data["source"]["references"]
                    .as_array()
                    .map(|references| {
                        references
                            .iter()
                            .filter_map(|reference| reference["label"].as_str())
                            .map(|label| bounded(label, 128))
                            .collect()
                    })
                    .unwrap_or_default();
                if !labels.is_empty()
                    && let Some(human) = rows.iter_mut().rev().find(|row| row.role == "You")
                {
                    human.references = labels;
                }
            }
            if !row.surface {
                rows.push(row);
                continue;
            }
            if let Err(error) =
                chat_presentation::surface_update(&mut surface_nodes, event, complete)
            {
                invalid.get_or_insert(error);
            }
            // Context replacements change future model input, never the human chat log.
            rows.push(row);
        }
        self.rows = rows;
        self.paired = paired_calls;
        // The work store is a projection of the same retained records as rows; a
        // malformed payload contributes nothing and never changes Send authority.
        self.work = work;
        if let Some(error) = invalid {
            return Err(error);
        }
        if unknown {
            return Err(ReduceError::UnknownRequiredEvent);
        }
        Ok(())
    }
}
pub fn bounded(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.into();
    }
    let marker = "\n[Display truncated; native limit reached]";
    let suffix = if max >= marker.len() { marker } else { "" };
    let mut end = max.saturating_sub(suffix.len()).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{suffix}", &text[..end])
}
pub fn safe_json(value: &Value) -> String {
    fn redact(value: &Value, depth: usize) -> Value {
        if depth > 32 {
            return Value::String("[depth limited]".into());
        }
        match value {
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(key, value)| {
                        let normalized = key.to_ascii_lowercase().replace(['_', '-'], "");
                        (
                            key.clone(),
                            if [
                                "token",
                                "secret",
                                "cookie",
                                "authorization",
                                "apikey",
                                "password",
                                "authorizeurl",
                            ]
                            .contains(&normalized.as_str())
                            {
                                Value::String("[redacted]".into())
                            } else {
                                redact(value, depth + 1)
                            },
                        )
                    })
                    .collect(),
            ),
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .take(256)
                    .map(|v| redact(v, depth + 1))
                    .collect(),
            ),
            Value::String(s) => Value::String(bounded(s, MAX_DISPLAY_BYTES)),
            _ => value.clone(),
        }
    }
    bounded(
        &serde_json::to_string_pretty(&redact(value, 0))
            .unwrap_or_else(|_| "[Unsupported JSON]".into()),
        MAX_DISPLAY_BYTES,
    )
}
/// A file part's bounded name and byte count: the two facts the reference's
/// `fileCard` shows. `None` when either is missing or mistyped, so malformed
/// attachment metadata never reaches the view as a card.
fn file_identity(attachment: &Value) -> Option<(String, u64)> {
    let name = attachment["name"].as_str()?;
    let bytes = attachment["bytes"].as_u64()?;
    Some((
        name.chars()
            .take(80)
            .map(|c| if c.is_control() { '�' } else { c })
            .collect(),
        bytes,
    ))
}
fn file_summary(attachment: &Value) -> String {
    let Some((name, bytes)) = file_identity(attachment) else {
        return "[File attachment: metadata unavailable]".into();
    };
    format!("File · {name} · {bytes} bytes [preview unavailable]")
}
/// The reference's `fileExtension` (`ui-primitives/FileTypeIcon.tsx:122-126`):
/// the basename's text after its final dot, empty when there is none.
fn file_extension(name: &str) -> &str {
    let base = name.rsplit_once(['/', '\\']).map_or(name, |(_, base)| base);
    base.rsplit_once('.').map_or("", |(_, extension)| extension)
}
/// The reference's `fileSizeText` (`ui-primitives/file-size.ts:8-15`): whole
/// units, and one decimal only below ten of the chosen unit.
fn file_size_text(bytes: u64) -> String {
    let bytes = bytes as f64;
    if bytes < 1024.0 {
        return format!("{}B", bytes as u64);
    }
    let kb = bytes / 1024.0;
    if kb < 1024.0 {
        return if kb < 10.0 {
            format!("{kb:.1}KB")
        } else {
            format!("{}KB", kb.round() as u64)
        };
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return if mb < 10.0 {
            format!("{mb:.1}MB")
        } else {
            format!("{}MB", mb.round() as u64)
        };
    }
    let gb = mb / 1024.0;
    if gb < 10.0 {
        format!("{gb:.1}GB")
    } else {
        format!("{}GB", gb.round() as u64)
    }
}
/// A file part as its card. The meta is the reference's
/// `[EXT, SIZE].filter(Boolean).join(' ')` (`MessageItem.tsx:213-214`), so an
/// extension-less name shows the size alone.
/// The reference's `FileCard` meta line for a typed Host attachment: the
/// extension in upper case (bounded) and the reference's size text, or the size
/// alone when the name has no extension (`FileCard.tsx:22-34`,
/// `ui-primitives/file-size.ts:8-15`). Shared by a message's cards and the
/// composer's rail so both read the same rules.
pub(crate) fn attachment_meta(name: &str, bytes: u64) -> String {
    let extension: String = file_extension(name)
        .to_uppercase()
        .chars()
        .take(8)
        .collect();
    let size = file_size_text(bytes);
    if extension.is_empty() {
        size
    } else {
        format!("{extension} {size}")
    }
}
fn file_card(attachment: &Value) -> Option<FileCard> {
    let (name, bytes) = file_identity(attachment)?;
    let extension: String = file_extension(&name)
        .to_uppercase()
        .chars()
        .take(8)
        .collect();
    let size = file_size_text(bytes);
    Some(FileCard {
        name,
        meta: if extension.is_empty() {
            size
        } else {
            format!("{extension} {size}")
        },
    })
}
/// A user message's content parts split into the body text its bubble shows and
/// the file cards stacked above it. Every other caller keeps [`content`], which
/// still joins a `File · …` line, because a tool result or a system message has
/// no card to move a file into.
fn user_content(value: &Value) -> (String, Vec<FileCard>) {
    let Some(parts) = value.as_array() else {
        return (content(value), Vec::new());
    };
    let mut text = Vec::new();
    let mut files = Vec::new();
    for part in parts {
        match part["type"].as_str() {
            Some("text" | "reasoning") => text.push(part["text"].as_str().unwrap_or("").to_owned()),
            Some("tool-call") => text.push(format!(
                "Tool {}: {}",
                part["name"].as_str().unwrap_or("unknown"),
                part["arguments"].as_str().unwrap_or("")
            )),
            Some("image") => text.push("[Image attachment: native image panel unsupported]".into()),
            Some("file") => match file_card(&part["attachment"]) {
                Some(card) => files.push(card),
                // A file whose metadata is missing keeps its text line rather
                // than disappearing from the bubble entirely.
                None => text.push(file_summary(&part["attachment"])),
            },
            _ => text.push(safe_json(part)),
        }
    }
    (text.join("\n"), files)
}
pub(super) fn content(value: &Value) -> String {
    if let Some(s) = value.as_str() {
        return bounded(s, MAX_DISPLAY_BYTES);
    }
    if let Some(parts) = value.as_array() {
        return parts
            .iter()
            .map(|part| match part["type"].as_str() {
                Some("text" | "reasoning") => part["text"].as_str().unwrap_or("").to_owned(),
                Some("tool-call") => format!(
                    "Tool {}: {}",
                    part["name"].as_str().unwrap_or("unknown"),
                    part["arguments"].as_str().unwrap_or("")
                ),
                Some("image") => "[Image attachment: native image panel unsupported]".into(),
                Some("file") => file_summary(&part["attachment"]),
                _ => safe_json(part),
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    safe_json(value)
}
fn compact_chunks(value: &Value) -> Result<Vec<Value>, ReduceError> {
    let records = value.as_array().ok_or(ReduceError::InvalidSnapshot)?;
    let mut chunks = vec![];
    for record in records {
        match record["type"].as_str() {
            Some("chunk") => chunks.push(record["chunk"].clone()),
            Some(kind @ ("text-chunks" | "reasoning-chunks" | "tool-call-chunks")) => {
                let members = record[if kind == "tool-call-chunks" {
                    "args"
                } else {
                    "texts"
                }]
                .as_array()
                .ok_or(ReduceError::InvalidSnapshot)?;
                let dt = record["dt"]
                    .as_array()
                    .ok_or(ReduceError::InvalidSnapshot)?;
                if members.is_empty() || dt.len() + 1 != members.len() {
                    return Err(ReduceError::InvalidSnapshot);
                }
                for member in members {
                    let text = member.as_str().ok_or(ReduceError::InvalidSnapshot)?;
                    chunks.push(if kind=="tool-call-chunks"{serde_json::json!({"type":"tool-call-delta","index":record["index"],"id":record["id"],"name":record["name"],"argumentsDelta":text})}else{serde_json::json!({"type":if kind=="text-chunks"{"text-delta"}else{"reasoning-delta"},"index":record["index"],"text":text})});
                }
            }
            _ => return Err(ReduceError::InvalidSnapshot),
        }
    }
    Ok(chunks)
}
fn fold_chunk(blocks: &mut BTreeMap<u64, Block>, chunk: &Value) -> Result<(), ReduceError> {
    let index = chunk["index"].as_u64().unwrap_or(0);
    match chunk["type"].as_str() {
        Some(kind @ ("text-delta" | "reasoning-delta")) => {
            let value = chunk["text"].as_str().ok_or(ReduceError::StreamGap)?;
            let new = !blocks.contains_key(&index);
            let block = blocks.entry(index).or_default();
            if block.text.len() + value.len() > MAX_DISPLAY_BYTES {
                return Err(ReduceError::RetentionLimit);
            }
            block.append_kind(
                if kind == "reasoning-delta" {
                    Kind::Reasoning
                } else {
                    Kind::Answer
                },
                new,
            );
            block.text.push_str(value);
        }
        Some("block-end") => {
            let value = &chunk["block"];
            let kind = match value["type"].as_str() {
                Some("reasoning") if value["text"].is_string() => Some(Kind::Reasoning),
                Some("text") if value["text"].is_string() => Some(Kind::Answer),
                // A tool call is model-internal output like reasoning; it must
                // never become answer prose. Attachments stay answer content.
                Some("tool-call") => Some(Kind::Reasoning),
                Some("image" | "file") => Some(Kind::Answer),
                _ => None,
            };
            blocks.insert(
                index,
                Block {
                    text: content(&Value::Array(vec![value.clone()])),
                    kind,
                },
            );
        }
        Some("tool-call-delta") => {
            let value = chunk["argumentsDelta"]
                .as_str()
                .ok_or(ReduceError::StreamGap)?;
            let new = !blocks.contains_key(&index);
            let block = blocks.entry(index).or_insert_with(|| Block {
                text: format!("Tool {}: ", chunk["name"].as_str().unwrap_or("call")),
                kind: Some(Kind::Reasoning),
            });
            if block.text.len() + value.len() > MAX_DISPLAY_BYTES {
                return Err(ReduceError::RetentionLimit);
            }
            block.append_kind(Kind::Reasoning, new);
            block.text.push_str(value);
        }
        Some("block-start" | "usage" | "finish") => {}
        _ => return Err(ReduceError::StreamGap),
    }
    if chunk["index"].as_u64().is_none()
        && let Some(block) = blocks.get_mut(&index)
        && !matches!(
            chunk["type"].as_str(),
            Some("block-start" | "usage" | "finish")
        )
    {
        block.kind = None;
    }
    if blocks.len() > 256
        || blocks.values().map(|b| b.text.len()).sum::<usize>() + blocks.len().saturating_sub(1)
            > MAX_DISPLAY_BYTES
    {
        return Err(ReduceError::RetentionLimit);
    }
    Ok(())
}
fn raw_blocks(blocks: &BTreeMap<u64, Block>) -> String {
    blocks
        .values()
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
fn stream_blocks(stream: &Value) -> Result<BTreeMap<u64, Block>, ReduceError> {
    let mut blocks = BTreeMap::new();
    for chunk in compact_chunks(stream)? {
        fold_chunk(&mut blocks, &chunk)?;
    }
    Ok(blocks)
}
fn stream_text(stream: &Value) -> Result<String, ReduceError> {
    Ok(raw_blocks(&stream_blocks(stream)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn durable_file_only_content_exposes_only_bounded_name_size_without_receipts_or_digest() {
        let text = content(
            &json!([{"type":"file","attachment":{"name":"PUBLIC.bin","bytes":0,"attachmentId":"PUBLIC_DO_NOT_DISPLAY_ID"},"receiptId":"PUBLIC_DO_NOT_DISPLAY_RECEIPT"}]),
        );
        assert_eq!(text, "File · PUBLIC.bin · 0 bytes [preview unavailable]");
        assert!(!text.contains("DO_NOT_DISPLAY"));
        let text = file_summary(&json!({"name":format!("line\n{}","界".repeat(5000)),"bytes":3}));
        assert!(!text.contains('\n'));
        assert!(text.len() < 400);
    }
    #[test]
    fn malformed_file_metadata_never_falls_back_to_raw_attachment_object() {
        for value in [
            json!({"name":"PUBLIC","bytes":-1,"secret":"DO_NOT_SHOW"}),
            json!({"bytes":0,"receiptId":"DO_NOT_SHOW"}),
            json!({"name":3,"bytes":0}),
        ] {
            assert_eq!(
                file_summary(&value),
                "[File attachment: metadata unavailable]"
            );
        }
    }
    #[test]
    fn user_content_moves_a_file_part_out_of_the_body_and_into_its_own_card() {
        let (text, files) = user_content(&json!([
            {"type":"text","text":"PUBLIC message text"},
            {"type":"file","attachment":{"name":"PUBLIC-note.txt","bytes":12}}
        ]));
        assert_eq!(text, "PUBLIC message text");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "PUBLIC-note.txt");
        assert_eq!(files[0].meta, "TXT 12B");
        // The joined form other callers keep still carries the file line, so a
        // tool result never loses it.
        assert!(
            content(&json!([{"type":"file","attachment":{"name":"PUBLIC-note.txt","bytes":12}}]))
                .starts_with("File · PUBLIC-note.txt · 12 bytes")
        );
    }
    #[test]
    fn file_card_follows_the_reference_extension_and_size_rules() {
        // `fileExtension`: the basename's text after its final dot, either
        // separator (`FileTypeIcon.tsx:122-126`).
        assert_eq!(file_extension("/PUBLIC/dir/note.txt"), "txt");
        assert_eq!(file_extension("C:\\PUBLIC\\dir\\note.tar.gz"), "gz");
        assert_eq!(file_extension(".PUBLIC-hidden"), "PUBLIC-hidden");
        assert_eq!(file_extension("PUBLIC-readme"), "");
        assert_eq!(file_extension("PUBLIC."), "");
        // `fileSizeText`: whole units, one decimal only below ten of the unit
        // (`file-size.ts:8-15`).
        for (bytes, size) in [
            (0u64, "0B"),
            (1023, "1023B"),
            (1024, "1.0KB"),
            (10 * 1024, "10KB"),
            (1536 * 1024, "1.5MB"),
            (2 * 1024 * 1024 * 1024, "2.0GB"),
            (10 * 1024 * 1024 * 1024, "10GB"),
        ] {
            assert_eq!(file_size_text(bytes), size, "{bytes} bytes");
        }
        // The meta is `[EXT, SIZE].filter(Boolean).join(' ')`, so a name with no
        // extension shows the size alone (`MessageItem.tsx:213-214`).
        assert_eq!(
            file_card(&json!({"name":"PUBLIC-readme","bytes":1024}))
                .unwrap()
                .meta,
            "1.0KB"
        );
    }
    #[test]
    fn a_file_part_without_usable_metadata_stays_a_bounded_text_line() {
        for attachment in [
            json!({"bytes":0,"receiptId":"DO_NOT_SHOW"}),
            json!({"name":3,"bytes":0}),
            json!({"name":"PUBLIC","bytes":-1}),
        ] {
            assert!(file_card(&attachment).is_none());
            let (text, files) = user_content(&json!([{"type":"file","attachment":attachment}]));
            assert!(files.is_empty());
            assert_eq!(text, "[File attachment: metadata unavailable]");
            assert!(!text.contains("DO_NOT_SHOW"));
        }
    }
    use serde_json::json;
    fn frame(value: Value) -> SessionFollowFrame {
        serde_json::from_value(value).unwrap()
    }
    fn snapshot(records: Vec<Value>, through: i64) -> SessionFollowFrame {
        frame(
            json!({"type":"snapshot","records":records,"cursor":through,"hasMore":true,"header":{"version":4,"id":"s1","createdAt":1000,"isSeeded":false},"projections":{"asOfSeq":through,"values":{}},"assistantStream":{"revision":0}}),
        )
    }
    fn user(seq: u64, text: &str) -> Value {
        json!({"type":"event","event":{"type":"user/message","seq":seq,"time":1000,"data":{"role":"user","id":"m1","source":{"kind":"user"},"content":[{"type":"text","text":text}]},"surfaceOp":"append"}})
    }
    #[test]
    fn the_agent_preset_projects_only_the_hosts_own_value() {
        let with = |values: Value| {
            let mut s = Transcript::new(1);
            s.apply(
                1,
                frame(
                    json!({"type":"snapshot","records":[],"cursor":0,"hasMore":false,
                    "header":{"version":4,"id":"s1","createdAt":1000,"isSeeded":false},
                    "projections":{"asOfSeq":0,"values":values},"assistantStream":{"revision":0}}),
                ),
            )
            .unwrap();
            s
        };
        // Absent, empty and malformed all mean no preset rather than an empty label.
        assert_eq!(with(json!({})).agent_preset(), None);
        assert_eq!(with(json!({"agentPreset":""})).agent_preset(), None);
        assert_eq!(with(json!({"agentPreset":{}})).agent_preset(), None);
        assert_eq!(with(json!({"agentPreset":{"id":""}})).agent_preset(), None);
        assert_eq!(with(json!({"agentPreset":7})).agent_preset(), None);
        // Both shapes the Host may use, and the string form wins nothing else.
        assert_eq!(
            with(json!({"agentPreset":"reviewer"})).agent_preset(),
            Some("reviewer")
        );
        assert_eq!(
            with(json!({"agentPreset":{"id":"implementer","name":"Implementer"}})).agent_preset(),
            Some("implementer")
        );
    }
    #[test]
    fn snapshot_append_replace_and_source_citations() {
        let mut s = Transcript::new(1);
        s.apply(1, snapshot(vec![user(0, "你好 🦀")], 0)).unwrap();
        s.apply(1, frame(user(1, "next"))).unwrap();
        let replacement = json!({"type":"event","event":{"type":"user/message","seq":2,"time":1002,"data":{"content":[{"type":"text","text":"summary"}]},"surfaceOp":{"op":"replace","startSeq":0,"endSeq":1},"sourceEventSeqs":[0,1]}});
        s.apply(1, frame(replacement)).unwrap();
        assert_eq!(s.rows.len(), 3);
        assert_eq!(
            s.display_rows(false)
                .map(|row| row.text.as_str())
                .collect::<Vec<_>>(),
            ["你好 🦀", "next"]
        );
        assert_eq!(s.rows[2].text, "summary");
    }
    #[test]
    fn a_session_reference_attaches_its_labels_to_the_message_it_follows() {
        let recall = |seq: u64, references: Value| json!({"type":"event","event":{"type":"user/message","seq":seq,"time":1000+seq,"data":{"role":"user","id":"ref","source":{"kind":"session-reference","references":references},"content":[{"type":"text","text":"PUBLIC recalled context"}]},"surfaceOp":"append"}});
        let mut s = Transcript::new(1);
        s.apply(
            1,
            snapshot(
                vec![
                    user(0, "PUBLIC question"),
                    recall(
                        1,
                        json!([{"label":"PUBLIC session A"},{"label":"PUBLIC session B"}]),
                    ),
                    user(2, "PUBLIC follow-up"),
                ],
                2,
            ),
        )
        .unwrap();
        // The labels land on the human message the record follows, not on the hidden
        // context row and not on a later message.
        assert_eq!(
            s.rows[0].references.join(" | "),
            "PUBLIC session A | PUBLIC session B"
        );
        assert!(s.rows[1].references.is_empty());
        assert!(s.rows[2].references.is_empty());
        // The context row itself stays out of the clean conversation.
        assert!(!s.rows[1].chat);
        // A malformed reference list contributes nothing rather than a label.
        let mut s = Transcript::new(1);
        s.apply(
            1,
            snapshot(
                vec![
                    user(0, "PUBLIC question"),
                    recall(1, json!([{"label":3},{},{"label":"PUBLIC keep"}])),
                ],
                1,
            ),
        )
        .unwrap();
        assert_eq!(s.rows[0].references.join(" | "), "PUBLIC keep");
    }
    #[test]
    fn the_permission_preset_projects_the_latest_record_only() {
        let preset = |seq: u64, value: Value| json!({"type":"event","event":{"type":"permission/preset","seq":seq,"time":1000+seq,"data":value}});
        let mut s = Transcript::new(1);
        s.apply(
            1,
            snapshot(vec![preset(0, json!({"preset":"read-only"}))], 0),
        )
        .unwrap();
        assert_eq!(s.permission_preset(), Some("read-only"));
        // A later record wins, whatever order the frame arrives in.
        s.apply(1, frame(preset(1, json!({"preset":"workspace-write"}))))
            .unwrap();
        assert_eq!(s.permission_preset(), Some("workspace-write"));
        // A malformed or missing value projects nothing rather than a guess.
        s.apply(1, frame(preset(2, json!({})))).unwrap();
        assert_eq!(s.permission_preset(), Some("workspace-write"));
        assert_eq!(Transcript::new(1).permission_preset(), None);
    }
    #[test]
    fn stale_generation_does_not_change_session() {
        let mut s = Transcript::new(2);
        s.apply(1, snapshot(vec![user(0, "wrong")], 0)).unwrap();
        assert!(s.rows().is_empty());
    }
    #[test]
    fn durable_gap_marks_unsafe() {
        let mut s = Transcript::new(1);
        s.apply(1, snapshot(vec![user(0, "hello")], 0)).unwrap();
        assert_eq!(
            s.apply(1, frame(user(2, "gap"))),
            Err(ReduceError::SequenceGap)
        );
        assert!(!s.safe_to_send());
    }
    #[test]
    fn alpha_compact_stream_commit_publishes_once() {
        let mut s = Transcript::new(1);
        s.apply(1, snapshot(vec![], -1)).unwrap();
        s.apply(1,frame(json!({"type":"assistant-stream","frame":{"type":"start","revision":1,"attemptId":"a","startedAfterSeq":-1,"turn":1,"step":1}}))).unwrap();
        s.apply(1,frame(json!({"type":"assistant-stream","frame":{"type":"chunk","revision":2,"attemptId":"a","index":0,"time":1000,"chunk":{"type":"text-delta","index":0,"text":"hello"}}}))).unwrap();
        assert_eq!(s.partial().as_deref(), Some("hello"));
        s.apply(1,frame(json!({"type":"event","event":{"type":"assistant/message","seq":0,"time":1000,"surfaceOp":"append","data":{"turn":1,"step":1,"stream":[{"type":"text-chunks","time0":1000,"index":0,"dt":[],"texts":["hello"]}]}}}))).unwrap();
        assert!(s.rows.is_empty());
        s.apply(1,frame(json!({"type":"assistant-stream","frame":{"type":"end","revision":3,"attemptId":"a","index":1,"outcome":{"kind":"committed","seq":0,"eventType":"assistant/message"}}}))).unwrap();
        assert!(s.partial().is_none());
        assert_eq!(s.rows.len(), 1);
        assert_eq!(s.rows[0].text, "hello");
    }
    #[test]
    fn abandoned_prefix_and_dense_gap() {
        let mut s = Transcript::new(1);
        s.apply(1, snapshot(vec![], -1)).unwrap();
        s.apply(1,frame(json!({"type":"assistant-stream","frame":{"type":"start","revision":1,"attemptId":"a","startedAfterSeq":-1,"turn":1,"step":1}}))).unwrap();
        s.apply(1,frame(json!({"type":"assistant-stream","frame":{"type":"end","revision":2,"attemptId":"a","index":0,"outcome":{"kind":"abandoned","reason":"cancelled"}}}))).unwrap();
        assert!(s.partial().is_none());
        s.apply(1,frame(json!({"type":"assistant-stream","frame":{"type":"start","revision":3,"attemptId":"b","startedAfterSeq":-1,"turn":1,"step":2}}))).unwrap();
        assert_eq!(s.apply(1,frame(json!({"type":"assistant-stream","frame":{"type":"chunk","revision":4,"attemptId":"b","index":1,"time":1000,"chunk":{"type":"text-delta","index":0,"text":"bad"}}}))),Err(ReduceError::StreamGap));
    }
    #[test]
    fn unicode_limits_and_json_secret_fields() {
        assert!(bounded("你好world", 5).starts_with("你"));
        assert!(
            !safe_json(&json!({"token":"credential","ordinary":"visible"})).contains("credential")
        );
    }
    #[test]
    fn unknown_required_refuses_but_keeps_plaintext_notice() {
        let mut s = Transcript::new(1);
        assert_eq!(s.apply(1,snapshot(vec![json!({"type":"event","event":{"type":"future/required","seq":0,"time":0,"data":{"x":1}}})],0)),Err(ReduceError::UnknownRequiredEvent));
        assert_eq!(s.rows.len(), 1);
        assert!(!s.safe_to_send());
    }
    #[test]
    fn chat_visibility_golden_projects_confirmed_input_and_supplied_agent_output() {
        let fixture: SessionFollowFrame =
            serde_json::from_str(include_str!("../fixtures/chat-visibility.json")).unwrap();
        let mut transcript = Transcript::new(1);
        transcript.apply(1, fixture).unwrap();
        let printed = transcript
            .display_rows(false)
            .map(|row| format!("{}\n{}\n---\n", row.role, row.text))
            .collect::<String>();
        assert_eq!(printed, include_str!("../fixtures/chat-visibility.txt"));
        assert!(transcript.safe_to_send());
    }
    #[test]
    fn a_goal_message_is_a_trigger_notice_and_injects_stay_out_of_chat() {
        let goal = json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
            "data":{"role":"user","id":"g1","source":{"kind":"goal"},
                "content":[{"type":"text","text":"PUBLIC goal reminder"}]},"surfaceOp":"append"}});
        let inject = json!({"type":"event","event":{"type":"user/message","seq":1,"time":1001,
            "data":{"role":"user","id":"c1","source":{"kind":"runtime-context"},
                "content":[{"type":"text","text":"PUBLIC runtime context"}]},"surfaceOp":"append"}});
        let mut transcript = Transcript::new(1);
        transcript
            .apply(1, snapshot(vec![goal, inject], 1))
            .unwrap();
        let rows: Vec<_> = transcript.rows().iter().filter(|row| row.chat).collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].trigger.map(|trigger| trigger.title),
            Some("Continuing goal")
        );
        assert_eq!(rows[0].role, "Continuing goal");
        assert_eq!(rows[0].text, "PUBLIC goal reminder");
        assert!(
            !transcript
                .rows()
                .iter()
                .any(|row| row.chat && row.text.contains("runtime context"))
        );
    }
    #[test]
    fn alpha_golden_and_reconnect_prefix() {
        let fixture: SessionFollowFrame =
            serde_json::from_str(include_str!("../fixtures/alpha-session.json")).unwrap();
        let mut s = Transcript::new(1);
        s.apply(1, fixture).unwrap();
        let printed = s
            .display_rows(false)
            .map(|r| format!("{}\n{}\n---\n", r.role, r.text))
            .collect::<String>();
        assert_eq!(printed, include_str!("../fixtures/alpha-session.txt"));
        assert_eq!(s.partial().as_deref(), Some("partial reply"));
        s.apply(1,frame(json!({"type":"assistant-stream","frame":{"type":"chunk","revision":17,"attemptId":"active-attempt","index":3,"time":1700000000017i64,"chunk":{"type":"text-delta","index":0,"text":"!"}}}))).unwrap();
        assert_eq!(s.partial().as_deref(), Some("partial reply!"));
        s.apply(1,frame(json!({"type":"assistant-stream","frame":{"type":"end","revision":18,"attemptId":"active-attempt","index":4,"outcome":{"kind":"abandoned"}}}))).unwrap();
        assert!(s.partial().is_none());
        assert_eq!(s.rows.len(), 7);
    }
    #[test]
    fn workflow_events_fold_into_one_chat_row() {
        let mut s = Transcript::new(1);
        let records = vec![
            json!({"type":"event","event":{"type":"tool-workflow/run-start","seq":0,"time":1,
                "data":{"runId":"r1","name":"PUBLIC workflow"}}}),
            json!({"type":"event","event":{"type":"tool-workflow/agent-start","seq":1,"time":2,
                "data":{"runId":"r1","seq":1,"label":"PUBLIC research","phase":"research",
                    "childId":"PUBLIC-child"}}}),
            json!({"type":"event","event":{"type":"tool-workflow/agent-end","seq":2,"time":3,
                "data":{"runId":"r1","seq":1,"outcome":"completed"}}}),
            json!({"type":"event","event":{"type":"tool-workflow/run-end","seq":3,"time":4,
                "data":{"runId":"r1","stopReason":"completed"}}}),
        ];
        s.apply(1, snapshot(records, 4)).unwrap();
        let runs = s.workflow_runs();
        let run = runs.get(&0).expect("run keyed by start seq");
        assert_eq!(run.name, "PUBLIC workflow");
        assert_eq!(run.stop.as_deref(), Some("completed"));
        assert_eq!(run.members.len(), 1);
        assert_eq!(run.members[0].outcome.as_deref(), Some("completed"));
        assert_eq!(run.members[0].phase.as_deref(), Some("research"));
        assert!(
            s.rows()
                .iter()
                .any(|row| row.kind == "tool-workflow/run-start"
                    && row.chat
                    && row.text.contains("PUBLIC workflow"))
        );
    }
    #[test]
    fn actual_blank_session_policy_metadata_is_read_only_and_sendable() {
        let mut s = Transcript::new(1);
        let records=[("permission/preset",json!({"preset":"workspace-write"})),("sandbox/mode",json!({"mode":"workspace-write"})),("approval/policy",json!({"policy":"ask"}))].into_iter().enumerate().map(|(seq,(kind,data))|json!({"type":"event","event":{"type":kind,"seq":seq,"time":0,"data":data}})).collect();
        s.apply(1, snapshot(records, 2)).unwrap();
        assert!(s.safe_to_send());
        assert_eq!(s.rows.len(), 3);
        assert!(s.rows[2].text.contains("ask"));
    }
    #[test]
    fn pagination_uses_the_opening_cut_and_deduplicates() {
        let mut s = Transcript::new(1);
        s.apply(1, snapshot(vec![user(2, "tail")], 2)).unwrap();
        s.apply(1, frame(user(3, "live"))).unwrap();
        let page = serde_json::from_value(
            json!({"records":[user(0,"old"),user(1,"middle"),user(2,"tail")],"hasMore":false}),
        )
        .unwrap();
        s.page(1, page).unwrap();
        assert_eq!(s.opening_cursor(), 2);
        assert_eq!(s.before(), Some(0));
        assert_eq!(s.rows.len(), 4);
    }
    #[test]
    fn bounds_include_truncation_marker_and_all_transient_blocks() {
        let text = bounded(&"你".repeat(20_000), MAX_DISPLAY_BYTES);
        assert!(text.len() <= MAX_DISPLAY_BYTES);
        assert!(text.contains("truncated"));
        let mut blocks = BTreeMap::new();
        fold_chunk(
            &mut blocks,
            &json!({"type":"text-delta","index":0,"text":"x".repeat(MAX_DISPLAY_BYTES/2)}),
        )
        .unwrap();
        assert_eq!(
            fold_chunk(
                &mut blocks,
                &json!({"type":"text-delta","index":1,"text":"x".repeat(MAX_DISPLAY_BYTES/2)})
            ),
            Err(ReduceError::RetentionLimit)
        );
    }
    #[test]
    fn unknown_ignorable_surface_metadata_stays_opaque() {
        let mut s = Transcript::new(1);
        s.apply(1,snapshot(vec![json!({"type":"event","event":{"type":"external/info","seq":0,"time":0,"ignorable":true,"surfaceOp":{"future":"opaque"},"data":{"visible":true}}})],0)).unwrap();
        assert!(s.safe_to_send());
    }
    #[test]
    fn rebaseline_with_published_settlement_never_duplicates_prefix() {
        let mut fixture: Value =
            serde_json::from_str(include_str!("../fixtures/alpha-session.json")).unwrap();
        fixture["assistantStream"]["activeAttempt"]["turn"] = json!(1);
        fixture["assistantStream"]["activeAttempt"]["step"] = json!(1);
        let mut s = Transcript::new(1);
        s.apply(1, frame(fixture)).unwrap();
        assert!(s.partial().is_none());
        s.apply(1,frame(json!({"type":"assistant-stream","frame":{"type":"end","revision":17,"attemptId":"active-attempt","index":3,"outcome":{"kind":"committed","seq":3,"eventType":"assistant/message"}}}))).unwrap();
        assert_eq!(s.rows.len(), 7);
        assert!(s.partial().is_none());
    }
}

#[cfg(test)]
mod fold_tests {
    //! One row per call, as the reference draws it: a paired result folds onto its call and stays
    //! a Trajectory record. The pairing uses the wire's own identifiers, never proximity.
    use super::*;

    fn event(seq: u64, kind: &str, data: serde_json::Value) -> Value {
        serde_json::json!({"type":"event","event":{"type":kind,"seq":seq,"time":1000+seq,"data":data}})
    }

    fn frame(records: Vec<Value>) -> SessionFollowFrame {
        serde_json::from_value(serde_json::json!({
            "type": "snapshot", "records": records, "cursor": 2, "hasMore": false,
            "header": {"version": 4, "id": "PUBLIC-fold", "createdAt": 0, "isSeeded": false},
            "projections": {"asOfSeq": 2, "values": {}},
            "assistantStream": {"revision": 0}
        }))
        .unwrap()
    }

    fn call(seq: u64, turn: u64, step: u64) -> Value {
        event(
            seq,
            "tool/call",
            serde_json::json!({"callId": "PUBLIC-call", "turn": turn, "step": step,
                "name": "read", "arguments": "{\"file_path\":\"src/a.rs\"}"}),
        )
    }

    fn result(seq: u64, turn: u64, step: u64, error: bool) -> Value {
        event(
            seq,
            "tool/result",
            serde_json::json!({"turn": turn, "step": step, "message": {
                "role": "tool", "toolCallId": "PUBLIC-call",
                "source": {"kind": "tool", "callId": "PUBLIC-call"},
                "isError": error,
                "content": [{"type": "text", "text": "PUBLIC read body"}]
            }}),
        )
    }

    #[test]
    fn a_paired_result_folds_onto_its_call_and_trajectory_keeps_the_record() {
        let mut transcript = Transcript::new(0);
        transcript
            .apply(0, frame(vec![call(1, 1, 1), result(2, 1, 1, true)]))
            .unwrap();
        // Chat: one row, the call, carrying the result.
        let chat: Vec<_> = transcript.display_rows(false).collect();
        assert_eq!(chat.len(), 1);
        assert_eq!(chat[0].role, "Tool call");
        assert_eq!(chat[0].output.as_deref(), Some("PUBLIC read body"));
        assert!(chat[0].failed);
        // Trajectory: both records.
        assert_eq!(transcript.display_rows(true).count(), 2);
    }

    #[test]
    fn an_unpaired_result_keeps_its_own_row() {
        let mut transcript = Transcript::new(0);
        transcript
            .apply(0, frame(vec![result(1, 1, 1, false)]))
            .unwrap();
        let chat: Vec<_> = transcript.display_rows(false).collect();
        assert_eq!(chat.len(), 1, "a result with no call is still information");
        assert_eq!(chat[0].role, "Tool result");
    }

    #[test]
    fn a_result_for_another_step_does_not_fold() {
        let mut transcript = Transcript::new(0);
        transcript
            .apply(0, frame(vec![call(1, 1, 1), result(2, 1, 9, false)]))
            .unwrap();
        // The call stays alone in Chat, and the unpaired result keeps its own row there.
        let chat: Vec<_> = transcript.display_rows(false).collect();
        assert_eq!(chat.len(), 2);
        assert!(chat[0].output.is_none());
        assert!(chat.iter().any(|row| row.role == "Tool result"));
    }
}

#[cfg(test)]
mod settled_diff_tests {
    //! A settled mutation shows what the Host recorded as applied, and keeps the reference's rule
    //! that a settled `str_replace_editor` result has no card.
    use super::*;

    fn event(seq: u64, kind: &str, data: serde_json::Value) -> Value {
        serde_json::json!({"type":"event","event":{"type":kind,"seq":seq,"time":1000+seq,"data":data}})
    }
    fn frame(records: Vec<Value>) -> SessionFollowFrame {
        serde_json::from_value(serde_json::json!({
            "type": "snapshot", "records": records, "cursor": 2, "hasMore": false,
            "header": {"version": 4, "id": "PUBLIC-diff", "createdAt": 0, "isSeeded": false},
            "projections": {"asOfSeq": 2, "values": {}},
            "assistantStream": {"revision": 0}
        }))
        .unwrap()
    }
    fn call(name: &str, args: serde_json::Value) -> Value {
        event(
            1,
            "tool/call",
            serde_json::json!({"callId": "PUBLIC-edit", "turn": 1, "step": 1,
                "name": name, "arguments": args.to_string()}),
        )
    }
    fn result(meta: serde_json::Value) -> Value {
        event(
            2,
            "tool/result",
            serde_json::json!({"turn": 1, "step": 1, "meta": meta, "message": {
                "role": "tool", "toolCallId": "PUBLIC-edit",
                "source": {"kind": "tool", "callId": "PUBLIC-edit"},
                "isError": false,
                "content": [{"type": "text", "text": "PUBLIC recorded"}]
            }}),
        )
    }
    fn lines(transcript: &Transcript) -> Vec<String> {
        transcript
            .activity(1, activity::PathRoots::default())
            .and_then(|summary| summary.diff)
            .map(|diff| diff.lines.into_iter().map(|line| line.text).collect())
            .unwrap_or_default()
    }

    #[test]
    fn the_applied_diff_replaces_the_intended_one() {
        let mut transcript = Transcript::new(0);
        transcript
            .apply(0, frame(vec![
                call(
                    "functions.edit",
                    serde_json::json!({"file_path": "src/a.rs", "old_string": "old line", "new_string": "intended line"}),
                ),
                result(serde_json::json!({"diffs": [
                    {"path": "src/a.rs", "oldText": "old line", "newText": "applied line"}
                ]})),
            ]))
            .unwrap();
        let lines = lines(&transcript);
        assert!(lines.contains(&"applied line".to_owned()), "{lines:?}");
        assert!(!lines.contains(&"intended line".to_owned()), "{lines:?}");
        // The row's totals describe the applied change too: one line out, one line in — not the
        // arguments' own counts, which happen to match here by construction.
        let summary = transcript
            .activity(1, activity::PathRoots::default())
            .unwrap();
        assert_eq!(summary.edits, Some((1, 1)));
    }

    #[test]
    fn the_totals_follow_the_applied_rows_not_the_arguments() {
        // Two lines applied where the arguments described a one-for-one replacement: the row must
        // state `+2 -1`, matching the card it draws.
        let mut transcript = Transcript::new(0);
        transcript
            .apply(0, frame(vec![
                call(
                    "functions.edit",
                    serde_json::json!({"file_path": "src/a.rs", "old_string": "old", "new_string": "intended"}),
                ),
                result(serde_json::json!({"diffs": [
                    {"path": "src/a.rs", "oldText": "old", "newText": "first\nsecond"}
                ]})),
            ]))
            .unwrap();
        let summary = transcript
            .activity(1, activity::PathRoots::default())
            .unwrap();
        assert_eq!(summary.edits, Some((2, 1)));
    }

    #[test]
    fn a_recorded_no_op_write_keeps_its_whole_file_diff_and_an_edit_drops_theirs() {
        let mut transcript = Transcript::new(0);
        transcript
            .apply(
                0,
                frame(vec![
                    call(
                        "functions.write",
                        serde_json::json!({"file_path": "src/a.rs", "content": "whole"}),
                    ),
                    result(serde_json::json!({"diffs": []})),
                ]),
            )
            .unwrap();
        assert_eq!(
            lines(&transcript),
            vec!["src/a.rs".to_owned(), "whole".to_owned()]
        );

        let mut transcript = Transcript::new(0);
        transcript
            .apply(0, frame(vec![
                call("functions.edit", serde_json::json!({"file_path": "src/a.rs", "old_string": "a", "new_string": "b"})),
                result(serde_json::json!({"diffs": []})),
            ]))
            .unwrap();
        assert!(lines(&transcript).is_empty(), "{:?}", lines(&transcript));
    }

    #[test]
    fn a_settled_editor_has_no_card_and_a_running_one_does() {
        let args = serde_json::json!({"command": "str_replace", "path": "src/a.rs",
            "old_str": "old line", "new_str": "new line"});
        // Running: the call alone, which is how a live session first shows it.
        let mut transcript = Transcript::new(0);
        transcript
            .apply(0, frame(vec![call("str_replace_editor", args.clone())]))
            .unwrap();
        let running = lines(&transcript);
        assert!(running.contains(&"new line".to_owned()), "{running:?}");
        // Settled: the reference has no result view for this tool, so the card goes away.
        let mut transcript = Transcript::new(0);
        transcript
            .apply(
                0,
                frame(vec![
                    call("str_replace_editor", args),
                    result(serde_json::json!({})),
                ]),
            )
            .unwrap();
        assert!(lines(&transcript).is_empty(), "{:?}", lines(&transcript));
    }

    #[test]
    fn a_multi_file_result_keeps_the_intended_diff_rather_than_showing_one_file() {
        let mut transcript = Transcript::new(0);
        transcript
            .apply(0, frame(vec![
                call("functions.edit", serde_json::json!({"file_path": "src/a.rs", "old_string": "old", "new_string": "intended"})),
                result(serde_json::json!({"diffs": [
                    {"path": "src/a.rs", "oldText": "old", "newText": "applied a"},
                    {"path": "src/b.rs", "oldText": "old", "newText": "applied b"}
                ]})),
            ]))
            .unwrap();
        let lines = lines(&transcript);
        assert!(lines.contains(&"intended".to_owned()), "{lines:?}");
        assert!(!lines.contains(&"applied b".to_owned()), "{lines:?}");
    }
}
