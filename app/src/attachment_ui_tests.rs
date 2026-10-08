//! Pure UI/channel proof only: no file reads, Host receipts, Node, dialogs or model requests.
use super::*;
use worker::attachments::{Metadata, Outcome, PromptOutcome, Ticket};
fn ready_app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
    let (handle, feed, receiver, _events, _closing) = worker::test_channels();
    let options = crate::config::parse(
        [
            "--runtime",
            "/explicit-fork/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/isolated-native-home",
            "--user-home",
            "/user-home",
            "--cwd",
            "/tmp",
        ]
        .map(str::to_owned),
    )
    .unwrap()
    .unwrap();
    let (mut app, _) = App::boot(options, handle, feed);
    app.management.baseline(vec![], vec![]);
    app.ready = true;
    app.root_ready = true;
    app.follow_ready = true;
    app.transport_epoch = worker::TRANSPORT_EPOCH;
    app.settings.adopt_epoch(app.transport_epoch);
    app.generation = 7;
    app.transcript = Transcript::new(7);
    app.selected = Some(SessionId::new("PUBLIC_attachment_ui").unwrap());
    assert!(app.file_context().allowed);
    (app, receiver)
}
fn upload(app: &mut App, receiver: &mut tokio::sync::mpsc::Receiver<Command>) -> Ticket {
    let _ = app.update(Message::FileOpen);
    let stamp = app.files.editor().unwrap().0;
    let _ = app.update(Message::FilePath(stamp, "/PUBLIC_pure_no_read.bin".into()));
    let stamp = app.files.editor().unwrap().0;
    let _ = app.update(Message::FileUpload(stamp));
    let ticket = match receiver.try_recv().unwrap() {
        Command::StageFile(submission) => submission.ticket,
        _ => panic!("wrong command"),
    };
    assert!(!app.send_allowed());
    let _ = app.receive(Event::FileStaged {
        ticket: ticket.clone(),
        outcome: Outcome::Staged(Metadata {
            name: "PUBLIC.bin".into(),
            bytes: 3,
        }),
    });
    ticket
}
fn send(
    app: &mut App,
    receiver: &mut tokio::sync::mpsc::Receiver<Command>,
) -> (Ticket, SessionPromptRequest) {
    assert!(app.send_allowed());
    let _ = app.update(Message::Send);
    match receiver.try_recv().unwrap() {
        Command::PromptFile { ticket, request } => (ticket, request),
        _ => panic!("file intention silently lost"),
    }
}
#[test]
fn file_only_send_uses_ticket_not_receipt_and_blocks_duplicate_send() {
    let (mut app, mut rx) = ready_app();
    let t = upload(&mut app, &mut rx);
    let (owner, request) = send(&mut app, &mut rx);
    assert_eq!(owner, t);
    assert!(request.content.is_empty());
    assert!(app.files.prompt_pending());
    let _ = app.update(Message::Send);
    assert!(rx.try_recv().is_err());
    let _ = app.receive(Event::Prompt {
        generation: t.generation,
        result: Ok(Accepted { accepted: true }),
    });
    assert!(app.files.prompt_pending());
    assert!(app.pending.contains_key(&t.generation));
    let _ = app.receive(Event::FilePrompt {
        ticket: t.clone(),
        request_id: SessionRequestId::new("PUBLIC_wrong_id").unwrap(),
        outcome: PromptOutcome::Accepted,
    });
    assert!(app.files.prompt_pending());
    assert!(app.pending.contains_key(&t.generation));
    let _ = app.receive(Event::FilePrompt {
        ticket: t,
        request_id: request.request_id,
        outcome: PromptOutcome::Accepted,
    });
    assert!(!app.files.prompt_pending());
    assert!(app.pending.is_empty());
    assert!(app.files.ticket().is_none());
}
#[test]
fn unknown_file_send_keeps_text_and_blocks_until_explicit_remove() {
    let (mut app, mut rx) = ready_app();
    upload(&mut app, &mut rx);
    app.editor = text_editor::Content::with_text("PUBLIC unchanged draft");
    let (t, request) = send(&mut app, &mut rx);
    assert!(
        matches!(request.content.as_slice(),[PromptContentPart::Text{text}] if text=="PUBLIC unchanged draft")
    );
    let _ = app.receive(Event::FilePrompt {
        ticket: t.clone(),
        request_id: request.request_id,
        outcome: PromptOutcome::Unknown,
    });
    assert_eq!(app.editor.text(), "PUBLIC unchanged draft");
    assert!(!app.send_allowed());
    assert!(app.files.notice().unwrap().contains("unknown"));
    let _ = app.update(Message::Send);
    assert!(rx.try_recv().is_err());
    let _ = app.update(Message::FileRemove(t.clone()));
    assert!(matches!(rx.try_recv().unwrap(),Command::DiscardFile(owner) if owner==t));
    assert!(app.send_allowed());
    assert!(app.files.ticket().is_none());
}
#[test]
fn local_prompt_queue_failure_restores_ready_without_auto_retry() {
    let (mut app, mut rx) = ready_app();
    upload(&mut app, &mut rx);
    while app.handle.send(Command::Catalog).is_ok() {}
    let _ = app.update(Message::Send);
    assert!(app.files.ready().is_some());
    assert!(!app.files.prompt_pending());
    assert!(app.pending.is_empty());
    while let Ok(command) = rx.try_recv() {
        assert!(matches!(command, Command::Catalog));
    }
    assert!(app.send_allowed());
}
#[test]
fn full_queue_remove_keeps_authority_until_discard_is_delivered() {
    let (mut app, mut rx) = ready_app();
    let t = upload(&mut app, &mut rx);
    while app.handle.send(Command::Catalog).is_ok() {}
    let _ = app.update(Message::FileRemove(t.clone()));
    assert_eq!(app.files.ticket(), Some(&t));
    assert!(app.files.ready().is_some());
    while rx.try_recv().is_ok() {}
    let _ = app.update(Message::FileRemove(t.clone()));
    assert!(matches!(rx.try_recv().unwrap(),Command::DiscardFile(owner) if owner==t));
    assert!(app.files.ticket().is_none());
}
#[test]
fn queued_old_remove_cannot_remove_newer_staged_file() {
    let (mut app, mut rx) = ready_app();
    let old = upload(&mut app, &mut rx);
    let _ = app.update(Message::FileRemove(old.clone()));
    let _ = rx.try_recv().unwrap();
    let new = upload(&mut app, &mut rx);
    assert_ne!(old, new);
    let _ = app.update(Message::FileRemove(old));
    assert!(rx.try_recv().is_err());
    assert_eq!(app.files.ticket(), Some(&new));
}
#[test]
fn hidden_settings_deny_file_actions_but_keep_owned_upload_completion() {
    let (mut app, mut rx) = ready_app();
    let _ = app.update(Message::FileOpen);
    let stamp = app.files.editor().unwrap().0;
    let _ = app.update(Message::FilePath(stamp, "/PUBLIC_no_read.bin".into()));
    let stamp = app.files.editor().unwrap().0;
    let _ = app.update(Message::Settings(crate::settings::Action::Open));
    let _ = app.update(Message::FileUpload(stamp));
    let _ = app.update(Message::FileCancel(stamp));
    while let Ok(command) = rx.try_recv() {
        assert!(
            matches!(command, Command::KeyMetadata(_)),
            "hidden file action unexpectedly emitted"
        );
    }
    assert!(app.files.editor().is_some());
    let _ = app.update(Message::Settings(crate::settings::Action::Close));
    let _ = app.update(Message::FileUpload(stamp));
    let t = match rx.try_recv().unwrap() {
        Command::StageFile(s) => s.ticket,
        _ => panic!("unexpected"),
    };
    let _ = app.update(Message::Settings(crate::settings::Action::Open));
    let _ = app.receive(Event::FileStaged {
        ticket: t,
        outcome: Outcome::Staged(Metadata {
            name: "PUBLIC.bin".into(),
            bytes: 0,
        }),
    });
    assert!(app.files.ready().is_some());
    assert!(!app.send_allowed());
}
#[test]
fn cancel_remains_available_while_model_change_temporarily_disables_upload() {
    let (mut app, mut rx) = ready_app();
    let t = upload(&mut app, &mut rx);
    app.model_pending = true;
    assert!(!app.file_context().allowed);
    assert!(app.file_local_context().allowed);
    let _ = app.update(Message::FileRemove(t));
    assert!(matches!(rx.try_recv().unwrap(), Command::DiscardFile(_)));
    assert!(app.files.ticket().is_none());
}
#[test]
fn before_follow_and_after_disconnect_cannot_upload_or_adopt_unsolicited_metadata() {
    let (mut app, mut rx) = ready_app();
    app.follow_ready = false;
    let _ = app.update(Message::FileOpen);
    assert!(app.files.editor().is_none());
    assert!(rx.try_recv().is_err());
    app.follow_ready = true;
    let t = upload(&mut app, &mut rx);
    let _ = app.receive(Event::NoBackendStarted);
    assert!(app.files.ticket().is_none());
    let _ = app.receive(Event::FileStaged {
        ticket: t,
        outcome: Outcome::Staged(Metadata {
            name: "PUBLIC_unowned.bin".into(),
            bytes: 9,
        }),
    });
    assert!(app.files.ticket().is_none());
    assert!(!app.send_allowed());
}
