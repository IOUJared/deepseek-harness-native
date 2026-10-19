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

/// One side of a side-by-side row (`SplitRow`'s halves, `FileDiff.tsx:23-27`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitCell {
    pub kind: Kind,
    pub text: String,
    /// The number this side shows: the old number on the left, the new one on the right.
    pub number: Option<u64>,
}

/// One row of the side-by-side comparison: the old side, the new side, both (a context line), or
/// neither. A row that belongs to no side — a file header or a fold separator — is `full` and draws
/// across both columns.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SplitRow {
    pub left: Option<SplitCell>,
    pub right: Option<SplitCell>,
    pub full: Option<Line>,
}

/// Pair a comparison's lines for the side-by-side view: each run of deletions is aligned with the run
/// of additions that follows it, row by row, and a context line sits on both sides
/// (`FileDiff.tsx:79-104` `splitRows`).
///
/// A run that ends with more deletions than additions (or the reverse) leaves the shorter side's
/// trailing rows empty, so the two columns stay aligned on the same source lines.
pub fn split_rows(lines: &[Line]) -> Vec<SplitRow> {
    let mut rows = Vec::new();
    let mut deletions: Vec<SplitCell> = Vec::new();
    let mut additions: Vec<SplitCell> = Vec::new();
    let flush = |rows: &mut Vec<SplitRow>,
                 deletions: &mut Vec<SplitCell>,
                 additions: &mut Vec<SplitCell>| {
        for at in 0..deletions.len().max(additions.len()) {
            rows.push(SplitRow {
                left: deletions.get(at).cloned(),
                right: additions.get(at).cloned(),
                full: None,
            });
        }
        deletions.clear();
        additions.clear();
    };
    for line in lines {
        match line.kind {
            Kind::Deleted => deletions.push(SplitCell {
                kind: Kind::Deleted,
                text: line.text.clone(),
                number: line.old,
            }),
            Kind::Added => additions.push(SplitCell {
                kind: Kind::Added,
                text: line.text.clone(),
                number: line.new,
            }),
            Kind::Context => {
                flush(&mut rows, &mut deletions, &mut additions);
                rows.push(SplitRow {
                    left: Some(SplitCell {
                        kind: Kind::Context,
                        text: line.text.clone(),
                        number: line.old,
                    }),
                    right: Some(SplitCell {
                        kind: Kind::Context,
                        text: line.text.clone(),
                        number: line.new,
                    }),
                    full: None,
                });
            }
            Kind::Path | Kind::Gap => {
                flush(&mut rows, &mut deletions, &mut additions);
                rows.push(SplitRow {
                    left: None,
                    right: None,
                    full: Some(line.clone()),
                });
            }
        }
    }
    flush(&mut rows, &mut deletions, &mut additions);
    rows
}

