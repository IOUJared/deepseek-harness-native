//! Presentation-only diagnostics; retained events and Send authority stay unchanged.
use serde_json::Value;
pub fn ui_control_event(kind: &str) -> bool {
    matches!(
        kind,
        "session/title" | "session/title-llm-request" | "model/selection"
    )
}
/// Roles the Chat view presents. The system prompt, developer injects, model-only
/// context and harness bookkeeping belong to Trajectory. The exclusion is
/// deliberately a known list rather than an allowlist: an event kind this build
/// does not recognize stays visible, so a future required diagnostic is never
/// silently dropped (`future/required` is the standing case).
pub fn conversation_role(role: &str) -> bool {
    if matches!(
        role,
        "System" | "Developer" | "Model context update" | "Injected context"
    ) {
        return false;
    }
    // A cancellation or inbox outcome shares the splice kind, so that one keeps
    // its own routine filter instead of the blanket bookkeeping exclusion.
    role == "agent/inbox/spliced" || !crate::known::LOG_EVENTS.contains(&role)
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
    #[test]
    fn chat_keeps_conversation_and_unknown_roles_but_moves_known_internals_to_trajectory() {
        // Conversation, tool activity and the splice kind (cancellation and inbox
        // outcomes carry their own routine filter) stay in Chat.
        for role in [
            "You",
            "Assistant",
            "Assistant · live",
            "Interrupted assistant attempt",
            "Tool call",
            "Tool result",
            "Agent notice",
            "Agent error",
            "agent/inbox/spliced",
        ] {
            assert!(conversation_role(role));
        }
        // A kind this build does not recognize stays visible: a future required
        // diagnostic must never be silently dropped.
        for role in ["future/required", "Permission/preset", "approval/policy-changed"] {
            assert!(conversation_role(role));
        }
        // The system prompt, developer injects, model-only context and every
        // known bookkeeping or telemetry event belong to Trajectory.
        for role in [
            "System",
            "Developer",
            "Model context update",
            "Injected context",
            "permission/preset",
            "sandbox/mode",
            "approval/policy",
            "session-log-deepseek/delivery-accepted",
            "model/selection",
            "command/run",
            "todo/write",
            "subagent/catalog",
        ] {
            assert!(!conversation_role(role), "{role}");
        }
    }
}
