//! Bounded, inert native reply presentation. Source text never becomes a URI or command.
use super::{FONT, MUTED, Message, TEXT, design, detail_panel::formatted, label};
use iced::advanced::text::{self as native, Paragraph as _};
use iced::widget::{Column, button, container, rich_text, row, scrollable, span, text};
use iced::{Element, Font, Length, Pixels, Size, alignment};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

const MONO: Font = Font::with_name("DejaVu Sans Mono");
type Paragraph = <iced::Renderer as native::Renderer>::Paragraph;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub generation: u64,
    pub key: u64,
    pub revision: u64,
}

pub struct Prepared {
    pub ticket: Option<Ticket>,
    pub reasoning: String,
    pub answer: String,
    document: Option<formatted::Document>,
    widths: RefCell<Vec<(u32, bool, f32)>>,
    /// The content-size deltas this reply's sizes were prepared at. The cache that
    /// built it clears whenever either changes, so a reply can never be drawn or
    /// measured at a size other than the one it was prepared for.
    delta: f32,
    delta_secondary: f32,
}
impl Prepared {
    /// The body text size at this reply's content size.
    fn body_size(&self) -> f32 {
        design::BODY_SIZE + self.delta
    }
    /// The reasoning text size, one step under the body.
    fn reasoning_size(&self) -> f32 {
        14.0 + self.delta_secondary
    }
    /// The mono size for recorded bodies and code blocks.
    fn mono_size(&self) -> f32 {
        13.0 + self.delta_secondary
    }
    /// The inset the expanded thinking body sits at, under the disclosure title
    /// rather than under its glyph. The web writes it
    /// `calc(22px + var(--dsh-content-font-delta))` (`ReasoningRow.module.css`),
    /// so it tracks the body size the way the row heights do.
    fn content_inset(&self) -> f32 {
        design::CONTENT_INSET + self.delta
    }
    pub fn code(&self, index: usize) -> Option<&str> {
        match self.document.as_ref()?.blocks.get(index)? {
            formatted::Block::Code { text, .. } => Some(text),
            _ => None,
        }
    }
    /// True when this reply renders an answer, not only thinking or a recorded
    /// tool call. An answerless reply keeps its disclosure and nothing else.
    /// An empty answer parses to an empty document, so the text is the test.
    pub fn has_answer(&self) -> bool {
        !self.answer.trim().is_empty()
    }
    pub fn body_height(&self, width: f32, expanded: bool) -> f32 {
        let width = bounded_width(width);
        if let Some((_, _, height)) = self
            .widths
            .borrow()
            .iter()
            .find(|(w, e, _)| *w == width.to_bits() && *e == expanded)
        {
            return *height;
        }
        let has_answer = self.has_answer();
        // An answerless reply reserves nothing for its answer. The web draws only
        // the disclosure there, and an empty document still measured 20px, which
        // left a blank gap under the disclosure that no render filled.
        let mut height = if !has_answer {
            0.0
        } else if let Some(document) = &self.document {
            document_height(document, width, self.delta, self.delta_secondary)
        } else {
            plain_height(
                &self.answer,
                width,
                self.body_size(),
                design::body_line(self.delta),
                FONT,
                false,
            )
        };
        if !self.reasoning.is_empty() {
            // The row, then the flow gap that separates this block from the next.
            height += REASONING_ROW + self.delta + design::BLOCK_GAP;
            if expanded {
                // The body is drawn indented, so its height has to be measured at
                // the same narrower width the offset calculation uses, and it
                // carries the padding above and below that the web's `.thinkBody`
                // sets.
                height += plain_height(
                    &self.reasoning,
                    bounded_width(width - self.content_inset()),
                    self.reasoning_size(),
                    design::compact_line(self.delta_secondary),
                    FONT,
                    false,
                ) + 2.0 * REASONING_BODY_PAD;
            }
        }
        // Only real answer content reserves the minimum bubble height; reserving
        // it for an answerless reply leaves a blank gap under the disclosure.
        let height = if has_answer { height.max(20.0) } else { height };
        let mut widths = self.widths.borrow_mut();
        if widths.len() >= 4 {
            widths.remove(0);
        }
        widths.push((width.to_bits(), expanded, height));
        height
    }
    /// Search rendered paragraph text, not the Markdown delimiters that produced it.
    /// Collapsed backend-supplied reasoning remains searchable so callers can reveal it.
    pub fn matches(&self, query: &str) -> bool {
        use super::conversation_search::{MAX_QUERY_BYTES, contains};
        if query.is_empty() || query.len() > MAX_QUERY_BYTES {
            return false;
        }
        contains(&self.reasoning, query)
            || self.document.as_ref().map_or_else(
                || contains(&self.answer, query),
                |document| {
                    document.blocks.iter().any(|block| match block {
                        formatted::Block::Paragraph { spans, .. }
                        | formatted::Block::Heading { spans, .. } => {
                            joined_text(spans.iter().map(|part| part.text.as_str()))
                                .is_some_and(|source| contains(&source, query))
                        }
                        formatted::Block::Code { text, .. } => contains(text, query),
                        formatted::Block::Rule => false,
                    })
                },
            )
    }
    /// Body-relative native glyph position; hidden thoughts require expansion first.
    pub fn first_match_offset(&self, width: f32, expanded: bool, query: &str) -> f32 {
        let width = bounded_width(width);
        if expanded
            && let Some(offset) = first_offset(
                vec![span(self.reasoning.clone()).color(MUTED)],
                query,
                bounded_width(width - design::CONTENT_INSET),
                self.reasoning_size(),
                design::compact_line(self.delta_secondary),
                FONT,
                false,
            )
        {
            return self.content_inset() + offset;
        }
        let mut y = if self.reasoning.is_empty() {
            0.0
        } else if expanded {
            REASONING_ROW
                + REASONING_BODY_PAD
                + self.delta
                + plain_height(
                    &self.reasoning,
                    bounded_width(width - self.content_inset()),
                    self.reasoning_size(),
                    design::compact_line(self.delta_secondary),
                    FONT,
                    false,
                )
        } else {
            REASONING_ROW + self.delta
        };
        if let Some(document) = &self.document {
            for block in &document.blocks {
                let found = match block {
                    formatted::Block::Paragraph {
                        spans,
                        indent,
                        marker,
                        quote,
                    } => first_offset(
                        inline_runs(spans),
                        query,
                        width
                            - *indent as f32 * 18.0
                            - if marker.is_some() { 36.0 } else { 0.0 }
                            - if *quote { 20.0 } else { 0.0 },
                        self.body_size(),
                        design::body_line(self.delta),
                        FONT,
                        false,
                    ),
                    formatted::Block::Heading { spans, level } => first_offset(
                        inline_runs(spans),
                        query,
                        width,
                        heading_size(*level) as f32 + self.delta,
                        design::body_line(self.delta),
                        FONT,
                        false,
                    ),
                    formatted::Block::Code { text, language, .. } => first_offset(
                        highlight(text, language.as_deref()),
                        query,
                        width,
                        self.mono_size(),
                        design::body_line(self.delta),
                        MONO,
                        true,
                    )
                    .map(|offset| offset + 49.0),
                    formatted::Block::Rule => None,
                };
                if let Some(offset) = found {
                    return (y + offset).clamp(0.0, self.body_height(width, expanded));
                }
                y += block_height(block, width, self.delta, self.delta_secondary)
                    + design::BLOCK_GAP;
            }
            0.0
        } else {
            first_offset(
                vec![span(self.answer.clone()).color(TEXT)],
                query,
                width,
                self.body_size(),
                design::body_line(self.delta),
                FONT,
                false,
            )
            .map_or(0.0, |offset| y + offset)
        }
    }
    /// Non-search view. The transcript always renders through the search-aware
    /// one, so only this module's tests read it.
    #[cfg(test)]
    pub fn view(&self, width: f32, expanded: bool) -> Element<'static, Message> {
        self.view_search(width, expanded, "")
    }
    pub fn view_search(
        &self,
        width: f32,
        expanded: bool,
        query: &str,
    ) -> Element<'static, Message> {
        let width = bounded_width(width);
        let mut body = Column::new().width(Length::Fill);
        if !self.reasoning.is_empty() {
            body = body.push(
                button(
                    label(
                        if expanded {
                            "▾ Thoughts"
                        } else {
                            "▸ Thoughts"
                        },
                        MUTED,
                    )
                    .size(12),
                )
                .on_press_maybe(self.ticket.map(Message::ToggleThoughts))
                .style(design::ghost)
                .padding([3, 0])
                .height(REASONING_ROW + self.delta),
            );
            if expanded {
                // Indent under the title, as the web does, and measure at the same
                // reduced width: the search offset below already assumed this inset.
                body = body.push(iced::widget::Space::new().height(REASONING_BODY_PAD));
                let inner = bounded_width(width - self.content_inset());
                body = body.push(
                    container(plain_view(
                        &self.reasoning,
                        query,
                        self.reasoning_size().round() as u32,
                        design::compact_line(self.delta_secondary),
                        MUTED,
                    ))
                    .padding(iced::Padding {
                        top: 0.0,
                        right: 0.0,
                        bottom: 0.0,
                        left: self.content_inset(),
                    })
                    .height(plain_height(
                        &self.reasoning,
                        inner,
                        self.reasoning_size(),
                        design::compact_line(self.delta_secondary),
                        FONT,
                        false,
                    ))
                    .clip(true),
                );
                body = body.push(iced::widget::Space::new().height(REASONING_BODY_PAD));
            }
            body = body.push(iced::widget::Space::new().height(design::BLOCK_GAP));
        }
        if !self.has_answer() {
            return body.into();
        }
        if let Some(document) = &self.document {
            body = body.push(document_view(
                document,
                width,
                self.ticket,
                query,
                self.delta,
                self.delta_secondary,
            ));
        } else {
            body = body.push(plain_view(
                &self.answer,
                query,
                self.body_size().round() as u32,
                design::body_line(self.delta),
                TEXT,
            ));
        }
        body.into()
    }
}

