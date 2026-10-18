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
        _ => return None,
    };
    let old_lines = content_lines(old.unwrap_or(""));
    let new_lines = content_lines(new);
    let mut lines = vec![Line {
        kind: Kind::Path,
        text: path.lines().next().unwrap_or(path).to_owned(),
    }];
    let script = script(&old_lines, &new_lines);
    let mut previous: Option<usize> = None;
    for index in kept(&script) {
        if previous.is_some_and(|previous| index > previous + 1) {
            lines.push(Line {
                kind: Kind::Gap,
                text: "⋯".to_owned(),
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
        });
        previous = Some(index);
    }
    Some(Diff { lines })
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
