//! Display-only separation of supplied alpha reasoning and answer blocks.
use super::MAX_DISPLAY_BYTES;
use std::collections::BTreeMap;

/// Backend-supplied output only; combined UTF-8 bytes never exceed the display limit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssistantParts {
    pub reasoning: String,
    pub answer: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Reasoning,
    Answer,
}

#[derive(Default)]
pub(super) struct Block {
    pub text: String,
    // None leaves unknown or mixed blocks on the existing raw display path.
    pub kind: Option<Kind>,
}
impl Block {
    pub fn append_kind(&mut self, kind: Kind, new: bool) {
        self.kind = if new || self.kind == Some(kind) {
            Some(kind)
        } else {
            None
        };
    }
}

pub(super) fn project(blocks: &BTreeMap<u64, Block>) -> Option<AssistantParts> {
    if blocks.values().any(|block| block.kind.is_none()) {
        return None;
    }
    // Charge the raw fold's separators, even across the two display sections.
    let raw_bytes = blocks.values().map(|block| block.text.len()).sum::<usize>()
        + blocks.len().saturating_sub(1);
    let truncated = raw_bytes > MAX_DISPLAY_BYTES;
    let marker = "\n[Display truncated; native limit reached]";
    let mut remaining = MAX_DISPLAY_BYTES - if truncated { marker.len() } else { 0 };
    let mut reasoning = Vec::new();
    let mut answer = Vec::new();
    let mut last_kind = Kind::Answer;
    for (position, block) in blocks.values().enumerate() {
        if position > 0 {
            if remaining == 0 {
                break;
            }
            remaining -= 1;
        }
        let kind = block.kind?;
        let mut end = block.text.len().min(remaining);
        while !block.text.is_char_boundary(end) {
            end -= 1;
        }
        match kind {
            Kind::Reasoning => reasoning.push(block.text[..end].to_owned()),
            Kind::Answer => answer.push(block.text[..end].to_owned()),
        }
        last_kind = kind;
        remaining -= end;
        if end < block.text.len() {
            break;
        }
    }
    let mut parts = AssistantParts {
        reasoning: reasoning.join("\n"),
        answer: answer.join("\n"),
    };
    if truncated {
        match last_kind {
            Kind::Reasoning => parts.reasoning.push_str(marker),
            Kind::Answer => parts.answer.push_str(marker),
        }
    }
    debug_assert!(parts.reasoning.len() + parts.answer.len() <= MAX_DISPLAY_BYTES);
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reducer::{ReduceError, Transcript};
    use dsh_native_transport::dto::SessionFollowFrame;
    use serde_json::{Value, json};

    fn frame(value: Value) -> SessionFollowFrame {
        serde_json::from_value(value).unwrap()
    }
    fn apply(transcript: &mut Transcript, value: Value) {
        transcript.apply(1, frame(value)).unwrap();
    }
    fn snapshot(records: Vec<Value>, baseline: Value) -> Value {
        let cursor = records
            .last()
            .map_or(-1, |r| r["event"]["seq"].as_i64().unwrap());
        json!({"type":"snapshot","header":{"version":4,"id":"PUBLIC-parts","createdAt":1000,"isSeeded":false},"records":records,"cursor":cursor,"hasMore":false,"projections":{"asOfSeq":cursor,"values":{}},"assistantStream":baseline})
    }
    fn opening() -> Transcript {
        let mut transcript = Transcript::new(1);
        apply(&mut transcript, snapshot(vec![], json!({"revision":0})));
        transcript
    }
    fn start() -> Value {
        json!({"type":"assistant-stream","frame":{"type":"start","revision":1,"attemptId":"PUBLIC-a","startedAfterSeq":-1,"turn":1,"step":1}})
    }
    fn chunk(revision: u64, index: u64, chunk: Value) -> Value {
        json!({"type":"assistant-stream","frame":{"type":"chunk","revision":revision,"attemptId":"PUBLIC-a","index":index,"time":1000,"chunk":chunk}})
    }
    fn end(revision: u64, index: u64, outcome: Value) -> Value {
        json!({"type":"assistant-stream","frame":{"type":"end","revision":revision,"attemptId":"PUBLIC-a","index":index,"outcome":outcome}})
    }
    fn packed(index: u64, reasoning: bool, text: &str) -> Value {
        json!({"type":if reasoning {"reasoning-chunks"} else {"text-chunks"},"time0":1000,"index":index,"dt":[],"texts":[text]})
    }
    fn close(index: u64, kind: &str, text: &str) -> Value {
        json!({"type":"chunk","time":1000,"chunk":{"type":"block-end","index":index,"block":{"type":kind,"text":text}}})
    }
    fn assistant(kind: &str, seq: u64, stream: Vec<Value>) -> Value {
        let mut event = json!({"type":"event","event":{"type":kind,"seq":seq,"time":1000,"data":{"turn":1,"step":1,"stream":stream}}});
        if kind == "assistant/message" {
            event["event"]["surfaceOp"] = json!("append");
        }
        event
    }
    fn parts(reasoning: &str, answer: &str) -> Option<AssistantParts> {
        Some(AssistantParts {
            reasoning: reasoning.into(),
            answer: answer.into(),
        })
    }

    #[test]
    fn supplied_reasoning_fixture_splits_without_changing_legacy_text() {
        let fixture: SessionFollowFrame =
            serde_json::from_str(include_str!("../fixtures/chat-visibility.json")).unwrap();
        let mut transcript = Transcript::new(1);
        transcript.apply(1, fixture).unwrap();
        assert_eq!(
            transcript.assistant_parts(11),
            parts(
                "PUBLIC backend-supplied reasoning",
                "PUBLIC assistant output"
            )
        );
        assert_eq!(
            transcript.rows().iter().find(|r| r.key == 11).unwrap().text,
            "PUBLIC backend-supplied reasoning\nPUBLIC assistant output"
        );
        for key in [3, 4, 5, 8, 12, 13, 100] {
            assert_eq!(transcript.assistant_parts(key), None);
        }
        assert!(transcript.safe_to_send());
    }

    #[test]
    fn live_reasoning_and_text_publish_canonical_settlement_exactly_once() {
        for kind in ["assistant/message", "assistant/attempt"] {
            let mut transcript = opening();
            apply(&mut transcript, start());
            apply(
                &mut transcript,
                chunk(
                    2,
                    0,
                    json!({"type":"reasoning-delta","index":0,"text":"PUBLIC thought"}),
                ),
            );
            apply(
                &mut transcript,
                chunk(
                    3,
                    1,
                    json!({"type":"text-delta","index":1,"text":"PUBLIC reply"}),
                ),
            );
            assert_eq!(
                transcript.partial_parts(),
                parts("PUBLIC thought", "PUBLIC reply")
            );
            assert_eq!(
                transcript.partial().as_deref(),
                Some("PUBLIC thought\nPUBLIC reply")
            );
            let durable = assistant(
                kind,
                0,
                vec![
                    packed(0, true, "PUBLIC canonical thought"),
                    close(0, "reasoning", "PUBLIC canonical thought"),
                    packed(1, false, "PUBLIC canonical reply"),
                    close(1, "text", "PUBLIC canonical reply"),
                ],
            );
            apply(&mut transcript, durable.clone());
            assert!(transcript.rows().is_empty());
            assert_eq!(transcript.assistant_parts(0), None);
            assert_eq!(
                transcript.partial_parts(),
                parts("PUBLIC thought", "PUBLIC reply")
            );
            apply(
                &mut transcript,
                end(4, 2, json!({"kind":"committed","seq":0,"eventType":kind})),
            );
            apply(&mut transcript, durable);
            assert_eq!(transcript.rows().len(), 1);
            assert_eq!(
                transcript.assistant_parts(0),
                parts("PUBLIC canonical thought", "PUBLIC canonical reply")
            );
            assert_eq!(
                transcript.rows()[0].text,
                "PUBLIC canonical thought\nPUBLIC canonical reply"
            );
            assert!(transcript.partial().is_none());
            assert!(transcript.partial_parts().is_none());
        }
    }

    #[test]
    fn reconnect_compact_baseline_continues_then_commits_without_prefix_duplication() {
        let stream = vec![
            packed(0, true, "PUBLIC thought"),
            packed(1, false, "PUBLIC reply"),
        ];
        let mut transcript = Transcript::new(1);
        apply(
            &mut transcript,
            snapshot(
                vec![],
                json!({"revision":8,"activeAttempt":{"attemptId":"PUBLIC-a","startedAfterSeq":-1,"turn":1,"step":1,"nextIndex":2,"stream":stream}}),
            ),
        );
        assert_eq!(
            transcript.partial_parts(),
            parts("PUBLIC thought", "PUBLIC reply")
        );
        apply(
            &mut transcript,
            chunk(9, 2, json!({"type":"text-delta","index":1,"text":"!"})),
        );
        apply(
            &mut transcript,
            assistant(
                "assistant/message",
                0,
                vec![
                    packed(0, true, "PUBLIC thought"),
                    packed(1, false, "PUBLIC reply!"),
                ],
            ),
        );
        apply(
            &mut transcript,
            end(
                10,
                3,
                json!({"kind":"committed","seq":0,"eventType":"assistant/message"}),
            ),
        );
        assert_eq!(transcript.rows().len(), 1);
        assert_eq!(
            transcript.assistant_parts(0),
            parts("PUBLIC thought", "PUBLIC reply!")
        );
        assert!(transcript.partial_parts().is_none());
    }

    #[test]
    fn reasoning_only_and_plain_text_do_not_invent_an_answer_or_thoughts() {
        for (reasoning, text) in [
            (true, "PUBLIC only reasoning"),
            (false, "<think>PUBLIC literal text</think>"),
        ] {
            let mut transcript = opening();
            apply(&mut transcript, start());
            apply(
                &mut transcript,
                chunk(
                    2,
                    0,
                    json!({"type":if reasoning {"reasoning-delta"} else {"text-delta"},"index":0,"text":text}),
                ),
            );
            let expected = if reasoning {
                parts(text, "")
            } else {
                parts("", text)
            };
            assert_eq!(transcript.partial_parts(), expected);
            apply(
                &mut transcript,
                assistant("assistant/attempt", 0, vec![packed(0, reasoning, text)]),
            );
            apply(
                &mut transcript,
                end(
                    3,
                    1,
                    json!({"kind":"committed","seq":0,"eventType":"assistant/attempt"}),
                ),
            );
            assert_eq!(transcript.assistant_parts(0), expected);
        }
        let mut transcript = opening();
        apply(&mut transcript, start());
        assert_eq!(transcript.partial_parts(), None);
    }

    #[test]
    fn stale_generation_abandonment_and_retained_settlement_suppress_partial_parts() {
        let mut transcript = opening();
        transcript.apply(0, frame(start())).unwrap();
        assert_eq!(transcript.partial_parts(), None);
        apply(&mut transcript, start());
        apply(
            &mut transcript,
            chunk(
                2,
                0,
                json!({"type":"reasoning-delta","index":0,"text":"PUBLIC thought"}),
            ),
        );
        transcript
            .apply(
                0,
                frame(chunk(
                    3,
                    1,
                    json!({"type":"text-delta","index":1,"text":"PUBLIC stale"}),
                )),
            )
            .unwrap();
        assert_eq!(transcript.partial_parts(), parts("PUBLIC thought", ""));
        apply(&mut transcript, end(3, 1, json!({"kind":"abandoned"})));
        assert_eq!(transcript.partial_parts(), None);
        assert!(transcript.rows().is_empty());

        let stream = vec![
            packed(0, true, "PUBLIC thought"),
            packed(1, false, "PUBLIC reply"),
        ];
        apply(
            &mut transcript,
            snapshot(
                vec![assistant("assistant/message", 0, stream.clone())],
                json!({"revision":8,"activeAttempt":{"attemptId":"PUBLIC-a","startedAfterSeq":0,"turn":1,"step":1,"nextIndex":2,"stream":stream}}),
            ),
        );
        assert_eq!(transcript.partial(), None);
        assert_eq!(transcript.partial_parts(), None);
        assert_eq!(
            transcript.assistant_parts(0),
            parts("PUBLIC thought", "PUBLIC reply")
        );
        apply(
            &mut transcript,
            end(
                9,
                2,
                json!({"kind":"committed","seq":0,"eventType":"assistant/message"}),
            ),
        );
        assert_eq!(transcript.rows().len(), 1);
        assert_eq!(transcript.partial_parts(), None);
    }

    #[test]
    fn block_end_replaces_deltas_and_each_section_keeps_existing_block_index_order() {
        let stream = vec![
            packed(3, false, "PUBLIC answer tail"),
            packed(2, true, "PUBLIC thought tail"),
            packed(1, false, "PUBLIC answer prefix"),
            packed(0, true, "PUBLIC provisional"),
            close(0, "reasoning", "PUBLIC final thought"),
        ];
        let mut transcript = opening();
        apply(&mut transcript, assistant("assistant/attempt", 0, stream));
        assert_eq!(
            transcript.assistant_parts(0),
            parts(
                "PUBLIC final thought\nPUBLIC thought tail",
                "PUBLIC answer prefix\nPUBLIC answer tail"
            )
        );
        assert_eq!(
            transcript.rows()[0].text,
            "PUBLIC final thought\nPUBLIC answer prefix\nPUBLIC thought tail\nPUBLIC answer tail"
        );
        let mut live = opening();
        apply(&mut live, start());
        apply(
            &mut live,
            chunk(
                2,
                0,
                json!({"type":"reasoning-delta","index":0,"text":"PUBLIC provisional"}),
            ),
        );
        apply(
            &mut live,
            chunk(
                3,
                1,
                json!({"type":"block-end","index":0,"block":{"type":"reasoning","text":"PUBLIC final thought"}}),
            ),
        );
        assert_eq!(live.partial_parts(), parts("PUBLIC final thought", ""));
        assert_eq!(live.partial().as_deref(), Some("PUBLIC final thought"));
    }

    #[test]
    fn a_recorded_tool_call_stays_internal_and_never_joins_the_answer() {
        let stream = vec![
            packed(0, true, "PUBLIC thought"),
            json!({"type":"tool-call-chunks","time0":1000,"index":1,"id":"PUBLIC-call","name":"read","dt":[1],"args":["{", "}"]}),
            json!({"type":"chunk","time":1002,"chunk":{"type":"block-end","index":1,"block":{"type":"tool-call","id":"PUBLIC-call","name":"read","arguments":"{}"}}}),
        ];
        let mut transcript = opening();
        apply(&mut transcript, assistant("assistant/attempt", 0, stream));
        let parts = transcript.assistant_parts(0).expect("parts");
        // A tool call is model-internal output: it groups with the thinking
        // rather than rendering as the assistant's prose answer.
        assert_eq!(parts.answer, "");
        assert!(
            parts.reasoning.contains("PUBLIC thought"),
            "{:?}",
            parts.reasoning
        );
        assert!(
            parts.reasoning.contains("Tool read: "),
            "{:?}",
            parts.reasoning
        );
        // Retention is unchanged: the recorded text still carries it for Trajectory.
        assert_eq!(transcript.rows()[0].text, "PUBLIC thought\nTool read: {}");
    }

    #[test]
    fn unknown_mixed_and_model_surface_records_keep_raw_fallback_without_inference() {
        let mut transcript = opening();
        let unknown = json!({"type":"chunk","time":1000,"chunk":{"type":"block-end","index":0,"block":{"type":"PUBLIC-future-reasoning","text":"PUBLIC unknown","token":"PUBLIC-redact"}}});
        apply(
            &mut transcript,
            assistant("assistant/attempt", 0, vec![unknown]),
        );
        assert_eq!(transcript.assistant_parts(0), None);
        assert!(transcript.rows()[0].text.contains("PUBLIC unknown"));
        assert!(!transcript.rows()[0].text.contains("PUBLIC-redact"));
        apply(
            &mut transcript,
            assistant(
                "assistant/attempt",
                1,
                vec![
                    packed(0, true, "PUBLIC thought"),
                    packed(0, false, "PUBLIC mixed"),
                ],
            ),
        );
        assert_eq!(transcript.assistant_parts(1), None);
        assert!(transcript.safe_to_send());
        let mut metadata = assistant(
            "assistant/attempt",
            2,
            vec![packed(0, false, "PUBLIC answer")],
        );
        metadata["event"]["data"]["message"] = json!({"content":[{"type":"reasoning","text":"PUBLIC ignored content"}],"reasoning":"PUBLIC ignored metadata"});
        apply(&mut transcript, metadata);
        assert_eq!(transcript.assistant_parts(2), parts("", "PUBLIC answer"));
        let mut replacement = assistant(
            "assistant/message",
            3,
            vec![packed(0, true, "PUBLIC context")],
        );
        replacement["event"]["surfaceOp"] = json!({"op":"replace","startSeq":3,"endSeq":3});
        // Invalid model-surface metadata still uses the existing safety verifier.
        assert!(transcript.apply(1, frame(replacement)).is_err());
        assert_eq!(transcript.assistant_parts(3), None);
        assert!(!transcript.safe_to_send());
    }

    #[test]
    fn shared_raw_byte_and_block_bounds_remain_enforced() {
        let thought = "界".repeat(4096);
        let answer = "x".repeat(MAX_DISPLAY_BYTES - thought.len() - 1);
        let mut transcript = opening();
        apply(
            &mut transcript,
            assistant(
                "assistant/attempt",
                0,
                vec![packed(0, true, &thought), packed(1, false, &answer)],
            ),
        );
        let parts = transcript.assistant_parts(0).unwrap();
        assert_eq!(
            parts.reasoning.len() + parts.answer.len(),
            MAX_DISPLAY_BYTES - 1
        );
        assert_eq!(transcript.rows()[0].text.len(), MAX_DISPLAY_BYTES);
        let mut oversized = opening();
        assert_eq!(
            oversized.apply(
                1,
                frame(assistant(
                    "assistant/attempt",
                    0,
                    vec![packed(0, true, &thought), packed(1, false, &(answer + "x"))]
                ))
            ),
            Err(ReduceError::RetentionLimit)
        );
        assert_eq!(oversized.assistant_parts(0), None);
        assert!(!oversized.safe_to_send());
        let mut many = opening();
        let stream = (0..257).map(|i| packed(i, i % 2 == 0, "x")).collect();
        assert_eq!(
            many.apply(1, frame(assistant("assistant/attempt", 0, stream))),
            Err(ReduceError::RetentionLimit)
        );
        assert_eq!(many.assistant_parts(0), None);
    }

    #[test]
    fn rejected_oversized_live_block_end_still_exposes_explicit_bounded_partial_notice() {
        let mut transcript = opening();
        apply(&mut transcript, start());
        apply(
            &mut transcript,
            chunk(
                2,
                0,
                json!({"type":"reasoning-delta","index":0,"text":"PUBLIC thought"}),
            ),
        );
        let large = "界".repeat(MAX_DISPLAY_BYTES);
        assert_eq!(
            transcript.apply(
                1,
                frame(chunk(
                    3,
                    1,
                    json!({"type":"block-end","index":1,"block":{"type":"text","text":large}})
                ))
            ),
            Err(ReduceError::RetentionLimit)
        );
        let parts = transcript.partial_parts().unwrap();
        assert_eq!(parts.reasoning, "PUBLIC thought");
        assert!(
            parts
                .answer
                .ends_with("[Display truncated; native limit reached]")
        );
        assert!(parts.reasoning.len() + parts.answer.len() <= MAX_DISPLAY_BYTES);
        // The new semantic API does not rewrite the legacy partial on a failed fold.
        assert!(transcript.partial().unwrap().len() > MAX_DISPLAY_BYTES);
        assert!(!transcript.safe_to_send());
    }

    #[test]
    fn a_tool_call_never_becomes_answer_prose() {
        // A tool call is model-internal output: the recorded stream still holds
        // it for Trajectory, but the answer body must not render `Tool …: {}`.
        let mut transcript = Transcript::new(1);
        let value = json!({"type":"chunk","time":1000,"chunk":{"type":"block-end","index":0,
            "block":{"type":"tool-call","id":"PUBLIC-call","name":"bash",
                "arguments":"{\"command\":\"date -u\"}"}}});
        // A stored tool call is a `tool-call-chunks` record whose members live
        // under `args`, exactly as a real session records them.
        let tool_call = json!({"type":"tool-call-chunks","time0":1000,"index":0,"id":"PUBLIC-call",
            "name":"bash","dt":[],"args":["{\"command\":\"date -u\"}"]});
        let fixture: SessionFollowFrame = serde_json::from_value(json!({"type":"snapshot",
            "records":[assistant("assistant/message", 0, vec![tool_call, value])],
            "cursor":0,"hasMore":false,
            "header":{"version":4,"id":"PUBLIC-session","createdAt":1000,"isSeeded":false},
            "projections":{"asOfSeq":0,"values":{}},"assistantStream":{"revision":0}}))
        .unwrap();
        transcript.apply(1, fixture).unwrap();
        let parts = transcript.assistant_parts(0).expect("parts");
        assert_eq!(parts.answer, "", "a tool call is not an answer");
        assert!(
            parts.reasoning.contains("Tool bash: "),
            "{:?}",
            parts.reasoning
        );
        assert!(parts.reasoning.contains("date -u"), "{:?}", parts.reasoning);
        // The recorded text is untouched, so Trajectory still shows it.
        let row = transcript.rows().iter().find(|r| r.key == 0).unwrap();
        assert!(row.text.contains("Tool bash: "), "{:?}", row.text);
    }
}
