//! The reference's command row: one slash-command run, its outcome and its output
//! (`command.ts:36-70`, `GenericCommandCard.tsx:30-60`, `locale.ts:378-381`).
//!
//! The Host records a command as a `command/run` and later a `command/done`, both keyed by a
//! `commandId`. Chat draws one row per run — the command's name, then `Running…`, the outcome's own
//! text, or `Completed`/`Command failed` — and the multi-line output expands beneath it. Without
//! this, a slash command that produced only a log record was invisible in the conversation.
use serde_json::Value;

const MAX_TEXT_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    Ok,
    Failed,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub args: Option<String>,
    pub state: State,
    /// The settled outcome's text, when the Host recorded one.
    pub text: Option<String>,
    /// A manual `/compact`'s structured counts, taken from the compaction summary the command
    /// produced: the reference draws them as one marker instead of the command text
    /// (`CompactionItem.tsx:56-66`, `locale.ts:331`).
    pub manual_compaction: Option<(u64, u64)>,
}

impl Row {
    /// The reference's summary: `Running…` while unsettled; the outcome's text when it is a single
    /// line; otherwise `Completed` or `Command failed` (`GenericCommandCard.tsx:44-52`).
    pub fn summary(&self) -> String {
        if self.state == State::Running {
            // A manual compaction says what it is doing, as the reference's running copy does.
            return if self.name == "compact" {
                "Compacting context…".to_owned()
            } else {
                "Running…".to_owned()
            };
        }
        if let Some((items, tokens)) = self.manual_compaction {
            return format!(
                "Compacted {items} history item{} (~{} tokens)",
                if items == 1 { "" } else { "s" },
                crate::reducer::command_row::compact_tokens(tokens)
            );
        }
        match self.text.as_deref() {
            Some(text) if !text.contains('\n') && !text.trim().is_empty() => {
                crate::reducer::bounded(text.trim(), MAX_TEXT_BYTES)
            }
            _ if self.state == State::Failed => "Command failed".to_owned(),
            _ => "Completed".to_owned(),
        }
    }
    /// The expandable body: the outcome's text only when it is multi-line.
    pub fn body(&self) -> Option<&str> {
        self.text
            .as_deref()
            .filter(|text| text.contains('\n') && !text.trim().is_empty())
    }
}

impl std::fmt::Debug for Row {
    /// Command arguments and output are user data; the debug form states their sizes.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Row")
            .field("name", &self.name)
            .field("args_bytes", &self.args.as_ref().map(String::len))
            .field("state", &self.state)
            .field("text_bytes", &self.text.as_ref().map(String::len))
            .finish()
    }
}

/// The reference's token figure: whole thousands from a thousand up (`locale.ts:331` shows the
/// raw count; the compact form keeps the row to one line).
pub fn compact_tokens(tokens: u64) -> String {
    if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

/// The command a `command/run` record opens, or `None` when the record cannot name one.
pub fn run(data: &Value) -> Option<(String, Row)> {
    let id = data["commandId"].as_str().filter(|id| !id.is_empty())?;
    let name = data["name"].as_str().filter(|name| !name.is_empty())?;
    let args = data["args"]
        .as_str()
        .filter(|args| !args.trim().is_empty())
        .map(|args| crate::reducer::bounded(args, MAX_TEXT_BYTES));
    Some((
        id.to_owned(),
        Row {
            name: crate::reducer::bounded(name, 128),
            args,
            state: State::Running,
            text: None,
            manual_compaction: None,
        },
    ))
}

/// Settle one row from a `command/done` record; an unrelated outcome changes nothing.
pub fn done(row: &mut Row, data: &Value) {
    let state = match data["kind"].as_str() {
        Some("success") => State::Ok,
        Some("error") => State::Failed,
        // A kind this build does not know cannot settle a command.
        _ => return,
    };
    row.state = state;
    row.text = data["text"]
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(|text| crate::reducer::bounded(text, MAX_TEXT_BYTES * 4));
}

#[cfg(test)]
mod tests {
    //! One row per command run, settled by its own `done`.
    use super::*;
    use serde_json::json;

    fn run_data(name: &str, args: Option<&str>) -> Value {
        json!({"commandId": "c1", "name": name, "args": args})
    }

    #[test]
    fn a_run_states_its_name_and_that_it_is_running() {
        let (id, row) = run(&run_data("/compact", Some("keep the plan"))).unwrap();
        assert_eq!(id, "c1");
        assert_eq!(row.name, "/compact");
        assert_eq!(row.args.as_deref(), Some("keep the plan"));
        assert_eq!(row.summary(), "Running…");
        assert_eq!(row.body(), None);
    }

    #[test]
    fn an_outcome_states_its_text_or_the_generic_verdict() {
        let (_, mut row) = run(&run_data("/compact", None)).unwrap();
        done(&mut row, &json!({"commandId": "c1", "kind": "success"}));
        assert_eq!(row.state, State::Ok);
        assert_eq!(row.summary(), "Completed");
        // A single-line text replaces the verdict, exactly as the reference draws it.
        done(
            &mut row,
            &json!({"commandId": "c1", "kind": "success", "text": "Compacted 12 records"}),
        );
        assert_eq!(row.summary(), "Compacted 12 records");
        assert_eq!(row.body(), None);
        // A multi-line text expands, and the summary falls back to the verdict.
        done(
            &mut row,
            &json!({"commandId": "c1", "kind": "error", "text": "line one\nline two"}),
        );
        assert_eq!(row.state, State::Failed);
        assert_eq!(row.summary(), "Command failed");
        assert_eq!(row.body(), Some("line one\nline two"));
    }

    #[test]
    fn a_record_that_cannot_name_a_command_opens_nothing() {
        assert!(run(&json!({"name": "/compact"})).is_none());
        assert!(run(&json!({"commandId": "c1"})).is_none());
        assert!(run(&json!({"commandId": "", "name": "/compact"})).is_none());
        assert!(run(&json!({"commandId": "c1", "name": ""})).is_none());
        // An unknown outcome kind does not settle anything.
        let (_, mut row) = run(&run_data("/compact", None)).unwrap();
        done(
            &mut row,
            &json!({"commandId": "c1", "kind": "unknown", "text": "x"}),
        );
        assert_eq!(row.state, State::Running);
        assert_eq!(row.text, None);
    }

    #[test]
    fn the_debug_form_states_sizes_not_words() {
        let (_, mut row) = run(&run_data("/compact", Some("PRIVATE-ARGS"))).unwrap();
        done(
            &mut row,
            &json!({"commandId": "c1", "kind": "success", "text": "PRIVATE-OUTPUT"}),
        );
        let debug = format!("{row:?}");
        assert!(!debug.contains("PRIVATE"), "{debug}");
    }
}
