//! The `grep`/`glob` search card, derived the way the reference derives it
//! (`search-card-model.ts:78-100`, `SearchBlock.tsx:126-270`).
//!
//! A search result carries its own shape in `meta`: `grep` reports matches grouped by file, `glob`
//! reports a flat path list, and both report how many results the search found (`total`) and
//! whether the page was capped (`truncated`). The banner states both so a capped page cannot read
//! as a complete one. Every rule the reference applies is applied here; a result that fails one
//! keeps the recorded text body.
use serde_json::Value;

/// Result rows a chat card draws before its middle collapses (`search-card-model.ts:17`).
pub const MAX_ROWS: usize = 8;
/// Per-line bound. The search tools cap a line upstream; a forged record must not grow a card.
const MAX_LINE_BYTES: usize = 512;
/// Bounds on the retained structure.
const MAX_PATHS: usize = 512;
const MAX_GROUPS: usize = 128;
const MAX_MATCHES: usize = 512;

/// One retained line of a `grep` result.
#[derive(Clone, PartialEq, Eq)]
pub struct Match {
    pub line_number: u64,
    pub line: String,
}

/// One file's grouped matches, in first-seen file order.
#[derive(Clone, PartialEq, Eq)]
pub struct Group {
    pub path: String,
    pub matches: Vec<Match>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Kind {
    /// `grep`: matched lines grouped by file.
    Matches(Vec<Group>),
    /// `glob`: discovered paths, in the tool's result order.
    Paths(Vec<String>),
}

#[derive(Clone, PartialEq, Eq)]
pub struct Card {
    pub kind: Kind,
    /// Results the search found before any cap.
    pub total: u64,
    /// The result page was capped, so the banner says "Showing X of N".
    pub truncated: bool,
}

/// One drawn row. The reference flattens groups into a header row plus its match rows, caps the
/// flattened list, and keeps the first and last rows of it (`SearchBlock.tsx:173-270`).
#[derive(Clone, PartialEq, Eq)]
pub enum Row {
    /// A `grep` file group's header: the path and its retained match count.
    File {
        path: String,
        count: usize,
    },
    Match {
        line_number: u64,
        line: String,
    },
    Path(String),
}

/// A row's text is file content, so its debug form prints the shape and counts, never the text.
impl std::fmt::Debug for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Row::File { path, count } => f
                .debug_struct("File")
                .field("path_bytes", &path.len())
                .field("count", count)
                .finish(),
            Row::Match { line_number, line } => f
                .debug_struct("Match")
                .field("line_number", line_number)
                .field("line_bytes", &line.len())
                .finish(),
            Row::Path(path) => f
                .debug_tuple("Path")
                .field(&format_args!("{} bytes", path.len()))
                .finish(),
        }
    }
}

pub struct Shown {
    pub head: Vec<Row>,
    pub tail: Vec<Row>,
    pub hidden: u64,
}

impl Card {
    /// The flattened rows the card draws, before the cap.
    pub fn rows(&self) -> Vec<Row> {
        match &self.kind {
            Kind::Matches(groups) => {
                let mut rows = Vec::new();
                for group in groups {
                    rows.push(Row::File {
                        path: group.path.clone(),
                        count: group.matches.len(),
                    });
                    for entry in &group.matches {
                        rows.push(Row::Match {
                            line_number: entry.line_number,
                            line: entry.line.clone(),
                        });
                    }
                }
                rows
            }
            Kind::Paths(paths) => paths.iter().cloned().map(Row::Path).collect(),
        }
    }
    /// The reference's banner: the retained count, the pre-cap total when capped, and the file
    /// count for matches (`SearchBlock.tsx:126-133`, `locales.ts:717-720`).
    pub fn summary(&self) -> String {
        let shown = match &self.kind {
            Kind::Matches(groups) => groups
                .iter()
                .map(|group| group.matches.len())
                .sum::<usize>(),
            Kind::Paths(paths) => paths.len(),
        };
        match &self.kind {
            Kind::Matches(groups) if self.truncated => format!(
                "Showing {shown} of {} matches · {} files",
                self.total,
                groups.len()
            ),
            Kind::Matches(groups) => format!("{shown} matches · {} files", groups.len()),
            Kind::Paths(paths) if self.truncated => {
                format!("Showing {shown} of {} paths", self.total)
            }
            Kind::Paths(paths) => format!("{} paths", paths.len()),
        }
    }
    pub fn empty(&self) -> bool {
        self.rows().is_empty()
    }
    pub fn shown(&self, expanded: bool) -> Shown {
        let rows = self.rows();
        let hidden = rows.len().saturating_sub(MAX_ROWS);
        if expanded || hidden == 0 {
            return Shown {
                head: rows,
                tail: Vec::new(),
                hidden: 0,
            };
        }
        let head = MAX_ROWS.div_ceil(2);
        let tail = MAX_ROWS - head;
        Shown {
            head: rows[..head].to_vec(),
            tail: rows[rows.len() - tail..].to_vec(),
            hidden: hidden as u64,
        }
    }
    pub fn drawn_rows(&self, expanded: bool) -> usize {
        let shown = self.shown(expanded);
        shown.head.len() + shown.tail.len() + usize::from(shown.hidden > 0)
    }
}

