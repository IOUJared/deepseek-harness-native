use super::*;
use crate::interactions::{Reply, Submission};
use dsh_native_transport::dto::{
    ApprovalOutcome, QuestionAnswer, QuestionAnswerItem, WaterfallKind,
};
use serde_json::{Value, json};

fn frame(id: &str, kind: WaterfallKind, request: Value) -> RemoteEventFrame {
    RemoteEventFrame::Waterfall {
        event_id: RemoteEventId::new(id).unwrap(),
        agent_id: AgentId::new("agent-NOT-selected-session").unwrap(),
        event: kind,
        request,
    }
}
fn approval(id: &str) -> RemoteEventFrame {
    frame(
        id,
        WaterfallKind::Approval,
        json!({"toolName":"PUBLIC fixture tool","reason":"PUBLIC reason"}),
    )
}
fn questions(id: &str, multi: bool) -> RemoteEventFrame {
    frame(
        id,
        WaterfallKind::Question,
        json!({"questions":[{"id":"q","question":"PUBLIC fixture question","options":[{"label":"yes"},{"label":"no"}],"multiSelect":multi}]}),
    )
}
fn answer(ids: &[&str], selected: &[&str], custom: Option<&str>) -> Reply {
    Reply::Question(QuestionAnswer {
        answers: ids
            .iter()
            .map(|id| QuestionAnswerItem {
                id: (*id).into(),
                selected: selected.iter().map(|value| (*value).into()).collect(),
                custom: custom.map(str::to_owned),
            })
            .collect(),
    })
}
fn submission(key: &Key, attempt: u64, reply: Reply) -> Submission {
    Submission {
        key: key.clone(),
        attempt,
        reply,
    }
}
fn is_rejected<D>(admission: Admission<D>) -> bool {
    matches!(admission, Admission::Reject)
}

#[test]
fn ready_admission_checks_exact_owner_liveness_without_sweeping_active_ack() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let life = Arc::new(AtomicBool::new(true));
    let mut worker = DecisionCoordinator::default();
    let key = worker.observe(1, &approval("one"), life.clone()).unwrap();
    assert!(worker.delivery_live(&key, |life| life.load(Ordering::SeqCst)));
    let active = submission(&key, 1, Reply::Approval(ApprovalOutcome::AllowedOnce));
    let enabled = worker.delivery_live(&key, |life| life.load(Ordering::SeqCst));
    assert!(matches!(
        worker.admit(&active, 1, enabled),
        Admission::Send(_)
    ));
    life.store(false, Ordering::SeqCst);
    assert!(!worker.delivery_live(&key, |life| life.load(Ordering::SeqCst)));
    assert!(worker.is_active(&active)); // Own ACK can revoke before matching completion arrives.
    worker.settle(&active, false);
    let enabled = worker.delivery_live(&key, |life| life.load(Ordering::SeqCst));
    assert!(is_rejected(worker.admit(
        &submission(&key, 2, Reply::Approval(ApprovalOutcome::AllowedOnce)),
        1,
        enabled
    )));
    assert!(!worker.delivery_live(
        &Key {
            serial: key.serial + 1,
            ..key
        },
        |_| panic!("wrong ticket must not inspect owner")
    ));
}

#[test]
fn worker_ticket_is_required_and_exact_epoch_serial_kind_attempt_are_checked() {
    let mut worker = DecisionCoordinator::default();
    let key = worker.observe(1, &approval("one"), 7u64).unwrap();
    for wrong in [
        Key {
            serial: key.serial + 1,
            ..key.clone()
        },
        Key {
            epoch: 2,
            ..key.clone()
        },
        Key {
            event_id: RemoteEventId::new("absent").unwrap(),
            ..key.clone()
        },
    ] {
        assert!(is_rejected(worker.admit(
            &submission(&wrong, 1, Reply::Approval(ApprovalOutcome::AllowedOnce)),
            1,
            true
        )));
    }
    assert!(is_rejected(worker.admit(
        &submission(&key, 0, Reply::Approval(ApprovalOutcome::AllowedOnce)),
        1,
        true
    )));
    assert!(is_rejected(worker.admit(
        &submission(&key, 1, Reply::CancelQuestion),
        1,
        true
    )));
    assert!(is_rejected(worker.admit(
        &submission(&key, 1, Reply::Approval(ApprovalOutcome::Unavailable)),
        1,
        true
    )));
    assert!(is_rejected(worker.admit(
        &submission(&key, 1, Reply::Approval(ApprovalOutcome::AllowedOnce)),
        1,
        false
    )));
    assert!(matches!(
        worker.admit(
            &submission(&key, 3, Reply::Approval(ApprovalOutcome::AllowedOnce)),
            1,
            true
        ),
        Admission::Send(7)
    ));
}

