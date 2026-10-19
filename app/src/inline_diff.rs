//! The reference's inline diff card, derived from one recorded `edit` or `write` call.
//!
//! The reference renders an `edit`/`write` summary's expanded body as a diff instead of an
//! input/output card (`ToolRow.tsx:169-170` takes the first of card, diff, …; `ToolRow.tsx:268`
//! draws `DiffBlock`), and derives the card from the call's own arguments while it runs
//! (`diff-card-model.ts:52-104`). This module is that derivation: pure, bounded, no Iced and no
//! app state, so the rows a test asserts are the rows the view draws.
use serde_json::{Map, Value};

/// The chat diff card's height cap in rows, path header and gaps included
/// (`CHAT_DIFF_MAX_LINES`, `diff-card-model.ts:7`).
pub const MAX_ROWS: usize = 9;
/// Unchanged lines kept on each side of a change (`structuredPatch`'s `context: 3`).
const CONTEXT: usize = 3;
/// Edits larger than this fall back to a whole-fragment replacement rather than an edit graph
/// (`MAX_DIFF_EDIT_LENGTH`, `DiffBlock.tsx:76-87`), and match the stat's own bound.
const MAX_EDIT_LINES: usize = 256;

/// One rendered row's role. `Path` opens the card, `Gap` stands where unchanged lines were
/// dropped between hunks, and the rest are diff lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Path,
    Deleted,
    Added,
    Context,
    Gap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub kind: Kind,
    pub text: String,
    /// The old-side line number a gutter shows, when this row has one. Tool diffs leave both sides
    /// unnumbered; a comparison read from the Host carries the numbers its hunks state
    /// (`FileDiff.tsx:246-253`).
    pub old: Option<u64>,
    /// The new-side line number, on the same terms.
    pub new: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diff {
    pub lines: Vec<Line>,
}

/// One row range the card draws: `head` then (when capped) a fold control then `tail`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub head: std::ops::Range<usize>,
    pub tail: std::ops::Range<usize>,
    /// Rows the fold control stands for; zero means nothing is folded.
    pub hidden: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Same,
    Deleted,
    Added,
}

/// Derive the diff card for one file-mutation call. `verb` is the label `activity` resolved
/// (`"Edit"` or `"Write"`); anything else has no card.
pub fn derive(verb: &str, args: &Map<String, Value>) -> Option<Diff> {
    let path = args
        .get("file_path")
        .or_else(|| args.get("path"))?
        .as_str()?;
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let (old, new) = match verb {
        // `old_string` may legitimately be empty (an insertion at a point); the reference draws
        // that as `oldText: null` (`diff-card-model.ts:94`).
        "Edit" => {
            let old = args.get("old_string")?.as_str()?;
            let new = args.get("new_string")?.as_str()?;
            (Some(old).filter(|old| !old.is_empty()), new)
        }
        "Write" => (None, args.get("content")?.as_str()?),
        // The Anthropic-style editor names the same mutation differently: `create` writes a whole
        // file, `str_replace` swaps one fragment (`diff-card-model.ts:47-64`).
        "str_replace_editor" => match args.get("command")?.as_str()? {
            "create" => (
                None,
                match args.get("file_text") {
                    None => "",
                    Some(value) => value.as_str()?,
                },
            ),
            "str_replace" => (
                match args.get("old_str") {
                    None => None,
                    Some(value) => Some(value.as_str()?).filter(|old| !old.is_empty()),
                },
                match args.get("new_str") {
                    None => "",
                    Some(value) => value.as_str()?,
                },
            ),
            _ => return None,
        },
        _ => return None,
    };
    file(path, old, new)
}

/// The card for one explicit file mutation: the path, the text it replaced (`None` for a whole
/// file) and the text it wrote. `derive` is the argument-reading front end; the applied-diff path
/// (`meta.diffs`) calls this directly.
pub fn file(path: &str, old: Option<&str>, new: &str) -> Option<Diff> {
    if path.trim().is_empty() {
        return None;
    }
    let old_lines = content_lines(old.unwrap_or(""));
    let new_lines = content_lines(new);
    let mut lines = vec![Line {
        kind: Kind::Path,
        text: path.lines().next().unwrap_or(path).to_owned(),
        old: None,
        new: None,
    }];
    let script = script(&old_lines, &new_lines);
    let mut previous: Option<usize> = None;
    for index in kept(&script) {
        if previous.is_some_and(|previous| index > previous + 1) {
            lines.push(Line {
                kind: Kind::Gap,
                text: "⋯".to_owned(),
                old: None,
                new: None,
            });
        }
        let (op, text) = &script[index];
        lines.push(Line {
            kind: match op {
                Op::Same => Kind::Context,
                Op::Deleted => Kind::Deleted,
                Op::Added => Kind::Added,
            },
            text: text.clone(),
            old: None,
            new: None,
        });
        previous = Some(index);
    }
    Some(Diff { lines })
}

