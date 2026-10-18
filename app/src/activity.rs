//! Read-only, bounded labels from alpha tool events. Full inputs/outputs stay in details.
use dsh_native_transport::dto::SessionWireEvent;
use serde_json::{Map, Value};

const MAX_ARGUMENT_BYTES: usize = 32 * 1024;
const MAX_NAME_BYTES: usize = 80;
/// Bound for the rendered target, matching the `Summary::target` contract.
const MAX_TARGET_BYTES: usize = 160;
const ARGUMENTS_AVAILABLE: &str = "Tool arguments available";

/// Display roots for a tool path, the reference's own inputs
/// (`tool-call-model.ts:261` combines `relativizeToCwd` with
/// `abbreviateHomePath`). Both are display-only: neither changes the path the
/// app would open.
#[derive(Clone, Copy, Default)]
pub struct PathRoots<'a> {
    /// The session's workspace root; a path inside it displays relative to it.
    pub cwd: Option<&'a str>,
    /// The host account home; a path inside it displays from `~`.
    pub home: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// At most 112 UTF-8 bytes; names describe recorded intent, not execution state.
    pub title: String,
    /// At most 160 UTF-8 bytes; never a result body or a dump of arguments.
    pub target: Option<String>,
    /// True only for an exact recorded message.isError boolean true.
    pub failed: bool,
    /// Line totals `(added, removed)` for an `edit` call whose contiguous
    /// old→new replacement is small enough to diff. Counts only; the strings
    /// never leave this module.
    pub edits: Option<(u32, u32)>,
}

pub(super) fn summarize<'a>(
    event: &SessionWireEvent,
    previous: impl Iterator<Item = &'a SessionWireEvent>,
    roots: PathRoots<'_>,
) -> Option<Summary> {
    match event.event_type.as_str() {
        "tool/call" => Some(text_summary_label(&event.data, false, false, roots)),
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
                    edits: None,
                },
                |call| text_summary_label(&call.data, true, failed, roots),
            ))
        }
        // Staged context management, described with the paper's own vocabulary:
        // elision replaces a stale observation body, summarization folds the
        // oldest middle region. Both are log-only, so these lines are the only
        // place the user learns the harness reclaimed context.
        "compaction/prune" => Some(Summary {
            title: match shadowed_records(&event.data) {
                Some(records) => format!("Context elided · {records} → stubs"),
                None => "Context elided · count available in details".into(),
            },
            target: shadowed_tokens(&event.data),
            failed: false,
            edits: None,
        }),
        "compaction/summary" => {
            let range = &event.data["shadowedRange"];
            let span = match (count(&range["start"]), count(&range["end"])) {
                (Some(start), Some(end)) => format!("seqs {start}–{end}"),
                _ => "range available in details".into(),
            };
            Some(Summary {
                title: format!("Context summarized · {span}"),
                target: shadowed_tokens(&event.data),
                failed: false,
                edits: None,
            })
        }
        "compaction/end" => event.data["error"].as_str().map_or_else(
            || None,
            |error| {
                Some(Summary {
                    title: "Compaction failed".into(),
                    target: Some(compact(error, 96)),
                    failed: true,
                    edits: None,
                })
            },
        ),
        // The loop guard injects an advisory reminder as a `user/message`; only
        // its producer source marks it, so this is not a human turn. The host
        // owns the counting and the thresholds — nothing is recounted here.
        "user/message" if event.data["source"]["kind"].as_str() == Some("repeat-tool-reminder") => {
            Some(loop_guard_summary(&event.data))
        }
        _ => None,
    }
}

fn count(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .filter(|value| *value <= 9_007_199_254_740_991)
}

/// Records replaced by one elision, when the log states them.
fn shadowed_records(data: &Value) -> Option<String> {
    data["shadowedSeqs"]
        .as_array()
        .filter(|seqs| !seqs.is_empty())
        .map(|seqs| match seqs.len() {
            1 => "1 record".to_owned(),
            n => format!("{n} records"),
        })
}

