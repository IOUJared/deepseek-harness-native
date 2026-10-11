use super::*;
use dsh_native_transport::{
    dto::{PromptContentPart, PromptMode},
    prompt_admission::{AdmissionEvidence, AdmissionPhase},
};
fn pending(id: &str) -> Pending {
    let session = SessionId::new("PUBLIC session").unwrap();
    Pending {
        session: session.clone(),
        text: "PUBLIC frozen original".into(),
        draft_serial: 0,
        epoch: 1,
        file_request_id: None,
        file: Vec::new(),
        file_ticket: Vec::new(),
        request: SessionPromptRequest {
            session_id: session,
            request_id: SessionRequestId::new(id).unwrap(),
            mode: PromptMode::Queue,
            content: vec![PromptContentPart::Text {
                text: "PUBLIC frozen original".into(),
            }],
            client_time_zone: Some("UTC".into()),
        },
    }
}
fn uncertain() -> Controller {
    let mut c = Controller::default();
    assert!(c.insert(3, pending("PUBLIC original id")));
    let t = c.ticket().unwrap();
    assert!(c.settle(&t, Receipt::Unknown).is_none());
    c
}
fn positive(phase: AdmissionPhase) -> PromptAdmission {
    PromptAdmission::Accepted {
        evidence: AdmissionEvidence::Journal,
        phase,
        message_id: None,
        seq: Some(0),
    }
}
#[test]
fn unknown_freezes_identity_payload_and_original_while_different_editor_draft_is_independent() {
    let mut c = uncertain();
    let original = c.ticket().unwrap();
    assert!(!c.insert(3, pending("PUBLIC replacement id")));
    let mut new_draft = "PUBLIC another draft".to_owned();
    new_draft.push('!');
    assert_eq!(c.ticket(), Some(original));
    assert_eq!(c.get(&3).unwrap().text, "PUBLIC frozen original");
    assert_eq!(new_draft, "PUBLIC another draft!");
    assert!(!format!("{c:?}").contains("PUBLIC frozen original"));
}
#[test]
fn only_exact_read_receipt_positive_or_proven_predispatch_rejection_releases_ownership() {
    for phase in [
        AdmissionPhase::Queued,
        AdmissionPhase::Claimed,
        AdmissionPhase::Admitted,
        AdmissionPhase::Recorded,
    ] {
        let mut c = uncertain();
        let t = c.ticket().unwrap();
        let read = c.begin_check(&t, 1).unwrap();
        let mut late = read.clone();
        late.serial += 1;
        assert!(c.checked(&late, Some(positive(phase))).is_none());
        assert!(!c.is_empty());
        assert_eq!(
            c.checked(&read, Some(positive(phase))).unwrap().1,
            Receipt::Accepted
        );
        assert!(c.is_empty());
    }
    for code in [
        AdmissionRejection::BadRequest,
        AdmissionRejection::InvalidTimeZone,
        AdmissionRejection::AttachmentInvalid,
        AdmissionRejection::NotFound,
        AdmissionRejection::AgentBusy,
        AdmissionRejection::PromptClosed,
    ] {
        let mut c = uncertain();
        let t = c.ticket().unwrap();
        let read = c.begin_check(&t, 1).unwrap();
        assert_eq!(
            c.checked(&read, Some(PromptAdmission::Rejected(code)))
                .unwrap()
                .1,
            Receipt::NotSent
        );
    }
}
#[test]
fn unknown_admitting_missing_and_internal_rejection_remain_owned_without_automatic_retry() {
    for value in [
        None,
        Some(PromptAdmission::Unknown),
        Some(PromptAdmission::Admitting),
        Some(PromptAdmission::Rejected(AdmissionRejection::Internal)),
    ] {
        let mut c = uncertain();
        let t = c.ticket().unwrap();
        let read = c.begin_check(&t, 1).unwrap();
        assert!(c.checked(&read, value).is_none());
        assert!(!c.is_empty());
        assert!(c.uncertain());
        assert!(!c.checking());
        assert_eq!(c.ticket(), Some(t));
    }
}
#[test]
fn old_epoch_session_request_and_read_serial_never_release_new_or_hidden_intent() {
    let mut c = uncertain();
    let t = c.ticket().unwrap();
    for wrong in [
        Ticket {
            epoch: 2,
            ..t.clone()
        },
        Ticket {
            generation: 4,
            ..t.clone()
        },
        Ticket {
            session: SessionId::new("PUBLIC other session").unwrap(),
            ..t.clone()
        },
        Ticket {
            request_id: SessionRequestId::new("PUBLIC old id").unwrap(),
            ..t.clone()
        },
    ] {
        assert!(c.settle(&wrong, Receipt::Accepted).is_none());
        assert!(c.begin_check(&wrong, 1).is_none());
    }
    let read = c.begin_check(&t, 1).unwrap();
    c.disconnect();
    assert!(
        c.checked(&read, Some(positive(AdmissionPhase::Recorded)))
            .is_none()
    );
    assert!(c.begin_check(&t, 0).is_none());
    assert_eq!(c.ticket(), Some(t));
}
#[test]
fn frozen_payload_bounds_and_mismatched_file_authority_fail_before_capture() {
    let mut c = Controller::default();
    let mut p = pending("PUBLIC id");
    p.text = "x".repeat(32 * 1024 + 1);
    assert!(!c.insert(3, p));
    let mut p = pending("PUBLIC id");
    p.request.content = vec![PromptContentPart::Text {
        text: "PUBLIC substituted".into(),
    }];
    assert!(!c.insert(3, p));
    let mut p = pending("PUBLIC id");
    p.file_request_id = Some(p.request.request_id.clone());
    assert!(!c.insert(3, p));
    assert!(c.is_empty());
}
