//! The settled turn's duration, as the reference states it above a closed turn's process content
//! (`TurnProcessNodeView.tsx:20-33`, `message-chrome.ts:54-64`, `locale.ts:264-266`).
//!
//! The duration is the recorded `turn/end` time minus the recorded `turn/start` time, floored at
//! one second — the reference's own `Math.max(1000, end - start)` — and it is *not* stated for a
//! cancelled or failed turn, whose tail says `Stopped` / `Failed` instead.
use serde_json::Value;

/// One settled turn's duration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Duration {
    pub seconds: u64,
}

/// One closed turn's process control: the label the reference states above the turn's process
/// content, and whether that content is always visible (`TurnProcessNodeView.tsx:19-33`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Control {
    pub label: String,
    /// A stopped or failed turn's process is never hidden (`turn-process.ts:70-75`,
    /// `ChatGroupSeat.tsx:146-147`).
    pub always_open: bool,
}

/// Read one turn's recorded start, end and end reason. Scans the retained records; the caller
/// runs this once per turn.
fn span<'a>(
    turn: u64,
    records: impl Iterator<Item = (&'a str, i64, &'a Value)>,
) -> (Option<i64>, Option<i64>, Option<String>) {
    let mut start = None;
    let mut end = None;
    let mut reason = None;
    for (kind, time, data) in records {
        if data["turn"].as_u64() != Some(turn) {
            continue;
        }
        match kind {
            "turn/start" => start = Some(time),
            "turn/end" => {
                end = Some(time);
                reason = data["reason"]["kind"].as_str().map(str::to_owned);
            }
            _ => {}
        }
    }
    (start, end, reason)
}

fn elapsed(start: i64, end: i64) -> Duration {
    Duration {
        seconds: ((end - start).max(1_000) / 1_000) as u64,
    }
}

/// Find the recorded start and end time of one turn and state its duration, when it settled
/// cleanly. Scans the retained records. The view states the duration through the process
/// control (`control`); this stays as that label's test and fixture oracle.
#[cfg(any(test, feature = "public-layout-fixture"))]
pub fn duration<'a>(
    turn: u64,
    records: impl Iterator<Item = (&'a str, i64, &'a Value)>,
) -> Option<Duration> {
    let (start, end, reason) = span(turn, records);
    let (start, end) = (start?, end?);
    if matches!(reason.as_deref(), Some("aborted" | "error")) {
        return None;
    }
    Some(elapsed(start, end))
}

/// One settled turn's process control, from its recorded end. `None` while the turn is still
/// open — the reference draws no control for a live turn (`TurnProcessNodeView.tsx:24`).
pub fn control<'a>(
    turn: u64,
    records: impl Iterator<Item = (&'a str, i64, &'a Value)>,
) -> Option<Control> {
    let (start, end, reason) = span(turn, records);
    let end = end?;
    let duration = start.map(|start| elapsed(start, end));
    Some(match reason.as_deref() {
        Some("aborted") => Control {
            label: "Stopped".into(),
            always_open: true,
        },
        Some("error") => Control {
            label: "Failed".into(),
            always_open: true,
        },
        _ => Control {
            label: label(duration),
            always_open: false,
        },
    })
}

