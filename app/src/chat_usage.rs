//! Read-only alpha usage display; context projections are approximate, not billing or Send gates.
use dsh_native_transport::dto::SessionWireEvent;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

const MAX_COUNT: u64 = 9_007_199_254_740_991;
const MAX_RECORDS: usize = 4096;
/// The plain label when neither a context reading nor a whole-log usage is available.
pub const UNAVAILABLE: &str = "Usage unavailable";

#[derive(Clone, PartialEq, Eq)]
struct Route<'a> {
    provider: &'a str,
    model: &'a str,
}
fn route(value: &Value) -> Option<Route<'_>> {
    let object = value.as_object()?;
    let provider = object.get("provider")?.as_str()?;
    let model = object.get("model")?.as_str()?;
    if provider.is_empty() || model.is_empty() || provider.len() > 4096 || model.len() > 4096 {
        return None;
    }
    Some(Route { provider, model })
}
fn count(value: &Value) -> Option<u64> {
    value.as_u64().filter(|value| *value <= MAX_COUNT)
}
fn field(object: &Map<String, Value>, key: &str) -> Result<Option<u64>, ()> {
    object
        .get(key)
        .map(count)
        .map_or(Ok(None), |value| value.map(Some).ok_or(()))
}
fn sum(values: &[u64]) -> Option<u64> {
    values
        .iter()
        .try_fold(0_u64, |total, value| total.checked_add(*value))
        .filter(|total| *total <= MAX_COUNT)
}
fn compact(value: u64) -> String {
    if value >= 1000 && value.is_multiple_of(1000) {
        format!("{}k", value / 1000)
    } else {
        value.to_string()
    }
}
struct Usage {
    prompt: Option<u64>,
    output: Option<u64>,
    total: u64,
}
fn usage(value: &Value) -> Option<Usage> {
    let object = value.as_object()?;
    let input = field(object, "inputTokens").ok()?;
    let output = field(object, "outputTokens").ok()?;
    let read = field(object, "cacheReadTokens").ok()?;
    let write = field(object, "cacheWriteTokens").ok()?;
    let reasoning = field(object, "reasoningTokens").ok()?;
    let reported = field(object, "totalTokens").ok()?;
    if let Some(reasoning) = reasoning
        && reasoning > output?
    {
        return None;
    }
    let known_prompt = sum(&[input.unwrap_or(0), read.unwrap_or(0), write.unwrap_or(0)])?;
    let prompt = match (input, read, write) {
        (Some(input), Some(read), Some(write)) => Some(sum(&[input, read, write])?),
        _ => None,
    };
    let total = match reported {
        Some(total) => {
            let known_total = sum(&[known_prompt, output.unwrap_or(0)])?;
            if total < known_total {
                return None;
            }
            if let (Some(prompt), Some(output)) = (prompt, output)
                && sum(&[prompt, output])? != total
            {
                return None;
            }
            total
        }
        None => sum(&[prompt?, output?])?,
    };
    Some(Usage {
        prompt,
        output,
        total,
    })
}
fn settlement_usage(event: &SessionWireEvent) -> Option<Usage> {
    if event.event_type == "assistant/message"
        && let Some(value) = event.data.get("usage")
    {
        return usage(value);
    }
    let stream = event.data.get("stream")?.as_array()?;
    if stream.len() > MAX_RECORDS {
        return None;
    }
    // Usage is never packed into text/reasoning/tool runs. Last sample replaces earlier samples.
    stream
        .iter()
        .rev()
        .find_map(|record| {
            (record.get("type")?.as_str()? == "chunk")
                .then(|| record.get("chunk"))?
                .filter(|chunk| chunk.get("type").and_then(Value::as_str) == Some("usage"))
        })
        .and_then(|chunk| chunk.get("usage"))
        .and_then(usage)
}
fn selection_pending(projections: &BTreeMap<String, Value>) -> bool {
    let Some(selection) = projections.get("modelSelection") else {
        return false;
    };
    let Some(object) = selection.as_object() else {
        return true;
    };
    let next = object.get("next").and_then(route);
    let used = object.get("lastUsed").and_then(route);
    match (next, used) {
        (Some(next), Some(used)) => next != used,
        (None, None)
            if object.get("next") == Some(&Value::Null)
                && object.get("lastUsed") == Some(&Value::Null) =>
        {
            false
        }
        _ => true,
    }
}
/// Occupancy and the window it is measured against, from valid projections only.
struct Occupancy {
    used: u64,
    capacity: u64,
}

