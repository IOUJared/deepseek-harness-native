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
    let mut body = Column::new().spacing(12).width(Length::Fill);
    for block in &document.blocks {
        let rendered: Element<'_, Message> = match block {
            formatted::Block::Heading { level, spans } => inline(
                spans,
                match level {
                    1 => 25,
                    2 => 21,
                    _ => 18,
                },
            ),
            formatted::Block::Paragraph {
                spans,
                indent,
                marker,
                quote,
            } => {
                let mut line = row![Space::new().width(*indent as f32 * 18.0)].spacing(8);
                if *quote {
                    line = line.push(label("│", design::FOAM));
                }
                if let Some(marker) = marker {
                    line = line.push(label(marker.as_str(), MUTED));
                }
                line.push(inline(spans, 15)).into()
            }
            formatted::Block::Code {
                language,
                text: code,
                indent,
            } => {
                let code_text = text(code.as_str())
                    .font(MONO)
                    .shaping(text::Shaping::Advanced)
                    .size(13)
                    .line_height(1.3)
                    .wrapping(text::Wrapping::None)
                    .color(TEXT);
                let code_scroll = scrollable(container(code_text).padding(12))
                    .direction(scrollable::Direction::Horizontal(
                        scrollable::Scrollbar::default(),
                    ))
                    .width(Length::Fill);
                container(
                    Column::new()
                        .push(
                            label(
                                super::short(language.as_deref().unwrap_or("code"), 48),
                                MUTED,
                            )
                            .size(11),
                        )
                        .push(code_scroll)
                        .spacing(5),
                )
                .padding(10)
                .style(|_| design::card(design::OVERLAY, 9.0))
                .width(Length::Fill)
                .padding(iced::Padding {
                    left: 10.0 + *indent as f32 * 18.0,
                    ..iced::Padding::from(10.0)
                })
                .into()
            }
            formatted::Block::Rule => container(Space::new().height(1).width(Length::Fill))
                .style(|_| container::Style {
                    background: Some(design::LINE.into()),
                    ..Default::default()
                })
                .into(),
        };
        body = body.push(rendered);
    }
    body.into()
}
