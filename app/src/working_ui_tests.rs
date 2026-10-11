//! PUBLIC status/tick regressions; test channels are never executed.
use super::*;
use serde_json::json;
fn app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
    let (handle, feed, queue, _, _) = worker::test_channels();
    let options = crate::config::parse(
        [
            "--runtime",
            "/explicit-fork/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/PUBLIC-home",
            "--user-home",
            "/PUBLIC-user",
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
    app.roster_pending = false;
    app.window_active = true;
    app.management.baseline(vec![], vec![]);
    let summary: SessionSummary = serde_json::from_value(json!({"sessionId":"PUBLIC-running", "agentAvailable":true,"updatedAt":0,"running":false,"blank":true,"cwd":"/PUBLIC/workspace"})).unwrap();
    app.selected = Some(summary.session_id.clone());
    app.sessions.insert(summary.session_id.clone(), summary);
    (app, queue)
}
fn status(app: &mut App, running: bool) {
    app.root(
        serde_json::from_value(
            json!({"type":"emit","event":"api-session/status","args":["PUBLIC-running",running]}),
        )
        .unwrap(),
        None,
    );
}
#[test]
fn only_host_status_starts_and_stops_the_shared_clock() {
    let (mut app, mut queue) = app();
    assert!(!app.working_animation_active());
    app.catalog_read.begin(worker::TRANSPORT_EPOCH).unwrap();
    app.model_pending = true;
    assert!(!app.working_animation_active());
    status(&mut app, true);
    assert!(app.working_animation_active());
    assert_eq!(
        app.working_label(app.sessions.values().next().unwrap()),
        Some("Working")
    );
    drop(app.header());
    drop(app.session_navigation());
    status(&mut app, false);
    assert!(!app.working_animation_active());
    let phase = app.working_phase;
    drop(app.update(Message::WorkingTick));
    assert_eq!(app.working_phase, phase);
    assert!(queue.try_recv().is_err());
}
#[test]
fn ticks_never_touch_chat_geometry_drafts_or_business_authority() {
    let (mut app, mut queue) = app();
    status(&mut app, true);
    app.editor = text_editor::Content::with_text("PUBLIC unsent draft");
    app.offset = 147.0;
    app.follow_bottom = false;
    let before = (
        app.generation,
        app.transcript.cursor(),
        app.scroll_revision,
        app.scroll_feedback,
        app.scroll_target,
        app.follow_bottom,
        app.offset,
        app.sidebar_motion.value(),
    );
    for _ in 0..working_indicator::PHASES {
        drop(app.update(Message::WorkingTick));
    }
    assert_eq!(app.working_phase, 0);
    drop(app.update(Message::WorkingTick));
    assert_eq!(app.working_phase, 1);
    assert_eq!(
        before,
        (
            app.generation,
            app.transcript.cursor(),
            app.scroll_revision,
            app.scroll_feedback,
            app.scroll_target,
            app.follow_bottom,
            app.offset,
            app.sidebar_motion.value()
        )
    );
    assert_eq!(app.editor.text(), "PUBLIC unsent draft");
    assert!(queue.try_recv().is_err());
}
#[test]
fn hidden_unfocused_modal_closing_and_disconnected_ticks_freeze() {
    let (mut app, mut queue) = app();
    status(&mut app, true);
    for gate in 0..6 {
        match gate {
            0 => app.window_active = false,
            1 => app.capabilities = true,
            2 => app.conversation_menu = true,
            3 => app.close = Close::Stopping,
            4 => app.root_ready = false,
            _ => app.roster_pending = true,
        }
        assert!(!app.working_animation_active());
        drop(app.update(Message::WorkingTick));
        assert_eq!(app.working_phase, 0);
        app.window_active = true;
        app.capabilities = false;
        app.conversation_menu = false;
        app.close = Close::None;
        app.root_ready = true;
        app.roster_pending = false;
    }
    app.ready = false; // Open the local modal without requesting settings metadata.
    drop(app.update(Message::Settings(crate::settings::Action::Open)));
    app.ready = true;
    assert!(!app.working_animation_active());
    assert!(queue.try_recv().is_err());
}
#[test]
fn background_work_animates_only_while_its_sidebar_indicator_is_mounted() {
    let (mut app, mut queue) = app();
    status(&mut app, true);
    app.selected = None;
    assert!(app.working_animation_active());
    app.filter = "does-not-match".into();
    assert!(!app.working_animation_active());
    app.filter.clear();
    app.sidebar_motion.snap(design::RAIL);
    assert!(!app.working_animation_active());
    app.sidebar_sheet = true;
    assert!(app.working_animation_active());
    app.sessions.clear();
    assert!(!app.working_animation_active());
    assert!(queue.try_recv().is_err());
}
#[test]
fn first_two_hundred_navigation_cap_gates_background_animation() {
    let (mut app, mut queue) = app();
    status(&mut app, true);
    app.selected = None;
    for index in 0..200 {
        let summary: SessionSummary = serde_json::from_value(json!({
            "sessionId":format!("PUBLIC-idle-{index}"), "agentAvailable":true,
            "updatedAt":1000+index,"running":false,"blank":true,"cwd":"/PUBLIC/workspace"
        }))
        .unwrap();
        app.sessions.insert(summary.session_id.clone(), summary);
    }
    assert!(!app.working_animation_active());
    let id = SessionId::new("PUBLIC-idle-0").unwrap();
    app.sessions.remove(&id);
    assert!(app.working_animation_active());
    assert!(queue.try_recv().is_err());
}
#[test]
fn disconnected_status_is_unknown_not_false_idle_or_active_animation() {
    let (mut app, mut queue) = app();
    status(&mut app, true);
    app.root_ready = false;
    let session = app.sessions.values().next().unwrap();
    assert!(session.running);
    assert_eq!(app.working_label(session), Some("Status unavailable"));
    assert!(!app.working_animation_active());
    drop(app.header());
    drop(app.session_navigation());
    assert!(queue.try_recv().is_err());
}
#[test]
fn cancel_ack_is_not_a_host_idle_status() {
    let (mut app, mut queue) = app();
    status(&mut app, true);
    app.cancel_pending = true;
    drop(app.receive(Event::Cancel {
        generation: app.generation,
        result: Ok(serde_json::from_value(json!({"accepted":true})).unwrap()),
    }));
    assert!(app.sessions.values().next().unwrap().running);
    assert!(app.working_animation_active());
    assert!(app.stop_allowed());
    assert!(queue.try_recv().is_err());
}