fn occupancy(projections: &BTreeMap<String, Value>) -> Option<Occupancy> {
    let object = projections.get("contextPressure")?.as_object()?;
    let projected = field(object, "projectedTokens").ok()?;
    let pressure = field(object, "pressureTokens").ok()?;
    let capacity = field(object, "contextWindow").ok()??;
    if capacity == 0 {
        return None;
    }
    let used = projected.or(pressure)?;
    Some(Occupancy { used, capacity })
}

/// The context-pressure tier. `Tight` is keyed to the mounted compaction
/// threshold (`thresholdRatio: 0.8` of the window): the host elides and
/// summarizes from there, so the user is told before it happens. No soft tier
/// is assumed — the alpha profile runs one threshold, not the paper's two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Normal,
    Tight,
    Critical,
}

impl Severity {
    /// `None` for occupancy the window cannot plausibly contain: a projection
    /// reporting orders of magnitude past its own window is malformed, and a
    /// percentage or tier derived from it would be confident nonsense.
    fn of(occupancy: &Occupancy) -> Option<Self> {
        let overrun = u128::from(occupancy.used) * 10 > u128::from(occupancy.capacity) * 100;
        if overrun {
            return None;
        }
        Some(if occupancy.used >= occupancy.capacity {
            Self::Critical
        } else if u128::from(occupancy.used) * 10 > u128::from(occupancy.capacity) * 8 {
            Self::Tight
        } else {
            Self::Normal
        })
    }
}

/// The composer's context reading: its one-line label, the tier that colours it,
/// and bounded disclosure entries. Presentation only — never a Send gate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextState {
    pub line: String,
    pub severity: Severity,
    pub detail: Vec<String>,
    /// The reading's own numbers, which the breakdown panel states beside its rows
    /// (`ContextMeter.tsx:168-176`). Zero when there is no reading.
    pub used: u64,
    pub capacity: u64,
}

const PRUNE_NOTE: &str = "Context management replaces tool output over 8k characters with a stub \
before it summarizes older history near 80% of the window. The paper's staged order — elide \
first, summarize second — is what controls peak context.";
const OVERFLOW_NOTE: &str = "At the window limit. Unmanaged contexts in the study overflowed and \
ended early on most tasks, so start a new conversation or compact rather than continuing here.";
const TOOLS_NOTE: &str =
    "Predefined file tools stay available; a bash-only interface is not offered.";
const MAX_DETAIL_LINES: usize = 4;

impl ContextState {
    /// Mark a reading the caller must treat as absent: no tier, no disclosure.
    fn unavailable(line: String) -> Self {
        Self {
            line,
            severity: Severity::Normal,
            detail: Vec::new(),
            used: 0,
            capacity: 0,
        }
    }
}

/// One owner for the projection, pending-selection, route-change and
/// projection-current gates, so the plain label and the tiered state can never
/// describe different things.
enum Reading {
    Context(Occupancy),
    Last(Usage),
    Unavailable,
}

/// Both callers own their own `records` iterator; this is the shared fold.
fn reading<'a>(
    projections: &BTreeMap<String, Value>,
    projection_current: bool,
    records: impl Iterator<Item = &'a SessionWireEvent>,
) -> Reading {
    let mut previous = None;
    let mut context_route = None;
    let mut last = None;
    let mut changed_route = false;
    for (index, event) in records.enumerate() {
        if index >= MAX_RECORDS
            || event.seq > MAX_COUNT
            || previous.is_some_and(|seq: u64| seq.checked_add(1) != Some(event.seq))
        {
            return Reading::Unavailable;
        }
        previous = Some(event.seq);
        match event.event_type.as_str() {
            "request/context" => {
                let Some(next) = route(&event.data) else {
                    return Reading::Unavailable;
                };
                if context_route.as_ref().is_some_and(|old| old != &next) {
                    last = None;
                    changed_route = true;
                }
                context_route = Some(next);
            }
            "model/selection" => {
                let Some(next) = route(&event.data) else {
                    return Reading::Unavailable;
                };
                if context_route.as_ref() != Some(&next) {
                    last = None;
                    changed_route = true;
                }
            }
            "assistant/message" | "assistant/attempt" => {
                let source = event
                    .data
                    .get("message")
                    .and_then(|message| message.get("source"));
                if let Some(source) = source {
                    let Some(source_route) = route(source) else {
                        return Reading::Unavailable;
                    };
                    if context_route
                        .as_ref()
                        .is_some_and(|current| current != &source_route)
                    {
                        return Reading::Unavailable;
                    }
                }
                last = settlement_usage(event);
                // A fresh settled request supplies pressure for the new route.
                changed_route = last.is_none();
            }
            _ => {}
        }
    }
    let pending = selection_pending(projections);
    if projection_current
        && !pending
        && !changed_route
        && let Some(occupancy) = occupancy(projections)
    {
        return Reading::Context(occupancy);
    }
    if pending || changed_route {
        return Reading::Unavailable;
    }
    match last {
        Some(usage) => Reading::Last(usage),
        None => Reading::Unavailable,
    }
}

