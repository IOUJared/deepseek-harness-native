//! One completed Turn's exact token accounting.
//!
//! The reference derives a Turn's usage away from any single message: it folds the Turn's durable
//! attempt lifecycle from `turn/start` to `turn/end` and sums one attempt per boundary, refusing the
//! whole disclosure when a boundary, a bucket or a count cannot be proven
//! (`packages/llm/token-meter/src/turn-usage.ts:190-300`). Native used to read one assistant
//! message, so a retried Turn reported only its last attempt.
//!
//! This module is the fold. It reads the source's own buckets and attempts and never infers a
//! sample: an attempt without usage, or a Turn whose start/end do not bracket its attempts, yields
//! `None` and the caller keeps the single-message reading it had.
use dsh_native_transport::dto::SessionWireEvent;

/// One attributed provider/model pair an attempt ran on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub provider: String,
    pub model: String,
}

/// Exact aggregate usage for one completed Turn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TurnUsage {
    /// Sum of uncached prompt input across attempts.
    pub uncached_input: u64,
    pub output: u64,
    pub total: u64,
    /// Present only when every attempt reported the bucket (`turn-usage.ts:134-141`).
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub reasoning: Option<u64>,
    /// Present only when every attempt named a route, unique in first-seen order.
    pub routes: Option<Vec<Route>>,
}

/// One attempt's reported buckets.
#[derive(Clone, Debug)]
struct Sample {
    input: u64,
    output: u64,
    total: u64,
    cache_read: Option<u64>,
    cache_write: Option<u64>,
    reasoning: Option<u64>,
    route: Option<Route>,
}

/// A safe count: non-negative and within `i64`.
fn count(value: Option<u64>) -> Option<u64> {
    value.filter(|value| *value <= i64::MAX as u64)
}

/// The usage an event reports, when it reports one.
fn sample(event: &SessionWireEvent) -> Option<Sample> {
    let usage = event.data.get("usage")?.as_object()?;
    let field = |names: &[&str]| {
        names
            .iter()
            .find_map(|name| usage.get(*name).and_then(serde_json::Value::as_u64))
    };
    let input = count(field(&["uncachedInputTokens", "inputTokens"]))?;
    let output = count(field(&["outputTokens"]))?;
    let total = count(field(&["totalTokens"]))
        .or_else(|| input.checked_add(output))
        .or_else(|| count(field(&["outputTokens"])))?;
    Some(Sample {
        input,
        output,
        total,
        cache_read: count(field(&["cacheReadTokens"])),
        cache_write: count(field(&["cacheWriteTokens"])),
        reasoning: count(field(&["reasoningTokens"])),
        route: route(event),
    })
}

/// The attempt's own attribution, when the record names one.
fn route(event: &SessionWireEvent) -> Option<Route> {
    let named = |value: &serde_json::Value| {
        let provider = value.get("provider")?.as_str()?;
        let model = value.get("model")?.as_str()?;
        Some(Route {
            provider: provider.to_owned(),
            model: model.to_owned(),
        })
    };
    // The reference reads the route from the message's own source, not from the usage sample
    // (`turn-usage.ts:72-75`).
    event.data.get("route").and_then(named).or_else(|| {
        event
            .data
            .get("message")
            .and_then(|message| message.get("source"))
            .and_then(named)
    })
}

fn sum(values: impl Iterator<Item = u64>) -> Option<u64> {
    values.fold(Some(0u64), |total, value| total?.checked_add(value))
}