#[derive(Default)]
pub struct Cache {
    entries: RefCell<BTreeMap<u64, Rc<Prepared>>>,
    revision: Cell<u64>,
    /// The content-size deltas every prepared reply was built at. Changing either
    /// invalidates the cache: a prepared reply owns measured heights and widget
    /// sizes for one pair of deltas only.
    delta: Cell<f32>,
    delta_secondary: Cell<f32>,
}
impl Cache {
    /// Adopt the content-size deltas, dropping replies prepared at the old ones.
    pub fn set_content_delta(&self, delta: f32, delta_secondary: f32) {
        let delta = if delta.is_finite() { delta } else { 0.0 };
        let delta_secondary = if delta_secondary.is_finite() {
            delta_secondary
        } else {
            0.0
        };
        if self.delta.get() != delta || self.delta_secondary.get() != delta_secondary {
            self.delta.set(delta);
            self.delta_secondary.set(delta_secondary);
            self.entries.borrow_mut().clear();
        }
    }
    pub fn get(&self, generation: u64, key: u64) -> Option<Rc<Prepared>> {
        self.entries
            .borrow()
            .get(&key)
            .filter(|entry| {
                entry
                    .ticket
                    .is_some_and(|ticket| ticket.generation == generation)
            })
            .cloned()
    }
    pub fn clear(&self) {
        self.entries.borrow_mut().clear();
    }
    pub fn clear_live(&self) {
        self.entries.borrow_mut().remove(&u64::MAX);
    }
    pub fn prepare(
        &self,
        generation: u64,
        key: u64,
        reasoning: String,
        answer: String,
    ) -> Rc<Prepared> {
        if let Some(entry) = self.entries.borrow().get(&key)
            && entry
                .ticket
                .is_some_and(|ticket| ticket.generation == generation)
            && entry.reasoning == reasoning
            && entry.answer == answer
        {
            return entry.clone();
        }
        let revision = self.revision.get().checked_add(1);
        if let Some(revision) = revision {
            self.revision.set(revision);
        }
        let document = formatted::parse(&answer).ok();
        let prepared = Rc::new(Prepared {
            ticket: revision.map(|revision| Ticket {
                generation,
                key,
                revision,
            }),
            reasoning,
            answer,
            document,
            widths: RefCell::new(Vec::new()),
            delta: self.delta.get(),
            delta_secondary: self.delta_secondary.get(),
        });
        let mut entries = self.entries.borrow_mut();
        if !entries.contains_key(&key) && entries.len() >= 4097 {
            entries.pop_first();
        }
        entries.insert(key, prepared.clone());
        prepared
    }
    pub fn retain(&self, keys: impl IntoIterator<Item = u64>) {
        let mut entries = self.entries.borrow_mut();
        let retained: std::collections::BTreeSet<_> = keys
            .into_iter()
            .filter(|key| entries.contains_key(key))
            .collect();
        entries.retain(|key, _| retained.contains(key));
    }
}