/// Describe current approximate Host context or the last retained authoritative request.
/// The caller owns session/epoch/watermark validity; stale projections cannot supply capacity.
/// Retained records must be a contiguous chronological window, not a whole-log usage aggregate.
/// The composer renders [`state`], which wraps this line; only the tests that
/// pin the exact wording call it directly.
#[cfg(test)]
pub fn describe<'a>(
    projections: &BTreeMap<String, Value>,
    projection_current: bool,
    records: impl Iterator<Item = &'a SessionWireEvent>,
) -> String {
    match reading(projections, projection_current, records) {
        Reading::Context(Occupancy { used, capacity }) => {
            format!("Context ~{} / {}", compact(used), compact(capacity))
        }
        Reading::Last(usage) => last_request(usage),
        Reading::Unavailable => UNAVAILABLE.into(),
    }
}

/// The composer's context reading: [`describe`]'s exact line plus the pressure
/// tier that colours it and bounded disclosure entries. A last-request or
/// unavailable line keeps [`Severity::Normal`] and stays undisclosed, so the app
/// warns only where a real window and occupancy are both known.
/// The whole-log token reading the reference's stats pill shows: every prompt-side billing
/// bucket plus output, and the cache-hit share of the billed input
/// (`StatsPills.tsx:281-300`). Absent when the projection is missing, malformed or has no
/// billed activity. Speed is deliberately absent: it needs recorded timings this app does
/// not carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenReading {
    pub total: u64,
    pub cache_hit: Option<String>,
}

pub fn token_reading(projections: &BTreeMap<String, Value>) -> Option<TokenReading> {
    let usage = projections.get("tokenUsage")?.as_object()?;
    let uncached = field(usage, "uncachedInputTokens").ok()??;
    let output = field(usage, "outputTokens").ok()??;
    let cached = field(usage, "cacheReadTokens").ok()??;
    let written = field(usage, "cacheWriteTokens").ok()??;
    let billed = sum(&[uncached, cached, written])?;
    let total = sum(&[billed, output])?;
    if billed == 0 && output == 0 {
        return None;
    }
    Some(TokenReading {
        total,
        cache_hit: cache_hit(cached, billed),
    })
}

/// The reference never rounds a near-full cache to a false 100: integer percent first, then
/// one decimal, and a capped `99.9` for the remaining edge.
fn cache_hit(cached: u64, billed: u64) -> Option<String> {
    if billed == 0 {
        return None;
    }
    if cached == billed {
        return Some("100".into());
    }
    let rounded = |scale: u128| -> u128 {
        (u128::from(cached) * 100 * scale + u128::from(billed) / 2) / u128::from(billed)
    };
    if rounded(1) < 100 {
        return Some(rounded(1).to_string());
    }
    if rounded(10) < 1000 {
        let units = rounded(10);
        return Some(format!("{}.{}", units / 10, units % 10));
    }
    Some("99.9".into())
}

/// One turn's accounting rows for the per-turn detail, from the assistant message's usage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnUsageDetail {
    pub total: Option<u64>,
    pub input: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub output: Option<u64>,
    pub reasoning: Option<u64>,
    /// The cached share of the billed input, one decimal, as the reference states it
    /// (`TurnUsagePanel.tsx:39-42`: `formatCacheHitPercent(cacheRead, total - output, 1)`).
    pub cache_hit: Option<String>,
    /// `provider/model` for every attributed attempt, comma-joined (`TurnUsagePanel.tsx:43-44`).
    pub routes: String,
}

impl TurnUsageDetail {
    pub fn has_rows(&self) -> bool {
        self.total.is_some()
            || self.input.is_some()
            || self.cache_read.is_some()
            || self.cache_write.is_some()
            || self.output.is_some()
            || self.reasoning.is_some()
            || self.cache_hit.is_some()
            || !self.routes.is_empty()
    }
}

/// The cached share of a billed prompt with one decimal, as the turn dialog states it: `100` when
/// every prompt token was cached, one rounded tenth otherwise (`token-format.ts:67-79`,
/// `TurnUsagePanel.tsx:39-42`). `None` when nothing was billed.
fn cache_hit_tenths(cache_read: u64, prompt: u64) -> Option<String> {
    if prompt == 0 {
        return None;
    }
    if cache_read >= prompt {
        return Some("100".into());
    }
    let units = (u128::from(cache_read) * 1_000 + u128::from(prompt) / 2) / u128::from(prompt);
    Some(format!("{}.{}", units / 10, units % 10))
}

