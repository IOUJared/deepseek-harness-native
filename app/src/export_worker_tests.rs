//! Guard/ownership tests only: no Backend, GUI, Host, filesystem writes, or Node fixture.
use super::*;
use crate::exporter::{Outcome, Receipt};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::{sync::oneshot, time::timeout};

fn receipt() -> Receipt {
    Receipt {
        ticket: crate::management::Ticket {
            epoch: TRANSPORT_EPOCH,
            generation: 3,
            serial: 5,
            target: SessionId::new("PUBLIC-export-target").unwrap(),
        },
        editor: 7,
        attempt: 9,
    }
}
fn admission(target: &SessionId) -> ExportAdmission<'_> {
    ExportAdmission {
        smoke: false,
        stopping: false,
        operations: 0,
        generation: 3,
        target: Some(target),
        key_saving: false,
        plugins_active: false,
        management_active: false,
    }
}
fn submission() -> crate::exporter::Submission {
    use crate::exporter::{Action, Context, Exporter, View};
    let view = View {
        epoch: TRANSPORT_EPOCH,
        generation: 3,
        target: receipt().ticket.target,
    };
    let context = Context {
        view: Some(view.clone()),
        allowed: true,
    };
    let mut exporter = Exporter::default();
    assert!(exporter.apply(Action::Open(view), &context).is_none());
    let ticket = exporter.ticket().unwrap();
    assert!(
        exporter
            .apply(
                Action::Edit {
                    ticket: ticket.clone(),
                    path: "/PUBLIC-not-created.zip".into()
                },
                &context
            )
            .is_none()
    );
    assert!(exporter.apply(Action::Review(ticket), &context).is_none());
    let review = exporter.review().unwrap().clone();
    exporter
        .apply(Action::Confirm(review), &context)
        .expect("reviewed public submission")
}

#[test]
fn export_requires_fresh_ordinary_selected_target_epoch_generation_and_idle_sensitive_operations() {
    let baseline = receipt();
    let wrong_target = SessionId::new("PUBLIC-other-target").unwrap();
    for case in 0..11 {
        let mut candidate = baseline.clone();
        let mut context = admission(&baseline.ticket.target);
        match case {
            0 => context.smoke = true,
            1 => context.stopping = true,
            2 => context.operations = MAX_OPERATIONS,
            3 => candidate.ticket.epoch = 0,
            4 => candidate.ticket.epoch = TRANSPORT_EPOCH + 1,
            5 => candidate.ticket.generation += 1,
            6 => context.target = Some(&wrong_target),
            7 => context.target = None,
            8 => context.key_saving = true,
            9 => context.plugins_active = true,
            10 => context.management_active = true,
            _ => unreachable!(),
        }
        let mut active = None;
        assert!(
            !admit_export(&mut active, &candidate, context),
            "rejected admission case"
        );
        assert!(active.is_none(), "rejection reserves no export slot");
    }
    let mut active = None;
    let mut context = admission(&baseline.ticket.target);
    context.operations = MAX_OPERATIONS - 1;
    assert!(admit_export(&mut active, &baseline, context));
    assert_eq!(active, Some(baseline));
}

#[test]
fn selected_address_never_grants_parent_subagent_or_agent_export_authority() {
    let root = SessionId::new("PUBLIC-root").unwrap();
    assert_eq!(
        selected_export_target(&SessionAddress::Session {
            session_id: root.clone()
        }),
        Some(root.clone())
    );
    for mode in [SubagentMode::OneShot, SubagentMode::Continuable] {
        assert!(
            selected_export_target(&SessionAddress::Subagent {
                parent_session_id: root.clone(),
                child_session_id: SessionId::new("PUBLIC-child").unwrap(),
                mode,
            })
            .is_none()
        );
    }
}

#[test]
fn one_global_export_slot_counts_once_in_both_download_and_blocking_save_phases() {
    assert_eq!(operation_count(0, false), 0);
    assert_eq!(operation_count(MAX_OPERATIONS - 1, true), MAX_OPERATIONS);
    assert_eq!(operation_count(usize::MAX, true), usize::MAX);
    // Phase transition keeps the same receipt; separate carrier/save sets add no second slot.
    let mut active = None;
    let first = receipt();
    assert!(admit_export(
        &mut active,
        &first,
        admission(&first.ticket.target)
    ));
    let mut other = first.clone();
    other.attempt += 1;
    other.ticket.target = SessionId::new("PUBLIC-other-target").unwrap();
    assert!(!admit_export(
        &mut active,
        &other,
        admission(&other.ticket.target)
    ));
    assert_eq!(active, Some(first.clone()));
    assert_eq!(operation_count(7, active.is_some()), 8);
    assert!(!admit_export(
        &mut active,
        &first,
        admission(&first.ticket.target)
    ));
    assert_eq!(active, Some(first));
}

