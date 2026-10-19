//! The `ask_user_question` transcript card, derived the way the reference derives it
//! (`ask-question-card-model.ts`, `ask-question-row.tsx:119-180`, `AskQuestionCard.tsx:11-39`).
//!
//! A question call records its questions in the arguments and the answers in the result text, both
//! as JSON. The card pairs them by the echoed stable ids — a batch that cannot be paired
//! unambiguously gets no card at all, because a dropped or blank answer would read as a choice the
//! user made. A call the user dismissed or a turn that interrupted the question records no answers;
//! those name their verdict and list the questions instead.
use serde_json::Value;

/// Question/answer rows a chat card draws before its middle folds.
pub const MAX_ROWS: usize = 8;
const MAX_TEXT_BYTES: usize = 512;
const MAX_QUESTIONS: usize = 32;
const MAX_ANSWERS: usize = 32;

#[derive(Clone, PartialEq, Eq)]
pub struct Question {
    pub id: String,
    pub text: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Answered {
    pub id: String,
    pub question: String,
    /// The chosen option labels, then the free-form answer when the user typed one.
    pub answers: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Kind {
    /// The batch settled with answers; each question carries what was chosen.
    Answered(Vec<Answered>),
    /// The question never took answers: dismissed, interrupted, or closed without one.
    Unanswered {
        verdict: &'static str,
        questions: Vec<Question>,
    },
}

#[derive(Clone, PartialEq, Eq)]
pub struct Card {
    pub kind: Kind,
    /// Questions that carry at least one answer.
    pub answered: usize,
    pub total: usize,
}

/// One drawn row: a question, one of its answers, the not-answered marker, or the verdict.
#[derive(Clone, PartialEq, Eq)]
pub enum Row {
    Verdict(&'static str),
    Question(String),
    Answer(String),
    Skipped,
}

impl Card {
    pub fn rows(&self) -> Vec<Row> {
        match &self.kind {
            Kind::Answered(questions) => {
                let mut rows = Vec::new();
                for entry in questions {
                    rows.push(Row::Question(entry.question.clone()));
                    if entry.answers.is_empty() {
                        rows.push(Row::Skipped);
                    } else {
                        for answer in &entry.answers {
                            rows.push(Row::Answer(answer.clone()));
                        }
                    }
                }
                rows
            }
            Kind::Unanswered { verdict, questions } => {
                let mut rows = vec![Row::Verdict(verdict)];
                for entry in questions {
                    rows.push(Row::Question(entry.text.clone()));
                }
                rows
            }
        }
    }
    /// The reference's row summary for a settled batch (`locales.ts:678`).
    pub fn summary(&self) -> String {
        match &self.kind {
            Kind::Answered(_) => format!("{}/{} answered", self.answered, self.total),
            Kind::Unanswered { verdict, .. } => (*verdict).to_owned(),
        }
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

/// Questions and answers are the user's own words, so the debug form prints counts only.
impl std::fmt::Debug for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Row::Verdict(verdict) => f.debug_tuple("Verdict").field(verdict).finish(),
            Row::Question(text) => f
                .debug_tuple("Question")
                .field(&format_args!("{} bytes", text.len()))
                .finish(),
            Row::Answer(text) => f
                .debug_tuple("Answer")
                .field(&format_args!("{} bytes", text.len()))
                .finish(),
            Row::Skipped => f.write_str("Skipped"),
        }
    }
}

impl std::fmt::Debug for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Card")
            .field("answered", &self.answered)
            .field("total", &self.total)
            .field("rows", &self.rows().len())
            .finish()
    }
}

pub struct Shown {
    pub head: Vec<Row>,
    pub tail: Vec<Row>,
    pub hidden: u64,
}