type Run = native::Span<'static, (), Font>;

/// `max().min()` rather than `clamp`: `f32::clamp` propagates NaN, while this
/// form answers a finite 1.0 for a non-finite width, which is what keeps a
/// degenerate measurement from reaching the layout as NaN.
#[allow(clippy::manual_clamp)]
fn bounded_width(width: f32) -> f32 {
    width.max(1.0).min(4096.0)
}

// Content-bearing native Span has Debug upstream; never format runs or paragraphs here.
fn joined_text<'a>(parts: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut source = String::new();
    for part in parts {
        if source.len().checked_add(part.len())? > super::conversation_search::MAX_SOURCE_BYTES {
            return None;
        }
        source.push_str(part);
    }
    Some(source)
}

/// Split only at literal match boundaries, retaining every byte and font/style attribute.
/// Joining before matching permits a match across adjacent bold/italic/code runs.
fn marked(runs: Vec<Run>, query: &str) -> Vec<Run> {
    if query.is_empty() || query.len() > super::conversation_search::MAX_QUERY_BYTES {
        return runs;
    }
    let Some(source) = joined_text(runs.iter().map(|run| run.text.as_ref())) else {
        return runs;
    };
    let ranges = super::conversation_search::ranges(&source, query);
    if ranges.is_empty() {
        return runs;
    }
    let gold = iced::Color {
        a: 0.28,
        ..design::GOLD
    };
    let mut result = Vec::new();
    let mut base = 0;
    let mut next_match = 0;
    for mut run in runs {
        // Clone only style metadata, not the whole paragraph for every small match.
        let text = std::mem::take(&mut run.text);
        let end = base + text.len();
        let mut cursor = base;
        while next_match < ranges.len() && ranges[next_match].end <= base {
            next_match += 1;
        }
        for range in ranges[next_match..]
            .iter()
            .take_while(|range| range.start < end)
        {
            let start = range.start.max(base);
            let stop = range.end.min(end);
            if cursor < start {
                let mut piece = run.clone();
                piece.text = text[cursor - base..start - base].to_owned().into();
                result.push(piece);
            }
            let mut piece = run.clone();
            piece.text = text[start - base..stop - base].to_owned().into();
            result.push(piece.background(gold).color(TEXT));
            cursor = stop;
        }
        if cursor < end || text.is_empty() {
            let mut piece = run;
            piece.text = text[cursor - base..].to_owned().into();
            result.push(piece);
        }
        base = end;
    }
    result
}

/// The floor a measured paragraph may not go under: the line box it is drawn in,
/// so one short line still reserves its full height.
fn line_box(size: f32, line: native::LineHeight) -> f32 {
    match line {
        native::LineHeight::Absolute(pixels) => pixels.0.max(size),
        native::LineHeight::Relative(factor) => size * factor,
    }
}
fn paragraph(
    runs: &[Run],
    width: f32,
    size: f32,
    line: native::LineHeight,
    font: Font,
    no_wrap: bool,
) -> Paragraph {
    Paragraph::with_spans(native::Text {
        content: runs,
        bounds: Size::new(bounded_width(width), f32::INFINITY),
        size: Pixels(size),
        line_height: line,
        font,
        align_x: native::Alignment::Default,
        align_y: alignment::Vertical::Top,
        shaping: native::Shaping::Advanced,
        wrapping: if no_wrap {
            native::Wrapping::None
        } else {
            native::Wrapping::Word
        },
    })
}