#[test]
fn exact_completion_only_releases_its_owned_slot_and_stale_rejection_cannot_clear_active() {
    let first = receipt();
    for outcome in [
        Outcome::Saved { bytes: 16 },
        Outcome::NotSent,
        Outcome::DownloadFailed,
        Outcome::NotCreated,
        Outcome::MayRemain,
    ] {
        let mut active = Some(first.clone());
        settle_export(&mut active, &Event::Ready("PUBLIC".into()));
        for field in 0..6 {
            let mut stale = first.clone();
            match field {
                0 => stale.ticket.epoch += 1,
                1 => stale.ticket.generation += 1,
                2 => stale.ticket.serial += 1,
                3 => stale.ticket.target = SessionId::new("PUBLIC-other-target").unwrap(),
                4 => stale.editor += 1,
                5 => stale.attempt += 1,
                _ => unreachable!(),
            }
            settle_export(
                &mut active,
                &Event::Exported {
                    receipt: stale,
                    outcome,
                },
            );
            assert_eq!(active, Some(first.clone()));
        }
        settle_export(
            &mut active,
            &Event::Exported {
                receipt: first.clone(),
                outcome,
            },
        );
        assert!(active.is_none());
        let mut next = first.clone();
        next.attempt += 1;
        assert!(admit_export(
            &mut active,
            &next,
            admission(&next.ticket.target)
        ));
        settle_export(
            &mut active,
            &Event::Exported {
                receipt: first.clone(),
                outcome: Outcome::NotSent,
            },
        );
        assert_eq!(active, Some(next));
    }
}

#[test]
fn active_export_reverse_fences_key_plugin_and_registry_writes() {
    let ticket = crate::settings::Ticket {
        epoch: TRANSPORT_EPOCH,
        panel: 3,
        serial: 4,
    };
    assert!(key_save_guard(ticket, false, false, false, false));
    assert!(!key_save_guard(ticket, false, false, false, true));
    let plugin = PluginOperation::Read(crate::plugins::ReadTicket {
        epoch: TRANSPORT_EPOCH,
        panel: 3,
        serial: 4,
    });
    let mut active_plugin = None;
    // Production folds active export into the existing exclusive-key-write guard.
    assert!(!admit_plugin(
        &mut active_plugin,
        plugin,
        false,
        false,
        1,
        true
    ));
    assert!(active_plugin.is_none());
    for operation in [
        crate::management::Operation::Pin,
        crate::management::Operation::Unpin,
        crate::management::Operation::Archive,
        crate::management::Operation::Restore,
    ] {
        let submission = crate::management::Submission {
            ticket: receipt().ticket,
            operation,
        };
        let mut active = None;
        assert!(!admit_management_without_export(
            &mut active,
            &submission,
            false,
            false,
            1,
            true
        ));
        assert!(active.is_none());
        assert!(admit_management_without_export(
            &mut active,
            &submission,
            false,
            false,
            0,
            false
        ));
    }
}

#[test]
fn rejected_export_event_is_only_receipt_and_fixed_metadata_and_never_prepares_path() {
    let submission = submission();
    let expected = submission.receipt.clone();
    let event = rejected(Command::Export(submission));
    assert_eq!(format!("{event:?}"), "NativeWorkerEvent(redacted)");
    assert!(
        matches!(event,Event::Exported {receipt,outcome:Outcome::NotSent} if receipt==expected)
    );
}

#[test]
fn bounded_command_queue_returns_owned_submission_and_shutdown_still_bypasses_it() {
    let (handle, _, _commands, _events, closing) = test_channels();
    for _ in 0..32 {
        assert!(handle.send(Command::Inspect).is_ok());
    }
    let submission = submission();
    let expected = submission.receipt.clone();
    assert!(
        matches!(handle.send(Command::Export(submission)),Err(Command::Export(returned)) if returned.receipt==expected)
    );
    assert!(handle.send(Command::Shutdown).is_ok());
    assert!(*closing.borrow());
}

