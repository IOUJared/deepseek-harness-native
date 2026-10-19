//! Human history is append-origin conversation, not the model's rewritten context.
use super::*;

pub(super) fn submitted_text(data: &Value, recorded: &BTreeSet<&str>) -> Option<String> {
    if !presentation::routine_inbox(data) {
        return None;
    }
    let messages = data["inserted"].as_array()?;
    let text = messages
        .iter()
        .filter(|message| {
            // An injected context message is not human history, so a splice that
            // carries only injects stays out of Chat and lives in Trajectory.
            presentation::human_message(&message["source"])
                && message["id"]
                    .as_str()
                    .is_some_and(|id| !recorded.contains(id))
        })
        .map(|message| content(&message["content"]))
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then(|| bounded(&text, MAX_DISPLAY_BYTES))
}

/// The reference's turn-failure and output-limit rows: a title, the reason, and the recorded code
/// (`MessageItem.tsx:129-160`, `locale.ts:358-362`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub title: String,
    pub message: String,
    /// The Host's own code, drawn as a chip so a reader can quote it; absent when none was recorded.
    pub code: Option<String>,
    /// The reference draws a failure with its error dot and the output cap with its warning dot
    /// (`MessageItem.tsx:136,152`).
    pub warning: bool,
}

pub(super) fn turn_notice(data: &Value) -> Option<String> {
    let reason = &data["reason"];
    let text = match reason["kind"].as_str() {
        Some("completed" | "forked") => return None,
        Some("error") => {
            let error = &reason["error"];
            // AUTH and credential errors may echo the provider's own message,
            // which can quote configuration or a rejected secret. Each known
            // code gets its own short, actionable line instead.
            match error["code"].as_str() {
                Some("AUTH") => {
                    "Authentication failed. Check API login or Codex in Settings.".to_owned()
                }
                Some("MISSING_CREDENTIAL") => {
                    "No API key stored for this model route. Add one in Settings → DeepSeek."
                        .to_owned()
                }
                Some("INVALID_CREDENTIAL") => {
                    "The stored API key was rejected. Re-enter it in Settings → DeepSeek."
                        .to_owned()
                }
                _ => {
                    let code = error["code"].as_str().unwrap_or("UNKNOWN");
                    let message = error["message"]
                        .as_str()
                        .unwrap_or("The agent could not complete the request.");
                    format!(
                        "Agent error ({}): {}",
                        bounded(code, 64),
                        bounded(message, MAX_DISPLAY_BYTES - 128)
                    )
                }
            }
        }
        Some("blocked") => {
            "The agent could not start this response: the request was blocked.".into()
        }
        Some("aborted") => "The response was stopped. Your submitted message is retained.".into(),
        Some("interrupted") => {
            "The response was interrupted. Your conversation is retained.".into()
        }
        // The reference states what to do about it, not only that it happened.
        Some("max-tokens") => "The response reached the model's output limit. The reply was cut off; earlier output is preserved in the conversation. Send \"continue\" to let the model resume.".into(),
        _ => format!("Unrecognized turn ending: {}", safe_json(reason)),
    };
    Some(bounded(&text, MAX_DISPLAY_BYTES))
}

pub(super) fn surface_update(
    nodes: &mut Vec<u64>,
    event: &SessionWireEvent,
    complete: bool,
) -> Result<(), ReduceError> {
    match &event.surface_op {
        Some(value) if value.is_object() => {
            let start = value["startSeq"]
                .as_u64()
                .ok_or(ReduceError::InvalidSurface)?;
            let end = value["endSeq"]
                .as_u64()
                .ok_or(ReduceError::InvalidSurface)?;
            if value["op"] != "replace" || start >= event.seq || end >= event.seq {
                return Err(ReduceError::InvalidSurface);
            }
            let sources = event
                .source_event_seqs
                .as_ref()
                .and_then(Value::as_array)
                .ok_or(ReduceError::InvalidSurface)?;
            let mut seen = BTreeSet::new();
            for source in sources {
                let seq = source.as_u64().ok_or(ReduceError::InvalidSurface)?;
                if seq >= event.seq || !seen.insert(seq) {
                    return Err(ReduceError::InvalidSurface);
                }
            }
            if sources.is_empty() || !seen.contains(&start) || !seen.contains(&end) {
                return Err(ReduceError::InvalidSurface);
            }
            match (
                nodes.iter().position(|seq| *seq == start),
                nodes.iter().position(|seq| *seq == end),
            ) {
                (Some(first), Some(last)) if first <= last => {
                    if complete && nodes[first..=last].iter().any(|seq| !seen.contains(seq)) {
                        return Err(ReduceError::InvalidSurface);
                    }
                    nodes.splice(first..=last, [event.seq]);
                }
                _ if !complete => {
                    // A paginated opening cannot establish the full model order.
                    // Preserve a known endpoint's position rather than append at the
                    // wrong end; strict range validation waits for complete history.
                    let first = nodes.iter().position(|seq| *seq == start).unwrap_or(0);
                    let last = nodes.iter().position(|seq| *seq == end).unwrap_or(first);
                    let has_endpoint = nodes.contains(&start) || nodes.contains(&end);
                    if has_endpoint && first <= last && last < nodes.len() {
                        nodes.splice(first..=last, [event.seq]);
                    } else {
                        nodes.insert(first.min(nodes.len()), event.seq);
                    }
                }
                _ => return Err(ReduceError::InvalidSurface),
            }
        }
        Some(value) if value.as_str() != Some("append") => return Err(ReduceError::InvalidSurface),
        _ => nodes.push(event.seq),
    }
    Ok(())
}