/// Whether a comparison is one-sided: every line was added or every line was deleted, so a file was
/// created or removed and the reference draws it in one column whatever layout was asked for
/// (`FileDiff.tsx:141-158`).
pub fn one_sided(diff: &Diff) -> bool {
    let mut added = false;
    let mut deleted = false;
    for line in &diff.lines {
        match line.kind {
            Kind::Added => added = true,
            Kind::Deleted => deleted = true,
            Kind::Path | Kind::Context | Kind::Gap => {}
        }
        if added && deleted {
            return false;
        }
    }
    added || deleted
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

    fn line(kind: Kind, text: &str, old: Option<u64>, new: Option<u64>) -> Line {
        Line {
            kind,
            text: text.to_owned(),
            old,
            new,
        }
    }

    #[test]
    fn a_split_row_pairs_each_deletion_run_with_the_additions_that_follow_it() {
        // Two deletions, one addition: the second deletion's row has no right side, and the two
        // columns stay aligned rather than the addition floating up to the first row.
        let rows = split_rows(&[
            line(Kind::Context, "same", Some(1), Some(1)),
            line(Kind::Deleted, "gone-a", Some(2), None),
            line(Kind::Deleted, "gone-b", Some(3), None),
            line(Kind::Added, "new", None, Some(2)),
            line(Kind::Context, "tail", Some(4), Some(3)),
        ]);
        assert_eq!(rows.len(), 4);
        // A context row carries both numbers, because each side numbers it its own way.
        assert_eq!(rows[0].left.as_ref().unwrap().number, Some(1));
        assert_eq!(rows[0].right.as_ref().unwrap().number, Some(1));
        assert_eq!(rows[1].left.as_ref().unwrap().text, "gone-a");
        assert_eq!(rows[1].right.as_ref().unwrap().text, "new");
        assert_eq!(rows[1].left.as_ref().unwrap().number, Some(2));
        assert_eq!(
            rows[1].right.as_ref().unwrap().number,
            Some(2),
            "the addition's new number, not its position"
        );
        // The unpaired deletion keeps the left column alone, so the sides stay on the same rows.
        assert_eq!(rows[2].left.as_ref().unwrap().text, "gone-b");
        assert!(rows[2].right.is_none());
        assert_eq!(rows[3].left.as_ref().unwrap().number, Some(4));
        assert_eq!(rows[3].right.as_ref().unwrap().number, Some(3));
    }

    #[test]
    fn a_split_pairs_a_two_addition_run_with_one_deletion_one_row_at_a_time() {
        // The shape the fixture's comparison has: one deletion, two additions, a context line, then
        // one more addition. The second addition and the trailing one each keep their own row with
        // an empty old side, and the context line keeps its own row on both sides; nothing is
        // pulled up to fill a gap.
        let rows = split_rows(&[
            line(Kind::Path, "@@ -11252,3 +11252,4 @@", None, None),
            line(Kind::Context, "fn changes_card", Some(11252), Some(11252)),
            line(Kind::Deleted, "let title = format!", Some(11253), None),
            line(
                Kind::Added,
                "let title = changes_text::title",
                None,
                Some(11253),
            ),
            line(Kind::Added, "let counts = file_counts", None, Some(11254)),
            line(
                Kind::Context,
                "let mut card = column![]",
                Some(11254),
                Some(11255),
            ),
            line(
                Kind::Added,
                "let clamped = clamped_offset",
                None,
                Some(11256),
            ),
        ]);
        assert_eq!(
            rows.len(),
            6,
            "header, context, pair, leftover, context, leftover"
        );
        assert_eq!(rows[1].left.as_ref().unwrap().number, Some(11252));
        assert_eq!(rows[2].right.as_ref().unwrap().number, Some(11253));
        assert!(
            rows[3].left.is_none(),
            "the second addition has no old line"
        );
        assert_eq!(rows[3].right.as_ref().unwrap().number, Some(11254));
        assert_eq!(rows[4].left.as_ref().unwrap().number, Some(11254));
        assert_eq!(
            rows[4].right.as_ref().unwrap().number,
            Some(11255),
            "the context line numbers each side its own way"
        );
        assert!(rows[5].left.is_none());
        assert_eq!(rows[5].right.as_ref().unwrap().number, Some(11256));
    }

    #[test]
    fn a_split_keeps_a_header_or_fold_across_both_columns() {
        let rows = split_rows(&[
            line(Kind::Path, "PUBLIC file.rs", None, None),
            line(Kind::Added, "one", None, Some(1)),
            line(Kind::Gap, "3 lines", None, None),
            line(Kind::Deleted, "two", Some(9), None),
        ]);
        // The path header spans the row instead of landing in the left column as a deletion would.
        assert_eq!(rows[0].full.as_ref().unwrap().text, "PUBLIC file.rs");
        assert!(rows[0].left.is_none() && rows[0].right.is_none());
        assert_eq!(rows[1].right.as_ref().unwrap().text, "one");
        assert_eq!(rows[2].full.as_ref().unwrap().text, "3 lines");
        assert_eq!(rows[3].left.as_ref().unwrap().text, "two");
        assert!(rows[3].right.is_none(), "the run was flushed at the fold");
    }

    #[test]
    fn only_a_comparison_with_one_side_stays_in_one_column() {
        let added = Diff {
            lines: vec![
                line(Kind::Path, "PUBLIC file.rs", None, None),
                line(Kind::Added, "one", None, Some(1)),
            ],
        };
        assert!(one_sided(&added), "a created file has no old side to pair");
        let changed = Diff {
            lines: vec![
                line(Kind::Deleted, "old", Some(1), None),
                line(Kind::Added, "new", None, Some(1)),
            ],
        };
        assert!(!one_sided(&changed));
        // A comparison with no lines at all is not one-sided: there is nothing to draw either way.
        assert!(!one_sided(&Diff { lines: Vec::new() }));
    }
}
