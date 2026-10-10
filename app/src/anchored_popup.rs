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
    })
}

struct Anchored<'a> {
    anchor: Element<'a, Message>,
    popup: Option<Element<'a, Message>>,
    dismiss: Message,
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
        let placed = popup_bounds(self.anchor, child.size(), self.viewport);
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

fn popup_bounds(anchor: Rectangle, requested: Size, viewport: Rectangle) -> Rectangle {
    let size = Size::new(
        requested.width.min(viewport.width),
        requested.height.min(viewport.height),
    );
    Rectangle {
        x: (anchor.x + anchor.width - size.width)
            .clamp(viewport.x, viewport.x + viewport.width - size.width),
        y: (anchor.y - size.height - 8.0)
            .clamp(viewport.y, viewport.y + viewport.height - size.height),
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
    fn right_aligned_above_actual_anchor_with_eight_pixel_gap() {
        assert_eq!(
            popup_bounds(
                rect(620.0, 600.0, 140.0, 32.0),
                Size::new(320.0, 200.0),
                rect(0.0, 0.0, 900.0, 700.0)
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
                rect(0.0, 0.0, 240.0, 220.0)
            ),
            rect(0.0, 0.0, 240.0, 200.0),
        );
        assert_eq!(
            popup_bounds(
                rect(900.0, 800.0, 100.0, 30.0),
                Size::new(200.0, 120.0),
                rect(0.0, 0.0, 240.0, 220.0)
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
                    Rectangle::with_size(size)
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
                rect(80.0, 90.0, 600.0, 500.0)
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
