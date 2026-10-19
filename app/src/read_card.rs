//! The `read` call's line-numbered card, derived the way the reference derives it
//! (`read-card-model.ts:104-137`, `ReadBlock.tsx:106-149`).
//!
//! The card needs three things a compact summary alone cannot give: the call's own arguments
//! (`file_path`, `offset`, `limit`), the result's persisted `meta` (path, offset, numbered lines,
//! `totalLines`, optional `lang`), and the model-facing envelope around the content. This module
//! validates all three with the reference's own rules — a metadata field that fails a rule drops
//! the card rather than being repaired — and returns a bounded, display-ready model. The text stays
//! here: [`Card`]'s `Debug` prints the line count, never the content.
use serde_json::Value;

/// Content lines the chat row shows before the middle collapses. A design constant of the row
/// geometry, not a deployment choice (`read-card-model.ts:18-23`).
pub const MAX_ROWS: usize = 8;
/// Per-line bound; the read tool caps a line upstream, and a forged record must not grow a card.
const MAX_LINE_BYTES: usize = 512;
/// Line-count bound; a card never keeps more than this many numbered lines.
const MAX_LINES: usize = 512;

#[derive(Clone, PartialEq, Eq)]
pub struct Line {
    pub number: u64,
    pub text: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Card {
    /// The relativized, home-abbreviated path the reference labels the card with.
    pub label: String,
    pub lines: Vec<Line>,
    pub total_lines: u64,
    /// The reference's toolbar language, when the read recorded one.
    pub lang: Option<String>,
    /// The read returned fewer lines than the file holds, so the reference states the window.
    pub windowed: bool,
}

/// The rows a card draws at one expansion state: `head`, a fold marker when capped, and `tail`.
pub struct Shown<'a> {
    pub head: &'a [Line],
    pub tail: &'a [Line],
    pub hidden: u64,
}

impl Card {
    pub fn shown(&self, expanded: bool) -> Shown<'_> {
        let hidden = self.lines.len().saturating_sub(MAX_ROWS) as u64;
        if expanded || hidden == 0 {
            return Shown {
                head: &self.lines,
                tail: &[],
                hidden: 0,
            };
        }
        let head = MAX_ROWS.div_ceil(2);
        let tail = MAX_ROWS - head;
        Shown {
            head: &self.lines[..head],
            tail: &self.lines[self.lines.len() - tail..],
            hidden,
        }
    }
    /// The reference's CodeToolbar status, present only for a windowed read
    /// (`ReadBlock.tsx:127-129`).
    pub fn status(&self) -> Option<String> {
        self.windowed
            .then(|| format!("Showing {} of {} lines", self.lines.len(), self.total_lines))
    }
    pub fn drawn_rows(&self, expanded: bool) -> usize {
        let shown = self.shown(expanded);
        shown.head.len() + shown.tail.len() + usize::from(shown.hidden > 0)
    }
}

impl std::fmt::Debug for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Card")
            .field("label", &self.label)
            .field("lines", &format_args!("{} lines", self.lines.len()))
            .field("total_lines", &self.total_lines)
            .field("lang", &self.lang)
            .field("windowed", &self.windowed)
            .finish()
    }
}

fn positive_integer(value: Option<&Value>) -> Option<u64> {
    value.and_then(Value::as_u64).filter(|number| *number >= 1)
}

/// The reference's `readMeta`: every field checked, nothing repaired.
fn meta(value: &Value) -> Option<Card> {
    let record = value.as_object()?;
    // Validated because the reference validates it; the card's label comes from the call's
    // arguments, which is what the user asked to read.
    let _path = record.get("path")?.as_str()?;
    let offset = positive_integer(record.get("offset"))?;
    let total_lines = record.get("totalLines")?.as_u64()?;
    let raw_lines = record.get("lines")?.as_array()?;
    let lang = match record.get("lang") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_str()?.to_owned()),
    };
    if raw_lines.len() > MAX_LINES {
        return None;
    }
    let mut lines = Vec::with_capacity(raw_lines.len());
    let mut previous = offset.saturating_sub(1);
    for line in raw_lines {
        let entry = line.as_object()?;
        let number = positive_integer(entry.get("number"))?;
        if number <= previous || number > total_lines {
            return None;
        }
        previous = number;
        lines.push(Line {
            number,
            text: crate::reducer::bounded(entry.get("text")?.as_str()?, MAX_LINE_BYTES),
        });
    }
    Some(Card {
        label: String::new(),
        windowed: (lines.len() as u64) < total_lines,
        lines,
        total_lines,
        lang,
    })
}

