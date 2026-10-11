//! Synthetic alpha events: Host-confirmed input, failures, and conversation history.
use super::*;
use serde_json::json;

fn message(id: &str, text: &str) -> Value {
    json!({"id":id,"role":"user","source":{"kind":"user","rpcId":format!("PUBLIC-request-{id}"),"promptFingerprint":"PUBLIC-opaque-fingerprint"},"content":[{"type":"text","text":text}]})
}
fn event(seq: u64, kind: &str, data: Value) -> Value {
    json!({"type":"event","event":{"type":kind,"seq":seq,"time":1000+seq,"data":data}})
}
fn append_user(seq: u64, id: &str, text: &str) -> Value {
    let mut value = event(seq, "user/message", message(id, text));
    value["event"]["surfaceOp"] = json!("append");
    value
}
fn append_assistant(seq: u64, text: &str) -> Value {
    let mut value = event(
        seq,
        "assistant/message",
        json!({"turn":1,"step":1,"message":{"id":"PUBLIC-assistant","role":"assistant","source":{"kind":"model","provider":"PUBLIC","model":"PUBLIC"},"content":[{"type":"text","text":text}]},"stream":[{"type":"text-chunks","time0":1000,"index":0,"dt":[],"texts":[text]}]}),
    );
    value["event"]["surfaceOp"] = json!("append");
    value
}
fn insert(seq: u64, messages: Vec<Value>) -> Value {
    event(
        seq,
        "agent/inbox/spliced",
        json!({"target":"next-turn","start":0,"inserted":messages}),
    )
}
fn claim(seq: u64, count: u64) -> Value {
    event(
        seq,
        "agent/inbox/spliced",
        json!({"target":"next-turn","start":0,"removedCount":count,"inserted":[]}),
    )
}
fn apply(transcript: &mut Transcript, value: Value) -> Result<(), ReduceError> {
    transcript.apply(1, serde_json::from_value(value).unwrap())
}
fn opening(records: Vec<Value>) -> Transcript {
    let cursor = records
        .last()
        .map_or(-1, |record| record["event"]["seq"].as_i64().unwrap());
    let mut transcript = Transcript::new(1);
    apply(&mut transcript, json!({"type":"snapshot","header":{"version":4,"id":"PUBLIC-session","createdAt":1000,"isSeeded":false},"cursor":cursor,"records":records,"hasMore":false,"projections":{"asOfSeq":cursor,"values":{}},"assistantStream":{"revision":0}})).unwrap();
    transcript
}
fn human_texts(transcript: &Transcript) -> Vec<String> {
    transcript
        .display_rows(false)
        .filter(|row| row.role == "You")
        .map(|row| row.text.clone())
        .collect()
}
fn displayed(transcript: &Transcript, records: bool) -> String {
    transcript
        .display_rows(records)
        .map(|row| row.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
fn replace_user(seq: u64, id: &str, text: &str, start: u64, end: u64, sources: Vec<u64>) -> Value {
    let mut value = append_user(seq, id, text);
    value["event"]["surfaceOp"] = json!({"op":"replace","startSeq":start,"endSeq":end});
    value["event"]["sourceEventSeqs"] = json!(sources);
    value
}

#[test]
fn host_insertion_claim_and_preparation_failure_keep_the_submitted_message() {
    let mut transcript = opening(vec![]);
    apply(
        &mut transcript,
        insert(0, vec![message("PUBLIC-one", "PUBLIC submitted text")]),
    )
    .unwrap();
    assert_eq!(human_texts(&transcript), ["PUBLIC submitted text"]);
    apply(&mut transcript, event(1, "turn/start", json!({"turn":1}))).unwrap();
    apply(&mut transcript, claim(2, 1)).unwrap();
    apply(
        &mut transcript,
        event(3, "step/start", json!({"turn":1,"step":1})),
    )
    .unwrap();
    apply(
        &mut transcript,
        event(4, "step/end", json!({"turn":1,"step":1})),
    )
    .unwrap();
    apply(&mut transcript, event(5, "turn/end", json!({"turn":1,"reason":{"kind":"error","error":{"code":"PUBLIC_ROUTE","message":"PUBLIC route unavailable"}}}))).unwrap();
    assert_eq!(human_texts(&transcript), ["PUBLIC submitted text"]);
    assert!(displayed(&transcript, false).contains("PUBLIC route unavailable"));
    assert!(transcript.safe_to_send());
    assert!(transcript.partial().is_none());
}

#[test]
fn same_host_message_id_hands_off_to_canonical_durable_content() {
    let mut transcript = opening(vec![
        insert(0, vec![message("PUBLIC-one", "PUBLIC original")]),
        claim(1, 1),
    ]);
    apply(
        &mut transcript,
        append_user(2, "PUBLIC-one", "PUBLIC canonical rewrite"),
    )
    .unwrap();
    assert_eq!(human_texts(&transcript), ["PUBLIC canonical rewrite"]);
}

#[test]
fn equal_text_different_host_ids_do_not_deduplicate() {
    let transcript = opening(vec![
        insert(0, vec![message("PUBLIC-one", "PUBLIC repeated")]),
        append_user(1, "PUBLIC-two", "PUBLIC repeated"),
    ]);
    assert_eq!(
        human_texts(&transcript),
        ["PUBLIC repeated", "PUBLIC repeated"]
    );
}

#[test]
fn grouped_insertion_keeps_only_unmatched_ids_after_a_partial_commit() {
    let mut transcript = opening(vec![insert(
        0,
        vec![
            message("PUBLIC-one", "PUBLIC first"),
            message("PUBLIC-two", "PUBLIC second"),
            message("PUBLIC-three", "PUBLIC third"),
        ],
    )]);
    assert_eq!(
        human_texts(&transcript),
        ["PUBLIC first\nPUBLIC second\nPUBLIC third"]
    );
    assert_eq!(
        transcript
            .display_rows(false)
            .map(|row| row.key)
            .collect::<Vec<_>>(),
        [0]
    );
    apply(
        &mut transcript,
        append_user(1, "PUBLIC-one", "PUBLIC first canonical"),
    )
    .unwrap();
    let texts = human_texts(&transcript);
    assert_eq!(
        texts,
        ["PUBLIC second\nPUBLIC third", "PUBLIC first canonical"]
    );
    let queued: Vec<_> = transcript
        .display_rows(false)
        .filter(|row| row.key == 0)
        .collect();
    assert_eq!(
        queued.len(),
        1,
        "unmatched IDs share the retained insertion render key"
    );
    assert_eq!(queued[0].role, "You");
    apply(&mut transcript, claim(2, 3)).unwrap();
    assert_eq!(human_texts(&transcript), texts);
}

#[test]
fn opaque_prompt_fingerprint_never_becomes_a_chat_body_or_blocks_host_input() {
    let transcript = opening(vec![insert(
        0,
        vec![message("PUBLIC-one", "PUBLIC ordinary text")],
    )]);
    assert_eq!(human_texts(&transcript), ["PUBLIC ordinary text"]);
    for records in [false, true] {
        assert!(!displayed(&transcript, records).contains("PUBLIC-opaque-fingerprint"));
    }
    assert!(transcript.safe_to_send());
}

#[test]
fn confirmed_file_only_insertion_exposes_name_size_not_attachment_authority() {
    let queued = json!({"id":"PUBLIC-file","role":"user","source":{"kind":"user","rpcId":"PUBLIC-request","clientTimeZone":"UTC","promptFingerprint":"PUBLIC-opaque-fingerprint"},"content":[{"type":"file","attachment":{"attachmentId":"PUBLIC_DO_NOT_DISPLAY_ATTACHMENT","name":"PUBLIC-note.txt","bytes":12}}]});
    let transcript = opening(vec![insert(0, vec![queued]), claim(1, 1)]);
    assert_eq!(
        human_texts(&transcript),
        ["File · PUBLIC-note.txt · 12 bytes [preview unavailable]"]
    );
    for records in [false, true] {
        assert!(!displayed(&transcript, records).contains("PUBLIC_DO_NOT_DISPLAY_ATTACHMENT"));
        assert!(!displayed(&transcript, records).contains("PUBLIC-opaque-fingerprint"));
    }
    assert!(transcript.safe_to_send());
}

#[test]
fn cancellation_diagnostic_and_original_host_submission_remain_visible() {
    let transcript = opening(vec![
        insert(0, vec![message("PUBLIC-one", "PUBLIC canceled input")]),
        event(
            1,
            "agent/inbox/spliced",
            json!({"target":"next-turn","start":0,"removedCount":1,"inserted":[],"outcome":"canceled"}),
        ),
    ]);
    assert_eq!(human_texts(&transcript), ["PUBLIC canceled input"]);
    for records in [false, true] {
        assert!(
            transcript
                .display_rows(records)
                .any(|row| row.key == 1 && row.text.contains("canceled"))
        );
    }
    assert!(transcript.inbox_canceled(1));
}

#[test]
fn authentication_failure_uses_generic_notice_without_the_provider_message() {
    let transcript = opening(vec![event(
        0,
        "turn/end",
        json!({"turn":1,"reason":{"kind":"error","error":{"code":"AUTH","message":"PUBLIC_DO_NOT_DISPLAY_AUTH_MESSAGE"}}}),
    )]);
    for records in [false, true] {
        let rows = displayed(&transcript, records);
        assert!(!rows.is_empty());
        assert!(!rows.contains("PUBLIC_DO_NOT_DISPLAY_AUTH_MESSAGE"));
    }
    assert!(
        transcript
            .rows()
            .iter()
            .all(|row| !row.text.contains("PUBLIC_DO_NOT_DISPLAY_AUTH_MESSAGE"))
    );
    assert!(transcript.safe_to_send());
}

#[test]
fn completed_turn_is_quiet_but_noncompleted_turn_notices_remain_visible() {
    for kind in ["completed", "forked"] {
        let completed = opening(vec![event(
            0,
            "turn/end",
            json!({"turn":1,"reason":{"kind":kind}}),
        )]);
        for records in [false, true] {
            assert!(completed.display_rows(records).next().is_none());
        }
    }
    for reason in [
        json!({"kind":"blocked"}),
        json!({"kind":"max-tokens"}),
        json!({"kind":"interrupted"}),
        json!({"kind":"aborted","reason":{"kind":"user"}}),
    ] {
        let transcript = opening(vec![event(
            0,
            "turn/end",
            json!({"turn":1,"reason":reason}),
        )]);
        assert!(transcript.display_rows(false).next().is_some());
        assert!(transcript.safe_to_send());
    }
}

#[test]
fn reordered_surface_endpoints_validate_without_erasing_original_chat() {
    let transcript = opening(vec![
        append_user(0, "PUBLIC-one", "PUBLIC original user"),
        append_assistant(1, "PUBLIC original assistant"),
        replace_user(
            2,
            "PUBLIC-rewrite-one",
            "PUBLIC first replacement",
            0,
            0,
            vec![0],
        ),
        replace_user(
            3,
            "PUBLIC-rewrite-two",
            "PUBLIC second replacement",
            2,
            1,
            vec![2, 1],
        ),
    ]);
    assert!(transcript.safe_to_send());
    let chat = displayed(&transcript, false);
    assert!(chat.contains("PUBLIC original user"));
    assert!(chat.contains("PUBLIC original assistant"));
    assert!(!chat.contains("PUBLIC first replacement"));
    assert!(!chat.contains("PUBLIC second replacement"));
    let records = displayed(&transcript, true);
    assert!(records.contains("PUBLIC first replacement"));
    assert!(records.contains("PUBLIC second replacement"));
    for text in [
        "PUBLIC original user",
        "PUBLIC original assistant",
        "PUBLIC first replacement",
        "PUBLIC second replacement",
    ] {
        assert!(transcript.rows().iter().any(|row| row.text == text));
    }
}

#[test]
fn surface_validation_preserves_non_surface_tool_diagnostics() {
    let transcript = opening(vec![
        append_user(0, "PUBLIC-one", "PUBLIC original user"),
        event(
            1,
            "tool/call",
            json!({"turn":1,"step":1,"callId":"PUBLIC-call","name":"PUBLIC_tool","arguments":"{}"}),
        ),
        append_assistant(2, "PUBLIC original assistant"),
        {
            let mut result = event(
                3,
                "tool/result",
                json!({"turn":1,"step":1,"message":{"role":"tool","callId":"PUBLIC-call","isError":false,"content":[{"type":"text","text":"PUBLIC original tool result"}]}}),
            );
            result["event"]["surfaceOp"] = json!("append");
            result
        },
        replace_user(
            4,
            "PUBLIC-rewrite",
            "PUBLIC summary copy",
            0,
            3,
            vec![0, 2, 3],
        ),
    ]);
    assert!(transcript.safe_to_send());
    for records in [false, true] {
        assert!(
            transcript
                .display_rows(records)
                .any(|row| row.key == 1 && row.text.contains("PUBLIC_tool"))
        );
    }
    assert!(displayed(&transcript, false).contains("PUBLIC original assistant"));
    assert!(displayed(&transcript, false).contains("PUBLIC original tool result"));
    assert!(
        transcript
            .rows()
            .iter()
            .any(|row| row.key == 3 && row.text == "PUBLIC original tool result")
    );
}

#[test]
fn replacement_missing_a_current_surface_citation_still_blocks_send() {
    let mut transcript = opening(vec![
        append_user(0, "PUBLIC-one", "PUBLIC user"),
        append_assistant(1, "PUBLIC assistant"),
    ]);
    assert_eq!(
        apply(
            &mut transcript,
            replace_user(
                2,
                "PUBLIC-rewrite",
                "PUBLIC invalid replacement",
                0,
                1,
                vec![0]
            )
        ),
        Err(ReduceError::InvalidSurface)
    );
    assert!(!transcript.safe_to_send());
}

#[test]
fn exact_control_events_stay_hidden_without_suppressing_ordinary_discussion() {
    let text = "PUBLIC discussion of session/title, session/title-llm-request and model/selection";
    let transcript = opening(vec![
        event(0, "session/title", json!({"title":"PUBLIC_METADATA_TITLE"})),
        event(
            1,
            "session/title-llm-request",
            json!({"request":"PUBLIC_METADATA_REQUEST"}),
        ),
        event(
            2,
            "model/selection",
            json!({"provider":"PUBLIC_METADATA_PROVIDER","model":"PUBLIC_METADATA_MODEL"}),
        ),
        append_user(3, "PUBLIC-one", text),
        append_assistant(4, text),
    ]);
    for records in [false, true] {
        let rows = displayed(&transcript, records);
        assert!(!rows.contains("PUBLIC_METADATA"));
        assert_eq!(rows.matches(text).count(), 2);
    }
}

#[test]
fn paginated_replacement_keeps_known_boundary_order_and_human_history() {
    let mut transcript = Transcript::new(1);
    apply(&mut transcript, json!({"type":"snapshot","header":{"version":4,"id":"PUBLIC-session","createdAt":1000,"isSeeded":false},"cursor":11,"records":[append_user(10,"PUBLIC-ten","PUBLIC earlier input"),append_assistant(11,"PUBLIC retained answer")],"hasMore":true,"projections":{"asOfSeq":11,"values":{}},"assistantStream":{"revision":0}})).unwrap();
    apply(
        &mut transcript,
        replace_user(
            12,
            "PUBLIC-context-12",
            "PUBLIC summary",
            0,
            10,
            vec![0, 10],
        ),
    )
    .unwrap();
    apply(
        &mut transcript,
        replace_user(
            13,
            "PUBLIC-context-13",
            "PUBLIC next summary",
            12,
            11,
            vec![12, 11],
        ),
    )
    .unwrap();
    assert!(transcript.safe_to_send());
    assert_eq!(human_texts(&transcript), ["PUBLIC earlier input"]);
    assert!(displayed(&transcript, false).contains("PUBLIC retained answer"));
    // Paging later reconstructs the full model surface, without erasing chat.
    let earlier = (0..10)
        .map(|seq| {
            if seq == 0 {
                append_user(seq, "PUBLIC-zero", "PUBLIC first input")
            } else {
                event(seq, "plugin/progress", json!({"progress":seq}))
            }
        })
        .map(|mut value| {
            if value["event"]["type"] == "plugin/progress" {
                value["event"]["ignorable"] = json!(true);
            }
            serde_json::from_value(value).unwrap()
        })
        .collect();
    transcript
        .page(
            1,
            SessionPage {
                records: earlier,
                has_more: false,
            },
        )
        .unwrap();
    assert!(transcript.safe_to_send());
    assert_eq!(
        human_texts(&transcript),
        ["PUBLIC first input", "PUBLIC earlier input"]
    );
}

#[test]
fn invalid_snapshot_keeps_valid_chat_and_required_warning_visible() {
    let mut transcript = Transcript::new(1);
    let records = vec![
        append_user(0, "PUBLIC-one", "PUBLIC preserved input"),
        event(
            1,
            "future/required",
            json!({"warning":"PUBLIC required warning"}),
        ),
        replace_user(2, "PUBLIC-bad", "PUBLIC invalid context", 0, 0, vec![1]),
    ];
    let result = apply(
        &mut transcript,
        json!({"type":"snapshot","header":{"version":4,"id":"PUBLIC-session","createdAt":1000,"isSeeded":false},"cursor":2,"records":records,"hasMore":false,"projections":{"asOfSeq":2,"values":{}},"assistantStream":{"revision":0}}),
    );
    assert_eq!(result, Err(ReduceError::InvalidSurface));
    assert!(!transcript.safe_to_send());
    assert_eq!(human_texts(&transcript), ["PUBLIC preserved input"]);
    assert!(displayed(&transcript, false).contains("PUBLIC required warning"));
}

#[test]
fn malformed_assistant_record_does_not_erase_valid_snapshot_input() {
    let mut transcript = Transcript::new(1);
    let mut assistant = append_assistant(1, "PUBLIC unused");
    assistant["event"]["data"]["stream"] =
        json!([{"type":"text-chunks","index":0,"time0":1000,"dt":[],"texts":[1]}]);
    let result = apply(
        &mut transcript,
        json!({"type":"snapshot","header":{"version":4,"id":"PUBLIC-session","createdAt":1000,"isSeeded":false},"cursor":1,"records":[append_user(0,"PUBLIC-one","PUBLIC preserved input"),assistant],"hasMore":false,"projections":{"asOfSeq":1,"values":{}},"assistantStream":{"revision":0}}),
    );
    assert!(result.is_err());
    assert!(!transcript.safe_to_send());
    assert_eq!(human_texts(&transcript), ["PUBLIC preserved input"]);
    assert!(displayed(&transcript, false).contains("could not be decoded"));
}

#[test]
fn backend_provided_reasoning_and_output_stream_then_commit_once() {
    let mut transcript = opening(vec![append_user(0, "PUBLIC-one", "PUBLIC question")]);
    apply(&mut transcript, json!({"type":"assistant-stream","frame":{"type":"start","attemptId":"PUBLIC-attempt","revision":1,"startedAfterSeq":0,"turn":1,"step":1}})).unwrap();
    let chunks = [
        json!({"type":"reasoning-delta","index":0,"text":"PUBLIC supplied reasoning"}),
        json!({"type":"block-end","index":0,"block":{"type":"reasoning","text":"PUBLIC supplied reasoning"}}),
        json!({"type":"text-delta","index":1,"text":"PUBLIC supplied answer"}),
        json!({"type":"block-end","index":1,"block":{"type":"text","text":"PUBLIC supplied answer"}}),
    ];
    for (index, chunk) in chunks.iter().enumerate() {
        apply(&mut transcript, json!({"type":"assistant-stream","frame":{"type":"chunk","attemptId":"PUBLIC-attempt","revision":index+2,"index":index,"time":1000,"chunk":chunk}})).unwrap();
    }
    let expected = "PUBLIC supplied reasoning\nPUBLIC supplied answer";
    assert_eq!(transcript.partial().as_deref(), Some(expected));
    let mut durable = append_assistant(1, "PUBLIC supplied answer");
    durable["event"]["data"]["message"]["content"] = json!([{"type":"reasoning","text":"PUBLIC supplied reasoning"},{"type":"text","text":"PUBLIC supplied answer"}]);
    durable["event"]["data"]["stream"] = json!([{"type":"reasoning-chunks","time0":1000,"index":0,"dt":[],"texts":["PUBLIC supplied reasoning"]},{"type":"chunk","time":1000,"chunk":chunks[1]},{"type":"text-chunks","time0":1000,"index":1,"dt":[],"texts":["PUBLIC supplied answer"]},{"type":"chunk","time":1000,"chunk":chunks[3]}]);
    apply(&mut transcript, durable).unwrap();
    assert_eq!(
        transcript
            .display_rows(false)
            .filter(|row| row.role == "Assistant")
            .count(),
        0
    );
    apply(&mut transcript, json!({"type":"assistant-stream","frame":{"type":"end","attemptId":"PUBLIC-attempt","revision":6,"index":4,"outcome":{"kind":"committed","eventType":"assistant/message","seq":1}}})).unwrap();
    assert!(transcript.partial().is_none());
    let assistant: Vec<_> = transcript
        .display_rows(false)
        .filter(|row| row.role == "Assistant")
        .collect();
    assert_eq!(assistant.len(), 1);
    assert_eq!(assistant[0].text, expected);
    apply(
        &mut transcript,
        event(2, "step/end", json!({"turn":1,"step":1})),
    )
    .unwrap();
    assert_eq!(
        displayed(&transcript, false)
            .matches("PUBLIC supplied answer")
            .count(),
        1
    );
}

#[test]
fn credential_failures_name_the_native_fix_instead_of_the_provider_message() {
    // The backend's own text mentions the web Models page and the credential
    // reference; the native notice must be short and point at the native page.
    for (code, expected) in [
        (
            "MISSING_CREDENTIAL",
            "No API key stored for this model route. Add one in Settings → DeepSeek.",
        ),
        (
            "INVALID_CREDENTIAL",
            "The stored API key was rejected. Re-enter it in Settings → DeepSeek.",
        ),
    ] {
        let transcript = opening(vec![event(
            0,
            "turn/end",
            json!({"turn":1,"reason":{"kind":"error","error":{
                "code":code,
                "message":"llm-deepseek: no API key for provider route \"deepseek-official\"; store DEEPSEEK_API_KEY through the credentials service"
            }}}),
        )]);
        let rows = displayed(&transcript, false);
        assert!(rows.contains(expected), "{code} notice missing: {rows:?}");
        assert!(!rows.contains("DEEPSEEK_API_KEY"));
        assert!(!rows.contains("credentials service"));
        assert!(transcript.safe_to_send());
    }
}

#[test]
fn an_unrecognized_error_code_still_reports_its_code_and_message() {
    let transcript = opening(vec![event(
        0,
        "turn/end",
        json!({"turn":1,"reason":{"kind":"error","error":{"code":"RATE_LIMIT","message":"PUBLIC_OTHER"}}}),
    )]);
    let rows = displayed(&transcript, false);
    assert!(rows.contains("Agent error (RATE_LIMIT): PUBLIC_OTHER"), "{rows:?}");
}

#[test]
fn chat_hides_harness_context_injects_that_trajectory_keeps() {
    // Shapes taken from a real recorded session: the human prompt is the only
    // `user` source, while the runtime snapshot, time sample, skill catalog and
    // system prompt are producer-owned and belong to Trajectory.
    let transcript = opening(vec![
        event(
            0,
            "system/message",
            json!({"message":{"content":[{"type":"text","text":"PUBLIC system prompt"}]}}),
        ),
        event(
            1,
            "user/message",
            json!({"source":{"kind":"runtime-context","form":"snapshot"},"content":[{"type":"text","text":"PUBLIC runtime context snapshot"}]}),
        ),
        event(
            2,
            "user/message",
            json!({"source":{"kind":"time-context","form":"snapshot"},"content":[{"type":"text","text":"PUBLIC time sampled while preparing"}]}),
        ),
        event(
            3,
            "user/message",
            json!({"source":{"kind":"skill-catalog","form":"catalog","entries":[]},"content":[{"type":"text","text":"PUBLIC system-reminder skill catalog"}]}),
        ),
        event(
            4,
            "user/message",
            json!({"source":{"kind":"user","rpcId":"PUBLIC-rpc","promptFingerprint":"PUBLIC-fingerprint"},"content":[{"type":"text","text":"PUBLIC real question"}]}),
        ),
        event(
            5,
            "session-log-deepseek/delivery-accepted",
            json!({"sessionFormatVersion":4,"throughSeq":15}),
        ),
        append_assistant(6, "PUBLIC answer"),
    ]);
    let chat: Vec<String> = transcript
        .display_rows(false)
        .map(|row| row.text.clone())
        .collect();
    assert_eq!(chat, ["PUBLIC real question", "PUBLIC answer"]);
    // Nothing internal reaches the clean conversation.
    for hidden in [
        "PUBLIC system prompt",
        "PUBLIC runtime context snapshot",
        "PUBLIC time sampled while preparing",
        "PUBLIC system-reminder skill catalog",
    ] {
        assert!(!chat.iter().any(|text| text.contains(hidden)), "{hidden}");
    }
    // Trajectory still retains every one of them.
    let trajectory: Vec<String> = transcript
        .display_rows(true)
        .map(|row| row.text.clone())
        .collect();
    for kept in [
        "PUBLIC system prompt",
        "PUBLIC runtime context snapshot",
        "PUBLIC time sampled while preparing",
        "PUBLIC system-reminder skill catalog",
    ] {
        assert!(
            trajectory.iter().any(|text| text.contains(kept)),
            "{kept} missing from trajectory: {trajectory:?}"
        );
    }
    // Retained records are never discarded, only presented elsewhere.
    assert_eq!(transcript.rows().len(), 7);
}
