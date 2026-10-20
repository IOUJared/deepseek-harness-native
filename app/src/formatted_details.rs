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
    /// `~~struck~~`, which the reference renders as `<del>` (`render.tsx:298-301`).
    pub struck: bool,
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
        /// Whether the quote bar sits outside the list indentations (the quote opened first) or
        /// inside them (the quote is nested in a list item). Both orders are the reference's own
        /// DOM nesting; the flat model keeps this one bit rather than a full container stack.
        quote_first: bool,
    },
    Heading {
        level: u8,
        spans: Vec<Span>,
        /// A heading inside a list or a quote keeps that container's indent and rule, like a
        /// paragraph does (`render.tsx:279-281`).
        indent: u8,
        quote: bool,
        quote_first: bool,
    },
    Code {
        language: Option<String>,
        text: String,
        indent: u8,
        /// A code block keeps its container's rule and indent, like any other block
        /// (`render.tsx:281-289`).
        quote: bool,
        quote_first: bool,
    },
    /// A GitHub table (`Options::ENABLE_TABLES`): the first row is the header.
    Table {
        header: Vec<Cell>,
        rows: Vec<Vec<Cell>>,
        indent: u8,
        /// A table keeps its container's rule and indent, like any other block
        /// (`render.tsx:281-289`).
        quote: bool,
        quote_first: bool,
    },
    /// A thematic rule. It keeps its container's indent and rule, and its own line shrinks by that
    /// inset (`render.tsx:290-291`).
    Rule {
        indent: u8,
        quote: bool,
        quote_first: bool,
    },
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
    // Tables, footnotes, math, strikethrough and task lists are the extensions the reference's
    // markdown renderer turns on: its pipeline renders `<del>` and disabled checkboxes
    // (`render.tsx:298-301,437-450`), so both are enabled here. A footnote renders as its marked
    // paragraph and the reference becomes `[n]` text; math has no native typesetter, so its source
    // is drawn as code — the recorded deliberate difference (`NATIVE-UI-PARITY.md`, renderer row).
    build(Parser::new_ext(
        source,
        Options::ENABLE_TABLES
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_MATH
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS,
    ))
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
    quote_first: bool,
    spans: Vec<Span>,
}

struct Code {
    language: Option<String>,
    text: String,
    indent: u8,
    quote: bool,
    quote_first: bool,
}

#[derive(Default)]
struct Table {
    header: Vec<Cell>,
    rows: Vec<Vec<Cell>>,
    row: Vec<Cell>,
    /// The table's container order, carried from the event that opened it.
    quote: bool,
    quote_first: bool,
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

/// One open block container, in opening order: what a nested list/quote pair's bar position
/// depends on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Container {
    List,
    Quote,
}

