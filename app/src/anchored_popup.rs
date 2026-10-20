//! Anchor-sized native control with a viewport-clamped, input-capturing popup.
use crate::ui::Message;
use iced::advanced::Renderer as _;
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer, widget};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, keyboard};

/// Keeps the anchor's flow size; an open popup is right-aligned above its actual bounds.
/// The caller owns opening identity, modal priority and queued-message validation.
pub fn anchored<'a>(
    anchor: Element<'a, Message>,
    popup: Option<Element<'a, Message>>,
    dismiss: Message,
) -> Element<'a, Message> {
    Element::new(Anchored {
        anchor,
        popup,
        dismiss,
        below: false,
    })
}

/// The same control with its popup laid **below** the anchor.
pub fn anchored_below<'a>(
    anchor: Element<'a, Message>,
    popup: Option<Element<'a, Message>>,
    dismiss: Message,
) -> Element<'a, Message> {
    Element::new(Anchored {
        anchor,
        popup,
        dismiss,
        below: true,
    })
}

/// A non-modal card laid **beside** its anchor's right edge, eight pixels out.
///
/// Unlike [`anchored`], input outside the card is never captured: the row that owns the anchor keeps
/// receiving motion, so the pointer can leave the row, cross the gap and rest on the card while the
/// row's own hover state is what closes it. A card that captured the window would end the row's
/// hover the moment it opened.
pub fn beside<'a>(
    anchor: Element<'a, Message>,
    card: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    Element::new(Beside { anchor, card })
}

struct Beside<'a> {
    anchor: Element<'a, Message>,
    card: Option<Element<'a, Message>>,
}

impl Widget<Message, Theme, Renderer> for Beside<'_> {
    fn children(&self) -> Vec<widget::Tree> {
        let mut children = vec![widget::Tree::new(&self.anchor)];
        if let Some(card) = &self.card {
            children.push(widget::Tree::new(card));
        }
        children
    }

    fn diff(&self, tree: &mut widget::Tree) {
        let mut children = vec![self.anchor.as_widget()];
        if let Some(card) = &self.card {
            children.push(card.as_widget());
        }
        tree.diff_children(&children);
    }

    fn size(&self) -> Size<Length> {
        self.anchor.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.anchor.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.anchor
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.anchor.as_widget().draw(
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
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.anchor
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.anchor.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.anchor.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        if let Some(card) = &mut self.card {
            Some(overlay::Element::new(Box::new(BesideCard {
                card,
                tree: &mut tree.children[1],
                anchor: layout.bounds() + translation,
            })))
        } else {
            self.anchor.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            )
        }
    }
}

struct BesideCard<'a, 'b> {
    card: &'b mut Element<'a, Message>,
    tree: &'b mut widget::Tree,
    anchor: Rectangle,
}

impl overlay::Overlay<Message, Theme, Renderer> for BesideCard<'_, '_> {
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        // The overlay is a child of a `scrollable`, which hands it the list's own visible strip as
        // the viewport. A card beside the sidebar must be placed and clipped against the window, or
        // the eight-pixel gap is clamped back inside the 260px column. `bounds` is the window here;
        // `UserInterface` re-instantiates the overlay for drawing without calling this again, so the
        // draw path derives the same window from its own layout root.
        let full = Rectangle::with_size(bounds);
        let child = self.card.as_widget_mut().layout(
            self.tree,
            renderer,
            &layout::Limits::new(Size::ZERO, full.size()),
        );
        let placed = beside_bounds(self.anchor, child.size(), full);
        layout::Node::with_children(bounds, vec![child.move_to(placed.position())])
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let viewport = layout.bounds();
        renderer.with_layer(viewport, |renderer| {
            self.card.as_widget().draw(
                self.tree,
                renderer,
                theme,
                style,
                layout.children().next().unwrap(),
                cursor,
                &viewport,
            );
        });
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.card.as_widget_mut().operate(
            self.tree,
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let child = layout.children().next().unwrap();
        let viewport = layout.bounds();
        let over = cursor.is_over(child.bounds());
        match event {
            // Movement reaches the card so its own `mouse_area` can publish enter and exit, but it is
            // never captured: the row behind the overlay needs the same motion to end its hover.
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                self.card.as_widget_mut().update(
                    self.tree, event, child, cursor, renderer, clipboard, shell, &viewport,
                );
            }
            _ if over => {
                self.card.as_widget_mut().update(
                    self.tree, event, child, cursor, renderer, clipboard, shell, &viewport,
                );
                if input_event(event) {
                    shell.capture_event();
                }
            }
            // Everything outside the card belongs to the row and the transcript beneath it.
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        // The card is a preview, not a control: reporting an interaction would make Iced withdraw the
        // cursor from the base layer and end the row's hover under the pointer.
        let _ = (layout, cursor, renderer);
        mouse::Interaction::None
    }
}

