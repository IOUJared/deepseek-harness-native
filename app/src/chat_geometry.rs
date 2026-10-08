//! Cached native preview geometry. This measures display text only, never durable records.
//!
//! The caller must supply the same bounded preview it renders, and render its body at
//! `width - 28` by `body_height`, with DejaVu Sans / 15px / Advanced / default
//! line-height / Word wrapping. The 24px header, 7px gap, inner 10px vertical
//! padding and outer 8px vertical padding account for the remaining 67px.
//! Fonts must remain stable after bootstrap; recreate the cache if fonts change.
use iced::advanced::text::{self, Paragraph as _};
use iced::{Font, Pixels, Size, alignment};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::fmt;

const MAX_ENTRIES: usize = 4097;
const MIN_LANE: f32 = 37.0; // Outer 8 + inner 28 + at least 1px of body width.
const MAX_LANE: f32 = 4096.0;
const MALFORMED_LANE: f32 = 104.0; // Outer 8 + ordinary minimum bubble 96.
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

impl Cache {
    pub(crate) fn geometry(
        &self,
        key: u64,
        role: &str,
        preview: &str,
        lane_width: f32,
    ) -> Geometry {
        let lane = lane_width_checked(lane_width);
        let human = role == "You";
        let assistant = role.starts_with("Assistant") || role == "Interrupted assistant attempt";
        // Extra empty lane width above the chat cap cannot change paragraph metrics.
        // Fixed-height tools/opaque rows still need their actual lane for card width.
        let measurement_width = if human || assistant {
            (lane - 8.0).min(if human { 640.0 } else { 800.0 })
        } else {
            lane
        };
        let lane_bits = measurement_width.to_bits();
        {
            let mut entries = self.entries.borrow_mut();
            if let Some(entry) = entries.get_mut(&key)
                && entry.role == role
                && entry.preview == preview
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
                    width: lane - 8.0,
                    body_height: 24.0,
                    height: 40.0,
                }
            } else if !human && !assistant {
                Geometry {
                    width: lane - 8.0,
                    body_height: 75.0,
                    height: 160.0,
                }
            } else {
                let max_width = measurement_width;
                #[cfg(feature = "public-layout-fixture")]
                let shaped = std::time::Instant::now();
                let paragraph = NativeParagraph::with_text(text::Text {
                    content: preview,
                    bounds: Size::new(max_width - 28.0, f32::INFINITY),
                    size: Pixels(15.0),
                    line_height: text::LineHeight::default(),
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
                        measured.width.max(0.0).ceil() + 28.0
                    } else {
                        max_width
                    };
                    measured_width.clamp(96.0_f32.min(max_width), max_width)
                } else {
                    max_width
                };
                let body_height = if measured.height.is_finite() {
                    measured.height.clamp(20.0, 78.0)
                } else {
                    78.0
                };
                Geometry {
                    width,
                    body_height,
                    height: 67.0 + body_height,
                }
            };

