//! The created-task card for a `schedule_create` call (`schedule-create-card.ts:135-152`,
//! `ScheduleCreateCard.tsx:36-110`, `frequency-locales.ts`).
//!
//! The tool's result is its canonical task serialized as JSON, so the card reads the task's
//! identity and rule from that text. Every step narrows wire JSON: a running call, a failed
//! creation or a replayed result that is not a complete task renders the title and the raw result
//! text instead, so the card never presents an identity it does not have.
use serde_json::Value;

const MAX_TEXT_BYTES: usize = 512;

/// One complete task rule, narrowed from the result.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Task {
    pub title: String,
    /// The reference's localized frequency line, e.g. `Daily at 17:30 (UTC)`.
    pub frequency: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Card {
    /// The complete task, or `None` when the result carries none.
    pub task: Option<Task>,
    /// The title to state: the task's own, the call's arguments' title, or the tool's name.
    pub title: String,
    /// The raw result text, drawn only when no task could be narrowed.
    pub fallback: Option<String>,
}

impl Card {
    /// The rule line, when the result narrowed to a task.
    pub fn frequency(&self) -> Option<String> {
        self.task.as_ref().map(|task| task.frequency.clone())
    }
}

impl std::fmt::Debug for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Card")
            .field("title", &self.title)
            .field(
                "frequency",
                &self.task.as_ref().map(|task| task.frequency.as_str()),
            )
            .field("fallback_bytes", &self.fallback.as_ref().map(String::len))
            .finish()
    }
}

/// First physical line of an instruction, without surrounding whitespace.
fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("").trim()
}

/// `clockLabel`: `HH:MM` from a recorded `HH:MM(:SS)?`, unchanged when it is neither.
fn clock_label(time: &str) -> String {
    let mut parts = time.split(':');
    match (parts.next(), parts.next()) {
        (Some(hour), Some(minute)) if hour.len() == 2 && minute.len() == 2 => {
            format!("{hour}:{minute}")
        }
        _ => time.to_owned(),
    }
}

/// The reference's weekday names, indexed by ISO weekday.
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// `Every {value} {unit}` with the largest unit that divides the interval evenly
/// (`schedule-format.ts:285-300`).
fn every_line(seconds: u64) -> String {
    let units = [("days", 86_400u64), ("hours", 3_600), ("minutes", 60)];
    for (name, size) in units {
        if seconds >= size && seconds % size == 0 {
            return format!(
                "Every {} {}",
                seconds / size,
                if seconds / size == 1 {
                    name.trim_end_matches('s')
                } else {
                    name
                }
            );
        }
    }
    format!("Every {seconds} seconds")
}

/// The schedules page's status filter (`TaskManagerPage.tsx:124-136`, `task-manager-locales.ts:22-24`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    Enabled,
    Inactive,
}

impl Filter {
    /// The reference's tab labels, in its own order.
    pub const ALL: [Filter; 3] = [Filter::All, Filter::Enabled, Filter::Inactive];

    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Enabled => "Enabled",
            Filter::Inactive => "Inactive",
        }
    }

    /// Whether one row belongs to this filter. A status this build does not know counts as
    /// inactive, because only `active` runs.
    pub fn admits(self, status: &str) -> bool {
        let enabled = status.eq_ignore_ascii_case("active");
        match self {
            Filter::All => true,
            Filter::Enabled => enabled,
            Filter::Inactive => !enabled,
        }
    }

    /// Whether the page's search matches one row: a case-insensitive substring of its title or
    /// instruction, which is what the user can see and edit.
    pub fn matches(query: &str, title: &str, prompt: Option<&str>) -> bool {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return true;
        }
        title.to_lowercase().contains(&needle)
            || prompt.is_some_and(|prompt| prompt.to_lowercase().contains(&needle))
    }
}

/// The reference's localized rule for one recorded schedule row, read from the row's own timing
/// fields (`schedule-format.ts:249-300`, `frequency-locales.ts:75-81`). The card's `narrow` needs a
/// complete task; a row only needs its rule, so this shares the same wording without the identity.
pub fn frequency_from(
    record: &serde_json::Map<String, serde_json::Value>,
    kind: &str,
) -> Option<String> {
    let text = |key: &str| record.get(key).and_then(|value| value.as_str());
    let number = |key: &str| record.get(key).and_then(|value| value.as_u64());
    match kind {
        "after" | "at" => Some("Once".to_owned()),
        "every" => Some(every_line(
            number("everySeconds").filter(|seconds| *seconds > 0)?,
        )),
        "daily" => Some(format!(
            "Daily at {} ({})",
            clock_label(text("time")?),
            text("timeZone")?
        )),
        "weekly" => {
            let weekdays = record.get("weekdays")?.as_array()?;
            let mut names = Vec::with_capacity(weekdays.len());
            for day in weekdays {
                let day = day.as_u64().filter(|day| (1..=7).contains(day))?;
                names.push(WEEKDAYS[(day - 1) as usize]);
            }
            Some(format!(
                "Weekly on {} at {} ({})",
                names.join(", "),
                clock_label(text("time")?),
                text("timeZone")?
            ))
        }
        "cron" => Some(format!(
            "Cron {} ({})",
            crate::reducer::bounded(text("expression")?, 128),
            text("timeZone")?
        )),
        _ => None,
    }
}

