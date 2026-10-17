//! Bounded, read-only projection of goal/todo/subagent/schedule records.
//!
//! The raw events stay authoritative in `reducer::Transcript`; this module keeps
//! a small, capped view for the More → Work panel. Every payload is parsed
//! defensively: a malformed or unrecognized shape contributes nothing instead of
//! crashing or being invented. Nothing here can send, cancel, retry or edit a
//! schedule, and an unknown event kind is untouched by this projection, so it
//! still appears in Trajectory through the existing reducer path.
use serde_json::Value;
use std::collections::BTreeMap;

/// Per-section retention. `200 = 4 × 50`, so one section can fill the whole
/// panel while the total still matches the existing 200-session scale.
pub const SECTION_CAP: usize = 50;
pub const TOTAL_CAP: usize = 200;
/// One rendered diagnostic line; wider than any planned field, far below 32 KiB.
const MAX_LINE_BYTES: usize = 512;
/// A goal objective is bounded harder so one goal stays a compact scannable row.
const MAX_OBJECTIVE_BYTES: usize = 90;
const MAX_LINES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    Goals,
    Todos,
    Subagents,
    Schedules,
}
impl Section {
    pub const ALL: [Section; 4] = [
        Section::Goals,
        Section::Todos,
        Section::Subagents,
        Section::Schedules,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Section::Goals => "Goals",
            Section::Todos => "Todos",
            Section::Subagents => "Subagents",
            Section::Schedules => "Schedules",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub lines: Vec<String>,
    /// Monotonic arrival order, so the panel can evict its globally oldest record.
    order: u64,
}

#[derive(Clone, Default)]
pub struct Store {
    sections: BTreeMap<Section, Vec<Entry>>,
    dropped: BTreeMap<Section, usize>,
    total: usize,
    next: u64,
    /// The newest `todo/write` list. Unlike [`Section::Todos`], which keeps every
    /// item ever written, this is the model's current standing plan: a later
    /// write replaces it, so an item the model dropped disappears.
    plan: Option<(usize, usize, Option<String>)>,
    /// The newest write's whole list as `(content, status)` in wire order.
    plan_items: Vec<(String, String)>,
}
impl Store {
    pub fn entries(&self, section: Section) -> &[Entry] {
        self.sections.get(&section).map_or(&[], Vec::as_slice)
    }
    pub fn dropped(&self, section: Section) -> usize {
        self.dropped.get(&section).copied().unwrap_or(0)
    }
    pub fn total(&self) -> usize {
        self.total
    }
    pub fn dropped_total(&self) -> usize {
        Section::ALL.iter().map(|s| self.dropped(*s)).sum()
    }
    pub fn is_empty(&self) -> bool {
        self.total == 0
    }
    /// The model's current standing plan as `(completed, total, next item)`.
    /// `None` when no `todo/write` has been retained, so the composer shows no
    /// chip rather than an empty one.
    pub fn plan(&self) -> Option<(usize, usize, Option<&str>)> {
        self.plan
            .as_ref()
            .map(|(done, total, next)| (*done, *total, next.as_deref()))
    }
    /// The newest write's items, for the composer's expandable todo dock.
    pub fn plan_items(&self) -> &[(String, String)] {
        &self.plan_items
    }
    /// Fold one retained wire event. Unknown or malformed payloads are ignored.
    pub fn ingest(&mut self, kind: &str, data: &Value) {
        match kind {
            "goal/change" => self.goal(data),
            "todo/write" => self.todos(data),
            "subagent/catalog" => self.catalog(data),
            "subagent/descriptor" => self.descriptor(data),
            "schedule/change" => self.schedule(data),
            _ => {}
        }
    }
    /// Keep the newest `SECTION_CAP` per section and `TOTAL_CAP` overall; every
    /// eviction is counted so the panel can state the omission instead of hiding it.
    fn push(&mut self, section: Section, key: String, lines: Vec<String>) {
        if lines.is_empty() || key.is_empty() {
            return;
        }
        {
            let entries = self.sections.entry(section).or_default();
            if let Some(existing) = entries.iter_mut().find(|entry| entry.key == key) {
                existing.lines = lines;
                return;
            }
            let order = self.next;
            self.next = self.next.saturating_add(1);
            entries.push(Entry { key, lines, order });
        }
        self.total += 1;
        // Two independent bounds, each driving at most one eviction: first the
        // section the record belongs to, then the globally oldest panel record.
        if self
            .sections
            .get(&section)
            .is_some_and(|e| e.len() > SECTION_CAP)
        {
            self.evict_oldest_of(section);
        }
        if self.total > TOTAL_CAP {
            self.evict_oldest();
        }
    }
    fn evict_oldest_of(&mut self, section: Section) {
        let entries = self.sections.entry(section).or_default();
        if entries.is_empty() {
            return;
        }
        entries.remove(0);
        *self.dropped.entry(section).or_default() += 1;
        self.total = self.total.saturating_sub(1);
    }
    /// Drop the record that arrived first anywhere in the panel.
    fn evict_oldest(&mut self) {
        let oldest = self
            .sections
            .iter()
            .filter_map(|(section, entries)| entries.first().map(|entry| (*section, entry.order)))
            .min_by_key(|(_, order)| *order);
        if let Some((section, order)) = oldest {
            let entries = self.sections.entry(section).or_default();
            if entries.first().is_some_and(|entry| entry.order == order) {
                entries.remove(0);
                *self.dropped.entry(section).or_default() += 1;
                self.total = self.total.saturating_sub(1);
            }
        }
    }
    fn goal(&mut self, data: &Value) {
        let Some(operation) = data.get("operation").and_then(Value::as_str) else {
            return;
        };
        if operation == "clear" {
            if let Some(id) = data.get("cleared").and_then(data_id) {
                self.remove(Section::Goals, &id);
            }
            return;
        }
        if !matches!(
            operation,
            "create" | "edit" | "pause" | "resume" | "complete" | "block"
        ) {
            return;
        }
        let Some(goal) = data.get("goal") else {
            return;
        };
        let Some(id) = data_id(goal) else { return };
        let Some(phase) = goal.get("phase").and_then(Value::as_str) else {
            return;
        };
        if !matches!(phase, "active" | "paused" | "blocked" | "complete") {
            return;
        }
        // An objective can be a paragraph; keep one compact scannable row.
        let objective = goal
            .get("objective")
            .and_then(Value::as_str)
            .map(|objective| bounded(objective, MAX_OBJECTIVE_BYTES));
        let mut lines = Lines::new();
        let rounds = goal
            .get("maxGoalRounds")
            .and_then(Value::as_u64)
            .map(|max| {
                let started = data
                    .get("roundsStarted")
                    .and_then(Value::as_u64)
                    .map_or_else(String::new, |n| format!(" · {n} started"));
                format!("rounds: {max} max{started}")
            });
        // Header: objective and phase on one compact row, then rounds.
        if let Some(objective) = objective.filter(|objective| !objective.is_empty()) {
            lines.push(format!("{objective} · {phase}"));
        } else {
            lines.push(format!("phase: {phase}"));
        }
        if let Some(rounds) = &rounds {
            lines.push(rounds.clone());
        }
        if let Some(reason) = goal.get("blockedReason") {
            let code = reason.get("code").and_then(Value::as_str).unwrap_or("");
            let message = reason.get("message").and_then(Value::as_str).unwrap_or("");
            let text = [code, message]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(": ");
            if !text.is_empty() {
                lines.push(format!("blocked: {text}"));
            }
        }
        lines.push(format!("last operation: {operation}"));
        self.push(Section::Goals, id, lines.take());
    }
    fn todos(&mut self, data: &Value) {
        let Some(todos) = data.get("todos").and_then(Value::as_array) else {
            return;
        };
        let mut folded = Vec::new();
        for todo in todos {
            let Some(object) = todo.as_object() else {
                continue;
            };
            let content = object.get("content").and_then(Value::as_str).unwrap_or("");
            if content.is_empty() {
                continue;
            }
            let status = match object.get("status").and_then(Value::as_str) {
                Some("pending") => "pending",
                Some("in_progress") => "in progress",
                Some("completed") => "completed",
                _ => continue,
            };
            folded.push((bounded(content, MAX_LINE_BYTES), status));
        }
        if folded.is_empty() {
            self.plan = None;
            self.plan_items.clear();
            return;
        }
        self.plan_items = folded
            .iter()
            .map(|(content, status)| (content.clone(), (*status).to_owned()))
            .collect();
        // The plan is the whole list as written, so the newest write replaces it.
        let completed = folded
            .iter()
            .filter(|(_, status)| *status == "completed")
            .count();
        let next = folded
            .iter()
            .find(|(_, status)| *status == "in progress")
            .map(|(content, _)| content.clone());
        self.plan = Some((completed, folded.len(), next));
        for (content, status) in folded {
            // A todo has no wire identity, so its own text is the stable key:
            // an unchanged item keeps one row across writes and status changes.
            self.push(
                Section::Todos,
                content.clone(),
                vec![format!("{content} · {status}")],
            );
        }
    }
    fn catalog(&mut self, data: &Value) {
        let version = data.get("version").and_then(Value::as_u64);
        if !matches!(version, Some(0) | Some(1)) {
            return;
        }
        let Some(child) = data.get("childId").and_then(Value::as_str) else {
            return;
        };
        let Some(mode) = data.get("mode").and_then(Value::as_str) else {
            return;
        };
        if !matches!(mode, "continuable" | "one-shot" | "unknown") {
            return;
        }
        let label = data.get("label").and_then(Value::as_str);
        // A continuable catalog fact must carry a label; missing it is malformed.
        if mode == "continuable" && label.is_none() {
            return;
        }
        // Several catalog facts may name one child; fold instead of appending duplicates.
        let mut lines = self
            .section_lines(Section::Subagents, child)
            .map(Lines::from)
            .unwrap_or_default();
        if let Some(label) = label {
            lines.push(format!("label: {label}"));
        }
        lines.push(format!("mode: {mode}"));
        self.push(Section::Subagents, child.to_owned(), lines.take());
    }
    fn descriptor(&mut self, data: &Value) {
        if data.get("version").and_then(Value::as_u64) != Some(3) {
            return;
        }
        let Some(provider) = data.get("provider").and_then(Value::as_str) else {
            return;
        };
        let Some(mode) = data.get("mode").and_then(Value::as_str) else {
            return;
        };
        let label = data.get("label").and_then(Value::as_str);
        let mut lines = Lines::new();
        match mode {
            "one-shot" => {
                lines.push(format!("provider: {provider}"));
                lines.push(format!("mode: {mode}"));
            }
            "continuable" => match label {
                Some(label) => {
                    lines.push(format!("label: {label}"));
                    lines.push(format!("mode: {mode}"));
                }
                // A continuable descriptor must carry its label; missing it is malformed.
                None => return,
            },
            _ => return,
        }
        // A descriptor carries no child id on the wire, so it keeps its own row
        // keyed by its label (or provider for a one-shot child). Inventing a
        // child identity here would be worse than showing the descriptor as such.
        let key = label
            .filter(|label| !label.is_empty())
            .unwrap_or(provider)
            .to_owned();
        if let Some(agent_provider) = data.get("agentProvider").and_then(Value::as_str) {
            let agent_model = data.get("agentModel").and_then(Value::as_str).unwrap_or("");
            lines.push(format!("agent: {agent_provider} / {agent_model}"));
        }
        if let Some(persona) = data.get("persona").and_then(Value::as_str) {
            lines.push(format!("persona: {persona}"));
        }
        self.push(Section::Subagents, key, lines.take());
    }
    fn schedule(&mut self, data: &Value) {
        if data.get("version").and_then(Value::as_u64) != Some(1) {
            return;
        }
        let Some(operation) = data.get("operation").and_then(Value::as_str) else {
            return;
        };
        if operation == "delete" {
            if let Some(id) = data.get("id").and_then(Value::as_str) {
                self.remove(Section::Schedules, id);
            }
            return;
        }
        if operation == "create" {
            let Some(record) = data.get("schedule") else {
                return;
            };
            let Some(id) = record.get("id").and_then(Value::as_str) else {
                return;
            };
            if id.is_empty() {
                return;
            }
            let mut lines = Lines::new();
            match record.get("kind").and_then(Value::as_str) {
                Some("at") => {
                    lines.push("type: at".into());
                    if let Some(at) = record.get("scheduledAt").and_then(Value::as_str) {
                        lines.push(format!("at: {at}"));
                    }
                }
                Some("after") => {
                    lines.push("type: after".into());
                    if let Some(seconds) = record.get("afterSeconds").and_then(Value::as_u64) {
                        lines.push(format!("after: {seconds}s"));
                    }
                }
                Some("every") => {
                    lines.push("type: every".into());
                    if let Some(seconds) = record.get("everySeconds").and_then(Value::as_u64) {
                        lines.push(format!("every: {seconds}s"));
                    }
                }
                _ => return,
            }
            if let Some(prompt) = record.get("prompt").and_then(Value::as_str) {
                lines.push(format!("prompt: {prompt}"));
            }
            self.push(Section::Schedules, id.to_owned(), lines.take());
            return;
        }
        if operation != "dispatch" {
            return;
        }
        let Some(id) = data.get("id").and_then(Value::as_str) else {
            return;
        };
        if !self
            .sections
            .get(&Section::Schedules)
            .is_some_and(|entries| entries.iter().any(|entry| entry.key == id))
        {
            return;
        }
        let mut lines = self
            .section_lines(Section::Schedules, id)
            .map(Lines::from)
            .unwrap_or_default();
        if let Some(accepted) = data.get("acceptedAt").and_then(Value::as_str) {
            lines.push(format!("last dispatched: {accepted}"));
        } else {
            lines.push("last dispatched".into());
        }
        self.push(Section::Schedules, id.to_owned(), lines.take());
    }
    fn remove(&mut self, section: Section, key: &str) {
        let Some(entries) = self.sections.get_mut(&section) else {
            return;
        };
        if let Some(index) = entries.iter().position(|entry| entry.key == key) {
            entries.remove(index);
            self.total = self.total.saturating_sub(1);
        }
    }
    fn section_lines(&self, section: Section, key: &str) -> Option<Vec<String>> {
        self.sections
            .get(&section)?
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.lines.clone())
    }
}
#[derive(Default)]
struct Lines(Vec<String>);
impl Lines {
    fn new() -> Self {
        Self(Vec::new())
    }
    fn from(lines: Vec<String>) -> Self {
        Self(lines)
    }
    fn push(&mut self, line: String) {
        if self.0.len() >= MAX_LINES {
            return;
        }
        let line = bounded(&line, MAX_LINE_BYTES);
        if !line.is_empty() {
            self.0.push(line);
        }
    }
    fn take(self) -> Vec<String> {
        self.0
    }
}
fn data_id(value: &Value) -> Option<String> {
    let id = value.get("id").and_then(Value::as_str)?;
    (!id.is_empty()).then(|| id.to_owned())
}
fn bounded(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.into();
    }
    let mut end = max.saturating_sub(1);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}