#[test]
fn timed_or_malformed_requests_cannot_bypass_ui_by_forged_worker_submission() {
    for request in [
        json!({"questions":[{"id":"q","question":"PUBLIC"}],"wait":{"callId":"call","timed":true}}),
        json!({"questions":[]}),
        json!({"questions":[{"id":"q","question":"PUBLIC"}],"wait":["call",true]}),
        json!({"questions":[{"id":"q","question":"PUBLIC"}],"agent":"forged"}),
        json!({"questions":[{"id":"q","question":"PUBLIC","options":[{"label":"yes"},{"label":"yes"}]}]}),
    ] {
        let mut worker = DecisionCoordinator::default();
        let key = worker
            .observe(
                1,
                &frame("question", WaterfallKind::Question, request),
                1u64,
            )
            .unwrap();
        for reply in [
            answer(&["q"], &[], None),
            Reply::CancelQuestion,
            Reply::Approval(ApprovalOutcome::AllowedOnce),
        ] {
            assert!(is_rejected(worker.admit(
                &submission(&key, 1, reply),
                1,
                true
            )));
        }
        assert!(worker.entries[&key.event_id].active.is_none());
    }
}

#[test]
fn duplicate_active_attempt_is_ignored_not_a_false_failure_receipt() {
    let mut worker = DecisionCoordinator::default();
    let key = worker.observe(1, &approval("one"), 1u64).unwrap();
    let active = submission(&key, 1, Reply::Approval(ApprovalOutcome::AllowedOnce));
    assert!(matches!(worker.admit(&active, 1, true), Admission::Send(_)));
    // Even changing the cloned reply cannot manufacture a receipt for the real active attempt.
    let duplicate = submission(&key, 1, Reply::CancelQuestion);
    assert!(worker.is_active(&duplicate));
    assert!(matches!(
        worker.admit(&duplicate, 1, false),
        Admission::Ignore
    ));
    let other = submission(&key, 2, Reply::Approval(ApprovalOutcome::Rejected));
    assert!(is_rejected(worker.admit(&other, 1, true)));
    worker.settle(&other, true);
    assert!(worker.is_active(&active));
    worker.settle(&active, false);
    assert!(!worker.is_active(&active));
    assert!(is_rejected(worker.admit(&active, 1, true)));
    assert!(matches!(worker.admit(&other, 1, true), Admission::Send(_)));
}

#[test]
fn cancel_and_id_reuse_fence_old_attempt_and_old_completion() {
    let mut worker = DecisionCoordinator::default();
    let old_key = worker.observe(1, &approval("reused"), 1u64).unwrap();
    let old = submission(&old_key, 1, Reply::Approval(ApprovalOutcome::AllowedOnce));
    assert!(matches!(worker.admit(&old, 1, true), Admission::Send(1)));
    worker.cancel(&old.key.event_id);
    let new_key = worker.observe(1, &approval("reused"), 2u64).unwrap();
    assert!(new_key.serial > old_key.serial);
    assert!(is_rejected(worker.admit(&old, 1, true)));
    let new = submission(&new_key, 8, Reply::Approval(ApprovalOutcome::Rejected));
    assert!(matches!(worker.admit(&new, 1, true), Admission::Send(2)));
    worker.settle(&old, true);
    assert!(worker.is_active(&new));
    worker.settle(&old, false);
    assert!(worker.is_active(&new));
    worker.settle(&new, true);
    assert!(worker.entries.is_empty());
    assert!(is_rejected(worker.admit(&new, 1, true)));
}

#[test]
fn duplicate_observation_does_not_replace_live_payload_or_active_owner() {
    let mut worker = DecisionCoordinator::default();
    let key = worker.observe(1, &approval("one"), 1u64).unwrap();
    let active = submission(&key, 1, Reply::Approval(ApprovalOutcome::AllowedOnce));
    assert!(matches!(worker.admit(&active, 1, true), Admission::Send(1)));
    assert!(worker.observe(1, &questions("one", false), 2).is_err());
    assert!(worker.is_active(&active));
    assert_eq!(worker.serial, key.serial);
    assert_eq!(worker.entries[&key.event_id].delivery, 1);
}