#[tokio::test]
async fn owned_download_cancellation_is_awaited_and_does_not_start_any_file_closure() {
    struct Guard(Arc<AtomicBool>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let file_calls = Arc::new(AtomicUsize::new(0));
    let guard = Guard(dropped.clone());
    let called = file_calls.clone();
    let (started_tx, started_rx) = oneshot::channel();
    let mut download = Some(tokio::spawn(async move {
        let _guard = guard;
        let _ = started_tx.send(());
        std::future::pending::<()>().await;
        called.fetch_add(1, Ordering::Relaxed);
        16usize
    }));
    started_rx.await.unwrap();
    timeout(Duration::from_secs(1), cancel_download(&mut download))
        .await
        .unwrap();
    assert!(download.is_none());
    assert!(dropped.load(Ordering::Acquire));
    assert_eq!(file_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn fresh_shutdown_closed_watch_and_closed_event_channel_each_prevent_file_admission() {
    for case in 0..3 {
        let (closing_tx, closing) = watch::channel(false);
        let (events, receiver) = mpsc::channel::<Event>(1);
        let mut closing_tx = Some(closing_tx);
        let mut receiver = Some(receiver);
        match case {
            0 => {
                closing_tx.as_ref().unwrap().send_replace(true);
            }
            1 => {
                closing_tx.take();
            }
            2 => {
                receiver.take();
            }
            _ => unreachable!(),
        }
        let called = Arc::new(AtomicBool::new(false));
        let marker = called.clone();
        let mut saves = JoinSet::new();
        assert!(!spawn_owned_save(
            &mut saves,
            &closing,
            &events,
            move || {
                marker.store(true, Ordering::Release);
                16usize
            }
        ));
        assert!(saves.is_empty());
        assert!(!called.load(Ordering::Acquire));
    }
}

#[tokio::test]
async fn completed_download_is_dropped_without_file_start_after_shutdown() {
    let (closing_tx, closing) = watch::channel(false);
    let (events, _receiver) = mpsc::channel::<Event>(1);
    let mut download = Some(tokio::spawn(async { Ok::<_, Outcome>(16usize) }));
    let token = download_completion(next_download(&mut download).await).unwrap();
    download.take();
    assert_eq!(token, 16);
    closing_tx.send_replace(true);
    let called = Arc::new(AtomicBool::new(false));
    let marker = called.clone();
    let mut saves = JoinSet::new();
    assert!(!spawn_owned_save(
        &mut saves,
        &closing,
        &events,
        move || {
            marker.store(true, Ordering::Release);
            token
        }
    ));
    assert_eq!(saves.len(), 0);
    assert!(!called.load(Ordering::Acquire));
}

#[tokio::test]
async fn admitted_blocking_save_stays_owned_until_join_even_after_shutdown_and_other_task_abort() {
    let (closing_tx, closing) = watch::channel(false);
    let (events, _receiver) = mpsc::channel::<Event>(1);
    let completed = Arc::new(AtomicBool::new(false));
    let stop_requested = Arc::new(AtomicBool::new(false));
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let marker = completed.clone();
    let stop = stop_requested.clone();
    let mut saves = JoinSet::new();
    assert!(spawn_owned_save(&mut saves, &closing, &events, move || {
        let _ = started_tx.send(());
        release_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("PUBLIC fake file release");
        assert!(
            stop.load(Ordering::Acquire),
            "owned stop requested before save join finishes"
        );
        marker.store(true, Ordering::Release);
        16usize
    }));
    started_rx.await.unwrap();
    let mut ordinary = JoinSet::new();
    ordinary.spawn(std::future::pending::<()>());
    // Fake lifecycle hooks mirror root ordering without creating any real Backend/Host.
    stop_requested.store(true, Ordering::Release);
    closing_tx.send_replace(true);
    let mut download = Some(tokio::spawn(std::future::pending::<()>()));
    cancel_download(&mut download).await;
    ordinary.abort_all();
    while ordinary.join_next().await.is_some() {}
    assert_eq!(
        saves.len(),
        1,
        "owned file save not included in aborted ordinary tasks"
    );
    assert!(
        timeout(Duration::from_millis(30), drain_saves(&mut saves))
            .await
            .is_err()
    );
    assert!(!completed.load(Ordering::Acquire));
    release_tx.send(()).unwrap();
    timeout(Duration::from_secs(1), drain_saves(&mut saves))
        .await
        .unwrap();
    assert!(completed.load(Ordering::Acquire));
    assert!(
        saves.is_empty(),
        "terminal lifecycle may follow only the save join"
    );
}

#[tokio::test]
async fn download_and_save_panics_have_fixed_stage_specific_metadata_outcomes() {
    async fn failed_download() -> Result<usize, Outcome> {
        panic!("PUBLIC fake download failure");
    }
    let download = tokio::spawn(failed_download());
    assert_eq!(
        download_completion(download.await),
        Err(Outcome::DownloadFailed)
    );
    let save = tokio::task::spawn_blocking(|| -> Event {
        panic!("PUBLIC fake save failure");
    });
    let expected = receipt();
    let event = save_completion(expected.clone(), save.await);
    assert!(
        matches!(event,Event::Exported {receipt,outcome:Outcome::MayRemain} if receipt==expected)
    );
}
