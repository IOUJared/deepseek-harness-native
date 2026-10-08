//! Read-only, bounded labels from alpha tool events. Full inputs/outputs stay in details.
use dsh_native_transport::dto::SessionWireEvent;
use serde_json::{Map, Value};

const MAX_ARGUMENT_BYTES: usize = 32 * 1024;
const MAX_NAME_BYTES: usize = 80;
const ARGUMENTS_AVAILABLE: &str = "Tool arguments available";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// At most 112 UTF-8 bytes; names describe recorded intent, not execution state.
    pub title: String,
    /// At most 160 UTF-8 bytes; never a result body or a dump of arguments.
    pub target: Option<String>,
    /// True only for an exact recorded message.isError boolean true.
    pub failed: bool,
}

pub(super) fn summarize<'a>(
    event: &SessionWireEvent,
    previous: impl Iterator<Item = &'a SessionWireEvent>,
) -> Option<Summary> {
    match event.event_type.as_str() {
        "tool/call" => Some(text_summary_label(&event.data, false, false)),
        "tool/result" => {
            let message = &event.data["message"];
            let failed = message["isError"].as_bool() == Some(true);
            // Alpha stores callId on the call, toolCallId on the result message,
            // and repeats that same id in message.source (session/index.ts).
            let paired = (|| {
                let id = message["toolCallId"].as_str().filter(|s| !s.is_empty())?;
                if message["role"].as_str() != Some("tool")
                    || message["source"]["kind"].as_str() != Some("tool")
                    || message["source"]["callId"].as_str() != Some(id)
                {
                    return None;
                }
                let turn = event.data["turn"].as_u64()?;
                let step = event.data["step"].as_u64()?;
                let mut calls = previous.filter(|candidate| {
                    candidate.event_type == "tool/call"
                        && candidate.data["callId"].as_str() == Some(id)
                        && candidate.data["turn"].as_u64() == Some(turn)
                        && candidate.data["step"].as_u64() == Some(step)
                });
                let call = calls.next()?;
                // Ambiguous repeated identifiers are not safe presentation metadata.
                if calls.next().is_some() {
                    return None;
                }
                Some(call)
            })();
            Some(paired.map_or_else(
                || Summary {
                    title: "Tool result".into(),
                    target: None,
                    failed,
                },
                |call| text_summary_label(&call.data, true, failed),
            ))
        }
        _ => None,
    }
}

fn text_summary_label(data: &Value, result: bool, failed: bool) -> Summary {
    let name = data["name"].as_str().unwrap_or("");
    let display_name = compact(name, MAX_NAME_BYTES);
    if display_name.is_empty() {
        return Summary {
            title: if result { "Tool result" } else { "Tool call" }.into(),
            target: None,
            failed,
        };
    }
    let verb = match name {
        "read" | "functions.read" | "tool-fs/read" => "Read",
        "read_image" | "functions.read_image" | "tool-fs/read_image" => "Read",
        "edit" | "functions.edit" | "tool-fs/edit" => "Edit",
        "write" | "functions.write" | "tool-fs/write" => "Write",
        "bash" | "functions.bash" | "tool-shell/bash" => "Bash",
        "shell" | "functions.shell" | "pwsh" | "functions.pwsh" => "Shell",
        _ => "Tool",
    };
    let title = format!(
        "{verb} · {display_name}{}",
        if result { " · result" } else { "" }
    );
    let target = parse_args(data).map_or_else(
        || Some(ARGUMENTS_AVAILABLE.into()),
        |args| {
            if args.keys().any(|key| sensitive_field(key)) {
                return Some(ARGUMENTS_AVAILABLE.into());
            }
            let picked = match verb {
                "Read" | "Edit" | "Write" => pick(&args, &["file_path", "path"]),
                "Bash" | "Shell" => {
                    // Never expose a credential-bearing command through a benign description.
                    if pick(&args, &["command"]).is_some_and(sensitive_text) {
                        return Some(ARGUMENTS_AVAILABLE.into());
                    }
                    pick(&args, &["command", "description"])
                }
                _ => None,
            };
            let label = match picked {
                Some(value) if sensitive_text(value) => ARGUMENTS_AVAILABLE.into(),
                Some(value) if matches!(verb, "Read" | "Edit" | "Write") => path_label(value),
                Some(value) => compact(value, 96),
                None => ARGUMENTS_AVAILABLE.into(),
            };
            Some(if label.is_empty() {
                ARGUMENTS_AVAILABLE.into()
            } else {
                label
            })
        },
    );
    Summary {
        title,
        target,
        failed,
    }
}

