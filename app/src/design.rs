//! Shared native Rosé Pine styles and a finite owned sidebar tween; no idle timer.
use crate::ui::{ACCENT, DANGER, MUTED, SURFACE, TEXT};
use iced::widget::{button, container, text_editor, text_input};
use iced::{Border, Color, Theme};
pub const BASE: Color = Color::from_rgb8(0x19, 0x17, 0x24);
pub const OVERLAY: Color = Color::from_rgb8(0x26, 0x23, 0x3a);
pub const FOAM: Color = Color::from_rgb8(0x9c, 0xcf, 0xd8);
pub const LINE: Color = Color::from_rgb8(0x40, 0x3d, 0x52);
pub const SIDEBAR: f32 = 260.0;
pub const RAIL: f32 = 68.0;
pub const CHAT: f32 = 800.0;
pub const BREAKPOINT: f32 = 1000.0;
fn border(radius: f32, color: Color, width: f32) -> Border {
    Border {
        radius: radius.into(),
        color,
        width,
    }
}
pub fn ghost(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(OVERLAY.into()),
        text_color: if matches!(status, button::Status::Disabled) {
            MUTED
        } else {
            TEXT
        },
        border: border(9.0, Color::TRANSPARENT, 0.0),
        ..Default::default()
    }
}
pub fn primary(_: &Theme, status: button::Status) -> button::Style {
    let disabled = matches!(status, button::Status::Disabled);
    let color = if disabled {
        OVERLAY
    } else if matches!(status, button::Status::Pressed) {
        ACCENT
    } else {
        FOAM
    };
    button::Style {
        background: Some(color.into()),
        text_color: if disabled { MUTED } else { BASE },
        border: border(12.0, Color::TRANSPARENT, 0.0),
        ..Default::default()
    }
}
pub fn navigation(theme: &Theme, status: button::Status, selected: bool) -> button::Style {
    let mut style = ghost(theme, status);
    if selected && !matches!(status, button::Status::Disabled) {
        style.background = Some(OVERLAY.into());
        style.text_color = ACCENT;
    }
    style
}
pub fn danger(_: &Theme, status: button::Status) -> button::Style {
    let disabled = matches!(status, button::Status::Disabled);
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(OVERLAY.into()),
        text_color: if disabled { MUTED } else { DANGER },
        border: border(9.0, Color::TRANSPARENT, 0.0),
        ..Default::default()
    }
}
pub fn field(_: &Theme, status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: SURFACE.into(),
        border: border(
            10.0,
            if matches!(status, text_input::Status::Focused { .. }) {
                ACCENT
            } else {
                OVERLAY
            },
            1.0,
        ),
        icon: MUTED,
        placeholder: MUTED,
        value: if matches!(status, text_input::Status::Disabled) {
            MUTED
        } else {
            TEXT
        },
        selection: OVERLAY,
    }
}
pub fn editor(_: &Theme, status: text_editor::Status) -> text_editor::Style {
    text_editor::Style {
        background: SURFACE.into(),
        border: border(
            12.0,
            if matches!(status, text_editor::Status::Focused { .. }) {
                ACCENT
            } else {
                Color::TRANSPARENT
            },
            0.0,
        ),
        placeholder: MUTED,
        value: TEXT,
        selection: OVERLAY,
    }
}
pub fn card(background: Color, radius: f32) -> container::Style {
    container::Style {
        background: Some(background.into()),
        text_color: Some(TEXT),
        border: border(radius, OVERLAY, 1.0),
        ..Default::default()
    }
}
// Stable component API; keep the original short names for existing consumers.
pub fn button_style(theme: &Theme, status: button::Status) -> button::Style {
    ghost(theme, status)
}
pub fn primary_button(theme: &Theme, status: button::Status) -> button::Style {
    primary(theme, status)
}
pub fn danger_button(theme: &Theme, status: button::Status) -> button::Style {
    danger(theme, status)
}
pub fn input_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    field(theme, status)
}
pub fn picker_style(
    _: &Theme,
    status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    iced::widget::pick_list::Style {
        text_color: TEXT,
        placeholder_color: MUTED,
        handle_color: MUTED,
        background: OVERLAY.into(),
        border: border(
            9.0,
            if matches!(
                status,
                iced::widget::pick_list::Status::Hovered
                    | iced::widget::pick_list::Status::Opened { .. }
            ) {
                LINE
            } else {
                Color::TRANSPARENT
            },
            1.0,
        ),
    }
}