/// Narrow one complete task from the result JSON, or `None`.
pub fn narrow(value: &Value) -> Option<Task> {
    let id = value["id"].as_str().filter(|id| !id.is_empty())?;
    let _ = id;
    let prompt = value["prompt"]
        .as_str()
        .filter(|prompt| !prompt.is_empty())?;
    value["scheduledAt"].as_str()?;
    let title = value["title"]
        .as_str()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| Some(first_line(prompt)).filter(|line| !line.is_empty()))?;
    let frequency = match value["kind"].as_str()? {
        "after" | "at" => "Once".to_owned(),
        "every" => every_line(
            value["everySeconds"]
                .as_u64()
                .filter(|seconds| *seconds > 0)?,
        ),
        "daily" => format!(
            "Daily at {} ({})",
            clock_label(value["time"].as_str()?),
            value["timeZone"].as_str()?
        ),
        "weekly" => {
            let weekdays = value["weekdays"].as_array()?;
            let mut names = Vec::with_capacity(weekdays.len());
            for day in weekdays {
                let day = day.as_u64().filter(|day| (1..=7).contains(day))?;
                names.push(WEEKDAYS[(day - 1) as usize]);
            }
            format!(
                "Weekly on {} at {} ({})",
                names.join(", "),
                clock_label(value["time"].as_str()?),
                value["timeZone"].as_str()?
            )
        }
        "cron" => format!(
            "Cron {} ({})",
            crate::reducer::bounded(value["expression"].as_str()?, 128),
            value["timeZone"].as_str()?
        ),
        _ => return None,
    };
    Some(Task {
        title: crate::reducer::bounded(title, MAX_TEXT_BYTES),
        frequency,
    })
}

/// The title a still-running call announces from its own arguments.
fn pending_title(args: &Value) -> Option<String> {
    args["title"]
        .as_str()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(|title| crate::reducer::bounded(title, MAX_TEXT_BYTES))
        .or_else(|| {
            Some(crate::reducer::bounded(
                first_line(args["prompt"].as_str()?),
                MAX_TEXT_BYTES,
            ))
            .filter(|line| !line.is_empty())
        })
}

/// Derive the card for one `schedule_create` call, with its result when it has settled.
pub fn derive(call: &Value, result: Option<&Value>) -> Option<Card> {
    if !matches!(
        call["name"].as_str(),
        Some("schedule_create" | "functions.schedule_create")
    ) {
        return None;
    }
    // A running call may carry no arguments yet; the card then states the tool's own name.
    let args: Value =
        serde_json::from_str(call["arguments"].as_str().unwrap_or("{}")).unwrap_or(Value::Null);
    let output = result.and_then(|result| {
        if result["message"]["isError"].as_bool() == Some(true) {
            return None;
        }
        let content = result["message"]["content"].as_array()?;
        if content.len() != 1 || content[0]["type"].as_str() != Some("text") {
            return None;
        }
        let text = content[0]["text"]
            .as_str()
            .filter(|text| !text.is_empty())?;
        Some(text.to_owned())
    });
    let task = output
        .as_deref()
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .and_then(|value| narrow(&value));
    let title = task
        .as_ref()
        .map(|task| task.title.clone())
        .or_else(|| pending_title(&args))
        .unwrap_or_else(|| "schedule_create".to_owned());
    // The fallback exists only for a settled result the card could not narrow; a running call has
    // no text to show and must still produce its card.
    let fallback = match (&task, output) {
        (None, Some(text)) => Some(crate::reducer::bounded(&text, MAX_TEXT_BYTES * 2)),
        _ => None,
    };
    Some(Card {
        fallback,
        task,
        title,
    })
}

