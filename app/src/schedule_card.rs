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