/// The logged shadow price is the host estimator's own number, not a count.
fn shadowed_tokens(data: &Value) -> Option<String> {
    count(&data["shadowedTokenCount"])
        .map(|tokens| format!("~{} tokens reclaimed", compact_tokens(tokens)))
}

fn compact_tokens(tokens: u64) -> String {
    if tokens >= 1000 {
        format!("{}k", tokens / 1000)
    } else {
        tokens.to_string()
    }
}

/// `"bash × 8"` is the guard's own producer summary: the tool name and the run
/// length it counted. It is presentation input, so an unexpected form degrades
/// to a generic label rather than inventing a count.
fn loop_guard_summary(data: &Value) -> Summary {
    let run = data["source"]["summary"]
        .as_str()
        .and_then(|summary| summary.split_once(" × "))
        .map(|(tool, times)| (compact(tool, MAX_NAME_BYTES), compact(times, 16)));
    match run {
        Some((tool, times)) if !tool.is_empty() && !times.is_empty() => Summary {
            title: "Loop guard · repeated identical call".into(),
            target: Some(format!("{tool} × {times}")),
            failed: false,
            edits: None,
        },
        _ => Summary {
            title: "Loop guard · repeated identical call".into(),
            target: None,
            failed: false,
            edits: None,
        },
    }
}

fn text_summary_label(data: &Value, result: bool, failed: bool, roots: PathRoots<'_>) -> Summary {
    let name = data["name"].as_str().unwrap_or("");
    let display_name = compact(name, MAX_NAME_BYTES);
    if display_name.is_empty() {
        return Summary {
            title: if result { "Tool result" } else { "Tool call" }.into(),
            target: None,
            failed,
            edits: None,
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
    let mut edits = None;
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
            if verb == "Edit" {
                edits = edit_totals(&args);
            }
            let label = match picked {
                Some(value) if sensitive_text(value) => ARGUMENTS_AVAILABLE.into(),
                Some(value) if matches!(verb, "Read" | "Edit" | "Write") => {
                    path_label(value, roots)
                }
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
        edits,
    }
}

/// Line totals for an `edit` call's contiguous old→new replacement. Only the
/// two counts escape; the strings are never stored, cached or surfaced.
/// Oversized inputs decline counts rather than spend unbounded work.
fn edit_totals(args: &Map<String, Value>) -> Option<(u32, u32)> {
    let old = args.get("old_string")?.as_str()?;
    let new = args.get("new_string")?.as_str()?;
    line_diff_totals(old, new)
}

const MAX_DIFF_LINES: usize = 256;

fn line_diff_totals(old: &str, new: &str) -> Option<(u32, u32)> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    if old_lines.len() > MAX_DIFF_LINES || new_lines.len() > MAX_DIFF_LINES {
        return None;
    }
    let common = lcs_len(&old_lines, &new_lines);
    let added = u32::try_from(new_lines.len().checked_sub(common)?).ok()?;
    let removed = u32::try_from(old_lines.len().checked_sub(common)?).ok()?;
    Some((added, removed))
}

/// Rolling two-row longest-common-subsequence length over lines; memory is
/// O(min(a, b)) and time O(a·b) within `MAX_DIFF_LINES`.
fn lcs_len(a: &[&str], b: &[&str]) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let mut previous = vec![0usize; short.len() + 1];
    let mut current = vec![0usize; short.len() + 1];
    for line in long {
        for index in 1..=short.len() {
            current[index] = if short[index - 1] == *line {
                previous[index - 1] + 1
            } else {
                previous[index].max(current[index - 1])
            };
        }
        std::mem::swap(&mut previous, &mut current);
        current.fill(0);
    }
    previous[short.len()]
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

/// The reference displays a file tool's path as `relativizeToCwd` then
/// `abbreviateHomePath` of the recorded argument (`tool-call-model.ts:261`,
/// `workspace-path/src/index.ts:107,44`). The earlier native form invented
/// `basename · parent/`, which the reference never draws.
fn path_label(path: &str, roots: PathRoots<'_>) -> String {
    let clean = compact(path, MAX_ARGUMENT_BYTES);
    let clean = clean.trim_end_matches(['/', '\\']);
    let relative = strip_root(clean, roots.cwd);
    let abbreviated = home_abbreviated(relative, roots.home);
    compact(&abbreviated, MAX_TARGET_BYTES)
}

/// Strip a workspace root prefix as `relativizeToCwd` does: only a path that
/// continues past the root loses it, and either separator ends the root. A
/// trailing separator on the root is ignored, as the reference ignores it.
fn strip_root<'a>(path: &'a str, cwd: Option<&str>) -> &'a str {
    let Some(cwd) = cwd.filter(|cwd| !cwd.is_empty()) else {
        return path;
    };
    let root = cwd.trim_end_matches(['/', '\\']);
    path.strip_prefix(root)
        .and_then(|rest| rest.strip_prefix(['/', '\\']))
        .unwrap_or(path)
}