pub fn plain_height(
    source: &str,
    width: f32,
    size: f32,
    line: native::LineHeight,
    font: Font,
    no_wrap: bool,
) -> f32 {
    paragraph(&[span(source.to_owned())], width, size, line, font, no_wrap)
        .min_height()
        .max(line_box(size, line))
}

pub fn plain_view(
    source: &str,
    query: &str,
    size: u32,
    line: native::LineHeight,
    color: iced::Color,
) -> Element<'static, Message> {
    rich_text(marked(vec![span(source.to_owned()).color(color)], query))
        .font(FONT)
        .size(size)
        .line_height(line)
        .wrapping(text::Wrapping::Word)
        .width(Length::Fill)
        .into()
}

/// Measured height of a tool row's recorded body, shown inline when expanded.
/// Mono 13px and word-wrapped: a collapsed chat row cannot scroll sideways, so
/// wrapping keeps the whole recorded command or output reachable. The cap keeps
/// one long output from owning the viewport.
pub fn tool_body_height(
    source: &str,
    width: f32,
    delta_secondary: f32,
    line: native::LineHeight,
) -> f32 {
    plain_height(source, width, 13.0 + delta_secondary, line, MONO, false).min(TOOL_BODY_MAX)
}
/// The reasoning disclosure row's height. The web writes it as
/// `calc(24px + var(--dsh-content-font-delta))` (`ReasoningRow.module.css:8`),
/// which is exactly one body line box.
const REASONING_ROW: f32 = design::LINE_BODY;
/// The thinking body's own vertical padding, `padding: 4px 0 4px …` on the web's
/// `.thinkBody` (`ReasoningRow.module.css:75`).
const REASONING_BODY_PAD: f32 = 4.0;
/// Longest inline tool body, in native px; the render clips to it too.
pub const TOOL_BODY_MAX: f32 = 320.0;

/// Recorded tool body for the same inline expansion. Inert text only. The
/// horizontal padding matches the compact line; it adds no vertical height, so
/// the caller's measured body height stays exact.
pub fn tool_body_view(
    source: &str,
    height: f32,
    delta_secondary: f32,
    line: native::LineHeight,
) -> Element<'static, Message> {
    container(
        rich_text(vec![span::<(), Font>(source.to_owned()).color(TEXT)])
            .font(MONO)
            .size((13.0 + delta_secondary).round() as u32)
            .line_height(line)
            .wrapping(text::Wrapping::Word)
            .width(Length::Fill),
    )
    .padding([0, 8])
    .height(height)
    .clip(true)
    .into()
}

fn first_offset(
    runs: Vec<Run>,
    query: &str,
    width: f32,
    size: f32,
    line: native::LineHeight,
    font: Font,
    no_wrap: bool,
) -> Option<f32> {
    let runs = marked(runs, query);
    let first = runs.iter().position(|run| run.highlight.is_some())?;
    let paragraph = paragraph(&runs, width, size, line, font, no_wrap);
    // A newline or combining-only span may have no glyph of its own. Inspect
    // contiguous highlighted fragments, never jump past a gap to a later match.
    // A wholly glyphless first match retains a safe body-origin fallback.
    Some(
        runs.iter()
            .enumerate()
            .skip(first)
            .take_while(|(_, run)| run.highlight.is_some())
            .flat_map(|(index, _)| paragraph.span_bounds(index))
            .map(|bounds| bounds.y)
            .find(|y| y.is_finite())
            .unwrap_or(0.0)
            .clamp(0.0, paragraph.min_height().max(line_box(size, line))),
    )
}

