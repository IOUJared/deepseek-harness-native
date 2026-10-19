//! The changed-files card's own copy, taken from the reference: one title for a turn that edited one
//! file and another for a turn that edited many, with additions and deletions grouped and signed
//! `+N` / `-N` (`ui-deliverables/src/client/locales.ts:100-110`, `ChangedFiles.tsx:25-26,58-60`).
//!
//! The card's counts came from a single muted string with a typographic minus and no grouping, so a
//! turn that edited `report.md` alone read "Changed files · 1 files" and a thousand-line change read
//! `+1000` in the same ink as everything else.

/// The reference's thousands grouping: `1,234`.
pub fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// The card's title: `Edited <name>` for one file, `Edited <n> files` for any other count
/// (`locales.ts:100-101`). The name is the file's basename, as the reference passes `basename(path)`.
pub fn title(total: u64, single: Option<&str>) -> String {
    match single {
        Some(path) if total <= 1 => format!("Edited {}", basename(path)),
        _ => format!("Edited {} files", grouped(total)),
    }
}

/// The last path segment, which is what the reference shows for a single edited file.
pub fn basename(path: &str) -> String {
    path.rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or(path)
        .to_owned()
}

/// One comparison's display name and body, as the detail panel renders it.
///
/// Shared so a preview and the panel it opens into cannot describe the same file differently.
pub fn comparison(diff: &dsh_native_transport::dto::ChangesDiff) -> (String, String) {
    use dsh_native_transport::dto::ChangesDiff;
    let (display, body) = match diff {
        ChangesDiff::Text {
            display,
            before,
            after,
            coarse,
            hunks,
            ..
        } => {
            let mut text = String::new();
            // The comparison's own qualifications, in the reference's order above the hunks
            // (`FileDiff.tsx:128-131, 229`): a created or deleted file, a pair whose sides
            // hold the same lines, and a timed-out comparison drawn as a whole-file
            // replacement. Without them a coarse replacement read as an exact line diff.
            if !before {
                text.push_str("Created in this turn\n");
            } else if !after {
                text.push_str("Deleted in this turn\n");
            } else if hunks.is_empty() {
                text.push_str("Both sides hold the same lines\n");
            }
            if *coarse {
                text.push_str("Line comparison timed out; shown as a whole-file replacement\n");
            }
            for hunk in hunks {
                text.push_str(&format!(
                    "@@ -{},{} +{},{} @@\n",
                    hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
                ));
                for line in &hunk.lines {
                    text.push_str(line);
                    text.push('\n');
                }
            }
            (display, text)
        }
        ChangesDiff::Binary { display, .. } => {
            (display, "Binary file; no textual comparison.\n".into())
        }
        ChangesDiff::Oversized { display, .. } => {
            (display, "File too large for a textual comparison.\n".into())
        }
    };
    (display.clone(), body)
}

/// A readable comparison as the diff widget's own lines: each hunk's header as a path row, its
/// additions and deletions marked, and everything else context (`FileDiff.tsx:165-253` draws the
/// header and the `+`/`-` rows this way). Returns `None` when the comparison has nothing to draw as
/// lines — a binary, an oversized file, or a pair with the same text on both sides.
pub fn diff_lines(
    diff: &dsh_native_transport::dto::ChangesDiff,
) -> Option<crate::reducer::inline_diff::Diff> {
    use dsh_native_transport::dto::ChangesDiff;
    let ChangesDiff::Text { hunks, .. } = diff else {
        return None;
    };
    if hunks.is_empty() {
        return None;
    }
    // The transport's own bound is already the card's; this guard keeps a malformed reply from
    // building an unbounded row set.
    let mut lines = Vec::new();
    for hunk in hunks.iter().take(64) {
        lines.push(crate::reducer::inline_diff::Line {
            kind: crate::reducer::inline_diff::Kind::Path,
            text: format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
            ),
        });
        for line in hunk.lines.iter().take(4_000) {
            let kind = match line.chars().next() {
                Some('+') => crate::reducer::inline_diff::Kind::Added,
                Some('-') => crate::reducer::inline_diff::Kind::Deleted,
                _ => crate::reducer::inline_diff::Kind::Context,
            };
            lines.push(crate::reducer::inline_diff::Line {
                kind,
                text: line.clone(),
            });
        }
    }
    (!lines.is_empty()).then_some(crate::reducer::inline_diff::Diff { lines })
}

/// The reference's status line while a comparison is being read (`FileDiff.tsx:147`,
/// `locales.ts:122`).
pub const READING: &str = "Reading changes…";

/// What the card draws for a given number of edited files. One file is not a list: the card *is* the
/// row, so pressing it opens the comparison directly (`ChangedFiles.tsx:54-77` — `singleFile ===
/// undefined ? header : <>`). Everything else is a header with a list under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Single,
    List,
}

pub fn shape(total: usize) -> Shape {
    if total == 1 {
        Shape::Single
    } else {
        Shape::List
    }
}

/// The reference's row count below which the list never folds (`ChangedFiles.tsx:18`).
pub const COLLAPSED_ROWS: usize = 4;

