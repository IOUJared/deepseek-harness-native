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

/// One attachment waiting with a queued message. The reference draws a thumbnail for an image and a
/// name-plus-size chip for a file (`QueueDock.tsx:70-79,213-233`); native has no image thumbnails
/// anywhere, so both are drawn as the chip and the difference is recorded rather than faked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub name: String,
    pub bytes: u64,
    pub image: bool,
}

impl Attachment {
    /// `4.9KB`, the reference's own size form (`QueueDock.tsx:70-79`, `fileSizeText`).
    pub fn size_text(&self) -> String {
        let kb = self.bytes as f64 / 1000.0;
        if self.bytes < 1000 {
            format!("{}B", self.bytes)
        } else if kb < 1000.0 {
            format!("{kb:.1}KB")
        } else {
            format!("{:.1}MB", kb / 1000.0)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub preview: String,
    /// The row's text for an inline edit; text blocks joined with newlines.
    pub text: String,
    /// True when the row carries an image or file block; such a row cannot be edited.
    pub has_non_text: bool,
    /// The attachments travelling with the message, in their own order.
    pub attachments: Vec<Attachment>,
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
            // An attachment that is named beside the preview does not also appear as a bracketed
            // placeholder: the reference draws the file chip and no `[file]` text
            // (`QueueDock.tsx:70-79,213-233`).
            _ if block.get("attachment").is_some() => {}
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
    let mut text = Vec::new();
    let mut has_non_text = false;
    let mut attachments = Vec::new();
    for block in content.and_then(Value::as_array).into_iter().flatten() {
        let kind = block.get("type").and_then(Value::as_str);
        match kind {
            Some("text") => {
                if let Some(value) = block.get("text").and_then(Value::as_str) {
                    text.push(value);
                }
            }
            Some(other) => {
                has_non_text = true;
                // A queued attachment keeps its own identity: the reference names it and states its
                // size beside the preview, so `[file]` alone is not the row.
                if let Some(attachment) = block.get("attachment") {
                    let name = attachment
                        .get("name")
                        .and_then(Value::as_str)
                        .filter(|name| !name.is_empty() && name.chars().count() <= 255);
                    if let Some(name) = name {
                        attachments.push(Attachment {
                            name: name.to_owned(),
                            bytes: attachment.get("bytes").and_then(Value::as_u64).unwrap_or(0),
                            image: other == "image",
                        });
                    }
                }
            }
            None => {}
        }
    }
    Item {
        id: id
            .filter(|id| !id.is_empty() && id.len() <= 512)
            .map_or_else(|| format!("queued-{seq}-{index}"), str::to_owned),
        preview: preview(content),
        text: text.join("\n"),
        has_non_text,
        attachments,
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
    fn a_queued_attachment_keeps_its_name_and_size() {
        let events = vec![event(
            0,
            json!({"target":"next-turn","start":0,"inserted":[
                {"id":"a","content":[
                    {"type":"text","text":"look at these"},
                    {"type":"file","attachment":{"name":"report.pdf","bytes":4900}},
                    {"type":"image","attachment":{"name":"screen.png","bytes":1234567}}
                ]}
            ]}),
        )];
        let items = from_events(&events);
        let names: Vec<_> = items[0]
            .attachments
            .iter()
            .map(|attachment| (attachment.name.as_str(), attachment.size_text()))
            .collect();
        // Both are chips: native has no thumbnails, and the audit's recorded difference is that an
        // image would be a picture in the reference.
        assert_eq!(
            names,
            [
                ("report.pdf", "4.9KB".to_owned()),
                ("screen.png", "1.2MB".to_owned())
            ]
        );
        assert!(items[0].attachments[1].image);
        assert!(!items[0].attachments[0].image);
        // A row with no attachment carries none.
        let plain = vec![event(
            0,
            json!({"target":"next-turn","start":0,"inserted":[
                {"id":"b","content":[{"type":"text","text":"plain"}]}
            ]}),
        )];
        assert!(from_events(&plain)[0].attachments.is_empty());
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
