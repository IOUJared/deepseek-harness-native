//! The `web_search`/`web_fetch` card, derived the way the reference derives it
//! (`web-card-model.ts:60-99`, `WebBlock.tsx:145-190`).
//!
//! A web result carries its shape in `meta` too: a search reports an optional provider answer plus
//! the cited sources and whether the list was capped; a fetch reports the URL it read, the HTTP
//! status, and whether the content was capped. Every field is validated with the reference's own
//! rules — a malformed source drops the whole card rather than being repaired — and a result that
//! fails one keeps the recorded text body.
use serde_json::Value;

/// Sources a chat card draws before the list folds. The reference's list scrolls in place; a chat
/// row has a fixed reservation, so the fold is the native translation of that scroll.
pub const MAX_ROWS: usize = 8;
/// Per-field bounds. The provider caps its own strings; a forged record must not grow a card.
const MAX_TEXT_BYTES: usize = 1024;
const MAX_SOURCES: usize = 128;

#[derive(Clone, PartialEq, Eq)]
pub struct Source {
    pub url: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Kind {
    /// A `web_search`: the provider's answer, when it gave one, over its cited sources.
    Search {
        answer: Option<String>,
        sources: Vec<Source>,
    },
    /// A `web_fetch`: the URL read and the HTTP status it answered with.
    Fetch { url: String, status_code: u32 },
}

#[derive(Clone, PartialEq, Eq)]
pub struct Card {
    pub kind: Kind,
    /// The list or the content was capped, so the card says so (`web.sourcesTruncated` /
    /// `web.contentTruncated`).
    pub truncated: bool,
}

/// One drawn row: the answer, a numbered source, the empty state, or the fetch line.
#[derive(Clone, PartialEq, Eq)]
pub enum Row {
    Answer(String),
    Source { ordinal: usize, source: Source },
    Empty,
    Fetch { url: String, status_code: u32 },
}

impl Card {
    pub fn rows(&self) -> Vec<Row> {
        match &self.kind {
            Kind::Search { answer, sources } => {
                let mut rows = Vec::new();
                // The reference treats an empty answer exactly like an absent one
                // (`WebBlock.tsx:154-161`), so both fall through to the empty state.
                let answered = answer.as_ref().filter(|answer| !answer.is_empty());
                if let Some(answer) = answered {
                    rows.push(Row::Answer(answer.clone()));
                }
                if sources.is_empty() && answered.is_none() {
                    rows.push(Row::Empty);
                }
                for (index, source) in sources.iter().enumerate() {
                    rows.push(Row::Source {
                        ordinal: index + 1,
                        source: source.clone(),
                    });
                }
                rows
            }
            Kind::Fetch { url, status_code } => vec![Row::Fetch {
                url: url.clone(),
                status_code: *status_code,
            }],
        }
    }
    /// The reference's truncation note for this shape, or the empty state's copy.
    pub fn note(&self) -> &'static str {
        if !self.truncated {
            return "";
        }
        match self.kind {
            Kind::Search { .. } => "Source list truncated",
            Kind::Fetch { .. } => "Content truncated",
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
        shown.head.len()
            + shown.tail.len()
            + usize::from(shown.hidden > 0)
            + usize::from(self.truncated)
    }
}

/// Answer, snippets and titles are page content, so the debug form prints counts only.
impl std::fmt::Debug for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            Kind::Search { answer, sources } => f
                .debug_struct("Search")
                .field("answer_bytes", &answer.as_ref().map(String::len))
                .field("sources", &sources.len())
                .field("truncated", &self.truncated)
                .finish(),
            Kind::Fetch { status_code, .. } => f
                .debug_struct("Fetch")
                .field("status_code", status_code)
                .field("truncated", &self.truncated)
                .finish(),
        }
    }
}

impl std::fmt::Debug for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Row::Answer(text) => f
                .debug_tuple("Answer")
                .field(&format_args!("{} bytes", text.len()))
                .finish(),
            Row::Source { ordinal, source } => f
                .debug_struct("Source")
                .field("ordinal", ordinal)
                .field("url_bytes", &source.url.len())
                .finish(),
            Row::Empty => f.write_str("Empty"),
            Row::Fetch { status_code, .. } => f
                .debug_struct("Fetch")
                .field("status_code", status_code)
                .finish(),
        }
    }
}