fn parse_args(data: &Value) -> Option<Map<String, Value>> {
    // Alpha arguments are raw strings, not root objects. Refuse oversized or
    // truncated JSON rather than parsing the display text or a prefix.
    let raw = data["arguments"].as_str()?;
    if raw.len() > MAX_ARGUMENT_BYTES {
        return None;
    }
    match serde_json::from_str::<Value>(raw).ok()? {
        Value::Object(args) => Some(args),
        _ => None,
    }
}

fn pick<'a>(args: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        args.get(*key)?
            .as_str()
            .filter(|value| !value.trim().is_empty())
    })
}

fn sensitive_field(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace(['_', '-'], "");
    matches!(
        normalized.as_str(),
        "token"
            | "tokens"
            | "accesstoken"
            | "refreshtoken"
            | "secret"
            | "secrets"
            | "clientsecret"
            | "cookie"
            | "cookies"
            | "sessioncookie"
            | "authorization"
            | "apikey"
            | "password"
            | "passwd"
            | "credentials"
            | "credential"
            | "privatekey"
            | "accesskey"
            | "secretkey"
            | "env"
            | "environment"
            | "environmentvariables"
            | "headers"
    )
}

fn sensitive_text(value: &str) -> bool {
    // Conservative suppression of obvious markers, not a general secret detector.
    let lower = value.to_ascii_lowercase();
    lower.contains(['=', '$'])
        || [
            "authorization",
            "bearer ",
            "cookie",
            "password",
            "passwd",
            "--token",
            "--secret",
            "api_key",
            "api-key",
            "apikey",
            "access_token",
            "access-key",
            "client_secret",
            "sk-",
            "ghp_",
            "github_pat_",
            "xoxb-",
            "printenv",
            "export ",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
        || lower.split_whitespace().any(|word| word == "env")
        || lower.split_whitespace().any(|word| {
            word.contains("://")
                && word
                    .split("://")
                    .nth(1)
                    .is_some_and(|rest| rest.contains('@'))
        })
}

fn path_label(path: &str) -> String {
    let clean = compact(path, MAX_ARGUMENT_BYTES);
    let clean = clean.trim_end_matches(['/', '\\']);
    let (parent, basename) = clean.rsplit_once(['/', '\\']).unwrap_or(("", clean));
    let basename = compact(basename, 64);
    if parent.is_empty() {
        return basename;
    }
    let parent = tail(parent, 80);
    format!("{basename} · {parent}/")
}

fn tail(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.into();
    }
    let mut start = text.len() - (max - '…'.len_utf8());
    while !text.is_char_boundary(start) {
        start += 1;
    }
    format!("…{}", &text[start..])
}