/// The body of the model-facing read envelope, or `None` when the text is not that envelope.
pub fn envelope_body(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("<path>")?;
    let start = rest.find("</path>\n<type>file</type>\n<content>\n")?;
    let after = &rest[start + "</path>\n<type>file</type>\n<content>\n".len()..];
    after.strip_suffix("\n</content>")
}

/// Derive the read card for a call, when every rule holds.
pub fn derive(
    call: &Value,
    meta_value: &Value,
    output: &str,
    roots: crate::reducer::activity::PathRoots<'_>,
) -> Option<Card> {
    // The reference compares the *parsed* tool name, which is `read`; an alpha record stores the
    // namespaced spelling, so the aliases are the same ones `activity` labels with.
    if !matches!(
        call["name"].as_str(),
        Some("read" | "functions.read" | "tool-fs/read")
    ) {
        return None;
    }
    let arguments = call["arguments"].as_str()?;
    let parsed: Value = serde_json::from_str(arguments).ok()?;
    let path = parsed["file_path"].as_str()?.trim();
    if path.is_empty() {
        return None;
    }
    for key in ["offset", "limit"] {
        if let Some(value) = parsed.get(key)
            && (value.as_u64().is_none() || value.as_u64() == Some(0))
        {
            return None;
        }
    }
    let mut card = meta(meta_value)?;
    envelope_body(output)?;
    card.label = super::activity::path_label(path, roots);
    Some(card)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(args: Value) -> Value {
        json!({"name": "read", "arguments": args.to_string()})
    }
    fn meta(lines: &[(u64, &str)], offset: u64, total: u64) -> Value {
        json!({
            "path": "/home/user/project/src/lib.rs",
            "offset": offset,
            "totalLines": total,
            "lines": lines.iter().map(|(number, text)| json!({"number": number, "text": text})).collect::<Vec<_>>(),
        })
    }
    fn envelope(body: &str) -> String {
        format!(
            "<path>/home/user/project/src/lib.rs</path>\n<type>file</type>\n<content>\n{body}\n</content>"
        )
    }
    fn roots() -> crate::reducer::activity::PathRoots<'static> {
        crate::reducer::activity::PathRoots {
            cwd: Some("/home/user/project"),
            home: Some("/home/user"),
        }
    }

    #[test]
    fn a_real_metadata_read_becomes_a_card() {
        let body = "first\nsecond\nthird";
        let card = derive(
            &call(json!({"file_path": "/home/user/project/src/lib.rs"})),
            &meta(&[(1, "first"), (2, "second"), (3, "third")], 1, 9),
            &envelope(body),
            roots(),
        )
        .expect("a complete read");
        assert_eq!(card.label, "src/lib.rs");
        assert_eq!(card.lines.len(), 3);
        assert_eq!(card.total_lines, 9);
        assert!(card.windowed);
        assert_eq!(card.status().as_deref(), Some("Showing 3 of 9 lines"));
        assert_eq!(card.drawn_rows(false), 3);
    }

    #[test]
    fn a_whole_file_is_not_windowed() {
        let card = derive(
            &call(json!({"file_path": "/home/user/project/src/lib.rs"})),
            &meta(&[(1, "only")], 1, 1),
            &envelope("only"),
            roots(),
        )
        .unwrap();
        assert!(!card.windowed);
        assert!(card.status().is_none());
    }

    #[test]
    fn the_cap_keeps_the_head_and_the_tail() {
        let lines: Vec<(u64, String)> = (1..=20).map(|n| (n, format!("line {n}"))).collect();
        let reference: Vec<(u64, &str)> = lines.iter().map(|(n, t)| (*n, t.as_str())).collect();
        let card = derive(
            &call(json!({"file_path": "/home/user/project/src/lib.rs"})),
            &meta(&reference, 1, 20),
            &envelope(
                &lines
                    .iter()
                    .map(|(_, t)| t.clone())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            roots(),
        )
        .unwrap();
        let shown = card.shown(false);
        assert_eq!(shown.head.len(), 4);
        assert_eq!(shown.tail.len(), 4);
        assert_eq!(shown.hidden, 12);
        // The head keeps the first lines, the tail the last: no line is invented.
        assert_eq!(shown.head[0].number, 1);
        assert_eq!(shown.head[3].number, 4);
        assert_eq!(shown.tail[0].number, 17);
        assert_eq!(shown.tail[3].number, 20);
        assert_eq!(card.drawn_rows(false), 9);
        assert_eq!(card.drawn_rows(true), 20);
    }

    #[test]
    fn metadata_that_fails_a_rule_drops_the_card() {
        let args = call(json!({"file_path": "/home/user/project/src/lib.rs"}));
        let envelope = envelope("a");
        let cases = [
            // A zero line number, a repeated one, one past the total, and a missing total.
            meta(&[(0, "a")], 1, 1),
            // Repeated numbers are not a line list, and a line before the read's own offset
            // is not either: the reference requires strictly increasing numbers after
            // `offset - 1` (`read-card-model.ts:70-80`).
            meta(&[(1, "a"), (1, "b")], 1, 4),
            meta(&[(1, "a")], 3, 4),
            meta(&[(9, "a")], 1, 4),
            json!({"path": "p", "offset": 1, "lines": [{"number": 1, "text": "a"}]}),
            // A negative offset, a non-integer total, a non-string line.
            json!({"path": "p", "offset": 0, "totalLines": 1, "lines": []}),
            json!({"path": "p", "offset": 1, "totalLines": 1.5, "lines": []}),
            json!({"path": "p", "offset": 1, "totalLines": 1, "lines": [{"number": 1, "text": 2}]}),
        ];
        for case in cases {
            assert!(derive(&args, &case, &envelope, roots()).is_none(), "{case}");
        }
    }

    #[test]
    fn a_missing_or_wrong_envelope_drops_the_card() {
        let args = call(json!({"file_path": "/home/user/project/src/lib.rs"}));
        let meta = meta(&[(1, "a")], 1, 1);
        for output in [
            "",
            "a",
            "<path>p</path>\n<content>\na\n</content>",
            "<path>p</path>\n<type>file</type>\n<content>\na",
        ] {
            assert!(derive(&args, &meta, output, roots()).is_none(), "{output}");
        }
    }

    #[test]
    fn a_non_read_call_or_bad_arguments_drops_the_card() {
        let meta = meta(&[(1, "a")], 1, 1);
        let envelope = envelope("a");
        for (args, name) in [
            (
                json!({"file_path": "/home/user/project/src/lib.rs"}),
                "bash",
            ),
            (
                json!({"file_path": "/home/user/project/src/lib.rs"}),
                "functions.edit",
            ),
            (json!({"file_path": "  "}), "read"),
            (json!({"file_path": "/p", "offset": 0}), "read"),
            (json!({"file_path": "/p", "limit": -1}), "read"),
            (json!({"file_path": "/p", "offset": "2"}), "read"),
        ] {
            let call = json!({"name": name, "arguments": args.to_string()});
            assert!(derive(&call, &meta, &envelope, roots()).is_none(), "{call}");
        }
        // Arguments that are not JSON at all are not a read.
        assert!(
            derive(
                &json!({"name": "read", "arguments": "not json"}),
                &meta,
                &envelope,
                roots()
            )
            .is_none()
        );
    }

    #[test]
    fn the_debug_form_never_prints_line_text() {
        let card = derive(
            &call(json!({"file_path": "/home/user/project/src/lib.rs"})),
            &meta(&[(1, "PRIVATE-CONTENT")], 1, 1),
            &envelope("PRIVATE-CONTENT"),
            roots(),
        )
        .unwrap();
        let debug = format!("{card:?}");
        assert!(!debug.contains("PRIVATE-CONTENT"), "{debug}");
        assert!(debug.contains("1 lines"), "{debug}");
    }
}