/// The search cards are file content, so their debug form prints counts, never paths or lines.
impl std::fmt::Debug for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, retained) = match &self.kind {
            Kind::Matches(groups) => (
                "matches",
                groups
                    .iter()
                    .map(|group| group.matches.len())
                    .sum::<usize>(),
            ),
            Kind::Paths(paths) => ("paths", paths.len()),
        };
        f.debug_struct("Card")
            .field("kind", &kind)
            .field("retained", &retained)
            .field("total", &self.total)
            .field("truncated", &self.truncated)
            .finish()
    }
}

/// `validInclude` (`search-card-model.ts:47-58`): a bare comma or a negation is not a pattern the
/// tool ever received.
fn valid_include(include: &str) -> bool {
    if include.trim().is_empty() || include.starts_with('!') {
        return false;
    }
    let mut depth = 0i32;
    for character in include.chars() {
        match character {
            '{' => depth += 1,
            '}' => depth = (depth - 1).max(0),
            ',' if depth == 0 => return false,
            _ => {}
        }
    }
    true
}

fn matches_of(meta: &Value) -> Option<Vec<Group>> {
    let files = meta["files"].as_array()?;
    if files.len() > MAX_GROUPS {
        return None;
    }
    let mut groups = Vec::with_capacity(files.len());
    for file in files {
        let path = file["path"].as_str()?.to_owned();
        let list = file["matches"].as_array()?;
        if list.len() > MAX_MATCHES {
            return None;
        }
        let mut matches = Vec::with_capacity(list.len());
        for entry in list {
            let line_number = entry["lineNumber"].as_u64().filter(|n| *n >= 1)?;
            matches.push(Match {
                line_number,
                line: crate::reducer::bounded(entry["line"].as_str()?, MAX_LINE_BYTES),
            });
        }
        groups.push(Group { path, matches });
    }
    Some(groups)
}

