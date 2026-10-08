//! Variable-height public-record presentation and page-anchor tests, without a Host.
use super::*;
fn row(key: u64, role: &str) -> DisplayRow {
    DisplayRow {
        key,
        role: role.into(),
        text: "PUBLIC".into(),
        time: 0,
        surface: false,
    }
}
#[test]
fn activity_is_compact_and_every_other_raw_role_keeps_message_height() {
    for role in ["Tool call", "Tool result"] {
        assert_eq!(row_height(&row(0, role)), 40.0);
    }
    for role in [
        "You",
        "Assistant",
        "Assistant · live",
        "Interrupted assistant attempt",
        "System",
        "Developer",
        "future/required",
        "sandbox/mode",
    ] {
        assert_eq!(row_height(&row(0, role)), 160.0);
    }
}
#[test]
fn mixed_height_range_spacers_and_cards_preserve_all_raw_geometry() {
    let rows: Vec<_> = (0..4096)
        .map(|i| {
            row(
                i,
                [
                    "You",
                    "Tool call",
                    "Tool result",
                    "Assistant",
                    "approval/policy",
                ][i as usize % 5],
            )
        })
        .collect();
    let refs: Vec<_> = rows.iter().collect();
    for offset in [0.0, 64.0, 160.0, 1000.0, 50_000.0, 500_000.0, 1_000_000.0] {
        let range = transcript_range(&refs, offset, 600.0);
        assert!(range.start <= range.end && range.end <= rows.len());
        assert!(range.len() <= 25);
        let before = transcript_height(&rows[..range.start]);
        let cards = transcript_height(&rows[range.clone()]);
        let after = transcript_height(&rows[range.end..]);
        assert_eq!(before + cards + after, transcript_height(&rows));
        if offset < transcript_height(&rows) {
            assert!(before <= offset);
            assert!(before + cards >= (offset + 600.0).min(transcript_height(&rows)));
        }
    }
    assert_eq!(rows.len(), 4096); // No metadata/result is filtered or renumbered.
}
#[test]
fn compact_range_handles_nonfinite_and_oversized_viewports_without_unbounded_widgets() {
    let rows: Vec<_> = (0..4096).map(|i| row(i, "Tool call")).collect();
    let refs: Vec<_> = rows.iter().collect();
    for offset in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, 1e30] {
        for viewport in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, 1e30] {
            let range = transcript_range(&refs, offset, viewport);
            assert!(range.start <= range.end && range.end <= rows.len() && range.len() <= 124);
        }
    }
}
#[test]
fn measured_chat_heights_drive_ranges_spacers_and_resize_consistently() {
    let cache = chat_geometry::Cache::default();
    let rows: Vec<_> = (0..4096)
        .map(|i| {
            row(
                i,
                ["You", "Tool call", "Assistant", "future/required"][i as usize % 4],
            )
        })
        .collect();
    for lane in [500.0, 800.0] {
        let heights: Vec<_> = rows
            .iter()
            .map(|r| {
                cache
                    .geometry(r.key, &r.role, &excerpt(&r.text), lane)
                    .height
            })
            .collect();
        assert_eq!(heights[0], 87.0);
        assert_eq!(heights[1], 40.0);
        assert_eq!(heights[2], 87.0);
        assert_eq!(heights[3], 160.0);
        let total: f32 = heights.iter().sum();
        for offset in [0.0, 87.0, 1000.0, 50_000.0, 1_000_000.0] {
            let range = height_range(&heights, offset, 600.0);
            assert!(range.start <= range.end && range.end <= heights.len() && range.len() <= 25);
            let before: f32 = heights[..range.start].iter().sum();
            let inside: f32 = heights[range.clone()].iter().sum();
            let after: f32 = heights[range.end..].iter().sum();
            assert!((before + inside + after - total).abs() < 1.0);
            if offset < total {
                assert!(before <= offset && before + inside >= (offset + 600.0).min(total));
            }
        }
    }
}

