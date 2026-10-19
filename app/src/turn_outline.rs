//! The Host's `turnOutline` projection: every Turn of a session, so the transcript's rail can mark
//! the Turns a client's paged window does not hold (`session-turn-outline/src/projection.ts:1-30`,
//! whose wire view is the entries array). Its previews are bounded to the rail card's clamps; this
//! module carries the identity and the load-through seq the mark needs, and the previews land with
//! the card (`NATIVE-UI-PARITY.md`, round 518).
use serde_json::Value;
use std::collections::BTreeMap;

/// One Turn the outline names: its number and the seq a window paged back through it would start
/// at (`turn/start` anchors every entry, `projection.ts:5-10`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutlineTurn {
    pub turn: u64,
    pub seq: u64,
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
        turns.push(OutlineTurn { turn, seq });
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
                OutlineTurn { turn: 1, seq: 0 },
                OutlineTurn { turn: 2, seq: 9 }
            ]
        );
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
            turns,
            vec![
                OutlineTurn { turn: 1, seq: 0 },
                OutlineTurn { turn: 2, seq: 4 },
                OutlineTurn { turn: 3, seq: 12 }
            ]
        );
    }

    #[test]
    fn an_absent_or_malformed_projection_states_no_turns() {
        assert!(turns(&BTreeMap::new()).is_empty());
        assert!(turns(&projections(json!({"turns": []}))).is_empty());
        assert!(turns(&projections(json!(null))).is_empty());
    }
}