/// The typed failure row for a terminal turn ending, mirroring the reference's
/// `TurnErrorItem`/`TurnMaxTokensItem`: a title, the mapped reason, and the recorded code.
/// `None` for every other ending — those keep their single notice line.
pub(super) fn notice_for(reason: &Value) -> Option<Notice> {
    match reason["kind"].as_str() {
        Some("error") => {
            let error = &reason["error"];
            let code = error["code"].as_str().map(|code| bounded(code, 64));
            // The reference maps its own codes to human sentences and shows anything else as the
            // provider wrote it (`MessageItem.tsx:52-60`).
            let message = match code.as_deref() {
                Some("ACCOUNT_SIGNED_OUT") => {
                    "Stopped because you signed out of DeepSeek.".to_owned()
                }
                Some("ACCOUNT_SIGN_IN_REQUIRED") => {
                    "Sign in to DeepSeek and ensure the request destination supports account authentication."
                        .to_owned()
                }
                Some("QUOTA" | "ACCOUNT_QUOTA") => "Request quota exhausted.".to_owned(),
                Some("AUTH") => "API key is invalid.".to_owned(),
                _ => bounded(
                    error["message"]
                        .as_str()
                        .unwrap_or("The agent could not complete the request."),
                    MAX_DISPLAY_BYTES,
                ),
            };
            Some(Notice {
                title: if code.as_deref() == Some("ACCOUNT_SIGNED_OUT") {
                    "Task stopped".to_owned()
                } else {
                    "This turn failed".to_owned()
                },
                message,
                code,
                warning: false,
            })
        }
        Some("max-tokens") => Some(Notice {
            title: "Output token limit reached".to_owned(),
            message:
                "The reply was cut off; earlier output is preserved in the conversation. Send \
                      \"continue\" to let the model resume."
                    .to_owned(),
            code: None,
            warning: true,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod notice_tests {
    //! The failure row's copy and code chip, checked against the reference's dictionary.
    use super::*;
    use serde_json::json;

    #[test]
    fn known_codes_get_the_reference_copy_and_the_chip() {
        let notice =
            notice_for(&json!({"kind": "error", "error": {"code": "QUOTA", "message": "x"}}))
                .unwrap();
        assert_eq!(notice.title, "This turn failed");
        assert_eq!(notice.message, "Request quota exhausted.");
        assert_eq!(notice.code.as_deref(), Some("QUOTA"));
        let signed_out =
            notice_for(&json!({"kind": "error", "error": {"code": "ACCOUNT_SIGNED_OUT"}})).unwrap();
        assert_eq!(signed_out.title, "Task stopped");
        // An unknown code keeps the provider's message and still shows the code.
        let other = notice_for(
            &json!({"kind": "error", "error": {"code": "SOMETHING", "message": "PUBLIC detail"}}),
        )
        .unwrap();
        assert_eq!(other.message, "PUBLIC detail");
        assert_eq!(other.code.as_deref(), Some("SOMETHING"));
    }

    #[test]
    fn the_output_limit_states_what_to_do() {
        let notice = notice_for(&json!({"kind": "max-tokens"})).unwrap();
        assert_eq!(notice.title, "Output token limit reached");
        assert!(
            notice.message.contains("Send \"continue\""),
            "{}",
            notice.message
        );
        assert_eq!(notice.code, None);
        // Every other ending keeps its single line.
        assert!(notice_for(&json!({"kind": "aborted"})).is_none());
        assert!(notice_for(&json!({"kind": "completed"})).is_none());
    }
}