#[test]
fn worker_revalidates_complete_answer_ids_labels_single_multi_custom_and_skip() {
    let invalid = [
        answer(&[], &[], None),
        answer(&["absent"], &[], None),
        answer(&["q", "q"], &[], None),
        answer(&["q"], &["unknown"], None),
        answer(&["q"], &["yes", "yes"], None),
        answer(&["q"], &["yes", "no"], None),
        answer(&["q"], &["yes"], Some("custom")),
        answer(&["q"], &[], Some(" ")),
        answer(&["q"], &[], Some(&"x".repeat(2049))),
    ];
    let mut worker = DecisionCoordinator::default();
    let key = worker.observe(1, &questions("q", false), 1u64).unwrap();
    for reply in invalid {
        assert!(is_rejected(worker.admit(
            &submission(&key, 1, reply),
            1,
            true
        )));
    }
    for reply in [
        answer(&["q"], &[], None),
        answer(&["q"], &["yes"], None),
        answer(&["q"], &[], Some("PUBLIC custom")),
        Reply::CancelQuestion,
    ] {
        let mut worker = DecisionCoordinator::default();
        let key = worker.observe(1, &questions("q", false), 1u64).unwrap();
        assert!(matches!(
            worker.admit(&submission(&key, 1, reply), 1, true),
            Admission::Send(1)
        ));
    }
    let mut worker = DecisionCoordinator::default();
    let key = worker.observe(1, &questions("multi", true), 1u64).unwrap();
    assert!(matches!(
        worker.admit(
            &submission(
                &key,
                1,
                answer(&["q"], &["yes", "no"], Some("PUBLIC custom"))
            ),
            1,
            true
        ),
        Admission::Send(1)
    ));
}

#[test]
fn worker_retains_failed_delivery_for_explicit_new_attempt_not_replay_or_auto_retry() {
    let mut worker = DecisionCoordinator::default();
    let key = worker.observe(1, &approval("one"), 1u64).unwrap();
    let attempt = submission(&key, 50, Reply::Approval(ApprovalOutcome::AllowedOnce));
    assert!(matches!(
        worker.admit(&attempt, 1, true),
        Admission::Send(_)
    ));
    worker.settle(&attempt, false);
    assert!(is_rejected(worker.admit(&attempt, 1, true)));
    assert!(is_rejected(worker.admit(
        &submission(&key, 49, Reply::Approval(ApprovalOutcome::Rejected)),
        1,
        true
    )));
    assert!(matches!(
        worker.admit(
            &submission(&key, 90, Reply::Approval(ApprovalOutcome::Rejected)),
            1,
            true
        ),
        Admission::Send(_)
    ));
}

#[test]
fn worker_capacity_and_checked_serial_counter_refuse_without_displacing_owner() {
    let mut worker = DecisionCoordinator::default();
    for index in 0..MAX_PENDING {
        worker
            .observe(1, &approval(&format!("id-{index}")), index)
            .unwrap();
    }
    let serial = worker.serial;
    assert!(worker.observe(1, &approval("overflow"), 99).is_err());
    assert_eq!(worker.entries.len(), MAX_PENDING);
    assert_eq!(worker.serial, serial);
    worker.cancel(&RemoteEventId::new("id-0").unwrap());
    assert!(worker.observe(1, &approval("new"), 99).is_ok());
    let mut worker = DecisionCoordinator::default();
    worker.serial = u64::MAX;
    assert!(worker.observe(1, &approval("one"), 1u64).is_err());
    assert!(worker.entries.is_empty());
}

