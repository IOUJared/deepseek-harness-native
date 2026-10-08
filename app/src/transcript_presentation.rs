//! Presentation-only diagnostics; retained events and Send authority stay unchanged.
use serde_json::Value;
pub fn ui_control_event(kind: &str) -> bool {
    matches!(kind, "session/title" | "session/title-llm-request")
}
pub fn conversation_role(role: &str) -> bool {
    !matches!(
        role,
        "permission/preset" | "sandbox/mode" | "approval/policy"
    )
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
                    s.keys()
                        .all(|k| matches!(k.as_str(), "kind" | "rpcId" | "clientTimeZone"))
                        && message["source"]["kind"] == "user"
                        && s.get("rpcId")
                            .is_none_or(|v| v.as_str().is_some_and(|id| !id.is_empty()))
                        && s.get("clientTimeZone").is_none_or(Value::is_string)
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
    fn unknown_roles_and_model_tool_system_messages_remain_primary() {
        for role in [
            "You",
            "Assistant",
            "Assistant · live",
            "Interrupted assistant attempt",
            "Tool call",
            "Tool result",
            "System",
            "Developer",
            "agent/inbox/spliced",
            "permission/preset-changed",
            "future/required",
        ] {
            assert!(conversation_role(role));
        }
        for role in ["permission/preset", "sandbox/mode", "approval/policy"] {
            assert!(!conversation_role(role));
        }
    }
}
