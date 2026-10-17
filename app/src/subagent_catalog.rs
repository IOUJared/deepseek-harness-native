//! The parent session's direct-child catalog, read from the Host's `subagentCatalog`
//! projection (`packages/subagent/subagent/src/catalog.ts:96-150`).
//!
//! The projection view is an array of `{ id, createdAt, mode, label? }`; `activity` is not a
//! projection field — the reference derives it from the live session summaries
//! (`SubagentHeaderLineage.tsx:459-465`). This module reads only the projection; the App
//! supplies the running state from its own session table.

use std::collections::BTreeMap;

use serde_json::Value;

/// The projection key the Host publishes under `projections.values`.
pub const KEY: &str = "subagentCatalog";
/// Bound the list like every other native surface; the Host owns the order.
pub const MAX_ENTRIES: usize = 64;
/// The menu tree's total rows, across every expanded branch.
pub const MAX_ROWS: usize = 64;
const MAX_ID: usize = 128;
const MAX_LABEL: usize = 256;

/// A child's continuation mode, exactly as the projection's view spells it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    OneShot,
    Continuable,
    Unknown,
}

/// One direct child: its session id, the label the parent published, and its creation time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub label: String,
    pub mode: Mode,
    pub created_at: u64,
}

/// Parse the catalog projection. A missing or malformed value yields no rows rather than an
/// error: the header simply does not appear, which is the reference's rule for an unready
/// catalog (`SubagentHeaderLineage.tsx:601-606`).
pub fn entries(projections: &BTreeMap<String, Value>) -> Vec<Entry> {
    let Some(Value::Array(rows)) = projections.get(KEY) else {
        return Vec::new();
    };
    rows.iter().take(MAX_ENTRIES).filter_map(parse).collect()
}

fn parse(row: &Value) -> Option<Entry> {
    let id = row.get("id")?.as_str()?.trim();
    if id.is_empty() || id.len() > MAX_ID {
        return None;
    }
    let label = row
        .get("label")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|label| !label.is_empty() && label.len() <= MAX_LABEL)
        .unwrap_or(id)
        .to_owned();
    let mode = match row.get("mode").and_then(Value::as_str)? {
        "one-shot" => Mode::OneShot,
        "continuable" => Mode::Continuable,
        "unknown" => Mode::Unknown,
        _ => return None,
    };
    let created_at = row.get("createdAt")?.as_u64()?;
    Some(Entry {
        id: id.to_owned(),
        label,
        mode,
        created_at,
    })
}

/// The mode named by the newest valid `subagent/descriptor` v3 event in a child's own log.
/// The descriptor is the child's durable fact (`projection.ts:61-90`), so this is how the
/// read-only composer learns whether the child accepts follow-ups. A malformed descriptor is
/// skipped; the newest valid one wins.
pub fn descriptor_mode<'a>(events: impl Iterator<Item = (&'a str, &'a Value)>) -> Option<Mode> {
    events
        .filter(|(kind, _)| *kind == "subagent/descriptor")
        .filter_map(|(_, data)| {
            if data.get("version").and_then(Value::as_u64) != Some(3) {
                return None;
            }
            match data.get("mode").and_then(Value::as_str)? {
                "one-shot" => Some(Mode::OneShot),
                "continuable" => Some(Mode::Continuable),
                "unknown" => Some(Mode::Unknown),
                _ => None,
            }
        })
        .last()
}

/// One row of the catalog menu's tree: the entry, its indent level, whether it has children,
/// and whether those children are showing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeRow {
    pub entry: Entry,
    pub level: usize,
    pub has_children: bool,
    pub expanded: bool,
}