/// Abbreviate the host home prefix as `~`, as `abbreviateHomePath` does. A
/// Windows-spelled path or home is left alone, since `~` is a POSIX spelling.
fn home_abbreviated(path: &str, home: Option<&str>) -> String {
    let Some(home) = home.filter(|home| !home.is_empty()) else {
        return path.into();
    };
    if windows_style(path) || windows_style(home) {
        return path.into();
    }
    let root = home.trim_end_matches('/');
    if root.is_empty() || root == "/" {
        return path.into();
    }
    if path.trim_end_matches('/') == root {
        return "~".into();
    }
    match path
        .strip_prefix(root)
        .and_then(|rest| rest.strip_prefix('/'))
    {
        Some(rest) => format!("~/{rest}"),
        None => path.into(),
    }
}

fn windows_style(value: &str) -> bool {
    let bytes = value.as_bytes();
    value.starts_with("\\\\")
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\'))
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

    /// Tests that do not care about path shortening call the production
    /// projection with no roots; a test about the display rule calls
    /// `super::summarize` with the roots it means.
    fn summarize<'a>(
        event: &SessionWireEvent,
        previous: impl Iterator<Item = &'a SessionWireEvent>,
    ) -> Option<Summary> {
        super::summarize(event, previous, PathRoots::default())
    }

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
                Some("/workspace/src/activity.rs")
            );
            assert!(!summary.failed);
            assert!(!format!("{summary:?}").contains("PRIVATE"));
        }
    }

    /// A file tool's label is the recorded path shortened the way the
    /// reference shortens it: workspace-relative first, then `~` for home
    /// (`tool-call-model.ts:261`, `workspace-path/src/index.ts:44,107`).
    #[test]
    fn a_file_path_displays_relative_to_the_workspace_and_from_home() {
        fn target(path: &str, cwd: Option<&str>, home: Option<&str>) -> Option<String> {
            super::summarize(
                &call(0, "a", "read", json!({ "file_path": path })),
                std::iter::empty(),
                PathRoots { cwd, home },
            )
            .unwrap()
            .target
        }
        for (path, cwd, home, expected) in [
            // Inside the workspace, relative to its root.
            (
                "/work/repo/src/ui.rs",
                Some("/work/repo"),
                None,
                "src/ui.rs",
            ),
            // A trailing separator on the root is the same root.
            (
                "/work/repo/src/ui.rs",
                Some("/work/repo/"),
                None,
                "src/ui.rs",
            ),
            // The root itself has nothing to strip and stays as recorded.
            ("/work/repo", Some("/work/repo"), None, "/work/repo"),
            // A sibling whose name merely shares the prefix is not inside it.
            (
                "/work/repository/a.rs",
                Some("/work/repo"),
                None,
                "/work/repository/a.rs",
            ),
            ("/etc/hosts", Some("/work/repo"), None, "/etc/hosts"),
            // Outside the workspace but under home: drawn from `~`.
            (
                "/home/jared/notes/a.md",
                Some("/work/repo"),
                Some("/home/jared"),
                "~/notes/a.md",
            ),
            ("/home/jared", None, Some("/home/jared"), "~"),
            // An already-relative argument is drawn as recorded.
            (
                "src/a.rs",
                Some("/work/repo"),
                Some("/home/jared"),
                "src/a.rs",
            ),
            // A Windows root is stripped too, but `~` is a POSIX spelling.
            (
                "C:\\repo\\a.rs",
                Some("C:\\repo"),
                Some("/home/jared"),
                "a.rs",
            ),
            (
                "C:\\Users\\a\\x.rs",
                None,
                Some("C:\\Users\\a"),
                "C:\\Users\\a\\x.rs",
            ),
        ] {
            assert_eq!(target(path, cwd, home).as_deref(), Some(expected), "{path}");
        }
        // The rendered target stays bounded even for a long unbroken path.
        let long = format!("/work/repo/{}", "a".repeat(400));
        let rendered = target(&long, Some("/work/repo"), None).unwrap();
        assert!(rendered.len() <= MAX_TARGET_BYTES, "{}", rendered.len());
        assert!(rendered.ends_with('…'));
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
        // A short path keeps its unicode name with the controls stripped; the
        // reference draws a path from its head, so a long one is clipped there.
        let short = label(&call(
            0,
            "a",
            "read",
            json!({"path":"/你好/文件\n\u{1b}\u{202e}名.rs"}),
        ));
        assert_eq!(short.target.as_deref(), Some("/你好/文件 名.rs"));
        let summary = label(&call(
            0,
            "a",
            "read",
            json!({"path":format!("/你好/{}/文件\n\u{1b}\u{202e}名.rs", "目录/".repeat(900))}),
        ));
        let target = summary.target.unwrap();
        assert!(target.starts_with("/你好/目录/"), "{target}");
        assert!(target.ends_with('…'), "{target}");
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
        let records = [
            call(0, "a", "read", json!({"file_path":"a.rs"})),
            result(1, "a", json!(false)),
        ];
        let mut transcript = Transcript::new(1);
        let frame: SessionFollowFrame = serde_json::from_value(json!({"type":"snapshot","cursor":1,"hasMore":false,"header":{"version":4,"id":"s1","createdAt":1000,"isSeeded":false},"projections":{"asOfSeq":1,"values":{}},"records":records.iter().map(|event|json!({"type":"event","event":event})).collect::<Vec<_>>()})).unwrap();
        transcript.apply(1, frame).unwrap();
        let before = transcript
            .rows()
            .iter()
            .map(|r| {
                (
                    r.key,
                    r.role.clone(),
                    r.kind.clone(),
                    r.chat,
                    r.text.clone(),
                    r.time,
                    r.surface,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(before[0].4, "read\n{\"file_path\":\"a.rs\"}");
        assert_eq!(before[1].4, "PRIVATE RESULT");
        let cursor = transcript.opening_cursor();
        let safe = transcript.safe_to_send();
        let stored = serde_json::to_value(&transcript.records).unwrap();
        let durable_cursor = transcript.cursor;
        let retained_bytes = transcript.retained_bytes;
        assert!(transcript.activity(0, PathRoots::default()).is_some());
        assert!(transcript.activity(1, PathRoots::default()).is_some());
        assert!(transcript.activity(999, PathRoots::default()).is_none());
        assert_eq!(
            before,
            transcript
                .rows()
                .iter()
                .map(|r| (
                    r.key,
                    r.role.clone(),
                    r.kind.clone(),
                    r.chat,
                    r.text.clone(),
                    r.time,
                    r.surface
                ))
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

    #[test]
    fn edit_calls_report_bounded_line_totals_without_content() {
        // Single contiguous replacement: one line added, one removed.
        let summary = label(&call(
            0,
            "a",
            "edit",
            json!({"file_path":"/workspace/app/src/ui.rs","old_string":"let a = 1;\nlet b = 2;","new_string":"let a = 1;\nlet c = 3;"}),
        ));
        assert_eq!(summary.edits, Some((1, 1)));
        assert_eq!(summary.title, "Edit · edit");
        assert_eq!(summary.target.as_deref(), Some("/workspace/app/src/ui.rs"));
        // Insertion and deletion only.
        assert_eq!(
            label(&call(
                0,
                "a",
                "edit",
                json!({"path":"x.rs","old_string":"one","new_string":"one\ntwo"}),
            ))
            .edits,
            Some((1, 0))
        );
        assert_eq!(
            label(&call(
                0,
                "a",
                "functions.edit",
                json!({"path":"x.rs","old_string":"one\ntwo","new_string":"one"}),
            ))
            .edits,
            Some((0, 1))
        );
        // Multiple separated hunks inside one replacement stay exact.
        assert_eq!(
            label(&call(
                0,
                "a",
                "edit",
                json!({"path":"x.rs","old_string":"a\nb\nc\nd","new_string":"a\nX\nc\nY"}),
            ))
            .edits,
            Some((2, 2))
        );
        // A re-derived result keeps the same call totals.
        let edit_call = call(
            0,
            "e",
            "edit",
            json!({"path":"x.rs","old_string":"a\nb","new_string":"a\nc"}),
        );
        let summary = summarize(&result(1, "e", json!(false)), [&edit_call].into_iter()).unwrap();
        assert_eq!(summary.title, "Edit · edit · result");
        assert_eq!(summary.edits, Some((1, 1)));
    }

    #[test]
    fn edit_totals_decline_unknown_oversized_and_sensitive_inputs() {
        // Missing half of the replacement, or unavailable arguments.
        assert_eq!(
            label(&call(
                0,
                "a",
                "edit",
                json!({"path":"x.rs","old_string":"a"})
            ))
            .edits,
            None
        );
        assert_eq!(label(&call(0, "a", "edit", json!({}))).edits, None);
        // Oversized line counts decline instead of running an unbounded diff.
        let oversized = "line\n".repeat(MAX_DIFF_LINES + 1);
        assert_eq!(
            label(&call(
                0,
                "a",
                "edit",
                json!({"path":"x.rs","old_string":oversized,"new_string":"one"}),
            ))
            .edits,
            None
        );
        // A sensitive sibling field suppresses totals with the label.
        let summary = label(&call(
            0,
            "a",
            "edit",
            json!({"path":"x.rs","old_string":"a","new_string":"b","token":"PRIVATE"}),
        ));
        assert_eq!(summary.edits, None);
        assert_eq!(summary.target.as_deref(), Some(ARGUMENTS_AVAILABLE));
        // Non-edit verbs never fabricate totals from edit-shaped fields.
        for (name, args) in [
            (
                "read",
                json!({"file_path":"x.rs","old_string":"a","new_string":"b"}),
            ),
            ("bash", json!({"command":"echo a"})),
            ("write", json!({"path":"x.rs","content":"a\nb"})),
        ] {
            assert_eq!(label(&call(0, "a", name, args)).edits, None);
        }
        // Counts are integers, so diagnostics never echo the edited text.
        let summary = label(&call(
            0,
            "a",
            "edit",
            json!({"path":"x.rs","old_string":"PRIVATE OLD","new_string":"PRIVATE NEW"}),
        ));
        assert_eq!(summary.edits, Some((1, 1)));
        assert!(!format!("{summary:?}").contains("PRIVATE"));
    }

    #[test]
    fn line_diff_totals_handle_empty_identical_and_unicode_lines() {
        assert_eq!(line_diff_totals("", ""), Some((0, 0)));
        assert_eq!(line_diff_totals("", "new"), Some((1, 0)));
        assert_eq!(line_diff_totals("old", ""), Some((0, 1)));
        assert_eq!(line_diff_totals("same\nsame", "same\nsame"), Some((0, 0)));
        assert_eq!(line_diff_totals("世界\ncafé", "世界\ncafe"), Some((1, 1)));
        assert_eq!(line_diff_totals("a\nb\nc", "c\nb\na"), Some((2, 2)));
    }

    /// Staged context management is log-only, so these labels are the only place
    /// the user sees what was reclaimed. Only logged counts are shown.
    #[test]
    fn context_management_labels_report_only_logged_counts() {
        let pruned = label(&event(
            7,
            "compaction/prune",
            json!({"shadowedRange":{"start":5,"end":40},"shadowedSeqs":[5,9,12,40],
                "shadowedTokenCount":34000,"content":[{"type":"text","text":"PRIVATE PRUNED BODY"}]}),
        ));
        assert_eq!(pruned.title, "Context elided · 4 records → stubs");
        assert_eq!(pruned.target.as_deref(), Some("~34k tokens reclaimed"));
        assert!(!pruned.failed);
        assert!(!format!("{pruned:?}").contains("PRIVATE"));

        let summarized = label(&event(
            8,
            "compaction/summary",
            json!({"compactionId":"c1","summary":[{"type":"text","text":"PRIVATE SUMMARY BODY"}],
                "shadowedRange":{"start":5,"end":208},"shadowedSeqs":[5,6,208],
                "shadowedTokenCount":96000,"provider":"PUBLIC","model":"public-model","llmStreamCall":true}),
        ));
        assert_eq!(summarized.title, "Context summarized · seqs 5–208");
        assert_eq!(summarized.target.as_deref(), Some("~96k tokens reclaimed"));
        assert!(!summarized.failed);
        // The summary body belongs to Details, never this line.
        assert!(!format!("{summarized:?}").contains("PRIVATE"));

        let failed = label(&event(
            9,
            "compaction/end",
            json!({"compactionId":"c1","turn":4,"error":"summarizer unavailable"}),
        ));
        assert_eq!(failed.title, "Compaction failed");
        assert_eq!(failed.target.as_deref(), Some("summarizer unavailable"));
        assert!(failed.failed);

        let closed = summarize(
            &event(10, "compaction/end", json!({"compactionId":"c1","turn":4})),
            std::iter::empty(),
        );
        assert!(closed.is_none());
    }

    /// The recorded shape, not a consensus of my own assumptions.
    ///
    /// No session on this machine ever compacted, so the payload below is copied
    /// field-for-field from a session the backend's own snapshot suite recorded
    /// against a real summarization call
    /// (`deepseek-harness-linux/snapshots/session/compaction-summary-headroom/session.v4.jsonl`).
    /// If the backend's recorded event shape moves, this fails here rather than in
    /// a user's session. `compaction/prune` has never been recorded: its shape is
    /// pinned from the producer instead
    /// (`compaction-tool-result-pruner/src/index.ts`, the append beside the
    /// replacement `tool/result`).
    #[test]
    fn the_recorded_compaction_shape_labels_as_documented() {
        let recorded_summary = json!({
            "compactionId": "{{id:1}}",
            "summary": [{"type": "text", "text": "The request established a durable compaction premise."}],
            "rawOutput": [{"type": "text", "text": "The request established a durable compaction premise."}],
            "llmStreamCall": true,
            "shadowedRange": {"start": 8, "end": 9},
            "shadowedSeqs": [8, 9],
            "shadowedTokenCount": 375,
            "provider": "deepseek-official",
            "model": "deepseek-v4-flash",
            "maxTokens": 1800,
            "usage": {"inputTokens": 20, "outputTokens": 4}
        });
        let summary = label(&event(7, "compaction/summary", recorded_summary.clone()));
        assert_eq!(summary.title, "Context summarized · seqs 8–9");
        assert_eq!(summary.target.as_deref(), Some("~375 tokens reclaimed"));
        assert!(!summary.failed);
        // The recorded summary body stays in the record, never on the row.
        assert!(!format!("{summary:?}").contains("durable compaction premise"));

        // The recorded brackets carry only the transaction identity.
        for (seq, kind) in [(6u64, "compaction/start"), (8, "compaction/end")] {
            assert!(
                summarize(
                    &event(seq, kind, json!({"compactionId": "{{id:1}}", "turn": 1})),
                    std::iter::empty()
                )
                .is_none(),
                "{kind} must produce no row"
            );
        }

        // The prune producer's shadow-price shape, which no recording contains.
        let pruned = label(&event(
            9,
            "compaction/prune",
            json!({"shadowedRange": {"start": 12, "end": 12}, "shadowedSeqs": [12],
                "shadowedTokenCount": 375}),
        ));
        assert_eq!(pruned.title, "Context elided · 1 record → stubs");
        assert_eq!(pruned.target.as_deref(), Some("~375 tokens reclaimed"));
    }
    /// Absent, malformed and unbounded payloads degrade to a bounded label
    /// instead of a guessed count or a fabricated tier.
    #[test]
    fn context_management_labels_survive_missing_and_hostile_fields() {
        let empty = label(&event(
            1,
            "compaction/prune",
            json!({"shadowedRange":{"start":0,"end":0},"shadowedSeqs":[]}),
        ));
        assert_eq!(empty.title, "Context elided · count available in details");
        assert!(empty.target.is_none());

        for bad in [
            json!(-1),
            json!(1.5),
            json!("12"),
            Value::Null,
            json!(9007199254740992u64),
        ] {
            let summary = label(&event(
                2,
                "compaction/summary",
                json!({"shadowedRange":{"start":1,"end":2},"shadowedTokenCount":bad}),
            ));
            assert!(summary.target.is_none(), "{bad}");
            assert_eq!(summary.title, "Context summarized · seqs 1–2");
        }
        let no_range = label(&event(3, "compaction/summary", json!({})));
        assert_eq!(
            no_range.title,
            "Context summarized · range available in details"
        );
        let long_error = label(&event(
            4,
            "compaction/end",
            json!({"compactionId":"c1","error":"e".repeat(4096)}),
        ));
        assert!(long_error.target.as_deref().unwrap().len() <= 96);
        let tiny = label(&event(
            5,
            "compaction/prune",
            json!({"shadowedTokenCount":999}),
        ));
        assert_eq!(tiny.target.as_deref(), Some("~999 tokens reclaimed"));
    }

    /// The reminder is a `user/message` carrying the guard's own `source`; the
    /// tool name and run length come from that producer summary.
    #[test]
    fn loop_guard_labels_use_the_producer_summary_without_recounting() {
        let reminder = label(&event(
            3,
            "user/message",
            json!({"content":[{"type":"text","text":"PRIVATE REMINDER TEXT"}],
                "source":{"kind":"repeat-tool-reminder","form":"notice","summary":"bash × 8"}}),
        ));
        assert_eq!(reminder.title, "Loop guard · repeated identical call");
        assert_eq!(reminder.target.as_deref(), Some("bash × 8"));
        assert!(!reminder.failed);
        assert!(!format!("{reminder:?}").contains("PRIVATE"));

        for source in [
            json!({"kind":"repeat-tool-reminder"}),
            json!({"kind":"repeat-tool-reminder","summary":"bash"}),
            json!({"kind":"repeat-tool-reminder","summary":"× 8"}),
            json!({"kind":"repeat-tool-reminder","summary":12}),
        ] {
            let summary = label(&event(
                4,
                "user/message",
                json!({"content":[],"source":source}),
            ));
            assert_eq!(summary.title, "Loop guard · repeated identical call");
            assert!(summary.target.is_none(), "{source}");
        }
        // A human turn is not a guard reminder.
        assert!(
            summarize(
                &event(
                    5,
                    "user/message",
                    json!({"content":[{"type":"text","text":"hello"}],"source":{"kind":"user"}}),
                ),
                std::iter::empty(),
            )
            .is_none()
        );
    }
}
