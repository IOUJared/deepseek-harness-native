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
