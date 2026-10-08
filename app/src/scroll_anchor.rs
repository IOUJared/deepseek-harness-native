//! Pure, bounded durable-row scroll anchoring; never folds or filters records.
//!
//! Callers supply the actual OLD/NEW displayed durable rows in display order, not
//! numerically sorted sequence order. Partial/live UI rows are excluded by the
//! caller, which must also fence session/generation and preserve follow-bottom
//! intent. A missing key gets only a best-effort neighbor-position fallback.
use std::collections::HashMap;

const MAX_ROWS: usize = 4096;

#[derive(Clone, Debug)]
pub(crate) struct Anchor {
    index: usize,
    intra: f64,
    // Sequence metadata and viewport-relative tops only: no stored message text.
    old: Vec<(u64, f64)>,
}

struct Layout {
    tops: Vec<f64>,
    indices: HashMap<u64, usize>,
    total: f64,
}

fn layout(keys: &[u64], heights: &[f32]) -> Option<Layout> {
    if keys.is_empty() || keys.len() > MAX_ROWS || keys.len() != heights.len() {
        return None;
    }
    let mut tops = Vec::with_capacity(keys.len());
    let mut indices = HashMap::with_capacity(keys.len());
    let mut total = 0.0_f64;
    for (index, (&key, &height)) in keys.iter().zip(heights).enumerate() {
        if !height.is_finite() || height <= 0.0 || indices.insert(key, index).is_some() {
            return None;
        }
        tops.push(total);
        let next = total + f64::from(height);
        // Reject layouts not representable as a finite usable f32 scroll extent.
        if !next.is_finite()
            || next <= total
            || (total > 0.0 && next <= f64::from(height))
            || next > f64::from(f32::MAX)
        {
            return None;
        }
        total = next;
    }
    Some(Layout {
        tops,
        indices,
        total,
    })
}

impl Anchor {
    /// Capture the durable row containing the viewport top, using half-open slots.
    /// An offset at/after the durable end (possibly inside live UI content) has no
    /// durable-row identity and returns None rather than inventing an anchor.
    pub(crate) fn capture(keys: &[u64], heights: &[f32], offset: f32) -> Option<Self> {
        if !offset.is_finite() || offset < 0.0 {
            return None;
        }
        let layout = layout(keys, heights)?;
        let offset = f64::from(offset);
        if offset >= layout.total {
            return None;
        }
        let index = layout
            .tops
            .partition_point(|top| *top <= offset)
            .checked_sub(1)?;
        Some(Self {
            index,
            intra: offset - layout.tops[index],
            old: keys
                .iter()
                .copied()
                .zip(layout.tops.into_iter().map(|top| top - offset))
                .collect(),
        })
    }

