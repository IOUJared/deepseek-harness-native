//! Cached native preview geometry. This measures display text only, never durable records.
//!
//! The caller must supply the same bounded preview it renders, and render its body at
//! `width - 28` by `body_height`, with DejaVu Sans / 15px / Advanced / default
//! line-height / Word wrapping. Routine messages have no role header; live and
//! interrupted responses retain a 24px status plus 7px gap. A settled row reserves
//! no body padding: only the bubble pads its own text, and the gap between rows is
//! the web's flow margin, which the view draws as the row's top padding. The App
//! adds the action row and its gap. Fonts must remain stable after bootstrap;
//! recreate the cache if fonts change.
use iced::advanced::text::{self, Paragraph as _};
use iced::{Font, Pixels, Size, alignment};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::fmt;

pub(crate) fn status_header(role: &str) -> bool {
    role == "Assistant · live" || role == "Interrupted assistant attempt"
}

/// The status line a live or interrupted attempt reserves above its body: the
/// reference's 24px status line plus the 7px gap under it.
pub(crate) const STATUS_CHROME: f32 = 31.0;

/// The vertical box a settled row reserves around its body. A row's leading gap is
/// not part of this: the view draws it as the row's top padding from the flow ladder
/// (`flow_gap_before`) and `row_geometries` reserves it separately.
///
/// The reference pads no unboxed body. Assistant prose starts at the column's flow
/// edge (`AssistantMarkdown.module.css:9-18`) and only the bubble pads its own text,
/// `10px 16px` (`MessageItem.module.css:22-40`), so a settled assistant row's height
/// is its body and a user row's is the bubble's 10px on each side. Native reserved
/// 16px above and below every message body, so each of those rows measured 32px
/// taller than the reference's and its first line sat 10px below the flow margin.
pub(crate) fn chrome_height(role: &str) -> f32 {
    let padding = if role == "You" {
        crate::design::USER_BUBBLE_PAD.top + crate::design::USER_BUBBLE_PAD.bottom
    } else {
        0.0
    };
    padding
        + if status_header(role) {
            STATUS_CHROME
        } else {
            0.0
        }
}

/// The drawn offset from a row's top to its body: the bubble's own 10px for a user
/// row, nothing for a settled assistant row, and the status block for a live or
/// interrupted one. The search jump reads this to scroll a hit into view.
pub(crate) fn body_top(role: &str) -> f32 {
    let padding = if role == "You" {
        crate::design::USER_BUBBLE_PAD.top
    } else {
        0.0
    };
    padding
        + if status_header(role) {
            STATUS_CHROME
        } else {
            0.0
        }
}

const MAX_ENTRIES: usize = 4097;
/// The compact tool line, and the outer vertical padding every row carries.
pub(crate) const TOOL_LINE_HEIGHT: f32 = 24.0;
/// The compact tool line at a content size. The web writes this row as
/// `calc(24px + var(--dsh-content-font-delta, 0px))`, so a larger content size
/// buys the line more room rather than clipping it.
pub(crate) fn tool_line_height(delta: f32) -> f32 {
    TOOL_LINE_HEIGHT + delta
}
pub(crate) const ROW_CHROME: f32 = 12.0;
/// The line a process group's seat header draws above its first member. Its title is a
/// flex row that pads nothing and inherits the body's type (`ChatGroupSeat.module.css:5-20`:
/// `padding: 0`, `font: inherit`), so the header is one 24px line box; the seat is drawn
/// inside the first member's row box, and this is what that row reserves for it. When the
/// two were separate values the reservation stayed in the row as an empty band under the
/// first tool line — the 24px gap round 511 removed.
pub(crate) fn seat_height(delta: f32) -> f32 {
    crate::design::LINE_BODY + delta
}
/// The gap an expanded seat puts between its title and its first member: the reference's
/// expanded title takes `padding-bottom: 8px` (`ChatGroupSeat.module.css:82-84`). A
/// collapsed header has none.
pub(crate) const SEAT_TITLE_GAP: f32 = 8.0;
/// A settled turn's process control: the reference draws the line at
/// `height: calc(33px + var(--dsh-content-font-delta))` with `padding: 0 0 8px` and a 0.5px
/// bottom rule over the 24px label line (`TurnProcessNodeView.module.css:1-13`), so the line,
/// its 8px gap and a hairline add up to that height.
pub(crate) fn turn_control_height(delta: f32) -> f32 {
    crate::design::LINE_BODY + delta + SEAT_TITLE_GAP + 1.0
}
/// Whole lines a collapsed reply previews; the body cap is this many line boxes.
const PREVIEW_LINES: f32 = 4.0;
/// The vertical gap the expanded tool body sits below its compact line. The view
/// and the geometry both read this one constant: when only the view reserved it,
/// a row whose body was measured at the same line count came out 4px short and
/// the bottom of the body was clipped.
pub(crate) const TOOL_BODY_GAP: f32 = 4.0;
/// The horizontal inset a row body is measured and rendered at, so the two agree.
///
/// Zero: the reference's chat rows sit flush with the message column. `.column` is the only
/// horizontal box they live in and adds no padding (`ChatView.module.css:54-59`), the assistant
/// body adds none of its own (`AssistantMarkdown.module.css:9-18`), and the user side pads only
/// *inside* `.bubble` (`MessageItem.module.css:22-40`). Native used to reserve 28px here (two 14px
/// halves) plus an 8px outer inset, which put every body 18px inside the column.
pub(crate) const BODY_INSET: f32 = 0.0;
/// The narrowest lane whose user bubble still has room: the reference's 0.702
/// share must cover the 32px bubble inset plus one pixel of body. Below this the
/// lane is malformed and takes [`MALFORMED_LANE`].
const MIN_LANE: f32 = 47.0;
const MAX_LANE: f32 = 4096.0;
/// The lane a malformed width falls back to: an ordinary minimum bubble's 96px.
const MALFORMED_LANE: f32 = 96.0;
const FONT: Font = Font::with_name("DejaVu Sans");
type NativeParagraph = <iced::Renderer as text::Renderer>::Paragraph;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Geometry {
    /// Bubble/card width, excluding the outer horizontal 4px padding on each side.
    pub width: f32,
    pub body_height: f32,
    /// Entire virtual row height, including the outer vertical padding.
    pub height: f32,
}

