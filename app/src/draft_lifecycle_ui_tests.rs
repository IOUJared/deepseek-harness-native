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
    drop(app.update(Message::Worker(Event::Ready("0.2.1-alpha.1".into()))));
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

/// Typing a leading slash must reach out for the advertisement exactly once, and
/// prose must not. This is the live path the fixture bypasses, so it is the one
/// that has to be pinned: a menu that never opens because nothing was ever asked
/// for is invisible to a fixture that pre-loads the list.
#[test]
fn a_leading_slash_asks_once_for_the_advertisement_and_prose_does_not() {
    // The file's own `app` binding shadows the helper, so reach it by path.
    let (mut app, mut queue) = super::conversation_actions_ui_tests::app();

    edit(&mut app, "/");
    assert_eq!(app.editor.text(), "/");
    assert!(
        command_menu::query(&app.editor.text()).is_some(),
        "the draft must still be a command position"
    );
    let asked = queue.try_recv().expect("a leading slash must ask the Host");
    assert!(
        matches!(asked, Command::CommandsList(_)),
        "the ask must be the advertisement read"
    );
    assert!(app.commands_pending, "the ask must be marked in flight");

    // One ask per session: a second character does not dispatch another.
    edit(&mut app, "/plan");
    assert!(
        queue.try_recv().is_err(),
        "a second keystroke must not ask again"
    );

    // Prose is not a command position and must not ask.
    let (mut other, mut other_queue) = super::conversation_actions_ui_tests::app();
    edit(&mut other, "what does / mean");
    assert!(command_menu::query(&other.editor.text()).is_none());
    assert!(
        other_queue.try_recv().is_err(),
        "prose must not ask for commands"
    );
}

/// The whole chain the composer depends on: a slash asks, the Host's own reply
/// opens the menu, navigation moves the highlight, and accepting rewrites the
/// draft without executing anything. A fixture cannot show this, because it
/// pre-loads the list the live app has to earn.
#[test]
fn the_host_reply_opens_the_menu_and_accepting_only_rewrites_the_draft() {
    let (mut app, mut queue) = super::conversation_actions_ui_tests::app();
    let session = app.selected.clone().unwrap();
    edit(&mut app, "/");
    let Command::CommandsList(asked) = queue.try_recv().unwrap() else {
        panic!("a leading slash must ask for the advertisement");
    };
    assert_eq!(asked, session);
    assert!(
        app.completion().is_none(),
        "no advertisement has arrived yet"
    );

    let listed = vec![
        dsh_native_transport::dto::CommandDescriptor {
            definition_id: None,
            name: "plan".into(),
            description: "Explore and design before executing".into(),
        },
        dsh_native_transport::dto::CommandDescriptor {
            definition_id: None,
            name: "compact".into(),
            description: "Condense history".into(),
        },
    ];
    drop(app.update(Message::Worker(Event::CommandsListed {
        session_id: session,
        result: Ok(listed),
    })));
    let menu = app
        .completion()
        .expect("the Host's reply must open the menu");
    assert_eq!(menu.matches.len(), 2, "every advertised command is offered");
    assert_eq!(menu.matches[0].name, "plan", "advertised order is kept");
    assert!(!app.commands_pending, "the ask is settled");

    // Navigation wraps and moves the highlight.
    drop(app.update(Message::CommandMenuMove(1)));
    assert_eq!(app.completion().unwrap().selected, 1);
    drop(app.update(Message::CommandMenuMove(1)));
    assert_eq!(app.completion().unwrap().selected, 0, "selection wraps");

    // Accepting rewrites the draft only: nothing is queued, nothing is sent.
    drop(app.update(Message::CommandMenuAccept));
    assert_eq!(app.editor.text(), "/plan ");
    assert!(queue.try_recv().is_err(), "accepting must not execute");
    assert!(
        app.pending.is_empty(),
        "accepting must not start a submission"
    );
    assert!(
        app.completion().is_none(),
        "the menu closes once the line leaves the command position"
    );

    // A reply for a session the user has left changes nothing.
    let (mut other, mut other_queue) = super::conversation_actions_ui_tests::app();
    edit(&mut other, "/");
    let _ = other_queue.try_recv();
    drop(other.update(Message::Worker(Event::CommandsListed {
        session_id: SessionId::new("PUBLIC-other-session").unwrap(),
        result: Ok(Vec::new()),
    })));
    assert!(
        other.completion().is_none(),
        "a foreign reply must not open the menu"
    );
}