#[test]
fn older_page_anchor_uses_real_compact_activity_height_not_record_count() {
    let (handle, feed, mut queue, _, _) = worker::test_channels();
    let options = crate::config::parse(
        [
            "--runtime",
            "/explicit-fork/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/PUBLIC-native-home",
            "--user-home",
            "/PUBLIC-user-home",
            "--cwd",
            "/tmp",
        ]
        .map(str::to_owned),
    )
    .unwrap()
    .unwrap();
    let (mut app, task) = App::boot(options, handle, feed);
    drop(task);
    let event = |seq, kind: &str, data| serde_json::json!({"type":"event","event":{"type":kind,"seq":seq,"time":1000,"data":data,"surfaceOp":"append"}});
    let tails: Vec<_> = (4..14)
        .map(|seq| {
            event(
                seq,
                "user/message",
                serde_json::json!({"content":[{"type":"text","text":"PUBLIC tail"}]}),
            )
        })
        .collect();
    let snapshot:SessionFollowFrame=serde_json::from_value(serde_json::json!({"type":"snapshot","records":tails,"cursor":13,"hasMore":true,"header":{"version":4,"id":"s1","createdAt":1000,"isSeeded":false},"projections":{"asOfSeq":13,"values":{}},"assistantStream":{"revision":0}})).unwrap();
    app.transcript.apply(0, snapshot).unwrap();
    app.offset = 320.0;
    app.viewport = 70.0;
    app.follow_bottom = false;
    let before_refs: Vec<_> = app.transcript.rows().iter().collect();
    let before = app.row_geometries(&before_refs);
    assert_eq!(before.iter().map(|g| g.height).sum::<f32>(), 870.0);
    assert!(app.offset + app.viewport < 870.0);
    assert_eq!(before_refs[3].key, 7);
    let within = app.offset - before[..3].iter().map(|g| g.height).sum::<f32>();
    let mut records = vec![
        event(
            0,
            "tool/call",
            serde_json::json!({"callId":"PUBLIC-call","turn":1,"step":1,"name":"read","arguments":"{\"file_path\":\"PUBLIC.rs\"}"}),
        ),
        event(
            1,
            "tool/result",
            serde_json::json!({"turn":1,"step":1,"message":{"role":"tool","toolCallId":"PUBLIC-call","source":{"kind":"tool","callId":"PUBLIC-call"},"content":[{"type":"text","text":"PUBLIC output hidden by default"}],"isError":false}}),
        ),
        event(
            2,
            "sandbox/mode",
            serde_json::json!({"value":"PUBLIC metadata"}),
        ),
        event(
            3,
            "user/message",
            serde_json::json!({"content":[{"type":"text","text":"PUBLIC older"}]}),
        ),
    ];
    records.extend(tails);
    let page =
        serde_json::from_value(serde_json::json!({"records":records,"hasMore":false})).unwrap();
    drop(app.receive(Event::Page {
        generation: 0,
        result: Ok(page),
    }));
    assert_eq!(app.transcript.rows().len(), 14);
    assert_eq!(app.offset, 487.0); //40+40+87 visible added; secondary160px policy row stays retained.
    let after_refs: Vec<_> = app.transcript.display_rows(app.records_expanded).collect();
    let after = app.row_geometries(&after_refs);
    let anchor = after_refs.iter().position(|r| r.key == 7).unwrap();
    assert_eq!(
        app.offset - after[..anchor].iter().map(|g| g.height).sum::<f32>(),
        within
    );
    let text = app.transcript.rows()[0].text.clone();
    drop(app.update(app.detail_request(0).unwrap()));
    assert_eq!(app.detail.as_ref().unwrap().source,text);
    drop(app.update(app.detail_back().unwrap()));
    drop(app.update(Message::ToggleConversationMenu));
    drop(app.update(app.detail_request(1).unwrap()));
    assert!(app.detail.is_none());
    assert!(queue.try_recv().is_err());
}