/// Finite, retargetable logical-width animation; no subscription when settled.
#[derive(Clone, Debug)]
pub struct SidebarMotion {
    value: f32,
    from: f32,
    target: f32,
    started: Option<std::time::Instant>,
}
impl SidebarMotion {
    const DURATION: std::time::Duration = std::time::Duration::from_millis(180);
    pub fn new(width: f32) -> Self {
        let width = width.clamp(RAIL, SIDEBAR);
        Self {
            value: width,
            from: width,
            target: width,
            started: None,
        }
    }
    pub fn value(&self) -> f32 {
        self.value
    }
    pub fn running(&self, visible: bool) -> bool {
        visible && self.started.is_some()
    }
    pub fn snap(&mut self, target: f32) {
        self.value = target.clamp(RAIL, SIDEBAR);
        self.from = self.value;
        self.target = self.value;
        self.started = None;
    }
    pub fn retarget(&mut self, target: f32, now: std::time::Instant, visible: bool) {
        self.advance(now);
        let target = target.clamp(RAIL, SIDEBAR);
        if !visible || (self.value - target).abs() < 0.01 {
            self.snap(target);
            return;
        }
        self.from = self.value;
        self.target = target;
        self.started = Some(now);
    }
    pub fn advance(&mut self, now: std::time::Instant) {
        let Some(started) = self.started else {
            return;
        };
        let elapsed = now.saturating_duration_since(started);
        if elapsed >= Self::DURATION {
            self.snap(self.target);
            return;
        }
        let t = elapsed.as_secs_f32() / Self::DURATION.as_secs_f32();
        self.value = (self.from + (self.target - self.from) * (1.0 - (1.0 - t).powi(3)))
            .clamp(RAIL, SIDEBAR);
    }
}
#[cfg(test)]
mod motion_tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn idle_hidden_and_settled_motion_has_no_subscription() {
        let now = Instant::now();
        let mut m = SidebarMotion::new(SIDEBAR);
        assert!(!m.running(true));
        m.retarget(RAIL, now, true);
        assert!(m.running(true));
        assert!(!m.running(false));
        m.advance(now + Duration::from_millis(181));
        assert_eq!(m.value(), RAIL);
        assert!(!m.running(true));
        m.retarget(SIDEBAR, now, false);
        assert_eq!(m.value(), SIDEBAR);
        assert!(!m.running(true));
    }
    #[test]
    fn interrupted_motion_is_continuous_and_clamped() {
        let now = Instant::now();
        let mut m = SidebarMotion::new(SIDEBAR);
        m.retarget(RAIL, now, true);
        m.advance(now + Duration::from_millis(60));
        let middle = m.value();
        assert!(middle > RAIL && middle < SIDEBAR);
        m.retarget(SIDEBAR, now + Duration::from_millis(60), true);
        assert_eq!(m.value(), middle);
        m.advance(now + Duration::from_secs(1));
        assert_eq!(m.value(), SIDEBAR);
        m.snap(-100.0);
        assert_eq!(m.value(), RAIL);
        m.snap(1000.0);
        assert_eq!(m.value(), SIDEBAR);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(color: Color) -> f32 {
        fn linear(v: f32) -> f32 {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    }
    #[test]
    fn primary_enabled_label_has_accessible_contrast() {
        let style = primary(&Theme::Dark, button::Status::Active);
        let Some(iced::Background::Color(fill)) = style.background else {
            panic!("primary fill required");
        };
        let contrast = (luminance(fill) + 0.05) / (luminance(style.text_color) + 0.05);
        assert!(contrast >= 4.5);
    }
    #[test]
    fn ghost_does_not_fill_idle_secondary_actions() {
        assert!(
            ghost(&Theme::Dark, button::Status::Active)
                .background
                .is_none()
        );
        assert!(
            ghost(&Theme::Dark, button::Status::Hovered)
                .background
                .is_some()
        );
        assert_eq!(
            primary(&Theme::Dark, button::Status::Disabled).text_color,
            MUTED
        );
    }
}