async fn bounded<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(std::time::Duration::from_secs(5), future)
        .await
        .expect("owned fixture operation deadline")
}
async fn carrier(mode: &str) -> (tokio::process::Child, dsh_native_transport::NativeClient) {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};
    let fixture = format!(
        "{}/../transport/tests/node_host_fixture.mjs",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut child = tokio::process::Command::new("/usr/bin/node")
        .env_clear()
        .arg(fixture)
        .arg(mode)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let url = tokio::time::timeout(std::time::Duration::from_secs(5), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let client = dsh_native_transport::NativeClient::connect(
        dsh_native_transport::SecretLaunchUrl::new(url).unwrap(),
        dsh_native_transport::Limits::default(),
    )
    .await
    .unwrap();
    (child, client)
}
#[tokio::test]
async fn actual_carrier_frontend_ticket_worker_admission_reply_and_exact_ack() {
    let (mut child, client) = carrier("normal").await;
    let mux = client.connect_mux().await.unwrap();
    let mut events = mux.events().await.unwrap();
    assert!(matches!(
        bounded(events.next_event()).await.unwrap().unwrap().frame,
        RemoteEventFrame::Ready { .. }
    ));
    let owned = bounded(events.next_event()).await.unwrap().unwrap();
    let delivery = std::sync::Arc::new(owned.delivery.unwrap());
    let mut coordinator = DecisionCoordinator::default();
    let key = coordinator
        .observe(1, &owned.frame, delivery.clone())
        .unwrap();
    let RemoteEventFrame::Waterfall {
        agent_id,
        event,
        request,
        ..
    } = owned.frame
    else {
        panic!("fixture waterfall missing");
    };
    let mut frontend = crate::interactions::Decisions::default();
    frontend
        .add_owned(key.clone(), agent_id, event, request)
        .unwrap();
    let submission = frontend
        .handle(
            crate::interactions::Action::Approval(key.clone(), ApprovalOutcome::AllowedOnce),
            1,
            true,
        )
        .unwrap();
    let Admission::Send(permit) = coordinator.admit(&submission, 1, true) else {
        panic!("valid explicit decision not admitted");
    };
    let result = client
        .reply_approval_for(&permit, ApprovalOutcome::AllowedOnce)
        .await;
    assert_eq!(result, Ok(()));
    assert!(!delivery.is_live()); // Own receipt already retires the token, not a business outcome.
    assert!(coordinator.is_active(&submission));
    coordinator.settle(&submission, result.is_ok());
    assert!(frontend.acknowledge(
        &key,
        submission.attempt,
        result.map_err(|error| error.to_string())
    ));
    assert_eq!(frontend.len(), 0);
    assert!(is_rejected(coordinator.admit(&submission, 1, true)));
    mux.close().await;
    client.close().await;
    child.kill().await.unwrap();
    child.wait().await.unwrap();
}
#[tokio::test]
async fn actual_timed_carrier_remains_unsupported_at_both_frontend_and_worker() {
    let (mut child, client) = carrier("claim-normal").await;
    let mux = client.connect_mux().await.unwrap();
    let mut events = mux.events().await.unwrap();
    let _ready = bounded(events.next_event()).await.unwrap().unwrap();
    let owned = bounded(events.next_event()).await.unwrap().unwrap();
    let mut coordinator = DecisionCoordinator::default();
    let key = coordinator
        .observe(
            1,
            &owned.frame,
            std::sync::Arc::new(owned.delivery.unwrap()),
        )
        .unwrap();
    let RemoteEventFrame::Waterfall {
        agent_id,
        event,
        request,
        ..
    } = owned.frame
    else {
        panic!("fixture waterfall missing");
    };
    let mut frontend = crate::interactions::Decisions::default();
    frontend
        .add_owned(key.clone(), agent_id, event, request)
        .unwrap();
    assert!(
        frontend
            .handle(
                crate::interactions::Action::CancelQuestion(key.clone()),
                1,
                true
            )
            .is_none()
    );
    for reply in [answer(&["q"], &["yes"], None), Reply::CancelQuestion] {
        assert!(is_rejected(coordinator.admit(
            &submission(&key, 1, reply),
            1,
            true
        )));
    }
    let roster = client.session_list().await.unwrap();
    let stats: Value = serde_json::from_str(roster.items[0].cwd.as_deref().unwrap()).unwrap();
    assert_eq!(stats["opens"], 0);
    assert_eq!(stats["replyRequests"], 0);
    assert_eq!(stats["methods"], json!({"session/list":1}));
    mux.close().await;
    client.close().await;
    child.kill().await.unwrap();
    child.wait().await.unwrap();
}

#[test]
fn worker_complete_retention_budget_refuses_large_valid_requests() {
    let mut worker = DecisionCoordinator::default();
    let big = json!({"toolName":"PUBLIC","reason":"x".repeat(30000)});
    let mut admitted = 0;
    while worker
        .observe(
            1,
            &frame(
                &format!("id-{admitted}"),
                WaterfallKind::Approval,
                big.clone(),
            ),
            admitted,
        )
        .is_ok()
    {
        admitted += 1;
    }
    assert!(admitted > 0 && admitted < MAX_PENDING);
    let total = worker
        .entries
        .values()
        .map(|entry| entry.bytes)
        .sum::<usize>();
    assert!(total <= MAX_RETAINED);
    worker.cancel(&RemoteEventId::new("id-0").unwrap());
    assert!(worker.observe(1, &approval("small"), 999).is_ok());
}