struct Anchored<'a> {
    anchor: Element<'a, Message>,
    popup: Option<Element<'a, Message>>,
    dismiss: Message,
    /// Lay the popup below the anchor instead of above it: a header anchor has no room above.
    below: bool,
}

impl Widget<Message, Theme, Renderer> for Anchored<'_> {
    fn children(&self) -> Vec<widget::Tree> {
        let mut children = vec![widget::Tree::new(&self.anchor)];
        if let Some(popup) = &self.popup {
            children.push(widget::Tree::new(popup));
        }
        children
    }

    fn diff(&self, tree: &mut widget::Tree) {
        let mut children = vec![self.anchor.as_widget()];
        if let Some(popup) = &self.popup {
            children.push(popup.as_widget());
        }
        tree.diff_children(&children);
    }

    fn size(&self) -> Size<Length> {
        self.anchor.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.anchor.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.anchor
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.anchor.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            if self.popup.is_some() {
                mouse::Cursor::Unavailable
            } else {
                cursor
            },
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.anchor
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if self.popup.is_some() && input_event(event) {
            shell.capture_event();
            return;
        }
        self.anchor.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.popup.is_some() {
            mouse::Interaction::None
        } else {
            self.anchor.as_widget().mouse_interaction(
                &tree.children[0],
                layout,
                cursor,
                viewport,
                renderer,
            )
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        if let Some(popup) = &mut self.popup {
            Some(overlay::Element::new(Box::new(Popup {
                popup,
                tree: &mut tree.children[1],
                anchor: layout.bounds() + translation,
                viewport: *viewport,
                dismiss: self.dismiss.clone(),
                below: self.below,
            })))
        } else {
            self.anchor.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            )
        }
    }
}

struct Popup<'a, 'b> {
    popup: &'b mut Element<'a, Message>,
    tree: &'b mut widget::Tree,
    anchor: Rectangle,
    viewport: Rectangle,
    dismiss: Message,
    below: bool,
}

impl overlay::Overlay<Message, Theme, Renderer> for Popup<'_, '_> {
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let full = Rectangle::with_size(bounds);
        self.viewport = self.viewport.intersection(&full).unwrap_or(full);
        let child = self.popup.as_widget_mut().layout(
            self.tree,
            renderer,
            &layout::Limits::new(Size::ZERO, self.viewport.size()),
        );
        let placed = popup_bounds(self.anchor, child.size(), self.viewport, self.below);
        // Full-window bounds make the transparent area intercept background input.
        layout::Node::with_children(bounds, vec![child.move_to(placed.position())])
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        renderer.with_layer(self.viewport, |renderer| {
            self.popup.as_widget().draw(
                self.tree,
                renderer,
                theme,
                style,
                layout.children().next().unwrap(),
                cursor,
                &self.viewport,
            );
        });
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.popup.as_widget_mut().operate(
            self.tree,
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let child = layout.children().next().unwrap();
        if escape(event) || outside_press(event, cursor, child.bounds()) {
            shell.publish(self.dismiss.clone());
        } else {
            self.popup.as_widget_mut().update(
                self.tree,
                event,
                child,
                cursor,
                renderer,
                clipboard,
                shell,
                &self.viewport,
            );
        }
        if input_event(event) {
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let interaction = self.popup.as_widget().mouse_interaction(
            self.tree,
            layout.children().next().unwrap(),
            cursor,
            &self.viewport,
            renderer,
        );
        if interaction == mouse::Interaction::None && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Idle
        } else {
            interaction
        }
    }

