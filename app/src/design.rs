//! Shared native Rosé Pine styles and a finite owned sidebar tween; no idle timer.
use iced::widget::{button, container, text_editor, text_input};
use iced::{Border, Color, Degrees, Theme, gradient};
// Dark Rosé Pine, declared once. `ui` re-exports these names because most
// modules spell them `crate::ui::ACCENT` and friends; nothing else may redeclare
// a palette entry.
pub const BASE: Color = Color::from_rgb8(0x19, 0x17, 0x24);
pub const SURFACE: Color = Color::from_rgb8(0x1f, 0x1d, 0x2e);
pub const OVERLAY: Color = Color::from_rgb8(0x26, 0x23, 0x3a);
pub const TEXT: Color = Color::from_rgb8(0xe0, 0xde, 0xf4);
pub const MUTED: Color = Color::from_rgb8(0x90, 0x8c, 0xaa);
pub const ACCENT: Color = Color::from_rgb8(0xc4, 0xa7, 0xe7);
pub const DANGER: Color = Color::from_rgb8(0xeb, 0x6f, 0x92);
pub const FOAM: Color = Color::from_rgb8(0x9c, 0xcf, 0xd8);
pub const GOLD: Color = Color::from_rgb8(0xf6, 0xc1, 0x77);
pub const LINE: Color = Color::from_rgb8(0x40, 0x3d, 0x52);
pub const SIDEBAR: f32 = 260.0;
pub const RAIL: f32 = 68.0;
pub const CHAT: f32 = 800.0;
pub const MIN_CHAT: f32 = 680.0;
pub const MAX_CHAT: f32 = 920.0;
pub const BREAKPOINT: f32 = 1000.0;
/// Horizontal inset for a disclosure's expanded body. The web indents those
/// bodies so they sit under the disclosure *title* rather than under its glyph:
/// `ReasoningRow.module.css` `.thinkBody` uses `calc(22px + delta)`, documented
/// there as the leading `16 + delta` plus a `6` gap. The same 22 already governed
/// this app's search-match offsets, so naming it makes the render agree with them.
pub const CONTENT_INSET: f32 = 22.0;
/// The fixed leading box a process row reserves for its glyph. The web's
/// `DisclosureRow` `.leading` is `calc(16px + delta)`, so the box holds the
/// chevron and the variant icon without the title shifting between them.
pub const LEADING_BOX: f32 = 16.0;
/// The web's transcript line boxes, in absolute pixels rather than a ratio:
/// body text is `calc(24px + var(--dsh-content-font-delta))`
/// (`AssistantMarkdown.module.css:13`, `ToolRow.module.css:42`) and compact or
/// secondary text is `calc(20px + var(--dsh-content-font-delta-secondary))`
/// (`MarkdownText.module.css:334`, `ReasoningRow.module.css:49`). Iced's relative
/// `LineHeight::default()` is `1.3`, which made every native line 19.5px where
/// the web gives 24.
pub const LINE_BODY: f32 = 24.0;
pub const LINE_COMPACT: f32 = 20.0;
/// The transcript body size: the web's `--dsh-content-font-size` default
/// (`gradient-shadow-text.css:57`). Native reserved and drew body text at 15,
/// which was a size the web never uses; with an absolute line box it also made
/// the ratio wrong.
pub const BODY_SIZE: f32 = 14.0;
/// The secondary tier: the web's `--dsh-content-font-size-secondary`
/// (`gradient-shadow-text.css:58`), which is 13 at the default size and steps
/// one under the body's own delta below it, two above.
pub const SECONDARY_SIZE: f32 = 13.0;
/// The gap between assistant blocks. The web's assistant body is a flex column
/// with `gap: 16px` (`AssistantMarkdown.module.css:20`) and its markdown blocks
/// carry `margin: 16px 0` (`MarkdownText.module.css:33`), where native used 12.
pub const BLOCK_GAP: f32 = 16.0;
/// The body line box at a content-size delta.
pub fn body_line(delta: f32) -> iced::advanced::text::LineHeight {
    iced::advanced::text::LineHeight::Absolute(iced::Pixels(LINE_BODY + delta))
}
/// The compact line box, at the secondary delta.
pub fn compact_line(delta_secondary: f32) -> iced::advanced::text::LineHeight {
    iced::advanced::text::LineHeight::Absolute(iced::Pixels(LINE_COMPACT + delta_secondary))
}
/// A markdown heading's size. The web's ladder is
/// `--dsw-font-markdown-h1..h3` = 21/19/18 with `h4` and below at the body size
/// (`gradient-shadow-text.css:61-74`), and native's type base sits one pixel
/// above the web's (body 15 against 14), so the ladder is 22/20/19 and the
/// fourth level falls back to the body.
///
/// One ladder for every surface: the transcript used 23/20/17 and the detail
/// panel 25/21/18, so the same document rendered at two different sizes.
pub fn heading_size(level: u8) -> f32 {
    match level {
        1 => 22.0,
        2 => 20.0,
        3 => 19.0,
        _ => 15.0,
    }
}
/// The web's default content size, and the range its settings offer.
pub const CONTENT_FONT_DEFAULT: f32 = 14.0;
pub const CONTENT_FONT_MIN: f32 = 11.0;
pub const CONTENT_FONT_MAX: f32 = 20.0;
/// The secondary line the transcript steps down to at the default size.
const CONTENT_SECONDARY_DEFAULT: f32 = 13.0;
/// A size outside the offered range is treated as the default rather than
/// clamped into a value no control can produce, so a corrupt or hand-edited
/// file cannot pin the UI somewhere the user cannot leave.
pub fn content_size(size: f32) -> f32 {
    if size.is_finite() && (CONTENT_FONT_MIN..=CONTENT_FONT_MAX).contains(&size) {
        size
    } else {
        CONTENT_FONT_DEFAULT
    }
}
/// `--dsh-content-font-delta`: what a size preference adds to every row height
/// and icon box, which the web writes as `calc(<base> + var(…delta, 0px))`.
pub fn content_delta(size: f32) -> f32 {
    content_size(size) - CONTENT_FONT_DEFAULT
}
/// `--dsh-content-font-size-secondary`: one step under the body — the setting
/// minus one at or below the default, minus two above it, as the web documents.
pub fn content_secondary(size: f32) -> f32 {
    let size = content_size(size);
    if size <= CONTENT_FONT_DEFAULT {
        size - 1.0
    } else {
        size - 2.0
    }
}
/// `--dsh-content-font-delta-secondary`, which drives line-heights rather than
/// the row boxes the primary delta drives.
pub fn content_delta_secondary(size: f32) -> f32 {
    content_secondary(size) - CONTENT_SECONDARY_DEFAULT
}
/// The main window's initial size, and the size the App assumes before its first
/// `Resized` event. The web desktop shell creates a 1280x820 window, so that is
/// the reference; these were two independent literals (1200x800 created,
/// 1280x820 assumed) and the lane/compact decisions ran on the wrong one.
pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 820.0;
/// Web transcript column: `clamp(680px, 64% of the conversation column, 920px)`
/// (ui-conversation `ConversationRoot`). `column` is the conversation pane
/// width before its horizontal padding. Malformed input falls back to `CHAT`.
///
/// Test-only: production always has a preference slot to fill, so it calls
/// [`chat_width_with`] and this spelling exists to keep the clamp assertions
/// readable.
#[cfg(test)]
pub fn chat_width(column: f32) -> f32 {
    chat_width_with(column, None)
}
/// Floor for a user-set conversation content width. The web calls this
/// `CONTENT_MIN` and matches its layout center-column minimum.
pub const CHAT_CONTENT_MIN: f32 = 640.0;
/// Horizontal room the web reserves for both drag handles and their safe edge
/// zones (`CONTENT_EDGE_BUDGET`); it bounds how wide a set preference may be.
pub const CHAT_EDGE_BUDGET: f32 = 176.0;
/// Conversation content width with the web's own precedence: a stored
/// preference wins and is clamped into `[CHAT_CONTENT_MIN, max(CHAT_CONTENT_MIN,
/// column - CHAT_EDGE_BUDGET)]`; otherwise the adaptive clamp applies. Mirrors
/// `resolveContentWidth` in `ui-conversation/.../ConversationWidthControls.tsx`.
///
/// A malformed column keeps the pre-existing `CHAT` fallback and ignores the
/// preference: without a measured column there is no bound to clamp against,
/// and inventing one would let a stored width exceed the pane.
pub fn chat_width_with(column: f32, preference: Option<f32>) -> f32 {
    if !column.is_finite() || column <= 0.0 {
        return CHAT;
    }
    if let Some(preference) = preference.filter(|value| value.is_finite() && *value > 0.0) {
        let max = CHAT_CONTENT_MIN.max(column - CHAT_EDGE_BUDGET);
        return preference.clamp(CHAT_CONTENT_MIN, max);
    }
    (column * 0.64).clamp(MIN_CHAT, MAX_CHAT)
}
/// Which width handle is being dragged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WidthSide {
    Left,
    Right,
}
/// Content width while a drag is in progress. Mirrors `outwardWidth` in
/// `ui-conversation/.../ConversationWidthControls.tsx`: the outward delta is
/// doubled, because the content is centered, so dragging one handle by `dx`
/// moves both edges and widens the column by `2dx`. The left handle mirrors the
/// sign. The result is deliberately unclamped: the caller resolves it through
/// [`chat_width_with`], so dragging past either bound simply stops growing.
pub fn chat_width_drag(base: f32, origin_x: f32, current_x: f32, side: WidthSide) -> f32 {
    let dx = current_x - origin_x;
    let outward = match side {
        WidthSide::Right => dx,
        WidthSide::Left => -dx,
    };
    base + outward * 2.0
}
/// A message's attachment card: the reference's `.fileCard` is a fixed
/// `240px` box with a `64px` minimum height (`MessageItem.module.css:320-332`),
/// whose name and meta lines (22 and 15) fit inside it at every content size —
/// only `.bubble` follows `--dsh-content-font-size`.
pub const ATTACHMENT_CARD_WIDTH: f32 = 240.0;
pub const ATTACHMENT_CARD_HEIGHT: f32 = 64.0;
/// `.userStack { gap: 8px }`, the gap between the attachment row and the bubble
/// and between the cards themselves (`MessageItem.module.css:10-21`).
pub const ATTACHMENT_CARD_GAP: f32 = 8.0;
/// The height `count` attachment cards add above a user bubble: one card each,
/// `ATTACHMENT_CARD_GAP` between them, and the same gap before the bubble when
/// one is drawn. The reservation and the view both read this, so a card cannot
/// be drawn without being measured.
pub fn attachment_height(count: usize, has_bubble: bool) -> f32 {
    if count == 0 {
        return 0.0;
    }
    let count = count as f32;
    count * ATTACHMENT_CARD_HEIGHT
        + (count - 1.0) * ATTACHMENT_CARD_GAP
        + if has_bubble { ATTACHMENT_CARD_GAP } else { 0.0 }
}
// Native radius scale, mapped to the web unified scale (docs/NATIVE-UI-PARITY.md):
// sm≈8, md=12, lg=16, xl=20. Native rounds sm to 9 and lg to 18; panel stays a
// tighter 20 than the web 28. Keep these stable — the parity skill forbids
// arbitrary redesigns.
pub const RADIUS_SM: f32 = 9.0;
pub const RADIUS_MD: f32 = 12.0;
pub const RADIUS_LG: f32 = 18.0;
pub const RADIUS_XL: f32 = 20.0;
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
        border: border(RADIUS_SM, Color::TRANSPARENT, 0.0),
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
        border: border(RADIUS_MD, Color::TRANSPARENT, 0.0),
        ..Default::default()
    }
}
pub fn navigation(theme: &Theme, status: button::Status, selected: bool) -> button::Style {
    let mut style = ghost(theme, status);
    if selected && !matches!(status, button::Status::Disabled) {
        style.background = Some(OVERLAY.into());
        // The web keeps primary ink on a selected row and changes only the
        // surface: its sidebar sets `--dsw-alias-label-primary` on `.panelRow`
        // and re-asserts it on `.panelRow.panelActive`, with no accent state
        // anywhere. Accent here contradicted the callers, which already pin the
        // title to TEXT.
        style.text_color = TEXT;
    }
    style
}
pub fn danger(_: &Theme, status: button::Status) -> button::Style {
    let disabled = matches!(status, button::Status::Disabled);
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(OVERLAY.into()),
        text_color: if disabled { MUTED } else { DANGER },
        border: border(RADIUS_SM, Color::TRANSPARENT, 0.0),
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
        // Constant width, transparent on blur and accent on focus: the caret is
        // visible without the box shifting a pixel as focus moves, which is what
        // a 0.0-width ring did (it could never draw). `text_input` above draws
        // its ring the same way.
        border: border(
            RADIUS_MD,
            if matches!(status, text_editor::Status::Focused { .. }) {
                ACCENT
            } else {
                Color::TRANSPARENT
            },
            1.0,
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
/// A message's attachment card. The reference's `.fileCard` fills with
/// `--dsw-specific-input-major` — the same bluish-850 the bubble uses — and
/// draws `0.5px solid var(--dsw-alias-border-l2)`
/// (`MessageItem.module.css:320-332`); in dark mode that edge resolves to
/// `rgba(255,255,255,0.12)` over the fill, which is this palette's `LINE`, so
/// the card keeps the reference's visible hairline instead of `card`'s
/// same-as-fill stroke.
pub fn file_card(_: &Theme) -> container::Style {
    container::Style {
        background: Some(OVERLAY.into()),
        text_color: Some(TEXT),
        border: border(RADIUS_XL, LINE, 0.5),
        ..Default::default()
    }
}
/// Height of the fade above the composer. The web fades over its composer seat
/// with `linear-gradient(180deg, … 0px, var(--dsw-alias-bg-base) 36px)`.
pub const COMPOSER_FADE_HEIGHT: f32 = 36.0;
/// The fade itself: transparent at the top, base colour by its bottom, so
/// scrolled content dissolves instead of meeting the composer at a hard edge.
///
/// `Degrees(180.0)` reads downward here because `Radians::to_distance` builds
/// its direction from `angle - FRAC_PI_2`, which yields `(0, 1)` — increasing y
/// — at 180 degrees on a y-down screen.
pub fn composer_fade(_: &Theme) -> container::Style {
    container::Style {
        background: Some(
            gradient::Linear::new(Degrees(180.0))
                .add_stop(0.0, Color { a: 0.0, ..BASE })
                .add_stop(1.0, BASE)
                .into(),
        ),
        ..Default::default()
    }
}
pub fn picker(
    _: &Theme,
    status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    iced::widget::pick_list::Style {
        text_color: TEXT,
        placeholder_color: MUTED,
        handle_color: MUTED,
        background: OVERLAY.into(),
        border: border(
            RADIUS_SM,
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
    fn attachment_cards_reserve_their_own_box_and_the_stack_gap() {
        // No cards, no height, and a file-only message has no gap after the last
        // card because `.userStack` gaps sit between children.
        assert_eq!(attachment_height(0, true), 0.0);
        assert_eq!(attachment_height(1, false), ATTACHMENT_CARD_HEIGHT);
        // One card above a bubble is the card plus one 8px gap: 72px.
        assert_eq!(attachment_height(1, true), 72.0);
        // Two cards add one more card and one more gap: 64 + 8 + 64 + 8.
        assert_eq!(attachment_height(2, true), 144.0);
        assert_eq!(attachment_height(2, false), 136.0);
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
    #[test]
    fn chat_width_matches_the_web_clamp_with_a_bounded_fallback() {
        // Narrow columns sit on the 680 floor, wide ones on the 920 cap.
        assert_eq!(chat_width(780.0), MIN_CHAT);
        assert_eq!(chat_width(2300.0), MAX_CHAT);
        // 64% governs the middle band.
        let middle = chat_width(1180.0);
        assert!(middle > MIN_CHAT && middle < MAX_CHAT);
        assert!((middle - 755.2).abs() < 0.05, "mid-band width {middle}");
        // Boundaries: the floor ends at 680/0.64 and the cap at 920/0.64.
        assert_eq!(chat_width(1062.0), MIN_CHAT);
        assert_eq!(chat_width(1438.0), MAX_CHAT);
        // Malformed or non-positive columns fall back to the shipped default.
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0] {
            assert_eq!(chat_width(value), CHAT, "column {value}");
        }
    }
    #[test]
    fn a_set_width_preference_wins_and_is_clamped_like_the_web() {
        // Mirrors resolveContentWidth: the bound is max(CONTENT_MIN, column - EDGE_BUDGET).
        assert_eq!(chat_width_with(1200.0, Some(800.0)), 800.0);
        // Below the floor clamps up; beyond the pane budget clamps down.
        assert_eq!(chat_width_with(1200.0, Some(100.0)), CHAT_CONTENT_MIN);
        assert_eq!(
            chat_width_with(1200.0, Some(5000.0)),
            1200.0 - CHAT_EDGE_BUDGET
        );
        // A pane too narrow for the budget keeps the floor as both bounds.
        assert_eq!(chat_width_with(700.0, Some(900.0)), CHAT_CONTENT_MIN);
        // Unusable values are treated as absent, so the adaptive clamp still applies.
        for value in [f32::NAN, f32::INFINITY, 0.0, -4.0] {
            assert_eq!(
                chat_width_with(1200.0, Some(value)),
                chat_width(1200.0),
                "preference {value}"
            );
        }
        assert_eq!(chat_width_with(1200.0, None), chat_width(1200.0));
        // A malformed column ignores a set preference rather than inventing a bound.
        assert_eq!(chat_width_with(f32::NAN, Some(800.0)), CHAT);
    }
    #[test]
    fn the_composer_fade_runs_transparent_to_base_over_its_height() {
        let style = composer_fade(&Theme::Dark);
        let Some(iced::Background::Gradient(gradient::Gradient::Linear(linear))) = style.background
        else {
            panic!("the composer fade must be a linear gradient");
        };
        let stops: Vec<_> = linear.stops.iter().flatten().collect();
        assert_eq!(stops.len(), 2, "exactly two stops");
        assert_eq!(stops[0].offset, 0.0);
        assert_eq!(stops[0].color.a, 0.0, "fully transparent at the top");
        assert_eq!(stops[1].offset, 1.0);
        assert_eq!(stops[1].color, BASE, "opaque base at the bottom");
        // 180 degrees is downward, so the transparent stop is the upper one.
        assert_eq!(linear.angle, iced::Radians::from(Degrees(180.0)));
    }
    #[test]
    fn the_content_size_preference_derives_the_web_deltas() {
        // The default is the identity: nothing about the shipped layout moves
        // until a preference is actually set.
        assert_eq!(content_delta(CONTENT_FONT_DEFAULT), 0.0);
        assert_eq!(content_delta_secondary(CONTENT_FONT_DEFAULT), 0.0);
        assert_eq!(content_secondary(CONTENT_FONT_DEFAULT), 13.0);
        // The delta is the setting minus the default, at both ends.
        assert_eq!(content_delta(CONTENT_FONT_MIN), -3.0);
        assert_eq!(content_delta(CONTENT_FONT_MAX), 6.0);
        // Secondary steps down one at or below the default, two above it.
        assert_eq!(content_secondary(11.0), 10.0);
        assert_eq!(content_secondary(14.0), 13.0);
        assert_eq!(content_secondary(15.0), 13.0);
        assert_eq!(content_secondary(16.0), 14.0);
        assert_eq!(content_secondary(20.0), 18.0);
        assert_eq!(content_delta_secondary(20.0), 5.0);
        // Out-of-range and unusable values behave as the default, not as a clamp
        // to a size the controls cannot produce.
        for value in [f32::NAN, f32::INFINITY, 0.0, -4.0, 10.0, 21.0, 100.0] {
            assert_eq!(content_delta(value), 0.0, "size {value}");
            assert_eq!(content_secondary(value), 13.0, "size {value}");
        }
    }
    #[test]
    fn the_heading_ladder_matches_the_web_tokens_at_natives_type_step() {
        // The web publishes 21/19/18 and the body size from h4 down; native's type
        // base is one pixel above it, and one ladder serves every surface.
        assert_eq!(heading_size(1), 22.0);
        assert_eq!(heading_size(2), 20.0);
        assert_eq!(heading_size(3), 19.0);
        assert_eq!(heading_size(4), 15.0);
        assert_eq!(heading_size(6), 15.0);
    }
    #[test]
    fn dragging_outward_widens_by_twice_the_pointer_delta() {
        // The right handle widens when the pointer moves right, and each pointer
        // pixel counts twice because both centered edges move.
        assert_eq!(
            chat_width_drag(700.0, 100.0, 110.0, WidthSide::Right),
            720.0
        );
        // The left handle mirrors the sign, so dragging left is still outward.
        assert_eq!(chat_width_drag(700.0, 100.0, 90.0, WidthSide::Left), 720.0);
        // Inward shrinks, and a press that never moves changes nothing.
        assert_eq!(chat_width_drag(700.0, 100.0, 90.0, WidthSide::Right), 680.0);
        assert_eq!(
            chat_width_drag(700.0, 100.0, 100.0, WidthSide::Right),
            700.0
        );
        // The raw result is unclamped; resolving it applies the floor and budget.
        let dragged = chat_width_drag(900.0, 0.0, 400.0, WidthSide::Right);
        assert_eq!(dragged, 1700.0);
        assert_eq!(
            chat_width_with(1200.0, Some(dragged)),
            1200.0 - CHAT_EDGE_BUDGET
        );
    }
}
