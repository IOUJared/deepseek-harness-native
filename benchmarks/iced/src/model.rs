use std::ops::Range;

pub const ROW_HEIGHT: f32 = 72.0;
pub const HEADER_HEIGHT: f32 = 48.0;
pub const COMPOSER_HEIGHT: f32 = 104.0;
pub const SIDEBAR_WIDTH: f32 = 260.0;
pub const OVERSCAN: usize = 3;
pub const INITIAL_ROWS: usize = 1_000;

#[derive(Debug, Clone)]
pub struct TranscriptRow {
    pub role: &'static str,
    pub body: String,
    pub footer: &'static str,
}

#[derive(Debug)]
pub struct FakeSession {
    pub selected: usize,
    pub rows: Vec<TranscriptRow>,
    pub streaming: bool,
    pub generation: u64,
    pub stream_target: usize,
    pub stream_updates: u64,
}

impl FakeSession {
    pub fn new() -> Self {
        Self {
            selected: 0,
            rows: (0..INITIAL_ROWS)
                .map(|i| TranscriptRow {
                    role: if i % 3 == 0 { "You" } else { "Assistant" },
                    body: format!(
                        "Message {i}: Review the workspace and keep changes small and tested."
                    ),
                    footer: if i % 5 == 0 {
                        "Tool result: completed"
                    } else {
                        "Synthetic benchmark transcript"
                    },
                })
                .collect(),
            streaming: false,
            generation: 0,
            stream_target: 0,
            stream_updates: 0,
        }
    }

    pub fn select(&mut self, session: usize) {
        self.stop_synthetic();
        self.selected = session.min(11);
    }

    pub fn send_synthetic(&mut self, body: String) {
        if body.trim().is_empty() {
            return;
        }
        self.stop_synthetic();
        self.rows.push(TranscriptRow {
            role: "You",
            body,
            footer: "Synthetic benchmark transcript",
        });
        self.rows.push(TranscriptRow {
            role: "Assistant",
            body: "Synthetic reply:".into(),
            footer: "Synthetic benchmark transcript",
        });
        self.start_synthetic(self.rows.len() - 1);
    }

    pub fn start_synthetic(&mut self, target: usize) -> u64 {
        self.generation += 1;
        self.streaming = true;
        self.stream_target = target.min(self.rows.len() - 1);
        self.generation
    }

    pub fn stop_synthetic(&mut self) {
        self.streaming = false;
        self.generation += 1;
    }

    pub fn tick(&mut self, generation: u64) -> bool {
        if !self.streaming || generation != self.generation {
            return false;
        }
        self.rows[self.stream_target].body.push_str(" token");
        self.stream_updates += 1;
        true
    }
}

/// Slice includes partially visible rows, with three extra rows on each side.
/// Only this slice is converted into widgets; all data remains in FakeSession.
pub fn visible_range(len: usize, offset: f32, viewport: f32) -> Range<usize> {
    if len == 0 {
        return 0..0;
    }
    let offset = offset.max(0.0).min(max_offset(len, viewport));
    let first = (offset / ROW_HEIGHT).floor() as usize;
    let end = ((offset + viewport.max(0.0)) / ROW_HEIGHT).ceil() as usize;
    first.saturating_sub(OVERSCAN)..end.saturating_add(OVERSCAN).min(len)
}

pub fn last_visible(len: usize, offset: f32, viewport: f32) -> usize {
    (((offset + viewport.max(1.0)) / ROW_HEIGHT).ceil() as usize)
        .saturating_sub(1)
        .min(len.saturating_sub(1))
}

pub fn max_offset(len: usize, viewport: f32) -> f32 {
    (len as f32 * ROW_HEIGHT - viewport.max(0.0)).max(0.0)
}

pub fn bounce(offset: f32, direction: &mut f32, maximum: f32) -> f32 {
    let next = offset + ROW_HEIGHT * *direction;
    if next >= maximum {
        *direction = -1.0;
        maximum
    } else if next <= 0.0 {
        *direction = 1.0;
        0.0
    } else {
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_full_data() {
        let s = FakeSession::new();
        assert_eq!(s.rows.len(), 1000);
        assert_eq!(s.rows[0].role, "You");
        assert_eq!(s.rows[1].role, "Assistant");
        assert_eq!(s.rows[5].footer, "Tool result: completed");
        assert_eq!(
            s.rows[999].body,
            "Message 999: Review the workspace and keep changes small and tested."
        );
    }
    #[test]
    fn virtualized_slice_and_spacers_pin_total_height() {
        for viewport in [1.0, 648.0, 800.0, 1300.0] {
            for offset in [0.0, 71.0, 72.0, 12345.0, 100000.0] {
                let r = visible_range(1000, offset, viewport);
                assert!(r.len() <= (viewport / ROW_HEIGHT).ceil() as usize + 1 + 2 * OVERSCAN);
                let top = r.start as f32 * ROW_HEIGHT;
                let rows = r.len() as f32 * ROW_HEIGHT;
                let bottom = (1000 - r.end) as f32 * ROW_HEIGHT;
                assert_eq!(top + rows + bottom, 72000.0);
            }
        }
    }
    #[test]
    fn partial_rows_and_boundaries() {
        assert_eq!(visible_range(1000, 0.0, 648.0), 0..12);
        assert_eq!(visible_range(1000, 73.0, 648.0), 0..14);
        assert_eq!(visible_range(1000, 71352.0, 648.0), 988..1000);
        assert_eq!(visible_range(0, 0.0, 800.0), 0..0);
        assert_eq!(last_visible(1000, 0.0, 648.0), 8);
    }
    #[test]
    fn synthetic_cancellation_discards_queued_ticks() {
        let mut s = FakeSession::new();
        let old = s.start_synthetic(8);
        assert!(s.tick(old));
        s.stop_synthetic();
        let stopped = s.rows[8].body.clone();
        assert!(!s.tick(old));
        assert_eq!(s.rows[8].body, stopped);
        let new = s.start_synthetic(9);
        assert!(!s.tick(old));
        assert!(s.tick(new));
        s.select(2);
        assert!(!s.tick(new));
    }
    #[test]
    fn sending_appends_fake_messages_and_stop_only_cancels_stream() {
        let mut s = FakeSession::new();
        s.send_synthetic("".into());
        assert_eq!(s.rows.len(), 1000);
        s.send_synthetic("hello".into());
        assert_eq!(s.rows.len(), 1002);
        assert_eq!(s.rows[1000].role, "You");
        let generation = s.generation;
        assert!(s.tick(generation));
        assert_eq!(s.rows[1001].body, "Synthetic reply: token");
        s.stop_synthetic();
        assert_eq!(s.rows.len(), 1002);
        assert!(!s.tick(generation));
    }
    #[test]
    fn scroll_bounces_at_both_bounds() {
        let mut direction = 1.0;
        assert_eq!(bounce(90.0, &mut direction, 100.0), 100.0);
        assert_eq!(direction, -1.0);
        assert_eq!(bounce(20.0, &mut direction, 100.0), 0.0);
        assert_eq!(direction, 1.0);
    }
    #[test]
    fn pinned_geometry() {
        assert_eq!(ROW_HEIGHT, 72.0);
        assert_eq!(SIDEBAR_WIDTH, 260.0);
        assert_eq!(HEADER_HEIGHT + COMPOSER_HEIGHT, 152.0);
        assert_eq!(800.0 - HEADER_HEIGHT - COMPOSER_HEIGHT, 648.0);
    }
}