    fn overlay<'b>(
        &'b mut self,
        layout: Layout<'b>,
        renderer: &Renderer,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        // Iced dispatches nested menus first and draws them above this popup.
        self.popup.as_widget_mut().overlay(
            self.tree,
            layout.children().next().unwrap(),
            renderer,
            &self.viewport,
            Vector::ZERO,
        )
    }
}

fn popup_bounds(anchor: Rectangle, requested: Size, viewport: Rectangle, below: bool) -> Rectangle {
    let size = Size::new(
        requested.width.min(viewport.width),
        requested.height.min(viewport.height),
    );
    Rectangle {
        x: (anchor.x + anchor.width - size.width)
            .clamp(viewport.x, viewport.x + viewport.width - size.width),
        y: if below {
            (anchor.y + anchor.height + 8.0)
                .clamp(viewport.y, viewport.y + viewport.height - size.height)
        } else {
            (anchor.y - size.height - 8.0)
                .clamp(viewport.y, viewport.y + viewport.height - size.height)
        },
        width: size.width,
        height: size.height,
    }
}

/// Place a hover card eight pixels past its anchor's right edge, kept inside the viewport's own
/// eight-pixel margin (`ui-primitives/src/HoverCard.tsx:205-207`, `HoverCard.module.css:6-16`).
fn beside_bounds(anchor: Rectangle, requested: Size, viewport: Rectangle) -> Rectangle {
    const GAP: f32 = 8.0;
    const MARGIN: f32 = 8.0;
    let size = Size::new(
        requested
            .width
            .min((viewport.width - 2.0 * MARGIN).max(0.0)),
        requested
            .height
            .min((viewport.height - 2.0 * MARGIN).max(0.0)),
    );
    let highest_x = (viewport.x + viewport.width - size.width - MARGIN).max(viewport.x + MARGIN);
    let highest_y = (viewport.y + viewport.height - size.height - MARGIN).max(viewport.y + MARGIN);
    Rectangle {
        x: (anchor.x + anchor.width + GAP).clamp(viewport.x + MARGIN, highest_x),
        y: anchor.y.clamp(viewport.y + MARGIN, highest_y),
        width: size.width,
        height: size.height,
    }
}

fn input_event(event: &Event) -> bool {
    matches!(
        event,
        Event::Mouse(_) | Event::Keyboard(_) | Event::Touch(_) | Event::InputMethod(_)
    )
}

fn escape(event: &Event) -> bool {
    matches!(event, Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(keyboard::key::Named::Escape), modifiers,
        repeat: false, ..
    }) if modifiers.is_empty())
}