pub struct Shown {
    pub head: Vec<Row>,
    pub tail: Vec<Row>,
    pub hidden: u64,
}

fn sources_of(value: &Value) -> Option<Vec<Source>> {
    let list = value.as_array()?;
    if list.len() > MAX_SOURCES {
        return None;
    }
    let mut sources = Vec::with_capacity(list.len());
    for entry in list {
        let url = entry["url"].as_str()?.to_owned();
        let optional = |key: &str| -> Option<Option<String>> {
            match entry.get(key) {
                None | Some(Value::Null) => Some(None),
                Some(value) => value
                    .as_str()
                    .map(|text| Some(crate::reducer::bounded(text, MAX_TEXT_BYTES))),
            }
        };
        sources.push(Source {
            url: crate::reducer::bounded(&url, MAX_TEXT_BYTES),
            title: optional("title")?,
            snippet: optional("snippet")?,
            published_at: optional("publishedAt")?,
        });
    }
    Some(sources)
}

/// Derive the web card for a settled `web_search`/`web_fetch` call, when every rule holds.
pub fn derive(call: &Value, result: &Value) -> Option<Card> {
    if result["message"]["isError"].as_bool() == Some(true) {
        return None;
    }
    let tool = match call["name"].as_str()? {
        "web_search" | "functions.web_search" | "tool-web/web_search" => "search",
        "web_fetch" | "functions.web_fetch" | "tool-web/web_fetch" => "fetch",
        _ => return None,
    };
    let args: Value = serde_json::from_str(call["arguments"].as_str()?).ok()?;
    let meta = &result["meta"];
    let truncated = meta["truncated"].as_bool()?;
    let kind = match tool {
        "search" => {
            let queries = args["queries"].as_array()?;
            if queries.is_empty() {
                return None;
            }
            if !queries
                .iter()
                .all(|query| query.as_str().is_some_and(|query| !query.trim().is_empty()))
            {
                return None;
            }
            let answer = match meta.get("answer") {
                None | Some(Value::Null) => None,
                Some(value) => Some(crate::reducer::bounded(value.as_str()?, MAX_TEXT_BYTES * 2)),
            };
            Kind::Search {
                answer,
                sources: sources_of(&meta["sources"])?,
            }
        }
        _ => {
            if !args["url"]
                .as_str()
                .is_some_and(|url| !url.trim().is_empty())
            {
                return None;
            }
            let url = meta["url"].as_str()?;
            let status_code = u32::try_from(meta["statusCode"].as_i64()?).ok()?;
            Kind::Fetch {
                url: crate::reducer::bounded(url, MAX_TEXT_BYTES),
                status_code,
            }
        }
    };
    Some(Card { kind, truncated })
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
    fn search_meta() -> Value {
        json!({"truncated": false,
        "answer": "PUBLIC provider answer",
        "sources": [
            {"url": "https://PUBLIC.example/a", "title": "PUBLIC A", "snippet": "PUBLIC snippet A"},
            {"url": "https://PUBLIC.example/b", "publishedAt": "PUBLIC 2026-01-01"}
        ]})
    }

    #[test]
    fn a_search_card_answers_over_its_sources() {
        let card = derive(
            &call("web_search", json!({"queries": ["PUBLIC query"]})),
            &result(search_meta()),
        )
        .expect("a settled search");
        assert_eq!(card.rows().len(), 3);
        assert_eq!(card.rows()[0], Row::Answer("PUBLIC provider answer".into()));
        assert!(
            matches!(&card.rows()[1], Row::Source { ordinal: 1, source } if source.url == "https://PUBLIC.example/a")
        );
        assert!(
            matches!(&card.rows()[2], Row::Source { ordinal: 2, source } if source.published_at.is_some())
        );
        assert_eq!(card.note(), "");
    }

    #[test]
    fn a_search_without_an_answer_or_sources_says_no_results() {
        let card = derive(
            &call("web_search", json!({"queries": ["PUBLIC query"]})),
            &result(json!({"truncated": false, "sources": []})),
        )
        .unwrap();
        assert_eq!(card.rows(), vec![Row::Empty]);
        // An empty answer string is no answer, not an empty paragraph.
        let card = derive(
            &call("web_search", json!({"queries": ["PUBLIC query"]})),
            &result(json!({"truncated": false, "answer": "", "sources": []})),
        )
        .unwrap();
        assert_eq!(card.rows(), vec![Row::Empty]);
    }

    #[test]
    fn a_capped_search_and_a_capped_fetch_state_it() {
        let mut meta = search_meta();
        meta["truncated"] = json!(true);
        let card = derive(
            &call("web_search", json!({"queries": ["PUBLIC query"]})),
            &result(meta),
        )
        .unwrap();
        assert_eq!(card.note(), "Source list truncated");
        assert_eq!(card.drawn_rows(false), 4);
        let card = derive(
            &call("web_fetch", json!({"url": "https://PUBLIC.example/a"})),
            &result(
                json!({"truncated": true, "url": "https://PUBLIC.example/a", "statusCode": 200}),
            ),
        )
        .unwrap();
        assert_eq!(card.note(), "Content truncated");
        assert_eq!(
            card.rows(),
            vec![Row::Fetch {
                url: "https://PUBLIC.example/a".into(),
                status_code: 200
            }]
        );
    }

    #[test]
    fn the_cap_keeps_the_head_and_the_tail() {
        let sources: Vec<Value> = (1..=12)
            .map(|n| json!({"url": format!("https://PUBLIC.example/{n}")}))
            .collect();
        let card = derive(
            &call("web_search", json!({"queries": ["PUBLIC query"]})),
            &result(json!({"truncated": false, "sources": sources})),
        )
        .unwrap();
        let shown = card.shown(false);
        assert_eq!(shown.head.len(), 4);
        assert_eq!(shown.tail.len(), 4);
        assert_eq!(shown.hidden, 4);
        assert!(matches!(&shown.head[0], Row::Source { ordinal: 1, .. }));
        assert!(matches!(&shown.tail[3], Row::Source { ordinal: 12, .. }));
        assert_eq!(card.drawn_rows(true), 12);
    }

    #[test]
    fn metadata_that_fails_a_rule_keeps_the_text_body() {
        let cases = [
            json!({"sources": []}),
            json!({"truncated": "no", "sources": []}),
            json!({"truncated": false, "sources": [{"url": 4}]}),
            json!({"truncated": false, "sources": [{"url": "u", "title": 4}]}),
            json!({"truncated": false, "sources": [], "answer": 4}),
            json!({"truncated": false, "url": "u"}),
            json!({"truncated": false, "url": "u", "statusCode": "200"}),
        ];
        for meta in cases {
            let card = derive(
                &call("web_search", json!({"queries": ["q"]})),
                &result(meta.clone()),
            );
            assert!(card.is_none(), "{meta}");
        }
        // A fetch needs its own shape.
        assert!(
            derive(
                &call("web_fetch", json!({"url": "u"})),
                &result(json!({"truncated": false, "sources": []}))
            )
            .is_none()
        );
    }

    #[test]
    fn a_call_that_fails_a_rule_or_an_error_result_keeps_the_text_body() {
        let ok = result(search_meta());
        for (call, expected) in [
            (json!({"name": "functions.read", "arguments": "{}"}), false),
            (call("web_search", json!({"queries": []})), false),
            (call("web_search", json!({"queries": ["  "]})), false),
            (call("web_search", json!({"queries": [4]})), false),
            (call("web_fetch", json!({"url": "  "})), false),
            (call("web_fetch", json!({})), false),
            (call("web_search", json!({"queries": ["a", "b"]})), true),
        ] {
            assert_eq!(derive(&call, &ok).is_some(), expected, "{call}");
        }
        let error = json!({"message": {"isError": true, "content": []}, "meta": search_meta()});
        assert!(derive(&call("web_search", json!({"queries": ["q"]})), &error).is_none());
    }

    #[test]
    fn the_debug_form_never_prints_content() {
        let card = derive(
            &call("web_search", json!({"queries": ["PUBLIC query"]})),
            &result(search_meta()),
        )
        .unwrap();
        let debug = format!("{card:?}");
        assert!(!debug.contains("PUBLIC"), "{debug}");
        assert!(debug.contains("sources: 2"), "{debug}");
        let rows = format!("{:?}", card.rows());
        assert!(!rows.contains("PUBLIC"), "{rows}");
    }
}
