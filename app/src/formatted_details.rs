//! Bounded, inert CommonMark presentation plan. Any failure requires displaying the
//! whole original source as plain text, never a partially constructed document.
//! No content-bearing type implements `Debug` (including private parser state).

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

pub(super) const INPUT_BYTES: usize = 32 * 1024;
const TEXT_BYTES: usize = 64 * 1024;
const BLOCKS: usize = 128;
const SPANS: usize = 1024;
/// Cells per row, and rows per table, stay bounded like every other block list.
const CELLS: usize = 64;
const ROWS: usize = 128;
const NESTING: usize = 16;

pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    /// The reference's `imageAlt` run: an image whose source it cannot resolve renders
    /// its alt text as tertiary italic (`render.tsx:640,653`). Iced has no image
    /// pipeline here, so every image takes that branch.
    pub alt: bool,
}

/// One table cell's inline runs. A cell holds no block structure: `pulldown-cmark`
/// emits only inline events inside `TableCell`.
pub type Cell = Vec<Span>;

pub enum Block {
    Paragraph {
        spans: Vec<Span>,
        indent: u8,
        marker: Option<String>,
        quote: bool,
    },
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Code {
        language: Option<String>,
        text: String,
        indent: u8,
    },
    /// A GitHub table (`Options::ENABLE_TABLES`): the first row is the header.
    Table {
        header: Vec<Cell>,
        rows: Vec<Vec<Cell>>,
        indent: u8,
    },
    Rule,
}

pub struct Document {
    pub blocks: Vec<Block>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    Bytes,
    Blocks,
    Spans,
    Nesting,
    Unsupported,
}

/// Parses only representable constructs. Destinations are visible plain text,
/// not URI actions. Fence info is an untrusted plain label, never a highlighter
/// directive. Budgets count UTF-8 bytes, all output blocks/spans and open tags.
pub fn parse(source: &str) -> Result<Document, Failure> {
    // This check must precede construction of the CommonMark parser.
    if source.len() > INPUT_BYTES {
        return Err(Failure::Bytes);
    }
    build(Parser::new_ext(source, Options::ENABLE_TABLES))
}

struct Frame {
    end: TagEnd,
    destination: Option<String>,
}

struct List {
    next: Option<u64>,
}

struct Item {
    marker: Option<String>,
}

struct Inline {
    heading: Option<u8>,
    explicit: bool,
    indent: u8,
    marker: Option<String>,
    quote: bool,
    spans: Vec<Span>,
}

struct Code {
    language: Option<String>,
    text: String,
    indent: u8,
}

#[derive(Default)]
struct Table {
    header: Vec<Cell>,
    rows: Vec<Vec<Cell>>,
    row: Vec<Cell>,
    /// True while the head section's cells are open; the head is the first row.
    head: bool,
}
impl Table {
    fn finish_row(&mut self) {
        let row = std::mem::take(&mut self.row);
        if self.head {
            self.header = row;
        } else {
            self.rows.push(row);
        }
    }
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    frames: Vec<Frame>,
    lists: Vec<List>,
    items: Vec<Item>,
    quotes: usize,
    bold: usize,
    italic: usize,
    inline: Option<Inline>,
    code: Option<Code>,
    table: Option<Table>,
    /// An open image: its alt text accumulates here, because the HTML alt attribute
    /// and the reference's fallback are the image's plain text.
    alt: Option<String>,
    text_bytes: usize,
    spans: usize,
}

fn build<'a>(events: impl IntoIterator<Item = Event<'a>>) -> Result<Document, Failure> {
    let mut builder = Builder::default();
    for event in events {
        match event {
            Event::Start(tag) => builder.start(tag)?,
            Event::End(end) => builder.end(end)?,
            Event::Text(text) => {
                if builder.code.is_some() {
                    builder.charge(text.len())?;
                    builder
                        .code
                        .as_mut()
                        .ok_or(Failure::Unsupported)?
                        .text
                        .push_str(&text);
                } else {
                    builder.text(&text, false)?;
                }
            }
            Event::Code(text) => builder.text(&text, true)?,
            Event::SoftBreak | Event::HardBreak => builder.text("\n", false)?,
            Event::Rule => {
                // Rule has no field for container provenance.
                if !builder.frames.is_empty() {
                    return Err(Failure::Unsupported);
                }
                builder.flush_synthetic()?;
                builder.push(Block::Rule)?;
            }
            // Includes HTML even with pulldown-cmark's default features disabled.
            _ => return Err(Failure::Unsupported),
        }
    }
    if !builder.frames.is_empty() || builder.code.is_some() || builder.inline.is_some() {
        return Err(Failure::Unsupported);
    }
    Ok(Document {
        blocks: builder.blocks,
    })
}

impl Builder {
    fn charge(&mut self, bytes: usize) -> Result<(), Failure> {
        self.text_bytes = self.text_bytes.checked_add(bytes).ok_or(Failure::Bytes)?;
        if self.text_bytes > TEXT_BYTES {
            return Err(Failure::Bytes);
        }
        Ok(())
    }