pub fn turn_usage_detail(event: &SessionWireEvent) -> Option<TurnUsageDetail> {
    if event.event_type != "assistant/message" {
        return None;
    }
    let usage = event.data.get("usage")?.as_object()?;
    let field_any = |names: &[&str]| {
        names
            .iter()
            .find_map(|name| field(usage, name).ok().flatten())
    };
    let input = field_any(&["inputTokens", "uncachedInputTokens"]);
    let output = field_any(&["outputTokens"]);
    let cached = field_any(&["cacheReadTokens"]);
    let written = field_any(&["cacheWriteTokens"]);
    let reasoning = field_any(&["reasoningTokens"]);
    let total = field_any(&["totalTokens"]);
    // The reference divides the cached tokens by the billed input — `total - output` — rather than
    // by a sum it composes itself (`TurnUsagePanel.tsx:39-42`).
    let cache_hit = cached.and_then(|cached| {
        let billed = total
            .map(|total| total.saturating_sub(output.unwrap_or(0)))
            .unwrap_or_else(|| {
                input
                    .unwrap_or(0)
                    .saturating_add(cached)
                    .saturating_add(written.unwrap_or(0))
            });
        cache_hit_tenths(cached, billed)
    });
    let routes = usage
        .get("routes")
        .and_then(serde_json::Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let provider = row.get("provider")?.as_str()?;
                    let model = row.get("model")?.as_str()?;
                    Some(format!("{provider}/{model}"))
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let detail = TurnUsageDetail {
        total,
        input,
        cache_read: cached,
        cache_write: written,
        output,
        reasoning,
        cache_hit,
        routes,
    };
    detail.has_rows().then_some(detail)
}

/// The whole-log decode speed the reference's activity pill shows
/// (`StatsPills.tsx:166-170`): output tokens over the span from the first streamed chunk to
/// the assistant message. Only durable `assistant/message` records count, and a record
/// without a usable time/token pair is skipped rather than guessed. `None` when no step
/// carries both, or the span is zero.
pub fn decode_speed<'a>(records: impl Iterator<Item = &'a SessionWireEvent>) -> Option<String> {
    let mut decode_ms = 0u64;
    let mut tokens = 0u64;
    for event in records {
        if event.event_type != "assistant/message" {
            continue;
        }
        let Some(output) = event
            .data
            .get("usage")
            .and_then(|usage| usage.get("outputTokens"))
            .and_then(Value::as_u64)
        else {
            continue;
        };
        let Some(first) = event
            .data
            .get("stream")
            .and_then(Value::as_array)
            .and_then(|stream| {
                stream
                    .iter()
                    .find_map(|chunk| chunk.get("time0").and_then(Value::as_i64))
            })
        else {
            continue;
        };
        let completed = event.time;
        if completed <= first {
            continue;
        }
        let Some(next_ms) = decode_ms.checked_add((completed - first) as u64) else {
            return None;
        };
        let Some(next_tokens) = tokens.checked_add(output) else {
            return None;
        };
        decode_ms = next_ms;
        tokens = next_tokens;
    }
    if decode_ms == 0 || tokens == 0 || tokens > MAX_COUNT {
        return None;
    }
    let tps = tokens as f64 / (decode_ms as f64 / 1000.0);
    Some(if tps >= 10.0 {
        format!("{} tok/s", tps.round() as u64)
    } else {
        format!("{:.1} tok/s", (tps * 10.0).round() / 10.0)
    })
}

/// One completed turn's compact usage label, from the assistant message's own `usage`
/// (`message.turnUsage.count` in the reference). Prefers the Host's `totalTokens`, then the
/// four disjoint buckets, then the output alone; a turn with no positive count yields nothing.
pub fn turn_usage(event: &SessionWireEvent) -> Option<String> {
    if event.event_type != "assistant/message" {
        return None;
    }
    let usage = event.data.get("usage")?.as_object()?;
    let reported = field(usage, "totalTokens").ok().flatten();
    let summed = || {
        let uncached = field(usage, "uncachedInputTokens").ok().flatten()?;
        let output = field(usage, "outputTokens").ok().flatten()?;
        let cached = field(usage, "cacheReadTokens").ok().flatten()?;
        let written = field(usage, "cacheWriteTokens").ok().flatten()?;
        sum(&[uncached, cached, written, output])
    };
    let total = reported
        .or_else(summed)
        .or_else(|| field(usage, "outputTokens").ok().flatten())?;
    if total == 0 || total > MAX_COUNT {
        return None;
    }
    Some(format!(
        "{} tok",
        crate::ui::subagent_catalog::format_tokens(total)
    ))
}