    /// Restore the same key/intra-row pixel offset when possible. If it vanished,
    /// preserve the first surviving OLD-display successor's relative top, else
    /// the nearest surviving predecessor's. No surviving old key restores zero.
    /// Final viewport-extent clamping is necessary and can prevent exact alignment.
    pub(crate) fn restore(
        &self,
        keys: &[u64],
        heights: &[f32],
        viewport: f32,
        trailing_height: f32,
    ) -> Option<f32> {
        if !viewport.is_finite()
            || viewport < 0.0
            || !trailing_height.is_finite()
            || trailing_height < 0.0
        {
            return None;
        }
        let layout = layout(keys, heights)?;
        let key = self.old[self.index].0;
        let stable_index = layout.indices.get(&key).copied();
        let target = if let Some(index) = stable_index {
            // Stay strictly inside the row if it shrank; landing at height would
            // otherwise select the following row at the half-open boundary.
            let inside = f64::from(heights[index].next_down());
            layout.tops[index] + self.intra.min(inside)
        } else {
            let survivor = self.old[self.index + 1..]
                .iter()
                .chain(self.old[..self.index].iter().rev())
                .find_map(|&(key, old_relative_top)| {
                    layout
                        .indices
                        .get(&key)
                        .map(|&index| layout.tops[index] - old_relative_top)
                });
            survivor.unwrap_or(0.0)
        };
        let complete = layout.total + f64::from(trailing_height);
        if !complete.is_finite()
            || complete > f64::from(f32::MAX)
            || (trailing_height > 0.0
                && (complete <= layout.total || complete <= f64::from(trailing_height)))
        {
            return None;
        }
        let max = (complete - f64::from(viewport)).max(0.0);
        let mut target = target.clamp(0.0, max) as f32;
        if f64::from(target) > max {
            target = target.next_down().max(0.0);
        }
        // Conversion of prefix+intra can round back up to the row end. Move one
        // representable offset down only when the row, not viewport extent, limits it.
        if let Some(index) = stable_index {
            let end = layout.tops[index] + f64::from(heights[index]);
            if f64::from(target) >= end {
                target = target.next_down().max(0.0);
            }
        }
        target.is_finite().then_some(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    impl Anchor {
        fn restore_durable(&self, keys: &[u64], heights: &[f32], viewport: f32) -> Option<f32> {
            self.restore(keys, heights, viewport, 0.0)
        }
    }

    #[test]
    fn pure_prepend_preserves_key_and_intra_not_total_height_change() {
        let anchor = Anchor::capture(&[4, 5, 6, 7], &[87.0; 4], 100.0).unwrap();
        let target = anchor
            .restore_durable(
                &[0, 1, 2, 4, 5, 6, 7],
                &[40.0, 160.0, 87.0, 87.0, 87.0, 87.0, 87.0],
                70.0,
            )
            .unwrap();
        assert_eq!(target, 387.0); // Key5: new top374 + old intra13.
    }

    #[test]
    fn append_and_resize_recompute_only_position_not_identity() {
        let anchor = Anchor::capture(&[5, 2, 9], &[100.0, 80.0, 200.0], 125.0).unwrap();
        assert_eq!(
            anchor.restore_durable(&[5, 2, 9, 1], &[100.0, 80.0, 200.0, 87.0], 50.0),
            Some(125.0)
        );
        assert_eq!(
            anchor.restore_durable(&[5, 2, 9], &[140.0, 50.0, 300.0], 50.0),
            Some(165.0)
        );
    }

    #[test]
    fn surviving_key_after_surface_reordering_uses_actual_display_prefix() {
        let anchor = Anchor::capture(&[10, 20, 30, 40], &[100.0; 4], 215.0).unwrap();
        // New replacement seq99 occupies the old prefix position, ahead of seq30.
        assert_eq!(
            anchor.restore_durable(&[99, 30, 40], &[70.0, 100.0, 200.0], 50.0),
            Some(85.0)
        );
    }

    #[test]
    fn removed_key_prefers_old_display_successor_not_numeric_sequence_order() {
        let anchor = Anchor::capture(&[90, 2, 70, 3], &[100.0; 4], 120.0).unwrap();
        // Old successor70 had top80 relative to viewport. Its new top160 =>80.
        assert_eq!(
            anchor.restore_durable(&[500, 90, 70, 3], &[60.0, 100.0, 100.0, 100.0], 50.0),
            Some(80.0)
        );
    }

    #[test]
    fn removed_successors_fall_back_to_nearest_surviving_old_predecessor() {
        let anchor = Anchor::capture(&[8, 50, 2, 99], &[100.0; 4], 225.0).unwrap();
        // Predecessor50 old relative top=-125, new top=130 =>255.
        assert_eq!(
            anchor.restore_durable(&[1000, 50, 8, 1001], &[130.0, 100.0, 100.0, 200.0], 50.0),
            Some(255.0)
        );
    }

    #[test]
    fn unrelated_snapshot_has_no_survivor_and_restores_zero() {
        let anchor = Anchor::capture(&[1, 2, 3], &[100.0; 3], 110.0).unwrap();
        assert_eq!(
            anchor.restore_durable(&[7, 8], &[100.0; 2], 50.0),
            Some(0.0)
        );
        assert_eq!(anchor.restore_durable(&[], &[], 50.0), None);
    }

    #[test]
    fn shrink_keeps_intra_inside_row_and_extent_can_force_alignment_clamp() {
        let anchor = Anchor::capture(&[1, 2, 3], &[100.0; 3], 180.0).unwrap();
        let target = anchor
            .restore_durable(&[1, 2, 3], &[100.0, 20.0, 200.0], 50.0)
            .unwrap();
        assert!(target >= 100.0 && target < 120.0);
        assert_eq!(
            anchor.restore_durable(&[1, 2, 3], &[100.0, 20.0, 200.0], 300.0),
            Some(20.0)
        );
        assert_eq!(
            anchor.restore_durable(&[1, 2, 3], &[100.0, 20.0, 200.0], 1000.0),
            Some(0.0)
        );
    }

    #[test]
    fn exact_boundaries_select_following_row_and_durable_end_has_no_anchor() {
        let anchor = Anchor::capture(&[9, 1], &[100.0; 2], 100.0).unwrap();
        assert_eq!(anchor.old[anchor.index].0, 1);
        assert_eq!(anchor.intra, 0.0);
        assert!(Anchor::capture(&[9, 1], &[100.0; 2], 200.0).is_none());
        assert!(Anchor::capture(&[9, 1], &[100.0; 2], 201.0).is_none());
    }

    #[test]
    fn malformed_offsets_viewports_keys_heights_and_lengths_fail_closed() {
        for offset in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
            assert!(Anchor::capture(&[1], &[100.0], offset).is_none());
        }
        let anchor = Anchor::capture(&[1], &[100.0], 0.0).unwrap();
        for viewport in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
            assert_eq!(anchor.restore_durable(&[1], &[100.0], viewport), None);
        }
        for height in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, 0.0] {
            assert!(Anchor::capture(&[1], &[height], 0.0).is_none());
            assert_eq!(anchor.restore_durable(&[1], &[height], 0.0), None);
        }
        assert!(Anchor::capture(&[1, 1], &[100.0; 2], 0.0).is_none());
        assert_eq!(anchor.restore_durable(&[1, 1], &[100.0; 2], 0.0), None);
        assert!(Anchor::capture(&[1], &[], 0.0).is_none());
        assert_eq!(anchor.restore_durable(&[1], &[], 0.0), None);
        assert!(Anchor::capture(&[], &[], 0.0).is_none());
        assert!(Anchor::capture(&[1, 2], &[f32::MAX; 2], 0.0).is_none());
    }

    #[test]
    fn trailing_live_extent_only_changes_legal_clamp_not_durable_identity() {
        let anchor = Anchor::capture(&[1, 2], &[100.0; 2], 150.0).unwrap();
        assert_eq!(
            anchor.restore(&[1, 2], &[100.0; 2], 100.0, 0.0),
            Some(100.0)
        );
        assert_eq!(
            anchor.restore(&[1, 2], &[100.0; 2], 100.0, 100.0),
            Some(150.0)
        );
        // A new trailing partial cannot map a disappeared durable key.
        assert_eq!(
            anchor.restore(&[8, 9], &[100.0; 2], 100.0, 100.0),
            Some(0.0)
        );
        for trailing in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, f32::MAX] {
            assert_eq!(anchor.restore(&[1, 2], &[100.0; 2], 100.0, trailing), None);
        }
    }

    #[test]
    fn arbitrary_keys_include_max_without_sentinel_semantics() {
        let anchor = Anchor::capture(&[u64::MAX, 0, 3], &[100.0; 3], 20.0).unwrap();
        assert_eq!(
            anchor.restore_durable(&[0, u64::MAX, 3], &[100.0; 3], 50.0),
            Some(120.0)
        );
    }

    #[test]
    fn deterministic_4096_rows_are_bounded_and_preserve_measured_position() {
        let keys: Vec<_> = (0..4096).map(|i| (i * 73 % 4096) as u64).collect();
        let heights: Vec<_> = (0..4096)
            .map(|i| [40.0, 87.0, 145.0, 160.0][i % 4])
            .collect();
        let anchor = Anchor::capture(&keys, &heights, 123456.75).unwrap();
        assert_eq!(anchor.old.len(), MAX_ROWS);
        assert_eq!(
            anchor.restore_durable(&keys, &heights, 600.0),
            Some(123456.75)
        );
        let taller: Vec<_> = heights.iter().map(|height| height + 10.0).collect();
        let expected = 123456.75 + anchor.index as f32 * 10.0;
        assert_eq!(
            anchor.restore_durable(&keys, &taller, 600.0),
            Some(expected)
        );
        let oversized: Vec<_> = (0..4097).collect();
        assert!(Anchor::capture(&oversized, &vec![40.0; 4097], 0.0).is_none());
        assert_eq!(
            anchor.restore_durable(&oversized, &vec![40.0; 4097], 0.0),
            None
        );
    }
}
