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

/// The reference's per-row metrics (`SubagentHeaderLineage.tsx:250-270`): a four-bucket token
/// total and the active duration, either of which may be absent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Metrics {
    pub tokens: Option<u64>,
    pub duration_ms: Option<u64>,
}

/// Read `tokenUsage` and `subagentTiming` from one session's projections. `now_ms` is only read
/// for a running child; an inactive one uses the recorded `through`. A partially written usage
/// object yields no token total rather than a partial sum, as the reference's required fields do.
pub fn metrics(projections: &BTreeMap<String, Value>, running: bool, now_ms: u64) -> Metrics {
    let tokens = projections.get("tokenUsage").and_then(|usage| {
        let mut total = 0u64;
        for key in [
            "uncachedInputTokens",
            "outputTokens",
            "cacheReadTokens",
            "cacheWriteTokens",
        ] {
            total = total.checked_add(usage.get(key)?.as_u64()?)?;
        }
        Some(total)
    });
    let duration_ms = projections.get("subagentTiming").and_then(|timing| {
        let settled = timing.get("settledMs")?.as_u64()?;
        let Some(active) = timing.get("active") else {
            return Some(settled);
        };
        let since = active.get("since")?.as_u64()?;
        let end = if running {
            now_ms
        } else {
            active.get("through")?.as_u64()?
        };
        Some(settled.saturating_add(end.saturating_sub(since)))
    });
    Metrics {
        tokens,
        duration_ms,
    }
}

/// The three activity states the reference distinguishes: a child that is running, one whose last
/// turn finished, and one that has never run and is not running
/// (`SubagentHeaderLineage.tsx:243-249`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    Running,
    Completed,
    Idle,
}

impl Activity {
    /// The word the row's secondary line carries.
    pub fn label(self) -> &'static str {
        match self {
            Activity::Running => "running",
            Activity::Completed => "completed",
            Activity::Idle => "inactive",
        }
    }
}

