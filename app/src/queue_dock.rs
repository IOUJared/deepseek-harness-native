//! The composer's queue strip, rebuilt from the session's own `agent/inbox/spliced` records.
//!
//! The reference reads the `inbox` projection (`QueueDock.tsx:109-127`); native retains the
//! events and applies the same splice shape here, bounded and without trusting malformed
//! content. This first slice is read-only: previews and a count, no edit/remove/steer.
use dsh_native_transport::dto::SessionWireEvent;
use serde_json::Value;

/// The most rows a dock will show; a longer queue is truncated with a trailing count.
pub const MAX_ITEMS: usize = 16;
/// Preview characters per row, the reference's `QUEUE_PREVIEW_CHARS`.
pub const PREVIEW_CHARS: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub preview: String,
}

/// One wire content array flattened to the reference's preview form. Non-text blocks become
/// bracketed placeholders so a file-only row still reads as queued.
fn preview(content: Option<&Value>) -> String {
    let mut parts = Vec::new();
    for block in content.and_then(Value::as_array).into_iter().flatten() {
        let Some(kind) = block.get("type").and_then(Value::as_str) else {
            continue;
        };
        match kind {
            "text" => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    parts.push(text.to_owned());
                }
            }
            other => parts.push(format!("[{other}]")),
        }
    }
    let flat = parts.join(" ");
    let flat = flat.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = flat.chars();
    let preview: String = chars.by_ref().take(PREVIEW_CHARS).collect();
    if chars.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

fn item(id: Option<&str>, content: Option<&Value>, seq: u64, index: usize) -> Item {
    Item {
        id: id
            .filter(|id| !id.is_empty() && id.len() <= 512)
            .map_or_else(|| format!("queued-{seq}-{index}"), str::to_owned),
        preview: preview(content),
    }
}

/// Apply one splice to the next-turn list. `start` is clamped to the list; `removedCount`
/// removes at most what is there, then `inserted` goes in at `start`.
fn apply(items: &mut Vec<Item>, data: &Value, seq: u64) {
    if data.get("target").and_then(Value::as_str) != Some("next-turn") {
        return;
    }
    let start = data
        .get("start")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(items.len() as u64) as usize;
    let removed = data
        .get("removedCount")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min((items.len() - start) as u64) as usize;
    items.drain(start..start + removed);
    for (index, row) in data
        .get("inserted")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        items.insert(
            (start + index).min(items.len()),
            item(
                row.get("id").and_then(Value::as_str),
                row.get("content"),
                seq,
                index,
            ),
        );
    }
    items.truncate(MAX_ITEMS);
}

/// One inbox projection's `next-turn` array, the reference's own source
/// (`QueueDock.tsx:110-112`). Missing or malformed entries are skipped.
pub fn from_rows(rows: &[Value]) -> Vec<Item> {
    rows.iter()
        .take(MAX_ITEMS)
        .enumerate()
        .map(|(index, row)| {
            item(
                row.get("id").and_then(Value::as_str),
                row.get("content"),
                0,
                index,
            )
        })
        .collect()
}

/// The current next-turn queue from retained inbox records, in wire order.
pub fn from_events<'a>(events: impl IntoIterator<Item = &'a SessionWireEvent>) -> Vec<Item> {
    let mut items = Vec::new();
    for event in events {
        if event.event_type == "agent/inbox/spliced" {
            apply(&mut items, &event.data, event.seq);
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(seq: u64, data: Value) -> SessionWireEvent {
        serde_json::from_value(json!({
            "type":"agent/inbox/spliced","seq":seq,"time":1000+seq,"data":data
        }))
        .unwrap()
    }

    #[test]
    fn splices_apply_in_order_and_remove_by_count() {
        let events = vec![
            event(
                0,
                json!({"target":"next-turn","start":0,"inserted":[
                    {"id":"a","content":[{"type":"text","text":"first"}]},
                    {"id":"b","content":[{"type":"text","text":"second"}]}
                ]}),
            ),
            event(
                1,
                json!({"target":"next-turn","start":0,"inserted":[
                    {"id":"c","content":[{"type":"text","text":"front"}]}
                ]}),
            ),
            event(
                2,
                json!({"target":"next-turn","start":1,"removedCount":1,"inserted":[]}),
            ),
            event(
                3,
                json!({"target":"transcript","start":0,"inserted":[
                    {"id":"ignored","content":[{"type":"text","text":"not the queue"}]}
                ]}),
            ),
        ];
        let items = from_events(&events);
        assert_eq!(
            items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["c", "b"]
        );
    }

    #[test]
    fn previews_flatten_text_and_mark_other_blocks() {
        let events = vec![event(
            0,
            json!({"target":"next-turn","start":0,"inserted":[
                {"id":"a","content":[
                    {"type":"text","text":"  two\n lines  "},
                    {"type":"image"}
                ]},
                {"id":"b","content":[{"type":"file"}]}
            ]}),
        )];
        let items = from_events(&events);
        assert_eq!(items[0].preview, "two lines [image]");
        assert_eq!(items[1].preview, "[file]");
    }

    #[test]
    fn previews_are_bounded_and_the_list_is_bounded() {
        let long = "x".repeat(PREVIEW_CHARS + 5);
        let rows: Vec<Value> = (0..MAX_ITEMS + 3)
            .map(|index| json!({"id":format!("q{index}"),"content":[{"type":"text","text":long}]}))
            .collect();
        let events = vec![event(
            0,
            json!({"target":"next-turn","start":0,"inserted":rows}),
        )];
        let items = from_events(&events);
        assert_eq!(items.len(), MAX_ITEMS);
        assert_eq!(items[0].preview.chars().count(), PREVIEW_CHARS + 1);
        assert!(items[0].preview.ends_with('…'));
    }

    #[test]
    fn an_inbox_projection_row_list_parses_the_same_way() {
        let rows = vec![
            json!({"id":"a","content":[{"type":"text","text":"projected"}]}),
            json!({"id":"b","content":[{"type":"file"}]}),
        ];
        let items = from_rows(&rows);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "a");
        assert_eq!(items[0].preview, "projected");
        assert_eq!(items[1].preview, "[file]");
        assert!(from_rows(&[]).is_empty());
    }

    #[test]
    fn malformed_and_out_of_range_splices_are_ignored() {
        let events = vec![
            event(
                0,
                json!({"target":"next-turn","start":99,"removedCount":9,"inserted":[]}),
            ),
            event(
                1,
                json!({"target":"next-turn","start":0,"inserted":"not an array"}),
            ),
            event(2, json!({"target":"next-turn"})),
        ];
        assert!(from_events(&events).is_empty());
    }
}