/// How many rows the card shows for its current state, and whether a bottom control offers the rest
/// (`ChangedFiles.tsx:48,98-104`). A list of at most [`COLLAPSED_ROWS`] files has no control at all.
pub fn visible_rows(total: usize, expanded: bool) -> (usize, Option<&'static str>) {
    if total <= COLLAPSED_ROWS {
        return (total, None);
    }
    if expanded {
        (total, Some("Collapse"))
    } else {
        (COLLAPSED_ROWS, Some("All files"))
    }
}

/// The fold control's own label. The reference prints the count in it — `All 7 files`
/// (`locales.ts:106`) — and a bare `Collapse` once everything is showing (`:108`).
pub fn fold_label(total: usize, expanded: bool) -> Option<String> {
    match visible_rows(total, expanded).1 {
        Some("All files") => Some(format!("All {total} files")),
        Some("Collapse") => Some("Collapse".to_owned()),
        _ => None,
    }
}

/// One file row's counts, or the reference's own word for a comparison it cannot show
/// (`locales.ts:110,125-126`).
pub fn file_counts(binary: bool, oversized: bool, added: u64, deleted: u64) -> String {
    if binary {
        "binary".to_owned()
    } else if oversized {
        "too large".to_owned()
    } else {
        format!("+{} -{}", grouped(added), grouped(deleted))
    }
}

#[cfg(test)]
mod tests {
    //! The card's copy, at the counts where the shapes differ.
    use super::*;

    #[test]
    fn one_file_names_itself_and_more_count_themselves() {
        assert_eq!(title(1, Some("src/report.md")), "Edited report.md");
        assert_eq!(title(2, None), "Edited 2 files");
        assert_eq!(title(1_234, None), "Edited 1,234 files");
        // A single file whose path cannot be read still counts rather than guessing a name.
        assert_eq!(title(1, None), "Edited 1 files");
    }

    #[test]
    fn a_comparison_becomes_rows_the_diff_widget_can_draw() {
        use dsh_native_transport::dto::{ChangesDiff, ChangesHunk};
        let hunk = |lines: &[&str]| ChangesHunk {
            old_start: 3,
            old_lines: 2,
            new_start: 3,
            new_lines: 3,
            lines: lines.iter().map(|line| line.to_string()).collect(),
        };
        let text = ChangesDiff::Text {
            path: "src/lib.rs".into(),
            display: "src/lib.rs".into(),
            before: true,
            after: true,
            coarse: false,
            hunks: vec![hunk(&[" context", "-gone", "+new", "+added"])],
        };
        let rows = diff_lines(&text).expect("a readable comparison has rows");
        let kinds: Vec<_> = rows.lines.iter().map(|line| line.kind).collect();
        use crate::reducer::inline_diff::Kind;
        assert_eq!(
            kinds,
            [
                Kind::Path,
                Kind::Context,
                Kind::Deleted,
                Kind::Added,
                Kind::Added
            ]
        );
        assert_eq!(rows.lines[0].text, "@@ -3,2 +3,3 @@");
        // A comparison with no hunks, or one with nothing textual, draws nothing.
        let empty = ChangesDiff::Text {
            path: "src/lib.rs".into(),
            display: "src/lib.rs".into(),
            before: true,
            after: true,
            coarse: false,
            hunks: vec![],
        };
        assert!(diff_lines(&empty).is_none());
        assert!(
            diff_lines(&ChangesDiff::Binary {
                path: "src/lib.rs".into(),
                display: "src/lib.rs".into(),
            })
            .is_none()
        );
    }

    #[test]
    fn a_pending_comparison_says_it_is_being_read() {
        assert_eq!(READING, "Reading changes…");
        assert!(
            READING.ends_with('…'),
            "an ellipsis, as the reference writes it"
        );
    }

    #[test]
    fn one_file_is_the_card_rather_than_a_list_of_one() {
        assert_eq!(shape(1), Shape::Single);
        assert_eq!(shape(2), Shape::List);
        // A single file never folds, whatever its state.
        assert_eq!(visible_rows(1, false), (1, None));
        assert_eq!(fold_label(1, false), None);
    }

    #[test]
    fn a_long_list_folds_at_four_rows_and_offers_the_rest() {
        // At or below the threshold there is no control: the list simply shows.
        assert_eq!(visible_rows(4, false), (4, None));
        assert_eq!(fold_label(4, false), None);
        // Past it, a closed card shows four rows and counts the rest; an open one shows all.
        assert_eq!(visible_rows(7, false), (4, Some("All files")));
        assert_eq!(fold_label(7, false).as_deref(), Some("All 7 files"));
        assert_eq!(visible_rows(7, true), (7, Some("Collapse")));
        assert_eq!(fold_label(7, true).as_deref(), Some("Collapse"));
    }

    #[test]
    fn counts_are_grouped_and_signed_the_way_the_reference_writes_them() {
        assert_eq!(file_counts(false, false, 0, 0), "+0 -0");
        assert_eq!(file_counts(false, false, 12, 3), "+12 -3");
        assert_eq!(
            file_counts(false, false, 1_500, 1_000_000),
            "+1,500 -1,000,000"
        );
        // The refusal words are the reference's, not "oversized".
        assert_eq!(file_counts(true, false, 0, 0), "binary");
        assert_eq!(file_counts(false, true, 0, 0), "too large");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000), "1,000");
        assert_eq!(grouped(1_234_567), "1,234,567");
    }
}