/// The reference's `questionEntries`: unique string ids, a string question, optional header,
/// detail and options. Options are validated but never drawn here.
fn questions_of(raw: &str) -> Option<Vec<Question>> {
    let parsed: Value = serde_json::from_str(raw).ok()?;
    let list = parsed["questions"].as_array()?;
    if list.is_empty() || list.len() > MAX_QUESTIONS {
        return None;
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut questions = Vec::with_capacity(list.len());
    for entry in list {
        let id = entry["id"].as_str()?.to_owned();
        if !seen.insert(id.clone()) {
            return None;
        }
        let text = entry["question"].as_str()?.to_owned();
        for key in ["header", "detail"] {
            if entry.get(key).is_some_and(|value| value.as_str().is_none()) {
                return None;
            }
        }
        if entry.get("options").is_some_and(|value| !value.is_array()) {
            return None;
        }
        if entry
            .get("multi_select")
            .is_some_and(|value| value.as_bool().is_none())
        {
            return None;
        }
        questions.push(Question {
            id,
            text: crate::reducer::bounded(&text, MAX_TEXT_BYTES),
        });
    }
    Some(questions)
}

/// One recorded answer entry (`AskUserQuestionAnswerItem`): selected labels plus an optional
/// free-form answer.
fn answers_of(text: &str) -> Option<Vec<(String, Vec<String>)>> {
    let parsed: Value = serde_json::from_str(text).ok()?;
    let list = parsed["answers"].as_array()?;
    if list.len() > MAX_ANSWERS {
        return None;
    }
    let mut out = Vec::with_capacity(list.len());
    for entry in list {
        let id = entry["id"].as_str()?.to_owned();
        let selected = entry["selected"].as_array()?;
        let mut chosen = Vec::with_capacity(selected.len());
        for value in selected {
            chosen.push(crate::reducer::bounded(value.as_str()?, MAX_TEXT_BYTES));
        }
        match entry.get("custom") {
            None | Some(Value::Null) => {}
            Some(value) => {
                let custom = value.as_str()?;
                if !custom.is_empty() {
                    chosen.push(crate::reducer::bounded(custom, MAX_TEXT_BYTES));
                }
            }
        }
        out.push((id, chosen));
    }
    Some(out)
}

/// The error code a settled question call records, when the Host carried one.
fn error_code(result: &Value) -> Option<&str> {
    result["error"]["code"]
        .as_str()
        .or_else(|| result["message"]["error"]["code"].as_str())
}

/// Derive the transcript card for a settled `ask_user_question` call, when every rule holds.
pub fn derive(call: &Value, result: &Value) -> Option<Card> {
    if !matches!(
        call["name"].as_str(),
        Some("ask_user_question" | "functions.ask_user_question")
    ) {
        return None;
    }
    let questions = questions_of(call["arguments"].as_str()?)?;
    if result["message"]["isError"].as_bool() == Some(true) {
        // A dismissed set and an interrupted turn each name their own verdict; every other failure
        // keeps the recorded text, because the row is the call's error, not a transcript.
        let verdict = match error_code(result)? {
            "ASK_CANCELLED" => "cancelled",
            "ASK_ABORTED" => "interrupted",
            _ => return None,
        };
        return Some(Card {
            answered: 0,
            total: questions.len(),
            kind: Kind::Unanswered { verdict, questions },
        });
    }
    let text = super::content(&result["message"]["content"]);
    let answers = answers_of(&text)?;
    // Pair by id, exactly: a different count, a repeated id, or an id that was never asked makes
    // the batch ambiguous, and a partial pairing would show answers to the wrong questions.
    if answers.len() != questions.len() {
        return None;
    }
    let mut answered = Vec::with_capacity(questions.len());
    let mut count = 0;
    for question in &questions {
        let entry = answers
            .iter()
            .find(|(id, _)| *id == question.id)
            .filter(|_| answers.iter().filter(|(id, _)| *id == question.id).count() == 1)?;
        if !entry.1.is_empty() {
            count += 1;
        }
        answered.push(Answered {
            id: question.id.clone(),
            question: question.text.clone(),
            answers: entry.1.clone(),
        });
    }
    Some(Card {
        kind: Kind::Answered(answered),
        answered: count,
        total: questions.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(questions: Value) -> Value {
        json!({"name": "ask_user_question", "arguments": json!({"questions": questions}).to_string()})
    }
    fn result(answers: Value) -> Value {
        json!({"message": {"isError": false,
            "content": [{"type": "text", "text": json!({"answers": answers}).to_string()}]}})
    }
    fn questions() -> Value {
        json!([
            {"id": "q1", "question": "PUBLIC first question", "options": [{"label": "A"}, {"label": "B"}]},
            {"id": "q2", "question": "PUBLIC second question", "multi_select": true}
        ])
    }

    #[test]
    fn an_answered_batch_pairs_its_questions_with_what_was_chosen() {
        let card = derive(
            &call(questions()),
            &result(json!([
                {"id": "q1", "selected": ["A"], "custom": "PUBLIC typed note"},
                {"id": "q2", "selected": []}
            ])),
        )
        .expect("a settled question call");
        assert_eq!(card.summary(), "1/2 answered");
        assert_eq!(
            card.rows(),
            vec![
                Row::Question("PUBLIC first question".into()),
                Row::Answer("A".into()),
                Row::Answer("PUBLIC typed note".into()),
                Row::Question("PUBLIC second question".into()),
                Row::Skipped,
            ]
        );
    }

    #[test]
    fn an_empty_custom_answer_is_not_an_answer() {
        let card = derive(
            &call(questions()),
            &result(json!([
                {"id": "q1", "selected": [], "custom": ""},
                {"id": "q2", "selected": ["X"]}
            ])),
        )
        .unwrap();
        assert_eq!(card.summary(), "1/2 answered");
        assert_eq!(card.rows()[1], Row::Skipped);
    }

    #[test]
    fn a_dismissed_or_interrupted_question_names_its_verdict() {
        let error = |code: &str| json!({"message": {"isError": true, "content": []}, "error": {"code": code}});
        let card = derive(&call(questions()), &error("ASK_CANCELLED")).unwrap();
        assert_eq!(card.summary(), "cancelled");
        assert_eq!(
            card.rows(),
            vec![
                Row::Verdict("cancelled"),
                Row::Question("PUBLIC first question".into()),
                Row::Question("PUBLIC second question".into()),
            ]
        );
        let card = derive(&call(questions()), &error("ASK_ABORTED")).unwrap();
        assert_eq!(card.summary(), "interrupted");
        // Any other failure is the call's error, not a transcript.
        assert!(derive(&call(questions()), &error("ASK_UNAVAILABLE")).is_none());
    }

    #[test]
    fn an_ambiguous_batch_gets_no_card() {
        for answers in [
            // Fewer answers than questions, a repeated id, and an id never asked.
            json!([{"id": "q1", "selected": ["A"]}]),
            json!([{"id": "q1", "selected": ["A"]}, {"id": "q1", "selected": ["B"]}]),
            json!([{"id": "q1", "selected": ["A"]}, {"id": "q3", "selected": ["B"]}]),
        ] {
            assert!(
                derive(&call(questions()), &result(answers.clone())).is_none(),
                "{answers}"
            );
        }
    }

    #[test]
    fn a_call_that_fails_a_rule_or_an_unparsable_result_keeps_the_text_body() {
        let ok = result(json!([{"id": "q1", "selected": []}, {"id": "q2", "selected": []}]));
        for bad in [
            call(json!([])),
            call(json!([{"id": "q1"}])),
            call(json!([{"id": "q1", "question": "a"}, {"id": "q1", "question": "b"}])),
            call(json!([{"id": "q1", "question": "a", "options": {}}])),
            call(json!([{"id": "q1", "question": "a", "multi_select": "yes"}])),
        ] {
            assert!(derive(&bad, &ok).is_none(), "{bad}");
        }
        assert!(derive(&json!({"name": "read", "arguments": "{}"}), &ok).is_none());
        assert!(
            derive(
                &call(questions()),
                &json!({"message": {"isError": false, "content": [{"type": "text", "text": "not json"}]}})
            )
            .is_none()
        );
    }

    #[test]
    fn the_cap_keeps_the_head_and_the_tail() {
        let questions: Vec<Value> = (1..=10)
            .map(|n| json!({"id": format!("q{n}"), "question": format!("question {n}")}))
            .collect();
        let answers: Vec<Value> = (1..=10)
            .map(|n| json!({"id": format!("q{n}"), "selected": [format!("answer {n}")]}))
            .collect();
        let card = derive(&call(json!(questions)), &result(json!(answers))).unwrap();
        assert_eq!(card.rows().len(), 20);
        let shown = card.shown(false);
        assert_eq!(shown.head.len(), 4);
        assert_eq!(shown.tail.len(), 4);
        assert_eq!(shown.hidden, 12);
        assert_eq!(card.drawn_rows(true), 20);
    }

    #[test]
    fn the_debug_form_never_prints_the_user_words() {
        let card = derive(
            &call(questions()),
            &result(json!([
                {"id": "q1", "selected": ["PRIVATE-OPTION"]},
                {"id": "q2", "selected": []}
            ])),
        )
        .unwrap();
        let debug = format!("{card:?} {:?}", card.rows());
        assert!(!debug.contains("PRIVATE"), "{debug}");
        assert!(!debug.contains("PUBLIC first question"), "{debug}");
    }
}