    fn push(&mut self, block: Block) -> Result<(), Failure> {
        if self.blocks.len() >= BLOCKS {
            return Err(Failure::Blocks);
        }
        self.blocks.push(block);
        Ok(())
    }

    fn indent(&self) -> u8 {
        // The first list level is represented by its marker, the first quote
        // level by `quote`; further levels require additional indentation.
        (self.lists.len().saturating_sub(1) + self.quotes.saturating_sub(1)) as u8
    }

    fn begin_inline(&mut self, heading: Option<u8>, explicit: bool) -> Result<(), Failure> {
        self.flush_synthetic()?;
        let marker = self.items.last_mut().and_then(|item| item.marker.take());
        if let Some(marker) = &marker {
            self.charge(marker.len())?;
        }
        // Once its marker has been consumed, a list paragraph must remain
        // indented under the item's text, rather than resemble outside text.
        let indent = self.indent() + u8::from(!self.lists.is_empty() && marker.is_none());
        self.inline = Some(Inline {
            heading,
            explicit,
            indent,
            marker,
            quote: self.quotes != 0,
            spans: Vec::new(),
        });
        Ok(())
    }

    fn ensure_inline(&mut self) -> Result<(), Failure> {
        if self.code.is_some() {
            return Err(Failure::Unsupported);
        }
        if self.inline.is_none() {
            // Tight lists omit Paragraph tags entirely.
            if self.items.is_empty() {
                return Err(Failure::Unsupported);
            }
            self.begin_inline(None, false)?;
        }
        Ok(())
    }

