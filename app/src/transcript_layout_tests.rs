//! Variable-height public-record presentation and page-anchor tests, without a Host.
use super::*;
fn row(key: u64, role: &str) -> DisplayRow {
    DisplayRow {
        key,
        role: role.into(),
        kind: role.into(),
        chat: crate::reducer::presentation::chat_visible(role, role),
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
        assert_eq!(heights[0], 52.0);
        assert_eq!(heights[1], 36.0);
        assert_eq!(heights[2], 52.0);
        assert_eq!(heights[3], 156.0);
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
    app.viewport = 70.0;
    app.follow_bottom = false;
    let before_refs: Vec<_> = app.transcript.display_rows(false).collect();
    let before = app.row_geometries(&before_refs);
    assert_eq!(before_refs[3].key, 7);
    let within = 59.0;
    let before_prefix = before[..3].iter().map(|g| g.height).sum::<f32>();
    let before_total = before.iter().map(|g| g.height).sum::<f32>();
    app.offset = before_prefix + within;
    assert!(within < before[3].height);
    assert!(app.offset + app.viewport < before_total);
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
    let after_refs: Vec<_> = app.transcript.display_rows(app.records_expanded).collect();
    assert!(!after_refs.iter().any(|row| row.key == 2)); // Policy record retained, never an added displayed slot.
    let after = app.row_geometries(&after_refs);
    let anchor = after_refs.iter().position(|r| r.key == 7).unwrap();
    assert!(app.offset + app.viewport < after.iter().map(|g| g.height).sum::<f32>());
    assert_eq!(
        app.offset - after[..anchor].iter().map(|g| g.height).sum::<f32>(),
        within
    );
    let text = app.transcript.rows()[0].text.clone();
    drop(app.update(app.detail_request(0).unwrap()));
    assert_eq!(app.detail.as_ref().unwrap().source, text);
    drop(app.update(app.detail_back().unwrap()));
    drop(app.update(Message::ToggleConversationMenu));
    drop(app.update(app.detail_request(1).unwrap()));
    assert!(app.detail.is_none());
    assert!(queue.try_recv().is_err());
}

#[test]
fn a_tool_row_expands_its_recorded_body_in_place_without_opening_the_detail_panel() {
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
    let records = vec![
        event(
            0,
            "tool/call",
            serde_json::json!({"name":"Bash","arguments":"cd repo && cat Cargo.toml"}),
        ),
        event(
            1,
            "tool/result",
            serde_json::json!({"message":{"content":[{"type":"text","text":"PUBLIC recorded output"}]}}),
        ),
        event(
            2,
            "tool/call",
            serde_json::json!({"name":"Bash","arguments":""}),
        ),
    ];
    let snapshot: SessionFollowFrame = serde_json::from_value(serde_json::json!({
        "type":"snapshot","records":records,"cursor":2,"hasMore":false,
        "header":{"version":4,"id":"s1","createdAt":1000,"isSeeded":false},
        "projections":{"asOfSeq":2,"values":{}},"assistantStream":{"revision":0}
    }))
    .unwrap();
    app.transcript.apply(0, snapshot).unwrap();
    let rows: Vec<DisplayRow> = app.transcript.display_rows(false).cloned().collect();
    assert_eq!(rows.len(), 3);
    let heights = |app: &App| {
        let refs: Vec<&DisplayRow> = rows.iter().collect();
        app.row_geometries(&refs)
            .iter()
            .map(|g| g.height)
            .collect::<Vec<_>>()
    };
    let collapsed = heights(&app);
    // Every tool row starts as a quiet single line with no expanded body.
    assert!(app.activity_expanded.is_empty());
    drop(app.update(Message::ToggleActivity {
        generation: app.generation,
        key: 0,
    }));
    assert!(
        app.detail.is_none(),
        "expanding a tool row must not leave the conversation for the detail panel"
    );
    assert!(app.activity_expanded.contains(&0));
    let expanded = heights(&app);
    assert!(expanded[0] > collapsed[0]);
    // Only the clicked row changes; the rest of the conversation is untouched.
    for index in 1..rows.len() {
        assert_eq!(expanded[index], collapsed[index]);
    }
    assert_eq!(expanded[1], collapsed[1], "a result row stays collapsed");
    // Collapsing restores exactly the compact geometry.
    drop(app.update(Message::ToggleActivity {
        generation: app.generation,
        key: 0,
    }));
    assert_eq!(heights(&app), collapsed);
    // A row with nothing recorded keeps no expansion and no affordance.
    assert!(activity_body_source(&rows[2]).is_empty());
    drop(app.update(Message::ToggleActivity {
        generation: app.generation,
        key: 2,
    }));
    assert!(!app.activity_expanded.contains(&2));
    assert_eq!(heights(&app), collapsed);
    assert!(queue.try_recv().is_err());
    // Switching conversations clears every expansion.
    drop(app.update(Message::ToggleActivity {
        generation: app.generation,
        key: 1,
    }));
    assert!(app.activity_expanded.contains(&1));
    app.choose(SessionId::new("PUBLIC-other-session").unwrap());
    assert!(app.activity_expanded.is_empty());
}

#[test]
fn an_answerless_tool_call_row_keeps_only_its_disclosure_without_an_action_row() {
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
    // The recorded stream from a real session: thinking plus a Bash tool call,
    // and no answer prose.
    let stream = serde_json::json!([
        {"type":"reasoning-chunks","time0":1000,"index":0,"dt":[],"texts":["PUBLIC thought"]},
        {"type":"tool-call-chunks","time0":1001,"index":1,"id":"PUBLIC-call","name":"bash","dt":[],
         "args":["{\"command\":\"date -u\"}"]}
    ]);
    let snapshot: SessionFollowFrame = serde_json::from_value(serde_json::json!({
        "type":"snapshot","cursor":0,"hasMore":false,
        "records":[{"type":"event","event":{"type":"assistant/message","seq":0,"time":1000,
            "surfaceOp":"append","data":{"turn":1,"step":1,"stream":stream}}}],
        "header":{"version":4,"id":"s1","createdAt":1000,"isSeeded":false},
        "projections":{"asOfSeq":0,"values":{}},"assistantStream":{"revision":0}
    }))
    .unwrap();
    app.transcript.apply(0, snapshot).unwrap();
    let reply = app.reply_parts(0).expect("parts");
    assert!(!reply.has_answer(), "a tool call is not an answer");
    let rows: Vec<DisplayRow> = app.transcript.display_rows(false).cloned().collect();
    let refs: Vec<&DisplayRow> = rows.iter().collect();
    let geometry = app.row_geometries(&refs);
    let index = rows.iter().position(|row| row.key == 0).unwrap();
    let measured = geometry[index];
    // No 22px action row is reserved, because the view renders no Copy/Details.
    assert_eq!(
        measured.height,
        chat_geometry::chrome_height(&rows[index].role) + measured.body_height
    );
    assert_eq!(
        measured.body_height,
        reply.body_height(measured.width - 28.0, false)
    );
    // The disclosure alone, without the empty answer bubble it used to leave.
    assert!(measured.body_height < 60.0, "{}", measured.body_height);
    assert!(queue.try_recv().is_err());
}