fn outside_press(event: &Event, cursor: mouse::Cursor, bounds: Rectangle) -> bool {
    match event {
        Event::Mouse(mouse::Event::ButtonPressed(_)) => !cursor.is_over(bounds),
        Event::Touch(iced::touch::Event::FingerPressed { position, .. }) => {
            !bounds.contains(*position)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Point;

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
        Rectangle {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn a_hover_card_sits_eight_pixels_beside_its_row_and_clamps_both_edges() {
        let viewport = rect(0.0, 0.0, 1040.0, 732.0);
        assert_eq!(
            beside_bounds(
                rect(0.0, 200.0, 260.0, 32.0),
                Size::new(244.0, 180.0),
                viewport,
            ),
            rect(268.0, 200.0, 244.0, 180.0),
        );
        // The bottom edge keeps the viewport's own eight-pixel margin.
        assert_eq!(
            beside_bounds(
                rect(0.0, 700.0, 260.0, 32.0),
                Size::new(244.0, 180.0),
                viewport,
            ),
            rect(268.0, 544.0, 244.0, 180.0),
        );
        // A narrow viewport pulls the card back over the anchor instead of past the right margin.
        assert_eq!(
            beside_bounds(
                rect(0.0, 10.0, 260.0, 32.0),
                Size::new(244.0, 100.0),
                rect(0.0, 0.0, 300.0, 200.0),
            ),
            rect(48.0, 10.0, 244.0, 100.0),
        );
        // An oversized request shrinks to the viewport minus both margins.
        assert_eq!(
            beside_bounds(
                rect(0.0, 0.0, 10.0, 10.0),
                Size::new(900.0, 900.0),
                rect(0.0, 0.0, 300.0, 200.0),
            ),
            rect(8.0, 8.0, 284.0, 184.0),
        );
    }

    #[test]
    fn right_aligned_above_actual_anchor_with_eight_pixel_gap() {
        assert_eq!(
            popup_bounds(
                rect(620.0, 600.0, 140.0, 32.0),
                Size::new(320.0, 200.0),
                rect(0.0, 0.0, 900.0, 700.0),
                false,
            ),
            rect(440.0, 392.0, 320.0, 200.0),
        );
    }

    #[test]
    fn narrow_viewport_clamps_both_edges_without_negative_position() {
        assert_eq!(
            popup_bounds(
                rect(8.0, 180.0, 60.0, 32.0),
                Size::new(320.0, 200.0),
                rect(0.0, 0.0, 240.0, 220.0),
                false,
            ),
            rect(0.0, 0.0, 240.0, 200.0),
        );
        assert_eq!(
            popup_bounds(
                rect(900.0, 800.0, 100.0, 30.0),
                Size::new(200.0, 120.0),
                rect(0.0, 0.0, 240.0, 220.0),
                false,
            ),
            rect(40.0, 100.0, 200.0, 120.0),
        );
    }

    #[test]
    fn small_and_empty_viewports_have_bounded_geometry() {
        for size in [Size::new(18.0, 12.0), Size::ZERO] {
            assert_eq!(
                popup_bounds(
                    rect(10.0, 10.0, 80.0, 32.0),
                    Size::new(320.0, 260.0),
                    Rectangle::with_size(size),
                    false,
                ),
                Rectangle::with_size(size),
            );
        }
    }

    #[test]
    fn translated_anchor_respects_nonzero_viewport_origin() {
        assert_eq!(
            popup_bounds(
                rect(140.0, 380.0, 80.0, 32.0),
                Size::new(260.0, 180.0),
                rect(80.0, 90.0, 600.0, 500.0),
                false,
            ),
            rect(80.0, 192.0, 260.0, 180.0),
        );
    }

    #[test]
    fn touch_press_uses_its_position_without_requiring_a_mouse_cursor() {
        let bounds = rect(40.0, 20.0, 200.0, 120.0);
        for (position, outside) in [
            (Point::new(20.0, 30.0), true),
            (Point::new(80.0, 30.0), false),
        ] {
            let press = Event::Touch(iced::touch::Event::FingerPressed {
                id: iced::touch::Finger(0),
                position,
            });
            assert_eq!(
                outside_press(&press, mouse::Cursor::Unavailable, bounds),
                outside
            );
            assert!(input_event(&press));
        }
    }

    #[test]
    fn outside_mouse_press_dismisses_but_inside_press_and_motion_do_not() {
        let bounds = rect(40.0, 20.0, 200.0, 120.0);
        let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        assert!(outside_press(
            &press,
            mouse::Cursor::Available(Point::new(20.0, 30.0)),
            bounds
        ));
        assert!(!outside_press(
            &press,
            mouse::Cursor::Available(Point::new(80.0, 30.0)),
            bounds
        ));
        assert!(!outside_press(
            &Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(20.0, 30.0)
            }),
            mouse::Cursor::Available(Point::new(20.0, 30.0)),
            bounds,
        ));
    }
}