    fn text(&mut self, text: &str, code: bool) -> Result<(), Failure> {
        if self.alt.is_some() {
            // The alt run keeps its bytes but none of its inline markup, exactly like
            // the attribute the reference falls back to.
            self.charge(text.len())?;
            if let Some(alt) = self.alt.as_mut() {
                alt.push_str(text);
            }
            return Ok(());
        }
        self.ensure_inline()?;
        self.charge(text.len())?;
        if text.is_empty() {
            return Ok(());
        }
        let inline = self.inline.as_mut().ok_or(Failure::Unsupported)?;
        let bold = self.bold != 0;
        let italic = self.italic != 0;
        if let Some(last) = inline.spans.last_mut()
            && last.bold == bold
            && last.italic == italic
            && last.code == code
        {
            last.text.push_str(text);
            return Ok(());
        }
        if self.spans >= SPANS {
            return Err(Failure::Spans);
        }
        self.spans += 1;
        inline.spans.push(Span {
            text: text.into(),
            bold,
            italic,
            code,
            alt: false,
        });
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Failure> {
        if let Some(inline) = self.inline.take() {
            let block = if let Some(level) = inline.heading {
                Block::Heading {
                    level,
                    spans: inline.spans,
                }
            } else {
                Block::Paragraph {
                    spans: inline.spans,
                    indent: inline.indent,
                    marker: inline.marker,
                    quote: inline.quote,
                }
            };
            self.push(block)?;
        }
        Ok(())
    }

    fn flush_synthetic(&mut self) -> Result<(), Failure> {
        if self.inline.as_ref().is_some_and(|inline| inline.explicit) {
            return Err(Failure::Unsupported);
        }
        self.flush()
    }

    fn preserve_item_marker(&mut self) -> Result<(), Failure> {
        if self.items.last().is_some_and(|item| item.marker.is_some()) {
            // Empty items, or items starting with a nested list/code, still
            // need a marker-bearing paragraph so the outer item is not lost.
            self.begin_inline(None, false)?;
            self.flush()?;
        }
        Ok(())
    }

    fn start(&mut self, tag: Tag<'_>) -> Result<(), Failure> {
        if self.code.is_some() {
            return Err(Failure::Unsupported);
        }
        if self.frames.len() >= NESTING {
            return Err(Failure::Nesting);
        }
        let end = tag.to_end();
        let mut destination = None;
        match tag {
            Tag::Paragraph => self.begin_inline(None, true)?,
            Tag::Heading {
                level,
                id,
                classes,
                attrs,
            } => {
                // Heading cannot represent quote/list provenance or attributes.
                if !self.frames.is_empty()
                    || id.is_some()
                    || !classes.is_empty()
                    || !attrs.is_empty()
                {
                    return Err(Failure::Unsupported);
                }
                self.begin_inline(Some(level as u8), true)?;
            }
            Tag::BlockQuote(None) => {
                // Paragraph's flat marker/quote fields cannot retain the order
                // of mixed list/quote ancestry. Fail the entire source instead.
                if !self.lists.is_empty() {
                    return Err(Failure::Unsupported);
                }
                self.flush_synthetic()?;
                self.quotes += 1;
            }
            Tag::List(next) => {
                if self.quotes != 0 {
                    return Err(Failure::Unsupported);
                }
                self.flush_synthetic()?;
                self.preserve_item_marker()?;
                self.lists.push(List { next });
            }
            Tag::Item => {
                self.flush_synthetic()?;
                let list = self.lists.last_mut().ok_or(Failure::Unsupported)?;
                let marker = if let Some(number) = list.next {
                    list.next = Some(number.checked_add(1).ok_or(Failure::Unsupported)?);
                    format!("{number}.")
                } else {
                    "•".into()
                };
                self.items.push(Item {
                    marker: Some(marker),
                });
            }
            Tag::CodeBlock(kind) => {
                if self.quotes != 0 {
                    return Err(Failure::Unsupported);
                }
                self.flush_synthetic()?;
                self.preserve_item_marker()?;
                let language = match kind {
                    CodeBlockKind::Indented => None,
                    CodeBlockKind::Fenced(info) if info.is_empty() => None,
                    CodeBlockKind::Fenced(info) => {
                        self.charge(info.len())?;
                        Some(info.into_string())
                    }
                };
                self.code = Some(Code {
                    language,
                    text: String::new(),
                    // No marker on Code: even the first list level is indented.
                    indent: self.lists.len() as u8,
                });
            }
            Tag::Strong => {
                self.ensure_inline()?;
                self.bold += 1;
            }
            Tag::Emphasis => {
                self.ensure_inline()?;
                self.italic += 1;
            }
            Tag::Link {
                dest_url, title, ..
            } => {
                // Link titles have no presentation field: never silently drop
                // one. The fallback retains its exact source representation.
                if !title.is_empty() {
                    return Err(Failure::Unsupported);
                }
                self.ensure_inline()?;
                destination = Some(dest_url.into_string());
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                self.ensure_inline()?;
                if !title.is_empty() || self.alt.is_some() {
                    return Err(Failure::Unsupported);
                }
                destination = Some(dest_url.into_string());
                self.alt = Some(String::new());
            }
            Tag::Table(_) => {
                // A table's cells carry the inline machinery, so a table inside a
                // list or quote cannot keep its provenance in this flat model.
                if self.table.is_some()
                    || self.inline.is_some()
                    || !self.frames.is_empty()
                    || !self.lists.is_empty()
                    || self.quotes != 0
                {
                    return Err(Failure::Unsupported);
                }
                self.table = Some(Table::default());
            }
            Tag::TableHead => {
                let table = self.table.as_mut().ok_or(Failure::Unsupported)?;
                if table.head || !table.header.is_empty() || !table.rows.is_empty() {
                    return Err(Failure::Unsupported);
                }
                table.head = true;
            }
            Tag::TableRow => {
                let table = self.table.as_mut().ok_or(Failure::Unsupported)?;
                if !table.row.is_empty() {
                    return Err(Failure::Unsupported);
                }
            }
            Tag::TableCell => {
                if self.inline.is_some() || self.table.is_none() {
                    return Err(Failure::Unsupported);
                }
                self.inline = Some(Inline {
                    heading: None,
                    explicit: true,
                    indent: 0,
                    marker: None,
                    quote: false,
                    spans: Vec::new(),
                });
            }
            // Images, HTML and all other unsupported tags fail closed.
            _ => return Err(Failure::Unsupported),
        }
        self.frames.push(Frame { end, destination });
        Ok(())
    }

    fn end(&mut self, end: TagEnd) -> Result<(), Failure> {
        let frame = self.frames.pop().ok_or(Failure::Unsupported)?;
        if frame.end != end {
            return Err(Failure::Unsupported);
        }
        match end {
            TagEnd::Paragraph | TagEnd::Heading(_) => self.flush()?,
            TagEnd::CodeBlock => {
                let code = self.code.take().ok_or(Failure::Unsupported)?;
                self.push(Block::Code {
                    language: code.language,
                    text: code.text,
                    indent: code.indent,
                })?;
            }
            TagEnd::BlockQuote(None) => {
                self.flush_synthetic()?;
                self.quotes = self.quotes.checked_sub(1).ok_or(Failure::Unsupported)?;
            }
            TagEnd::List(_) => {
                self.flush_synthetic()?;
                self.lists.pop().ok_or(Failure::Unsupported)?;
            }
            TagEnd::Item => {
                self.flush_synthetic()?;
                self.preserve_item_marker()?;
                self.items.pop().ok_or(Failure::Unsupported)?;
            }
            TagEnd::Strong => self.bold = self.bold.checked_sub(1).ok_or(Failure::Unsupported)?,
            TagEnd::Emphasis => {
                self.italic = self.italic.checked_sub(1).ok_or(Failure::Unsupported)?
            }
            TagEnd::Link => {
                let destination = frame.destination.ok_or(Failure::Unsupported)?;
                self.text(" (", false)?;
                self.text(&destination, false)?;
                self.text(")", false)?;
            }
            TagEnd::TableCell => {
                let cell = self.inline.take().ok_or(Failure::Unsupported)?.spans;
                let table = self.table.as_mut().ok_or(Failure::Unsupported)?;
                if table.row.len() >= CELLS {
                    return Err(Failure::Blocks);
                }
                table.row.push(cell);
            }
            TagEnd::TableRow => {
                let table = self.table.as_mut().ok_or(Failure::Unsupported)?;
                if table.head && table.row.is_empty() {
                    return Err(Failure::Unsupported);
                }
                if table.rows.len() >= ROWS {
                    return Err(Failure::Blocks);
                }
                table.finish_row();
            }
            TagEnd::TableHead => {
                let table = self.table.as_mut().ok_or(Failure::Unsupported)?;
                if !table.head || table.row.is_empty() {
                    return Err(Failure::Unsupported);
                }
                table.finish_row();
                table.head = false;
            }
            TagEnd::Image => {
                let alt = self.alt.take().ok_or(Failure::Unsupported)?;
                let destination = frame.destination.ok_or(Failure::Unsupported)?;
                // The sheet's unresolvable branch shows the alt text; its failed-load
                // branch adds the destination when there is no alt, so nothing on the
                // line can be lost.
                let rendered = if alt.is_empty() { destination } else { alt };
                self.charge(rendered.len())?;
                let inline = self.inline.as_mut().ok_or(Failure::Unsupported)?;
                if self.spans >= SPANS {
                    return Err(Failure::Spans);
                }
                self.spans += 1;
                inline.spans.push(Span {
                    text: rendered,
                    bold: false,
                    italic: true,
                    code: false,
                    alt: true,
                });
            }
            TagEnd::Table => {
                let table = self.table.take().ok_or(Failure::Unsupported)?;
                if table.header.is_empty() || table.head {
                    return Err(Failure::Unsupported);
                }
                self.push(Block::Table {
                    header: table.header,
                    rows: table.rows,
                    indent: self.indent(),
                })?;
            }
            _ => return Err(Failure::Unsupported),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{CowStr, HeadingLevel};

    fn document(source: &str) -> Document {
        match parse(source) {
            Ok(document) => document,
            Err(failure) => panic!("unexpected parser failure: {failure:?}"),
        }
    }

    fn failure(source: &str, expected: Failure) {
        match parse(source) {
            Err(actual) => assert_eq!(actual, expected),
            Ok(_) => panic!("expected parser failure"),
        }
    }

    fn paragraph(block: &Block) -> (&[Span], u8, Option<&str>, bool) {
        match block {
            Block::Paragraph {
                spans,
                indent,
                marker,
                quote,
            } => (spans, *indent, marker.as_deref(), *quote),
            _ => panic!("expected paragraph"),
        }
    }

    fn joined(spans: &[Span]) -> String {
        spans.iter().map(|span| span.text.as_str()).collect()
    }

    fn code(block: &Block) -> (Option<&str>, &str, u8) {
        match block {
            Block::Code {
                language,
                text,
                indent,
            } => (language.as_deref(), text, *indent),
            _ => panic!("expected code"),
        }
    }

    #[test]
    fn empty_whitespace_and_reference_definitions_have_no_display_blocks() {
        assert!(document("").blocks.is_empty());
        assert!(document(" \n\t\n").blocks.is_empty());
        assert!(
            document("[unused]: https://example.invalid\n")
                .blocks
                .is_empty()
        );
    }

    #[test]
    fn paragraphs_headings_rule_breaks_unicode_and_entities() {
        let doc = document("# Héllo &amp; 世界\n\nplain\nsoft  \nhard\n\n---\n\nlast");
        assert_eq!(doc.blocks.len(), 4);
        match &doc.blocks[0] {
            Block::Heading { level, spans } => {
                assert_eq!(*level, 1);
                assert_eq!(joined(spans), "Héllo & 世界");
            }
            _ => panic!("expected heading"),
        }
        let (spans, indent, marker, quote) = paragraph(&doc.blocks[1]);
        assert_eq!(joined(spans), "plain\nsoft\nhard");
        assert_eq!((indent, marker, quote), (0, None, false));
        assert!(matches!(doc.blocks[2], Block::Rule));
        assert_eq!(joined(paragraph(&doc.blocks[3]).0), "last");
        let doc = document("setext\n======\n\n###### six");
        assert!(matches!(&doc.blocks[0], Block::Heading { level: 1, .. }));
        assert!(matches!(&doc.blocks[1], Block::Heading { level: 6, .. }));
    }

    #[test]
    fn nested_styles_restore_outer_counters_and_inline_code() {
        let doc = document("**outer *both* outer** plain *italic* `x < y` ***all***");
        let spans = paragraph(&doc.blocks[0]).0;
        assert_eq!(joined(spans), "outer both outer plain italic x < y all");
        assert!(spans.iter().any(|s| s.text == "both" && s.bold && s.italic));
        assert!(
            spans
                .iter()
                .any(|s| s.text == " outer" && s.bold && !s.italic)
        );
        assert!(
            spans
                .iter()
                .any(|s| s.text == "italic" && !s.bold && s.italic)
        );
        assert!(
            spans
                .iter()
                .any(|s| s.text == "x < y" && s.code && !s.bold && !s.italic)
        );
        assert!(spans.iter().any(|s| s.text == "all" && s.bold && s.italic));
    }

    #[test]
    fn same_kind_style_nesting_is_not_a_boolean_toggle() {
        let events = vec![
            Event::Start(Tag::Paragraph),
            Event::Start(Tag::Strong),
            Event::Text("a".into()),
            Event::Start(Tag::Strong),
            Event::Text("b".into()),
            Event::End(TagEnd::Strong),
            Event::Text("c".into()),
            Event::End(TagEnd::Strong),
            Event::Text("d".into()),
            Event::End(TagEnd::Paragraph),
        ];
        let doc = build(events).unwrap_or_else(|_| panic!("valid events"));
        let spans = paragraph(&doc.blocks[0]).0;
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text, "abc");
        assert!(spans[0].bold);
        assert!(!spans[1].bold);
    }

    #[test]
    fn links_are_visible_inert_text_including_unsafe_schemes_and_references() {
        let doc = document(
            "[**label**](javascript:alert) [file](file:///private) <https://example.invalid> [r][id]\n\n[id]: data:text/plain,hello",
        );
        let spans = paragraph(&doc.blocks[0]).0;
        assert_eq!(
            joined(spans),
            "label (javascript:alert) file (file:///private) https://example.invalid (https://example.invalid) r (data:text/plain,hello)"
        );
        assert!(spans[0].bold);
        assert!(!spans[1].bold);
        // Core emits the email address itself as dest_url. Only its optional
        // HTML writer prepends mailto:; this inert plan does not invent a URI.
        let email = document("<a@example.invalid>");
        assert_eq!(
            joined(paragraph(&email.blocks[0]).0),
            "a@example.invalid (a@example.invalid)"
        );
        failure(
            "[label](https://example.invalid \"title\")",
            Failure::Unsupported,
        );
    }

    #[test]
    fn tight_ordered_unordered_and_nested_lists_preserve_order_and_markers() {
        let doc = document("7. outer\n   - inner **bold**\n   - next\n8. tail\n");
        assert_eq!(doc.blocks.len(), 4);
        let expected = [
            ("outer", 0, "7."),
            ("inner bold", 1, "•"),
            ("next", 1, "•"),
            ("tail", 0, "8."),
        ];
        for (block, (text, depth, label)) in doc.blocks.iter().zip(expected) {
            let (spans, indent, marker, quote) = paragraph(block);
            assert_eq!(joined(spans), text);
            assert_eq!((indent, marker, quote), (depth, Some(label), false));
        }
    }

    #[test]
    fn loose_items_and_separate_quotes_preserve_depth_without_repeated_markers() {
        let doc = document("- first\n\n  second\n\n- last\n\n> quoted\n>\n> > deeper\n");
        assert_eq!(doc.blocks.len(), 5);
        let expected = [
            ("first", 0, Some("•"), false),
            ("second", 1, None, false),
            ("last", 0, Some("•"), false),
            ("quoted", 0, None, true),
            ("deeper", 1, None, true),
        ];
        for (block, (text, depth, label, quoted)) in doc.blocks.iter().zip(expected) {
            let (spans, indent, marker, quote) = paragraph(block);
            assert_eq!(joined(spans), text);
            assert_eq!((indent, marker, quote), (depth, label, quoted));
        }
        let doc = document("-\n- bare\n");
        assert_eq!(paragraph(&doc.blocks[0]).2, Some("•"));
        assert!(paragraph(&doc.blocks[0]).0.is_empty());
        assert_eq!(paragraph(&doc.blocks[1]).2, Some("•"));
        assert!(!paragraph(&doc.blocks[1]).3);
    }

    #[test]
    fn mixed_quote_list_ancestries_fail_instead_of_collapsing_to_the_same_plan() {
        for source in [
            "> - text",
            "- > text",
            "> 7. text",
            "7. > text",
            "> > - nested",
            "- first\n  - > nested",
            "- first\n\n  second\n\n  > quoted\n  >\n  > > deeper\n\n- last\n",
            "-\n- > quoted\n",
        ] {
            failure(source, Failure::Unsupported);
            // A preceding valid block must not yield a partial display plan.
            failure(
                &format!("before\n\n{source}\n\nafter"),
                Failure::Unsupported,
            );
        }
    }

    #[test]
    fn loose_list_continuations_remain_distinct_from_outside_paragraphs() {
        let doc = document("- first\n\n  second\n\noutside");
        assert_eq!(doc.blocks.len(), 3);
        assert_eq!(paragraph(&doc.blocks[0]).1, 0);
        assert_eq!(paragraph(&doc.blocks[0]).2, Some("•"));
        assert_eq!(paragraph(&doc.blocks[1]).1, 1);
        assert_eq!(paragraph(&doc.blocks[1]).2, None);
        assert_eq!(joined(paragraph(&doc.blocks[1]).0), "second");
        assert_eq!(paragraph(&doc.blocks[2]).1, 0);
        assert_eq!(paragraph(&doc.blocks[2]).2, None);
        assert_eq!(joined(paragraph(&doc.blocks[2]).0), "outside");

        let doc = document(
            "- first\n\n  second\n\n  - nested first\n\n    nested second\n\n  outer third\n\noutside",
        );
        assert_eq!(doc.blocks.len(), 6);
        let expected = [
            ("first", 0, Some("•")),
            ("second", 1, None),
            ("nested first", 1, Some("•")),
            ("nested second", 2, None),
            ("outer third", 1, None),
            ("outside", 0, None),
        ];
        for (block, (text, depth, label)) in doc.blocks.iter().zip(expected) {
            let (spans, indent, marker, quote) = paragraph(block);
            assert_eq!(joined(spans), text);
            assert_eq!((indent, marker, quote), (depth, label, false));
        }
    }

    #[test]
    fn nested_only_items_retain_parent_marker() {
        let doc = document("-\n  - child\n");
        assert_eq!(doc.blocks.len(), 2);
        assert!(paragraph(&doc.blocks[0]).0.is_empty());
        assert_eq!(paragraph(&doc.blocks[0]).2, Some("•"));
        assert_eq!(paragraph(&doc.blocks[1]).1, 1);
    }

    #[test]
    fn fenced_indented_empty_and_unclosed_code_keep_exact_core_text() {
        let doc = document("```rust arbitrary label\nline\n\n  spaces\n```\n\n    a\n    b\n");
        assert_eq!(
            code(&doc.blocks[0]),
            (Some("rust arbitrary label"), "line\n\n  spaces\n", 0)
        );
        assert_eq!(code(&doc.blocks[1]), (None, "a\nb\n", 0));
        let doc = document("```\n```");
        assert_eq!(code(&doc.blocks[0]), (None, "", 0));
        let doc = document("```plain\nno final newline");
        assert_eq!(code(&doc.blocks[0]), (Some("plain"), "no final newline", 0));
        let doc = document("```plain\nwith final newline\n");
        assert_eq!(code(&doc.blocks[0]).1, "with final newline\n");
        let doc = document("- before\n\n  ```lang\n  nested\n  ```\n");
        assert_eq!(code(&doc.blocks[1]), (Some("lang"), "nested\n", 1));
        let doc = document("- ```\n  first code\n  ```\n");
        assert_eq!(paragraph(&doc.blocks[0]).2, Some("•"));
        assert_eq!(code(&doc.blocks[1]).1, "first code\n");
    }

    #[test]
    fn images_fall_back_to_their_alt_run_and_keep_the_rest_of_the_message() {
        // The reference's unresolvable branch renders the alt text as tertiary italic
        // (`render.tsx:640`); the failed-load branch adds the destination when there is
        // no alt (`:653`).
        let inline = document("before ![PUBLIC alt](https://example.invalid/a.png) after");
        let spans = paragraph(&inline.blocks[0]).0;
        let image: Vec<_> = spans.iter().filter(|span| span.alt).collect();
        assert_eq!(image.len(), 1);
        assert_eq!(image[0].text, "PUBLIC alt");
        assert!(image[0].italic);
        assert!(!image[0].bold && !image[0].code);
        // The alt keeps its bytes but none of its inline markup: `![**x**](u)` reads
        // "x", exactly like an HTML alt attribute.
        let marked = document("![**PUBLIC** `alt`](u)");
        let spans = paragraph(&marked.blocks[0]).0;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "PUBLIC alt");
        assert!(!spans[0].bold && !spans[0].code);
        // With no alt the destination stands in, so the image cannot vanish silently.
        let without_alt = document("![](image.png)");
        let spans = paragraph(&without_alt.blocks[0]).0;
        assert_eq!(spans[0].text, "image.png");
        assert!(spans[0].alt);
        // A titled image is refused rather than silently losing the title, and the
        // destination stays visible as text for a plain link.
        failure("![alt](u \"title\")", Failure::Unsupported);
    }
    #[test]
    fn tables_keep_their_header_inline_runs_and_bounds() {
        let document = document("| a | *b* |\n|---|---|\n| `c` | d |\n| e | f |\n");
        let (header, rows, indent) = match &document.blocks[0] {
            Block::Table {
                header,
                rows,
                indent,
            } => (header, rows, *indent),
            _ => panic!("expected table"),
        };
        assert_eq!(indent, 0);
        assert_eq!(header.len(), 2);
        assert_eq!(joined(&header[0]), "a");
        assert!(header[1][0].italic);
        assert_eq!(rows.len(), 2);
        assert!(rows[0][0][0].code);
        assert_eq!(joined(&rows[1][0]), "e");
        // A table stays out of list/quote provenance, which the flat block model cannot
        // represent, and its rows are bounded like the other block lists.
        failure("> | a |\n> |---|\n> | b |\n", Failure::Unsupported);
        let mut rows = String::from("| a |\n|---|\n");
        for index in 0..ROWS {
            rows.push_str(&format!("| {index} |\n"));
        }
        assert!(parse(&rows).is_ok());
        rows.push_str("| over |\n");
        failure(&rows, Failure::Blocks);
        // Cells per row are bounded too.
        // The per-row cell cap is exercised through the event stream directly:
        // pulldown-cmark normalises column counts from the syntax, so a hand-written
        // 65-column table never reaches the cell push unchanged.
        let mut events = vec![
            Event::Start(Tag::Table(Vec::new())),
            Event::Start(Tag::TableHead),
        ];
        for _ in 0..=CELLS {
            events.push(Event::Start(Tag::TableCell));
            events.push(Event::Text(CowStr::from("a")));
            events.push(Event::End(TagEnd::TableCell));
        }
        events.push(Event::End(TagEnd::TableHead));
        events.push(Event::End(TagEnd::Table));
        assert_eq!(build(events).err(), Some(Failure::Blocks));
    }
    #[test]
    fn unsupported_content_anywhere_fails_whole_source() {
        for source in [
            "before <b>html</b> after",
            "before\n\n<div>html</div>\n",
            "> # heading",
            "- # heading",
            "> ---",
            "- ***",
            "> ```\n> code\n> ```",
            ">     code\n",
            "- > ```\n  > code\n  > ```",
        ] {
            failure(source, Failure::Unsupported);
        }
        // Escaped HTML and HTML in code remain inert text, not unsupported nodes.
        assert_eq!(
            joined(paragraph(&document("\\<b> safe").blocks[0]).0),
            "<b> safe"
        );
        assert_eq!(
            code(&document("```\n<div>safe</div>\n```").blocks[0]).1,
            "<div>safe</div>\n"
        );
    }

    #[test]
    fn byte_limit_is_inclusive_and_counts_utf8_not_characters() {
        assert_eq!(
            joined(paragraph(&document(&"a".repeat(INPUT_BYTES)).blocks[0]).0).len(),
            INPUT_BYTES
        );
        failure(&"a".repeat(INPUT_BYTES + 1), Failure::Bytes);
        assert!(parse(&"é".repeat(INPUT_BYTES / 2)).is_ok());
        failure(&"é".repeat(INPUT_BYTES / 2 + 1), Failure::Bytes);
        // Bytes wins even when the oversized input also contains unsupported HTML.
        failure(&format!("<div>{}", "a".repeat(INPUT_BYTES)), Failure::Bytes);
    }

    #[test]
    fn block_limit_is_global_inclusive_and_includes_rules_code_empty_items() {
        assert_eq!(document(&"p\n\n".repeat(BLOCKS)).blocks.len(), BLOCKS);
        failure(&"p\n\n".repeat(BLOCKS + 1), Failure::Blocks);
        assert_eq!(document(&"---\n\n".repeat(BLOCKS)).blocks.len(), BLOCKS);
        failure(&"```\n```\n\n".repeat(BLOCKS + 1), Failure::Blocks);
        failure(&"-\n".repeat(BLOCKS + 1), Failure::Blocks);
    }

    #[test]
    fn span_limit_is_global_inclusive_and_counts_coalesced_output_spans() {
        let source = "a`b`".repeat(SPANS / 2);
        assert_eq!(paragraph(&document(&source).blocks[0]).0.len(), SPANS);
        failure(&(source + "a"), Failure::Spans);
        failure(
            &format!("{}\n\n{}", "a`b`".repeat(300), "a`b`".repeat(300)),
            Failure::Spans,
        );
        // Adjacent plain parser text events (entity/escape splits) are coalesced.
        let doc = document(&"a&amp;\\*".repeat(1500));
        assert_eq!(paragraph(&doc.blocks[0]).0.len(), 1);
    }

    #[test]
    fn nesting_limit_counts_all_open_tags_including_paragraph_and_styles() {
        let source = format!("{}text", "> ".repeat(NESTING - 1));
        assert!(parse(&source).is_ok());
        failure(&format!("{}text", "> ".repeat(NESTING)), Failure::Nesting);
        // A tight nested item contributes List and Item, not Paragraph.
        let mut source = String::new();
        for depth in 0..NESTING / 2 {
            source.push_str(&format!("{}- item\n", "  ".repeat(depth)));
        }
        assert!(parse(&source).is_ok());
        source.push_str(&format!("{}- item\n", "  ".repeat(NESTING / 2)));
        failure(&source, Failure::Nesting);
        let mut events = vec![Event::Start(Tag::Paragraph)];
        events.extend((0..NESTING).map(|_| Event::Start(Tag::Strong)));
        assert!(matches!(build(events), Err(Failure::Nesting)));
    }

    #[test]
    fn generated_text_budget_includes_reference_uri_expansion() {
        let uri = format!("https://example.invalid/{}", "x".repeat(2000));
        let source = format!("{}\n\n[id]: {uri}", "[a][id] ".repeat(40));
        assert!(source.len() < INPUT_BYTES);
        failure(&source, Failure::Bytes);
        let source = format!("{}\n\n[id]: {uri}", "[a][id] ".repeat(30));
        assert!(parse(&source).is_ok());
        // Exercise the inclusive generated limit independently of the smaller
        // input budget. This private event seam has no production I/O.
        let events = vec![
            Event::Start(Tag::Paragraph),
            Event::Text(CowStr::from("x".repeat(TEXT_BYTES))),
            Event::End(TagEnd::Paragraph),
        ];
        assert!(build(events).is_ok());
        let events = vec![
            Event::Start(Tag::Paragraph),
            Event::Text(CowStr::from("é".repeat(TEXT_BYTES / 2 + 1))),
            Event::End(TagEnd::Paragraph),
        ];
        assert!(matches!(build(events), Err(Failure::Bytes)));
        let mut builder = Builder::default();
        builder.charge(TEXT_BYTES - 2).unwrap();
        assert!(builder.charge(2).is_ok());
        assert_eq!(builder.charge(1), Err(Failure::Bytes));
    }

    #[test]
    fn generated_budget_also_counts_language_code_and_markers() {
        let fenced = |bytes| {
            vec![
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced("lang".into()))),
                Event::Text(CowStr::from("x".repeat(bytes))),
                Event::End(TagEnd::CodeBlock),
            ]
        };
        assert!(build(fenced(TEXT_BYTES - 4)).is_ok());
        assert!(matches!(build(fenced(TEXT_BYTES - 3)), Err(Failure::Bytes)));
        let item = |bytes| {
            vec![
                Event::Start(Tag::List(None)),
                Event::Start(Tag::Item),
                Event::Text(CowStr::from("x".repeat(bytes))),
                Event::End(TagEnd::Item),
                Event::End(TagEnd::List(false)),
            ]
        };
        assert!(build(item(TEXT_BYTES - "•".len())).is_ok());
        assert!(matches!(
            build(item(TEXT_BYTES - "•".len() + 1)),
            Err(Failure::Bytes)
        ));
    }

    #[test]
    fn malformed_or_unsupported_event_streams_never_yield_partial_document() {
        let prefix = || {
            vec![
                Event::Start(Tag::Paragraph),
                Event::Text("private".into()),
                Event::End(TagEnd::Paragraph),
            ]
        };
        for event in [
            Event::InlineHtml("<b>".into()),
            Event::Html("<div>".into()),
            Event::InlineMath("x".into()),
            Event::DisplayMath("x".into()),
            Event::FootnoteReference("secret".into()),
            Event::TaskListMarker(true),
        ] {
            let mut events = prefix();
            events.push(event);
            assert!(matches!(build(events), Err(Failure::Unsupported)));
        }
        for tag in [
            Tag::Image {
                link_type: pulldown_cmark::LinkType::Inline,
                dest_url: "secret".into(),
                title: "".into(),
                id: "".into(),
            },
            Tag::Table(vec![]),
            Tag::Strikethrough,
            Tag::FootnoteDefinition("secret".into()),
            Tag::DefinitionList,
            Tag::BlockQuote(Some(pulldown_cmark::BlockQuoteKind::Note)),
        ] {
            let mut events = prefix();
            events.push(Event::Start(tag));
            assert!(matches!(build(events), Err(Failure::Unsupported)));
        }
        assert!(matches!(
            build(vec![Event::End(TagEnd::Paragraph)]),
            Err(Failure::Unsupported)
        ));
        assert!(matches!(
            build(vec![
                Event::Start(Tag::Paragraph),
                Event::End(TagEnd::Heading(HeadingLevel::H1))
            ]),
            Err(Failure::Unsupported)
        ));
        assert!(matches!(
            build(vec![Event::Start(Tag::Paragraph)]),
            Err(Failure::Unsupported)
        ));
    }

    #[test]
    fn content_types_cannot_be_debug_formatted() {
        // If any of these types implements Debug, inference becomes ambiguous
        // between () and HasDebug and this regression stops compiling.
        trait AmbiguousIfDebug<A> {
            fn check() {}
        }
        struct HasDebug;
        impl<T: ?Sized> AmbiguousIfDebug<()> for T {}
        impl<T: ?Sized + std::fmt::Debug> AmbiguousIfDebug<HasDebug> for T {}
        let _ = <Document as AmbiguousIfDebug<_>>::check;
        let _ = <Block as AmbiguousIfDebug<_>>::check;
        let _ = <Span as AmbiguousIfDebug<_>>::check;
        let _ = <Builder as AmbiguousIfDebug<_>>::check;
        let _ = <Frame as AmbiguousIfDebug<_>>::check;
        let _ = <Item as AmbiguousIfDebug<_>>::check;
        let _ = <Inline as AmbiguousIfDebug<_>>::check;
        let _ = <Code as AmbiguousIfDebug<_>>::check;
    }

    #[test]
    fn diagnostics_only_expose_content_free_failure_variants() {
        // Raw HTML stays unsupported, so it can stand in for content that must never
        // reach a diagnostic.
        let private_source = "PRIVATE_TOKEN <b>PRIVATE_HTML</b>";
        let error = match parse(private_source) {
            Err(error) => error,
            Ok(_) => panic!("expected unsupported HTML"),
        };
        assert_eq!(format!("{error:?}"), "Unsupported");
        let copied = error;
        assert_eq!(copied, error);
    }
}
