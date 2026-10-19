//! The session-statistics dialog behind the composer's usage pill
//! (`StatsPills.tsx:225-270`, `stat-dialog.ts`, `session-stats/src/projection.ts:72-80`).
//!
//! Every figure rides the Host's durable `sessionStats` projection, because the record window is
//! paged and compaction rewrites it; the fold over the records on hand is only the fallback for an
//! assembly that serves no projection, and it answers "what is on screen".
use serde_json::Value;
use std::collections::BTreeMap;

/// The eight figures the Host's projection states, validated strictly (`sessionStatsSchema`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub turns: u64,
    pub steps: u64,
    pub llm_ms: i64,
    pub tool_ms: i64,
    pub ttft_ms: i64,
    pub ttft_steps: u64,
    pub decode_ms: i64,
    pub decode_tokens: u64,
}

fn integer(value: Option<&Value>) -> Option<u64> {
    value.and_then(Value::as_u64)
}

impl Stats {
    /// The projection's own value, or `None` when it is absent or fails a field.
    pub fn from_projection(values: &BTreeMap<String, Value>) -> Option<Stats> {
        let value = values.get("sessionStats")?;
        let stats = Stats {
            turns: integer(value.get("turns"))?,
            steps: integer(value.get("steps"))?,
            llm_ms: i64::try_from(integer(value.get("llmMs"))?).ok()?,
            tool_ms: i64::try_from(integer(value.get("toolMs"))?).ok()?,
            ttft_ms: i64::try_from(integer(value.get("ttftMs"))?).ok()?,
            ttft_steps: integer(value.get("ttftSteps"))?,
            decode_ms: i64::try_from(integer(value.get("decodeMs"))?).ok()?,
            decode_tokens: integer(value.get("decodeTokens"))?,
        };
        Some(stats)
    }
    /// The dialog's rows, in the reference's order, skipping a figure nobody recorded
    /// (`StatsPills.tsx:252-270`).
    pub fn rows(&self) -> Vec<(&'static str, String)> {
        let mut rows = Vec::new();
        if self.llm_ms > 0 {
            rows.push(("LLM time", duration(self.llm_ms)));
        }
        if self.tool_ms > 0 {
            rows.push(("Tool time", duration(self.tool_ms)));
        }
        if self.ttft_steps > 0 && self.ttft_ms > 0 {
            rows.push((
                "Avg time to first token (TTFT)",
                duration(self.ttft_ms / self.ttft_steps as i64),
            ));
        }
        if let Some(speed) = self.speed() {
            rows.push(("Tokens per second (TPS)", speed));
        }
        rows
    }
    /// The pill's and dialog's speed reading: one decimal below ten tokens a second, whole numbers
    /// from there (`formatTokensPerSecond`, `message-chrome.ts:66-80`).
    pub fn speed(&self) -> Option<String> {
        if self.decode_ms <= 0 || self.decode_tokens == 0 {
            return None;
        }
        let tps = self.decode_tokens as f64 / (self.decode_ms as f64 / 1_000.0);
        if !tps.is_finite() || tps < 0.0 {
            return None;
        }
        Some(if tps >= 10.0 {
            format!("{} tok/s", tps.round() as u64)
        } else {
            format!("{tps:.1} tok/s")
        })
    }
    /// The reference's `{turns} turns {steps} steps` pill label (`stats.counts`).
    pub fn counts(&self) -> String {
        format!(
            "{} turn{} {} step{}",
            self.turns,
            if self.turns == 1 { "" } else { "s" },
            self.steps,
            if self.steps == 1 { "" } else { "s" }
        )
    }
    /// Whether any figure is worth a dialog: a window with nothing timed keeps the pill a plain
    /// reading (`StatsPills.tsx:246-248`).
    pub fn has_dialog(&self) -> bool {
        !self.rows().is_empty()
    }
}