/// What a settled result's `meta.diffs` says was actually written.
#[derive(Clone, PartialEq, Eq)]
pub enum Applied {
    /// One hunk per changed file, in the Host's order; a path with `None` had no previous text.
    Hunks(Vec<(String, Option<String>, String)>),
    /// The Host recorded that the write changed nothing.
    Empty,
    /// No usable `meta.diffs`: absent, malformed, or too large to draw.
    None,
}

/// The hunks are file content, so the debug form prints the path count and the byte lengths.
impl std::fmt::Debug for Applied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Applied::Hunks(hunks) => f
                .debug_struct("Hunks")
                .field("files", &hunks.len())
                .field(
                    "bytes",
                    &hunks
                        .iter()
                        .map(|(path, old, new)| {
                            path.len() + old.as_ref().map_or(0, String::len) + new.len()
                        })
                        .sum::<usize>(),
                )
                .finish(),
            Applied::Empty => f.write_str("Empty"),
            Applied::None => f.write_str("None"),
        }
    }
}

/// The reference's `appliedDiffs` (`diff-card-model.ts:71-79`): a strict narrowing of
/// `meta.diffs`, where any malformed hunk invalidates the whole list rather than being skipped.
pub fn applied(meta: &Value) -> Applied {
    let Some(diffs) = meta.get("diffs").and_then(Value::as_array) else {
        return Applied::None;
    };
    if diffs.is_empty() {
        return Applied::Empty;
    }
    if diffs.len() > 8 {
        return Applied::None;
    }
    let mut hunks = Vec::with_capacity(diffs.len());
    for hunk in diffs {
        if !hunk.is_object() {
            return Applied::None;
        }
        let Some(path) = hunk.get("path").and_then(Value::as_str) else {
            return Applied::None;
        };
        let old = match hunk.get("oldText") {
            None | Some(Value::Null) => None,
            Some(value) => match value.as_str() {
                Some(text) => Some(text.to_owned()),
                None => return Applied::None,
            },
        };
        let Some(new) = hunk.get("newText").and_then(Value::as_str) else {
            return Applied::None;
        };
        hunks.push((path.to_owned(), old, new.to_owned()));
    }
    Applied::Hunks(hunks)
}

/// The rows drawn at one fold state, over `total` rows: the head and tail split around a fold
/// control (`DiffBlock.tsx:194-206`; `headLines = ceil(cap / 2)`).
pub fn shown(total: usize, expanded: bool) -> Shown {
    let hidden = total.saturating_sub(MAX_ROWS);
    if hidden == 0 || expanded {
        return Shown {
            head: 0..total,
            tail: total..total,
            hidden,
        };
    }
    let head = MAX_ROWS.div_ceil(2);
    let tail = MAX_ROWS - head;
    Shown {
        head: 0..head,
        tail: total - tail..total,
        hidden,
    }
}

/// The rows the card draws: the shown head, the fold control when `hidden > 0`, the tail.
pub fn drawn_rows(diff: &Diff, expanded: bool) -> usize {
    let shown = shown(diff.lines.len(), expanded);
    shown.head.len() + usize::from(shown.hidden > 0) + shown.tail.len()
}

/// Split one side's text into content lines. Empty text is zero lines, and one trailing
/// newline is a terminator rather than an empty final line; an interior blank line survives
/// (`DiffBlock.tsx:135-142`).
fn content_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    text.strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .collect()
}

/// The edit script for two line lists. Above the edit bound the whole fragment is drawn as a
/// replacement, the reference's own fallback when its edit graph would exceed
/// `MAX_DIFF_EDIT_LENGTH` (`DiffBlock.tsx:80-87`).
fn script(old: &[&str], new: &[&str]) -> Vec<(Op, String)> {
    if old.len() > MAX_EDIT_LINES || new.len() > MAX_EDIT_LINES {
        return old
            .iter()
            .map(|line| (Op::Deleted, (*line).to_owned()))
            .chain(new.iter().map(|line| (Op::Added, (*line).to_owned())))
            .collect();
    }
    // Longest-common-subsequence table over lines, then a forward walk. Both sides are
    // bounded above, so the table is at most 257×257 cells.
    let (rows, columns) = (old.len() + 1, new.len() + 1);
    let mut table = vec![0u32; rows * columns];
    for row in (0..old.len()).rev() {
        for column in (0..new.len()).rev() {
            table[row * columns + column] = if old[row] == new[column] {
                table[(row + 1) * columns + column + 1] + 1
            } else {
                table[(row + 1) * columns + column].max(table[row * columns + column + 1])
            };
        }
    }
    let (mut row, mut column) = (0usize, 0usize);
    let mut out = Vec::new();
    while row < old.len() && column < new.len() {
        if old[row] == new[column] {
            out.push((Op::Same, old[row].to_owned()));
            row += 1;
            column += 1;
        } else if table[(row + 1) * columns + column] >= table[row * columns + column + 1] {
            out.push((Op::Deleted, old[row].to_owned()));
            row += 1;
        } else {
            out.push((Op::Added, new[column].to_owned()));
            column += 1;
        }
    }
    for line in &old[row..] {
        out.push((Op::Deleted, (*line).to_owned()));
    }
    for line in &new[column..] {
        out.push((Op::Added, (*line).to_owned()));
    }
    out
}

