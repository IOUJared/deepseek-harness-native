//! The Host's `turnOutline` projection: every Turn of a session, so the transcript's rail can mark
//! the Turns a client's paged window does not hold (`session-turn-outline/src/projection.ts:1-30`,
//! whose wire view is the entries array). Its previews are bounded to the rail card's clamps; this
//! module carries the identity and the load-through seq the mark needs, and the previews land with
//! the card (`NATIVE-UI-PARITY.md`, round 518).
use serde_json::Value;
use std::collections::BTreeMap;

/// One Turn the outline names: its number, the seq a window paged back through it would start at
/// (`turn/start` anchors every entry, `projection.ts:5-10`), and the bounded previews the rail's card
/// shows (`projection.ts:32-34`: 50 characters of prompt, 120 of response).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineTurn {
    pub turn: u64,
    pub seq: u64,
    pub prompt: String,
    pub response: String,
}

/// A preview bound the wire already enforces; loaded Turns reuse it so a card states the same words
/// before and after its events load (`turn-navigation.ts:8-14`).
pub const PROMPT_LIMIT: usize = 50;
pub const RESPONSE_LIMIT: usize = 120;

/// The reference's preview fold: space-join the text, collapse whitespace, and cap at the limit with
/// a trailing ellipsis (`projection.ts:37-55`).
pub fn preview(text: &str, limit: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() > limit {
        let kept: String = normalized.chars().take(limit.saturating_sub(1)).collect();
        format!("{}…", kept.trim_end())
    } else {
        normalized
    }
}

/// Decode the projection's wire view. A damaged entry is dropped — a mark cannot exist without its
/// Turn and seq — and the result stays strictly ascending by Turn, as the wire schema requires
/// (`projection.ts:61-82`).
pub fn turns(projections: &BTreeMap<String, Value>) -> Vec<OutlineTurn> {
    let Some(rows) = projections.get("turnOutline").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut turns: Vec<OutlineTurn> = Vec::new();
    for row in rows {
        let (Some(turn), Some(seq)) = (
            row.get("turn").and_then(Value::as_u64),
            row.get("seq").and_then(Value::as_u64),
        ) else {
            continue;
        };
        if turns.last().is_some_and(|last| last.turn >= turn) {
            continue;
        }
        turns.push(OutlineTurn {
            turn,
            seq,
            prompt: row
                .get("prompt")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_default(),
            response: row
                .get("response")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_default(),
        });
    }
    turns
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn projections(value: Value) -> BTreeMap<String, Value> {
        let mut projections = BTreeMap::new();
        projections.insert("turnOutline".into(), value);
        projections
    }

    #[test]
    fn the_outline_names_its_turns_in_ascending_order_with_their_load_through_seq() {
        let turns = turns(&projections(json!([
            {"turn": 1, "seq": 0, "prompt": "PUBLIC prompt", "response": "PUBLIC answer"},
            {"turn": 2, "seq": 9, "prompt": "", "response": ""},
        ])));
        assert_eq!(
            turns,
            vec![
                OutlineTurn {
                    turn: 1,
                    seq: 0,
                    prompt: "PUBLIC prompt".into(),
                    response: "PUBLIC answer".into(),
                },
                OutlineTurn {
                    turn: 2,
                    seq: 9,
                    prompt: String::new(),
                    response: String::new(),
                }
            ]
        );
    }

    #[test]
    fn a_preview_collapses_whitespace_and_caps_with_an_ellipsis() {
        assert_eq!(
            preview("  PUBLIC   spaced\n\ttext  ", PROMPT_LIMIT),
            "PUBLIC spaced text"
        );
        let long = "word ".repeat(40);
        let clipped = preview(&long, PROMPT_LIMIT);
        assert_eq!(clipped.chars().count(), PROMPT_LIMIT);
        assert!(clipped.ends_with('…'));
        // A value exactly at the limit is not clipped.
        let exact = "x".repeat(PROMPT_LIMIT);
        assert_eq!(preview(&exact, PROMPT_LIMIT), exact);
    }

    #[test]
    fn a_damaged_or_out_of_order_entry_is_dropped_rather_than_misplacing_a_mark() {
        // A missing seq cannot anchor a jump; a repeated or descending Turn would stack marks.
        let turns = turns(&projections(json!([
            {"turn": 1, "seq": 0},
            {"turn": 2},
            {"turn": 2, "seq": 4},
            {"turn": 1, "seq": 7},
            {"seq": 8},
            {"turn": 3, "seq": 12},
        ])));
        // The entry without a seq drops; the descending Turn 1 after it drops; the good Turn 2
        // that follows the damaged one is still a mark.
        assert_eq!(
            turns
                .iter()
                .map(|turn| (turn.turn, turn.seq))
                .collect::<Vec<_>>(),
            vec![(1, 0), (2, 4), (3, 12)]
        );
    }

    #[test]
    fn an_absent_or_malformed_projection_states_no_turns() {
        assert!(turns(&BTreeMap::new()).is_empty());
        assert!(turns(&projections(json!({"turns": []}))).is_empty());
        assert!(turns(&projections(json!(null))).is_empty());
    }
}
