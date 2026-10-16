//! Local frozen native display snapshots; copying is an explicit stamped UI action.
//! Body rendering has no URI, clipboard or business actions.
use super::{FONT, MUTED, Message, TEXT, design, label};
use iced::widget::{Column, Space, container, rich_text, row, scrollable, span, text};
use iced::{Element, Font, Length};

#[path = "formatted_details.rs"]
pub mod formatted;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stamp {
    pub generation: u64,
    pub opening: u64,
}
/// Local opener lease: generation plus the last consumed panel identity.
/// Contains no display source. Reopening consumes the prior value exactly once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Opening {
    pub generation: u64,
    pub prior_opening: u64,
    pub key: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Source,
    Formatted,
}

pub struct Detail {
    pub heading: String,
    pub source: String,
    pub metadata: Option<String>,
    pub ticket: Option<Stamp>,
    pub mode: Mode,
    plan: Option<Result<formatted::Document, formatted::Failure>>,
}
impl std::fmt::Debug for Detail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeDetail")
            .field("mode", &self.mode)
            .field("ticket", &self.ticket)
            .field("formattedAvailable", &self.formatted_available())
            .finish_non_exhaustive()
    }
}
impl Detail {
    pub fn new(
        heading: String,
        source: String,
        metadata: Option<String>,
        eligible: bool,
        ticket: Option<Stamp>,
    ) -> Self {
        let plan = eligible.then(|| formatted::parse(&source));
        Self {
            heading,
            source,
            metadata,
            ticket,
            mode: Mode::Source,
            plan,
        }
    }
    /// Copy only the nonempty bounded display source, never metadata or rendered output.
    pub fn copy_available(&self) -> bool {
        self.ticket.is_some()
            && !self.source.is_empty()
            && self.source.len() <= formatted::INPUT_BYTES
    }
    pub fn formatted_available(&self) -> bool {
        self.plan.as_ref().is_some_and(Result::is_ok)
    }
    pub fn set_mode(&mut self, ticket: Stamp, mode: Mode) -> bool {
        if self.ticket != Some(ticket) || (mode == Mode::Formatted && !self.formatted_available()) {
            return false;
        }
        self.mode = mode;
        true
    }
    pub fn notice(&self) -> &'static str {
        match &self.plan {
            None => "Frozen native display source · read-only",
            Some(Ok(_)) => "Basic native formatting · links are inert · no code execution",
            Some(Err(formatted::Failure::Unsupported)) => {
                "Source only: this snapshot includes unsupported formatting"
            }
            Some(Err(_)) => "Source only: this snapshot exceeds the bounded formatting budget",
        }
    }
    pub fn body(&self) -> Element<'_, Message> {
        if self.mode == Mode::Formatted
            && let Some(Ok(document)) = &self.plan
        {
            return view(document);
        }
        label(self.source.as_str(), TEXT)
            .size(15)
            .width(Length::Fill)
            .into()
    }
}
const MONO: Font = Font::with_name("DejaVu Sans Mono");
fn inline<'a>(spans: &'a [formatted::Span], size: u32) -> Element<'a, Message> {
    let runs: Vec<iced::advanced::text::Span<'_, ()>> = spans
        .iter()
        .map(|part| {
            let font = Font {
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
            };
            span(part.text.as_str())
                .font(font)
                .color(if part.code { design::FOAM } else { TEXT })
        })
        .collect::<Vec<_>>();
    rich_text(runs)
        .font(FONT)
        .size(size)
        .line_height(1.3)
        .wrapping(text::Wrapping::Word)
        .width(Length::Fill)
        .into()
}
fn view(document: &formatted::Document) -> Element<'_, Message> {
    // The document's own rhythm, shared with the transcript: the reference spaces
    // paragraphs by 16px and steps a following `h1`-`h3` up to 32px
    // (`ui-primitives/markdown/MarkdownText.module.css:15-44`).
    // Every gap comes from the block pair's collapsed reference margins, so the
    // column itself spaces nothing (`MarkdownText.module.css`).
    let mut body = Column::new().width(Length::Fill);
    for (index, block) in document.blocks.iter().enumerate() {
        let rendered: Element<'_, Message> = match block {
            formatted::Block::Heading { level, spans } => {
                inline(spans, design::heading_size(*level).round() as u32)
            }
            formatted::Block::Paragraph {
                spans,
                indent,
                marker,
                quote,
            } => {
                // Gaps are explicit so the blockquote's inset is exactly the
                // reference's 2px rule plus 14px of padding
                // (`MarkdownText.module.css:151-156`).
                let mut line = row![Space::new().width(*indent as f32 * 18.0)].spacing(0);
                if *quote {
                    // The rule spans the paragraph's own measured height, matching the
                    // reference's 2px border plus a 14px padding
                    // (`MarkdownText.module.css:151-156`).
                    let bar = super::conversation_content::quote_bar_height(spans);
                    line = line.push(
                        container(Space::new().width(design::QUOTE_BAR))
                            .height(bar)
                            .style(|_| container::Style {
                                background: Some(design::MUTED.into()),
                                ..Default::default()
                            }),
                    );
                    line = line.push(Space::new().width(design::QUOTE_PAD));
                }
                if let Some(marker) = marker {
                    line = line.push(label(marker.as_str(), MUTED));
                    line = line.push(Space::new().width(8));
                }
                line.push(inline(spans, 15)).into()
            }
            formatted::Block::Code {
                language,
                text: code,
                indent,
            } => {
                // The same code box the transcript draws: the reference's 11px/19px
                // code font, a 9px/14px banner at 11px/18px, a 16px pre padding and
                // a radius-lg block with no padding of its own
                // (`ui-primitives/markdown/CodeBlock.module.css`).
                let code_text = text(code.as_str())
                    .font(MONO)
                    .shaping(text::Shaping::Advanced)
                    .size(design::CODE_FONT)
                    .line_height(design::code_line(0.0))
                    .wrapping(text::Wrapping::None)
                    .color(TEXT);
                let code_scroll = scrollable(container(code_text).padding(design::CODE_PAD))
                    .direction(scrollable::Direction::Horizontal(
                        scrollable::Scrollbar::default(),
                    ))
                    .width(Length::Fill);
                let banner = container(
                    label(
                        super::short(language.as_deref().unwrap_or("code"), 48),
                        MUTED,
                    )
                    .size(design::CODE_FONT)
                    .line_height(iced::Pixels(design::CODE_BANNER_LINE))
                    .width(Length::Fill),
                )
                .padding(design::CODE_BANNER_PAD)
                .style(|_| design::card(design::LINE, 0.0));
                container(Column::new().push(banner).push(code_scroll).spacing(0))
                    .style(|_| design::card(design::OVERLAY, design::RADIUS_LG))
                    .width(Length::Fill)
                    .padding(iced::Padding {
                        left: *indent as f32 * 18.0,
                        ..iced::Padding::ZERO
                    })
                    .clip(true)
                    .into()
            }
            formatted::Block::Rule => container(Space::new().height(1).width(Length::Fill))
                .style(|_| container::Style {
                    background: Some(design::LINE.into()),
                    ..Default::default()
                })
                .into(),
        };
        let gap = super::conversation_content::document_gap(
            &document.blocks,
            index,
            super::conversation_content::Ladder::Body,
        );
        body = body.push(if gap > 0.0 {
            Column::new()
                .push(Space::new().height(gap))
                .push(rendered)
                .into()
        } else {
            rendered
        });
    }
    body.into()
}
