//! Drag selection over the conversation's rendered text.
//!
//! The conversation draws prose, headings and code through shaped rich text, and Iced
//! gives that no selection model: `text_editor` is the only widget with one
//! (`iced_widget-0.14.2/src/text_editor.rs:359`). This wrapper adds the missing half
//! around the text widget the block already shows. It keeps the paragraph it draws,
//! hit-tests the pointer against that same shape, and publishes the selected byte
//! range so the owner can copy it and redraw the highlight by restyling its spans.
//!
//! Selection stays inside one block: a drag is clamped to the paragraph it started
//! in. A second block needs a second drag, and the owner records that as a limit
//! rather than pretending the selection crosses paragraphs.
use super::{Message, conversation_content, design};
use iced::advanced::text::{self, Paragraph as _};
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, mouse, renderer};
use iced::{Element, Point, Size};
use std::rc::Rc;

/// One text block's stable place in the conversation: the row's record key and the
/// block's number inside that row. Frames are built from the top of the visible
/// transcript, so a block's ordinal alone would move with scrolling or with a
/// disclosure above it; the record key does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub row: u64,
    pub block: u16,
}

/// One published drag: the block, the byte range inside that block's rendered text,
/// and the text itself, so a Ctrl+C needs no second pass over the conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub piece: Piece,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

impl Selection {
    pub fn range(&self) -> std::ops::Range<usize> {
        self.start.min(self.end)..self.start.max(self.end)
    }
}

#[derive(Default)]
struct State {
    /// Where the current drag started, in bytes of the block's rendered text.
    anchor: Option<usize>,
    /// The range last published, so a pointer moving inside one character does not
    /// republish the same selection on every frame.
    last: Option<std::ops::Range<usize>>,
    /// The same paragraph the block draws, shaped on demand. It exists only between a
    /// press and the end of that drag, so ordinary frames pay nothing for selection.
    geometry: Option<<iced::Renderer as text::Renderer>::Paragraph>,
    width: f32,
}

/// A block of rendered text that can be dragged over.
pub struct Field<'a> {
    content: Element<'a, Message>,
    runs: Rc<Vec<conversation_content::Run>>,
    text: Rc<str>,
    piece: Piece,
    size: u32,
    line: text::LineHeight,
    font: iced::Font,
    wrap: bool,
}

/// Wraps one text element. `runs` must be the exact spans the element draws, because
/// the pointer is hit-tested against a paragraph shaped from them.
pub fn field<'a>(
    content: Element<'a, Message>,
    runs: Rc<Vec<conversation_content::Run>>,
    piece: Piece,
    size: u32,
    line: text::LineHeight,
    font: iced::Font,
    wrap: bool,
) -> Element<'a, Message> {
    let text: Rc<str> = runs
        .iter()
        .map(|run| run.text.as_ref())
        .collect::<String>()
        .into();
    Element::new(Field {
        content,
        runs,
        text,
        piece,
        size,
        line,
        font,
        wrap,
    })
}

impl Field<'_> {
    /// The byte offset under `point` (block-local). The paragraph is shaped once per
    /// width and kept for the drag, so a move costs one hit test, not a reshape.
    fn offset_at(&self, state: &mut State, width: f32, point: Point) -> usize {
        if state.geometry.is_none() {
            state.geometry = Some(conversation_content::geometry(
                &self.runs, width, self.size, self.line, self.font, self.wrap,
            ));
        }
        state
            .geometry
            .as_ref()
            .and_then(|paragraph| paragraph.hit_test(point).map(|hit| hit.cursor()))
            .unwrap_or(0)
            .min(self.text.len())
    }
}

impl Widget<Message, iced::Theme, iced::Renderer> for Field<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.content.as_widget())]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn size(&self) -> Size<iced::Length> {
        self.content.as_widget().size()
    }
    fn size_hint(&self) -> Size<iced::Length> {
        self.content.as_widget().size_hint()
    }
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let node = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let width = node.size().width;
        let state = tree.state.downcast_mut::<State>();
        if state.width != width {
            state.width = width;
            // A resize invalidates the shape the drag was hit-testing against.
            state.geometry = None;
        }
        node
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        // The block's own widget reads the event first: a code block's toolbar and a
        // link keep their presses, and selection only claims what nobody else took.
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<State>();
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                // `position_in` is already block-local; subtracting the bounds again put
                // the press off the paragraph, where the hit test answered byte zero.
                if let Some(local) = cursor.position_in(bounds) {
                    state.anchor = Some(self.offset_at(state, bounds.width, local));
                    state.last = None;
                    // A press that starts a drag also clears whatever was selected before.
                    shell.publish(Message::TextSelect(None));
                }
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let Some(anchor) = state.anchor else {
                    return;
                };
                let Some(position) = cursor.position() else {
                    return;
                };
                // Clamp to this block: the pointer may leave it, but the selection may not.
                let local = Point::new(
                    (position.x - bounds.x).clamp(0.0, bounds.width.max(0.0)),
                    (position.y - bounds.y).clamp(0.0, (bounds.height - 1.0).max(0.0)),
                );
                let focus = self.offset_at(state, bounds.width, local);
                let range = anchor.min(focus)..anchor.max(focus);
                if state.last.as_ref() == Some(&range) {
                    return;
                }
                shell.publish(if range.is_empty() {
                    Message::TextSelect(None)
                } else {
                    Message::TextSelect(Some(Selection {
                        piece: self.piece,
                        start: range.start,
                        end: range.end,
                        text: self.text[range.clone()].to_owned(),
                    }))
                });
                state.last = Some(range);
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.anchor = None;
            }
            _ => {}
        }
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        // A text cursor tells the user the block can be dragged over.
        if cursor.is_over(layout.bounds()) {
            return mouse::Interaction::Text;
        }
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

/// The tint behind selected conversation text. The reference paints its document
/// selection with `--dsw-alias-bg-document-selection` (blue-500 at 40%,
/// `ui-theme/src/styles/design-platform.css:169`); Rosé Pine's blue stand-in is FOAM.
pub fn selection_tint() -> iced::Background {
    iced::Color {
        a: 0.40,
        ..design::FOAM
    }
    .into()
}

/// Replaces the selection with the same text drawn over [`selection_tint`]. Kept here
/// beside the hit test so the drawn range and the copied range are the same bytes.
pub fn highlighted<'a>(
    runs: Vec<conversation_content::Run>,
    selection: Option<&Selection>,
    piece: Piece,
) -> Vec<conversation_content::Run> {
    let Some(selection) = selection.filter(|selection| selection.piece == piece) else {
        return runs;
    };
    let range = selection.range();
    if range.is_empty() {
        return runs;
    }
    let mut result = Vec::new();
    let mut base = 0;
    for mut run in runs {
        let text = std::mem::take(&mut run.text);
        let end = base + text.len();
        let start = range.start.max(base);
        let stop = range.end.min(end);
        if start >= stop {
            run.text = text.into();
            result.push(run);
        } else {
            if base < start {
                let mut head = run.clone();
                head.text = text[..start - base].to_owned().into();
                result.push(head);
            }
            let mut marked = run.clone();
            marked.text = text[start - base..stop - base].to_owned().into();
            result.push(marked.background(selection_tint()));
            if stop < end {
                let mut tail = run;
                tail.text = text[stop - base..].to_owned().into();
                result.push(tail);
            }
        }
        base = end;
    }
    result
}