pub fn state<'a>(
    projections: &BTreeMap<String, Value>,
    projection_current: bool,
    records: impl Iterator<Item = &'a SessionWireEvent>,
) -> ContextState {
    let Occupancy { used, capacity } = match reading(projections, projection_current, records) {
        Reading::Context(occupancy) => occupancy,
        Reading::Last(usage) => return ContextState::unavailable(last_request(usage)),
        Reading::Unavailable => return ContextState::unavailable(UNAVAILABLE.into()),
    };
    let severity = Severity::of(&Occupancy { used, capacity });
    let Some(severity) = severity else {
        // Implausible occupancy: keep the two counts the Host reported and add
        // nothing the numbers cannot support.
        return ContextState::unavailable(format!(
            "Context ~{} / {}",
            compact(used),
            compact(capacity)
        ));
    };
    let mut detail = vec![
        format!(
            "Context is approximate: {used} of {capacity} tokens reported by the Host, {}% of the window.",
            percent(used, capacity)
        ),
        PRUNE_NOTE.into(),
        TOOLS_NOTE.into(),
    ];
    if severity == Severity::Critical {
        detail.push(OVERFLOW_NOTE.into());
    }
    detail.truncate(MAX_DETAIL_LINES);
    ContextState {
        line: format!(
            "Context ~{} / {} · {}%",
            compact(used),
            compact(capacity),
            percent(used, capacity)
        ),
        severity,
        detail,
        used,
        capacity,
    }
}

/// Integer percentage of the window in use. Widened to `u128` so the largest
/// accepted count still divides exactly instead of saturating.
fn percent(used: u64, capacity: u64) -> u64 {
    (u128::from(used) * 100 / u128::from(capacity)) as u64
}

