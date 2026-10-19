//! The `bash` call's terminal card, derived the way the reference derives it
//! (`terminal-card-model.ts:283-341`, `TerminalBlock.tsx:191-266`).
//!
//! A shell call settles a *failing* command as a completed call: `isError` stays false and the exit
//! status is result data, carried as a trailing `[exit code: N]` or `[killed by signal: X]` marker
//! the shell renderer appends. The card extracts that marker, drops it from the output, and states
//! the status in a pill. Everything the reference declines to draw is declined here too: a
//! persistent shell (no `description`), a background call, an error result, a non-shell call, or
//! arguments that fail a rule all keep the recorded text body.
use serde_json::Value;

/// Output lines drawn before the middle collapses (`TerminalBlock.tsx:12`).
pub const MAX_ROWS: usize = 16;
/// Per-line bound. The shell caps a line upstream; a forged record must not grow a card.
const MAX_LINE_BYTES: usize = 2048;
/// Bound on the drawn output; the card never keeps more than this many lines.
const MAX_LINES: usize = 512;

#[derive(Clone, PartialEq, Eq)]
pub struct Card {
    /// The prompt label: the working directory's last segment, `~` for home, or the path itself.
    pub cwd: Option<String>,
    /// The command, split into prompt lines the way the reference splits it (`:203-206`).
    pub command: Vec<String>,
    /// The output without its status marker, one entry per drawn line.
    pub output: Vec<String>,
    /// The status pill's text, absent for a clean settle (`:115-123`).
    pub status: Option<String>,
}

/// The prompt, the command and the output are all user content, so the debug form names the card
/// and its row counts instead of printing them — the same rule as the read and diff cards.
impl std::fmt::Debug for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Card")
            .field("cwd", &self.cwd)
            .field("command", &format_args!("{} lines", self.command.len()))
            .field("output", &format_args!("{} lines", self.output.len()))
            .field("status", &self.status)
            .finish()
    }
}

impl Card {
    /// The reference's head/tail cap (`head-tail-cap.ts`, `ReadBlock.tsx:106-109`).
    pub fn shown(&self, expanded: bool) -> Shown<'_> {
        let hidden = self.output.len().saturating_sub(MAX_ROWS);
        if expanded || hidden == 0 {
            return Shown {
                head: &self.output,
                tail: &[],
                hidden: 0,
            };
        }
        let head = MAX_ROWS.div_ceil(2);
        let tail = MAX_ROWS - head;
        Shown {
            head: &self.output[..head],
            tail: &self.output[self.output.len() - tail..],
            hidden: hidden as u64,
        }
    }
    pub fn drawn_rows(&self, expanded: bool) -> usize {
        let shown = self.shown(expanded);
        // The prompt rows, the output rows, one fold row when capped, and the `No output` row.
        self.command.len() + shown.head.len() + shown.tail.len() + usize::from(shown.hidden > 0)
    }
}

pub struct Shown<'a> {
    pub head: &'a [String],
    pub tail: &'a [String],
    pub hidden: u64,
}

/// Strip SGR/OSC/CSI escape sequences. The reference parses ANSI into styled runs; native has no
/// run renderer, so the bytes are removed rather than drawn as literal escape text.
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            // CSI: parameters then a final byte in @..~.
            Some('[') => {
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            // OSC: terminated by BEL or ST.
            Some(']') => {
                while let Some(next) = chars.next() {
                    if next == '\u{7}' {
                        break;
                    }
                    if next == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            // Any other two-byte sequence, and a lone trailing escape.
            _ => {}
        }
    }
    out
}

/// The reference's `parseExitStatus`: a trailing marker is status, not output (`:222-232`).
fn parse_exit_status(text: &str) -> (String, Option<u32>, Option<String>) {
    if let Some(at) = text.rfind("\n[killed by signal: ") {
        let rest = &text[at + "\n[killed by signal: ".len()..];
        if let Some(name) = rest.strip_suffix(']') {
            return (text[..at].to_owned(), None, Some(name.to_owned()));
        }
    }
    if let Some(at) = text.rfind("\n[exit code: ") {
        let rest = &text[at + "\n[exit code: ".len()..];
        if let Some(code) = rest.strip_suffix(']').and_then(|n| n.parse::<u32>().ok()) {
            return (text[..at].to_owned(), Some(code), None);
        }
    }
    (text.to_owned(), Some(0), None)
}

fn positive_number(value: Option<&Value>) -> bool {
    value.is_none_or(|value| value.as_f64().is_some_and(|n| n.is_finite() && n > 0.0))
}