/// Decide a child's activity from its own run state and the `subagentTiming` projection the reference
/// reads for the same purpose (`SubagentHeaderLineage.tsx:243-249`): `completed` is "not running, and
/// its last turn finished", which is not the same as "has not run".
pub fn activity(projections: &BTreeMap<String, Value>, running: bool) -> Activity {
    if running {
        return Activity::Running;
    }
    let completed = projections
        .get("subagentTiming")
        .and_then(|timing| timing.get("lastTurnCompleted"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if completed {
        Activity::Completed
    } else {
        Activity::Idle
    }
}

/// The reference's compact token form: whole below 1000, one decimal under 100K, then whole
/// thousands, then millions (`SubagentHeaderLineage.tsx:57-65`).
pub fn format_tokens(value: u64) -> String {
    let scaled = |next: f64| -> String {
        if next >= 100.0 {
            format!("{}", next.round() as u64)
        } else {
            format!("{:.1}", (next * 10.0).round() / 10.0)
        }
    };
    match value {
        0..=999 => value.to_string(),
        1_000..=999_999 => format!("{}K", scaled(value as f64 / 1_000.0)),
        _ => format!("{}M", scaled(value as f64 / 1_000_000.0)),
    }
}

/// The reference's compact duration ladder: seconds, minutes+seconds, hours+minutes+seconds,
/// days(+hours), months(+days), years(+months) (`SubagentHeaderLineage.tsx:117-155`).
pub fn format_duration(ms: u64) -> String {
    let total_seconds = ms / 1_000;
    let minutes = total_seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    let seconds = total_seconds % 60;
    let rest_minutes = minutes % 60;
    let rest_hours = hours % 24;
    if days >= 365 {
        let years = days / 365;
        let months = (days % 365) / 30;
        return if months == 0 {
            format!("~{years}y")
        } else {
            format!("~{years}y {months}mo")
        };
    }
    if days >= 30 {
        let months = days / 30;
        let rest_days = days % 30;
        return if rest_days == 0 {
            format!("~{months}mo")
        } else {
            format!("~{months}mo {rest_days}d")
        };
    }
    if days > 0 {
        return if rest_hours == 0 {
            format!("{days}d")
        } else {
            format!("{days}d {rest_hours}h")
        };
    }
    if hours > 0 {
        return format!("{hours}h {rest_minutes}m {seconds}s");
    }
    if minutes > 0 {
        return format!("{minutes}m {seconds}s");
    }
    format!("{seconds}s")
}

/// One row of the catalog menu's tree: the entry, its indent level, whether it can be opened, and
/// whether its children are showing.
///
/// `expandable` is not "has children": the reference shows a chevron for anything that is not a
/// **known leaf** — a child whose own catalog has not been read yet can still have descendants
/// (`SubagentHeaderLineage.tsx:195-198,319-331`), so the row must offer the toggle and let the read
/// that follows fill the branch in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeRow {
    pub entry: Entry,
    pub level: usize,
    pub expandable: bool,
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
    mut catalog_for: impl FnMut(&str) -> Option<Vec<Entry>>,
) -> Vec<TreeRow> {
    fn walk(
        entries: &[Entry],
        level: usize,
        expanded: &std::collections::BTreeSet<String>,
        catalog_for: &mut impl FnMut(&str) -> Option<Vec<Entry>>,
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
            let catalog = if level + 1 < MAX_DEPTH {
                catalog_for(&entry.id)
            } else {
                Some(Vec::new())
            };
            // Known leaf: the catalog was read and held nothing. Anything else — rows present, or no
            // catalog yet — keeps the toggle.
            let known_leaf = catalog.as_ref().is_some_and(Vec::is_empty);
            let children: Vec<_> = catalog
                .unwrap_or_default()
                .into_iter()
                .filter(|child| !visited.contains(&child.id))
                .collect();
            let has_children = !children.is_empty();
            let open = expanded.contains(&entry.id) && !known_leaf;
            out.push(TreeRow {
                entry: entry.clone(),
                level,
                expandable: !known_leaf,
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

/// The header action's copy. The reference always states the **total** here and marks live work with
/// a state dot instead of words (`SubagentHeaderLineage.tsx:686-694`); its "running" wording is the
/// button's accessible label, not the text. An earlier version replaced the visible total with the
/// running count, so "3 subagents, 1 running" read as "1 subagent running".
pub fn count_label(total: usize, running: usize) -> String {
    let _ = running;
    format!("{total} subagent{}", if total == 1 { "" } else { "s" })
}

/// The accessible description the reference puts on the same button: the running count while
/// anything runs, otherwise the total (`SubagentHeaderLineage.tsx:660-665`).
pub fn count_description(total: usize, running: usize) -> String {
    if running > 0 {
        format!(
            "{running} subagent{} running",
            if running == 1 { "" } else { "s" }
        )
    } else {
        count_label(total, running)
    }
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
    fn metrics_sum_the_four_token_buckets_and_the_active_duration() {
        let projections = BTreeMap::from([
            (
                "tokenUsage".to_owned(),
                json!({"uncachedInputTokens":1000,"outputTokens":250,"cacheReadTokens":2000,"cacheWriteTokens":50}),
            ),
            (
                "subagentTiming".to_owned(),
                json!({"settledMs":84000,"active":{"since":1000,"through":61000}}),
            ),
        ]);
        assert_eq!(
            metrics(&projections, false, 999_999),
            Metrics {
                tokens: Some(3300),
                duration_ms: Some(84_000 + 60_000)
            }
        );
        // Running uses the supplied now instead of `through`.
        assert_eq!(
            metrics(&projections, true, 31_000).duration_ms,
            Some(84_000 + 30_000)
        );
        // A usage object missing one bucket yields no total rather than a partial sum.
        let partial = BTreeMap::from([(
            "tokenUsage".to_owned(),
            json!({"uncachedInputTokens":1,"outputTokens":2,"cacheReadTokens":3}),
        )]);
        assert_eq!(metrics(&partial, false, 0).tokens, None);
        // Timing without `active` is the settled duration alone.
        let settled = BTreeMap::from([("subagentTiming".to_owned(), json!({"settledMs":5000}))]);
        assert_eq!(metrics(&settled, true, 0).duration_ms, Some(5000));
        assert_eq!(metrics(&BTreeMap::new(), true, 0), Metrics::default());
    }

    #[test]
    fn tokens_and_durations_follow_the_reference_compact_forms() {
        for (value, shown) in [
            (0, "0"),
            (999, "999"),
            (1_000, "1.0K"),
            (12_345, "12.3K"),
            (99_950, "100.0K"),
            (999_999, "1000K"),
            (1_000_000, "1.0M"),
            (2_500_000, "2.5M"),
        ] {
            assert_eq!(format_tokens(value), shown, "{value}");
        }
        for (ms, shown) in [
            (0, "0s"),
            (9_000, "9s"),
            (65_000, "1m 5s"),
            (3_661_000, "1h 1m 1s"),
            (90_000_000, "1d 1h"),
            (86_400_000 * 45, "~1mo 15d"),
            (86_400_000 * 400, "~1y 1mo"),
        ] {
            assert_eq!(format_duration(ms), shown, "{ms}");
        }
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
            "PUBLIC-a" => Some(catalogs(&["PUBLIC-b", "PUBLIC-c"])),
            "PUBLIC-b" => Some(catalogs(&["PUBLIC-d"])),
            _ => None,
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
            "PUBLIC-a" => Some(catalogs(&["PUBLIC-b", "PUBLIC-c"])),
            "PUBLIC-b" => Some(catalogs(&["PUBLIC-d"])),
            _ => None,
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
                "PUBLIC-a" => Some(catalogs(&["PUBLIC-b"])),
                "PUBLIC-b" => Some(catalogs(&["PUBLIC-a"])),
                _ => None,
            },
        );
        assert_eq!(cycle.len(), 2);
        assert!(!cycle[1].has_children);
    }

    #[test]
    fn a_childs_activity_distinguishes_completed_from_never_run() {
        use serde_json::json;
        let mut projections = BTreeMap::new();
        // Running wins over everything the projections say.
        assert_eq!(activity(&projections, true), Activity::Running);
        // Not running, no timing: it has never run.
        assert_eq!(activity(&projections, false), Activity::Idle);
        // Not running with a finished last turn: completed, not idle.
        projections.insert(
            "subagentTiming".to_owned(),
            json!({"settledMs": 1200, "lastTurnCompleted": true}),
        );
        assert_eq!(activity(&projections, false), Activity::Completed);
        assert_eq!(activity(&projections, true), Activity::Running);
        // A recorded timing whose last turn is still open is idle, not completed.
        projections.insert(
            "subagentTiming".to_owned(),
            json!({"lastTurnCompleted": false}),
        );
        assert_eq!(activity(&projections, false), Activity::Idle);
        // The row's word for each state.
        assert_eq!(Activity::Running.label(), "running");
        assert_eq!(Activity::Completed.label(), "completed");
        assert_eq!(Activity::Idle.label(), "inactive");
    }

    #[test]
    fn the_header_copy_matches_the_reference_plurals_and_modes() {
        // The visible text is always the total, whatever is running.
        assert_eq!(count_label(1, 0), "1 subagent");
        assert_eq!(count_label(3, 0), "3 subagents");
        assert_eq!(count_label(3, 1), "3 subagents");
        assert_eq!(count_label(3, 2), "3 subagents");
        // The running form is the button's description.
        assert_eq!(count_description(3, 1), "1 subagent running");
        assert_eq!(count_description(3, 2), "2 subagents running");
        assert_eq!(count_description(3, 0), "3 subagents");
        assert_eq!(mode_label(Mode::OneShot), "one-shot");
        assert_eq!(mode_label(Mode::Continuable), "continuable");
        assert_eq!(mode_label(Mode::Unknown), "unknown mode");
    }
}