fn last_request(usage: Usage) -> String {
    match usage {
        Usage {
            prompt: Some(prompt),
            output: Some(output),
            ..
        } => format!(
            "Last request: {} in · {} out",
            compact(prompt),
            compact(output)
        ),
        Usage { total, .. } => format!("Last request: {} tokens", compact(total)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn event(seq: u64, kind: &str, data: Value) -> SessionWireEvent {
        SessionWireEvent {
            event_type: kind.into(),
            seq,
            time: 1,
            data,
            ignorable: None,
            source_event_seqs: None,
            surface_op: None,
        }
    }
    fn pressure(value: Value) -> BTreeMap<String, Value> {
        BTreeMap::from([("contextPressure".into(), value)])
    }
    fn message(value: Value) -> SessionWireEvent {
        event(10, "assistant/message", json!({"usage":value}))
    }
    /// The tier is presentation only; the line it colours is `describe`'s exact
    /// line, so a warning can never describe a different session than the label.
    #[test]
    fn tier_and_percentage_follow_the_reported_occupancy() {
        let read = |value: Value| state(&pressure(value), true, [].iter());
        let normal = read(json!({"pressureTokens":17,"contextWindow":128000}));
        assert_eq!(normal.line, "Context ~17 / 128k · 0%");
        assert_eq!(normal.severity, Severity::Normal);
        assert_eq!(normal.detail.len(), 3);

        let tight = read(json!({"pressureTokens":100,"contextWindow":128000}));
        assert_eq!(tight.line, "Context ~100 / 128k · 0%");
        assert_eq!(tight.severity, Severity::Normal);

        let tight =
            read(json!({"pressureTokens":17000,"projectedTokens":18000,"contextWindow":128000}));
        assert_eq!(tight.line, "Context ~18k / 128k · 14%");
        assert_eq!(tight.severity, Severity::Normal);
        assert!(
            !tight
                .detail
                .iter()
                .any(|line| line.contains("window limit"))
        );

        // Exactly at the mounted compaction threshold is still Normal: nothing
        // has been reclaimed yet.
        let at_threshold = read(json!({"pressureTokens":10240,"contextWindow":12800}));
        assert_eq!(at_threshold.severity, Severity::Normal);
        let above = read(json!({"pressureTokens":10241,"contextWindow":12800}));
        assert_eq!(above.severity, Severity::Tight);
        assert_eq!(above.line, "Context ~10241 / 12800 · 80%");
        assert!(
            !above
                .detail
                .iter()
                .any(|line| line.contains("window limit"))
        );

        // At or past the window the run is the paper's overflow case.
        let critical = read(json!({"pressureTokens":128000,"contextWindow":128000}));
        assert_eq!(critical.severity, Severity::Critical);
        assert_eq!(critical.line, "Context ~128k / 128k · 100%");
        assert!(
            critical
                .detail
                .iter()
                .any(|line| line.contains("window limit"))
        );
        let over = read(json!({"pressureTokens":130000,"contextWindow":128000}));
        assert_eq!(over.severity, Severity::Critical);
        assert_eq!(over.line, "Context ~130k / 128k · 101%");
        assert!(over.detail.len() <= MAX_DETAIL_LINES);
    }
    /// A percentage that overflows or an absent window must not be guessed, and
    /// a last-request line is never a context warning.
    #[test]
    fn absent_or_unsafe_occupancy_discloses_nothing() {
        let p = pressure(json!({"pressureTokens":1,"contextWindow":0}));
        assert_eq!(state(&p, true, [].iter()).line, UNAVAILABLE);
        assert_eq!(state(&p, true, [].iter()).severity, Severity::Normal);
        assert!(state(&p, true, [].iter()).detail.is_empty());
        let huge = pressure(json!({"pressureTokens":MAX_COUNT,"contextWindow":1}));
        let implausible = state(&huge, true, [].iter());
        assert_eq!(implausible.line, format!("Context ~{MAX_COUNT} / 1"));
        assert_eq!(implausible.severity, Severity::Normal);
        assert!(implausible.detail.is_empty());
        let stale = pressure(json!({"pressureTokens":120000,"contextWindow":128000}));
        assert_eq!(state(&stale, false, [].iter()).line, UNAVAILABLE);
        assert!(state(&stale, false, [].iter()).detail.is_empty());
        let events = [message(
            json!({"inputTokens":100,"outputTokens":20,"totalTokens":120}),
        )];
        let line = state(&BTreeMap::new(), false, events.iter());
        assert_eq!(line.line, "Last request: 120 tokens");
        assert_eq!(line.severity, Severity::Normal);
        assert!(line.detail.is_empty());
        let summary = state(&BTreeMap::new(), true, events.iter());
        assert_eq!(summary.line, "Last request: 120 tokens");
        assert_eq!(summary.severity, Severity::Normal);
        assert_eq!(summary.detail, Vec::<String>::new());
    }
    #[test]
    fn public_context_is_approximate_and_never_uses_stale_capacity() {
        let p = pressure(
            json!({"pressureTokens":17000,"projectedTokens":18000,"contextWindow":128000}),
        );
        assert_eq!(describe(&p, true, [].iter()), "Context ~18k / 128k");
        assert_eq!(describe(&p, false, [].iter()), UNAVAILABLE);
        assert_eq!(
            describe(
                &pressure(json!({"pressureTokens":0,"contextWindow":128000})),
                true,
                [].iter()
            ),
            "Context ~0 / 128k"
        );
    }
    #[test]
    fn complete_disjoint_buckets_and_reasoning_are_not_double_counted() {
        let events = [message(
            json!({"inputTokens":100,"cacheReadTokens":40,"cacheWriteTokens":10,
            "outputTokens":20,"reasoningTokens":8,"totalTokens":170}),
        )];
        assert_eq!(
            describe(&BTreeMap::new(), false, events.iter()),
            "Last request: 150 in · 20 out"
        );
        let incomplete = [message(
            json!({"inputTokens":100,"outputTokens":20,"totalTokens":170}),
        )];
        assert_eq!(
            describe(&BTreeMap::new(), false, incomplete.iter()),
            "Last request: 170 tokens"
        );
        let unknown = [message(json!({"inputTokens":100,"outputTokens":20}))];
        assert_eq!(
            describe(&BTreeMap::new(), false, unknown.iter()),
            UNAVAILABLE
        );
    }
    #[test]
    fn raw_compact_last_sample_and_top_level_usage_replace_not_sum() {
        let events = [event(
            0,
            "assistant/attempt",
            json!({"stream":[
                {"type":"chunk","time":1,"chunk":{"type":"usage","usage":{"totalTokens":10}}},
                {"type":"chunk","time":2,"chunk":{"type":"usage","usage":{"totalTokens":20}}}
            ]}),
        )];
        assert_eq!(
            describe(&BTreeMap::new(), false, events.iter()),
            "Last request: 20 tokens"
        );
        let events = [event(
            0,
            "assistant/message",
            json!({"usage":{"totalTokens":30},"stream":[
                {"type":"chunk","time":2,"chunk":{"type":"usage","usage":{"totalTokens":20}}}
            ]}),
        )];
        assert_eq!(
            describe(&BTreeMap::new(), false, events.iter()),
            "Last request: 30 tokens"
        );
    }
    #[test]
    fn malformed_numbers_and_unsafe_sums_fail_closed() {
        for bad in [
            json!(-1),
            json!(1.5),
            Value::Null,
            json!("12"),
            json!(MAX_COUNT + 1),
        ] {
            let p = pressure(json!({"projectedTokens":bad,"contextWindow":128000}));
            assert_eq!(describe(&p, true, [].iter()), UNAVAILABLE);
            let events = [message(json!({"inputTokens":bad,"totalTokens":100}))];
            assert_eq!(
                describe(&BTreeMap::new(), false, events.iter()),
                UNAVAILABLE
            );
        }
        for bad in [
            json!({"totalTokens":5,"outputTokens":10}),
            json!({"inputTokens":MAX_COUNT,"outputTokens":1,"cacheReadTokens":0,"cacheWriteTokens":0}),
            json!({"inputTokens":1,"outputTokens":1,"cacheReadTokens":0,"cacheWriteTokens":0,"totalTokens":3}),
            json!({"outputTokens":1,"reasoningTokens":2,"totalTokens":3}),
        ] {
            assert_eq!(
                describe(&BTreeMap::new(), false, [message(bad)].iter()),
                UNAVAILABLE
            );
        }
        assert_eq!(
            describe(
                &pressure(json!({"pressureTokens":1,"contextWindow":0})),
                true,
                [].iter()
            ),
            UNAVAILABLE
        );
    }
    #[test]
    fn later_route_and_pending_selection_do_not_borrow_old_window() {
        let p = pressure(json!({"pressureTokens":100,"contextWindow":128000}));
        let events = [
            event(
                0,
                "request/context",
                json!({"provider":"PUBLIC","model":"a"}),
            ),
            event(1, "assistant/message", json!({"usage":{"totalTokens":100}})),
            event(
                2,
                "request/context",
                json!({"provider":"PUBLIC","model":"b"}),
            ),
        ];
        assert_eq!(describe(&p, true, events.iter()), UNAVAILABLE);
        let mut pending = p;
        pending.insert(
            "modelSelection".into(),
            json!({"lastUsed":{"provider":"PUBLIC","model":"a"},
            "next":{"provider":"PUBLIC","model":"b"}}),
        );
        assert_eq!(describe(&pending, true, [].iter()), UNAVAILABLE);
    }
    #[test]
    fn the_token_reading_sums_the_buckets_and_never_rounds_a_near_full_cache_to_100() {
        let with = |uncached: u64, output: u64, cached: u64, written: u64| {
            let mut p = BTreeMap::new();
            p.insert(
                "tokenUsage".to_owned(),
                json!({"uncachedInputTokens":uncached,"outputTokens":output,
                    "cacheReadTokens":cached,"cacheWriteTokens":written}),
            );
            p
        };
        let reading = token_reading(&with(6000, 345, 6000, 0)).unwrap();
        assert_eq!(reading.total, 12345);
        assert_eq!(reading.cache_hit.as_deref(), Some("50"));
        let reading = token_reading(&with(0, 0, 1000, 0)).unwrap();
        assert_eq!(reading.cache_hit.as_deref(), Some("100"));
        let reading = token_reading(&with(1, 0, 999, 0)).unwrap();
        assert_eq!(reading.cache_hit.as_deref(), Some("99.9"));
        assert_eq!(token_reading(&with(0, 0, 0, 0)), None);
        let mut partial = BTreeMap::new();
        partial.insert(
            "tokenUsage".to_owned(),
            json!({"uncachedInputTokens":1,"outputTokens":2}),
        );
        assert_eq!(token_reading(&partial), None);
        assert_eq!(token_reading(&BTreeMap::new()), None);
    }

    #[test]
    fn decode_speed_sums_durable_steps_and_skips_records_without_both_readings() {
        let step = |seq: u64, time0: i64, time: i64, output: u64| {
            let mut event = event(
                seq,
                "assistant/message",
                json!({"usage":{"outputTokens":output},
                    "stream":[{"type":"text-chunks","time0":time0,"dt":[],"texts":["PUBLIC"]}]}),
            );
            event.time = time;
            event
        };
        let events = vec![
            step(0, 1000, 3000, 100),
            step(1, 5000, 6000, 20),
            event(2, "assistant/message", json!({"stream":[]})),
        ];
        assert_eq!(decode_speed(events.iter()).as_deref(), Some("40 tok/s"));
        let slow = vec![step(0, 0, 2000, 5)];
        assert_eq!(decode_speed(slow.iter()).as_deref(), Some("2.5 tok/s"));
        assert_eq!(decode_speed([].iter()), None);
    }

    #[test]
    fn a_turn_usage_detail_lists_the_reported_buckets_and_cache_share() {
        let mut event = SessionWireEvent {
            event_type: "assistant/message".into(),
            seq: 0,
            time: 1,
            data: json!({"usage":{"inputTokens":1000,"outputTokens":345,
                "cacheReadTokens":6000,"cacheWriteTokens":0,"totalTokens":7345}}),
            ignorable: None,
            source_event_seqs: None,
            surface_op: None,
        };
        let detail = turn_usage_detail(&event).unwrap();
        assert_eq!(detail.input, Some(1000));
        assert_eq!(detail.cache_read, Some(6000));
        assert_eq!(detail.output, Some(345));
        assert_eq!(detail.total, Some(7345));
        // `formatCacheHitPercent(cacheRead, total - output, 1)`: 6000 / 7000.
        assert_eq!(detail.cache_hit.as_deref(), Some("85.7"));
        assert!(detail.routes.is_empty(), "this record names no route");
        event.data = json!({"usage":{}});
        assert_eq!(turn_usage_detail(&event), None);
    }

    #[test]
    fn a_turn_usage_detail_names_every_attributed_route_and_the_reasoning_share() {
        let event = SessionWireEvent {
            event_type: "assistant/message".into(),
            seq: 0,
            time: 1,
            data: json!({"usage":{
                "uncachedInputTokens":1200,"cacheReadTokens":6000,"cacheWriteTokens":800,
                "outputTokens":900,"reasoningTokens":250,"totalTokens":8100,
                "routes":[{"provider":"PUBLIC","model":"PUBLIC-model"},
                    {"provider":"PUBLIC-2","model":"PUBLIC-2"}]
            }}),
            ignorable: None,
            source_event_seqs: None,
            surface_op: None,
        };
        let detail = turn_usage_detail(&event).unwrap();
        assert_eq!(detail.routes, "PUBLIC/PUBLIC-model, PUBLIC-2/PUBLIC-2");
        assert_eq!(detail.reasoning, Some(250));
        // 6000 / (8100 - 900) = 83.33…, one decimal as the reference states it.
        assert_eq!(detail.cache_hit.as_deref(), Some("83.3"));
    }

    #[test]
    fn a_turn_usage_prefers_total_then_buckets_then_output() {
        let turn = |usage: Value| SessionWireEvent {
            event_type: "assistant/message".into(),
            seq: 0,
            time: 1,
            data: json!({"usage": usage}),
            ignorable: None,
            source_event_seqs: None,
            surface_op: None,
        };
        assert_eq!(
            turn_usage(&turn(json!({"totalTokens":12345}))).as_deref(),
            Some("12.3K tok")
        );
        assert_eq!(
            turn_usage(&turn(json!({"uncachedInputTokens":1,"cacheReadTokens":2,
                "cacheWriteTokens":3,"outputTokens":4})))
            .as_deref(),
            Some("10 tok")
        );
        assert_eq!(
            turn_usage(&turn(json!({"outputTokens":345}))).as_deref(),
            Some("345 tok")
        );
        assert_eq!(turn_usage(&turn(json!({"outputTokens":0}))), None);
        assert_eq!(turn_usage(&turn(json!({"totalTokens":-1}))), None);
        assert_eq!(turn_usage(&turn(json!({}))), None);
    }

    #[test]
    fn nonsequential_range_and_latest_missing_usage_fail_closed() {
        let p = pressure(json!({"pressureTokens":100,"contextWindow":128000}));
        for sequences in [[1, 3], [2, 1], [1, 1], [MAX_COUNT, MAX_COUNT + 1]] {
            let events = sequences.map(|seq| event(seq, "PUBLIC/ignorable", json!({})));
            assert_eq!(describe(&p, true, events.iter()), UNAVAILABLE);
        }
        let events = [
            event(0, "assistant/message", json!({"usage":{"totalTokens":100}})),
            event(1, "assistant/message", json!({"stream":[]})),
        ];
        assert_eq!(describe(&p, true, events.iter()), UNAVAILABLE);
        let events: Vec<_> = (0..=MAX_RECORDS)
            .map(|seq| event(seq as u64, "PUBLIC/ignorable", json!({})))
            .collect();
        assert_eq!(describe(&p, true, events.iter()), UNAVAILABLE);
    }
}