#[derive(Default)]
struct Builder {
    containers: Vec<Container>,
    blocks: Vec<Block>,
    frames: Vec<Frame>,
    lists: Vec<List>,
    items: Vec<Item>,
    quotes: usize,
    bold: usize,
    italic: usize,
    struck: usize,
    inline: Option<Inline>,
    code: Option<Code>,
    table: Option<Table>,
    /// An open image: its alt text accumulates here, because the HTML alt attribute
    /// and the reference's fallback are the image's plain text.
    alt: Option<String>,
    /// How deep the open image run is. A nested image's own `End` must not close the run the outer
    /// image opened, and its alt text simply joins the same plain string.
    image_depth: usize,
    /// A footnote definition's `[n]` marker, waiting for the paragraph it labels.
    pending_marker: Option<String>,
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
            // No native typesetter: the TeX source is drawn as code rather than left as literal
            // `$…$` text, which is the closest honest rendering.
            Event::InlineMath(text) => builder.text(&text, true)?,
            Event::DisplayMath(text) => {
                // The parser wraps display math in a paragraph, so the source is drawn inside it
                // as code — the same "no typesetter, show the source" rule as inline math.
                builder.text(&text, true)?;
            }
            Event::TaskListMarker(checked) => {
                // The reference keeps the list's own marker and prepends a disabled checkbox to
                // the item's first paragraph (`render.tsx:437-450`); native states both in the
                // paragraph's marker, which is where a list item's bullet is drawn.
                let item = builder.items.last_mut().ok_or(Failure::Unsupported)?;
                let marker = item.marker.as_mut().ok_or(Failure::Unsupported)?;
                let box_ = if checked { "\u{2611}" } else { "\u{2610}" };
                *marker = format!("{marker} {box_}");
            }
            Event::Html(text) | Event::InlineHtml(text) => {
                // The reference runs no HTML parser: raw HTML stays literal text in the flow
                // (`ui-primitives/src/markdown/render.tsx:333-335`). A block-level event arrives
                // outside any inline, so it opens the paragraph it will be drawn in, and its
                // trailing line break is layout rather than text.
                if builder.inline.is_none() && builder.alt.is_none() {
                    builder.begin_inline(None, true)?;
                    builder.text(text.trim_end_matches('\n'), false)?;
                } else {
                    builder.text(&text, false)?;
                }
            }
            Event::FootnoteReference(name) => {
                builder.text(&format!("[{}]", crate::reducer::bounded(&name, 64)), false)?;
            }
            Event::SoftBreak | Event::HardBreak => builder.text("\n", false)?,
            Event::Rule => {
                // A rule inside a container keeps that container's indent and rule, so the line it
                // draws is measured inside them (`render.tsx:290-291`).
                builder.flush_synthetic()?;
                builder.push(Block::Rule {
                    indent: builder.indent() + u8::from(!builder.lists.is_empty()),
                    quote: builder.quotes != 0,
                    quote_first: builder.quotes != 0
                        && builder.containers.first() == Some(&Container::Quote),
                })?;
            } // Every `Event` variant is named above: a new one in a parser upgrade must be decided
              // here rather than swallowed, so the match is exhaustive instead of falling through.
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
        let marker = self
            .pending_marker
            .take()
            .or_else(|| self.items.last_mut().and_then(|item| item.marker.take()));
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
            // A quote that opened before any list keeps its bar outside the list indents.
            quote_first: self.quotes != 0 && self.containers.first() == Some(&Container::Quote),
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
        let struck = self.struck != 0;
        if let Some(last) = inline.spans.last_mut()
            && last.bold == bold
            && last.italic == italic
            && last.struck == struck
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
            struck,
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
                    indent: inline.indent,
                    quote: inline.quote,
                    quote_first: inline.quote_first,
                }
            } else {
                Block::Paragraph {
                    spans: inline.spans,
                    indent: inline.indent,
                    marker: inline.marker,
                    quote: inline.quote,
                    quote_first: inline.quote_first,
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
                // A heading keeps attributes nowhere, so a decorated one still fails closed; its
                // containers are carried by the inline like a paragraph's (`render.tsx:279-281`).
                if id.is_some() || !classes.is_empty() || !attrs.is_empty() {
                    return Err(Failure::Unsupported);
                }
                self.begin_inline(Some(level as u8), true)?;
            }
            // A block-level HTML fragment's own container: its text is drawn literally by the
            // `Event::Html` arm, so the container adds nothing.
            Tag::HtmlBlock => {}
            Tag::BlockQuote(None) => {
                // A quote may sit inside a list item: the paragraph records which of the two
                // opened first, so the bar and the indents keep the reference's nesting order.
                self.flush_synthetic()?;
                self.quotes += 1;
                self.containers.push(Container::Quote);
            }
            Tag::List(next) => {
                // A list may sit inside a quote, for the same reason.
                self.flush_synthetic()?;
                self.preserve_item_marker()?;
                self.lists.push(List { next });
                self.containers.push(Container::List);
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
                    quote: self.quotes != 0,
                    quote_first: self.quotes != 0
                        && self.containers.first() == Some(&Container::Quote),
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
            Tag::Strikethrough => {
                self.ensure_inline()?;
                self.struck += 1;
            }
            Tag::Link { dest_url, .. } => {
                // A link title is a tooltip in the reference's own renderer, never visible copy
                // (`render.tsx:333-337`): dropping it changes no pixel, and refusing it would drop
                // the whole reply to plain text.
                self.ensure_inline()?;
                destination = Some(dest_url.into_string());
            }
            Tag::Image { dest_url, .. } => {
                // Same for an image's title: the reference shows the image, and native its alt
                // text; neither draws the title. A nested image keeps accumulating into the same
                // alt run — its own alt text is part of the outer one — so it is not refused.
                self.ensure_inline()?;
                if self.alt.is_none() {
                    destination = Some(dest_url.into_string());
                    self.alt = Some(String::new());
                }
                self.image_depth += 1;
            }
            Tag::Table(_) => {
                // A table keeps its container's provenance on its own block now; what still has
                // nowhere to live is a table inside a table or inside inline text.
                if self.table.is_some() || self.inline.is_some() {
                    return Err(Failure::Unsupported);
                }
                self.table = Some(Table {
                    quote: self.quotes != 0,
                    quote_first: self.quotes != 0
                        && self.containers.first() == Some(&Container::Quote),
                    ..Table::default()
                });
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
                    quote_first: false,
                    spans: Vec::new(),
                });
            }
            // A footnote definition labels the paragraph it contains with the reference's `[n]`.
            // The definition itself is a container, not a paragraph, so the marker waits for the
            // paragraph inside it.
            Tag::FootnoteDefinition(name) => {
                let marker = format!("[{}] ", crate::reducer::bounded(&name, 64));
                self.charge(marker.len())?;
                self.pending_marker = Some(marker);
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
                    quote: code.quote,
                    quote_first: code.quote_first,
                })?;
            }
            // A footnote definition is a container: its paragraph has already flushed itself.
            TagEnd::FootnoteDefinition => {
                self.flush_synthetic()?;
                self.pending_marker = None;
            }
            TagEnd::BlockQuote(None) => {
                self.flush_synthetic()?;
                self.quotes = self.quotes.checked_sub(1).ok_or(Failure::Unsupported)?;
                if self.containers.pop() != Some(Container::Quote) {
                    return Err(Failure::Unsupported);
                }
            }
            TagEnd::List(_) => {
                self.flush_synthetic()?;
                self.lists.pop().ok_or(Failure::Unsupported)?;
                if self.containers.pop() != Some(Container::List) {
                    return Err(Failure::Unsupported);
                }
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
            TagEnd::Strikethrough => {
                self.struck = self.struck.checked_sub(1).ok_or(Failure::Unsupported)?
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
                // Only the outermost image closes the run; a nested one just leaves it open.
                self.image_depth = self
                    .image_depth
                    .checked_sub(1)
                    .ok_or(Failure::Unsupported)?;
                if self.image_depth != 0 {
                    // `end()` already popped this image's own frame.
                    return Ok(());
                }
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
                    struck: false,
                    code: false,
                    alt: true,
                });
            }
            // The block's own fragment has been drawn as one paragraph; close it here.
            TagEnd::HtmlBlock => self.flush()?,
            TagEnd::Table => {
                let table = self.table.take().ok_or(Failure::Unsupported)?;
                if table.header.is_empty() || table.head {
                    return Err(Failure::Unsupported);
                }
                // A table has no marker column, so every list level indents it — the same rule a
                // fenced block follows (`Tag::CodeBlock`).
                self.push(Block::Table {
                    header: table.header,
                    rows: table.rows,
                    indent: self.lists.len() as u8,
                    quote: table.quote,
                    quote_first: table.quote_first,
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
                ..
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
                ..
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
            Block::Heading { level, spans, .. } => {
                assert_eq!(*level, 1);
                assert_eq!(joined(spans), "Héllo & 世界");
            }
            _ => panic!("expected heading"),
        }
        let (spans, indent, marker, quote) = paragraph(&doc.blocks[1]);
        assert_eq!(joined(spans), "plain\nsoft\nhard");
        assert_eq!((indent, marker, quote), (0, None, false));
        assert!(matches!(doc.blocks[2], Block::Rule { .. }));
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
    fn an_image_inside_an_image_joins_the_outer_alt_run() {
        // The reference's alt attribute is a plain string, so a nested image contributes its own
        // alt text to the outer run instead of being refused (`render.tsx:640-653`).
        let doc = document("before ![outer ![inner](a.png) tail](b.png) after");
        let spans = paragraph(&doc.blocks[0]).0;
        let alt: Vec<&Span> = spans.iter().filter(|span| span.alt).collect();
        assert_eq!(alt.len(), 1, "one alt run, not two images");
        assert!(alt[0].text.contains("outer"), "{}", alt[0].text);
        assert!(alt[0].text.contains("inner"), "{}", alt[0].text);
        assert!(alt[0].text.contains("tail"), "{}", alt[0].text);
        assert!(
            !alt[0].text.contains("(b.png)"),
            "the outer destination is not text"
        );
        // A whole source of only nested images still parses.
        assert!(parse("![![x](a)](b)").is_ok());
    }

    #[test]
    fn a_code_block_inside_a_container_keeps_its_rule_and_indent() {
        // The reference nests a fenced block in the same containers as any block
        // (`render.tsx:281-289`); a block inside a list item is indented at every level because a
        // code card has no marker column.
        let quoted = document("> ```rust\n> let x = 1;\n> ```\n");
        match &quoted.blocks[0] {
            Block::Code {
                quote,
                quote_first,
                indent,
                language,
                text,
            } => {
                assert!(*quote, "the block is quoted");
                assert!(*quote_first);
                assert_eq!(*indent, 0);
                assert_eq!(language.as_deref(), Some("rust"));
                assert_eq!(text, "let x = 1;\n");
            }
            _ => panic!("a code block"),
        }
        // An item's marker paragraph comes first, so the block is found rather than indexed.
        let listed = document("- ```\n  let y = 2;\n  ```\n");
        let listed = listed
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Code { quote, indent, .. } => Some((*quote, *indent)),
                _ => None,
            })
            .expect("the item's code block");
        assert_eq!(listed, (false, 1), "not a quote, one level in");
        // A plain block keeps zero provenance.
        match &document("```\nx\n```\n").blocks[0] {
            Block::Code {
                quote, quote_first, ..
            } => assert_eq!((*quote, *quote_first), (false, false)),
            _ => panic!("a code block"),
        }
    }

    #[test]
    fn a_table_inside_a_container_keeps_its_rule_and_indent() {
        // The reference nests a table in the same containers as any block, and a table inside a
        // blockquote fills the column instead of scrolling (`render.tsx:281-289`).
        let quoted = document("> | a | b |\n> |---|---|\n> | c | d |\n");
        match &quoted.blocks[0] {
            Block::Table {
                quote,
                quote_first,
                indent,
                header,
                ..
            } => {
                assert!(*quote, "the table is quoted");
                assert!(*quote_first);
                assert_eq!(*indent, 0);
                assert_eq!(header.len(), 2, "the header survives the container");
            }
            _ => panic!("a table"),
        }
        let listed = document("- | a | b |\n  |---|---|\n  | c | d |\n");
        match &listed.blocks[0] {
            Block::Table {
                quote,
                quote_first,
                indent,
                ..
            } => {
                assert!(!*quote, "a list item is not a quote");
                assert!(!*quote_first);
                assert_eq!(*indent, 1, "the item's text sits one level in");
            }
            _ => panic!("a table"),
        }
        // A plain table keeps zero provenance.
        match &document("| a |\n|---|\n| b |\n").blocks[0] {
            Block::Table {
                quote, quote_first, ..
            } => assert_eq!((*quote, *quote_first), (false, false)),
            _ => panic!("a table"),
        }
    }

    #[test]
    fn a_rule_inside_a_container_keeps_its_rule_and_indent() {
        // The reference draws `<hr>` inside the same blockquote/list container as any block
        // (`render.tsx:290-291`), so the line it draws shrinks by that container's inset.
        let quoted = document("> ---\n");
        match &quoted.blocks[0] {
            Block::Rule {
                indent,
                quote,
                quote_first,
            } => {
                assert!(*quote, "the rule sits in the quote");
                assert!(*quote_first);
                assert_eq!(*indent, 0);
            }
            _ => panic!("a rule"),
        }
        let listed = document("- ***\n");
        let rule = listed
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Rule {
                    indent,
                    quote,
                    quote_first,
                } => Some((*indent, *quote, *quote_first)),
                _ => None,
            })
            .expect("the item's rule");
        assert_eq!(rule, (1, false, false), "inside the item, one indent deep");
        // A plain rule keeps zero provenance.
        match &document("---\n").blocks[0] {
            Block::Rule {
                indent,
                quote,
                quote_first,
            } => assert_eq!((*indent, *quote, *quote_first), (0, false, false)),
            _ => panic!("a rule"),
        }
    }

    #[test]
    fn a_heading_inside_a_container_keeps_its_rule_and_indent() {
        // The reference nests a heading in the same blockquote/list containers as a paragraph
        // (`render.tsx:279-289`), so native records the same provenance on it.
        let quoted = document("> ## PUBLIC headed quote\n");
        match &quoted.blocks[0] {
            Block::Heading {
                level,
                quote,
                quote_first,
                indent,
                ..
            } => {
                assert_eq!(*level, 2, "the heading level survives");
                assert!(*quote, "the quote is recorded");
                assert!(*quote_first);
                assert_eq!(*indent, 0);
            }
            _ => panic!("a heading"),
        }
        let listed = document("- ## PUBLIC headed item\n");
        match &listed.blocks[0] {
            Block::Heading { indent, quote, .. } => {
                assert!(!*quote, "a list item is not a quote");
                assert_eq!(*indent, 0, "the item's own marker carries its level");
            }
            _ => panic!("a heading"),
        }
        // Plain headings keep zero provenance.
        match &document("## PUBLIC plain\n").blocks[0] {
            Block::Heading {
                indent,
                quote,
                quote_first,
                ..
            } => {
                assert_eq!((*indent, *quote, *quote_first), (0, false, false));
            }
            _ => panic!("a heading"),
        }
    }

    #[test]
    fn a_quote_and_a_list_compose_in_the_order_they_opened() {
        // `> - item` opens the quote first, so its rule sits outside the list's indent
        // (`render.tsx:281-289,425-460`).
        let quoted_list = document("> - PUBLIC item\n");
        let block = &quoted_list.blocks[0];
        let (_, indent, marker, quote) = paragraph(block);
        assert_eq!((indent, marker, quote), (0, Some("•"), true));
        let quote_first = match block {
            Block::Paragraph { quote_first, .. } => *quote_first,
            _ => panic!("a paragraph"),
        };
        assert!(quote_first, "the quote opened before the list");
        // `- > item` opens the list first: the item's own marker stands before the rule.
        let list_quote = document("- > PUBLIC item\n");
        let block = &list_quote.blocks[0];
        let (_, _, marker, quote) = paragraph(block);
        assert!(quote);
        assert_eq!(
            marker,
            Some("•"),
            "the item keeps its marker outside the quote"
        );
        let quote_first = match block {
            Block::Paragraph { quote_first, .. } => *quote_first,
            _ => panic!("a paragraph"),
        };
        assert!(!quote_first, "the list opened before the quote");
        // A plain quote and a plain list still record their own shape.
        let plain_quote = match &document("> PUBLIC\n").blocks[0] {
            Block::Paragraph { quote_first, .. } => *quote_first,
            _ => panic!("a paragraph"),
        };
        assert!(plain_quote, "a bare quote is quote-first by construction");
    }

    #[test]
    fn raw_html_stays_literal_text_in_the_flow() {
        // The reference has no HTML parser, so `<b>` is shown as those characters rather than
        // failing the document or being interpreted (`render.tsx:333-335`).
        let doc = parse("PUBLIC <b>bold?</b> text <br> next").unwrap_or_else(|_| panic!("valid"));
        let spans = paragraph(&doc.blocks[0]).0;
        let text: String = spans.iter().map(|span| span.text.as_str()).collect();
        assert_eq!(text, "PUBLIC <b>bold?</b> text <br> next");
        assert!(
            spans.iter().all(|span| !span.bold),
            "no span is treated as markup"
        );
        // A block-level fragment opens its own paragraph instead of failing.
        let doc = parse("<div align=\"center\">PUBLIC</div>\n\ntext\n")
            .unwrap_or_else(|_| panic!("valid block html"));
        assert_eq!(doc.blocks.len(), 2);
        let first: String = paragraph(&doc.blocks[0])
            .0
            .iter()
            .map(|span| span.text.as_str())
            .collect();
        assert_eq!(first, "<div align=\"center\">PUBLIC</div>");
    }

    #[test]
    fn a_struck_run_is_its_own_span_and_never_merges_with_the_plain_text() {
        // The reference renders `~~…~~` as `<del>` (`render.tsx:298-301`), so the run keeps its
        // own style instead of collapsing into the neighbouring text.
        let doc = parse("PUBLIC ~~gone~~ stays").unwrap_or_else(|_| panic!("valid markdown"));
        let spans = paragraph(&doc.blocks[0]).0;
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].text, "gone");
        assert!(spans[1].struck);
        assert!(!spans[0].struck && !spans[2].struck);
        // Balance: an unbalanced end is refused rather than silently drawn struck.
        let events = vec![
            Event::Start(Tag::Paragraph),
            Event::End(TagEnd::Strikethrough),
        ];
        assert!(matches!(build(events), Err(Failure::Unsupported)));
    }

    #[test]
    fn a_task_item_states_the_box_beside_its_own_bullet() {
        // The reference keeps the item's list marker and prepends a disabled checkbox
        // (`render.tsx:437-450`); native draws both in the item's marker.
        let doc =
            parse("- [x] done\n- [ ] waiting\n").unwrap_or_else(|_| panic!("valid task list"));
        let markers: Vec<String> = doc
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph { marker, .. } => marker.clone(),
                _ => None,
            })
            .collect();
        assert_eq!(
            markers,
            vec!["• \u{2611}".to_string(), "• \u{2610}".to_string()]
        );
        // A task marker outside an item is the source's contradiction.
        let events = vec![
            Event::Start(Tag::Paragraph),
            Event::TaskListMarker(true),
            Event::End(TagEnd::Paragraph),
        ];
        assert!(matches!(build(events), Err(Failure::Unsupported)));
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
        // A title is a tooltip, never visible copy: it is dropped rather than failing the
        // reply, and the destination still reads as inert text (`render.tsx:333-337`).
        let titled = document("[label](https://example.invalid \"title\")");
        assert_eq!(
            joined(paragraph(&titled.blocks[0]).0),
            "label (https://example.invalid)"
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
    fn mixed_quote_list_ancestries_keep_their_opening_order_instead_of_collapsing() {
        // Round 530 composes the two containers instead of refusing them: the paragraph records
        // which opened first, so the rule and the indents keep the reference's nesting
        // (`render.tsx:281-289,425-460`). More than two containers of mixed kind still collapse
        // to one bit, which this test states rather than hides.
        for (source, quote_first, marker) in [
            ("> - text", true, Some("•")),
            ("- > text", false, Some("•")),
            ("> 7. text", true, Some("7.")),
            ("7. > text", false, Some("7.")),
        ] {
            let doc = document(source);
            let block = &doc.blocks[0];
            let (_, _, got_marker, quote) = paragraph(block);
            assert!(quote, "{source}");
            assert_eq!(got_marker, marker, "{source}");
            let first = match block {
                Block::Paragraph { quote_first, .. } => *quote_first,
                _ => panic!("a paragraph"),
            };
            assert_eq!(first, quote_first, "{source}");
        }
        // A third level inside a nested quote still draws: the plan keeps a paragraph per block.
        let deeper = document("> - text\n>   > nested");
        assert_eq!(deeper.blocks.len(), 2);
        assert!(
            paragraph(&deeper.blocks[1]).3,
            "the nested quote stays quoted"
        );
        // A preceding valid block is no longer a reason to refuse the whole source.
        let doc = document("before\n\n> - text\n\nafter");
        assert_eq!(doc.blocks.len(), 3);
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
        // A titled image keeps its alt run: the title is a tooltip neither renderer draws.
        let titled = document("![PUBLIC alt](u \"title\")");
        let spans = paragraph(&titled.blocks[0]).0;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "PUBLIC alt");
        assert!(spans[0].alt);
    }
    #[test]
    fn tables_keep_their_header_inline_runs_and_bounds() {
        let document = document("| a | *b* |\n|---|---|\n| `c` | d |\n| e | f |\n");
        let (header, rows, indent) = match &document.blocks[0] {
            Block::Table {
                header,
                rows,
                indent,
                ..
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
        // A table keeps its container's provenance now, so it parses inside a quote; its rows
        // stay bounded like the other block lists. (`quoted` is the parsed document here, because
        // the helper's name is shadowed above.)
        let quoted = crate::ui::detail_panel::formatted::parse("> | a |\n> |---|\n> | b |\n")
            .unwrap_or_else(|_| panic!("valid quoted table"));
        match &quoted.blocks[0] {
            Block::Table {
                quote, quote_first, ..
            } => {
                assert!(*quote, "the table is quoted");
                assert!(*quote_first);
            }
            _ => panic!("a table"),
        }
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
        // Round 538 closed the renderer's last structural refusal: every plain Markdown construct
        // now has a representation, including an image nested in an image, whose alt text joins the
        // outer run. What still fails a whole source is a budget, and it fails it everywhere.
        for source in [
            "before ![![nested](a.png)](b.png) after",
            "> - | a |\n>   |---|\n>   | b |",
            "> ```\n> code\n> ```",
        ] {
            assert!(parse(source).is_ok(), "{source:?}");
        }
        let oversized = "x".repeat(INPUT_BYTES + 1);
        failure(&oversized, Failure::Bytes);
        failure(&format!("before\n\n{oversized}"), Failure::Bytes);
        // Escaped HTML and HTML in code remain inert text, not unsupported nodes.
        assert_eq!(
            joined(paragraph(&document("\\<b> safe").blocks[0]).0),
            "<b> safe"
        );
        // Raw HTML is literal text with its markup intact.
        assert_eq!(
            joined(paragraph(&document("a <b>c</b>").blocks[0]).0),
            "a <b>c</b>"
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
            Event::InlineMath("x".into()),
            Event::DisplayMath("x".into()),
            Event::FootnoteReference("secret".into()),
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
        // A source past the byte budget still fails, and its diagnostic names only the variant: no
        // length, no bytes and none of the content reach it.
        let private_source = format!("PRIVATE_TOKEN {}", "x".repeat(INPUT_BYTES));
        let error = match parse(&private_source) {
            Err(error) => error,
            Ok(_) => panic!("expected a byte-budget failure"),
        };
        assert_eq!(format!("{error:?}"), "Bytes");
        let copied = error;
        assert_eq!(copied, error);
    }
}

#[cfg(test)]
mod extension_tests {
    //! Footnotes and math: the two markdown extensions the reference enables.
    use super::*;

    fn texts(blocks: &[Block]) -> Vec<String> {
        blocks
            .iter()
            .map(|block| match block {
                Block::Paragraph { spans, marker, .. } => format!(
                    "{}{}",
                    marker.clone().unwrap_or_default(),
                    spans
                        .iter()
                        .map(|span| span.text.as_str())
                        .collect::<String>()
                ),
                Block::Code { language, text, .. } => {
                    format!("```{} {text}", language.clone().unwrap_or_default())
                }
                _ => "<other>".to_owned(),
            })
            .collect()
    }

    #[test]
    fn a_footnote_keeps_its_reference_and_its_definition() {
        let doc = parse("Body text[^1].\n\n[^1]: The note.\n").expect("a documented footnote");
        let texts = texts(&doc.blocks);
        assert_eq!(texts[0], "Body text[1].", "{texts:?}");
        assert!(
            texts
                .iter()
                .any(|text| text.starts_with("[1] ") && text.contains("The note.")),
            "{texts:?}"
        );
    }

    #[test]
    fn math_is_drawn_as_its_source() {
        let doc = parse("Inline $a^2$ and a fence:\n\n```math\nx = 1\n```\n").expect("math");
        let rendered = texts(&doc.blocks);
        assert_eq!(rendered[0], "Inline a^2 and a fence:", "{rendered:?}");
        // A fenced `math` block is a code block like any other, so it keeps the fence's newline.
        assert_eq!(rendered[1], "```math x = 1\n", "{rendered:?}");
        // Display math arrives inside its paragraph, so it draws as code text there.
        let doc = parse("$$x = 1$$\n").expect("display math");
        assert_eq!(texts(&doc.blocks), vec!["x = 1".to_owned()]);
    }

    #[test]
    fn a_document_without_either_extension_is_unchanged() {
        let doc = parse("Plain **bold** text.\n").unwrap();
        assert_eq!(texts(&doc.blocks), vec!["Plain bold text.".to_owned()]);
        // A nested image flattens into its outer alt run, and raw HTML is drawn literally.
        assert!(parse("![![nested](a.png)](b.png)").is_ok());
        assert!(parse("<div>html</div>\n").is_ok());
    }
}

#[cfg(test)]
mod unsupported_probe {
    use super::*;

    #[test]
    fn probe_unsupported() {
        for source in [
            "> ```\n> code\n> ```",
            ">     code\n",
            "- > ```\n  > code\n  > ```",
        ] {
            println!("{source:?} => {:?}", parse(source).map(|d| d.blocks.len()));
        }
    }
}