/// Which catalog rows the open Session's header chip counts: its own session's still-active tasks.
///
/// The reference's header source reads `schedule/list({ sessionId })`, which the Host answers with
/// the Session's active tasks only (`session-schedule-state.ts:22-41`); the Sidebar marks project the
/// same filter onto the whole catalog (`session-schedule-state.ts:73-82`), which is the shape native
/// owns because `schedule/catalog` is its only schedule read.
pub fn is_open_reminder(
    item: &dsh_native_transport::dto::ScheduleCatalogItem,
    session: &dsh_native_transport::dto::SessionId,
) -> bool {
    &item.session_id == session && Filter::Enabled.admits(&item.status)
}

/// One Session's active reminders, ordered as the header popover lists them
/// (`ScheduleCatalogAction.tsx:70`, `schedule-format.ts:442-458`): an overdue target first, then
/// ascending next-run, keeping the catalog's own order where two targets are equal.
pub fn order_reminders<'a>(
    items: impl IntoIterator<Item = &'a dsh_native_transport::dto::ScheduleCatalogItem>,
    now_millis: i64,
) -> Vec<&'a dsh_native_transport::dto::ScheduleCatalogItem> {
    let mut rows: Vec<(bool, i64, &dsh_native_transport::dto::ScheduleCatalogItem)> = items
        .into_iter()
        .map(|item| {
            // A row the Host stamped with no readable target sorts after every dated one, as the
            // page's own ordering puts an undated task last.
            let at = item
                .scheduled_at
                .as_deref()
                .and_then(crate::ui::time_text::parse_instant);
            match at {
                Some(instant) => (instant <= now_millis, instant, item),
                None => (false, i64::MAX, item),
            }
        })
        .collect();
    rows.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    rows.into_iter().map(|(_, _, item)| item).collect()
}

