use std::ops::Range;

pub const INITIAL_ROWS: usize = 1_000;
pub const ROW_HEIGHT: f32 = 72.0;
pub const OVERSCAN: usize = 2;
pub const TOKEN: &str = " token";
pub const WORKSPACES: [&str; 3] = ["Workspace 1", "Workspace 2", "Workspace 3"];
pub const SESSIONS: [&str; 12] = [
    "Review workspace",
    "Small tested changes",
    "Native UI prototype",
    "Tool results",
    "Session 5",
    "Session 6",
    "Session 7",
    "Session 8",
    "Session 9",
    "Session 10",
    "Session 11",
    "Session 12",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub role: String,
    pub body: String,
    pub footer: String,
}

pub fn initial_messages() -> Vec<Message> {
    (0..INITIAL_ROWS)
        .map(|i| Message {
            role: if i % 3 == 0 { "You" } else { "Assistant" }.into(),
            body: format!("Message {i}: Review the workspace and keep changes small and tested."),
            footer: if i % 5 == 0 {
                "Tool result: completed"
            } else {
                "Synthetic benchmark transcript"
            }
            .into(),
        })
        .collect()
}

pub fn max_offset(rows: usize, viewport: f32) -> f32 {
    (rows as f32 * ROW_HEIGHT - viewport.max(0.0)).max(0.0)
}

pub fn clamp_offset(offset: f32, rows: usize, viewport: f32) -> f32 {
    offset.max(0.0).min(max_offset(rows, viewport))
}

/// Half-open range: intersecting rows plus at most two extra rows on either side.
pub fn visible_range(rows: usize, offset: f32, viewport: f32) -> Range<usize> {
    if rows == 0 || viewport <= 0.0 {
        return 0..0;
    }
    let offset = clamp_offset(offset, rows, viewport);
    let first = (offset / ROW_HEIGHT).floor() as usize;
    let end = ((offset + viewport) / ROW_HEIGHT).ceil() as usize;
    first.saturating_sub(OVERSCAN)..(end + OVERSCAN).min(rows)
}

/// The last intersecting row, not an overscan row; used by the shared stream workload.
pub fn last_visible(rows: usize, offset: f32, viewport: f32) -> Option<usize> {
    if rows == 0 || viewport <= 0.0 {
        return None;
    }
    let end = ((clamp_offset(offset, rows, viewport) + viewport) / ROW_HEIGHT).ceil() as usize;
    Some(end.saturating_sub(1).min(rows - 1))
}

pub fn advance_scroll(offset: f32, forward: &mut bool, maximum: f32) -> f32 {
    if maximum <= 0.0 {
        return 0.0;
    }
    let next = offset + if *forward { ROW_HEIGHT } else { -ROW_HEIGHT };
    if next >= maximum {
        *forward = false;
        maximum
    } else if next <= 0.0 {
        *forward = true;
        0.0
    } else {
        next
    }
}

pub fn append_user(messages: &mut Vec<Message>, text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    messages.push(Message {
        role: "You".into(),
        body: text.into(),
        footer: "Synthetic benchmark transcript".into(),
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_shared_data() {
        let rows = initial_messages();
        assert_eq!(rows.len(), 1_000);
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(row.role, if i % 3 == 0 { "You" } else { "Assistant" });
            assert_eq!(
                row.body,
                format!("Message {i}: Review the workspace and keep changes small and tested.")
            );
            assert_eq!(
                row.footer,
                if i % 5 == 0 {
                    "Tool result: completed"
                } else {
                    "Synthetic benchmark transcript"
                }
            );
        }
        assert_eq!(WORKSPACES.len(), 3);
        assert_eq!(SESSIONS.len(), 12);
    }
    #[test]
    fn row_boundaries_and_overscan() {
        assert_eq!(visible_range(1_000, 0.0, 648.0), 0..11);
        assert_eq!(visible_range(1_000, 72.0, 648.0), 0..12);
        assert_eq!(visible_range(1_000, 720.0, 648.0), 8..21);
        assert_eq!(visible_range(1_000, 721.0, 648.0), 8..22);
        assert_eq!(last_visible(1_000, 720.0, 648.0), Some(18));
        assert_eq!(last_visible(1_000, 721.0, 648.0), Some(19));
        assert_eq!(visible_range(1_000, 100_000.0, 648.0), 989..1_000);
        assert_eq!(visible_range(1_000, -1.0, 648.0), 0..11);
        assert_eq!(visible_range(0, 0.0, 648.0), 0..0);
        assert_eq!(visible_range(1_000, 0.0, 0.0), 0..0);
        assert_eq!(last_visible(0, 0.0, 648.0), None);
        assert_eq!(visible_range(3, 0.0, 1_000.0), 0..3);
    }
    #[test]
    fn virtualization_remains_bounded_and_covers_all_intersections() {
        for offset in (0..72_000).step_by(17) {
            let offset = clamp_offset(offset as f32, 1_000, 647.0);
            let range = visible_range(1_000, offset, 647.0);
            assert!(range.len() <= (647.0 / ROW_HEIGHT).ceil() as usize + 1 + OVERSCAN * 2);
            for i in 0..1_000 {
                if (i as f32 + 1.0) * ROW_HEIGHT > offset
                    && (i as f32) * ROW_HEIGHT < offset + 647.0
                {
                    assert!(range.contains(&i));
                }
            }
        }
    }
    #[test]
    fn send_and_stream_are_synthetic_only() {
        let mut rows = initial_messages();
        assert!(!append_user(&mut rows, "  "));
        assert!(append_user(&mut rows, " test "));
        assert_eq!(rows.len(), 1_001);
        assert_eq!(rows[1_000].body, "test");
        let target = last_visible(rows.len(), 720.0, 648.0).unwrap();
        let original = rows[target].body.clone();
        rows[target].body.push_str(TOKEN);
        assert_eq!(rows[target].body, original + " token");
    }
    #[test]
    fn bounce_at_each_boundary() {
        let mut forward = true;
        assert_eq!(advance_scroll(0.0, &mut forward, 100.0), 72.0);
        assert!(forward);
        assert_eq!(advance_scroll(72.0, &mut forward, 100.0), 100.0);
        assert!(!forward);
        assert_eq!(advance_scroll(100.0, &mut forward, 100.0), 28.0);
        assert_eq!(advance_scroll(28.0, &mut forward, 100.0), 0.0);
        assert!(forward);
        assert_eq!(advance_scroll(0.0, &mut forward, 0.0), 0.0);
    }
}
