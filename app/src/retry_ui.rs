//! The reference's model-request retry row (`retry.ts:20-100`, `TurnTriggerNodeView.tsx:60-110`,
//! `locale.ts:349-356`).
//!
//! A retry chain is one row, not one per event: the Host opens it with `llm/retry`, updates it with
//! further `llm/retry` attempts and close each with `llm/retry-started`, all sharing a `retryId`.
//! The row states the chain's latest attempt — waiting, retrying, retried or cancelled — with its
//! `(retry/max) · seconds` position, and carries the delay and the failure reason as its body.
use serde_json::Value;

/// Hard bounds from the session format (`payload-validation.ts:119-138`).
const MAX_DELAY_MS: u64 = 2_147_483_647;
const MAX_TEXT_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The Host scheduled this attempt and is waiting out the delay.
    Scheduled,
    /// The attempt's delay elapsed and the request is being retried.
    Started,
    /// A scheduled attempt whose turn or step closed before it ran.
    Cancelled,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Attempt {
    pub retry: u32,
    /// `None` in `always` mode, which retries without a ceiling.
    pub max: Option<u32>,
    pub delay_ms: u64,
    pub failure: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct State {
    /// The chain's attempts, in recorded order; the last is the one stated.
    pub attempts: Vec<Attempt>,
    /// Attempts the Host has started, by their `retry` number.
    pub started: Vec<u32>,
    pub cancelled: bool,
}

impl State {
    /// The attempt the row states.
    pub fn current(&self) -> Option<&Attempt> {
        self.attempts.last()
    }
    pub fn phase(&self) -> Option<Phase> {
        let current = self.current()?;
        if self.started.contains(&current.retry) {
            return Some(Phase::Started);
        }
        Some(if self.cancelled {
            Phase::Cancelled
        } else {
            Phase::Scheduled
        })
    }
    /// The reference's label (`locale.ts:349-356`).
    pub fn title(&self) -> &'static str {
        match self.phase() {
            Some(Phase::Started) => "Retried model request",
            Some(Phase::Cancelled) => "Model request retry cancelled",
            _ => "Waiting to retry model request",
        }
    }
    /// The reference's `{label} ({retry}/{maximum}) · {seconds}s`, with the `always` mode omitting
    /// the ceiling and a started attempt omitting the countdown.
    pub fn line(&self) -> String {
        let Some(current) = self.current() else {
            return self.title().to_owned();
        };
        let position = match current.max {
            Some(max) => format!("({}/{max})", current.retry),
            None => format!("({})", current.retry),
        };
        match self.phase() {
            Some(Phase::Started) | Some(Phase::Cancelled) => {
                format!("{} {}", self.title(), position)
            }
            _ => format!(
                "{} {} · {}s",
                self.title(),
                position,
                current.delay_ms / 1000
            ),
        }
    }
    /// The body: the recorded delay and the failure that caused the retry.
    pub fn detail(&self) -> Option<String> {
        let current = self.current()?;
        let mut parts = vec![format!("Retry delay: {}s", current.delay_ms / 1000)];
        if let Some(failure) = &current.failure {
            parts.push(format!("Failure reason: {failure}"));
        }
        Some(parts.join("\n"))
    }
    pub fn has_body(&self) -> bool {
        self.current().is_some()
    }
}