/// The menu rows in display order, with each expanded branch's children directly after it.
/// `catalog_for` returns one session's own catalog; a missing one is an empty list. The walk is
/// bounded by [`MAX_ROWS`] and three nested levels, and a repeated id never expands again, so a
/// malformed catalog cannot cycle or blow the menu up.
pub fn tree_rows(
    roots: &[Entry],
    expanded: &std::collections::BTreeSet<String>,
    mut catalog_for: impl FnMut(&str) -> Vec<Entry>,
) -> Vec<TreeRow> {
    fn walk(
        entries: &[Entry],
        level: usize,
        expanded: &std::collections::BTreeSet<String>,
        catalog_for: &mut impl FnMut(&str) -> Vec<Entry>,
        visited: &mut std::collections::BTreeSet<String>,
        out: &mut Vec<TreeRow>,
    ) {
        const MAX_DEPTH: usize = 4;
        for entry in entries {
            if out.len() >= MAX_ROWS {
                return;
            }
            if !visited.insert(entry.id.clone()) {
                continue;
            }
            let children: Vec<_> = if level + 1 < MAX_DEPTH {
                catalog_for(&entry.id)
                    .into_iter()
                    .filter(|child| !visited.contains(&child.id))
                    .collect()
            } else {
                Vec::new()
            };
            let has_children = !children.is_empty();
            let open = has_children && expanded.contains(&entry.id);
            out.push(TreeRow {
                entry: entry.clone(),
                level,
                has_children,
                expanded: open,
            });
            if open {
                walk(&children, level + 1, expanded, catalog_for, visited, out);
            }
        }
    }
    let mut out = Vec::new();
    let mut visited = std::collections::BTreeSet::new();
    walk(roots, 0, expanded, &mut catalog_for, &mut visited, &mut out);
    out
}

/// The header action's copy, from the reference's `count.total.*`/`count.running.*` strings:
/// the running count wins while anything runs, otherwise the total.
pub fn count_label(total: usize, running: usize) -> String {
    let (count, suffix) = if running > 0 {
        (running, " running")
    } else {
        (total, "")
    };
    format!(
        "{count} subagent{}{suffix}",
        if count == 1 { "" } else { "s" }
    )
}