/// The settled turn's tail label: `Completed in 2m 3s`, `Completed in 45s`, or the bare
/// `Completed` when no duration was recorded. The unit strings are the reference's own
/// (`duration.hourUnit` = `h `, `minuteUnit` = `m `, `secondUnit` = `s`).
pub fn label(duration: Option<Duration>) -> String {
    let Some(duration) = duration else {
        return "Completed".into();
    };
    let total = duration.seconds;
    let hours = total / 3_600;
    let minutes = (total / 60) % 60;
    let seconds = total % 60;
    let mut text = String::from("Completed in ");
    if hours > 0 {
        text.push_str(&format!("{hours}h "));
    }
    if total >= 60 {
        text.push_str(&format!("{minutes}m "));
    }
    text.push_str(&format!("{seconds}s"));
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record(kind: &str, turn: u64, time: i64, reason: Option<&str>) -> (String, i64, Value) {
        let mut data = json!({"turn": turn});
        if let Some(reason) = reason {
            data["reason"] = json!({"kind": reason});
        }
        (kind.to_owned(), time, data)
    }
    fn scan<'a>(
        records: &'a [(String, i64, Value)],
    ) -> impl Iterator<Item = (&'a str, i64, &'a Value)> {
        records
            .iter()
            .map(|(kind, time, data)| (kind.as_str(), *time, data))
    }

    #[test]
    fn a_stopped_or_failed_turn_states_a_control_that_never_folds() {
        let stopped = vec![
            record("turn/start", 1, 1_000, None),
            record("turn/end", 1, 61_000, Some("aborted")),
        ];
        let stated = control(1, scan(&stopped)).expect("a closed turn states a control");
        assert_eq!(stated.label, "Stopped");
        assert!(
            stated.always_open,
            "a stopped turn's process is never hidden"
        );
        let failed = vec![
            record("turn/start", 1, 1_000, None),
            record("turn/end", 1, 2_000, Some("error")),
        ];
        let stated = control(1, scan(&failed)).expect("a closed turn states a control");
        assert_eq!(stated.label, "Failed");
        assert!(stated.always_open);
        // A clean turn states its elapsed time and folds.
        let clean = vec![
            record("turn/start", 1, 1_000, None),
            record("turn/end", 1, 61_000, Some("completed")),
        ];
        let stated = control(1, scan(&clean)).expect("a closed turn states a control");
        assert_eq!(stated.label, "Completed in 1m 0s");
        assert!(!stated.always_open);
        // A live turn has no control at all (`TurnProcessNodeView.tsx:24`).
        assert!(control(1, scan(&[record("turn/start", 1, 1_000, None)])).is_none());
    }

    #[test]
    fn a_clean_turn_states_its_elapsed_time() {
        let records = vec![
            record("turn/start", 1, 1_000, None),
            record("turn/end", 1, 46_000, Some("stop")),
        ];
        assert_eq!(duration(1, scan(&records)).map(|d| d.seconds), Some(45));
        assert_eq!(label(duration(1, scan(&records))), "Completed in 45s");
    }

    #[test]
    fn the_floor_is_one_second_and_a_cancelled_or_failed_turn_states_none() {
        let quick = vec![
            record("turn/start", 1, 1_000, None),
            record("turn/end", 1, 1_050, Some("stop")),
        ];
        assert_eq!(duration(1, scan(&quick)).map(|d| d.seconds), Some(1));
        for reason in ["aborted", "error"] {
            let records = vec![
                record("turn/start", 1, 1_000, None),
                record("turn/end", 1, 9_000, Some(reason)),
            ];
            assert!(duration(1, scan(&records)).is_none(), "{reason}");
        }
        // A turn with no recorded end, or a different turn, states nothing.
        assert!(duration(1, scan(&[record("turn/start", 1, 1_000, None)])).is_none());
        let other = vec![
            record("turn/start", 2, 1_000, None),
            record("turn/end", 2, 9_000, Some("stop")),
        ];
        assert!(duration(1, scan(&other)).is_none());
        assert_eq!(label(None), "Completed");
    }

    #[test]
    fn the_units_follow_the_reference() {
        let at = |seconds: u64| label(Some(Duration { seconds }));
        assert_eq!(at(1), "Completed in 1s");
        assert_eq!(at(59), "Completed in 59s");
        assert_eq!(at(60), "Completed in 1m 0s");
        assert_eq!(at(123), "Completed in 2m 3s");
        assert_eq!(at(3_600), "Completed in 1h 0m 0s");
        assert_eq!(at(3_723), "Completed in 1h 2m 3s");
    }
}
