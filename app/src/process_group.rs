//! Chat's process-group titles (`process-activity.ts:8-23`, `step-process.ts:10-28`,
//! `locale.ts:228-248`).
//!
//! Consecutive step activity inside one turn — tool calls, reasoning, plans — forms one group. The
//! group's header names what happened by category, not by tool: the top three categories as
//! `Searched code, Read files and Edited files`, with `, etc.` when there were more.

/// One work category, named as the reference's `message.stepProcess.done.*` keys are.
pub const DONE_THINKING: &str = "Analysis completed";
const DONE_LABELS: &[(&str, &str)] = &[
    ("read", "Read files"),
    ("readImage", "Read images"),
    ("write", "Wrote files"),
    ("search", "Searched code"),
    ("edit", "Edited files"),
    ("commands", "Ran commands"),
    ("code", "Ran code"),
    ("webSearch", "Searched the web"),
    ("webFetch", "Visited web pages"),
    ("subagents", "Coordinated subagents"),
    ("plan", "Updated the plan"),
    ("questions", "Asked questions"),
    ("tools", "Called tools"),
];

/// The reference's tool-name → category map (`process-activity.ts:8-23`).
pub fn category(name: &str) -> &'static str {
    let name = name.strip_prefix("functions.").unwrap_or(name);
    match name {
        "read" => "read",
        "read_image" => "readImage",
        "write" => "write",
        "edit" | "apply_patch" | "str_replace_editor" | "multiedit" | "multi_edit" => "edit",
        "grep" | "glob" => "search",
        "bash" | "pwsh" | "exec_command" | "write_stdin" => "commands",
        "run_code" => "code",
        "web_search" => "webSearch",
        "web_fetch" => "webFetch",
        "subagent" => "subagents",
        "todo_write" | "create_goal" | "update_goal" | "get_goal" | "update_plan" => "plan",
        "ask_user_question" | "request_user_input" => "questions",
        _ if name.starts_with("terminal_") => "commands",
        _ if name.starts_with("subagent_") => "subagents",
        _ if name.ends_with("_inspect") => "search",
        _ => "tools",
    }
}

/// One ranked category count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Count {
    pub category: &'static str,
    pub count: usize,
}

/// Rank categories by count, largest first, ties ordered by the category's own key so a title is
/// stable across replays (`process-activity.ts` ranks the same way).
pub fn counts(categories: impl IntoIterator<Item = &'static str>) -> Vec<Count> {
    // First appearance breaks ties, as the reference's stable sort over its count map does, so a
    // group that searched and then read keeps `Searched code and read files`.
    let mut totals: Vec<(&'static str, usize)> = Vec::new();
    for category in categories {
        match totals.iter_mut().find(|(key, _)| *key == category) {
            Some((_, count)) => *count += 1,
            None => totals.push((category, 1)),
        }
    }
    totals.sort_by(|left, right| right.1.cmp(&left.1));
    totals
        .into_iter()
        .map(|(category, count)| Count { category, count })
        .collect()
}

/// The reference's live label for a running group: `message.stepProcess.<activity>`
/// (`locale.ts:201-214`), which names what the group is doing rather than summarizing it.
const LIVE_LABELS: &[(&str, &str)] = &[
    ("read", "Reading files"),
    ("readImage", "Reading images"),
    ("write", "Writing files"),
    ("search", "Searching code"),
    ("edit", "Editing files"),
    ("commands", "Running commands"),
    ("code", "Running code"),
    ("webSearch", "Searching the web"),
    ("webFetch", "Visiting web pages"),
    ("subagents", "Coordinating subagents"),
    ("plan", "Updating the plan"),
    ("questions", "Waiting for your action"),
    ("tools", "Calling tools"),
];

/// `message.stepProcess.<activity>` for a live group, or `Analyzing the request` when the group has
/// no tool activity yet (the reference's `thinking` default).
pub fn live_label(category: Option<&str>) -> &'static str {
    category
        .and_then(|category| {
            LIVE_LABELS
                .iter()
                .find_map(|(key, label)| (*key == category).then_some(*label))
        })
        .unwrap_or("Analyzing the request")
}

/// The seat's title. A running group names its current activity, as the reference's live header
/// does; a settled group states the composed summary.
pub fn seat_title(counts: &[Count], live: Option<&str>) -> String {
    match live {
        Some(category) => live_label(Some(category)).to_owned(),
        None => title(counts),
    }
}