/// The script indices to keep: every changed line and `CONTEXT` unchanged lines either side,
/// so two changes closer than the context merge into one hunk and a wider distance leaves a gap.
fn kept(script: &[(Op, String)]) -> Vec<usize> {
    let mut keep = vec![false; script.len()];
    for (index, (op, _)) in script.iter().enumerate() {
        if *op == Op::Same {
            continue;
        }
        let start = index.saturating_sub(CONTEXT);
        let end = (index + CONTEXT + 1).min(script.len());
        keep[start..end].fill(true);
    }
    keep.iter()
        .enumerate()
        .filter_map(|(index, keep)| keep.then_some(index))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn args(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    fn kinds(diff: &Diff) -> Vec<(Kind, &str)> {
        diff.lines
            .iter()
            .map(|line| (line.kind, line.text.as_str()))
            .collect()
    }

    #[test]
    fn an_edit_draws_its_path_then_the_replaced_and_inserted_lines() {
        let diff = derive(
            "Edit",
            &args(json!({"file_path": "src/a.rs", "old_string": "let a = 1;", "new_string": "let a = 2;"})),
        )
        .unwrap();
        assert_eq!(
            kinds(&diff),
            vec![
                (Kind::Path, "src/a.rs"),
                (Kind::Deleted, "let a = 1;"),
                (Kind::Added, "let a = 2;"),
            ]
        );
    }

    #[test]
    fn a_write_draws_the_whole_file_as_additions() {
        let diff = derive(
            "Write",
            &args(json!({"file_path": "src/b.rs", "content": "one\ntwo\n"})),
        )
        .unwrap();
        assert_eq!(
            kinds(&diff),
            vec![
                (Kind::Path, "src/b.rs"),
                (Kind::Added, "one"),
                (Kind::Added, "two"),
            ],
            "a trailing newline terminates the last line instead of adding an empty one"
        );
    }

    #[test]
    fn context_is_three_lines_and_distant_changes_leave_a_gap() {
        let old = (1..=30)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let new = old
            .replace("line 3", "line 3 changed")
            .replace("line 28", "line 28 changed");
        let diff = derive(
            "Edit",
            &args(json!({"file_path": "src/c.rs", "old_string": old, "new_string": new})),
        )
        .unwrap();
        let rows = kinds(&diff);
        assert_eq!(rows[0], (Kind::Path, "src/c.rs"));
        // First hunk: the line before the context start, context, the change, context.
        assert_eq!(rows[1], (Kind::Context, "line 1"));
        assert_eq!(rows[3], (Kind::Deleted, "line 3"));
        assert_eq!(rows[4], (Kind::Added, "line 3 changed"));
        // A gap stands for the unchanged middle.
        assert!(rows.iter().any(|(kind, _)| *kind == Kind::Gap));
        assert!(
            rows.iter()
                .any(|(kind, text)| *kind == Kind::Added && *text == "line 28 changed")
        );
    }

    #[test]
    fn an_insertion_and_an_unchanged_edit_stay_bounded() {
        let inserted = derive(
            "Edit",
            &args(json!({"path": "src/d.rs", "old_string": "", "new_string": "new"})),
        )
        .unwrap();
        assert_eq!(
            kinds(&inserted),
            vec![(Kind::Path, "src/d.rs"), (Kind::Added, "new")]
        );
        // Nothing changed: the card is its path header alone, as the reference's empty patch is.
        let unchanged = derive(
            "Edit",
            &args(json!({"path": "src/d.rs", "old_string": "same", "new_string": "same"})),
        )
        .unwrap();
        assert_eq!(kinds(&unchanged), vec![(Kind::Path, "src/d.rs")]);
    }

    #[test]
    fn an_oversized_edit_draws_a_whole_fragment_replacement() {
        let old = (0..300)
            .map(|n| format!("old {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let new = (0..300)
            .map(|n| format!("new {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let diff = derive(
            "Edit",
            &args(json!({"file_path": "src/e.rs", "old_string": old, "new_string": new})),
        )
        .unwrap();
        assert_eq!(diff.lines[1].kind, Kind::Deleted);
        assert_eq!(diff.lines[1].text, "old 0");
        assert_eq!(diff.lines[301].kind, Kind::Added);
        assert_eq!(diff.lines[301].text, "new 0");
    }

    #[test]
    fn a_call_without_a_path_or_content_has_no_card() {
        assert!(derive("Edit", &args(json!({"old_string": "a", "new_string": "b"}))).is_none());
        assert!(derive("Edit", &args(json!({"file_path": "a", "old_string": "a"}))).is_none());
        assert!(derive("Write", &args(json!({"file_path": "a"}))).is_none());
        assert!(derive("Read", &args(json!({"file_path": "a", "content": "b"}))).is_none());
    }

    #[test]
    fn the_cap_draws_a_head_a_fold_row_and_a_tail() {
        let capped = shown(13, false);
        assert_eq!(capped.head, 0..5);
        assert_eq!(capped.tail, 9..13);
        assert_eq!(capped.hidden, 4);
        let all = shown(13, true);
        assert_eq!(all.head, 0..13);
        assert!(all.tail.is_empty());
        assert_eq!(all.hidden, 4);
        // Under the cap nothing folds and the whole list draws.
        let short = shown(3, false);
        assert_eq!(short.hidden, 0);
        assert_eq!(short.head, 0..3);
    }
}

#[cfg(test)]
mod applied_tests {
    //! The settled path: what the Host recorded as applied, and the editor's own argument names.
    use super::*;
    use serde_json::json;

    fn args(value: serde_json::Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap()
    }
    fn texts(diff: &Diff) -> Vec<(Kind, &str)> {
        diff.lines
            .iter()
            .map(|line| (line.kind, line.text.as_str()))
            .collect()
    }

    #[test]
    fn the_editor_creates_and_replaces() {
        // `create`: a whole file, like `write`.
        let diff = derive(
            "str_replace_editor",
            &args(json!({"command": "create", "path": "src/a.rs", "file_text": "one\ntwo"})),
        )
        .expect("a create");
        assert_eq!(
            texts(&diff),
            vec![
                (Kind::Path, "src/a.rs"),
                (Kind::Added, "one"),
                (Kind::Added, "two")
            ]
        );
        // `str_replace`: the fragment it replaced.
        let diff = derive(
            "str_replace_editor",
            &args(json!({"command": "str_replace", "path": "src/a.rs",
                "old_str": "one\ntwo", "new_str": "one\nthree"})),
        )
        .unwrap();
        assert!(texts(&diff).contains(&(Kind::Deleted, "two")));
        assert!(texts(&diff).contains(&(Kind::Added, "three")));
        // Anything else has no card.
        assert!(
            derive(
                "str_replace_editor",
                &args(json!({"command": "view", "path": "src/a.rs"}))
            )
            .is_none()
        );
        assert!(
            derive(
                "str_replace_editor",
                &args(json!({"command": "str_replace", "path": "src/a.rs", "old_str": 4}))
            )
            .is_none()
        );
    }

    #[test]
    fn the_applied_list_narrows_strictly() {
        // Absent is not empty: an absent list means the Host recorded none.
        assert_eq!(applied(&json!({})), Applied::None);
        assert_eq!(applied(&json!({"diffs": []})), Applied::Empty);
        assert_eq!(applied(&json!({"diffs": "no"})), Applied::None);
        // One malformed hunk invalidates the list rather than being skipped: a non-string path, a
        // non-string old side, a missing new side, or a hunk that is not an object at all.
        for hunk in [
            json!({"path": 4, "newText": "x"}),
            json!({"path": "p", "oldText": 4, "newText": "x"}),
            json!({"path": "p", "oldText": null}),
            json!([1]),
        ] {
            assert_eq!(applied(&json!({"diffs": [hunk]})), Applied::None, "{hunk}");
        }
        assert_eq!(
            applied(&json!({"diffs": [{"path": "p", "oldText": null, "newText": "x"}]})),
            Applied::Hunks(vec![("p".into(), None, "x".into())])
        );
        // A path with previous text keeps it.
        assert_eq!(
            applied(&json!({"diffs": [{"path": "p", "oldText": "a", "newText": "b"}]})),
            Applied::Hunks(vec![("p".into(), Some("a".into()), "b".into())])
        );
    }

    #[test]
    fn a_file_diff_needs_a_path_and_takes_an_absent_old_side() {
        assert!(file("  ", None, "x").is_none());
        let diff = file("src/a.rs", None, "x").unwrap();
        assert_eq!(
            texts(&diff),
            vec![(Kind::Path, "src/a.rs"), (Kind::Added, "x")]
        );
        let diff = file("src/a.rs", Some("a"), "b").unwrap();
        assert_eq!(
            texts(&diff),
            vec![
                (Kind::Path, "src/a.rs"),
                (Kind::Deleted, "a"),
                (Kind::Added, "b")
            ]
        );
    }
}
