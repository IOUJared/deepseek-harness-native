//! The composer's context-occupancy ring.
//!
//! The reference draws a 14×14 SVG ring — 5.5px radius, 2px stroke, round cap, rotated so the fill
//! starts at twelve o'clock (`ui-conversation/src/client/skeleton/ContextMeter.tsx:17-20,110-117`;
//! `ContextMeter.module.css:38-49`). Iced has no SVG widget, so the same geometry is stroked on a
//! canvas; the reading beside it stays a text label and still opens the breakdown panel.
use crate::design;
use crate::ui::Message;
use iced::widget::canvas::{self, Canvas, Frame, Geometry, LineCap, Path, Stroke, Style};
use iced::{Color, Element, Length, Point, Radians, Rectangle, Renderer, Theme};

/// The reference's 14px viewBox.
pub const SIZE: f32 = 14.0;
/// Ring radius inside that viewBox.
const RADIUS: f32 = 5.5;
/// Stroke width in the reference.
const STROKE: f32 = 2.0;

#[derive(Clone, Copy)]
pub struct Ring {
    /// Occupancy clamped to 0–100; the label carries the same number.
    percent: u8,
    color: Color,
}

/// The filled sweep in radians, clockwise from twelve o'clock. Clamped so an over-capacity reading
/// (or a stale one) can never draw past a full ring.
pub fn sweep(percent: u8) -> f32 {
    percent.min(100) as f32 * std::f32::consts::TAU / 100.0
}

/// The ring element, sized exactly as the reference's SVG.
pub fn view(percent: u64, color: Color) -> Element<'static, Message> {
    Canvas::new(Ring {
        percent: percent.min(100) as u8,
        color,
    })
    .width(Length::Fixed(SIZE))
    .height(Length::Fixed(SIZE))
    .into()
}

impl canvas::Program<Message> for Ring {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let center = Point::new(SIZE / 2.0, SIZE / 2.0);
        // The reference's track is `border-l3`, the quietest rule in the palette.
        frame.stroke(
            &Path::circle(center, RADIUS),
            Stroke {
                style: Style::Solid(design::LINE),
                width: STROKE,
                line_cap: LineCap::Round,
                ..Stroke::default()
            },
        );
        if self.percent > 0 {
            let start = -std::f32::consts::FRAC_PI_2;
            let arc = Path::new(|builder| {
                builder.arc(canvas::path::Arc {
                    center,
                    radius: RADIUS,
                    start_angle: Radians(start),
                    end_angle: Radians(start + sweep(self.percent)),
                })
            });
            frame.stroke(
                &arc,
                Stroke {
                    style: Style::Solid(self.color),
                    width: STROKE,
                    line_cap: LineCap::Round,
                    ..Stroke::default()
                },
            );
        }
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ring_sweeps_only_the_clamped_occupancy() {
        assert_eq!(sweep(0), 0.0);
        assert!((sweep(50) - std::f32::consts::PI).abs() < 1e-6);
        assert!((sweep(100) - std::f32::consts::TAU).abs() < 1e-6);
        assert!((sweep(150) - std::f32::consts::TAU).abs() < 1e-6);
        assert_eq!(
            Ring {
                percent: 99,
                color: Color::BLACK
            }
            .percent,
            99
        );
    }
}
