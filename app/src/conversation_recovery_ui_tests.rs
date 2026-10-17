//! PUBLIC production-App actions; Tasks dropped unless specifically testing disk execution.
#[test]
fn recovered_delivery_is_unknown_and_consent_cannot_survive_new_editor_revision() {
    let (mut app, mut queue) = app();
    app.transport_epoch = worker::TRANSPORT_EPOCH; // Represent a current owned transport; no executor.
    let session = app.selected.clone().unwrap();
    let load = app.draft_state.start_load().unwrap();
    drop(app.update(Message::DraftOutcome(Outcome::Loaded {
        ticket: load,
        result: Ok([(session, "PUBLIC recovered original".into())].into()),
    })));
    assert!(app.recovered_draft_needs_review());
    assert!(!app.send_allowed());
    drop(app.update(Message::KeyboardSend));
    drop(app.update(Message::Send));
    assert!(queue.try_recv().is_err());
    let serial = app.draft_serial;
    drop(app.update(Message::ReviewRecovered {
        generation: app.generation,
        serial: serial + 1,
    }));
    assert!(!app.send_allowed());
    drop(app.update(Message::ReviewRecovered {
        generation: app.generation,
        serial,
    }));
    assert!(app.send_allowed());
    drop(app.update(Message::Editor(text_editor::Action::Edit(
        text_editor::Edit::Insert('x'),
    ))));
    assert!(app.recovered_draft_needs_review());
    assert!(!app.send_allowed());
    drop(app.update(Message::ReviewRecovered {
        generation: app.generation,
        serial: app.draft_serial,
    }));
    drop(app.update(Message::Send));
    assert!(matches!(queue.try_recv(), Ok(Command::Prompt { .. })));
    drop(app.update(Message::Send));
    assert!(queue.try_recv().is_err());
    assert!(
        app.draft_snapshot().is_empty(),
        "owned original is not an unsent snapshot"
    );
    drop(app.update(Message::Editor(text_editor::Action::Edit(
        text_editor::Edit::Insert('y'),
    ))));
    assert!(
        !app.draft_snapshot().is_empty(),
        "genuinely newer editor survives original ownership"
    );
}
#[test]
fn clean_failed_read_exposes_close_controls_instead_of_hanging() {
    let (mut app, _) = app();
    let load = app.draft_state.start_load().unwrap();
    drop(app.update(Message::DraftOutcome(Outcome::Loaded {
        ticket: load,
        result: Err(draft_store::Failure::InvalidData),
    })));
    assert!(!app.draft_state.dirty());
    app.stopped = true;
    app.close = Close::Stopping;
    app.draft_exit_requested = true;
    drop(app.finish_draft_exit());
    assert!(matches!(app.close, Close::Failed(_)));
    assert!(!app.draft_state.busy());
}

