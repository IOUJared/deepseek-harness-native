//! Small native loading ring. One App-owned clock; no timers or input effects here.
use super::Message;
use crate::design::FOAM;
use iced::advanced::Renderer as _;
use iced::advanced::{Layout, Widget, layout, mouse, renderer, widget};
use iced::{Border, Color, Element, Length, Rectangle, Renderer, Size, Theme};

pub const PHASES: u8 = 12;
const SIZE: f32 = 16.0;

pub fn view<'a>(phase: u8) -> Element<'a, Message> {
    Element::new(Ring(phase % PHASES))
}
struct Ring(u8);
fn leading_dot(bounds: Rectangle, phase: u8, diameter: f32) -> Rectangle {
    let radius = (bounds.width.min(bounds.height) - diameter) / 2.0;
    let angle = f32::from(phase % PHASES) * std::f32::consts::TAU / f32::from(PHASES)
        - std::f32::consts::FRAC_PI_2;
    Rectangle {
        x: bounds.center_x() + radius * angle.cos() - diameter / 2.0,
        y: bounds.center_y() + radius * angle.sin() - diameter / 2.0,
        width: diameter,
        height: diameter,
    }
}
impl Widget<Message, Theme, Renderer> for Ring {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(SIZE), Length::Fixed(SIZE))
    }
    fn layout(
        &mut self,
        _: &mut widget::Tree,
        _: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.resolve(Length::Fixed(SIZE), Length::Fixed(SIZE), Size::ZERO))
    }
    fn draw(
        &self,
        _: &widget::Tree,
        renderer: &mut Renderer,
        _: &Theme,
        _: &renderer::Style,
        layout: Layout<'_>,
        _: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let Some(clip) = bounds.intersection(viewport) else {
            return;
        };
        renderer.with_layer(clip, |renderer| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: (SIZE / 2.0).into(),
                        width: 1.5,
                        color: Color { a: 0.35, ..FOAM },
                    },
                    ..Default::default()
                },
                Color::TRANSPARENT,
            );
            for (phase, diameter, alpha) in [
                ((self.0 + PHASES - 1) % PHASES, 2.5, 0.55),
                (self.0, 3.5, 1.0),
            ] {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: leading_dot(bounds, phase, diameter),
                        border: Border {
                            radius: (diameter / 2.0).into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    Color { a: alpha, ..FOAM },
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orbit_is_bounded_distinct_and_wraps_without_changing_layout() {
        let bounds = Rectangle {
            x: 10.0,
            y: 20.0,
            width: SIZE,
            height: SIZE,
        };
        let first = leading_dot(bounds, 0, 3.5);
        for phase in 0..PHASES {
            let dot = leading_dot(bounds, phase, 3.5);
            assert!(dot.x >= bounds.x - 0.001 && dot.y >= bounds.y - 0.001);
            assert!(dot.x + dot.width <= bounds.x + bounds.width + 0.001);
            assert!(dot.y + dot.height <= bounds.y + bounds.height + 0.001);
            if phase != 0 {
                assert_ne!(dot, first);
            }
        }
        assert_eq!(leading_dot(bounds, PHASES, 3.5), first);
    }
}
