use super::super::{Command, Event, Handle};
use super::*;
use dsh_native_transport::{
    dto::{Accepted, PromptContentPart, PromptMode, SessionRequestId},
    prompt_admission::{AdmissionEvidence, AdmissionPhase},
};
fn request() -> SessionPromptRequest {
    SessionPromptRequest {
        session_id: SessionId::new("PUBLIC session").unwrap(),
        request_id: SessionRequestId::new("PUBLIC request").unwrap(),
        mode: PromptMode::Queue,
        content: vec![PromptContentPart::Text {
            text: "PUBLIC original".into(),
        }],
        client_time_zone: Some("UTC".into()),
    }
}
#[test]
fn actual_outer_capacity_predicate_preserves_generic_and_codex_cancel_priority_only() {
    let session = SessionId::new("PUBLIC session").unwrap();
    assert!(!normal_capacity_rejects(
        &Command::Cancel {
            generation: 3,
            session
        },
        MAX_OPERATIONS
    ));
    assert!(!normal_capacity_rejects(
        &Command::Codex(crate::codex::Effect::CancelLogin {
            ticket: crate::codex::Ticket {
                epoch: 1,
                panel: 1,
                serial: 1
            },
            attempt: 11
        }),
        MAX_OPERATIONS
    ));
    assert!(normal_capacity_rejects(
        &Command::Prompt {
            generation: 3,
            request: request()
        },
        MAX_OPERATIONS
    ));
    assert!(normal_capacity_rejects(&Command::Inspect, MAX_OPERATIONS));
}
#[test]
fn priority_queue_is_independent_of_full_thirty_two_command_business_queue_and_bounded() {
    let (commands, mut normal) = tokio::sync::mpsc::channel(32);
    let (priority, mut stop) = tokio::sync::mpsc::channel(2);
    let (shutdown, _) = tokio::sync::watch::channel(false);
    let handle = Handle {
        commands,
        priority,
        shutdown,
    };
    for _ in 0..32 {
        assert!(handle.send(Command::Inspect).is_ok());
    }
    for _ in 0..2 {
        assert!(
            handle
                .send(Command::Cancel {
                    generation: 3,
                    session: SessionId::new("PUBLIC session").unwrap()
                })
                .is_ok()
        );
    }
    assert!(
        handle
            .send(Command::Cancel {
                generation: 3,
                session: SessionId::new("PUBLIC session").unwrap()
            })
            .is_err()
    );
    assert!(matches!(stop.try_recv(), Ok(Command::Cancel { .. })));
    assert_eq!(normal.len(), 32);
    assert!(matches!(normal.try_recv(), Ok(Command::Inspect)));
}
#[test]
fn unknown_text_file_and_missing_read_hold_worker_mutation_owner_until_exact_verified_check() {
    let mut c = Coordinator::default();
    let body = request();
    let t = ticket(3, &body);
    assert!(c.admit(t.clone(), 3, Some(&body.session_id), true));
    c.settle(&Event::Prompt {
        generation: 3,
        request_id: body.request_id.clone(),
        result: Err("PUBLIC lost ACK".into()),
    });
    assert!(c.busy());
    assert!(!c.admit(t.clone(), 3, Some(&body.session_id), true));
    let read = CheckTicket {
        submission: t.clone(),
        serial: 1,
    };
    assert!(c.check(&read));
    let mut old = read.clone();
    old.serial = 0;
    c.settle(&Event::SubmissionChecked {
        ticket: old,
        value: Some(PromptAdmission::Accepted {
            evidence: AdmissionEvidence::Journal,
            phase: AdmissionPhase::Recorded,
            message_id: None,
            seq: Some(0),
        }),
    });
    assert!(c.busy());
    c.settle(&Event::SubmissionChecked {
        ticket: read.clone(),
        value: None,
    });
    assert!(c.busy());
    assert!(c.check(&read));
    c.settle(&Event::SubmissionChecked {
        ticket: read,
        value: Some(PromptAdmission::Accepted {
            evidence: AdmissionEvidence::Inbox,
            phase: AdmissionPhase::Queued,
            message_id: None,
            seq: None,
        }),
    });
    assert!(!c.busy());
}
#[test]
fn reserved_stop_binds_current_or_original_submission_and_ack_never_changes_host_running_state() {
    let mut c = Coordinator::default();
    let body = request();
    let t = ticket(3, &body);
    assert!(c.admit(t, 3, Some(&body.session_id), true));
    let other = SessionId::new("PUBLIC new session").unwrap();
    assert!(c.stop(3, &body.session_id, 4, Some(&other), true));
    assert!(!c.stop(3, &body.session_id, 4, Some(&other), true));
    c.settle(&Event::Cancel {
        generation: 3,
        result: Ok(Accepted { accepted: true }),
    });
    assert!(c.busy());
    assert!(!c.stop(99, &other, 4, Some(&other), true));
    assert!(c.stop(3, &body.session_id, 4, Some(&other), true));
}
#[test]
fn duplicate_original_with_changed_payload_is_ignored_not_false_not_sent_or_replayed() {
    let mut c = Coordinator::default();
    let mut body = request();
    let owner = ticket(3, &body);
    assert!(c.admit(owner.clone(), 3, Some(&body.session_id), true));
    assert!(c.freeze(&owner, &body));
    let original =
        serde_json::to_value(c.owner.as_ref().unwrap().request.as_ref().unwrap()).unwrap();
    c.settle(&Event::Prompt {
        generation: 3,
        request_id: body.request_id.clone(),
        result: Err("PUBLIC lost ACK".into()),
    });
    body.content = vec![PromptContentPart::Text {
        text: "PUBLIC replacement must not be sent".into(),
    }];
    assert!(c.duplicate(&Command::Prompt {
        generation: 3,
        request: body.clone()
    }));
    assert!(c.busy());
    assert!(!c.freeze(&owner, &body));
    assert_eq!(
        serde_json::to_value(c.owner.as_ref().unwrap().request.as_ref().unwrap()).unwrap(),
        original
    );
    body.request_id = SessionRequestId::new("PUBLIC different request").unwrap();
    assert!(!c.duplicate(&Command::Prompt {
        generation: 3,
        request: body
    }));
    assert!(c.busy());
}