/// Parse one `llm/retry` event's data. Every field is validated with the format's own rules; a
/// payload that fails one contributes no row rather than a half-stated retry.
pub fn attempt(data: &Value) -> Option<Attempt> {
    let mode = data["mode"].as_str()?;
    let retry = u32::try_from(data["retry"].as_u64()?).ok()?;
    if retry == 0 {
        return None;
    }
    let max = match mode {
        "normal" => {
            let max = u32::try_from(data["maxRetries"].as_u64()?).ok()?;
            if retry > max {
                return None;
            }
            Some(max)
        }
        "always" => {
            if data.get("maxRetries").is_some_and(|value| !value.is_null()) {
                return None;
            }
            None
        }
        _ => return None,
    };
    let delay_ms = data["delayMs"].as_u64()?;
    if delay_ms > MAX_DELAY_MS {
        return None;
    }
    // The provider and policy are required on the wire; the row does not draw them, but a payload
    // missing one is not a retry record this row may state.
    data["provider"].as_str().filter(|v| !v.is_empty())?;
    data["policyKey"].as_str().filter(|v| !v.is_empty())?;
    let failure = data["failure"]["message"]
        .as_str()
        .map(|text| crate::reducer::bounded(text, MAX_TEXT_BYTES))
        .filter(|text| !text.trim().is_empty());
    Some(Attempt {
        retry,
        max,
        delay_ms,
        failure,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn data(retry: u32, mode: &str, max: Value, delay: u64) -> Value {
        json!({"retryId": "r1", "turn": 1, "step": 1, "provider": "deepseek",
            "mode": mode, "policyKey": "PUBLIC-policy", "retry": retry, "maxRetries": max,
            "delayMs": delay, "failure": {"message": "PUBLIC failure text"}})
    }
    fn chain(attempts: Vec<Attempt>) -> State {
        State {
            attempts,
            started: Vec::new(),
            cancelled: false,
        }
    }

    #[test]
    fn a_normal_retry_states_its_position_and_countdown() {
        let state = chain(vec![attempt(&data(2, "normal", json!(4), 3_000)).unwrap()]);
        assert_eq!(state.phase(), Some(Phase::Scheduled));
        assert_eq!(state.line(), "Waiting to retry model request (2/4) · 3s");
        assert_eq!(
            state.detail().as_deref(),
            Some("Retry delay: 3s\nFailure reason: PUBLIC failure text")
        );
    }

    #[test]
    fn an_always_retry_omits_the_ceiling() {
        let state = chain(vec![
            attempt(&data(7, "always", Value::Null, 1_500)).unwrap(),
        ]);
        assert_eq!(state.line(), "Waiting to retry model request (7) · 1s");
    }

    #[test]
    fn a_started_attempt_drops_the_countdown_and_says_so() {
        let mut state = chain(vec![
            attempt(&data(1, "normal", json!(3), 2_000)).unwrap(),
            attempt(&data(2, "normal", json!(3), 4_000)).unwrap(),
        ]);
        state.started.push(2);
        assert_eq!(state.phase(), Some(Phase::Started));
        assert_eq!(state.line(), "Retried model request (2/3)");
    }

    #[test]
    fn a_closed_turn_cancels_a_scheduled_attempt() {
        let mut state = chain(vec![attempt(&data(1, "normal", json!(2), 0)).unwrap()]);
        state.cancelled = true;
        assert_eq!(state.phase(), Some(Phase::Cancelled));
        assert_eq!(state.line(), "Model request retry cancelled (1/2)");
    }

    #[test]
    fn the_payload_rules_are_the_formats_own() {
        for bad in [
            // Missing mode, unknown mode, retry zero, retry past the ceiling.
            json!({"retryId": "r", "provider": "p", "policyKey": "k", "retry": 1, "delayMs": 0}),
            data(1, "sometimes", json!(2), 0),
            data(0, "normal", json!(2), 0),
            data(3, "normal", json!(2), 0),
            // `always` must omit the ceiling; a negative or oversized delay is not a timer.
            json!({"retryId": "r", "provider": "p", "mode": "always", "policyKey": "k", "retry": 1,
                "maxRetries": 2, "delayMs": 0}),
            json!({"retryId": "r", "provider": "p", "mode": "normal", "policyKey": "k", "retry": 1,
                "maxRetries": 2, "delayMs": -1}),
            json!({"retryId": "r", "provider": "p", "mode": "normal", "policyKey": "k", "retry": 1,
                "maxRetries": 2, "delayMs": 2_147_483_648u64}),
        ] {
            assert!(attempt(&bad).is_none(), "{bad}");
        }
        // A failure without a message leaves the reason absent rather than empty.
        let mut without = data(1, "normal", json!(2), 0);
        without["failure"] = json!({});
        assert_eq!(attempt(&without).unwrap().failure, None);
    }
}
