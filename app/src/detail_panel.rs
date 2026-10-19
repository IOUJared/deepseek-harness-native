//! Local frozen native display snapshots; copying is an explicit stamped UI action.
//! Body rendering has no URI, clipboard or business actions.
use super::{FONT, MUTED, Message, TEXT, design, label};
use iced::widget::{Column, Space, container, rich_text, row, scrollable, span, text};
use iced::{Element, Font, Length};

#[path = "formatted_details.rs"]
pub mod formatted;

/// The width a details-panel table fills: the panel is a fixed-width column, and the
/// shared view only uses this to choose between filling and scrolling.
const AT_DETAIL_WIDTH: f32 = 640.0;

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
    /// A comparison drawn as diff rows: colour-barred additions and deletions in monospace, the way
    /// the reference's `FileDiff` renders one (`FileDiff.tsx:246-253`).
    Diff,
}

pub struct Detail {
    pub heading: String,
    pub source: String,
    pub metadata: Option<String>,
    pub ticket: Option<Stamp>,
    pub mode: Mode,
    plan: Option<Result<formatted::Document, formatted::Failure>>,
    diff: Option<crate::reducer::inline_diff::Diff>,
    /// Whether a comparison's rows wrap. On by default, as a native reading of the reference's default;
    /// the toolbar control states the other state (`ReviewTab.tsx:131-142`).
    pub diff_wrap: bool,
    /// Whether a comparison draws side by side. The reference's review tab seeds this on for its
    /// first file and offers a toggle beside wrapping (`review-store.ts:52-81`,
    /// `ReviewTab.tsx:130-141`); native keeps its wrapped unified view as the default, which the
    /// user asked for, and offers the same toggle.
    pub diff_split: bool,
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
            diff: None,
            diff_wrap: true,
            diff_split: false,
        }
    }
    /// Draw this panel as a comparison instead of as text. The source stays for copying.
    /// Flip the comparison's wrapping. Only a comparison has the control.
    pub fn toggle_diff_wrap(&mut self) -> bool {
        if self.mode != Mode::Diff {
            return false;
        }
        self.diff_wrap = !self.diff_wrap;
        true
    }
    /// Switch the comparison between one column and side by side. Only a comparison has the control,
    /// and only a comparison with two sides: a created or deleted file has nothing to pair
    /// (`FileDiff.tsx:141-158`, which draws `split && !oneSided`).
    pub fn toggle_diff_split(&mut self) -> bool {
        if self.mode != Mode::Diff {
            return false;
        }
        if self
            .diff
            .as_ref()
            .is_none_or(crate::reducer::inline_diff::one_sided)
        {
            return false;
        }
        self.diff_split = !self.diff_split;
        true
    }
    /// Whether the split control would do anything, which is when it is drawn.
    pub fn split_available(&self) -> bool {
        self.mode == Mode::Diff
            && self
                .diff
                .as_ref()
                .is_some_and(|diff| !crate::reducer::inline_diff::one_sided(diff))
    }
    pub fn set_diff(&mut self, diff: crate::reducer::inline_diff::Diff) {
        if diff.lines.is_empty() {
            return;
        }
        self.mode = Mode::Diff;
        self.diff = Some(diff);
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
        if self.mode == Mode::Diff
            && let Some(diff) = &self.diff
        {
            // One row per line, in the same widget the inline tool diff uses, so a comparison read
            // here and one read in a tool row cannot disagree about colour or rhythm.
            // Each row reserves the lines it wraps onto, so a long comparison row is readable
            // instead of clipped at the panel's edge (round 122).
            let chars = ((AT_DETAIL_WIDTH - 2.0 * design::DIFF_GUTTER - design::DIFF_BAR)
                / crate::ui::changes_text::CODE_CELL)
                .floor()
                .max(1.0) as usize;
            let height = design::CODE_LINE;
            // Side by side: the two columns share the row's height, and with wrapping on each cell
            // wraps inside its own half (`FileDiff.tsx:176-254`: `SplitColumns` when unwrapped, the
            // stacked `splitLine` rows when wrapped). A one-sided comparison stays in one column.
            if self.diff_split && !crate::reducer::inline_diff::one_sided(diff) {
                let half = (AT_DETAIL_WIDTH - design::DIFF_BAR) / 2.0;
                let chars = ((half - design::DIFF_GUTTER - design::DIFF_BAR)
                    / crate::ui::changes_text::CODE_CELL)
                    .floor()
                    .max(1.0) as usize;
                let mut rows = Column::new().width(Length::Fill);
                for row in crate::reducer::inline_diff::split_rows(&diff.lines) {
                    if let Some(full) = &row.full {
                        // A header or fold belongs to neither side, so it draws across the row.
                        rows = rows.push(crate::ui::diff_split_full(full, height));
                        continue;
                    }
                    // Each cell wraps inside its own half, so the row is as tall as its taller side.
                    let lines = |cell: Option<&crate::reducer::inline_diff::SplitCell>| match (
                        cell,
                        self.diff_wrap,
                    ) {
                        (Some(cell), true) => crate::ui::changes_text::visual_lines(
                            &cell.text,
                            chars as f32 * crate::ui::changes_text::CODE_CELL,
                        ),
                        _ => 1,
                    };
                    let tall = lines(row.left.as_ref()).max(lines(row.right.as_ref()));
                    rows = rows.push(crate::ui::diff_split_row(
                        row.left.as_ref(),
                        row.right.as_ref(),
                        height * tall as f32,
                        self.diff_wrap,
                    ));
                }
                return scrollable(rows)
                    .direction(scrollable::Direction::Vertical(
                        scrollable::Scrollbar::default(),
                    ))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();
            }
            let mut rows = Column::new().width(Length::Fill);
            for line in &diff.lines {
                // With wrapping off a row is one clipped line, which is what the reference's unwrapped
                // state shows; the panel scrolls horizontally instead.
                let lines = if self.diff_wrap {
                    crate::ui::changes_text::visual_lines(
                        &line.text,
                        chars as f32 * crate::ui::changes_text::CODE_CELL,
                    )
                } else {
                    1
                };
                rows = rows.push(crate::ui::diff_line_view_wrapped(
                    line,
                    height * lines as f32,
                    lines > 1,
                ));
            }
            return scrollable(rows)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::default(),
                ))
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }
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
            formatted::Block::Heading { level, spans, .. } => {
                inline(spans, design::heading_size(*level).round() as u32)
            }
            formatted::Block::Paragraph {
                spans,
                indent,
                marker,
                quote,
                ..
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
            // The reference's table text stays at the secondary 13px/22px in its own
            // ladder, so the panel reuses the shared view rather than inventing a
            // second one (`MarkdownText.module.css:235-283`).
            formatted::Block::Table {
                header,
                rows,
                indent,
            } => super::conversation_content::table_view(
                header,
                rows,
                *indent,
                AT_DETAIL_WIDTH,
                0.0,
                super::conversation_content::Ladder::Body,
                "",
            ),
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

#[cfg(test)]
mod diff_wrap_tests {
    //! The comparison's wrap toggle, and what it refuses to be.
    use super::*;

    fn comparison() -> Detail {
        let mut detail = Detail::new(
            "Changes".into(),
            String::new(),
            None,
            false,
            Some(Stamp {
                generation: 0,
                opening: 1,
            }),
        );
        detail.set_diff(crate::reducer::inline_diff::Diff {
            lines: vec![crate::reducer::inline_diff::Line {
                kind: crate::reducer::inline_diff::Kind::Added,
                text: "x".repeat(200),
                old: None,
                new: Some(1),
            }],
        });
        detail
    }

    #[test]
    fn a_comparison_wraps_by_default_and_the_toggle_flips_it() {
        let mut detail = comparison();
        assert_eq!(detail.mode, Mode::Diff, "the sample set a comparison");
        assert!(detail.diff_wrap, "wrapping is the default");
        assert!(detail.toggle_diff_wrap());
        assert!(!detail.diff_wrap);
        assert!(detail.toggle_diff_wrap());
        assert!(detail.diff_wrap, "and back");
    }

    #[test]
    fn a_split_needs_a_comparison_with_two_sides() {
        // A created file has no old side, so the reference draws it in one column whatever layout
        // was requested (`FileDiff.tsx:141-158`); the control is not drawn at all, and a toggle
        // begun before the file changed cannot leave the panel split.
        let mut added = comparison();
        assert!(!added.split_available(), "one side has nothing to pair");
        assert!(!added.toggle_diff_split());
        assert!(!added.diff_split);
        // A comparison with both sides splits, and toggles back.
        let mut changed = comparison();
        changed.set_diff(crate::reducer::inline_diff::Diff {
            lines: vec![
                crate::reducer::inline_diff::Line {
                    kind: crate::reducer::inline_diff::Kind::Deleted,
                    text: "PUBLIC old".into(),
                    old: Some(1),
                    new: None,
                },
                crate::reducer::inline_diff::Line {
                    kind: crate::reducer::inline_diff::Kind::Added,
                    text: "PUBLIC new".into(),
                    old: None,
                    new: Some(1),
                },
            ],
        });
        assert!(changed.split_available());
        assert!(changed.toggle_diff_split());
        assert!(changed.diff_split);
        assert!(changed.toggle_diff_split());
        assert!(!changed.diff_split, "and back to one column");
        // A text panel has no comparison, so neither control is offered.
        let mut plain = Detail::new("Record".into(), "plain".into(), None, false, None);
        assert!(!plain.split_available());
        assert!(!plain.toggle_diff_split());
    }

    #[test]
    fn a_text_panel_has_no_wrap_to_toggle() {
        let mut detail = Detail::new("Record".into(), "plain".into(), None, false, None);
        assert_eq!(detail.mode, Mode::Source);
        assert!(!detail.toggle_diff_wrap(), "nothing to wrap");
        assert!(detail.diff_wrap, "and nothing changed");
    }
}