/// Word-wrapped FONT paragraph offset for user previews and raw fallback rows.
/// An absent/over-budget match returns zero; matching is owned by `contains`.
pub fn plain_first_match_offset(
    source: &str,
    width: f32,
    size: f32,
    line: native::LineHeight,
    query: &str,
) -> f32 {
    first_offset(
        vec![span(source.to_owned())],
        query,
        width,
        size,
        line,
        FONT,
        false,
    )
    .unwrap_or(0.0)
}
fn inline_runs(parts: &[formatted::Span]) -> Vec<native::Span<'static, (), Font>> {
    parts
        .iter()
        .map(|part| {
            span(part.text.clone())
                .font(Font {
                    weight: if part.bold {
                        iced::font::Weight::Bold
                    } else {
                        iced::font::Weight::Normal
                    },
                    style: if part.italic {
                        iced::font::Style::Italic
                    } else {
                        iced::font::Style::Normal
                    },
                    ..if part.code { MONO } else { FONT }
                })
                .color(if part.code { design::FOAM } else { TEXT })
        })
        .collect()
}
fn inline_height(
    parts: &[formatted::Span],
    width: f32,
    size: f32,
    line: native::LineHeight,
) -> f32 {
    paragraph(&inline_runs(parts), width, size, line, FONT, false)
        .min_height()
        .max(line_box(size, line))
}
fn inline_view(
    parts: &[formatted::Span],
    size: u32,
    line: native::LineHeight,
    query: &str,
) -> Element<'static, Message> {
    rich_text(marked(inline_runs(parts), query))
        .font(FONT)
        .size(size)
        .line_height(line)
        .wrapping(text::Wrapping::Word)
        .width(Length::Fill)
        .into()
}
fn heading_size(level: u8) -> u32 {
    design::heading_size(level).round() as u32
}
fn block_height(block: &formatted::Block, width: f32, delta: f32, delta_secondary: f32) -> f32 {
    match block {
        formatted::Block::Paragraph {
            spans,
            indent,
            marker,
            quote,
        } => inline_height(
            spans,
            width
                - *indent as f32 * 18.0
                - if marker.is_some() { 36.0 } else { 0.0 }
                - if *quote { 20.0 } else { 0.0 },
            design::BODY_SIZE + delta,
            design::body_line(delta),
        ),
        formatted::Block::Heading { level, spans } => inline_height(
            spans,
            width,
            heading_size(*level) as f32 + delta,
            design::body_line(delta),
        ),
        formatted::Block::Code { text, .. } => {
            plain_height(
                text,
                width,
                13.0 + delta_secondary,
                design::body_line(delta),
                MONO,
                true,
            ) + 83.0
        }
        formatted::Block::Rule => 1.0,
    }
}
fn document_height(
    document: &formatted::Document,
    width: f32,
    delta: f32,
    delta_secondary: f32,
) -> f32 {
    if document.blocks.is_empty() {
        return 20.0;
    }
    document
        .blocks
        .iter()
        .map(|block| block_height(block, width, delta, delta_secondary))
        .sum::<f32>()
        + document.blocks.len().saturating_sub(1) as f32 * design::BLOCK_GAP
}
fn document_view(
    document: &formatted::Document,
    width: f32,
    ticket: Option<Ticket>,
    query: &str,
    delta: f32,
    delta_secondary: f32,
) -> Element<'static, Message> {
    let mut body = Column::new().spacing(design::BLOCK_GAP).width(Length::Fill);
    for (index, block) in document.blocks.iter().enumerate() {
        let content: Element<'static, Message> = match block {
            formatted::Block::Paragraph {
                spans,
                indent,
                marker,
                quote,
            } => {
                let mut line =
                    row![iced::widget::Space::new().width(*indent as f32 * 18.0)].spacing(0);
                if *quote {
                    line = line.push(container(label("│", design::FOAM)).width(20));
                }
                if let Some(marker) = marker {
                    line = line.push(
                        container(
                            label(marker.clone(), MUTED)
                                .size((design::BODY_SIZE + delta).round() as u32),
                        )
                        .width(36),
                    );
                }
                line.push(inline_view(
                    spans,
                    (design::BODY_SIZE + delta).round() as u32,
                    design::body_line(delta),
                    query,
                ))
                .into()
            }
            formatted::Block::Heading { level, spans } => inline_view(
                spans,
                (heading_size(*level) as f32 + delta).round() as u32,
                design::body_line(delta),
                query,
            ),
            formatted::Block::Code {
                language,
                text: source,
                indent,
            } => {
                let header = row![
                    label(
                        super::short(language.as_deref().unwrap_or("code"), 48),
                        MUTED
                    )
                    .size(11)
                    .width(Length::Fill),
                    button(label("Copy code", MUTED).size(11))
                        .on_press_maybe(ticket.map(|ticket| Message::CopyCode(ticket, index)))
                        .style(design::ghost)
                        .padding([2, 4])
                ]
                .height(22);
                let code = rich_text(marked(highlight(source, language.as_deref()), query))
                    .font(MONO)
                    .size((13.0 + delta_secondary).round() as u32)
                    .line_height(design::body_line(delta))
                    .wrapping(text::Wrapping::None);
                let code = scrollable(container(code).padding(12))
                    .direction(scrollable::Direction::Horizontal(
                        scrollable::Scrollbar::default(),
                    ))
                    .height(
                        plain_height(
                            source,
                            width,
                            13.0 + delta_secondary,
                            design::body_line(delta),
                            MONO,
                            true,
                        ) + 36.0,
                    )
                    .width(Length::Fill);
                container(Column::new().push(header).push(code).spacing(5))
                    .padding(10)
                    .style(|_| design::card(design::OVERLAY, 9.0))
                    .width(Length::Fill)
                    .padding(iced::Padding {
                        left: 10.0 + *indent as f32 * 18.0,
                        ..iced::Padding::from(10.0)
                    })
                    .into()
            }
            formatted::Block::Rule => {
                container(iced::widget::Space::new().height(1).width(Length::Fill))
                    .style(|_| container::Style {
                        background: Some(design::LINE.into()),
                        ..Default::default()
                    })
                    .into()
            }
        };
        body = body.push(
            container(content)
                .height(block_height(block, width, delta, delta_secondary))
                .clip(true),
        );
    }
    body.into()
}