fn compact(text: &str, max: usize) -> String {
    let mut out = String::new();
    let mut space = false;
    for c in text.chars() {
        if c.is_control()
            || c.is_whitespace()
            || matches!(c,
            '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}' | '\u{feff}')
        {
            space = !out.is_empty();
            continue;
        }
        let required = c.len_utf8() + usize::from(space);
        if out.len() + required > max {
            while out.len() + '…'.len_utf8() > max {
                out.pop();
            }
            out.push('…');
            break;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reducer::Transcript;
    use dsh_native_transport::dto::SessionFollowFrame;
    use serde_json::json;

    fn event(seq: u64, kind: &str, data: Value) -> SessionWireEvent {
        serde_json::from_value(json!({"seq":seq,"time":1000,"type":kind,"data":data})).unwrap()
    }
    fn call(seq: u64, id: &str, name: &str, args: Value) -> SessionWireEvent {
        event(
            seq,
            "tool/call",
            json!({"callId":id,"turn":1,"step":2,"name":name,"arguments":args.to_string()}),
        )
    }
    fn result(seq: u64, id: &str, error: Value) -> SessionWireEvent {
        event(
            seq,
            "tool/result",
            json!({"turn":1,"step":2,"message":{"role":"tool","toolCallId":id,"source":{"kind":"tool","callId":id},"isError":error,"content":[{"type":"text","text":"PRIVATE RESULT"}]}}),
        )
    }
    fn label(call: &SessionWireEvent) -> Summary {
        summarize(call, std::iter::empty()).unwrap()
    }

    #[test]
    fn known_verbs_preserve_full_names_and_root_paths() {
        for (name, verb, field) in [
            ("read", "Read", "file_path"),
            ("functions.edit", "Edit", "path"),
            ("tool-fs/write", "Write", "file_path"),
        ] {
            let summary = label(&call(
                0,
                "a",
                name,
                json!({field:"/workspace/src/activity.rs","content":"PRIVATE CONTENT","old_string":"PRIVATE OLD"}),
            ));
            assert_eq!(summary.title, format!("{verb} · {name}"));
            assert_eq!(
                summary.target.as_deref(),
                Some("activity.rs · /workspace/src/")
            );
            assert!(!summary.failed);
            assert!(!format!("{summary:?}").contains("PRIVATE"));
        }
    }

    #[test]
    fn shell_prefers_actual_safe_command_and_description_only_without_command() {
        let summary = label(&call(
            0,
            "a",
            "functions.bash",
            json!({"command":"cargo test activity","description":"Check activity labels"}),
        ));
        assert_eq!(summary.title, "Bash · functions.bash");
        assert_eq!(summary.target.as_deref(), Some("cargo test activity"));
        let summary = label(&call(
            0,
            "a",
            "shell",
            json!({"command":"printf hello","description":"Print a greeting"}),
        ));
        assert_eq!(summary.target.as_deref(), Some("printf hello"));
        let summary = label(&call(
            0,
            "a",
            "bash",
            json!({"description":"Check activity labels"}),
        ));
        assert_eq!(summary.target.as_deref(), Some("Check activity labels"));
        let summary = label(&call(0, "a", "bash", json!({"command":"x".repeat(200)})));
        assert!(summary.target.unwrap().len() <= 96);
    }

    #[test]
    fn results_pair_exact_ids_source_turn_step_and_not_proximity() {
        let read = call(0, "read-id", "read", json!({"file_path":"a.rs"}));
        let edit = call(1, "edit-id", "edit", json!({"file_path":"b.rs"}));
        let summary = summarize(
            &result(2, "read-id", json!(true)),
            [&edit, &read].into_iter(),
        )
        .unwrap();
        assert_eq!(summary.title, "Read · read · result");
        assert_eq!(summary.target.as_deref(), Some("a.rs"));
        assert!(summary.failed);
        assert!(!format!("{summary:?}").contains("PRIVATE RESULT"));
        for invalid in [json!("true"), Value::Null, json!(false)] {
            assert!(
                !summarize(&result(2, "read-id", invalid), [&read].into_iter())
                    .unwrap()
                    .failed
            );
        }
        let mut unflagged = result(2, "read-id", Value::Null);
        unflagged.data["message"]["content"] = json!([{"type":"text","text":"Error: failed"}]);
        unflagged.data["error"] = json!({"name":"ToolOutcomeUnknownError","code":"unknown"});
        assert!(!summarize(&unflagged, [&read].into_iter()).unwrap().failed);
        for mutate in 0..6 {
            let mut res = result(2, "read-id", json!(true));
            match mutate {
                0 => res.data["message"]["source"]["callId"] = json!("different"),
                1 => res.data["message"]["source"]["kind"] = json!("model"),
                2 => res.data["turn"] = json!(3),
                3 => res.data["step"] = Value::Null,
                4 => res.data["message"]["role"] = Value::Null,
                _ => res.data["message"]["toolCallId"] = json!(""),
            }
            let summary = summarize(&res, [&edit, &read].into_iter()).unwrap();
            assert_eq!(summary.title, "Tool result");
            assert_eq!(summary.target, None);
            assert!(summary.failed);
        }
        let mut wrong_field = read.clone();
        wrong_field.data.as_object_mut().unwrap().remove("callId");
        wrong_field.data["id"] = json!("read-id");
        assert_eq!(
            summarize(
                &result(2, "read-id", json!(false)),
                [&wrong_field].into_iter()
            )
            .unwrap()
            .title,
            "Tool result"
        );
        let duplicate = call(1, "read-id", "edit", json!({"file_path":"b.rs"}));
        assert_eq!(
            summarize(
                &result(2, "read-id", json!(false)),
                [&duplicate, &read].into_iter()
            )
            .unwrap()
            .title,
            "Tool result"
        );
        assert_eq!(
            summarize(
                &result(2, "missing", json!(false)),
                [&edit, &read].into_iter()
            )
            .unwrap()
            .title,
            "Tool result"
        );
    }

    #[test]
    fn invalid_oversized_non_string_and_unknown_args_stay_generic() {
        for args in [
            json!("{\"file_path\":"),
            json!("[1,2]"),
            json!({"file_path":"a.rs"}),
            json!("x".repeat(MAX_ARGUMENT_BYTES + 1)),
        ] {
            let mut record = call(0, "a", "read", json!({}));
            record.data["arguments"] = args;
            assert_eq!(label(&record).target.as_deref(), Some(ARGUMENTS_AVAILABLE));
        }
        let summary = label(&call(
            0,
            "a",
            "custom/read",
            json!({"path":"PRIVATE UNKNOWN","description":"PRIVATE DESCRIPTION"}),
        ));
        assert_eq!(summary.title, "Tool · custom/read");
        assert_eq!(summary.target.as_deref(), Some(ARGUMENTS_AVAILABLE));
        let summary = label(&call(
            0,
            "a",
            "read",
            json!({"file_path":"a.rs","content":"x".repeat(MAX_ARGUMENT_BYTES)}),
        ));
        assert_eq!(summary.target.as_deref(), Some(ARGUMENTS_AVAILABLE));
    }

    #[test]
    fn summary_never_reads_result_output_or_nested_argument_fields() {
        let call = call(
            0,
            "a",
            "read",
            json!({"file_path":"a.rs","nested":{"path":"PRIVATE PATH"},"content":"PRIVATE CONTENT"}),
        );
        let mut result = result(1, "a", json!(false));
        result.data["message"]["content"] =
            json!([{"type":"text","text":"PRIVATE OUTPUT".repeat(100_000)}]);
        result.data["meta"] = json!({"description":"PRIVATE META","path":"PRIVATE PATH"});
        let summary = summarize(&result, [&call].into_iter()).unwrap();
        assert_eq!(summary.target.as_deref(), Some("a.rs"));
        assert!(!format!("{summary:?}").contains("PRIVATE"));
        let mut nested = call;
        nested.data["arguments"] =
            json!(json!({"nested":{"file_path":"PRIVATE PATH"}}).to_string());
        assert_eq!(label(&nested).target.as_deref(), Some(ARGUMENTS_AVAILABLE));
    }

    #[test]
    fn unicode_long_paths_names_and_controls_are_bounded_single_line() {
        let summary = label(&call(
            0,
            "a",
            "read",
            json!({"path":format!("/你好/{}/文件\n\u{1b}\u{202e}名.rs", "目录/".repeat(900))}),
        ));
        let target = summary.target.unwrap();
        assert!(target.starts_with("文件 名.rs"));
        assert!(target.len() <= 160);
        assert!(!target.chars().any(char::is_control));
        assert!(!target.contains('\u{202e}'));
        let summary = label(&call(0, "a", &"你🦀\n".repeat(300), json!({})));
        assert!(summary.title.len() <= 112);
        assert!(!summary.title.chars().any(char::is_control));
    }

    #[test]
    fn known_sensitive_root_fields_and_obvious_commands_are_suppressed() {
        for key in [
            "token",
            "API_key",
            "client-secret",
            "Cookie",
            "authorization",
            "password",
            "env",
            "headers",
        ] {
            let summary = label(&call(
                0,
                "a",
                "bash",
                json!({"command":"printf harmless",key:"PRIVATE CREDENTIAL"}),
            ));
            assert_eq!(summary.target.as_deref(), Some(ARGUMENTS_AVAILABLE));
        }
        for command in [
            "TOKEN=private program",
            "curl -H 'Authorization: Bearer sk-private'",
            "echo $SECRET",
            "env",
            "printenv",
            "curl https://name:private@example.org",
            "echo ghp_private",
            "program --token private",
        ] {
            let summary = label(&call(
                0,
                "a",
                "bash",
                json!({"command":command,"description":"Benign label"}),
            ));
            assert_eq!(summary.target.as_deref(), Some(ARGUMENTS_AVAILABLE));
        }
    }

    #[test]
    fn getter_leaves_stored_rows_and_reducer_state_untouched() {
        let records = vec![
            call(0, "a", "read", json!({"file_path":"a.rs"})),
            result(1, "a", json!(false)),
        ];
        let mut transcript = Transcript::new(1);
        let frame: SessionFollowFrame = serde_json::from_value(json!({"type":"snapshot","cursor":1,"hasMore":false,"header":{"version":4,"id":"s1","createdAt":1000,"isSeeded":false},"projections":{"asOfSeq":1,"values":{}},"records":records.iter().map(|event|json!({"type":"event","event":event})).collect::<Vec<_>>()})).unwrap();
        transcript.apply(1, frame).unwrap();
        let before = transcript
            .rows()
            .iter()
            .map(|r| (r.key, r.role.clone(), r.text.clone(), r.time, r.surface))
            .collect::<Vec<_>>();
        assert_eq!(before[0].2, "read\n{\"file_path\":\"a.rs\"}");
        assert_eq!(before[1].2, "PRIVATE RESULT");
        let cursor = transcript.opening_cursor();
        let safe = transcript.safe_to_send();
        let stored = serde_json::to_value(&transcript.records).unwrap();
        let durable_cursor = transcript.cursor;
        let retained_bytes = transcript.retained_bytes;
        assert!(transcript.activity(0).is_some());
        assert!(transcript.activity(1).is_some());
        assert!(transcript.activity(999).is_none());
        assert_eq!(
            before,
            transcript
                .rows()
                .iter()
                .map(|r| (r.key, r.role.clone(), r.text.clone(), r.time, r.surface))
                .collect::<Vec<_>>()
        );
        assert_eq!(cursor, transcript.opening_cursor());
        assert_eq!(safe, transcript.safe_to_send());
        assert_eq!(stored, serde_json::to_value(&transcript.records).unwrap());
        assert_eq!(durable_cursor, transcript.cursor);
        assert_eq!(retained_bytes, transcript.retained_bytes);
        for kind in [
            "user/message",
            "assistant/message",
            "system/message",
            "future/notice",
        ] {
            assert!(summarize(&event(9, kind, json!({})), std::iter::empty()).is_none());
        }
    }
}