struct Entry {
    role: String,
    preview: String,
    /// The summary line's height, part of the cached geometry and part of the entry's
    /// identity: a later `session-reference` record re-measures the row it lands on.
    summary: f32,
    lane_bits: u32,
    geometry: Geometry,
    // Two most recently used measurement widths, sharing one role/preview string.
    previous: Option<(u32, Geometry)>,
}

/// Entries are bounded to the retained 4096 records plus one partial preview.
/// Interior mutability permits measurements from the ordinary `view(&self)` path.
#[derive(Default)]
pub(crate) struct Cache {
    entries: RefCell<BTreeMap<u64, Entry>>,
    /// The content-size delta every stored height was computed at. Changing it
    /// invalidates the cache, because a measurement belongs to one text size.
    delta: std::cell::Cell<f32>,
    #[cfg(any(test, feature = "public-layout-fixture"))]
    shapes: std::cell::Cell<usize>,
    #[cfg(feature = "public-layout-fixture")]
    shape_ns: std::cell::Cell<u64>,
    #[cfg(feature = "public-layout-fixture")]
    hits: std::cell::Cell<usize>,
}

impl fmt::Debug for Cache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Do not expose stored preview text through diagnostic formatting.
        formatter
            .debug_struct("Cache")
            .field("entries", &self.entries.borrow().len())
            .finish_non_exhaustive()
    }
}

