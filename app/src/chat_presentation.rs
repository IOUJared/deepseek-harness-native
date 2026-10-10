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
            message["id"]
                .as_str()
                .is_some_and(|id| !recorded.contains(id))
        })
        .map(|message| content(&message["content"]))
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then(|| bounded(&text, MAX_DISPLAY_BYTES))
}

pub(super) fn turn_notice(data: &Value) -> Option<String> {
    let reason = &data["reason"];
    let text = match reason["kind"].as_str() {
        Some("completed" | "forked") => return None,
        Some("error") => {
            let error = &reason["error"];
            // AUTH errors may echo credentials in the provider's message string.
            if error["code"] == "AUTH" {
                "Authentication failed. Check API login or Codex in Settings.".to_owned()
            } else {
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
        Some("blocked") => {
            "The agent could not start this response: the request was blocked.".into()
        }
        Some("aborted") => "The response was stopped. Your submitted message is retained.".into(),
        Some("interrupted") => {
            "The response was interrupted. Your conversation is retained.".into()
        }
        Some("max-tokens") => "The response reached the model's output limit.".into(),
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