        let mut entries = self.entries.borrow_mut();
        if !entries.contains_key(&key) && entries.len() >= MAX_ENTRIES {
            // Durable sequence keys ascend; evict the earliest cached key, never a row.
            entries.pop_first();
        }
        let previous = entries
            .get(&key)
            .filter(|entry| entry.role == role && entry.preview == preview)
            .map(|entry| (entry.lane_bits, entry.geometry));
        entries.insert(
            key,
            Entry {
                role: role.into(),
                preview: preview.into(),
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
    fn two_recent_widths_reuse_actual_native_geometry_without_more_shapes() {
        let cache = Cache::default();
        let text = "PUBLIC native wide words ".repeat(6);
        let wide = cache.geometry(1, "You", &text, 740.0);
        let narrow = cache.geometry(1, "You", &text, 542.0);
        assert_eq!(cache.shapes.get(), 2);
        for _ in 0..16 {
            assert_eq!(cache.geometry(1, "You", &text, 740.0), wide);
            assert_eq!(cache.geometry(1, "You", &text, 542.0), narrow);
        }
        assert_eq!(cache.shapes.get(), 2);
        assert_eq!(cache.entries.borrow().len(), 1);
    }
    #[test]
    fn third_width_evicts_only_the_least_recent_geometry_not_the_row() {
        let cache = Cache::default();
        cache.geometry(1, "You", "PUBLIC", 740.0);
        cache.geometry(1, "You", "PUBLIC", 542.0);
        cache.geometry(1, "You", "PUBLIC", 740.0);
        cache.geometry(1, "You", "PUBLIC", 420.0);
        assert_eq!(cache.shapes.get(), 3);
        cache.geometry(1, "You", "PUBLIC", 740.0);
        assert_eq!(cache.shapes.get(), 3);
        cache.geometry(1, "You", "PUBLIC", 542.0);
        assert_eq!(cache.shapes.get(), 4);
        assert_eq!(cache.entries.borrow().len(), 1);
    }
    #[test]
    fn changed_preview_or_role_discards_both_cached_widths() {
        let cache = Cache::default();
        for lane in [740.0, 542.0] {
            cache.geometry(1, "You", "PUBLIC old", lane);
        }
        for lane in [740.0, 542.0] {
            cache.geometry(1, "You", "PUBLIC replacement", lane);
        }
        assert_eq!(cache.shapes.get(), 4);
        for lane in [740.0, 542.0] {
            cache.geometry(1, "Assistant", "PUBLIC replacement", lane);
        }
        assert_eq!(cache.shapes.get(), 6);
        cache.retain_keys([]);
        assert!(cache.entries.borrow().is_empty());
    }
    #[test]
    fn capped_chat_measurement_width_ignores_only_extra_empty_lane_width() {
        let cache = Cache::default();
        let human = cache.geometry(1, "You", "PUBLIC", 740.0);
        assert_eq!(cache.geometry(1, "You", "PUBLIC", 4096.0), human);
        assert_eq!(cache.shapes.get(), 1);
        let assistant = cache.geometry(2, "Assistant", "PUBLIC", 850.0);
        assert_eq!(cache.geometry(2, "Assistant", "PUBLIC", 4096.0), assistant);
        assert_eq!(cache.shapes.get(), 2);
        let tool = cache.geometry(3, "Tool call", "PUBLIC", 740.0);
        assert_ne!(
            cache.geometry(3, "Tool call", "PUBLIC", 800.0).width,
            tool.width
        );
        assert_eq!(cache.shapes.get(), 2);
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
                    let expected = Cache::default().geometry(1, role, preview, width);
                    assert_eq!(
                        cache.geometry(1, role, preview, width),
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
            cache.geometry(key, "You", "PUBLIC", 740.0);
            cache.geometry(key, "You", "PUBLIC", 542.0);
        }
        let shapes = cache.shapes.get();
        cache.retain_keys((0..4096).chain(std::iter::once(u64::MAX)));
        for key in (0..4096).chain(std::iter::once(u64::MAX)) {
            cache.geometry(key, "You", "PUBLIC", 740.0);
            cache.geometry(key, "You", "PUBLIC", 542.0);
        }
        let entries = cache.entries.borrow();
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert!(entries.values().all(|v| v.previous.is_some()));
        assert_eq!(cache.shapes.get(), shapes);
    }
    #[test]
    fn same_key_hits_reuse_shapes_and_preview_role_width_changes_invalidate() {
        let cache = Cache::default();
        let first = cache.geometry(7, "You", "Hi", 600.0);
        assert_eq!(first.height, 87.0);
        for _ in 0..8 {
            assert_eq!(cache.geometry(7, "You", "Hi", 600.0), first);
        }
        assert_eq!(cache.shapes.get(), 1);
        let longer = cache.geometry(7, "You", "A substantially longer PUBLIC preview", 600.0);
        assert!(longer.width > first.width);
        let assistant = cache.geometry(
            7,
            "Assistant · live",
            "A substantially longer PUBLIC preview",
            600.0,
        );
        assert_eq!(assistant.width, 592.0);
        let narrow = cache.geometry(
            7,
            "Assistant · live",
            "A substantially longer PUBLIC preview",
            140.0,
        );
        assert_eq!(narrow.width, 132.0);
        assert!(narrow.body_height > assistant.body_height);
        assert_eq!(cache.shapes.get(), 4);
        assert_eq!(
            cache.geometry(
                7,
                "Assistant · live",
                "A substantially longer PUBLIC preview",
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
                cache.geometry(0, role, "PUBLIC", 700.0),
                Geometry {
                    width: 692.0,
                    body_height: 24.0,
                    height: 40.0
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
                cache.geometry(0, role, "PUBLIC", 700.0),
                Geometry {
                    width: 692.0,
                    body_height: 75.0,
                    height: 160.0
                }
            );
        }
        assert_eq!(cache.shapes.get(), 0);
    }

    #[test]
    fn retain_and_cap_bound_entries_without_evicting_any_durable_data() {
        let cache = Cache::default();
        for key in 0..5000 {
            cache.geometry(key, "System", "PUBLIC", 500.0);
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
                    width,
                );
                bounded(geometry, lane);
                if role == "You"
                    || role.starts_with("Assistant")
                    || role == "Interrupted assistant attempt"
                {
                    assert!(
                        geometry.width >= 29.0 && (20.0..=78.0).contains(&geometry.body_height)
                    );
                    assert_eq!(geometry.height, 67.0 + geometry.body_height);
                }
            }
        }
    }

    #[test]
    fn unicode_newlines_empty_and_role_caps_use_real_paragraph_dimensions() {
        let cache = Cache::default();
        let short = cache.geometry(0, "You", "Hi", 1000.0);
        let unicode = cache.geometry(
            1,
            "You",
            "世界 · café · Ελληνικά\nPUBLIC second line\nPUBLIC third line",
            1000.0,
        );
        assert!(unicode.body_height > short.body_height);
        let four = cache.geometry(2, "You", "one\ntwo\nthree\nfour", 1000.0);
        assert_eq!(four.body_height, 78.0);
        assert_eq!(
            cache.geometry(3, "You", "", 1000.0),
            Geometry {
                width: 96.0,
                body_height: 20.0,
                height: 87.0
            }
        );
        let long = "PUBLIC words ".repeat(80);
        assert!(cache.geometry(4, "You", &long, 1000.0).width <= 640.0);
        assert_eq!(cache.geometry(5, "Assistant", "Hi", 1000.0).width, 800.0);
        assert_eq!(
            cache
                .geometry(6, "Interrupted assistant attempt", "Hi", 1000.0)
                .width,
            800.0
        );
    }

    #[test]
    fn diagnostics_do_not_echo_preview_text() {
        let cache = Cache::default();
        cache.geometry(
            1,
            "You",
            "PUBLIC private preview should not be in Debug",
            500.0,
        );
        assert!(!format!("{cache:?}").contains("PUBLIC private"));
    }
}
