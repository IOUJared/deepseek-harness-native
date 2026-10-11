//! Production App/channel tests. Synthetic callbacks do not establish OS pointer/drop delivery.
use super::*;
use crate::ui::attachment_ui_tests::{ready_app, upload};
fn outcome() -> Arc<attachment_preview::Outcome> {
    Arc::new(attachment_preview::Outcome::Ready {
        width: 1,
        height: 1,
        rgba: vec![1, 2, 3, 255],
    })
}
fn selected(app: &mut App) -> (attachment_draft::EditorStamp, PreviewTicket) {
    drop(app.choose_local_file(PathBuf::from("/PUBLIC-local-image.png")));
    let stamp = app.files.editor().unwrap().0;
    let ticket = app.local_files.pending.as_ref().unwrap().0;
    (stamp, ticket)
}
#[test]
fn drop_selects_only_into_review_and_ignores_foreign_windows() {
    let (mut app, mut rx) = ready_app();
    let id = window::Id::unique();
    app.own_window = Some(id);
    drop(app.update(Message::FileDropped(
        window::Id::unique(),
        PathBuf::from("/PUBLIC.png"),
    )));
    assert!(app.files.editor().is_none());
    assert!(!app.local_files.busy());
    drop(app.update(Message::FileDropped(id, PathBuf::from("/PUBLIC.png"))));
    assert_eq!(app.files.editor().unwrap().1, "/PUBLIC.png");
    assert!(rx.try_recv().is_err());
    let owner = app.files.editor().unwrap().0;
    drop(app.update(Message::FileDropped(id, PathBuf::from("/SECOND.png"))));
    assert_eq!(app.files.editor().unwrap().0, owner);
    assert!(app.status.contains("current file selection"));
    assert!(!app.send_allowed());
    assert!(!app.files.pending());
}
#[test]
fn failed_drop_is_visible_only_for_owned_active_window_without_upload() {
    let (mut app, mut rx) = ready_app();
    let id = window::Id::unique();
    app.own_window = Some(id);
    let status = app.status.clone();
    drop(app.update(Message::FileDropCancelled(window::Id::unique())));
    assert_eq!(app.status, status);
    drop(app.update(Message::FileDropCancelled(id)));
    assert!(app.status.contains("Drop canceled"));
    assert!(!routine_status(&app.status));
    assert!(app.files.editor().is_none());
    assert!(!app.local_files.busy());
    assert!(rx.try_recv().is_err());
    app.new_pending = true;
    app.status = "PUBLIC pending".into();
    drop(app.update(Message::FileDropCancelled(id)));
    assert_eq!(app.status, "PUBLIC pending");
}
#[test]
fn blocked_and_invalid_drops_never_read_or_upload() {
    for blocked in 0..5 {
        let (mut app, mut rx) = ready_app();
        let id = window::Id::unique();
        app.own_window = Some(id);
        match blocked {
            0 => app.observations_lost = true,
            1 => app.follow_ready = false,
            2 => {
                app.settings
                    .handle(crate::settings::Action::Open, app.transport_epoch, true);
            }
            3 => app.close = Close::Confirm("PUBLIC".into()),
            _ => {
                app.options.smoke = Some(crate::config::Smoke {
                    report: PathBuf::from("/PUBLIC-unused"),
                    screenshot: None,
                    exit_after: Duration::from_secs(1),
                })
            }
        }
        drop(app.update(Message::FileDropped(id, PathBuf::from("/PUBLIC.png"))));
        assert!(app.files.editor().is_none());
        assert!(!app.local_files.busy());
        assert!(rx.try_recv().is_err());
    }
    for path in [
        PathBuf::from("relative.png"),
        PathBuf::from("/"),
        PathBuf::from("/bad\0.png"),
    ] {
        let (mut app, mut rx) = ready_app();
        drop(app.choose_local_file(path));
        assert!(app.files.editor().is_none());
        assert!(!app.local_files.busy());
        assert!(rx.try_recv().is_err());
    }
}
#[test]
fn cancellation_holds_local_read_slot_and_late_preview_never_reappears() {
    let (mut app, mut rx) = ready_app();
    let (stamp, ticket) = selected(&mut app);
    let canceled = app.local_files.pending.as_ref().unwrap().1.clone();
    drop(app.update(Message::FileCancel(stamp)));
    assert!(canceled.load(Ordering::Relaxed));
    assert!(app.local_files.busy());
    assert!(!app.preview_current());
    drop(app.choose_local_file(PathBuf::from("/SECOND.png")));
    assert!(app.files.editor().is_none());
    drop(app.update(Message::FilePreviewRead(
        PreviewTicket {
            serial: ticket.serial + 1,
            ..ticket
        },
        outcome(),
    )));
    assert!(app.local_files.busy());
    drop(app.update(Message::FilePreviewRead(ticket, outcome())));
    assert!(!app.local_files.busy());
    assert!(app.local_files.preview.is_none());
    assert!(rx.try_recv().is_err());
}
#[test]
fn edits_and_repeated_preview_reads_have_distinct_authority() {
    let (mut app, mut rx) = ready_app();
    let (stamp, old) = selected(&mut app);
    drop(app.update(Message::FilePath(stamp, "/NEW.png".into())));
    assert!(
        app.local_files
            .pending
            .as_ref()
            .unwrap()
            .1
            .load(Ordering::Relaxed)
    );
    drop(app.update(Message::FilePreviewRead(old, outcome())));
    assert!(app.local_files.preview.is_none());
    let fresh = app.files.editor().unwrap().0;
    drop(app.update(Message::FilePreview(fresh)));
    let next = app.local_files.pending.as_ref().unwrap().0;
    assert_ne!(old, next);
    drop(app.update(Message::FilePreviewRead(old, outcome())));
    assert!(app.local_files.busy());
    drop(app.update(Message::FilePreviewRead(next, outcome())));
    assert!(app.local_files.preview.is_some());
    drop(app.update(Message::FilePreview(fresh)));
    let repeat = app.local_files.pending.as_ref().unwrap().0;
    assert_eq!(repeat.stamp, next.stamp);
    assert!(repeat.serial > next.serial);
    drop(app.update(Message::FilePreviewRead(next, outcome())));
    assert!(app.local_files.busy());
    assert!(rx.try_recv().is_err());
}
#[test]
fn preview_follows_exact_upload_and_remove_clears_it_without_replaying_bytes() {
    let (mut app, mut rx) = ready_app();
    let (stamp, ticket) = selected(&mut app);
    drop(app.update(Message::FilePreviewRead(ticket, outcome())));
    drop(app.update(Message::FileUpload(stamp)));
    let Command::StageFile(submission) = rx.try_recv().unwrap() else {
        panic!("expected guarded upload")
    };
    let uploaded = submission.ticket;
    assert!(app.preview_current());
    assert!(app.local_files.preview.is_some());
    drop(app.receive(Event::FileStaged {
        ticket: uploaded.clone(),
        outcome: worker::attachments::Outcome::Staged(worker::attachments::Metadata {
            name: "PUBLIC.png".into(),
            bytes: 4,
        }),
    }));
    assert!(app.preview_current());
    drop(app.update(Message::FileRemove(uploaded.clone())));
    assert!(matches!(rx.try_recv().unwrap(),Command::DiscardFile(t) if t==uploaded));
    assert!(!app.preview_current());
    assert!(app.local_files.preview.is_none());
}
#[test]
fn picker_cancel_and_manual_fallback_preserve_consent_and_joining() {
    let (mut app, mut rx) = ready_app();
    drop(app.update(Message::FileChoose));
    let ticket = app.local_files.picker.ticket().unwrap();
    assert!(app.local_files.busy());
    drop(app.update(Message::Send));
    drop(app.update(Message::FileOpen));
    assert!(rx.try_recv().is_err());
    drop(app.update(Message::FilePicker(Action::Close(ticket))));
    assert!(app.files.editor().is_none());
    assert!(app.local_files.busy());
    drop(app.update(Message::FilePickerRead(Arc::new(
        attachment_picker::Completion {
            ticket,
            result: Ok(vec![]),
        },
    ))));
    assert!(!app.local_files.busy());
    drop(app.update(Message::FileChoose));
    let fresh = app.local_files.picker.ticket().unwrap();
    drop(app.update(Message::FilePicker(Action::Manual(fresh))));
    assert!(app.files.editor().is_some());
    assert!(!app.local_files.picker.is_open());
    drop(app.update(Message::FilePickerRead(Arc::new(
        attachment_picker::Completion {
            ticket: fresh,
            result: Ok(vec![]),
        },
    ))));
    assert!(app.files.editor().unwrap().1.is_empty());
    assert!(rx.try_recv().is_err());
}
#[test]
fn picker_entry_selects_review_without_upload_and_obsolete_rows_are_ignored() {
    let (mut app, mut rx) = ready_app();
    drop(app.update(Message::FileChoose));
    let ticket = app.local_files.picker.ticket().unwrap();
    drop(app.update(Message::FilePickerRead(Arc::new(
        attachment_picker::Completion {
            ticket,
            result: Ok(vec![attachment_picker::Entry {
                name: "PUBLIC.png".into(),
                path: PathBuf::from("/PUBLIC.png"),
                directory: false,
            }]),
        },
    ))));
    drop(app.update(Message::FilePicker(Action::Choose(
        Ticket {
            serial: ticket.serial + 1,
            ..ticket
        },
        0,
    ))));
    assert!(app.local_files.picker.is_open());
    assert!(app.files.editor().unwrap().1.is_empty());
    drop(app.update(Message::FilePicker(Action::Choose(ticket, 0))));
    assert!(!app.local_files.picker.is_open());
    assert_eq!(app.files.editor().unwrap().1, "/PUBLIC.png");
    assert!(app.local_files.busy());
    assert!(rx.try_recv().is_err());
}
#[test]
fn local_callbacks_settle_behind_settings_and_shutdown_drains_before_exit() {
    let (mut app, mut rx) = ready_app();
    let (_, ticket) = selected(&mut app);
    {
        app.settings
            .handle(crate::settings::Action::Open, app.transport_epoch, true);
    };
    drop(app.update(Message::FilePreviewRead(ticket, outcome())));
    assert!(!app.local_files.busy());
    assert!(app.local_files.preview.is_some());
    app.settings.close();
    let stamp = app.files.editor().unwrap().0;
    drop(app.update(Message::FilePreview(stamp)));
    let pending = app.local_files.pending.as_ref().unwrap().0;
    app.shutdown();
    assert!(app.local_files.busy());
    assert!(app.local_files.preview.is_none());
    assert!(matches!(app.close, Close::Stopping));
    assert!(rx.try_recv().is_err()); // Shutdown uses its reserved watch signal, not this queue.
    let task = app.finish_draft_exit();
    assert!(task.units() == 0);
    drop(app.update(Message::FilePreviewRead(pending, outcome())));
    assert!(!app.local_files.busy());
    assert!(app.local_files.preview.is_none());
}
#[test]
fn ready_file_drop_never_replaces_existing_receipt() {
    let (mut app, mut rx) = ready_app();
    let ticket = upload(&mut app, &mut rx);
    drop(app.choose_local_file(PathBuf::from("/SECOND.png")));
    // The staged receipt keeps its slot; the drop opens a separate bounded review slot.
    assert_eq!(app.files.ticket(), Some(&ticket));
    assert_eq!(app.files.count(), 1);
    assert!(app.files.ready().is_some());
    assert_eq!(app.files.editor().unwrap().1, "/SECOND.png");
    assert!(app.local_files.busy());
    assert!(rx.try_recv().is_err());
}
#[test]
fn session_creation_and_preview_counter_exhaustion_fail_closed() {
    let (mut app, mut rx) = ready_app();
    app.new_pending = true;
    drop(app.choose_local_file(PathBuf::from("/PUBLIC.png")));
    assert!(app.files.editor().is_none());
    assert!(!app.local_files.busy());
    app.new_pending = false;
    drop(app.update(Message::FileOpen));
    let stamp = app.files.editor().unwrap().0;
    drop(app.update(Message::FilePath(stamp, "/PUBLIC.png".into())));
    let stamp = app.files.editor().unwrap().0;
    app.local_files.preview_serial = u64::MAX;
    drop(app.update(Message::FilePreview(stamp)));
    assert!(!app.local_files.busy());
    assert!(app.status.contains("exhausted"));
    assert!(rx.try_recv().is_err());
}