/// Resolve the shell's working directory and its prompt label (`:139-163`, `:97-101`).
fn prompt_label(
    workdir: Option<&str>,
    roots: crate::reducer::activity::PathRoots<'_>,
) -> Option<String> {
    let joined = match (workdir, roots.cwd) {
        (None | Some(""), Some(cwd)) => cwd.to_owned(),
        (None | Some(""), None) => return None,
        (Some(workdir), Some(cwd)) if !workdir.starts_with('/') => {
            format!("{}/{}", cwd.trim_end_matches('/'), workdir)
        }
        (Some(workdir), _) => workdir.to_owned(),
    };
    let collapsed = collapse_segments(&joined);
    let trimmed = collapsed.trim_end_matches(['/', '\\']);
    if roots
        .home
        .is_some_and(|home| trimmed == home.trim_end_matches(['/', '\\']))
    {
        return Some("~".into());
    }
    let segment = trimmed
        .rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(trimmed);
    (!segment.is_empty()).then(|| segment.to_owned())
}

/// Drop `.` segments and resolve `..` against what precedes it, as the reference's `collapse` does.
fn collapse_segments(path: &str) -> String {
    let rooted = path.starts_with('/');
    let mut kept: Vec<&str> = Vec::new();
    for segment in path.split(['/', '\\']) {
        match segment {
            "" | "." => {}
            ".." => {
                if kept.last().is_some_and(|last| *last != "..") {
                    kept.pop();
                } else if !rooted {
                    kept.push(segment);
                }
            }
            other => kept.push(other),
        }
    }
    let body = kept.join("/");
    if rooted { format!("/{body}") } else { body }
}

