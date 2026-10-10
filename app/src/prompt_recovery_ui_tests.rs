//! Actual App update/receive/queue routing; PUBLIC data only, no runtime/model/file effects.
use super::attachment_ui_tests::{ready_app, upload};
use super::*;
use dsh_native_transport::prompt_admission::{
    AdmissionEvidence, AdmissionPhase, AdmissionRejection, PromptAdmission,
};
fn send_unknown(
    app: &mut App,
    rx: &mut tokio::sync::mpsc::Receiver<Command>,
) -> prompt_recovery::Ticket {
    app.editor = text_editor::Content::with_text("PUBLIC original text");
    drop(app.update(Message::Send));
    let Command::Prompt {
        generation,
        request,
    } = rx.try_recv().unwrap()
    else {
        panic!("expected text-only prompt")
    };
    let ticket = app.pending.ticket().unwrap();
    drop(app.receive(Event::Prompt {
        generation,
        request_id: request.request_id,
        result: Err("PUBLIC lost ACK".into()),
    }));
    ticket
}
fn checked() -> PromptAdmission {
    PromptAdmission::Accepted {
        evidence: AdmissionEvidence::Journal,
        phase: AdmissionPhase::Recorded,
        message_id: None,
        seq: Some(0),
    }
}
#[test]
fn lost_text_ack_preserves_original_and_new_draft_and_check_never_replays() {
    let (mut app, mut rx) = ready_app();
    let ticket = send_unknown(&mut app, &mut rx);
    app.editor = text_editor::Content::with_text("PUBLIC another draft");
    assert!(!app.send_allowed());
    assert!(!app.settings_enabled());
    drop(app.update(Message::KeyboardSend));
    assert!(rx.try_recv().is_err());
    assert_eq!(
        app.pending.get(&ticket.generation).unwrap().text,
        "PUBLIC original text"
    );
    drop(app.update(Message::CheckSubmission(ticket.clone())));
    let Command::CheckSubmission(read) = rx.try_recv().unwrap() else {
        panic!("read identity only")
    };
    assert_eq!(read.submission, ticket);
    drop(app.receive(Event::SubmissionChecked {
        ticket: read,
        value: Some(checked()),
    }));
    assert_eq!(app.editor.text(), "PUBLIC another draft");
    assert!(app.pending.is_empty());
    assert!(rx.try_recv().is_err());
    assert!(app.status.contains("not provider completion"));
}
#[test]
fn unverified_and_late_read_results_never_release_or_replace_owned_request() {
    let (mut app, mut rx) = ready_app();
    let ticket = send_unknown(&mut app, &mut rx);
    drop(app.update(Message::CheckSubmission(ticket.clone())));
    let Command::CheckSubmission(read) = rx.try_recv().unwrap() else {
        panic!()
    };
    let mut old = read.clone();
    old.serial -= 1;
    drop(app.receive(Event::SubmissionChecked {
        ticket: old,
        value: Some(checked()),
    }));
    assert!(!app.pending.is_empty());
    drop(app.receive(Event::SubmissionChecked {
        ticket: read,
        value: None,
    }));
    assert!(!app.pending.is_empty());
    assert_eq!(app.pending.ticket(), Some(ticket));
    assert!(rx.try_recv().is_err());
}
#[test]
fn explicit_predispatch_rejection_releases_original_but_never_sends_it_again() {
    let (mut app, mut rx) = ready_app();
    let ticket = send_unknown(&mut app, &mut rx);
    drop(app.update(Message::CheckSubmission(ticket)));
    let Command::CheckSubmission(read) = rx.try_recv().unwrap() else {
        panic!()
    };
    drop(app.receive(Event::SubmissionChecked {
        ticket: read,
        value: Some(PromptAdmission::Rejected(AdmissionRejection::AgentBusy)),
    }));
    assert!(app.pending.is_empty());
    assert_eq!(app.editor.text(), "PUBLIC original text");
    assert!(rx.try_recv().is_err());
    assert!(app.send_allowed());
}
#[test]
fn file_unknown_keeps_exact_owner_and_positive_read_clears_only_original_file_not_new_text() {
    let (mut app, mut rx) = ready_app();
    let file = upload(&mut app, &mut rx);
    app.editor = text_editor::Content::with_text("PUBLIC file original");
    drop(app.update(Message::Send));
    let Command::PromptFile { ticket, request } = rx.try_recv().unwrap() else {
        panic!()
    };
    assert_eq!(ticket, file);
    let owner = app.pending.ticket().unwrap();
    drop(app.receive(Event::FilePrompt {
        ticket: ticket.clone(),
        request_id: request.request_id,
        outcome: worker::attachments::PromptOutcome::Unknown,
    }));
    assert!(app.files.prompt_pending());
    assert!(!app.send_allowed());
    app.editor = text_editor::Content::with_text("PUBLIC new text");
    drop(app.update(Message::CheckSubmission(owner)));
    let Command::CheckSubmission(read) = rx.try_recv().unwrap() else {
        panic!()
    };
    drop(app.receive(Event::SubmissionChecked {
        ticket: read,
        value: Some(checked()),
    }));
    assert!(!app.files.prompt_pending());
    assert!(app.files.ticket().is_none());
    assert!(app.pending.is_empty());
    assert_eq!(app.editor.text(), "PUBLIC new text");
    assert!(rx.try_recv().is_err());
}
#[test]
fn same_owner_observation_rejoin_does_not_follow_resume_or_send_and_keeps_old_intent() {
    let (mut app, mut rx) = ready_app();
    let original = send_unknown(&mut app, &mut rx);
    drop(app.receive(Event::ObservationsLost));
    assert!(!app.root_ready);
    assert!(!app.follow_ready);
    assert!(app.ready);
    drop(app.update(Message::RejoinObservations));
    assert!(matches!(
        rx.try_recv(),
        Ok(Command::RejoinObservations { epoch: 1 })
    ));
    drop(app.receive(Event::ObservationsRejoined {
        epoch: 1,
        result: Ok(()),
    }));
    drop(app.receive(Event::Roster(Ok(SessionListValue { items: vec![] }))));
    assert_eq!(app.pending.ticket(), Some(original));
    assert!(!app.follow_ready);
    assert!(rx.try_recv().is_err());
    drop(app.receive(Event::Fault("PUBLIC owner lost".into())));
    drop(app.update(Message::CheckSubmission(app.pending.ticket().unwrap())));
    assert!(rx.try_recv().is_err());
    assert!(!app.pending.is_empty());
}
#[test]
fn idle_inspection_never_resolves_owned_unknown_or_closes_without_explicit_confirmation() {
    let (mut app, mut rx) = ready_app();
    let original = send_unknown(&mut app, &mut rx);
    app.close = Close::Inspecting;
    drop(app.receive(Event::Inspection(Ok(Inspection {
        active_tasks: 0,
        scheduled_tasks: 0,
        unknown: false,
    }))));
    assert!(matches!(app.close, Close::Confirm(_)));
    assert_eq!(app.pending.ticket(), Some(original));
    assert!(rx.try_recv().is_err());
}
#[test]
fn original_stop_uses_frozen_scope_and_late_ack_retires_stop_without_clearing_new_draft() {
    let (mut app, mut rx) = ready_app();
    let original = send_unknown(&mut app, &mut rx);
    app.generation += 1;
    app.selected = None;
    app.editor = text_editor::Content::with_text("PUBLIC new draft");
    drop(app.update(Message::StopSubmission(original.clone())));
    assert!(
        matches!(rx.try_recv(),Ok(Command::Cancel{generation,session}) if generation==original.generation && session==original.session)
    );
    drop(app.update(Message::CheckSubmission(original)));
    let Command::CheckSubmission(read) = rx.try_recv().unwrap() else {
        panic!()
    };
    let old_generation = read.submission.generation;
    drop(app.receive(Event::SubmissionChecked {
        ticket: read,
        value: Some(checked()),
    }));
    drop(app.receive(Event::Cancel {
        generation: old_generation,
        result: Ok(Accepted { accepted: true }),
    }));
    assert!(!app.cancel_pending);
    assert_eq!(app.editor.text(), "PUBLIC new draft");
    assert!(rx.try_recv().is_err());
}
#[test]
fn identical_retyped_new_draft_is_not_original_submission_ownership() {
    let (mut app, mut rx) = ready_app();
    let original = send_unknown(&mut app, &mut rx);
    drop(app.update(Message::Editor(text_editor::Action::Edit(
        text_editor::Edit::Insert('!'),
    ))));
    app.editor = text_editor::Content::with_text("PUBLIC original text");
    assert_ne!(
        app.draft_serial,
        app.pending.get(&original.generation).unwrap().draft_serial
    );
    drop(app.update(Message::CheckSubmission(original)));
    let Command::CheckSubmission(read) = rx.try_recv().unwrap() else {
        panic!()
    };
    drop(app.receive(Event::SubmissionChecked {
        ticket: read,
        value: Some(checked()),
    }));
    assert_eq!(app.editor.text(), "PUBLIC original text");
    assert!(app.pending.is_empty());
    assert!(rx.try_recv().is_err());
}
