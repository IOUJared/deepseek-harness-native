//! Presentation-only diagnostics; retained events and Send authority stay unchanged.
use serde_json::Value;
pub fn ui_control_event(kind: &str) -> bool {
    matches!(
        kind,
        "session/title" | "session/title-llm-request" | "model/selection"
    )
}
/// Whether the Chat view presents an event of this kind with this role.
///
/// The two inputs are deliberately separate. A kind that this build does not
/// recognize stays visible, so a future required diagnostic is never silently
/// dropped (`future/required` is the standing case). A recognized bookkeeping
/// kind is never presented, whatever label a caller hands it — that is why the
/// role alone cannot be the input: `LOG_EVENTS` asks about kinds, and a caller
/// that passes a display label asks a question the list cannot answer.
///
/// The explicit role labels cover the decisions a dedicated reducer branch makes
/// from the event's own producer: a human prompt, an inject, the loop guard's
/// reminder, a model-context replacement.
pub fn chat_visible(kind: &str, role: &str) -> bool {
    // A cancellation or a claimed submission shares the splice kind with a
    // routine queue outcome, so that one kind keeps its own filter.
    if kind == "agent/inbox/spliced" {
        return true;
    }
    // A recognized bookkeeping kind is never conversation. This is checked
    // before any role label, so no caller can relabel one into Chat.
    if crate::known::LOG_EVENTS.contains(&kind) {
        return false;
    }
    // Message-form kinds: the branch that folded the event decided the role.
    !matches!(
        role,
        "System"
            | "Developer"
            | "Model context update"
            | "Injected context"
            | "Context elided"
            | "Context summarized"
            | "Loop guard"
    )
}
/// True when a logged message is human history rather than a harness inject.
/// `MessageSourceMap` is merge-extensible, but every producer declares its `kind`
/// on the wire: a real session records the human prompt as `user` and each
/// context inject as its own kind (`runtime-context`, `time-context`,
/// `skill-catalog`, `session-reference`, …). A message carrying no kind is
/// therefore not a recognized inject, and Chat keeps it rather than risk hiding
/// a real user message. Trajectory keeps every kind, including unknown ones.
pub fn human_message(source: &Value) -> bool {
    matches!(source["kind"].as_str(), Some("user") | None)
}
/// A non-human source that can begin a turn, using the reference's own titles
/// (`ui-chat/src/client/chat/turn-trigger.ts:25-66`, `locale.ts`). Only producers the
/// reference attributes are listed; an unknown kind stays hidden, and only a context
/// message the Host has actually claimed as a waking turn is a trigger there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trigger {
    pub title: &'static str,
}

pub fn turn_trigger(source: &Value) -> Option<Trigger> {
    let kind = source["kind"].as_str()?;
    let title = match kind {
        "goal" => "Continuing goal",
        "agent-message" => "Task message received",
        "team-message" => "Team message received",
        "subagent-settled" => "Subtask status updated",
        "webhook" => {
            if source["provider"].as_str() == Some("github") {
                "GitHub event received"
            } else {
                "External event received"
            }
        }
        "schedule" => "Automation task",
        "tool-jobs" => "Background task updated",
        "cordis-host-runner" => "Plugin status updated",
        _ => return None,
    };
    Some(Trigger { title })
}