/// The longest live detail the reference keeps before eliding (`process-activity.ts:25-27`).
const LIVE_DETAIL_MAX_CHARS: usize = 160;
/// The argument keys the reference reads, in order (`process-activity.ts:28-34`).
const LIVE_DETAIL_KEYS: &[&str] = &[
    "title",
    "description",
    "objective",
    "task",
    "task_name",
    "name",
    "question",
    "questions",
    "prompt",
    "message",
    "command",
    "cmd",
    "queries",
    "query",
    "pattern",
    "url",
    "uri",
    "file_path",
    "path",
    "target",
    "action",
    "status",
];

/// The live detail of one tool call: the first present key's text, whitespace-collapsed, elided at
/// the reference's bound (`process-activity.ts:64-100`). Strings or arrays of strings only.
pub fn live_detail(args: &serde_json::Value) -> Option<String> {
    for key in LIVE_DETAIL_KEYS {
        let value = &args[*key];
        let text = match value {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Array(items)
                if !items.is_empty()
                    && items
                        .iter()
                        .all(|item| item.as_str().is_some_and(|text| !text.is_empty())) =>
            {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            }
            _ => continue,
        };
        let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            continue;
        }
        if collapsed.chars().count() <= LIVE_DETAIL_MAX_CHARS {
            return Some(collapsed);
        }
        let mut short: String = collapsed.chars().take(LIVE_DETAIL_MAX_CHARS - 1).collect();
        short = short.trim_end().to_owned();
        short.push('\u{2026}');
        return Some(short);
    }
    None
}

/// `message.stepProcess.done.*` for one category.
pub fn label(category: &str) -> &'static str {
    DONE_LABELS
        .iter()
        .find_map(|(key, label)| (*key == category).then_some(*label))
        .unwrap_or(DONE_LABELS[DONE_LABELS.len() - 1].1)
}

/// The reference's title composition (`step-process.ts:10-28`): the top three labels, the second and
/// later ones lower-cased in their first character, joined with `, ` and ` and ` — and `, etc.` when
/// there were more than three categories. No counts appear.
pub fn title(counts: &[Count]) -> String {
    let labels: Vec<&str> = counts
        .iter()
        .take(3)
        .map(|count| label(count.category))
        .collect();
    let Some(first) = labels.first().copied() else {
        return DONE_THINKING.to_owned();
    };
    let lower = |label: &str| {
        label
            .char_indices()
            .next()
            .map(|(_, first)| first.to_lowercase().collect::<String>() + &label[first.len_utf8()..])
            .unwrap_or_default()
    };
    let title = match labels.len() {
        1 => first.to_owned(),
        2 => format!("{first} and {}", lower(labels[1])),
        _ => {
            let mut title = first.to_owned();
            for label in &labels[1..] {
                title.push_str(", ");
                title.push_str(&lower(label));
            }
            title
        }
    };
    if counts.len() > 3 {
        format!("{title}, etc.")
    } else {
        title
    }
}

#[cfg(test)]
mod tests {
    //! The categories and the title composition, as the reference writes them.
    use super::*;

    #[test]
    fn tools_map_to_the_reference_categories() {
        let cases = [
            ("read", "read"),
            ("read_image", "readImage"),
            ("grep", "search"),
            ("glob", "search"),
            ("codebase_inspect", "search"),
            ("write", "write"),
            ("edit", "edit"),
            ("apply_patch", "edit"),
            ("bash", "commands"),
            ("exec_command", "commands"),
            ("terminal_start", "commands"),
            ("write_stdin", "commands"),
            ("run_code", "code"),
            ("web_search", "webSearch"),
            ("web_fetch", "webFetch"),
            ("subagent", "subagents"),
            ("subagent_write", "subagents"),
            ("todo_write", "plan"),
            ("create_goal", "plan"),
            ("update_goal", "plan"),
            ("ask_user_question", "questions"),
            ("schedule_create", "tools"),
            ("functions.read", "read"),
        ];
        for (name, expected) in cases {
            assert_eq!(category(name), expected, "{name}");
        }
    }

    /// A title from tool names, mapped through the category table first.
    fn title_of(names: &[&str]) -> String {
        let ranked = counts(names.iter().map(|name| category(name)));
        title(&ranked)
    }