/// Derive the terminal card for a settled shell call, when every rule holds.
pub fn derive(
    call: &Value,
    result: &Value,
    roots: crate::reducer::activity::PathRoots<'_>,
) -> Option<Card> {
    let name = call["name"].as_str()?;
    if !matches!(
        name,
        "bash" | "pwsh" | "functions.bash" | "functions.pwsh" | "tool-shell/bash"
    ) {
        return None;
    }
    if result["message"]["isError"].as_bool() == Some(true) {
        return None;
    }
    let args: Value = serde_json::from_str(call["arguments"].as_str()?).ok()?;
    let command = args["command"].as_str()?;
    if command.trim().is_empty() {
        return None;
    }
    if !positive_number(args.get("timeoutMs")) {
        return None;
    }
    if args
        .get("workdir")
        .is_some_and(|value| value.as_str().is_none())
    {
        return None;
    }
    if args
        .get("run_in_background")
        .is_some_and(|value| value.as_bool().is_none())
    {
        return None;
    }
    if args.get("run_in_background").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    // A shell without `description` is the persistent provider, whose result has no single exit
    // status; the reference keeps that on the generic path (`:203-215`).
    match args.get("description") {
        None => return None,
        Some(value) => {
            if value.as_str().is_none_or(|text| text.trim().is_empty()) {
                return None;
            }
        }
    }
    let workdir = args.get("workdir").and_then(Value::as_str);
    let text = super::content(&result["message"]["content"]);
    let (output, code, signal) = parse_exit_status(&strip_ansi(&text));
    let status = match (&signal, code) {
        (Some(signal), _) => Some(format!("signal {signal}")),
        (None, Some(code)) if code != 0 => Some(format!("exit code {code}")),
        _ => None,
    };
    let lines: Vec<String> = if output.trim().is_empty() {
        vec!["No output".to_owned()]
    } else {
        let body = output.strip_suffix('\n').unwrap_or(&output);
        body.split('\n')
            .take(MAX_LINES)
            .map(|line| crate::reducer::bounded(line, MAX_LINE_BYTES))
            .collect()
    };
    let command_body = command.strip_suffix('\n').unwrap_or(command);
    Some(Card {
        cwd: prompt_label(workdir, roots),
        command: command_body.split('\n').map(str::to_owned).collect(),
        output: lines,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reducer::activity::PathRoots;
    use serde_json::json;

    fn call(args: Value) -> Value {
        json!({"name": "functions.bash", "arguments": args.to_string()})
    }
    fn result(text: &str) -> Value {
        json!({"message": {"isError": false, "content": [{"type": "text", "text": text}]}})
    }
    fn args(extra: Value) -> Value {
        let mut base = json!({"command": "cargo test", "description": "PUBLIC run the suite"});
        if let (Some(base), Some(extra)) = (base.as_object_mut(), extra.as_object()) {
            for (key, value) in extra {
                base.insert(key.clone(), value.clone());
            }
        }
        base
    }
    fn roots() -> PathRoots<'static> {
        PathRoots {
            cwd: Some("/work/app"),
            home: Some("/home/user"),
        }
    }

    #[test]
    fn a_clean_settle_has_no_pill() {
        let card = derive(
            &call(args(json!({}))),
            &result("running 5 tests\nall passed\n[exit code: 0]"),
            roots(),
        )
        .expect("a settled shell call");
        assert_eq!(card.cwd.as_deref(), Some("app"));
        assert_eq!(card.command, vec!["cargo test"]);
        assert_eq!(card.output, vec!["running 5 tests", "all passed"]);
        assert!(card.status.is_none(), "{:?}", card.status);
    }

    #[test]
    fn a_failing_command_is_a_completed_call_with_a_pill() {
        // The bash tool settles a failing command as a completed call: isError stays false and
        // the exit status is result data (`terminal-card-model.ts:86-92`).
        let card = derive(
            &call(args(json!({}))),
            &result("error: something broke\n[exit code: 2]"),
            roots(),
        )
        .unwrap();
        assert_eq!(card.status.as_deref(), Some("exit code 2"));
        assert_eq!(card.output, vec!["error: something broke"]);
    }

    #[test]
    fn a_signal_takes_precedence_and_names_itself() {
        let card = derive(
            &call(args(json!({}))),
            &result("partial\n[killed by signal: SIGTERM]"),
            roots(),
        )
        .unwrap();
        assert_eq!(card.status.as_deref(), Some("signal SIGTERM"));
        assert_eq!(card.output, vec!["partial"]);
    }

    #[test]
    fn no_output_says_so_and_escape_bytes_are_not_drawn() {
        let card = derive(
            &call(args(json!({}))),
            &result("\u{1b}[31m\u{1b}[0m\n[exit code: 0]"),
            roots(),
        )
        .unwrap();
        assert_eq!(card.output, vec!["No output"]);
        let card = derive(
            &call(args(json!({}))),
            &result("\u{1b}[32mok\u{1b}[0m\n[exit code: 0]"),
            roots(),
        )
        .unwrap();
        assert_eq!(card.output, vec!["ok"]);
    }

    #[test]
    fn a_multiline_command_keeps_one_prompt_row_per_line() {
        let card = derive(
            &call(args(json!({"command": "cd src\ncargo test\n"}))),
            &result("ok\n[exit code: 0]"),
            roots(),
        )
        .unwrap();
        assert_eq!(card.command, vec!["cd src", "cargo test"]);
    }

    #[test]
    fn the_cap_keeps_eight_and_eight() {
        let output = (1..=20)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let card = derive(
            &call(args(json!({}))),
            &result(&format!("{output}\n[exit code: 0]")),
            roots(),
        )
        .unwrap();
        let shown = card.shown(false);
        assert_eq!(shown.head.len(), 8);
        assert_eq!(shown.tail.len(), 8);
        assert_eq!(shown.hidden, 4);
        assert_eq!(shown.head[0], "line 1");
        assert_eq!(shown.tail[7], "line 20");
        assert_eq!(card.shown(true).head.len(), 20);
    }

    #[test]
    fn the_cases_the_reference_refuses_keep_the_text_body() {
        let cases = [
            // Persistent shell (no description), background, an error result, a bad workdir,
            // a zero timeout, an empty command.
            (call(json!({"command": "cargo test"})), json!({})),
            (call(args(json!({"run_in_background": true}))), json!({})),
            (
                call(args(json!({}))),
                json!({"message": {"isError": true, "content": []}}),
            ),
            (call(args(json!({"workdir": 4}))), json!({})),
            (call(args(json!({"timeoutMs": 0}))), json!({})),
            (call(args(json!({"command": "   "}))), json!({})),
        ];
        for (call, extra) in cases {
            let mut result = result("ok\n[exit code: 0]");
            if let Some(object) = extra.as_object() {
                for (key, value) in object {
                    result[key] = value.clone();
                }
            }
            assert!(derive(&call, &result, roots()).is_none(), "{call}");
        }
        assert!(
            derive(
                &json!({"name": "functions.read", "arguments": "{}"}),
                &result("ok"),
                roots()
            )
            .is_none()
        );
    }

    #[test]
    fn the_prompt_label_resolves_home_and_relative_workdirs() {
        assert_eq!(prompt_label(None, roots()).as_deref(), Some("app"));
        assert_eq!(prompt_label(Some(""), roots()).as_deref(), Some("app"));
        assert_eq!(prompt_label(Some(".."), roots()).as_deref(), Some("work"));
        assert_eq!(prompt_label(Some("/etc"), roots()).as_deref(), Some("etc"));
        assert_eq!(
            prompt_label(Some("/home/user"), roots()).as_deref(),
            Some("~")
        );
        // A relative workdir with no session cwd stays as authored.
        assert_eq!(
            prompt_label(
                Some("src/lib"),
                PathRoots {
                    cwd: None,
                    home: None
                }
            )
            .as_deref(),
            Some("lib")
        );
        assert_eq!(
            prompt_label(
                None,
                PathRoots {
                    cwd: None,
                    home: None
                }
            ),
            None
        );
    }
}