pub fn routine_inbox(data: &Value) -> bool {
    let Some(object) = data.as_object() else {
        return false;
    };
    // Cancellation, future outcomes/fields and malformed diagnostics stay visible.
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "target" | "start" | "inserted" | "removedCount"
        )
    }) || !matches!(data["target"].as_str(), Some("next-turn" | "next-step"))
        || data["start"]
            .as_u64()
            .is_none_or(|n| n > 9_007_199_254_740_991)
        || object.get("removedCount").is_some_and(|v| {
            v.as_u64()
                .is_none_or(|n| n == 0 || n > 9_007_199_254_740_991)
        })
    {
        return false;
    }
    let Some(inserted) = data["inserted"].as_array() else {
        return false;
    };
    // Only actual queue insertion/removal, not a no-op/future payload.
    (object.contains_key("removedCount") || !inserted.is_empty())
        && inserted.iter().all(|message| {
            message.as_object().is_some_and(|m| m.len() == 4)
                && message["id"].as_str().is_some_and(|id| !id.is_empty())
                && message["role"] == "user"
                && message["source"].as_object().is_some_and(|s| {
                    s.keys().all(|k| {
                        matches!(
                            k.as_str(),
                            "kind" | "rpcId" | "clientTimeZone" | "promptFingerprint"
                        )
                    }) && message["source"]["kind"] == "user"
                        && s.get("rpcId")
                            .is_none_or(|v| v.as_str().is_some_and(|id| !id.is_empty()))
                        && s.get("clientTimeZone").is_none_or(Value::is_string)
                        && s.get("promptFingerprint").is_none_or(Value::is_string)
                })
                && message["content"].as_array().is_some_and(|parts| {
                    !parts.is_empty()
                        && parts.iter().all(|part| {
                            let Some(p) = part.as_object() else {
                                return false;
                            };
                            match part["type"].as_str() {
                                Some("text") => p.len() == 2 && part["text"].is_string(),
                                Some("file") => {
                                    p.len() == 2
                                        && part["attachment"]
                                            .as_object()
                                            .is_some_and(|a| a.len() == 3)
                                        && part["attachment"]["attachmentId"]
                                            .as_str()
                                            .is_some_and(|s| !s.is_empty())
                                        && part["attachment"]["name"]
                                            .as_str()
                                            .is_some_and(|s| !s.is_empty())
                                        && part["attachment"]["bytes"]
                                            .as_u64()
                                            .is_some_and(|n| n <= 9_007_199_254_740_991)
                                }
                                _ => false,
                            }
                        })
                })
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    /// The (kind, role) pairs the reducer actually produces. A harness
    /// bookkeeping kind stays out of Chat whatever label it carries — the
    /// regression that let `session-log-deepseek/*` and the system prompt render
    /// as JSON in the conversation.
    #[test]
    fn chat_classification_uses_the_kind_for_bookkeeping_and_the_role_for_messages() {
        for (kind, role) in [
            ("system/message", "System"),
            ("developer/message", "Developer"),
            ("user/message", "Injected context"),
            ("user/message", "Loop guard"),
            ("user/message", "Model context update"),
            ("compaction/prune", "Context elided"),
            ("compaction/summary", "Context summarized"),
            (
                "session-log-deepseek/delivery-accepted",
                "session-log-deepseek/delivery-accepted",
            ),
            ("model/selection", "model/selection"),
            ("plan/mode", "plan/mode"),
            ("sandbox/mode", "sandbox/mode"),
            ("todo/write", "todo/write"),
            ("goal/change", "goal/change"),
            ("subagent/catalog", "subagent/catalog"),
        ] {
            assert!(
                !chat_visible(kind, role),
                "{kind} / {role} must stay out of Chat"
            );
        }
        // A relabelled bookkeeping kind is still bookkeeping, not conversation.
        for kind in ["compaction/prune", "session-log-deepseek/delivery-accepted"] {
            assert!(
                !chat_visible(kind, "You"),
                "{kind} must ignore a human label"
            );
        }
        for (kind, role) in [
            ("user/message", "You"),
            ("assistant/message", "Assistant"),
            ("assistant/message", "Assistant · live"),
            ("assistant/attempt", "Interrupted assistant attempt"),
            ("tool/call", "Tool call"),
            ("tool/result", "Tool result"),
            ("turn/end", "Agent error"),
            ("agent/inbox/spliced", "You"),
        ] {
            assert!(
                chat_visible(kind, role),
                "{kind} / {role} must stay visible"
            );
        }
        // A kind this build does not recognize stays visible by policy: a future
        // required diagnostic must never be silently dropped.
        for kind in ["future/required", "Permission/preset", "plugin/progress"] {
            assert!(chat_visible(kind, kind), "{kind} must stay visible");
        }
    }
    #[test]
    fn turn_trigger_titles_match_the_reference_and_unknown_kinds_stay_hidden() {
        for (kind, title) in [
            ("goal", "Continuing goal"),
            ("agent-message", "Task message received"),
            ("team-message", "Team message received"),
            ("subagent-settled", "Subtask status updated"),
            ("webhook", "External event received"),
            ("schedule", "Automation task"),
            ("tool-jobs", "Background task updated"),
            ("cordis-host-runner", "Plugin status updated"),
        ] {
            assert_eq!(
                turn_trigger(&json!({"kind": kind})).map(|trigger| trigger.title),
                Some(title),
                "{kind}"
            );
        }
        assert_eq!(
            turn_trigger(&json!({"kind": "webhook", "provider": "github"}))
                .map(|trigger| trigger.title),
            Some("GitHub event received")
        );
        for hidden in ["runtime-context", "unknown", "repeat-tool-reminder"] {
            assert_eq!(turn_trigger(&json!({"kind": hidden})), None, "{hidden}");
        }
        assert_eq!(turn_trigger(&Value::Null), None);
    }

    #[test]
    fn only_known_routine_queue_splices_are_secondary() {
        for target in ["next-turn", "next-step"] {
            assert!(routine_inbox(
                &json!({"target":target,"start":0,"inserted":[{"id":"native-public","role":"user","source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC"}]}]})
            ));
            assert!(routine_inbox(
                &json!({"target":target,"start":0,"removedCount":1,"inserted":[]})
            ));
        }
    }
    #[test]
    fn cancellation_future_fields_and_malformed_splices_stay_primary() {
        let removal = json!({"target":"next-turn","start":0,"removedCount":1,"inserted":[]});
        for value in [
            json!("canceled"),
            json!("future"),
            Value::Null,
            json!(false),
        ] {
            let mut event = removal.clone();
            event["outcome"] = value;
            assert!(!routine_inbox(&event));
        }
        for (key, value) in [
            ("error", json!("PUBLIC error")),
            ("future", json!(true)),
            ("target", json!("future")),
            ("start", json!(-1)),
            ("removedCount", json!(false)),
            ("removedCount", json!(0)),
            ("inserted", json!({})),
            ("inserted", json!([{"content":[]}])),
            ("inserted", json!([{"id":"id","content":null}])),
        ] {
            let mut event = removal.clone();
            event[key] = value;
            assert!(!routine_inbox(&event));
        }
        assert!(!routine_inbox(
            &json!({"target":"next-turn","start":0,"inserted":[]})
        ));
        assert!(!routine_inbox(&Value::Null));
    }
    #[test]
    fn actual_rpc_text_and_file_sources_are_secondary_without_interpreting_file_authority() {
        for bytes in [0, 5003, 4 * 1024 * 1024] {
            let event = json!({"target":"next-turn","start":0,"inserted":[{"id":"native-public","role":"user","source":{"kind":"user","rpcId":"native-request","clientTimeZone":"UTC"},"content":[{"type":"text","text":"PUBLIC"},{"type":"file","attachment":{"attachmentId":"sha256:PUBLIC","name":"PUBLIC.bin","bytes":bytes}}]}]});
            assert!(routine_inbox(&event));
        }
    }
    #[test]
    fn future_source_message_content_extensions_and_unsafe_coordinates_stay_primary() {
        let event = json!({"target":"next-turn","start":0,"inserted":[{"id":"native-public","role":"user","source":{"kind":"user","rpcId":"request"},"content":[{"type":"text","text":"PUBLIC"}]}]});
        for key in ["id", "role", "source", "content"] {
            let mut v = event.clone();
            v["inserted"][0].as_object_mut().unwrap().remove(key);
            assert!(!routine_inbox(&v));
        }
        for source in [
            json!({"kind":"future"}),
            json!({"kind":"user","warning":"PUBLIC"}),
            json!({"kind":"user","rpcId":false}),
            json!({"kind":"user","clientTimeZone":false}),
        ] {
            let mut v = event.clone();
            v["inserted"][0]["source"] = source;
            assert!(!routine_inbox(&v));
        }
        for part in [
            json!({"type":"future","text":"PUBLIC"}),
            json!({"type":"text","text":"PUBLIC","warning":true}),
            json!({"type":"file","attachment":{"attachmentId":"id","name":"PUBLIC","bytes":0,"future":true}}),
        ] {
            let mut v = event.clone();
            v["inserted"][0]["content"] = json!([part]);
            assert!(!routine_inbox(&v));
        }
        let mut v = event.clone();
        v["inserted"][0]["warning"] = json!(true);
        assert!(!routine_inbox(&v));
        for key in ["start", "removedCount"] {
            let mut v = event.clone();
            v[key] = json!(9_007_199_254_740_992u64);
            assert!(!routine_inbox(&v));
        }
    }
}