/// Derive the search card for a settled `grep`/`glob` call, when every rule holds.
pub fn derive(call: &Value, result: &Value) -> Option<Card> {
    if result["message"]["isError"].as_bool() == Some(true) {
        return None;
    }
    let name = call["name"].as_str()?;
    let tool = match name {
        "grep" | "functions.grep" | "tool-search/grep" => "grep",
        "glob" | "functions.glob" | "tool-search/glob" => "glob",
        _ => return None,
    };
    let args: Value = serde_json::from_str(call["arguments"].as_str()?).ok()?;
    let pattern = args["pattern"].as_str()?;
    match tool {
        "grep" if pattern.is_empty() => return None,
        "glob" if pattern.trim().is_empty() => return None,
        _ => {}
    }
    if args
        .get("path")
        .is_some_and(|value| value.as_str().is_none_or(|path| path.trim().is_empty()))
    {
        return None;
    }
    if tool == "grep"
        && args
            .get("include")
            .is_some_and(|value| value.as_str().is_none_or(|include| !valid_include(include)))
    {
        return None;
    }
    let meta = &result["meta"];
    let truncated = meta["truncated"].as_bool()?;
    let total = meta["total"].as_u64()?;
    let kind = match tool {
        "grep" => {
            if meta["shape"].as_str() != Some("matches") {
                return None;
            }
            Kind::Matches(matches_of(meta)?)
        }
        _ => {
            if meta["shape"].as_str() != Some("paths") {
                return None;
            }
            let paths = meta["paths"].as_array()?;
            if paths.len() > MAX_PATHS {
                return None;
            }
            let mut out = Vec::with_capacity(paths.len());
            for path in paths {
                out.push(crate::reducer::bounded(path.as_str()?, MAX_LINE_BYTES * 4));
            }
            Kind::Paths(out)
        }
    };
    Some(Card {
        kind,
        total,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(name: &str, args: Value) -> Value {
        json!({"name": name, "arguments": args.to_string()})
    }
    fn result(meta: Value) -> Value {
        json!({"message": {"isError": false, "content": []}, "meta": meta})
    }
    fn grep_meta() -> Value {
        json!({"shape": "matches", "total": 3, "truncated": false, "files": [
            {"path": "src/a.rs", "matches": [
                {"lineNumber": 4, "line": "PUBLIC first"},
                {"lineNumber": 9, "line": "PUBLIC second"}
            ]},
            {"path": "src/b.rs", "matches": [
                {"lineNumber": 1, "line": "PUBLIC third"}
            ]}
        ]})
    }

    #[test]
    fn a_grep_result_groups_matches_under_their_file() {
        let card = derive(
            &call(
                "functions.grep",
                json!({"pattern": "PUBLIC", "path": "src"}),
            ),
            &result(grep_meta()),
        )
        .expect("a settled grep");
        assert_eq!(card.summary(), "3 matches · 2 files");
        assert_eq!(
            card.rows(),
            vec![
                Row::File {
                    path: "src/a.rs".into(),
                    count: 2
                },
                Row::Match {
                    line_number: 4,
                    line: "PUBLIC first".into()
                },
                Row::Match {
                    line_number: 9,
                    line: "PUBLIC second".into()
                },
                Row::File {
                    path: "src/b.rs".into(),
                    count: 1
                },
                Row::Match {
                    line_number: 1,
                    line: "PUBLIC third".into()
                },
            ]
        );
    }

    #[test]
    fn a_capped_result_states_both_counts() {
        let mut meta = grep_meta();
        meta["truncated"] = json!(true);
        meta["total"] = json!(120);
        let card = derive(&call("grep", json!({"pattern": "PUBLIC"})), &result(meta)).unwrap();
        assert_eq!(card.summary(), "Showing 3 of 120 matches · 2 files");
    }

    #[test]
    fn a_glob_result_lists_paths() {
        let card = derive(
            &call("glob", json!({"pattern": "**/*.rs"})),
            &result(json!({"shape": "paths", "total": 2, "truncated": false,
                "paths": ["src/a.rs", "src/b.rs"]})),
        )
        .unwrap();
        assert_eq!(card.summary(), "2 paths");
        assert_eq!(
            card.rows(),
            vec![Row::Path("src/a.rs".into()), Row::Path("src/b.rs".into())]
        );
        let meta = json!({"shape": "paths", "total": 90, "truncated": true,
            "paths": ["src/a.rs"]});
        let card = derive(&call("glob", json!({"pattern": "**/*.rs"})), &result(meta)).unwrap();
        assert_eq!(card.summary(), "Showing 1 of 90 paths");
    }

    #[test]
    fn no_results_is_an_empty_card() {
        let card = derive(
            &call("grep", json!({"pattern": "PUBLIC"})),
            &result(json!({"shape": "matches", "total": 0, "truncated": false, "files": []})),
        )
        .unwrap();
        assert!(card.empty());
        assert_eq!(card.summary(), "0 matches · 0 files");
        assert_eq!(card.drawn_rows(false), 0);
    }

    #[test]
    fn the_cap_keeps_the_head_and_the_tail_of_the_flattened_rows() {
        // Ten files of one match each: twenty flattened rows, capped at eight.
        let files: Vec<Value> = (1..=10)
            .map(|n| {
                json!({"path": format!("src/f{n}.rs"),
                    "matches": [{"lineNumber": n, "line": format!("line {n}")}]})
            })
            .collect();
        let card = derive(
            &call("grep", json!({"pattern": "PUBLIC"})),
            &result(json!({"shape": "matches", "total": 10, "truncated": false, "files": files})),
        )
        .unwrap();
        let shown = card.shown(false);
        assert_eq!(shown.head.len(), 4);
        assert_eq!(shown.tail.len(), 4);
        assert_eq!(shown.hidden, 12);
        assert_eq!(
            shown.head[0],
            Row::File {
                path: "src/f1.rs".into(),
                count: 1
            }
        );
        assert_eq!(
            shown.tail[3],
            Row::Match {
                line_number: 10,
                line: "line 10".into()
            }
        );
        assert_eq!(card.drawn_rows(false), 9);
        assert_eq!(card.drawn_rows(true), 20);
    }

    #[test]
    fn metadata_that_fails_a_rule_keeps_the_text_body() {
        let cases = [
            (
                call("grep", json!({"pattern": "PUBLIC"})),
                json!({"shape": "paths", "total": 1, "truncated": false, "paths": []}),
            ),
            (
                call("grep", json!({"pattern": "PUBLIC"})),
                json!({"shape": "matches", "total": 1, "truncated": false}),
            ),
            (
                call("grep", json!({"pattern": "PUBLIC"})),
                json!({"shape": "matches", "total": "1", "truncated": false, "files": []}),
            ),
            (
                call("grep", json!({"pattern": "PUBLIC"})),
                json!({"shape": "matches", "total": 1, "files": []}),
            ),
            (
                call("grep", json!({"pattern": "PUBLIC"})),
                json!({"shape": "matches", "total": 1, "truncated": false, "files": [{"path": "a", "matches": [{"lineNumber": 0, "line": "x"}]}]}),
            ),
            (
                call("grep", json!({"pattern": "PUBLIC"})),
                json!({"shape": "matches", "total": 1, "truncated": false, "files": [{"path": "a", "matches": [{"lineNumber": 1, "line": 2}]}]}),
            ),
            (
                call("glob", json!({"pattern": "**/*.rs"})),
                json!({"shape": "paths", "total": 1, "truncated": false, "paths": [4]}),
            ),
        ];
        for (call, meta) in cases {
            assert!(derive(&call, &result(meta.clone())).is_none(), "{meta}");
        }
    }

    #[test]
    fn a_call_that_fails_a_rule_or_an_error_result_keeps_the_text_body() {
        let ok = result(grep_meta());
        for (call, expected) in [
            (
                json!({"name": "functions.read", "arguments": "{\"pattern\":\"x\"}"}),
                false,
            ),
            (call("grep", json!({"pattern": ""})), false),
            (call("glob", json!({"pattern": "  "})), false),
            (call("grep", json!({"pattern": "x", "path": " "})), false),
            (
                call("grep", json!({"pattern": "x", "include": "!*.rs"})),
                false,
            ),
            (
                call("grep", json!({"pattern": "x", "include": "*.rs,*.md"})),
                false,
            ),
            // A braced include is the tool's own alternation syntax and is valid.
            (
                call("grep", json!({"pattern": "x", "include": "*.{rs,md}"})),
                true,
            ),
        ] {
            assert_eq!(derive(&call, &ok).is_some(), expected, "{call}");
        }
        let error = json!({"message": {"isError": true, "content": []}, "meta": grep_meta()});
        assert!(derive(&call("grep", json!({"pattern": "x"})), &error).is_none());
    }

    #[test]
    fn the_debug_form_never_prints_paths_or_lines() {
        let card = derive(
            &call("grep", json!({"pattern": "PUBLIC"})),
            &result(grep_meta()),
        )
        .unwrap();
        let debug = format!("{card:?}");
        assert!(!debug.contains("src/"), "{debug}");
        assert!(!debug.contains("PUBLIC"), "{debug}");
        assert!(debug.contains("retained: 3"), "{debug}");
    }
}