/// Fold one Turn's attempts into its exact accounting.
///
/// `events` must be the Turn's records in log order, from its `turn/start` through its `turn/end`.
/// Returns `None` when the lifecycle is not exactly one bracketed Turn with closed attempts, or when
/// a required sum cannot be proved — the caller states what it had before rather than a guess.
pub fn derive<'a>(events: impl Iterator<Item = &'a SessionWireEvent>) -> Option<TurnUsage> {
    let mut started = false;
    let mut ended = false;
    let mut attempts: Vec<Sample> = Vec::new();
    // An attempt is *open* between its step or retry start and its report, and *settled* between a
    // report and its step end. `turn-usage.ts:190-290` keeps the same three states.
    let mut open = false;
    let mut settled = false;
    for event in events {
        match event.event_type.as_str() {
            "turn/start" => {
                if started || ended {
                    return None;
                }
                started = true;
            }
            "turn/end" => {
                // Only an idle state may see the Turn end: an attempt still open or just reported
                // without its step end is the source's contradiction (`turn-usage.ts:206-212`).
                if !started || ended || open || settled {
                    return None;
                }
                ended = true;
            }
            "step/start" | "llm/retry-started" => {
                if !started || ended || open || settled {
                    return None;
                }
                open = true;
            }
            "assistant/attempt" | "assistant/message" => {
                // A report with no open attempt is the source's own contradiction.
                if !started || ended || !open {
                    return None;
                }
                attempts.push(sample(event)?);
                open = false;
                settled = true;
            }
            "step/end" | "llm/retry" => {
                open = false;
                settled = false;
            }
            _ => {}
        }
    }
    if !started || !ended || attempts.is_empty() {
        return None;
    }
    let total = sum(attempts.iter().map(|attempt| attempt.total))?;
    // Optional buckets survive only when every attempt reported them, as the reference requires.
    let all = |pick: fn(&Sample) -> Option<u64>| {
        attempts
            .iter()
            .map(pick)
            .try_fold(Vec::new(), |mut values, value| {
                values.push(value?);
                Some(values)
            })
            .and_then(|values| sum(values.into_iter()))
    };
    let route_all = attempts
        .iter()
        .map(|attempt| attempt.route.clone())
        .collect::<Option<Vec<Route>>>();
    let routes = route_all.map(|routes| {
        let mut unique: Vec<Route> = Vec::new();
        for route in routes {
            if !unique.contains(&route) {
                unique.push(route);
            }
        }
        unique
    });
    Some(TurnUsage {
        uncached_input: sum(attempts.iter().map(|attempt| attempt.input))?,
        output: sum(attempts.iter().map(|attempt| attempt.output))?,
        total,
        cache_read: all(|attempt| attempt.cache_read),
        cache_write: all(|attempt| attempt.cache_write),
        reasoning: all(|attempt| attempt.reasoning),
        routes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(kind: &str, seq: u64, data: serde_json::Value) -> SessionWireEvent {
        SessionWireEvent {
            event_type: kind.into(),
            seq,
            time: i64::try_from(seq).unwrap_or(i64::MAX),
            data,
            ignorable: None,
            source_event_seqs: None,
            surface_op: None,
        }
    }

    fn message(seq: u64, usage: serde_json::Value) -> SessionWireEvent {
        event("assistant/message", seq, json!({"usage": usage}))
    }

    #[test]
    fn two_attempts_are_summed_into_one_turn() {
        let events = [
            event("turn/start", 0, json!({"turn": 1})),
            event("step/start", 1, json!({"turn": 1, "step": 1})),
            event(
                "assistant/attempt",
                2,
                json!({"turn": 1, "step": 1,
                    "usage": {"uncachedInputTokens": 100, "outputTokens": 10,
                        "cacheReadTokens": 900, "totalTokens": 1010}}),
            ),
            message(
                3,
                json!({"uncachedInputTokens": 100, "outputTokens": 10,
                    "cacheReadTokens": 900, "cacheWriteTokens": 0, "totalTokens": 1010}),
            ),
            event("step/end", 4, json!({"turn": 1, "step": 1})),
            event("turn/end", 5, json!({"turn": 1})),
        ];
        // The attempt and the message are the same attempt's boundary: only one sample per step is
        // required, and a second sample in the step is the source's contradiction.
        assert!(
            derive(events.iter()).is_none(),
            "two samples for one attempt is refused"
        );

        let events = [
            event("turn/start", 0, json!({"turn": 1})),
            event("step/start", 1, json!({"turn": 1, "step": 1})),
            message(
                2,
                json!({"uncachedInputTokens": 100, "outputTokens": 10, "totalTokens": 110}),
            ),
            event("step/end", 3, json!({"turn": 1, "step": 1})),
            event("step/start", 4, json!({"turn": 1, "step": 2})),
            message(
                5,
                json!({"uncachedInputTokens": 200, "outputTokens": 20, "totalTokens": 220}),
            ),
            event("step/end", 6, json!({"turn": 1, "step": 2})),
            event("turn/end", 7, json!({"turn": 1})),
        ];
        let usage = derive(events.iter()).expect("two closed attempts");
        assert_eq!(usage.uncached_input, 300);
        assert_eq!(usage.output, 30);
        assert_eq!(usage.total, 330);
        // Cache buckets exist only when every attempt reported them.
        assert_eq!(usage.cache_read, None);
    }

    #[test]
    fn an_unbracketed_or_open_attempt_yields_none() {
        let open = [
            event("turn/start", 0, json!({"turn": 1})),
            event("step/start", 1, json!({"turn": 1, "step": 1})),
        ];
        assert_eq!(derive(open.iter()), None, "no attempt and no end");
        let unclosed = [
            event("turn/start", 0, json!({"turn": 1})),
            event("step/start", 1, json!({"turn": 1, "step": 1})),
            message(
                2,
                json!({"uncachedInputTokens": 1, "outputTokens": 1, "totalTokens": 2}),
            ),
            event("turn/end", 3, json!({"turn": 1})),
        ];
        // The step never ended; the reference refuses the disclosure rather than guess.
        assert_eq!(derive(unclosed.iter()), None);
        let missing_usage = [
            event("turn/start", 0, json!({"turn": 1})),
            event("step/start", 1, json!({"turn": 1, "step": 1})),
            event("assistant/message", 2, json!({"stream": []})),
            event("step/end", 3, json!({"turn": 1, "step": 1})),
            event("turn/end", 4, json!({"turn": 1})),
        ];
        assert_eq!(
            derive(missing_usage.iter()),
            None,
            "an attempt without usage"
        );
    }

    #[test]
    fn cache_buckets_and_routes_survive_only_when_every_attempt_names_them() {
        let attempt = |seq: u64, _step: u64, route: Option<(&str, &str)>| {
            let usage = json!({"uncachedInputTokens": 10, "outputTokens": 2,
                "cacheReadTokens": 90, "cacheWriteTokens": 5, "reasoningTokens": 1,
                "totalTokens": 102});
            let mut event = message(seq, usage);
            if let Some((provider, model)) = route {
                // The reference reads the route from the message's own source
                // (`turn-usage.ts:72-75`), not from the usage sample.
                event.data["message"] = json!({"source": {"provider": provider, "model": model}});
            }
            event
        };
        let events = [
            event("turn/start", 0, json!({"turn": 1})),
            event("step/start", 1, json!({"turn": 1, "step": 1})),
            attempt(2, 1, Some(("PUBLIC", "PUBLIC-model"))),
            event("step/end", 3, json!({"turn": 1, "step": 1})),
            event("step/start", 4, json!({"turn": 1, "step": 2})),
            attempt(5, 2, None),
            event("step/end", 6, json!({"turn": 1, "step": 2})),
            event("turn/end", 7, json!({"turn": 1})),
        ];
        let usage = derive(events.iter()).expect("every bucket was reported");
        assert_eq!(usage.cache_read, Some(180));
        assert_eq!(usage.cache_write, Some(10));
        assert_eq!(usage.reasoning, Some(2));
        // One attempt named no route, so the aggregate names none.
        assert_eq!(usage.routes, None);

        let mut attributed = events;
        attributed[5] = attempt(5, 2, Some(("PUBLIC", "PUBLIC-model")));
        let usage = derive(attributed.iter()).expect("attributed");
        assert_eq!(
            usage.routes,
            Some(vec![Route {
                provider: "PUBLIC".into(),
                model: "PUBLIC-model".into()
            }])
        );
    }
}