    #[test]
    fn a_title_names_at_most_three_categories_without_counts() {
        assert_eq!(title(&[]), "Analysis completed");
        assert_eq!(title_of(&["read"]), "Read files");
        // One category repeated twice keeps one label and no count.
        assert_eq!(title_of(&["read", "read"]), "Read files");
        // Two categories join with `and` and the second label is lower-cased.
        assert_eq!(title_of(&["grep", "read"]), "Searched code and read files");
        // Three or more join with commas only: the reference's `joinTwo` is the only `and`.
        assert_eq!(
            title_of(&["grep", "read", "edit"]),
            "Searched code, read files, edited files"
        );
        // More than three adds the reference's `, etc.`.
        assert_eq!(
            title_of(&["grep", "read", "edit", "write"]),
            "Searched code, read files, edited files, etc."
        );
        // Ranked by count first: two searches outrank one read.
        assert_eq!(
            title_of(&["read", "grep", "glob"]),
            "Searched code and read files"
        );
    }

    #[test]
    fn counts_rank_and_order_stably() {
        let ranked = counts(["read", "read", "bash"].map(category));
        assert_eq!(ranked[0].category, "read");
        assert_eq!(ranked[0].count, 2);
        assert_eq!(ranked[1].category, "commands");
        assert_eq!(ranked[1].count, 1);
    }
}

/// One group of consecutive step rows and the title its seat states.
#[derive(Clone, PartialEq, Eq)]
pub struct Group {
    /// The row index the seat precedes, and the only member that draws when the group is closed.
    pub first: usize,
    pub members: Vec<usize>,
    pub title: String,
}

impl std::fmt::Debug for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Group")
            .field("first", &self.first)
            .field("members", &self.members.len())
            .field("title", &self.title)
            .finish()
    }
}

/// Whether a row's role is step activity the reference groups (`process-groups.ts:134-180`: every
/// node that is neither a user message, a turn trigger, a retry, an error nor an assistant reply).
pub fn groups_role(role: &str) -> bool {
    matches!(
        role,
        "Tool call" | "Skill" | "Command" | "Plan" | "Question" | "Subagent"
    )
}

/// One row as the segmenter reads it: its role, its turn and the tool that names its category.
pub type SegmentRow<'a> = (&'a str, Option<u64>, Option<&'a str>);

/// Segment rows into groups. A group ends at the first row that is not step activity or that
/// belongs to another turn; a single member still forms a group, as the reference's seat does.
pub fn groups(rows: &[SegmentRow<'_>]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    let mut open: Option<(u64, Vec<usize>)> = None;
    for (index, (role, turn, tool)) in rows.iter().enumerate() {
        let grouped = groups_role(role) && tool.is_some();
        if let (Some((turn_of_group, members)), true) = (open.as_mut(), grouped)
            && Some(*turn_of_group) == *turn
        {
            members.push(index);
            continue;
        }
        if let Some((_, members)) = open.take() {
            groups.push(finish_group(rows, members));
        }
        if grouped && let Some(turn) = *turn {
            open = Some((turn, vec![index]));
        }
    }
    if let Some((_, members)) = open {
        groups.push(finish_group(rows, members));
    }
    groups
}

fn finish_group(rows: &[SegmentRow<'_>], members: Vec<usize>) -> Group {
    let ranked = counts(
        members
            .iter()
            .filter_map(|index| rows.get(*index).and_then(|row| row.2))
            .map(category),
    );
    Group {
        first: members.first().copied().unwrap_or_default(),
        title: title(&ranked),
        members,
    }
}

#[cfg(test)]
mod group_tests {
    //! Consecutive step rows form one group; anything else ends it.
    use super::*;

    #[test]
    fn consecutive_tool_rows_of_one_turn_form_a_group() {
        let rows: Vec<SegmentRow<'_>> = vec![
            ("You", Some(1), None),
            ("Tool call", Some(1), Some("grep")),
            ("Tool call", Some(1), Some("read")),
            ("Assistant", Some(1), None),
        ];
        let groups = groups(&rows);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].first, 1);
        assert_eq!(groups[0].members, vec![1, 2]);
        assert_eq!(groups[0].title, "Searched code and read files");
    }

    #[test]
    fn a_reply_or_a_new_turn_ends_the_group() {
        let rows: Vec<SegmentRow<'_>> = vec![
            ("Tool call", Some(1), Some("bash")),
            ("Assistant", Some(1), None),
            ("Tool call", Some(1), Some("read")),
            ("Tool call", Some(2), Some("write")),
        ];
        let groups = groups(&rows);
        assert_eq!(groups.len(), 3, "{groups:?}");
        assert_eq!(groups[0].members, vec![0]);
        assert_eq!(groups[1].members, vec![2]);
        assert_eq!(groups[2].members, vec![3]);
        assert_eq!(groups[0].title, "Ran commands");
        assert_eq!(groups[2].title, "Wrote files");
    }

    #[test]
    fn a_row_with_no_tool_name_is_not_grouped() {
        let rows: Vec<SegmentRow<'_>> = vec![("Tool call", Some(1), Some("read"))];
        assert_eq!(groups(&rows)[0].members, vec![0]);
        let rows: Vec<SegmentRow<'_>> = vec![("Tool call", Some(1), None)];
        assert!(groups(&rows).is_empty());
        // A reasoning row has no tool name, so it ends the group rather than joining it.
        let rows: Vec<SegmentRow<'_>> = vec![
            ("Tool call", Some(1), Some("read")),
            ("Assistant", Some(1), None),
            ("Tool call", Some(1), Some("edit")),
        ];
        assert_eq!(groups(&rows).len(), 2);
    }

    #[test]
    fn the_debug_form_states_the_member_count() {
        let group = Group {
            first: 3,
            members: vec![3, 4, 5],
            title: "PRIVATE-TITLE".into(),
        };
        let debug = format!("{group:?}");
        assert!(debug.contains("members: 3"), "{debug}");
    }
}

