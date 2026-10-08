//! PUBLIC projection/anchor tests; no runtime, subscription executor or model.
use super::*;
use serde_json::{Value, json};
fn event(seq: u64, kind: &str, data: Value) -> Value {
    json!({"type":"event","event":{"type":kind,"seq":seq,"time":0,"data":data}})
}
fn user(seq: u64) -> Value {
    let mut v = event(
        seq,
        "user/message",
        json!({"content":[{"type":"text","text":"PUBLIC"}]}),
    );
    v["event"]["surfaceOp"] = json!("append");
    v
}
fn splice(seq: u64, outcome: Option<&str>) -> Value {
    let mut data = json!({"target":"next-turn","start":0,"removedCount":1,"inserted":[]});
    if let Some(v) = outcome {
        data["outcome"] = json!(v);
    }
    event(seq, "agent/inbox/spliced", data)
}
fn snapshot(records: Vec<Value>) -> SessionFollowFrame {
    let cursor = records
        .last()
        .map_or(-1, |v| v["event"]["seq"].as_i64().unwrap());
    serde_json::from_value(json!({"type":"snapshot","records":records,"cursor":cursor,"hasMore":false,"header":{"version":4,"id":"PUBLIC-projection","createdAt":0,"isSeeded":false},"projections":{"asOfSeq":cursor,"values":{}},"assistantStream":{"revision":0}})).unwrap()
}
fn app(records: Vec<Value>) -> (App, tokio::sync::mpsc::Receiver<Command>) {
    let (handle, feed, queue, _, _) = worker::test_channels();
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
    app.ready = true;
    app.root_ready = true;
    app.follow_ready = true;
    app.selected = Some(serde_json::from_value(json!("PUBLIC-projection")).unwrap());
    app.transcript.apply(0, snapshot(records)).unwrap();
    app.follow_bottom = false;
    (app, queue)
}
#[test]
fn default_view_geometry_and_anchor_exclude_only_routine_diagnostics() {
    let (mut app, mut queue) = app(vec![
        event(0, "permission/preset", json!({})),
        splice(1, None),
        user(2),
        splice(3, Some("canceled")),
        user(4),
    ]);
    let (keys, heights, _) = app.scroll_projection();
    assert_eq!(keys, vec![2, 3, 4]);
    assert_eq!(app.transcript.rows().len(), 5);
    assert!(app.transcript.inbox_canceled(3));
    assert!(!app.transcript.inbox_canceled(1));
    assert!(app.transcript.safe_to_send());
    app.offset = heights[0] + heights[1] + 12.0;
    let old_revision = app.scroll_revision;
    let old_id = app.scroll_id();
    let anchor = app.reader_anchor().unwrap();
    drop(app.update(Message::ToggleRecords));
    assert!(app.records_expanded);
    assert_ne!(app.scroll_id(), old_id);
    assert!(!app.follow_bottom);
    assert_eq!(app.scroll_projection().0, vec![0, 1, 2, 3, 4]);
    assert_eq!(
        app.offset,
        anchor
            .restore(
                &app.scroll_projection().0,
                &app.scroll_projection().1,
                0.0,
                0.0
            )
            .unwrap()
    );
    let expanded = app.offset;
    app.observe_scroll(app.generation, old_revision, 0.0, 20.0, 1.0, 4000.0);
    assert_eq!(app.offset, expanded);
    drop(app.update(Message::ScrollApplied {
        generation: app.generation,
        revision: old_revision,
        feedback: app.scroll_feedback,
        y: 0.0,
        viewport: 20.0,
    }));
    assert_eq!(app.offset, expanded);
    drop(app.update(Message::ToggleRecords));
    assert!(!app.records_expanded);
    assert_eq!(app.offset, heights[0] + heights[1] + 12.0);
    assert!(queue.try_recv().is_err());
}
#[test]
fn only_metadata_welcome_preserves_record_count_and_disclosure_roundtrip() {
    let (mut app, mut queue) = app(vec![event(0, "sandbox/mode", json!({})), splice(1, None)]);
    assert!(!app.transcript_active());
    assert!(app.scroll_projection().0.is_empty());
    assert_eq!(app.transcript.rows().len(), 2);
    let _ = app.transcript_view();
    for following in [false, true] {
        app.follow_bottom = following;
        let old = app.scroll_id();
        drop(app.update(Message::ToggleRecords));
        assert!(app.transcript_active());
        assert_ne!(old, app.scroll_id());
        assert_eq!(app.scroll_projection().0, vec![0, 1]);
        assert_eq!(app.follow_bottom, following);
        drop(app.update(Message::ToggleRecords));
        assert!(!app.transcript_active());
        assert!(app.scroll_projection().0.is_empty());
        assert_eq!(app.follow_bottom, following);
    }
    assert!(queue.try_recv().is_err());
    assert!(app.transcript.safe_to_send());
}
#[test]
fn future_required_record_stays_visible_and_keeps_send_disabled() {
    let (mut app, mut queue) = app(vec![user(0)]);
    let result = app.transcript.apply(
        0,
        snapshot(vec![
            splice(0, None),
            event(1, "future/required", json!({"warning":"PUBLIC"})),
        ]),
    );
    assert_eq!(result, Err(reducer::ReduceError::UnknownRequiredEvent));
    assert!(!app.transcript.safe_to_send());
    assert!(app.scroll_projection().0.contains(&1));
    assert!(app.transcript_active());
    drop(app.update(Message::ToggleRecords));
    assert!(!app.transcript.safe_to_send());
    assert!(queue.try_recv().is_err());
}
#[test]
fn live_assistant_remains_active_without_creating_a_durable_anchor() {
    let (mut app, mut queue) = app(vec![splice(0, None)]);
    for frame in [
        json!({"type":"assistant-stream","frame":{"type":"start","revision":1,"attemptId":"PUBLIC-live","startedAfterSeq":0,"turn":1,"step":1}}),
        json!({"type":"assistant-stream","frame":{"type":"chunk","revision":2,"attemptId":"PUBLIC-live","index":0,"time":1000,"chunk":{"type":"text-delta","index":0,"text":"PUBLIC live"}}}),
    ] {
        app.transcript
            .apply(0, serde_json::from_value(frame).unwrap())
            .unwrap();
    }
    assert!(app.transcript_active());
    let (keys, heights, trailing) = app.scroll_projection();
    assert!(keys.is_empty() && heights.is_empty());
    assert!(trailing > 0.0);
    assert!(app.reader_anchor().is_none());
    let _ = app.transcript_view();
    assert!(queue.try_recv().is_err());
}
#[test]
fn projected_mixed_4096_rows_preserve_spacers_and_bounded_widgets_in_both_modes() {
    let records=(0..4096).map(|seq| match seq%4 {0=>event(seq,"sandbox/mode",json!({})),1=>splice(seq,None),2=>user(seq),_=>event(seq,"tool/call",json!({"callId":format!("PUBLIC-{seq}"),"turn":1,"step":1,"name":"read","arguments":"{}"}))}).collect();
    let (mut app, mut queue) = app(records);
    for expanded in [false, true] {
        app.records_expanded = expanded;
        let (keys, heights, _) = app.scroll_projection();
        assert_eq!(keys.len(), if expanded { 4096 } else { 2048 });
        let total: f32 = heights.iter().sum();
        for offset in [0.0, 50_000.0, 500_000.0] {
            let range = height_range(&heights, offset, 600.0);
            assert!(range.len() <= 25);
            let sum: f32 = heights[..range.start]
                .iter()
                .chain(&heights[range.clone()])
                .chain(&heights[range.end..])
                .sum();
            assert_eq!(total, sum);
        }
    }
    assert_eq!(app.transcript.rows().len(), 4096);
    assert!(queue.try_recv().is_err());
}
#[test]
fn canceled_future_and_malformed_inbox_records_are_visible_in_both_modes() {
    let mut malformed = splice(2, None);
    malformed["event"]["data"]["future"] = json!("PUBLIC");
    let (app, mut queue) = app(vec![
        splice(0, None),
        splice(1, Some("canceled")),
        malformed,
        splice(3, Some("future")),
    ]);
    assert_eq!(app.scroll_projection().0, vec![1, 2, 3]);
    assert_eq!(app.transcript.display_rows(true).count(), 4);
    assert!(app.transcript.inbox_canceled(1));
    assert!(!app.transcript.inbox_canceled(3));
    assert!(app.transcript.safe_to_send());
    assert!(queue.try_recv().is_err());
}
