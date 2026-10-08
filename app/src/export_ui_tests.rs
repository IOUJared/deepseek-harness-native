// Included in the actual UI test module. No Backend, filesystem or model operations.
fn open_export(app: &mut App) -> crate::exporter::Ticket {
    assert!(app.management.baseline(vec![], vec![]));
    open_management(app);
    let ticket = app.management.ticket().unwrap();
    let _ = app.update(Message::ExportSession(ticket));
    assert!(!app.management.is_open());
    app.exporter.ticket().unwrap()
}
fn review_export(app: &mut App, ticket: crate::exporter::Ticket) -> crate::exporter::Review {
    let _ = app.update(Message::Export(crate::exporter::Action::Edit {
        ticket: ticket.clone(),
        path: "/PUBLIC/explicit-new-session.zip".into(),
    }));
    let _ = app.update(Message::Export(crate::exporter::Action::Review(ticket)));
    app.exporter.review().unwrap().clone()
}
#[test]
fn export_only_review_then_confirm_dispatches_one_consuming_command() {
    let (mut app, mut receiver) = managed_app();
    let ticket = open_export(&mut app);
    assert!(!app.composer_visible());
    assert!(!app.animation_active());
    let review = review_export(&mut app, ticket);
    assert!(receiver.try_recv().is_err());
    let _ = app.view();
    let _ = app.update(Message::Export(crate::exporter::Action::Confirm(
        review.clone(),
    )));
    assert!(app.exporter.path().is_empty());
    assert!(app.exporter.review().is_none());
    let submission = match receiver.try_recv().unwrap() {
        Command::Export(submission) => submission,
        _ => panic!("wrong native command"),
    };
    assert_eq!(
        submission.receipt.ticket.target,
        app.selected.clone().unwrap()
    );
    assert_eq!(app.exporter.pending(), Some(&submission.receipt));
    let _ = app.update(Message::Export(crate::exporter::Action::Confirm(review)));
    assert!(receiver.try_recv().is_err());
    // Dropping this owned destination does not touch the filesystem.
    let _ = app.receive(Event::Exported {
        receipt: submission.receipt.clone(),
        outcome: crate::exporter::Outcome::Saved { bytes: 512 },
    });
    assert_eq!(
        app.exporter.notice(),
        crate::exporter::Notice::Saved { bytes: 512 }
    );
}
#[test]
fn export_modal_blocks_shell_settings_registry_and_old_open_messages() {
    let (mut app, mut receiver) = managed_app();
    let ticket = open_export(&mut app);
    let _ = review_export(&mut app, ticket.clone());
    let target = app.selected.clone().unwrap();
    let initial_workspace_path = app.workspace_path.clone();
    let initial_filter = app.filter.clone();
    for message in [
        Message::Send,
        Message::Stop,
        Message::NewSession,
        Message::LoadModels,
        Message::Select(target),
        Message::WorkspacePath("unrequested".into()),
        Message::Filter("unrequested".into()),
        Message::ToggleSidebar,
        Message::ToggleRecords,
        Message::Capabilities,
        Message::Settings(crate::settings::Action::Open),
        Message::ExportSession(ticket),
        Message::Management(crate::management::Action::Open(
            app.operator_context().view.unwrap(),
        )),
    ] {
        let _ = app.update(message);
    }
    assert!(receiver.try_recv().is_err());
    assert!(!app.settings.is_open());
    assert!(!app.management.is_open());
    assert!(app.exporter.review().is_some());
    assert_eq!(app.workspace_path, initial_workspace_path);
    assert_eq!(app.filter, initial_filter);
}
#[test]
fn settings_foreground_fences_export_and_escape_only_closes_visible_settings() {
    let (mut app, mut receiver) = managed_app();
    let ticket = open_export(&mut app);
    let review = review_export(&mut app, ticket);
    let _ = app
        .settings
        .handle(crate::settings::Action::Open, app.transport_epoch, false);
    assert!(app.settings_modal_visible());
    let _ = app.update(Message::Export(crate::exporter::Action::Confirm(review)));
    let _ = app.update(Message::DismissPanel);
    assert!(!app.settings.is_open());
    assert!(app.exporter.is_open());
    assert!(receiver.try_recv().is_err());
    let _ = app.update(Message::DismissPanel);
    assert!(!app.exporter.is_open());
    assert!(app.exporter.path().is_empty());
}
#[test]
fn export_open_from_stale_management_target_or_generation_never_retargets() {
    let (mut app, mut receiver) = managed_app();
    assert!(app.management.baseline(vec![], vec![]));
    open_management(&mut app);
    let original = app.management.ticket().unwrap();
    let mut wrong = original.clone();
    wrong.serial += 1;
    let _ = app.update(Message::ExportSession(wrong));
    assert!(!app.exporter.is_open());
    app.generation += 1;
    let _ = app.update(Message::ExportSession(original));
    assert!(!app.exporter.is_open());
    assert!(app.management.is_open());
    assert!(receiver.try_recv().is_err());
}
#[test]
fn export_queue_rejection_is_not_sent_and_does_not_restore_consumed_path() {
    let (mut app, receiver) = managed_app();
    let ticket = open_export(&mut app);
    let review = review_export(&mut app, ticket);
    drop(receiver);
    let _ = app.update(Message::Export(crate::exporter::Action::Confirm(review)));
    assert!(app.exporter.pending().is_none());
    assert!(app.exporter.path().is_empty());
    assert_eq!(app.exporter.notice(), crate::exporter::Notice::NotSent);
}
#[test]
fn export_close_keeps_pending_and_old_receipt_cannot_replace_reopened_draft() {
    let (mut app, mut receiver) = managed_app();
    let ticket = open_export(&mut app);
    let review = review_export(&mut app, ticket);
    let _ = app.update(Message::Export(crate::exporter::Action::Confirm(review)));
    let submission = match receiver.try_recv().unwrap() {
        Command::Export(s) => s,
        _ => panic!("wrong native command"),
    };
    let _ = app.update(Message::DismissPanel);
    assert!(app.exporter.pending().is_some());
    assert!(!app.settings_enabled());
    assert!(!app.management_context().allowed);
    let view = app.export_context().view.unwrap();
    let _ = app.update(Message::Export(crate::exporter::Action::Open(view)));
    let ticket = app.exporter.ticket().unwrap();
    let _ = app.update(Message::Export(crate::exporter::Action::Edit {
        ticket: ticket.clone(),
        path: "/PUBLIC/newer-draft.zip".into(),
    }));
    let _ = app.update(Message::Export(crate::exporter::Action::Review(ticket)));
    assert!(app.exporter.review().is_none());
    let _ = app.receive(Event::Exported {
        receipt: submission.receipt.clone(),
        outcome: crate::exporter::Outcome::MayRemain,
    });
    assert!(app.exporter.pending().is_none());
    assert_eq!(app.exporter.path(), "/PUBLIC/newer-draft.zip");
    assert_ne!(app.exporter.notice(), crate::exporter::Notice::MayRemain);
}
#[test]
fn export_shutdown_invalidates_late_receipts_without_false_saved_claim() {
    let (mut app, mut receiver) = managed_app();
    let ticket = open_export(&mut app);
    let review = review_export(&mut app, ticket);
    let _ = app.update(Message::Export(crate::exporter::Action::Confirm(review)));
    let submission = match receiver.try_recv().unwrap() {
        Command::Export(s) => s,
        _ => panic!("wrong native command"),
    };
    app.shutdown();
    assert!(!app.exporter.is_open());
    assert!(app.exporter.pending().is_none());
    assert_eq!(app.exporter.notice(), crate::exporter::Notice::MayRemain);
    let _ = app.receive(Event::Exported {
        receipt: submission.receipt.clone(),
        outcome: crate::exporter::Outcome::Saved { bytes: 512 },
    });
    assert_eq!(app.exporter.notice(), crate::exporter::Notice::MayRemain);
}
#[test]
fn export_generation_change_invalidates_confirm_before_dispatch() {
    let (mut app, mut receiver) = managed_app();
    let ticket = open_export(&mut app);
    let review = review_export(&mut app, ticket);
    app.generation += 1;
    let _ = app.update(Message::Export(crate::exporter::Action::Confirm(review)));
    assert!(receiver.try_recv().is_err());
    assert!(app.exporter.pending().is_none());
}