/// The width a human row is measured at: the user-stack cap the bubble and the
/// referenced-session summary share (`MessageItem.module.css:10-21`).
pub(crate) fn human_measure_width(lane: f32) -> f32 {
    lane.min((lane * crate::design::USER_BUBBLE_SHARE).min(lane * crate::design::USER_STACK_SHARE))
}
/// The height the referenced-session summary adds to a row: the user stack's gap plus
/// the line's own measured height at that same width, so a label list that wraps
/// reserves every wrapped line rather than one.
pub(crate) fn reference_summary_height(labels: &[String], lane: f32, delta_secondary: f32) -> f32 {
    if labels.is_empty() {
        return 0.0;
    }
    let text = crate::reducer::reference_summary_text(labels);
    let span = crate::ui::detail_panel::formatted::Span {
        text,
        bold: false,
        italic: false,
        struck: false,
        code: false,
        alt: false,
    };
    let width = human_measure_width(lane).max(24.0);
    crate::design::ATTACHMENT_CARD_GAP
        + crate::ui::conversation_content::inline_height(
            std::slice::from_ref(&span),
            width,
            crate::design::SECONDARY_SIZE + delta_secondary,
            crate::design::reference_line(delta_secondary),
        )
}
impl Cache {
    /// The compact tool line at this cache's content size; the view draws the same
    /// value the geometry reserved.
    pub(crate) fn tool_line(&self) -> f32 {
        tool_line_height(self.delta.get())
    }
    /// Adopt a content-size delta, dropping measurements taken at the old one.
    pub(crate) fn set_content_delta(&self, delta: f32) {
        let delta = if delta.is_finite() { delta } else { 0.0 };
        if self.delta.get() != delta {
            self.delta.set(delta);
            self.entries.borrow_mut().clear();
        }
    }
    pub(crate) fn geometry(
        &self,
        key: u64,
        role: &str,
        preview: &str,
        // Extra height the reference's referenced-session summary line adds to this row,
        // computed by the caller that knows the secondary delta. Zero when none.
        summary: f32,
        lane_width: f32,
    ) -> Geometry {
        let lane = lane_width_checked(lane_width);
        let human = role == "You";
        let assistant = role.starts_with("Assistant") || role == "Interrupted assistant attempt";
        // The web user bubble is capped at 640 native px (≈70.2% of the widest
        // content column); the unboxed assistant column spans the full shared
        // content width. Fixed-height tools/opaque rows still need their actual
        // lane for card width. The fixed user cap keeps extra empty lane width
        // from re-measuring ordinary user rows during a resize.
        // The reference caps the user stack at `min(content * 0.702, 82%)`
        // (`MessageItem.module.css:10-21`); both terms move with the live column,
        // so a narrow lane narrows the bubble instead of holding a fixed cap.
        let user_cap =
            (lane * crate::design::USER_BUBBLE_SHARE).min(lane * crate::design::USER_STACK_SHARE);
        let measurement_width = if human { lane.min(user_cap) } else { lane };
        let lane_bits = measurement_width.to_bits();
        {
            let mut entries = self.entries.borrow_mut();
            if let Some(entry) = entries.get_mut(&key)
                && entry.role == role
                && entry.preview == preview
                && entry.summary == summary
            {
                if entry.lane_bits == lane_bits {
                    #[cfg(feature = "public-layout-fixture")]
                    self.hits.set(self.hits.get().saturating_add(1));
                    return entry.geometry;
                }
                if let Some((previous_bits, previous)) = entry.previous
                    && previous_bits == lane_bits
                {
                    entry.previous = Some((entry.lane_bits, entry.geometry));
                    entry.lane_bits = previous_bits;
                    entry.geometry = previous;
                    #[cfg(feature = "public-layout-fixture")]
                    self.hits.set(self.hits.get().saturating_add(1));
                    return previous;
                }
            }
        }
        let geometry =
            if role == "Tool call" || role == "Tool result" {
                Geometry {
                    width: lane,
                    body_height: self.tool_line(),
                    height: self.tool_line() + ROW_CHROME,
                }
            } else if !human && !assistant {
                Geometry {
                    width: lane,
                    body_height: 75.0,
                    height: 156.0,
                }
            } else {
                let max_width = measurement_width;
                #[cfg(feature = "public-layout-fixture")]
                let shaped = std::time::Instant::now();
                let inset = if human {
                    crate::design::USER_BUBBLE_INSET
                } else {
                    BODY_INSET
                };
                // The bubble draws its own line box, so the human row measures the
                // one the transcript will draw: `MessageItem.module.css:38` gives
                // `.bubble` 22px where the assistant body takes 24px.
                let line_px = if human {
                    crate::design::BUBBLE_LINE
                } else {
                    crate::design::LINE_BODY
                };
                let line = if human {
                    crate::design::bubble_line(self.delta.get())
                } else {
                    crate::design::body_line(self.delta.get())
                };
                let paragraph = NativeParagraph::with_text(text::Text {
                    content: preview,
                    bounds: Size::new(max_width - inset, f32::INFINITY),
                    // Measured at the size the transcript draws, so a content-size
                    // preference cannot reserve height for a smaller text than the
                    // one that renders (which clipped the last wrapped line).
                    size: Pixels(crate::design::BODY_SIZE + self.delta.get()),
                    // The same box the transcript draws in. Measuring with Iced's
                    // relative default (1.3) reserved 19.5px lines for text drawn
                    // in the web's 24px box, which clipped the last wrapped line.
                    line_height: line,
                    font: FONT,
                    align_x: text::Alignment::Default,
                    align_y: alignment::Vertical::Top,
                    shaping: text::Shaping::Advanced,
                    wrapping: text::Wrapping::Word,
                });
                #[cfg(any(test, feature = "public-layout-fixture"))]
                self.shapes.set(self.shapes.get().saturating_add(1));
                #[cfg(feature = "public-layout-fixture")]
                self.shape_ns.set(self.shape_ns.get().saturating_add(
                    u64::try_from(shaped.elapsed().as_nanos()).unwrap_or(u64::MAX),
                ));
                let measured = paragraph.min_bounds();
                let width = if human {
                    // Word wrapping may leave an unbreakable token wider than its bound.
                    // Keep the row bounded and let the rendering parent's body clip it.
                    let measured_width = if measured.width.is_finite() {
                        measured.width.max(0.0).ceil() + inset
                    } else {
                        max_width
                    };
                    measured_width.clamp(96.0_f32.min(max_width), max_width)
                } else {
                    max_width
                };
                let body_height = if measured.height.is_finite() {
                    // The floor is one line box and the cap is the four-line chat
                    // preview, so a collapsed reply always shows whole lines at the
                    // box the transcript draws (`PREVIEW_LINES`).
                    measured.height.clamp(line_px, PREVIEW_LINES * line_px)
                } else {
                    PREVIEW_LINES * line_px
                };
                // The reference's referenced-session summary is one 13px/18px line
                // under the bubble, inside `.userStack` (its 8px gap) and above the
                // action row (`MessageItem.module.css:10-45`, `MessageItem.tsx:225-229`).
                let summary_height = summary;
                Geometry {
                    width,
                    body_height,
                    height: chrome_height(role) + body_height + summary_height,
                }
            };

        let mut entries = self.entries.borrow_mut();
        if !entries.contains_key(&key) && entries.len() >= MAX_ENTRIES {
            // Durable sequence keys ascend; evict the earliest cached key, never a row.
            entries.pop_first();
        }
        let previous = entries
            .get(&key)
            .filter(|entry| {
                entry.role == role && entry.preview == preview && entry.summary == summary
            })
            .map(|entry| (entry.lane_bits, entry.geometry));
        entries.insert(
            key,
            Entry {
                role: role.into(),
                preview: preview.into(),
                summary,
                lane_bits,
                geometry,
                previous,
            },
        );
        geometry
    }