/// The reference's mode labels (`locales.ts` `mode.*`).
pub fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::OneShot => "one-shot",
        Mode::Continuable => "continuable",
        Mode::Unknown => "unknown mode",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn projections(value: Value) -> BTreeMap<String, Value> {
        BTreeMap::from([(KEY.to_owned(), value)])
    }

    #[test]
    fn the_catalog_reads_ids_labels_modes_and_times_and_drops_malformed_rows() {
        let rows = entries(&projections(json!([
            {"id":"PUBLIC-child","createdAt":1000,"mode":"continuable","label":"PUBLIC reviewer"},
            {"id":"PUBLIC-oneshot","createdAt":1001,"mode":"one-shot"},
            {"id":"PUBLIC-unknown","createdAt":1002,"mode":"unknown","label":"PUBLIC scout"},
            {"id":"","createdAt":1,"mode":"one-shot"},
            {"id":"PUBLIC-bad-mode","createdAt":1,"mode":"sideways"},
            {"id":"PUBLIC-bad-time","createdAt":-1,"mode":"one-shot"},
            {"id":"PUBLIC-extra","createdAt":1,"mode":"one-shot","label":"x","extra":true},
        ])));
        let ids: Vec<_> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "PUBLIC-child",
                "PUBLIC-oneshot",
                "PUBLIC-unknown",
                "PUBLIC-extra"
            ]
        );
        assert_eq!(rows[0].label, "PUBLIC reviewer");
        assert_eq!(
            rows[1].label, "PUBLIC-oneshot",
            "a missing label falls back to the id"
        );
        assert_eq!(rows[0].mode, Mode::Continuable);
        assert_eq!(rows[1].mode, Mode::OneShot);
        assert_eq!(rows[2].mode, Mode::Unknown);
        assert_eq!(rows[0].created_at, 1000);
    }

    #[test]
    fn a_missing_or_wrong_typed_catalog_is_empty_not_an_error() {
        assert!(entries(&BTreeMap::new()).is_empty());
        assert!(entries(&projections(json!({"not":"an array"}))).is_empty());
        assert!(entries(&projections(json!([]))).is_empty());
    }

    #[test]
    fn the_descriptor_mode_is_the_newest_valid_child_descriptor() {
        use serde_json::json;
        let one = json!({"version":3,"provider":"PUBLIC","mode":"one-shot"});
        let cont = json!({"version":3,"provider":"PUBLIC","mode":"continuable","label":"x"});
        let bad = json!({"version":2,"provider":"PUBLIC","mode":"continuable","label":"x"});
        let events = vec![
            ("subagent/descriptor", &one),
            ("user/message", &bad),
            ("subagent/descriptor", &bad),
            ("subagent/descriptor", &cont),
        ];
        assert_eq!(descriptor_mode(events.into_iter()), Some(Mode::Continuable));
        assert_eq!(descriptor_mode(std::iter::empty()), None);
    }

    #[test]
    fn the_tree_expands_branches_indents_them_and_survives_a_cycle() {
        use std::collections::BTreeSet;
        let entry = |id: &str| Entry {
            id: id.to_owned(),
            label: id.to_owned(),
            mode: Mode::Continuable,
            created_at: 1,
        };
        let catalogs = |children: &[&'static str]| children.iter().map(|id| entry(id)).collect();
        let roots = vec![entry("PUBLIC-a")];
        let mut expanded = BTreeSet::new();
        expanded.insert("PUBLIC-a".to_owned());
        let rows = tree_rows(&roots, &expanded, |id| match id {
            "PUBLIC-a" => catalogs(&["PUBLIC-b", "PUBLIC-c"]),
            "PUBLIC-b" => catalogs(&["PUBLIC-d"]),
            _ => Vec::new(),
        });
        let seen: Vec<_> = rows
            .iter()
            .map(|row| {
                (
                    row.entry.id.as_str(),
                    row.level,
                    row.has_children,
                    row.expanded,
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                ("PUBLIC-a", 0, true, true),
                ("PUBLIC-b", 1, true, false),
                ("PUBLIC-c", 1, false, false),
            ]
        );
        expanded.insert("PUBLIC-b".to_owned());
        let rows = tree_rows(&roots, &expanded, |id| match id {
            "PUBLIC-a" => catalogs(&["PUBLIC-b", "PUBLIC-c"]),
            "PUBLIC-b" => catalogs(&["PUBLIC-d"]),
            _ => Vec::new(),
        });
        let seen: Vec<_> = rows
            .iter()
            .map(|row| (row.entry.id.as_str(), row.level))
            .collect();
        assert_eq!(
            seen,
            [
                ("PUBLIC-a", 0),
                ("PUBLIC-b", 1),
                ("PUBLIC-d", 2),
                ("PUBLIC-c", 1),
            ]
        );
        // A cycle stops at the repeated id instead of recursing forever.
        let cycle = tree_rows(
            &roots,
            &BTreeSet::from(["PUBLIC-a".to_owned()]),
            |id| match id {
                "PUBLIC-a" => catalogs(&["PUBLIC-b"]),
                "PUBLIC-b" => catalogs(&["PUBLIC-a"]),
                _ => Vec::new(),
            },
        );
        assert_eq!(cycle.len(), 2);
        assert!(!cycle[1].has_children);
    }

    #[test]
    fn the_header_copy_matches_the_reference_plurals_and_modes() {
        assert_eq!(count_label(1, 0), "1 subagent");
        assert_eq!(count_label(3, 0), "3 subagents");
        assert_eq!(count_label(3, 1), "1 subagent running");
        assert_eq!(count_label(3, 2), "2 subagents running");
        assert_eq!(mode_label(Mode::OneShot), "one-shot");
        assert_eq!(mode_label(Mode::Continuable), "continuable");
        assert_eq!(mode_label(Mode::Unknown), "unknown mode");
    }
}