#[test]
fn production_joined_disk_operations_restore_saved_body_in_a_new_app() {
    use std::os::unix::fs::DirBuilderExt;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let base = std::env::temp_dir().canonicalize().unwrap();
    let path = base.join(format!(
        "PUBLIC-native-recovery-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .unwrap();
    let store = std::sync::Arc::new(std::sync::Mutex::new(
        draft_store::Store::new(path.clone()).unwrap(),
    ));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (mut first, mut queue) = app();
    first.draft_store = Some(store.clone());
    let load = first.draft_state.start_load().unwrap();
    let outcome = runtime.block_on(draft_recovery_ui::operation(
        load,
        Some(store.clone()),
        None,
    ));
    drop(first.update(Message::DraftOutcome(outcome)));
    first.editor = text_editor::Content::with_text("PUBLIC actual private disk recovery 世界");
    first.touch_draft();
    let save = first.draft_state.next_save().unwrap();
    let snapshot = first.drafts.clone();
    let outcome = runtime.block_on(draft_recovery_ui::operation(
        save,
        Some(store.clone()),
        Some(snapshot),
    ));
    drop(first.update(Message::DraftOutcome(outcome)));
    assert!(!first.draft_state.dirty());
    let (mut second, mut queue2) = app();
    second.draft_store = Some(store.clone());
    let load = second.draft_state.start_load().unwrap();
    let outcome = runtime.block_on(draft_recovery_ui::operation(load, Some(store), None));
    drop(second.update(Message::DraftOutcome(outcome)));
    assert_eq!(
        second.editor.text(),
        "PUBLIC actual private disk recovery 世界"
    );
    assert!(queue.try_recv().is_err());
    assert!(queue2.try_recv().is_err());
    // Verified exact, newly-created PUBLIC test directory; never delete a computed unchecked path.
    assert_eq!(path.canonicalize().unwrap(), path);
    assert_eq!(path.parent(), Some(base.as_path()));
    assert!(
        path.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("PUBLIC-native-recovery-")
    );
    std::fs::remove_dir_all(&path).unwrap();
}

use super::*;
use draft_recovery::Outcome;
fn app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
    conversation_actions_ui_tests::app()
}
fn input(app: &mut App, query: &str) {
    drop(app.update(Message::SearchInput {
        generation: app.generation,
        serial: app.search_serial,
        query: query.into(),
    }));
}
#[test]
fn search_is_local_bounded_and_navigates_loaded_messages_and_supplied_thoughts() {
    let (mut app, mut queue) = app();
    let cursor = app.transcript.cursor();
    assert!(!app.search_open);
    drop(app.update(Message::KeyPressed(
        keyboard::Key::Character("f".into()),
        keyboard::Modifiers::CTRL,
    )));
    assert!(app.search_open);
    input(&mut app, "PUBLIC");
    assert_eq!(app.search_hits(), vec![0, 1]);
    assert_eq!(app.search_cursor, Some(0));
    assert!(!app.follow_bottom);
    drop(app.update(Message::SearchNext {
        generation: app.generation,
        serial: app.search_serial,
        backwards: false,
    }));
    assert_eq!(app.search_cursor, Some(1));
    assert!(app.thoughts_expanded.contains(&1));
    assert!(app.offset > 0.0);
    let _ = app.transcript_view();
    let _ = app.search_bar();
    input(&mut app, "**");
    assert!(app.search_hits().is_empty());
    input(&mut app, &"x".repeat(257));
    assert!(app.search_query.is_empty());
    assert_eq!(app.transcript.cursor(), cursor);
    assert!(queue.try_recv().is_err());
}
#[test]
fn search_stale_session_panel_and_modal_actions_are_fenced() {
    let (mut app, mut queue) = app();
    drop(app.update(Message::SearchOpen));
    input(&mut app, "PUBLIC");
    let generation = app.generation;
    let serial = app.search_serial;
    drop(app.update(Message::DismissPanel));
    assert!(!app.search_open);
    drop(app.update(Message::SearchOpen));
    assert_ne!(app.search_serial, serial);
    drop(app.update(Message::SearchInput {
        generation,
        serial,
        query: "stale".into(),
    }));
    assert_eq!(app.search_query, "PUBLIC");
    drop(app.update(Message::SearchClose { generation, serial }));
    assert!(app.search_open);
    assert!(queue.try_recv().is_err());
    drop(app.update(Message::Settings(crate::settings::Action::Open)));
    // Settings owns its explicit metadata read; isolate search's background fence.
    while queue.try_recv().is_ok() {}
    input(&mut app, "blocked");
    assert!(queue.try_recv().is_err());
    assert_eq!(app.search_query, "PUBLIC");
    drop(app.update(Message::Settings(crate::settings::Action::Close)));
    app.choose(SessionId::new("PUBLIC-other").unwrap());
    assert!(!app.search_open);
    assert!(app.search_query.is_empty());
    drop(app.update(Message::SearchNext {
        generation,
        serial,
        backwards: false,
    }));
    assert_eq!(app.search_cursor, None);
    // Selecting is the existing intentional follow command; search itself emitted none.
    while let Ok(command) = queue.try_recv() {
        assert!(matches!(command, Command::Select { .. }));
    }
}
#[test]
fn recovered_drafts_merge_only_untouched_bodies_and_no_send_is_dispatched() {
    let (mut app, mut queue) = app();
    let session = app.selected.clone().unwrap();
    let ticket = app.draft_state.start_load().unwrap();
    drop(app.update(Message::DraftOutcome(Outcome::Loaded {
        ticket,
        result: Ok([(session.clone(), "PUBLIC recovered unsent text".into())].into()),
    })));
    assert_eq!(app.editor.text(), "PUBLIC recovered unsent text");
    assert_eq!(app.draft_serial, 1);
    assert!(!app.draft_state.dirty());
    drop(app.update(Message::Editor(text_editor::Action::Edit(
        text_editor::Edit::Paste(std::sync::Arc::new("PUBLIC fresh body".into())),
    ))));
    let text = app.editor.text();
    assert!(app.draft_state.touched(&session));
    assert!(app.draft_state.dirty());
    let read = app.draft_state.reload().unwrap();
    drop(app.update(Message::DraftOutcome(Outcome::Loaded {
        ticket: read,
        result: Ok([(session.clone(), "PUBLIC old disk body".into())].into()),
    })));
    assert_eq!(app.editor.text(), text);
    assert_eq!(app.drafts.get(&session), Some(&text));
    assert!(queue.try_recv().is_err());
}
#[test]
fn stale_load_new_edits_during_save_and_explicit_reconcile_never_ack_new_bodies() {
    let (mut app, mut queue) = app();
    let session = app.selected.clone().unwrap();
    let load = app.draft_state.start_load().unwrap();
    app.editor = text_editor::Content::with_text("PUBLIC typed before read");
    app.touch_draft();
    drop(app.update(Message::DraftOutcome(Outcome::Loaded {
        ticket: load,
        result: Ok([(session.clone(), "PUBLIC late recovered old text".into())].into()),
    })));
    assert_eq!(app.editor.text(), "PUBLIC typed before read");
    let saving = app.draft_state.next_save().unwrap();
    app.editor = text_editor::Content::with_text("PUBLIC newer unsent body");
    app.touch_draft();
    drop(app.update(Message::DraftOutcome(Outcome::Saved {
        ticket: saving,
        result: Ok(()),
    })));
    assert!(app.draft_state.dirty());
    assert_eq!(app.editor.text(), "PUBLIC newer unsent body");
    let saving = app.draft_state.next_save().unwrap();
    drop(app.update(Message::DraftOutcome(Outcome::Saved {
        ticket: saving,
        result: Err(draft_store::Failure::Indeterminate),
    })));
    assert!(app.draft_state.next_save().is_none());
    assert!(app.draft_state.failure().is_some());
    drop(app.update(Message::DraftOutcome(Outcome::Loaded {
        ticket: load,
        result: Ok(Default::default()),
    })));
    assert_eq!(app.editor.text(), "PUBLIC newer unsent body");
    assert!(app.draft_state.failure().is_some());
    drop(app.update(Message::DraftReload));
    assert!(app.draft_state.is_loading());
    assert!(queue.try_recv().is_err());
}
#[test]
fn close_waits_for_owned_io_and_stay_cancels_exit_intent_not_io() {
    let (mut app, _) = app();
    let load = app.draft_state.start_load().unwrap();
    app.close = Close::Stopping;
    app.draft_exit_requested = true;
    app.stopped = true;
    drop(app.update(Message::ExitAnyway));
    assert!(!app.draft_discard_on_exit);
    assert_eq!(app.draft_state.pending(), Some(load));
    drop(app.update(Message::Stay));
    assert!(!app.draft_exit_requested);
    assert_eq!(app.draft_state.pending(), Some(load));
    drop(app.update(Message::DraftOutcome(Outcome::Loaded {
        ticket: load,
        result: Ok(Default::default()),
    })));
    assert!(matches!(app.close, Close::None));
    assert!(!app.draft_state.busy());
}