#[cfg(test)]
mod live_title_tests {
    //! A running group names its activity; a settled one summarizes it.
    use super::*;

    #[test]
    fn a_live_group_names_its_current_activity() {
        assert_eq!(live_label(Some("commands")), "Running commands");
        assert_eq!(live_label(Some("read")), "Reading files");
        assert_eq!(live_label(Some("tools")), "Calling tools");
        // No activity recorded yet is the reference's `thinking` default.
        assert_eq!(live_label(None), "Analyzing the request");
        assert_eq!(live_label(Some("schedule_create")), "Analyzing the request");
    }

    #[test]
    fn the_seat_title_prefers_the_live_label_and_falls_back_to_the_summary() {
        let ranked = counts(["grep", "read"].map(category));
        assert_eq!(
            seat_title(&ranked, Some("commands")),
            "Running commands",
            "a live group names what it is doing"
        );
        assert_eq!(
            seat_title(&ranked, None),
            "Searched code and read files",
            "a settled group states the summary"
        );
    }
}

#[cfg(test)]
mod live_detail_tests {
    //! The live detail a group title carries: the call's own first useful argument.
    use super::*;
    use serde_json::json;

    #[test]
    fn the_first_present_key_wins_and_whitespace_collapses() {
        assert_eq!(
            live_detail(&json!({"command": "printf  a\n  b"})).as_deref(),
            Some("printf a b")
        );
        // `title` is read before `command`, as the reference's key order does.
        assert_eq!(
            live_detail(&json!({"title": "PUBLIC title", "command": "printf x"})).as_deref(),
            Some("PUBLIC title")
        );
        // An array of strings joins with commas; a mixed array is skipped as the reference does.
        assert_eq!(
            live_detail(&json!({"queries": ["a", "b"]})).as_deref(),
            Some("a, b")
        );
        assert_eq!(live_detail(&json!({"queries": ["a", 1]})), None);
        assert_eq!(live_detail(&json!({"count": 3})), None);
        assert_eq!(live_detail(&json!({})), None);
        assert_eq!(live_detail(&json!({"command": "   "})), None);
    }

    #[test]
    fn a_long_detail_is_elided_at_the_reference_bound() {
        let long = "x".repeat(400);
        let detail = live_detail(&json!({"command": long})).unwrap();
        assert_eq!(detail.chars().count(), LIVE_DETAIL_MAX_CHARS);
        assert!(detail.ends_with('…'), "{detail}");
        // Exactly at the bound is kept whole.
        let exact = "y".repeat(LIVE_DETAIL_MAX_CHARS);
        assert_eq!(
            live_detail(&json!({"command": exact}))
                .unwrap()
                .chars()
                .count(),
            LIVE_DETAIL_MAX_CHARS
        );
    }
}
