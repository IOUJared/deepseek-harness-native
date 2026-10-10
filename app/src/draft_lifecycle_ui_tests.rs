//! PUBLIC production-App routing only. Tasks are dropped; no filesystem, Host or durability claim.
use super::*;
use draft_recovery::{Kind, Outcome};

fn app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
    let (mut app, queue) = conversation_actions_ui_tests::app();
    app.transport_epoch = worker::TRANSPORT_EPOCH;
    (app, queue)
}

fn draft_ready(app: &mut App) {
    let ticket = app.draft_state.start_load().unwrap();
    drop(app.update(Message::DraftOutcome(Outcome::Loaded {
        ticket,
        result: Ok(Default::default()),
    })));
}

fn edit(app: &mut App, text: &str) {
    drop(app.update(Message::Editor(text_editor::Action::Edit(
        text_editor::Edit::Paste(std::sync::Arc::new(text.into())),
    ))));
}

fn send(app: &mut App, queue: &mut tokio::sync::mpsc::Receiver<Command>) -> Event {
    drop(app.update(Message::Send));
    let Command::Prompt {
        generation,
        request,
    } = queue.try_recv().unwrap()
    else {
        panic!("expected the single PUBLIC text prompt intent");
    };
    assert!(!app.pending.is_empty());
    Event::Prompt {
        generation,
        request_id: request.request_id,
        result: Ok(Accepted { accepted: true }),
    }
}

#[test]
fn ready_during_startup_close_then_stay_admits_initial_draft_load_once() {
    let (mut app, mut queue) = app();
    app.ready = false;
    drop(app.update(Message::Close));
    assert!(matches!(app.close, Close::Confirm(_)));
    drop(app.update(Message::Worker(Event::Ready(
        "0.2.1-alpha.1".into(),
    ))));
    assert!(app.ready);
    assert!(!app.draft_state.ready());
    assert!(!app.draft_state.busy());
    assert!(queue.try_recv().is_err());

    drop(app.update(Message::Stay));
    assert!(matches!(app.close, Close::None));
    assert!(app.draft_state.is_loading());
    let ticket = app.draft_state.pending().unwrap();
    assert_eq!(ticket.kind, Kind::Load);
    assert!(!app.send_allowed());
    drop(app.update(Message::Stay));
    assert_eq!(app.draft_state.pending(), Some(ticket));
    assert!(queue.try_recv().is_err());
}

#[test]
fn stay_after_failed_read_never_retries_or_clears_recovery_failure() {
    let (mut app, mut queue) = app();
    let ticket = app.draft_state.start_load().unwrap();
    drop(app.update(Message::DraftOutcome(Outcome::Loaded {
        ticket,
        result: Err(draft_store::Failure::InvalidData),
    })));
    let issue = app.draft_state.failure();
    assert!(issue.is_some());
    app.close = Close::Failed("PUBLIC failed recovery".into());
    app.draft_exit_requested = true;
    drop(app.update(Message::Stay));
    assert!(matches!(app.close, Close::None));
    assert!(!app.draft_exit_requested);
    assert!(!app.draft_state.is_loading());
    assert!(!app.draft_state.busy());
    assert_eq!(app.draft_state.failure(), issue);
    assert!(queue.try_recv().is_err());
}

#[test]
fn late_original_ack_retires_its_session_revision_not_another_sessions_editor() {
    let (mut app, mut queue) = app();
    draft_ready(&mut app);
    let original = app.selected.clone().unwrap();
    edit(&mut app, "PUBLIC original admitted body");
    let original_serial = app.draft_serial;
    let ack = send(&mut app, &mut queue);
    assert_eq!(app.draft_versions.get(&original), Some(&original_serial));
    assert!(!app.draft_snapshot().contains_key(&original));

    // Direct choose represents the already-admitted late Created result, not a new allowed Select.
    let other = SessionId::new("PUBLIC-other-session").unwrap();
    app.choose(other.clone());
    assert!(matches!(queue.try_recv(), Ok(Command::Select { .. })));
    edit(&mut app, "PUBLIC newer other-session body");
    let newer_serial = app.draft_serial;
    assert_ne!(newer_serial, original_serial);
    drop(app.update(Message::Worker(ack.clone())));

    assert!(app.pending.is_empty());
    assert!(!app.drafts.contains_key(&original));
    assert!(!app.draft_versions.contains_key(&original));
    assert_eq!(app.editor.text(), "PUBLIC newer other-session body");
    assert_eq!(app.draft_versions.get(&other), Some(&newer_serial));
    assert_eq!(
        app.drafts.get(&other).map(String::as_str),
        Some("PUBLIC newer other-session body")
    );
    assert!(app.draft_state.dirty());
    assert!(!app.draft_snapshot().contains_key(&original));
    drop(app.update(Message::Worker(ack)));
    assert_eq!(app.editor.text(), "PUBLIC newer other-session body");
    assert!(!app.draft_snapshot().contains_key(&original));
    assert!(queue.try_recv().is_err());
}

#[test]
fn late_original_ack_preserves_identical_retyped_body_at_a_new_session_revision() {
    let (mut app, mut queue) = app();
    draft_ready(&mut app);
    let original = app.selected.clone().unwrap();
    let body = "PUBLIC identical but newer owned draft";
    edit(&mut app, body);
    let original_serial = app.draft_serial;
    let ack = send(&mut app, &mut queue);
    drop(app.update(Message::Editor(text_editor::Action::Edit(
        text_editor::Edit::Insert('!'),
    ))));
    // Restore identical bytes after a genuine production Editor revision; text equality is not ownership.
    app.editor = text_editor::Content::with_text(body);
    app.touch_draft();
    let newer_original_serial = app.draft_serial;
    assert_ne!(newer_original_serial, original_serial);
    assert_eq!(
        app.draft_versions.get(&original),
        Some(&newer_original_serial)
    );

    let other = SessionId::new("PUBLIC-other-newer-session").unwrap();
    app.choose(other.clone());
    assert!(matches!(queue.try_recv(), Ok(Command::Select { .. })));
    edit(&mut app, "PUBLIC other draft remains");
    drop(app.update(Message::Worker(ack)));
    assert!(app.pending.is_empty());
    assert_eq!(app.drafts.get(&original).map(String::as_str), Some(body));
    assert_eq!(
        app.draft_versions.get(&original),
        Some(&newer_original_serial)
    );
    assert_eq!(app.editor.text(), "PUBLIC other draft remains");
    let snapshot = app.draft_snapshot();
    assert_eq!(snapshot.get(&original).map(String::as_str), Some(body));
    assert_eq!(
        snapshot.get(&other).map(String::as_str),
        Some("PUBLIC other draft remains")
    );
    assert!(app.draft_state.dirty());
    assert!(queue.try_recv().is_err());
}

#[test]
fn outstanding_new_session_blocks_button_and_keyboard_prompt_intents() {
    let (mut app, mut queue) = app();
    draft_ready(&mut app);
    edit(&mut app, "PUBLIC draft waits for session creation");
    assert!(app.send_allowed());
    drop(app.update(Message::NewSession));
    assert!(app.new_pending);
    assert!(matches!(queue.try_recv(), Ok(Command::Create { .. })));
    assert!(!app.send_allowed());
    drop(app.update(Message::Send));
    drop(app.update(Message::KeyboardSend));
    assert!(app.pending.is_empty());
    assert_eq!(app.editor.text(), "PUBLIC draft waits for session creation");
    assert!(queue.try_recv().is_err());
}
