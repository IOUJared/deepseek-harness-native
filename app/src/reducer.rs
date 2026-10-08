//! Alpha wire fold. Durable cursor and transient dense indices are independent.
use dsh_native_transport::dto::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[path = "activity.rs"]
pub mod activity;
#[path = "transcript_presentation.rs"]
pub mod presentation;

const MAX_RECORDS: usize = 4096;
const MAX_RETAINED_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_DISPLAY_BYTES: usize = 32 * 1024;
#[derive(Clone)]
pub struct DisplayRow {
    pub key: u64,
    pub role: String,
    pub text: String,
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
    blocks: BTreeMap<u64, String>,
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
    retained_bytes: usize,
    rows: Vec<DisplayRow>,
    attempt: Option<Attempt>,
    retained: Option<(String, u64, u64)>,
    error: Option<ReduceError>,
    gap: bool,
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
            retained_bytes: 0,
            rows: vec![],
            attempt: None,
            retained: None,
            error: None,
            gap: false,
        }
    }
    pub fn rows(&self) -> &[DisplayRow] {
        &self.rows
    }
    /// Project retained rows; cancellation, malformed/future records and Send gates are unchanged.
    pub fn display_rows(&self, records: bool) -> impl Iterator<Item = &DisplayRow> {
        self.rows.iter().filter(move |row| {
            !self.records.get(&row.key).is_some_and(|event| presentation::ui_control_event(&event.event_type))
                && (records
                || (presentation::conversation_role(&row.role)
                    && !(row.role == "agent/inbox/spliced"
                        && self
                            .records
                            .get(&row.key)
                            .is_some_and(|event| presentation::routine_inbox(&event.data)))))
        })
    }
    /// Display formatting eligibility comes from exact retained provenance, never a role label.
    pub fn assistant_record(&self, key: u64) -> bool {
        self.records.get(&key).is_some_and(|event|matches!(event.event_type.as_str(),"assistant/message"|"assistant/attempt"))
    }
    pub fn inbox_canceled(&self, key: u64) -> bool {
        self.records.get(&key).is_some_and(|event| {
            event.event_type == "agent/inbox/spliced" && event.data["outcome"] == "canceled"
        })
    }
    /// Derive a compact tool label from retained metadata, without changing the fold.
    pub fn activity(&self, key: u64) -> Option<activity::Summary> {
        activity::summarize(
            self.records.get(&key)?,
            self.records.range(..key).rev().map(|(_, event)| event),
        )
    }
    pub fn before(&self) -> Option<u64> {
        self.before
    }
    pub fn opening_cursor(&self) -> i64 {
        self.opening
    }
    pub fn has_more(&self) -> bool {
        self.more
    }
    pub fn safe_to_send(&self) -> bool {
        self.error.is_none() && self.cursor >= -1 && !self.gap
    }
    pub fn partial(&self) -> Option<String> {
        self.attempt
            .as_ref()
            .filter(|a| {
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
            .map(|a| a.blocks.values().cloned().collect::<Vec<_>>().join("\n"))
            .filter(|s| !s.is_empty())
    }
    pub fn apply(&mut self, generation: u64, frame: SessionFollowFrame) -> Result<(), ReduceError> {
        if generation != self.generation {
            return Ok(());
        }
        if !matches!(&frame, SessionFollowFrame::Snapshot { .. }) {
            if let Some(error) = self.error {
                return Err(error);
            }
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
                ..
            } => {
                self.records.clear();
                self.retained_bytes = 0;
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
                if let Some(attempt) = &mut self.attempt {
                    if matches!(
                        event.event_type.as_str(),
                        "assistant/message" | "assistant/attempt"
                    ) && event.data.get("turn").and_then(Value::as_u64) == Some(attempt.turn)
                        && event.data.get("step").and_then(Value::as_u64) == Some(attempt.step)
                    {
                        attempt.pending.insert(event.seq);
                    }
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
        let mut unknown = false;
        for event in self.records.values() {
            if self
                .attempt
                .as_ref()
                .is_some_and(|a| a.pending.contains(&event.seq))
            {
                continue;
            }
            let kind = event.event_type.as_str();
            let text = match kind {
                "user/message" => content(&event.data["content"]),
                "system/message" | "developer/message" | "tool/result" => {
                    content(&event.data["message"]["content"])
                }
                "assistant/message" | "assistant/attempt" => stream_text(&event.data["stream"])?,
                "tool/call" => format!(
                    "{}\n{}",
                    event.data["name"].as_str().unwrap_or("Tool"),
                    bounded(
                        event.data["arguments"].as_str().unwrap_or(""),
                        MAX_DISPLAY_BYTES
                    )
                ),
                kind if crate::known::LOG_EVENTS.contains(&kind) => safe_json(&event.data),
                "turn/start" | "turn/end" | "step/start" | "step/end" | "request/header"
                | "request/context" | "session/end-seed" | "image/offload" => continue,
                _ => {
                    if event.ignorable != Some(true) {
                        unknown = true;
                    }
                    safe_json(&event.data)
                }
            };
            let role = match kind {
                "user/message" => "You",
                "assistant/message" => "Assistant",
                "assistant/attempt" => "Interrupted assistant attempt",
                "tool/result" => "Tool result",
                "tool/call" => "Tool call",
                "system/message" => "System",
                "developer/message" => "Developer",
                _ => kind,
            };
            let row = DisplayRow {
                key: event.seq,
                role: bounded(role, 128),
                text: bounded(&text, MAX_DISPLAY_BYTES),
                time: event.time,
                surface: matches!(
                    kind,
                    "system/message"
                        | "developer/message"
                        | "user/message"
                        | "assistant/message"
                        | "tool/result"
                ),
            };
            if !row.surface {
                rows.push(row);
                continue;
            }
            match &event.surface_op {
                Some(value) if value.is_object() => {
                    let op = value["op"].as_str().ok_or(ReduceError::InvalidSurface)?;
                    let start_seq = value["startSeq"]
                        .as_u64()
                        .ok_or(ReduceError::InvalidSurface)?;
                    let end_seq = value["endSeq"]
                        .as_u64()
                        .ok_or(ReduceError::InvalidSurface)?;
                    if op != "replace" || start_seq > end_seq {
                        return Err(ReduceError::InvalidSurface);
                    }
                    let start = rows
                        .iter()
                        .position(|r| r.key >= start_seq && r.key <= end_seq)
                        .unwrap_or(rows.len());
                    if let Some(cited) = &event.source_event_seqs {
                        if rows.iter().any(|r| {
                            r.surface
                                && r.key >= start_seq
                                && r.key <= end_seq
                                && !cited.as_array().is_some_and(|items| {
                                    items.iter().any(|v| v.as_u64() == Some(r.key))
                                })
                        }) {
                            return Err(ReduceError::InvalidSurface);
                        }
                    }
                    rows.retain(|r| r.key < start_seq || r.key > end_seq);
                    rows.insert(start.min(rows.len()), row);
                }
                Some(value) if value.as_str() != Some("append") => {
                    return Err(ReduceError::InvalidSurface);
                }
                _ => rows.push(row),
            }
        }
        self.rows = rows;
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
fn file_summary(attachment: &Value) -> String {
    let Some(name) = attachment["name"].as_str() else {
        return "[File attachment: metadata unavailable]".into();
    };
    let Some(bytes) = attachment["bytes"].as_u64() else {
        return "[File attachment: metadata unavailable]".into();
    };
    let name: String = name
        .chars()
        .take(80)
        .map(|c| if c.is_control() { '�' } else { c })
        .collect();
    format!("File · {name} · {bytes} bytes [preview unavailable]")
}
fn content(value: &Value) -> String {
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
fn fold_chunk(blocks: &mut BTreeMap<u64, String>, chunk: &Value) -> Result<(), ReduceError> {
    let index = chunk["index"].as_u64().unwrap_or(0);
    match chunk["type"].as_str() {
        Some("text-delta" | "reasoning-delta") => {
            let value = chunk["text"].as_str().ok_or(ReduceError::StreamGap)?;
            let block = blocks.entry(index).or_default();
            if block.len() + value.len() > MAX_DISPLAY_BYTES {
                return Err(ReduceError::RetentionLimit);
            }
            block.push_str(value);
        }
        Some("block-end") => {
            blocks.insert(index, content(&Value::Array(vec![chunk["block"].clone()])));
        }
        Some("tool-call-delta") => {
            let value = chunk["argumentsDelta"]
                .as_str()
                .ok_or(ReduceError::StreamGap)?;
            let block = blocks
                .entry(index)
                .or_insert_with(|| format!("Tool {}: ", chunk["name"].as_str().unwrap_or("call")));
            if block.len() + value.len() > MAX_DISPLAY_BYTES {
                return Err(ReduceError::RetentionLimit);
            }
            block.push_str(value);
        }
        Some("block-start" | "usage" | "finish") => {}
        _ => return Err(ReduceError::StreamGap),
    }
    if blocks.len() > 256
        || blocks.values().map(String::len).sum::<usize>() + blocks.len().saturating_sub(1)
            > MAX_DISPLAY_BYTES
    {
        return Err(ReduceError::RetentionLimit);
    }
    Ok(())
}
fn stream_text(stream: &Value) -> Result<String, ReduceError> {
    let mut blocks = BTreeMap::new();
    for chunk in compact_chunks(stream)? {
        fold_chunk(&mut blocks, &chunk)?;
    }
    Ok(blocks.values().cloned().collect::<Vec<_>>().join("\n"))
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
        json!({"type":"event","event":{"type":"user/message","seq":seq,"time":1000,"data":{"role":"user","id":"m1","source":{"kind":"human"},"content":[{"type":"text","text":text}]},"surfaceOp":"append"}})
    }
    #[test]
    fn snapshot_append_replace_and_source_citations() {
        let mut s = Transcript::new(1);
        s.apply(1, snapshot(vec![user(0, "你好 🦀")], 0)).unwrap();
        s.apply(1, frame(user(1, "next"))).unwrap();
        let replacement = json!({"type":"event","event":{"type":"user/message","seq":2,"time":1002,"data":{"content":[{"type":"text","text":"summary"}]},"surfaceOp":{"op":"replace","startSeq":0,"endSeq":1},"sourceEventSeqs":[0,1]}});
        s.apply(1, frame(replacement)).unwrap();
        assert_eq!(s.rows.len(), 1);
        assert_eq!(s.rows[0].text, "summary");
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
    fn alpha_golden_and_reconnect_prefix() {
        let fixture: SessionFollowFrame =
            serde_json::from_str(include_str!("../fixtures/alpha-session.json")).unwrap();
        let mut s = Transcript::new(1);
        s.apply(1, fixture).unwrap();
        let printed = s
            .rows()
            .iter()
            .map(|r| format!("{}\n{}\n---\n", r.role, r.text))
            .collect::<String>();
        assert_eq!(printed, include_str!("../fixtures/alpha-session.txt"));
        assert_eq!(s.partial().as_deref(), Some("partial reply"));
        s.apply(1,frame(json!({"type":"assistant-stream","frame":{"type":"chunk","revision":17,"attemptId":"active-attempt","index":3,"time":1700000000017i64,"chunk":{"type":"text-delta","index":0,"text":"!"}}}))).unwrap();
        assert_eq!(s.partial().as_deref(), Some("partial reply!"));
        s.apply(1,frame(json!({"type":"assistant-stream","frame":{"type":"end","revision":18,"attemptId":"active-attempt","index":4,"outcome":{"kind":"abandoned"}}}))).unwrap();
        assert!(s.partial().is_none());
        assert_eq!(s.rows.len(), 6);
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
        assert_eq!(s.rows.len(), 6);
        assert!(s.partial().is_none());
    }
}
