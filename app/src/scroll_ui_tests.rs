//! Owned PUBLIC model-only scroll tests. No worker subscription/effect executor or real input.
use super::*;
fn reader() -> (App, tokio::sync::mpsc::Receiver<Command>) {
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
    app.selected = Some(serde_json::from_value(serde_json::json!("PUBLIC-reader")).unwrap());
    let records: Vec<_> = (0..20).map(|seq| serde_json::json!({"type":"event","event":{"type":"user/message","seq":seq,"time":1000,"surfaceOp":"append","data":{"content":[{"type":"text","text":format!("PUBLIC row {seq}: {}", "words ".repeat(30))}]}}})).collect();
    let snapshot = serde_json::from_value(serde_json::json!({"type":"snapshot","records":records,"cursor":19,"hasMore":true,"header":{"version":4,"id":"PUBLIC-reader","createdAt":1000,"isSeeded":false},"projections":{"asOfSeq":19,"values":{}},"assistantStream":{"revision":0}})).unwrap();
    app.transcript.apply(0, snapshot).unwrap();
    app.follow_bottom = false;
    app.viewport = 100.0;
    let (_, heights, _) = app.scroll_projection();
    app.offset = heights[..5].iter().sum::<f32>() + 12.0;
    assert!(app.transcript_active());
    (app, queue)
}
fn intra(app: &App, index: usize) -> f32 {
    let (_, heights, _) = app.scroll_projection();
    app.offset - heights[..index].iter().sum::<f32>()
}
#[test]
fn resize_preserves_durable_row_intra_and_changes_operation_identity() {
    let (mut app, mut queue) = reader();
    let old_id = app.scroll_id();
    drop(app.update(Message::Resized(iced::Size::new(760.0, 820.0))));
    assert_eq!(intra(&app, 5), 12.0);
    assert!(!app.follow_bottom);
    assert_ne!(app.scroll_id(), old_id);
    assert!(queue.try_recv().is_err());
}
#[test]
fn stale_generation_revision_and_exhaustion_cannot_apply_feedback() {
    let (mut app, mut queue) = reader();
    let generation = app.generation;
    let revision = app.scroll_revision;
    let old = app.offset;
    app.invalidate_scroll();
    app.observe_scroll(generation, revision, 0.0, 100.0, 1.0, 2000.0);
    assert_eq!(app.offset, old);
    drop(app.update(Message::ScrollApplied {
        generation,
        revision,
        feedback: app.scroll_feedback,
        y: 0.0,
        viewport: 100.0,
    }));
    assert_eq!(app.offset, old);
    app.observe_scroll(generation + 1, app.scroll_revision, 0.0, 100.0, 1.0, 2000.0);
    assert_eq!(app.offset, old);
    app.scroll_revision = Some(u64::MAX);
    app.invalidate_scroll();
    assert_eq!(app.scroll_revision, None);
    app.observe_scroll(generation, None, 0.0, 100.0, 1.0, 2000.0);
    assert_eq!(app.offset, old);
    assert!(queue.try_recv().is_err());
}
#[test]
fn own_clamped_feedback_preserves_reader_intent_but_other_scroll_can_follow() {
    let (mut app, mut queue) = reader();
    let generation = app.generation;
    let revision = app.scroll_revision;
    drop(app.update(Message::ScrollApplied {
        generation,
        revision,
        feedback: app.scroll_feedback,
        y: 123.0,
        viewport: 100.0,
    }));
    app.observe_scroll(generation, revision, 123.0, 100.0, 1.0, 2000.0);
    assert!(!app.follow_bottom);
    app.observe_scroll(generation, revision, 130.0, 100.0, 1.0, 2000.0);
    assert!(app.follow_bottom);
    assert_eq!(app.scroll_target, None);
    assert!(queue.try_recv().is_err());
}
#[test]
fn malformed_feedback_and_hidden_transcript_are_ignored() {
    let (mut app, mut queue) = reader();
    let old = app.offset;
    for (y, h) in [
        (f32::NAN, 100.0),
        (f32::INFINITY, 100.0),
        (-1.0, 100.0),
        (0.0, f32::NAN),
        (0.0, -1.0),
    ] {
        app.observe_scroll(app.generation, app.scroll_revision, y, h, 1.0, 2000.0);
        assert_eq!(app.offset, old);
    }
    app.conversation_menu = true;
    app.observe_scroll(app.generation, app.scroll_revision, 0.0, 100.0, 1.0, 2000.0);
    assert_eq!(app.offset, old);
    assert!(queue.try_recv().is_err());
}
#[test]
fn hidden_remount_gets_fresh_namespace_without_business_effects() {
    let (mut app, mut queue) = reader();
    let old = app.scroll_id();
    let offset = app.offset;
    drop(app.update(Message::ToggleConversationMenu));
    assert!(!app.transcript_active());
    drop(app.update(Message::ToggleConversationMenu));
    assert!(app.transcript_active());
    assert_ne!(app.scroll_id(), old);
    assert_eq!(app.offset, offset);
    assert!(!app.follow_bottom);
    assert!(queue.try_recv().is_err());
}
#[test]
fn clamped_feedback_before_apply_preserves_reader_and_late_apply_cannot_rewind_input() {
    let (mut app, mut queue) = reader();
    let generation = app.generation;
    let revision = app.scroll_revision;
    let feedback = app.scroll_feedback;
    app.scroll_target = Some(100_000.0);
    app.observe_scroll(generation, revision, 50.0, 100.0, 1.0, 150.0);
    assert!(!app.follow_bottom);
    assert_eq!(app.scroll_feedback, feedback);
    drop(app.update(Message::ScrollApplied {
        generation,
        revision,
        feedback,
        y: 50.0,
        viewport: 100.0,
    }));
    assert!(!app.follow_bottom);
    let old_id = app.scroll_id();
    app.observe_scroll(generation, revision, 40.0, 100.0, 0.8, 150.0);
    app.observe_scroll(generation, revision, 30.0, 100.0, 0.6, 150.0);
    assert_eq!(app.offset, 30.0);
    assert_eq!(app.scroll_revision, revision);
    assert_ne!(app.scroll_id(), old_id);
    drop(app.update(Message::ScrollApplied {
        generation,
        revision,
        feedback,
        y: 50.0,
        viewport: 100.0,
    }));
    assert_eq!(app.offset, 30.0);
    assert!(!app.follow_bottom);
    assert!(queue.try_recv().is_err());
}
#[test]
fn exhausted_feedback_serial_does_not_reuse_operation_namespace_or_apply_callback() {
    let (mut app, mut queue) = reader();
    let generation = app.generation;
    let revision = app.scroll_revision;
    app.scroll_feedback = Some(u64::MAX);
    let feedback = app.scroll_feedback;
    app.observe_scroll(generation, revision, 20.0, 100.0, 0.1, 2000.0);
    assert_eq!(app.scroll_feedback, None);
    drop(app.update(Message::ScrollApplied {
        generation,
        revision,
        feedback,
        y: 0.0,
        viewport: 100.0,
    }));
    assert_eq!(app.offset, 20.0);
    assert!(queue.try_recv().is_err());
}
#[test]
fn worker_cancel_remounts_reader_with_fresh_scroll_identity() {
    let (mut app, mut queue) = reader();
    let offset = app.offset;
    let key = crate::interactions::Key {
        epoch: app.transport_epoch,
        event_id: RemoteEventId::new("PUBLIC-scroll-decision").unwrap(),
        serial: 1,
    };
    app.decisions.add_owned(key.clone(),AgentId::new("PUBLIC-agent").unwrap(),WaterfallKind::Approval,serde_json::json!({"toolName":"PUBLIC tool","callId":"PUBLIC-tool-call","reason":"PUBLIC scroll remount regression"})).unwrap();
    drop(
        app.update(Message::Interaction(crate::interactions::Action::Open(
            key.clone(),
        ))),
    );
    assert!(!app.transcript_active());
    let hidden_id = app.scroll_id();
    drop(app.receive(Event::Root {
        frame: RemoteEventFrame::Cancel {
            event_id: key.event_id,
        },
        decision_key: None,
    }));
    assert!(app.transcript_active());
    assert_ne!(app.scroll_id(), hidden_id);
    assert_eq!(app.offset, offset);
    assert_eq!(app.scroll_target, Some(offset));
    assert!(!app.follow_bottom);
    assert!(queue.try_recv().is_err());
}
#[test]
fn viewport_only_redraw_near_end_preserves_reader_intent_and_feedback_serial() {
    let (mut app, mut queue) = reader();
    app.offset = 1400.0;
    app.viewport = 500.0;
    app.scroll_target = None;
    let feedback = app.scroll_feedback;
    app.observe_scroll(
        app.generation,
        app.scroll_revision,
        1400.0,
        580.0,
        1400.0 / 1420.0,
        2000.0,
    );
    assert!(!app.follow_bottom);
    assert_eq!(app.scroll_feedback, feedback);
    assert_eq!(app.viewport, 580.0);
    assert!(queue.try_recv().is_err());
}
#[test]
fn viewport_only_redraw_leaving_end_preserves_following_intent() {
    let (mut app, mut queue) = reader();
    app.follow_bottom = true;
    app.offset = 1500.0;
    app.viewport = 500.0;
    app.scroll_target = None;
    let feedback = app.scroll_feedback;
    app.observe_scroll(
        app.generation,
        app.scroll_revision,
        1500.0,
        460.0,
        1500.0 / 1540.0,
        2000.0,
    );
    assert!(app.follow_bottom);
    assert_eq!(app.scroll_feedback, feedback);
    assert!(queue.try_recv().is_err());
}
#[test]
fn viewport_only_native_clamp_preserves_reader_but_differing_input_supersedes() {
    let (mut app, mut queue) = reader();
    app.offset = 1400.0;
    app.viewport = 500.0;
    app.scroll_target = None;
    let feedback = app.scroll_feedback;
    app.observe_scroll(
        app.generation,
        app.scroll_revision,
        1300.0,
        700.0,
        1.0,
        2000.0,
    );
    assert!(!app.follow_bottom);
    assert_eq!(app.offset, 1300.0);
    assert_eq!(app.scroll_feedback, feedback);
    // Changed height and genuinely different displacement are not geometry-only.
    app.observe_scroll(
        app.generation,
        app.scroll_revision,
        1400.0,
        600.0,
        1.0,
        2000.0,
    );
    assert!(app.follow_bottom);
    assert_ne!(app.scroll_feedback, feedback);
    assert!(queue.try_recv().is_err());
}