/// The chip's count in the reference's two forms (`locales.ts:42-43` `trigger.one`/`trigger.other`):
/// the phrase states the number, so one reminder never reads "1 reminders".
pub fn reminder_count(count: usize) -> String {
    format!("{count} reminder{}", if count == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(args: Value) -> Value {
        json!({"name": "schedule_create", "arguments": args.to_string()})
    }
    fn result(task: Value) -> Value {
        json!({"message": {"isError": false, "content": [{"type": "text", "text": task.to_string()}]}})
    }
    fn at() -> Value {
        json!({"id": "s1", "title": "PUBLIC nightly report", "prompt": "PUBLIC instruction\nmore",
            "scheduledAt": "2026-08-31T17:30:00.000Z", "kind": "at"})
    }

    #[test]
    fn a_settled_task_states_its_title_and_frequency() {
        let card = derive(&call(json!({"prompt": "x"})), Some(&result(at()))).unwrap();
        let task = card.task.expect("a complete task");
        assert_eq!(task.title, "PUBLIC nightly report");
        assert_eq!(task.frequency, "Once");
        assert_eq!(card.title, "PUBLIC nightly report");
        assert_eq!(card.fallback, None);
    }

    #[test]
    fn every_kind_states_its_rule() {
        let cases = [
            (
                json!({"id":"s","prompt":"p","scheduledAt":"t","kind":"every","everySeconds":300}),
                "Every 5 minutes",
            ),
            (
                json!({"id":"s","prompt":"p","scheduledAt":"t","kind":"every","everySeconds":3600}),
                "Every 1 hour",
            ),
            (
                json!({"id":"s","prompt":"p","scheduledAt":"t","kind":"every","everySeconds":86400}),
                "Every 1 day",
            ),
            (
                json!({"id":"s","prompt":"p","scheduledAt":"t","kind":"every","everySeconds":90}),
                "Every 90 seconds",
            ),
            (
                json!({"id":"s","prompt":"p","scheduledAt":"t","kind":"daily","time":"17:30:00","timeZone":"UTC"}),
                "Daily at 17:30 (UTC)",
            ),
            (
                json!({"id":"s","prompt":"p","scheduledAt":"t","kind":"weekly","time":"09:05","timeZone":"Europe/Berlin","weekdays":[1,3,5]}),
                "Weekly on Mon, Wed, Fri at 09:05 (Europe/Berlin)",
            ),
            (
                json!({"id":"s","prompt":"p","scheduledAt":"t","kind":"cron","expression":"0 9 * * 1","timeZone":"UTC"}),
                "Cron 0 9 * * 1 (UTC)",
            ),
        ];
        for (task, expected) in cases {
            let card = derive(&call(json!({})), Some(&result(task.clone()))).unwrap();
            assert_eq!(card.task.expect("a task").frequency, expected, "{task}");
        }
    }

    #[test]
    fn an_incomplete_task_falls_back_to_the_raw_result_text() {
        for broken in [
            json!({"prompt": "p", "scheduledAt": "t", "kind": "at"}),
            json!({"id": "s", "prompt": "", "scheduledAt": "t", "kind": "at"}),
            json!({"id": "s", "prompt": "p", "scheduledAt": "t", "kind": "sometimes"}),
            json!({"id": "s", "prompt": "p", "scheduledAt": "t", "kind": "every"}),
            json!({"id": "s", "prompt": "p", "scheduledAt": "t", "kind": "weekly", "time": "09:00", "timeZone": "UTC", "weekdays": [9]}),
        ] {
            let card = derive(&call(json!({})), Some(&result(broken.clone()))).unwrap();
            assert!(card.task.is_none(), "{broken}");
            assert!(card.fallback.is_some(), "{broken}");
        }
        // A result that is not JSON at all is the raw text too.
        let card = derive(
            &call(json!({})),
            Some(&json!({"message": {"isError": false, "content": [{"type": "text", "text": "not json"}]}})),
        )
        .unwrap();
        assert_eq!(card.fallback.as_deref(), Some("not json"));
    }

    #[test]
    fn a_running_call_announces_its_own_title() {
        let card = derive(&call(json!({"prompt": "PUBLIC first line\nsecond"})), None).unwrap();
        assert!(card.task.is_none());
        assert_eq!(card.title, "PUBLIC first line");
        assert_eq!(card.fallback, None);
        // A call with no arguments at all states the tool's name.
        let card = derive(&json!({"name": "schedule_create", "arguments": ""}), None).unwrap();
        assert_eq!(card.title, "schedule_create");
        // Another tool has no card.
        assert!(derive(&json!({"name": "read", "arguments": "{}"}), None).is_none());
        // A failed creation states no task.
        let failed = json!({"message": {"isError": true, "content": []}});
        let card = derive(&call(json!({"title": "PUBLIC t"})), Some(&failed)).unwrap();
        assert!(card.task.is_none());
        assert_eq!(card.fallback, None);
    }

    #[test]
    fn the_debug_form_states_the_rule_not_the_instruction() {
        let card = derive(
            &call(json!({"prompt": "PRIVATE-INSTRUCTION"})),
            Some(&result(json!({"id":"s","prompt":"PRIVATE-INSTRUCTION","scheduledAt":"t","kind":"at","title":"PUBLIC name"}))),
        )
        .unwrap();
        let debug = format!("{card:?}");
        assert!(!debug.contains("PRIVATE"), "{debug}");
        assert!(debug.contains("PUBLIC name"), "{debug}");
    }
}

#[cfg(test)]
mod row_frequency_tests {
    //! A schedule row's rule line, read from the row's own timing fields.
    use super::*;
    use serde_json::json;

    fn record(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        value.as_object().cloned().expect("an object")
    }

    #[test]
    fn every_kind_states_its_rule() {
        let cases = [
            (json!({"kind": "at"}), "at", "Once"),
            (
                json!({"kind": "after", "afterSeconds": 60}),
                "after",
                "Once",
            ),
            (
                json!({"kind": "every", "everySeconds": 300}),
                "every",
                "Every 5 minutes",
            ),
            (
                json!({"kind": "daily", "time": "17:30:00", "timeZone": "UTC"}),
                "daily",
                "Daily at 17:30 (UTC)",
            ),
            (
                json!({"kind": "weekly", "time": "09:00", "timeZone": "Europe/Berlin", "weekdays": [1, 5]}),
                "weekly",
                "Weekly on Mon, Fri at 09:00 (Europe/Berlin)",
            ),
            (
                json!({"kind": "cron", "expression": "0 9 * * 1", "timeZone": "UTC"}),
                "cron",
                "Cron 0 9 * * 1 (UTC)",
            ),
        ];
        for (fields, kind, expected) in cases {
            let mut value = fields;
            value["kind"] = serde_json::Value::String(kind.into());
            assert_eq!(
                frequency_from(&record(value), kind).as_deref(),
                Some(expected),
                "{kind}"
            );
        }
    }

    #[test]
    fn an_incomplete_rule_states_nothing_rather_than_guessing() {
        assert_eq!(
            frequency_from(&record(json!({"kind": "daily"})), "daily"),
            None
        );
        assert_eq!(
            frequency_from(&record(json!({"kind": "every"})), "every"),
            None
        );
        assert_eq!(frequency_from(&record(json!({})), "sometimes"), None);
    }
}

#[cfg(test)]
mod filter_tests {
    //! The schedules page's tabs and search, as the reference applies them.
    use super::*;

    #[test]
    fn the_tabs_admit_the_statuses_they_name() {
        assert!(Filter::All.admits("active"));
        assert!(Filter::All.admits("inactive"));
        assert!(Filter::Enabled.admits("active"));
        assert!(!Filter::Enabled.admits("inactive"));
        assert!(Filter::Inactive.admits("inactive"));
        assert!(!Filter::Inactive.admits("active"));
        // Only `active` runs, so a status this build does not know counts as inactive.
        assert!(Filter::Inactive.admits("ended"));
        assert_eq!(
            Filter::ALL.map(Filter::label),
            ["All", "Enabled", "Inactive"]
        );
    }

    #[test]
    fn the_search_matches_a_title_or_instruction_case_insensitively() {
        assert!(Filter::matches("", "PUBLIC", Some("x")));
        assert!(Filter::matches("  ", "PUBLIC", None));
        assert!(Filter::matches("public", "PUBLIC reminder", None));
        assert!(Filter::matches("REMINDER", "PUBLIC reminder", None));
        assert!(Filter::matches(
            "instruction",
            "PUBLIC",
            Some("The INSTRUCTION text")
        ));
        assert!(!Filter::matches(
            "nightly",
            "PUBLIC reminder",
            Some("daily text")
        ));
        assert!(!Filter::matches("nightly", "PUBLIC", None));
    }

    fn reminder(
        id: &str,
        session: &str,
        status: &str,
        at: Option<&str>,
    ) -> dsh_native_transport::dto::ScheduleCatalogItem {
        dsh_native_transport::dto::ScheduleCatalogItem {
            id: id.to_owned(),
            title: format!("PUBLIC {id}"),
            kind: "daily".to_owned(),
            session_id: dsh_native_transport::dto::SessionId::new(session).expect("a session id"),
            status: status.to_owned(),
            scheduled_at: at.map(str::to_owned),
            last_delivery: None,
            record: serde_json::Map::new(),
        }
    }

    #[test]
    fn the_header_chip_counts_only_the_open_sessions_active_reminders() {
        let open = dsh_native_transport::dto::SessionId::new("PUBLIC-open").expect("a session id");
        // Same session, still active: counted.
        let active = reminder("a", "PUBLIC-open", "active", Some("2026-10-17T09:00:00Z"));
        // Same session but inactive: the reference's list admits active tasks only.
        let inactive = reminder("b", "PUBLIC-open", "inactive", None);
        // Another session's live task: the page lists it, this Session's chip must not count it.
        let elsewhere = reminder("c", "PUBLIC-elsewhere", "active", None);
        assert!(is_open_reminder(&active, &open));
        assert!(!is_open_reminder(&inactive, &open));
        assert!(!is_open_reminder(&elsewhere, &open));
        // The status comparison is the page's own: `ACTIVE` still runs.
        let upper = reminder("d", "PUBLIC-open", "ACTIVE", None);
        assert!(is_open_reminder(&upper, &open));
    }

    #[test]
    fn an_overdue_reminder_is_listed_before_a_later_one() {
        let now =
            crate::ui::time_text::parse_instant("2026-10-17T12:00:00Z").expect("a readable now");
        let overdue = reminder(
            "overdue",
            "PUBLIC-open",
            "active",
            Some("2026-10-17T09:00:00Z"),
        );
        let soon = reminder(
            "soon",
            "PUBLIC-open",
            "active",
            Some("2026-10-17T13:00:00Z"),
        );
        let later = reminder(
            "later",
            "PUBLIC-open",
            "active",
            Some("2026-10-17T14:00:00Z"),
        );
        let undated = reminder("undated", "PUBLIC-open", "active", None);
        // The overdue target leads even though another row's target is later than it: the reference
        // orders overdue first, then ascending (`schedule-format.ts:448-457`).
        let ordered = order_reminders([&soon, &undated, &later, &overdue], now);
        let ids: Vec<&str> = ordered.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["overdue", "soon", "later", "undated"]);
        // Two equal targets keep the catalog's own order rather than reversing or interleaving.
        let left = reminder(
            "left",
            "PUBLIC-open",
            "active",
            Some("2026-10-17T13:00:00Z"),
        );
        let right = reminder(
            "right",
            "PUBLIC-open",
            "active",
            Some("2026-10-17T13:00:00Z"),
        );
        let tied = order_reminders([&left, &right], now);
        assert_eq!(
            tied.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(),
            ["left", "right"]
        );
    }

    #[test]
    fn one_reminder_says_one_and_two_say_two() {
        assert_eq!(reminder_count(1), "1 reminder");
        assert_eq!(reminder_count(2), "2 reminders");
        // The label states its own number, so zero never borrows the singular form.
        assert_eq!(reminder_count(0), "0 reminders");
    }
}