    #[cfg(feature = "public-layout-fixture")]
    pub(crate) fn observations(&self) -> (usize, usize, u64, usize) {
        (
            self.entries.borrow().len(),
            self.shapes.get(),
            self.shape_ns.get(),
            self.hits.get(),
        )
    }

    pub(crate) fn retain_keys(&self, keys: impl IntoIterator<Item = u64>) {
        let mut entries = self.entries.borrow_mut();
        // Filter before collecting: even a larger caller iterator cannot grow this
        // temporary set beyond the bounded number of cached entries.
        let retained: HashSet<_> = keys
            .into_iter()
            .filter(|key| entries.contains_key(key))
            .collect();
        entries.retain(|key, _| retained.contains(key));
    }
}

fn lane_width_checked(width: f32) -> f32 {
    if width.is_finite() {
        width.clamp(MIN_LANE, MAX_LANE)
    } else {
        MALFORMED_LANE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounded(geometry: Geometry, lane: f32) {
        assert!(geometry.width.is_finite() && geometry.width > 0.0 && geometry.width <= lane);
        assert!(geometry.body_height.is_finite() && geometry.body_height > 0.0);
        assert!(geometry.height.is_finite() && geometry.height > 0.0 && geometry.height <= 160.0);
    }

    #[test]
    fn a_settled_row_reserves_the_bubbles_padding_and_no_other_chrome() {
        let cache = Cache::default();
        for role in ["You", "Assistant"] {
            let geometry = cache.geometry(1, role, "PUBLIC", 0.0, 700.0);
            assert!(!status_header(role));
            // The bubble pads its own text 10px top and bottom
            // (`MessageItem.module.css:38`); every other settled row's body starts at
            // the column's flow edge, so its height is the body alone. The leading
            // flow gap is the view's top padding and not part of this reservation.
            let (top, chrome) = if role == "You" {
                (crate::design::USER_BUBBLE_PAD.top, 20.0)
            } else {
                (0.0, 0.0)
            };
            assert_eq!(body_top(role), top, "{role}");
            assert_eq!(geometry.height - geometry.body_height, chrome, "{role}");
        }
        for role in ["Assistant · live", "Interrupted assistant attempt"] {
            let geometry = cache.geometry(1, role, "PUBLIC", 0.0, 700.0);
            assert!(status_header(role));
            // The reference's status line is 24px with a 7px gap under it.
            assert_eq!(STATUS_CHROME, 24.0 + 7.0);
            assert_eq!(body_top(role), STATUS_CHROME);
            assert_eq!(geometry.height - geometry.body_height, STATUS_CHROME);
        }
    }

    #[test]
    fn two_recent_widths_reuse_actual_native_geometry_without_more_shapes() {
        let cache = Cache::default();
        let text = "PUBLIC native wide words ".repeat(6);
        let wide = cache.geometry(1, "You", &text, 0.0, 740.0);
        let narrow = cache.geometry(1, "You", &text, 0.0, 542.0);
        assert_eq!(cache.shapes.get(), 2);
        for _ in 0..16 {
            assert_eq!(cache.geometry(1, "You", &text, 0.0, 740.0), wide);
            assert_eq!(cache.geometry(1, "You", &text, 0.0, 542.0), narrow);
        }
        assert_eq!(cache.shapes.get(), 2);
        assert_eq!(cache.entries.borrow().len(), 1);
    }
    #[test]
    fn third_width_evicts_only_the_least_recent_geometry_not_the_row() {
        let cache = Cache::default();
        cache.geometry(1, "You", "PUBLIC", 0.0, 740.0);
        cache.geometry(1, "You", "PUBLIC", 0.0, 542.0);
        cache.geometry(1, "You", "PUBLIC", 0.0, 740.0);
        cache.geometry(1, "You", "PUBLIC", 0.0, 420.0);
        assert_eq!(cache.shapes.get(), 3);
        cache.geometry(1, "You", "PUBLIC", 0.0, 740.0);
        assert_eq!(cache.shapes.get(), 3);
        cache.geometry(1, "You", "PUBLIC", 0.0, 542.0);
        assert_eq!(cache.shapes.get(), 4);
        assert_eq!(cache.entries.borrow().len(), 1);
    }
    #[test]
    fn changed_preview_or_role_discards_both_cached_widths() {
        let cache = Cache::default();
        for lane in [740.0, 542.0] {
            cache.geometry(1, "You", "PUBLIC old", 0.0, lane);
        }
        for lane in [740.0, 542.0] {
            cache.geometry(1, "You", "PUBLIC replacement", 0.0, lane);
        }
        assert_eq!(cache.shapes.get(), 4);
        for lane in [740.0, 542.0] {
            cache.geometry(1, "Assistant", "PUBLIC replacement", 0.0, lane);
        }
        assert_eq!(cache.shapes.get(), 6);
        cache.retain_keys([]);
        assert!(cache.entries.borrow().is_empty());
    }
    #[test]
    fn user_cap_follows_the_column_share_while_the_assistant_column_follows_the_lane() {
        let cache = Cache::default();
        // The reference caps the user stack at `min(content * 0.702, 82%)`, so a
        // wider lane widens the bubble: the two lanes must measure differently
        // rather than reuse one fixed-cap geometry.
        let long = "PUBLIC words ".repeat(80);
        let wide = cache.geometry(1, "You", &long, 0.0, 4096.0).width;
        let narrow = cache.geometry(1, "You", &long, 0.0, 740.0).width;
        assert!(wide > narrow, "{wide} must exceed {narrow}");
        // Word wrapping leaves the widest line at or under the paragraph bound, so
        // the bubble sits at or under the cap rather than exactly on it.
        assert!(narrow <= 740.0 * crate::design::USER_BUBBLE_SHARE + 1.0);
        assert!(narrow > 740.0 * crate::design::USER_BUBBLE_SHARE - 64.0);
        assert_eq!(cache.shapes.get(), 2);
        // The unboxed assistant column tracks the shared content width.
        let assistant = cache.geometry(2, "Assistant", "PUBLIC", 0.0, 850.0);
        // The assistant column is the lane itself: the reference's rows add no horizontal inset
        // (`ChatView.module.css:54-59`), so nothing is subtracted here.
        assert_eq!(assistant.width, 850.0);
        assert_eq!(
            cache.geometry(2, "Assistant", "PUBLIC", 0.0, 1200.0).width,
            1200.0
        );
        assert_eq!(cache.shapes.get(), 4);
        let tool = cache.geometry(3, "Tool call", "PUBLIC", 0.0, 740.0);
        assert_ne!(
            cache.geometry(3, "Tool call", "PUBLIC", 0.0, 800.0).width,
            tool.width
        );
        // Two user lanes and two assistant lanes; a tool row's width is arithmetic
        // on the lane, so it never shapes.
        assert_eq!(cache.shapes.get(), 4);
    }
    #[test]
    fn a_message_row_spans_the_whole_lane_without_a_horizontal_inset() {
        let cache = Cache::default();
        // The reference's chat rows live in `.column`, which is the only horizontal box they sit
        // in and carries no padding of its own (`ChatView.module.css:54-59`), so every row spans
        // the lane. Native reserved 8px here plus 28px inside the row, which put each body 18px
        // inside the column.
        assert_eq!(
            cache.geometry(1, "Assistant", "PUBLIC", 0.0, 900.0).width,
            900.0
        );
        assert_eq!(
            cache.geometry(2, "Tool call", "PUBLIC", 0.0, 900.0).width,
            900.0
        );
        // The view draws a row body at `width - BODY_INSET`, so a non-zero inset would leave the
        // body short of the lane even when the row itself spans it.
        assert_eq!(BODY_INSET, 0.0);
        assert_eq!(
            cache.geometry(3, "Assistant", "PUBLIC", 0.0, 900.0).width - BODY_INSET,
            900.0
        );
        // The user bubble keeps the reference's 0.702 share of that same lane; the share is the
        // binding term, so no outer subtraction may shrink it further.
        let long = "PUBLIC words ".repeat(200);
        let bubble = cache.geometry(4, "You", &long, 0.0, 900.0).width;
        assert!(
            bubble > 900.0 * crate::design::USER_BUBBLE_SHARE - 64.0,
            "the bubble is measured at the lane's share, not a lane less an outer inset"
        );
        assert!(bubble <= 900.0 * crate::design::USER_BUBBLE_SHARE + 1.0);
    }

    #[test]
    fn cached_geometry_matches_fresh_native_oracle_across_caps_roles_and_unicode() {
        let cache = Cache::default();
        let roles = [
            "You",
            "Assistant",
            "Interrupted assistant attempt",
            "Tool call",
            "System",
        ];
        let widths = [
            f32::NAN,
            -1.0,
            37.0,
            104.0,
            542.0,
            647.99,
            648.0,
            648.01,
            740.0,
            807.99,
            808.0,
            808.01,
            850.0,
            4096.0,
            f32::INFINITY,
        ];
        for role in roles {
            for preview in [
                "PUBLIC short",
                "Γειά σου 世界 👩‍💻 مرحبا PUBLIC words\nsecond line",
            ] {
                for width in widths.into_iter().chain(widths.into_iter().rev()) {
                    let expected = Cache::default().geometry(1, role, preview, 0.0, width);
                    assert_eq!(
                        cache.geometry(1, role, preview, 0.0, width),
                        expected,
                        "cached metrics must preserve exact native geometry"
                    );
                }
            }
        }
    }
    #[test]
    fn retained_plus_live_capacity_bounds_both_geometry_slots_without_duplicate_plaintext() {
        let cache = Cache::default();
        for key in (0..4096).chain(std::iter::once(u64::MAX)) {
            cache.geometry(key, "You", "PUBLIC", 0.0, 740.0);
            cache.geometry(key, "You", "PUBLIC", 0.0, 542.0);
        }
        let shapes = cache.shapes.get();
        cache.retain_keys((0..4096).chain(std::iter::once(u64::MAX)));
        for key in (0..4096).chain(std::iter::once(u64::MAX)) {
            cache.geometry(key, "You", "PUBLIC", 0.0, 740.0);
            cache.geometry(key, "You", "PUBLIC", 0.0, 542.0);
        }
        let entries = cache.entries.borrow();
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert!(entries.values().all(|v| v.previous.is_some()));
        assert_eq!(cache.shapes.get(), shapes);
    }
    #[test]
    fn same_key_hits_reuse_shapes_and_preview_role_width_changes_invalidate() {
        let cache = Cache::default();
        let first = cache.geometry(7, "You", "Hi", 0.0, 600.0);
        assert_eq!(
            first.height,
            crate::design::USER_BUBBLE_PAD.top
                + crate::design::USER_BUBBLE_PAD.bottom
                + crate::design::BUBBLE_LINE
        );
        for _ in 0..8 {
            assert_eq!(cache.geometry(7, "You", "Hi", 0.0, 600.0), first);
        }
        assert_eq!(cache.shapes.get(), 1);
        let longer = cache.geometry(
            7,
            "You",
            "A substantially longer PUBLIC preview",
            0.0,
            600.0,
        );
        assert!(longer.width > first.width);
        let assistant = cache.geometry(
            7,
            "Assistant · live",
            "A substantially longer PUBLIC preview",
            0.0,
            600.0,
        );
        // The assistant column is the lane, with no outer inset of its own.
        assert_eq!(assistant.width, 600.0);
        let narrow = cache.geometry(
            7,
            "Assistant · live",
            "A substantially longer PUBLIC preview",
            0.0,
            140.0,
        );
        assert_eq!(narrow.width, 140.0);
        assert!(narrow.body_height > assistant.body_height);
        assert_eq!(cache.shapes.get(), 4);
        assert_eq!(
            cache.geometry(
                7,
                "Assistant · live",
                "A substantially longer PUBLIC preview",
                0.0,
                140.0
            ),
            narrow
        );
        assert_eq!(cache.shapes.get(), 4);
        assert_eq!(cache.entries.borrow().len(), 1);
    }

    #[test]
    fn tools_and_unknown_roles_preserve_fixed_geometry_without_shaping() {
        let cache = Cache::default();
        for role in ["Tool call", "Tool result"] {
            assert_eq!(
                cache.geometry(0, role, "PUBLIC", 0.0, 700.0),
                Geometry {
                    width: 700.0,
                    body_height: 24.0,
                    height: 36.0
                }
            );
        }
        for role in [
            "System",
            "Developer",
            "Tool call · future",
            "future/required",
            "approval/policy",
        ] {
            assert_eq!(
                cache.geometry(0, role, "PUBLIC", 0.0, 700.0),
                Geometry {
                    width: 700.0,
                    body_height: 75.0,
                    height: 156.0
                }
            );
        }
        assert_eq!(cache.shapes.get(), 0);
    }

    #[test]
    fn retain_and_cap_bound_entries_without_evicting_any_durable_data() {
        let cache = Cache::default();
        for key in 0..5000 {
            cache.geometry(key, "System", "PUBLIC", 0.0, 500.0);
        }
        assert_eq!(cache.entries.borrow().len(), MAX_ENTRIES);
        assert!(!cache.entries.borrow().contains_key(&0));
        assert!(cache.entries.borrow().contains_key(&4999));
        cache.retain_keys([4999, 4999, 4998, 0]);
        assert_eq!(cache.entries.borrow().len(), 2);
        assert!(cache.entries.borrow().contains_key(&4998));
        cache.retain_keys(std::iter::empty());
        assert!(cache.entries.borrow().is_empty());
        assert_eq!(cache.shapes.get(), 0);
    }

    #[test]
    fn reference_summary_height_measures_every_wrapped_line() {
        // No labels reserve nothing.
        assert_eq!(reference_summary_height(&[], 700.0, 0.0), 0.0);
        let one = reference_summary_height(&["PUBLIC session".into()], 700.0, 0.0);
        assert!(one > crate::design::ATTACHMENT_CARD_GAP);
        // A list long enough to wrap reserves more than two lines, not one.
        let many: Vec<String> = (0..12).map(|i| format!("PUBLIC session {i}")).collect();
        let wrapped = reference_summary_height(&many, 700.0, 0.0);
        assert!(wrapped > one * 1.9, "{wrapped} vs {one}");
        // A narrower lane can only wrap the same labels onto more lines.
        let narrow = reference_summary_height(&many, 400.0, 0.0);
        assert!(narrow >= wrapped, "{narrow} vs {wrapped}");
        // The copy itself is shared with the view.
        assert_eq!(
            crate::reducer::reference_summary_text(&["a".into(), "b".into()]),
            "Referenced session · a, b"
        );
    }
    #[test]
    fn malformed_and_extreme_widths_produce_positive_finite_bounded_geometry() {
        let cache = Cache::default();
        for width in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1e30,
            -0.0,
            0.0,
            1.0,
            35.0,
            37.0,
            96.0,
            1e30,
            f32::MAX,
        ] {
            let lane = lane_width_checked(width);
            assert!(lane.is_finite() && (MIN_LANE..=MAX_LANE).contains(&lane));
            for role in [
                "You",
                "Assistant",
                "Interrupted assistant attempt",
                "System",
                "Tool result",
            ] {
                let geometry = cache.geometry(
                    1,
                    role,
                    "PUBLIC verylongunbreakabletoken世界\nPUBLIC line",
                    0.0,
                    width,
                );
                bounded(geometry, lane);
                if role == "You"
                    || role.starts_with("Assistant")
                    || role == "Interrupted assistant attempt"
                {
                    // The bubble draws a 22px line and the assistant body a 24px one,
                    // so each role's floor and cap follow its own line box.
                    let line = if role == "You" {
                        crate::design::BUBBLE_LINE
                    } else {
                        crate::design::LINE_BODY
                    };
                    let cap = PREVIEW_LINES * line;
                    assert!(
                        // The narrowest accepted lane's 0.702 share is 32.99, so the
                        // floor is the bubble's own minimum rather than a round 33.
                        // No outer inset is subtracted any more, so the assistant row's
                        // width is the lane itself.
                        geometry.width >= 32.0 && (line..=cap).contains(&geometry.body_height)
                    );
                    assert_eq!(geometry.height, chrome_height(role) + geometry.body_height);
                }
            }
        }
    }

    #[test]
    fn unicode_newlines_empty_and_role_caps_use_real_paragraph_dimensions() {
        let cache = Cache::default();
        let short = cache.geometry(0, "You", "Hi", 0.0, 1000.0);
        let unicode = cache.geometry(
            1,
            "You",
            "世界 · café · Ελληνικά\nPUBLIC second line\nPUBLIC third line",
            0.0,
            1000.0,
        );
        assert!(unicode.body_height > short.body_height);
        let four = cache.geometry(2, "You", "one\ntwo\nthree\nfour", 0.0, 1000.0);
        assert_eq!(four.body_height, PREVIEW_LINES * crate::design::BUBBLE_LINE);
        assert_eq!(
            cache.geometry(3, "You", "", 0.0, 1000.0),
            Geometry {
                width: 96.0,
                body_height: crate::design::BUBBLE_LINE,
                height: crate::design::USER_BUBBLE_PAD.top
                    + crate::design::USER_BUBBLE_PAD.bottom
                    + crate::design::BUBBLE_LINE
            }
        );
        let long = "PUBLIC words ".repeat(80);
        assert!(
            cache.geometry(4, "You", &long, 0.0, 1000.0).width
                <= 1000.0 * crate::design::USER_BUBBLE_SHARE
        );
        // The unboxed assistant column spans the lane exactly.
        assert_eq!(
            cache.geometry(5, "Assistant", "Hi", 0.0, 1000.0).width,
            1000.0
        );
        assert_eq!(
            cache
                .geometry(6, "Interrupted assistant attempt", "Hi", 0.0, 1000.0)
                .width,
            1000.0
        );
    }

    #[test]
    fn a_single_line_bubble_draws_the_bubbles_own_twenty_two_pixel_line() {
        let cache = Cache::default();
        // `MessageItem.module.css:38`: `.bubble` takes `22px + delta`, the assistant
        // body `24px + delta`, so one line of each must measure differently.
        let bubble = cache.geometry(0, "You", "Hi", 0.0, 1000.0);
        let assistant = cache.geometry(1, "Assistant", "Hi", 0.0, 1000.0);
        assert_eq!(bubble.body_height, crate::design::BUBBLE_LINE);
        assert_eq!(assistant.body_height, crate::design::LINE_BODY);
        // The sheet's own contract: "42px single-line bubble at the default size:
        // 22 line + 10 vertical padding each side". The geometry owns the line;
        // `USER_BUBBLE_PAD` owns the padding, so the two together must make 42.
        assert_eq!(
            bubble.body_height
                + crate::design::USER_BUBBLE_PAD.top
                + crate::design::USER_BUBBLE_PAD.bottom,
            42.0
        );
        // A content-size preference moves the bubble's line with the body's delta
        // (`calc(22px + var(--dsh-content-font-delta))`), and it stays narrower.
        // Two lines, because a single line's minimum bounds are the glyph box
        // rather than the line box (the floor above covers that case). The delta
        // is the cache's own content size, not the per-row summary argument.
        cache.set_content_delta(3.0);
        let grown = cache.geometry(2, "You", "one\ntwo", 0.0, 1000.0);
        assert_eq!(grown.body_height, 2.0 * (crate::design::BUBBLE_LINE + 3.0));
        let grown_body = cache.geometry(3, "Assistant", "one\ntwo", 0.0, 1000.0);
        assert_eq!(
            grown_body.body_height,
            2.0 * (crate::design::LINE_BODY + 3.0)
        );
    }

    #[test]
    fn diagnostics_do_not_echo_preview_text() {
        let cache = Cache::default();
        cache.geometry(
            1,
            "You",
            "PUBLIC private preview should not be in Debug",
            0.0,
            500.0,
        );
        assert!(!format!("{cache:?}").contains("PUBLIC private"));
    }

    #[test]
    fn the_content_delta_grows_process_rows_and_invalidates_their_measurements() {
        let cache = Cache::default();
        // At the default size the delta is the identity.
        assert_eq!(cache.tool_line(), TOOL_LINE_HEIGHT);
        let base = cache.geometry(1, "Tool call", "PUBLIC preview", 0.0, 800.0);
        assert_eq!(base.height, TOOL_LINE_HEIGHT + ROW_CHROME);
        // A larger content size buys the line more room. Re-measuring the same key
        // also proves the stored entry was dropped: without invalidation this would
        // return the height measured at the old size.
        cache.set_content_delta(3.0);
        assert_eq!(cache.tool_line(), TOOL_LINE_HEIGHT + 3.0);
        let grown = cache.geometry(1, "Tool call", "PUBLIC preview", 0.0, 800.0);
        assert_eq!(grown.height, TOOL_LINE_HEIGHT + ROW_CHROME + 3.0);
        assert_eq!(grown.body_height, base.body_height + 3.0);
        // Card padding does not scale: the web scales process rows, not the
        // padding around a message, so a human row keeps its chrome.
        let human = cache.geometry(2, "You", "PUBLIC preview", 0.0, 800.0);
        assert_eq!(human.height, chrome_height("You") + human.body_height);
        // A smaller content size shrinks it by the same rule.
        cache.set_content_delta(-3.0);
        assert_eq!(
            cache
                .geometry(1, "Tool call", "PUBLIC preview", 0.0, 800.0)
                .height,
            TOOL_LINE_HEIGHT + ROW_CHROME - 3.0
        );
    }
}