/// The reference's `formatDuration`: `45.2s` under a minute, `2m42s` from there on
/// (`StatsPills.tsx:84-97`).
pub fn duration(ms: i64) -> String {
    let seconds = ms.max(0) as f64 / 1_000.0;
    if seconds < 60.0 {
        let tenths = (seconds * 10.0).round() / 10.0;
        return format!("{tenths:.1}s");
    }
    let whole = seconds.round() as i64;
    format!("{}m{}s", whole / 60, whole % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn values(stats: Value) -> BTreeMap<String, Value> {
        let mut values = BTreeMap::new();
        values.insert("sessionStats".into(), stats);
        values
    }
    fn projection() -> Value {
        json!({"turns": 2, "steps": 3, "llmMs": 1500, "toolMs": 4500,
            "ttftMs": 900, "ttftSteps": 3, "decodeMs": 2000, "decodeTokens": 84})
    }

    #[test]
    fn the_projection_is_read_strictly() {
        let stats = Stats::from_projection(&values(projection())).expect("a complete projection");
        assert_eq!(stats.turns, 2);
        assert_eq!(stats.steps, 3);
        assert_eq!(stats.counts(), "2 turns 3 steps");
        // Any field missing or negative drops the whole reading.
        for broken in [
            json!({"turns": 1}),
            json!({"turns": -1, "steps": 1, "llmMs": 0, "toolMs": 0, "ttftMs": 0, "ttftSteps": 0, "decodeMs": 0, "decodeTokens": 0}),
            json!({"turns": 1, "steps": 1, "llmMs": 0, "toolMs": 0, "ttftMs": 0, "ttftSteps": 0, "decodeMs": 0, "decodeTokens": "8"}),
        ] {
            assert!(
                Stats::from_projection(&values(broken.clone())).is_none(),
                "{broken}"
            );
        }
        assert!(Stats::from_projection(&BTreeMap::new()).is_none());
    }

    #[test]
    fn the_rows_skip_what_nobody_recorded() {
        let stats = Stats::from_projection(&values(projection())).unwrap();
        assert_eq!(
            stats.rows(),
            vec![
                ("LLM time", "1.5s".to_owned()),
                ("Tool time", "4.5s".to_owned()),
                // `formatDuration` is seconds-based throughout; a sub-second figure reads `0.3s`.
                ("Avg time to first token (TTFT)", "0.3s".to_owned()),
                ("Tokens per second (TPS)", "42 tok/s".to_owned()),
            ]
        );
        // Only a turn/step count: the pill stays a plain reading.
        let quiet = Stats::from_projection(&values(json!({"turns": 1, "steps": 1, "llmMs": 0,
            "toolMs": 0, "ttftMs": 0, "ttftSteps": 0, "decodeMs": 0, "decodeTokens": 0})))
        .unwrap();
        assert!(quiet.rows().is_empty());
        assert!(!quiet.has_dialog());
        assert_eq!(quiet.counts(), "1 turn 1 step");
        assert_eq!(quiet.speed(), None);
    }

    #[test]
    fn the_duration_and_speed_follow_the_reference() {
        assert_eq!(duration(0), "0.0s");
        assert_eq!(duration(45_200), "45.2s");
        assert_eq!(duration(59_940), "59.9s");
        assert_eq!(duration(162_000), "2m42s");
        assert_eq!(duration(3_600_000), "60m0s");
        // Speed: one decimal below ten, whole numbers above.
        let slow = Stats {
            decode_ms: 2_000,
            decode_tokens: 9,
            ..Stats::from_projection(&values(projection())).unwrap()
        };
        assert_eq!(slow.speed().as_deref(), Some("4.5 tok/s"));
        let fast = Stats {
            decode_ms: 1_000,
            decode_tokens: 12,
            ..slow
        };
        assert_eq!(fast.speed().as_deref(), Some("12 tok/s"));
        // No decode time or no tokens is no reading at all.
        let none = Stats {
            decode_ms: 0,
            ..fast
        };
        assert_eq!(none.speed(), None);
    }
}