/// Small native lexical palette for common fence labels; never loads plugins or evaluates source.
fn highlight(source: &str, language: Option<&str>) -> Vec<native::Span<'static, (), Font>> {
    let language = language
        .unwrap_or("")
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(
        language.as_str(),
        "rust"
            | "rs"
            | "python"
            | "py"
            | "javascript"
            | "js"
            | "typescript"
            | "ts"
            | "json"
            | "bash"
            | "sh"
    ) {
        return vec![span(source.to_owned()).color(TEXT)];
    }
    let mut runs = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        if runs.len() >= 1023 {
            runs.push(span(source[cursor..].to_owned()).color(TEXT));
            break;
        }
        let start = cursor;
        let ch = source[cursor..].chars().next().unwrap();
        cursor += ch.len_utf8();
        let mut color = TEXT;
        if ch == '\"' || ch == '\'' {
            let mut escaped = false;
            for next in source[cursor..].chars() {
                cursor += next.len_utf8();
                if escaped {
                    escaped = false;
                } else if next == '\\' {
                    escaped = true;
                } else if next == ch {
                    break;
                }
            }
            color = design::FOAM;
        } else if (ch == '#' && matches!(language.as_str(), "py" | "python" | "bash" | "sh"))
            || (ch == '/' && source[cursor..].starts_with('/'))
        {
            cursor += source[cursor..].find('\n').unwrap_or(source.len() - cursor);
            color = MUTED;
        } else if ch.is_ascii_digit() {
            for next in source[cursor..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_')
            {
                cursor += next.len_utf8();
            }
            color = design::GOLD;
        } else if ch.is_alphabetic() || ch == '_' {
            for next in source[cursor..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
            {
                cursor += next.len_utf8();
            }
            if matches!(
                &source[start..cursor],
                "fn" | "let"
                    | "mut"
                    | "pub"
                    | "use"
                    | "impl"
                    | "struct"
                    | "enum"
                    | "match"
                    | "if"
                    | "else"
                    | "return"
                    | "for"
                    | "while"
                    | "async"
                    | "await"
                    | "def"
                    | "class"
                    | "import"
                    | "from"
                    | "const"
                    | "function"
                    | "export"
                    | "true"
                    | "false"
                    | "null"
                    | "None"
                    | "True"
                    | "False"
            ) {
                color = super::ACCENT;
            }
        }
        runs.push(span(source[start..cursor].to_owned()).color(color));
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text_of(runs: &[Run]) -> String {
        runs.iter().map(|run| run.text.as_ref()).collect()
    }

    fn highlighted_text(runs: &[Run]) -> String {
        runs.iter()
            .filter(|run| run.highlight.is_some())
            .map(|run| run.text.as_ref())
            .collect()
    }

    #[test]
    fn literal_marks_cross_styles_preserving_bytes_fonts_and_owned_fragments() {
        let parts = [
            formatted::Span {
                text: "PUBLIC fo".into(),
                bold: false,
                italic: false,
                code: false,
            },
            formatted::Span {
                text: "İ世".into(),
                bold: true,
                italic: true,
                code: false,
            },
            formatted::Span {
                text: "界z".into(),
                bold: false,
                italic: false,
                code: true,
            },
            formatted::Span {
                text: " tail".into(),
                bold: false,
                italic: false,
                code: false,
            },
        ];
        let runs = inline_runs(&parts);
        let original = text_of(&runs);
        let fonts: Vec<_> = runs.iter().map(|run| run.font).collect();
        let marked = marked(runs, "oi\u{307}世界z");
        // Never Debug-format body-bearing native spans, even in assertion failures.
        assert!(text_of(&marked) == original);
        assert!(highlighted_text(&marked) == "oİ世界z");
        assert!(
            marked
                .iter()
                .all(|run| matches!(run.text, std::borrow::Cow::Owned(_)))
        );
        assert_eq!(marked[0].font, fonts[0]);
        assert_eq!(marked[1].font, fonts[0]);
        assert_eq!(marked[2].font, fonts[1]);
        assert_eq!(marked[3].font, fonts[2]);
        assert_eq!(marked[4].font, fonts[3]);
        assert!(
            marked
                .iter()
                .filter(|run| run.highlight.is_some())
                .all(|run| run.color == Some(TEXT))
        );
        assert_eq!(marked[0].color, Some(TEXT));
    }

    #[test]
    fn literal_marks_preserve_all_non_color_attributes_and_unicode_source() {
        let original = span(String::from("PUBLIC É世界 İ [.*] e\u{301} office 👩‍💻"))
            .font(MONO)
            .size(13)
            .line_height(1.3)
            .underline(true)
            .strikethrough(true)
            .padding(2)
            .color(design::FOAM);
        for query in [
            "é世界",
            "i\u{307}",
            "[.*]",
            "e\u{301}",
            "office",
            "👩‍💻",
            "absent",
            "",
        ] {
            let result = marked(vec![original.clone()], query);
            assert!(text_of(&result) == original.text);
            for run in &result {
                assert_eq!(run.font, original.font);
                assert_eq!(run.size, original.size);
                assert_eq!(run.line_height, original.line_height);
                assert_eq!(run.padding, original.padding);
                assert_eq!(run.underline, original.underline);
                assert_eq!(run.strikethrough, original.strikethrough);
                assert!(run.link.is_none());
                if run.highlight.is_none() {
                    assert_eq!(run.color, original.color);
                }
            }
        }
        // Lowercase expansion interiors are not partial original-byte matches.
        assert!(highlighted_text(&marked(vec![span(String::from("İ"))], "i")).is_empty());
    }

    #[test]
    fn literal_marks_fail_closed_at_budgets_and_bound_match_capacity() {
        use super::super::conversation_search::{MAX_MATCHES, MAX_QUERY_BYTES, MAX_SOURCE_BYTES};
        let source = "x".repeat(MAX_SOURCE_BYTES);
        let result = marked(vec![span(source.clone())], "x");
        assert!(text_of(&result) == source);
        assert_eq!(
            result.iter().filter(|run| run.highlight.is_some()).count(),
            MAX_MATCHES
        );
        for (source, query) in [
            ("x".repeat(MAX_SOURCE_BYTES + 1), String::from("x")),
            (String::from("x"), "x".repeat(MAX_QUERY_BYTES + 1)),
        ] {
            let result = marked(vec![span(source.clone())], &query);
            assert!(text_of(&result) == source);
            assert!(result.iter().all(|run| run.highlight.is_none()));
        }
        // Apply the source budget to the joined paragraph, not each individual span.
        let result = marked(
            vec![span("x".repeat(MAX_SOURCE_BYTES)), span(String::from("x"))],
            "x",
        );
        assert!(result.iter().all(|run| run.highlight.is_none()));
    }

    #[test]
    fn displayed_text_search_ignores_markdown_but_keeps_inert_destinations_and_fallback() {
        let cache = Cache::default();
        let p = cache.prepare(1, 2, "PUBLIC supplied reasoning".into(),
            "# PUBLIC **Need**le\n\n[example](https://example.invalid/PUBLIC)\n\n```rust\nlet value = 42;\n```".into());
        assert!(p.matches("needle"));
        assert!(!p.matches("**Need**"));
        assert!(!p.matches("```rust"));
        assert!(p.matches("https://example.invalid/PUBLIC"));
        assert!(p.matches("let value"));
        assert!(p.matches("supplied reasoning"));
        assert!(!p.matches("absent"));
        let raw = cache.prepare(1, 3, String::new(), "<script>PUBLIC inert</script>".into());
        assert!(raw.document.is_none());
        assert!(raw.matches("<script>"));
    }

    #[test]
    fn query_keeps_retained_answer_code_copy_ticket_and_height_unchanged() {
        let cache = Cache::default();
        let p = cache.prepare(
            1,
            2,
            "PUBLIC supplied thoughts".into(),
            "**PUBLIC** answer\n\n```rust\nlet PUBLIC_value = 42;\n```".into(),
        );
        let ticket = p.ticket;
        let answer = p.answer.clone();
        let code = p.code(1).unwrap().to_owned();
        let height = p.body_height(320.0, true);
        for query in ["PUBLIC", "42", "absent", ""] {
            let _ = p.view_search(320.0, true, query);
            assert!(p.answer == answer);
            assert!(p.code(1) == Some(code.as_str()));
            assert_eq!(p.ticket, ticket);
            assert_eq!(p.body_height(320.0, true), height);
            let runs = highlight(&code, Some("rust"));
            let result = marked(runs, query);
            assert!(text_of(&result) == code);
        }
        let runs = highlight(&code, Some("rust"));
        let result = marked(runs, "let public_value = 42");
        assert!(highlighted_text(&result) == "let PUBLIC_value = 42");
        assert!(text_of(&result) == code);
        assert!(result.iter().all(|run| run.font.is_none()));
        let _ = p.view(320.0, true);
    }

    #[test]
    fn native_first_match_offsets_reach_long_wrapped_plain_formatted_code_and_reasoning() {
        let prefix = "PUBLIC words filling the paragraph. ".repeat(80);
        let source = format!("{prefix}TARGET end");
        let offset =
            plain_first_match_offset(&source, 180.0, 15.0, design::body_line(0.0), "target");
        let height = plain_height(&source, 180.0, 15.0, design::body_line(0.0), FONT, false);
        assert!(offset > 500.0 && offset < height);
        assert_eq!(
            plain_first_match_offset(&source, 180.0, 15.0, design::body_line(0.0), "absent"),
            0.0
        );
        let cache = Cache::default();
        let p = cache.prepare(1, 2, String::new(), format!("{prefix}**TAR**GET end"));
        let offset = p.first_match_offset(180.0, false, "target");
        assert!(offset > 500.0 && offset < p.body_height(180.0, false));
        let code = format!("{}TARGET end\n", "let PUBLIC_value = 42;\n".repeat(80));
        let p = cache.prepare(1, 3, String::new(), format!("```rust\n{code}```"));
        let offset = p.first_match_offset(180.0, false, "target");
        assert!(offset > 1000.0 && offset < p.body_height(180.0, false));
        let expected = first_offset(
            highlight(&code, Some("rust")),
            "target",
            180.0,
            13.0,
            design::body_line(0.0),
            MONO,
            true,
        )
        .unwrap();
        assert!((offset - (expected + 49.0)).abs() < 0.1);
        let p = cache.prepare(1, 4, source, "PUBLIC answer".into());
        let reasoning = p.first_match_offset(180.0, true, "target");
        assert!(reasoning > 500.0 && reasoning < p.body_height(180.0, true));
        assert_eq!(p.first_match_offset(180.0, false, "target"), 0.0);
        let answer = p.first_match_offset(180.0, true, "answer");
        // The expanded reasoning body is drawn indented, so it wraps at the
        // narrower measure; the answer's offset has to be built from that same
        // width or the two disagree by the indent.
        let reasoning_height = plain_height(
            &p.reasoning,
            bounded_width(180.0 - design::CONTENT_INSET),
            14.0,
            design::compact_line(0.0),
            FONT,
            false,
        );
        // The answer's first line sits one row box plus the thinking body's top
        // padding below the disclosure, which is where the offset math puts it.
        let after_row = REASONING_ROW + REASONING_BODY_PAD;
        assert!((answer - (reasoning_height + after_row)).abs() < 0.1);
        assert_eq!(p.first_match_offset(180.0, false, "answer"), REASONING_ROW);
    }

    #[test]
    fn nominal_native_geometry_is_measured_before_and_after_representative_marks() {
        // Metadata splitting can alter ligatures/graphemes in some fonts. This
        // checks these fixtures, not a universal shape-invariance guarantee.
        let parts = [
            formatted::Span {
                text: "PUBLIC normal ".repeat(10),
                bold: false,
                italic: false,
                code: false,
            },
            formatted::Span {
                text: "bold PUBLIC ".repeat(10),
                bold: true,
                italic: false,
                code: false,
            },
            formatted::Span {
                text: "italic PUBLIC ".repeat(10),
                bold: false,
                italic: true,
                code: false,
            },
            formatted::Span {
                text: "code PUBLIC ".repeat(10),
                bold: false,
                italic: false,
                code: true,
            },
        ];
        let runs = inline_runs(&parts);
        for width in [180.0, 600.0] {
            let baseline =
                paragraph(&runs, width, 15.0, design::body_line(0.0), FONT, false).min_bounds();
            for query in ["PUBLIC", "normal", "bold", "italic", "code"] {
                let result = marked(runs.clone(), query);
                let measured = paragraph(&result, width, 15.0, design::body_line(0.0), FONT, false)
                    .min_bounds();
                assert!((measured.height - baseline.height).abs() < 0.1);
                assert!((measured.width - baseline.width).abs() < 0.1);
            }
        }
        let code = "let PUBLIC_value = 42;\n".repeat(20);
        let runs = highlight(&code, Some("rust"));
        let baseline =
            paragraph(&runs, 180.0, 13.0, design::body_line(0.0), MONO, true).min_bounds();
        let measured = paragraph(
            &marked(runs, "PUBLIC"),
            180.0,
            13.0,
            design::body_line(0.0),
            MONO,
            true,
        )
        .min_bounds();
        assert!((measured.height - baseline.height).abs() < 0.1);
        assert!((measured.width - baseline.width).abs() < 0.1);
        assert_eq!(
            plain_height("PUBLIC", 0.0, 15.0, design::body_line(0.0), FONT, false),
            design::LINE_BODY
        );
        for width in [f32::NAN, -10.0, f32::INFINITY] {
            let height = plain_height(
                "PUBLIC\nTARGET",
                width,
                15.0,
                design::body_line(0.0),
                FONT,
                false,
            );
            let offset = plain_first_match_offset(
                "PUBLIC\nTARGET",
                width,
                15.0,
                design::body_line(0.0),
                "target",
            );
            assert!(height.is_finite() && height > 0.0);
            assert!(offset.is_finite() && offset >= 0.0 && offset <= height);
        }
    }

    #[test]
    fn cache_stamps_freeze_source_and_never_reuse_changed_code() {
        let cache = Cache::default();
        let a = cache.prepare(
            1,
            2,
            "PRIVATE thoughts".into(),
            "```rust\nfn main() {}\n```".into(),
        );
        let b = cache.prepare(
            1,
            2,
            "PRIVATE thoughts".into(),
            "```rust\nfn main() {}\n```".into(),
        );
        assert_eq!(a.ticket, b.ticket);
        assert_eq!(a.code(0), Some("fn main() {}\n"));
        let c = cache.prepare(1, 2, "PRIVATE thoughts".into(), "new answer".into());
        assert_ne!(a.ticket, c.ticket);
        let d = cache.prepare(2, 2, "PRIVATE thoughts".into(), "new answer".into());
        assert_ne!(c.ticket, d.ticket);
    }
    #[test]
    fn native_geometry_has_separate_collapsed_thoughts_and_exact_code_source() {
        let cache = Cache::default();
        let p = cache.prepare(
            1,
            2,
            "PUBLIC supplied thoughts\nsecond line".into(),
            "# PUBLIC answer\n\n**Bold** and `code`.\n\n```python\nprint(42)\n```".into(),
        );
        assert!(p.body_height(600.0, true) > p.body_height(600.0, false));
        assert!(p.body_height(100.0, false) >= p.body_height(600.0, false));
        assert_eq!(p.code(2), Some("print(42)\n"));
        let _ = p.view(600.0, true);
    }
    #[test]
    fn unsupported_markup_is_inert_full_source_not_partial_formatting() {
        let cache = Cache::default();
        let source = "<script>PUBLIC no execution</script>\n![image](file:///PUBLIC)";
        let p = cache.prepare(1, 0, String::new(), source.into());
        assert!(p.document.is_none());
        assert_eq!(p.answer, source);
        assert!(p.code(0).is_none());
        let _ = p.view(300.0, false);
    }

    #[test]
    fn the_content_setting_invalidates_prepared_replies_and_scales_their_size() {
        let answer = "PUBLIC answer line one\nline two with enough words to wrap";
        let cache = Cache::default();
        let base = cache.prepare(1, 7, String::new(), answer.into());
        let base_height = base.body_height(300.0, false);
        // Re-preparing the identical text at the unchanged delta reuses the entry,
        // which is what makes the next assertion meaningful.
        assert!(std::rc::Rc::ptr_eq(
            &base,
            &cache.prepare(1, 7, String::new(), answer.into())
        ));
        cache.set_content_delta(5.0, 5.0);
        let scaled = cache.prepare(1, 7, String::new(), answer.into());
        assert!(
            !std::rc::Rc::ptr_eq(&base, &scaled),
            "a new content size must not reuse a reply measured at the old one"
        );
        assert!(
            scaled.body_height(300.0, false) > base_height,
            "a larger content size measures taller"
        );
        // The two tiers step independently, so setting the primary delta alone must
        // not leave the secondary sizes scaled from the old value.
        cache.set_content_delta(1.0, 0.0);
        let mixed = cache.prepare(1, 7, String::new(), answer.into());
        assert!(!std::rc::Rc::ptr_eq(&scaled, &mixed));
        let _ = mixed.view(300.0, false);
    }
}
