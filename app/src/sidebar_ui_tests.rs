//! Production App sidebar actions and projection, with synthetic metadata and no Host.
use super::*;
use sidebar_organization::{Entry, Location, Mutation};
use sidebar_ui::Action;

#[test]
fn registry_projection_changes_invalidate_old_ordering_menus_and_unknown_registry_disables_order() {
    let (mut app, mut queue) = app();
    open(&mut app, "b");
    let stamp = app.sidebar_organization.stamp().unwrap();
    let _ = app.receive(Event::Workspace(WorkspaceFollowFrame::Archived {
        archived_session_ids: vec![id("b")],
    }));
    assert!(!app.sidebar_organization.is_open());
    let _ = app.update(Message::Sidebar(Action::Step(stamp, true)));
    assert!(!app.sidebar_organization.busy());
    open(&mut app, "a");
    let stamp = app.sidebar_organization.stamp().unwrap();
    let _ = app.receive(Event::Workspace(WorkspaceFollowFrame::Pinned {
        pinned_session_ids: vec![id("a")],
    }));
    assert!(!app.sidebar_organization.is_open());
    let _ = app.update(Message::Sidebar(Action::Step(stamp, false)));
    app.management.disconnect();
    assert!(app.sidebar_step_order(&id("b"), false).is_none());
    assert!(!app.sidebar_organization.busy());
    assert!(queue.try_recv().is_err());
}

#[test]
fn row_export_stays_bound_to_unselected_target_through_review_and_confirm() {
    use crate::exporter::Action as Export;
    let (mut app, mut queue) = app();
    app.sidebar_sheet = true;
    open(&mut app, "b");
    assert!(!app.sidebar_sheet);
    let stamp = app.sidebar_organization.stamp().unwrap();
    let _ = app.update(Message::Sidebar(Action::Export(stamp.clone())));
    assert!(!app.sidebar_organization.is_open());
    assert_eq!(app.exporter.ticket().unwrap().target, id("b"));
    let _ = app.update(Message::Sidebar(Action::Export(stamp)));
    assert!(queue.try_recv().is_err());
    let ticket = app.exporter.ticket().unwrap();
    let _ = app.update(Message::Export(Export::Edit {
        ticket,
        path: "/PUBLIC-output/row-b.zip".into(),
    }));
    let ticket = app.exporter.ticket().unwrap();
    let _ = app.update(Message::Export(Export::Review(ticket)));
    let review = app.exporter.review().cloned().unwrap();
    let _ = app.update(Message::Export(Export::Confirm(review)));
    let Command::Export(submission) = queue.try_recv().unwrap() else {
        panic!("only explicit export expected")
    };
    assert_eq!(submission.receipt.ticket.target, id("b"));
    assert_eq!(app.selected, Some(id("a")));
    assert!(queue.try_recv().is_err());
}

#[test]
fn reorder_matches_projected_membership_even_with_duplicate_accounting_and_local_overrides() {
    let (mut app, mut queue) = app();
    app.workspaces[1].session_ids.push(id("b"));
    app.workspace = Some(workspace("w2"));
    assert_eq!(
        app.sidebar_step_order(&id("b"), true),
        Some(vec![id("c"), id("b")])
    );
    open(&mut app, "a");
    commit(
        &mut app,
        Mutation::Move(Location::Workspace(workspace("w2"))),
    );
    app.sidebar_organization.dismiss();
    assert_eq!(
        app.sidebar_step_order(&id("b"), false),
        Some(vec![id("b"), id("a"), id("c")])
    );
    assert!(queue.try_recv().is_err());
}

#[test]
fn all_conversations_order_crosses_groups_but_workspace_order_does_not() {
    let (mut app, mut queue) = app();
    assert_eq!(
        app.sidebar_step_order(&id("b"), true),
        Some(vec![id("a"), id("c"), id("b")])
    );
    app.workspace = Some(workspace("w1"));
    assert_eq!(
        app.sidebar_step_order(&id("b"), false),
        Some(vec![id("b"), id("a")])
    );
    assert!(app.sidebar_step_order(&id("b"), true).is_none());
    app.management.pins(vec![id("b")]);
    assert!(app.sidebar_step_order(&id("b"), false).is_none());
    assert!(queue.try_recv().is_err());
}

#[tokio::test]
async fn real_sidebar_io_commits_restarts_and_rejects_a_stale_second_writer_without_touching_drafts()
 {
    use std::{
        os::unix::fs::PermissionsExt,
        sync::{Arc, Mutex},
    };
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let home = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("evidence")
        .join(format!("PUBLIC-sidebar-ui-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&home).unwrap();
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
    let drafts = draft_store::Drafts::from([(id("a"), "PUBLIC unsent text".into())]);
    draft_store::Store::new(home.clone())
        .unwrap()
        .save(&drafts)
        .unwrap();
    let (mut app, mut queue) = app();
    app.sidebar_store = Some(Arc::new(Mutex::new(
        draft_store::Store::sidebar(home.clone()).unwrap(),
    )));
    open(&mut app, "b");
    let stamp = app.sidebar_organization.stamp().unwrap();
    let write = app
        .sidebar_organization
        .propose(&stamp, Mutation::Move(Location::Workspace(workspace("w2"))))
        .unwrap();
    let (ticket, result) = sidebar_ui::save_sidebar(write, app.sidebar_store.clone()).await;
    assert!(result.is_ok());
    let _ = app.update(Message::SidebarSaved { ticket, result });
    let disk = app
        .sidebar_store
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .load()
        .unwrap();
    let mut restarted = sidebar_organization::State::default();
    let ticket = restarted.start_load(false).unwrap();
    restarted.loaded(ticket, Ok(disk.clone()));
    assert_eq!(
        restarted.location(&id("b")),
        Location::Workspace(workspace("w2"))
    );
    let stamp = app.sidebar_organization.stamp().unwrap();
    let write = app
        .sidebar_organization
        .propose(&stamp, Mutation::Move(Location::Ungrouped))
        .unwrap();
    let mut external = sidebar_organization::decode(&disk).unwrap();
    external.insert(
        id("c"),
        Entry {
            location: Location::Ungrouped,
            rank: None,
            trashed: true,
        },
    );
    draft_store::Store::sidebar(home.clone())
        .unwrap()
        .save_checked(&sidebar_organization::encode(&external).unwrap(), &disk)
        .unwrap();
    let (ticket, result) = sidebar_ui::save_sidebar(write, app.sidebar_store.clone()).await;
    assert_eq!(result, Err(draft_store::Failure::Conflict));
    let _ = app.update(Message::SidebarSaved { ticket, result });
    assert_eq!(
        app.sidebar_organization.location(&id("b")),
        Location::Workspace(workspace("w2"))
    );
    assert_eq!(
        app.sidebar_organization.failure(),
        Some(draft_store::Failure::Conflict)
    );
    assert_eq!(
        draft_store::Store::new(home).unwrap().load().unwrap(),
        drafts
    );
    assert!(queue.try_recv().is_err());
}

fn id(value: &str) -> SessionId {
    SessionId::new(value).unwrap()
}
fn workspace(value: &str) -> WorkspaceId {
    WorkspaceId::new(value).unwrap()
}
fn app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
    let (handle, feed, queue, _, _) = worker::test_channels();
    let options = crate::config::parse(
        [
            "--runtime",
            "/explicit-fork/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/PUBLIC-native-home",
            "--user-home",
            "/PUBLIC-user-home",
            "--cwd",
            "/tmp",
        ]
        .map(str::to_owned),
    )
    .unwrap()
    .unwrap();
    let (mut app, task) = App::boot(options, handle, feed);
    drop(task);
    app.ready = true;
    app.root_ready = true;
    app.roster_pending = false;
    app.transport_epoch = worker::TRANSPORT_EPOCH;
    app.generation = 7;
    app.selected = Some(id("a"));
    app.management.baseline(vec![], vec![]);
    for (name, time) in [("a", 300), ("b", 200), ("c", 100)] {
        app.sessions.insert(
            id(name),
            SessionSummary {
                agent_available: true,
                session_id: id(name),
                updated_at: time,
                running: false,
                blank: false,
                parent_session_id: None,
                origin: None,
                cwd: None,
                projections: None,
            },
        );
    }
    app.workspaces = vec![
        WorkspaceView {
            workspace_id: workspace("w1"),
            path: "/PUBLIC-one".into(),
            title: "One".into(),
            created_at: "PUBLIC".into(),
            updated_at: "PUBLIC".into(),
            session_ids: vec![id("a"), id("b")],
        },
        WorkspaceView {
            workspace_id: workspace("w2"),
            path: "/PUBLIC-two".into(),
            title: "Two".into(),
            created_at: "PUBLIC".into(),
            updated_at: "PUBLIC".into(),
            session_ids: vec![id("c")],
        },
    ];
    let ticket = app.sidebar_organization.start_load(false).unwrap();
    let _ = app.update(Message::SidebarLoaded {
        ticket,
        result: Ok(Default::default()),
    });
    (app, queue)
}
fn open(app: &mut App, target: &str) {
    let message = app.sidebar_open_message(&id(target));
    let _ = app.update(message);
    assert_eq!(app.sidebar_organization.stamp().unwrap().target, id(target));
}
fn commit(app: &mut App, mutation: Mutation) {
    let stamp = app.sidebar_organization.stamp().unwrap();
    let write = app.sidebar_organization.propose(&stamp, mutation).unwrap();
    let _ = app.update(Message::SidebarSaved {
        ticket: write.ticket,
        result: Ok(()),
    });
}
#[test]
fn row_actions_target_unselected_conversation_without_selecting_or_resuming_it() {
    let (mut app, mut queue) = app();
    open(&mut app, "b");
    assert_eq!(app.selected, Some(id("a")));
    assert!(!app.composer_visible());
    let stamp = app.sidebar_organization.stamp().unwrap();
    let _ = app.update(Message::Sidebar(Action::Registry(
        stamp,
        crate::management::Operation::Archive,
    )));
    assert_eq!(app.management.target(), Some(&id("b")));
    let ticket = app.management.ticket().unwrap();
    let _ = app.update(Message::Management(crate::management::Action::Confirm(
        ticket,
    )));
    let Command::Manage(submission) = queue.try_recv().unwrap() else {
        panic!("only an explicit management command expected")
    };
    assert_eq!(submission.ticket.target, id("b"));
    assert_eq!(submission.operation, crate::management::Operation::Archive);
    assert_eq!(app.selected, Some(id("a")));
    assert!(queue.try_recv().is_err());
}
#[test]
fn stale_row_targets_epochs_generations_and_dimmed_background_are_fenced() {
    let (mut app, mut queue) = app();
    let stale = app.sidebar_open_message(&id("b"));
    app.generation += 1;
    let _ = app.update(stale);
    assert!(!app.sidebar_organization.is_open());
    open(&mut app, "b");
    let stamp = app.sidebar_organization.stamp().unwrap();
    let _ = app.update(Message::Select(id("c")));
    let _ = app.update(Message::Workspace(Some(workspace("w2"))));
    let _ = app.update(Message::Send);
    let _ = app.update(Message::Settings(crate::settings::Action::Open));
    assert_eq!(app.selected, Some(id("a")));
    assert!(app.workspace.is_none());
    assert!(!app.settings.is_open());
    assert!(queue.try_recv().is_err());
    app.transport_epoch += 1;
    let _ = app.update(Message::Sidebar(Action::Registry(
        stamp,
        crate::management::Operation::Archive,
    )));
    assert!(!app.management.is_open());
    assert!(queue.try_recv().is_err());
}
#[test]
fn every_destination_the_store_would_refuse_is_drawn_as_the_current_group() {
    let (mut app, mut queue) = app();
    open(&mut app, "b");
    // "b" starts in its original workspace: that destination is current, the others are not.
    assert!(app.sidebar_destination_current(&id("b"), &Location::Original));
    assert!(!app.sidebar_destination_current(&id("b"), &Location::Ungrouped));
    assert!(!app.sidebar_destination_current(&id("b"), &Location::Workspace(workspace("w2"))));
    // Its original workspace IS w1 through the roster, so naming w1 leaves it in the group it is
    // already drawn in: the row must read Current rather than offering a press that only moves the
    // marker from "Original workspace" to "w1".
    assert!(app.sidebar_destination_current(&id("b"), &Location::Workspace(workspace("w1"))));
    // The store refuses that move, which is why the drawn button could only be inert.
    let stamp = app.sidebar_organization.stamp().unwrap();
    assert!(
        app.sidebar_organization
            .propose(&stamp, Mutation::Move(Location::Original))
            .is_none()
    );
    // After a real move the predicate follows the committed location, so the row that just became
    // current stops being pressable and the old one becomes pressable again.
    commit(
        &mut app,
        Mutation::Move(Location::Workspace(workspace("w2"))),
    );
    assert!(app.sidebar_destination_current(&id("b"), &Location::Workspace(workspace("w2"))));
    assert!(!app.sidebar_destination_current(&id("b"), &Location::Original));
    assert!(!app.sidebar_destination_current(&id("b"), &Location::Ungrouped));
    assert!(queue.try_recv().is_err());
}
#[test]
fn moved_group_is_local_and_empty_backend_membership_does_not_hide_overrides() {
    let (mut app, mut queue) = app();
    open(&mut app, "b");
    commit(
        &mut app,
        Mutation::Move(Location::Workspace(workspace("w2"))),
    );
    app.sidebar_organization.dismiss();
    app.workspace = Some(workspace("w1"));
    assert_eq!(
        app.navigation_sessions()
            .iter()
            .map(|s| s.session_id.clone())
            .collect::<Vec<_>>(),
        vec![id("a")]
    );
    app.workspaces[1].session_ids.clear();
    app.workspace = Some(workspace("w2"));
    assert_eq!(
        app.navigation_sessions()
            .iter()
            .map(|s| s.session_id.clone())
            .collect::<Vec<_>>(),
        vec![id("b")]
    );
    assert!(app.workspaces[0].session_ids.contains(&id("b")));
    assert_eq!(app.workspaces[0].path, "/PUBLIC-one");
    assert!(queue.try_recv().is_err());
}
#[test]
fn trash_requires_review_retains_group_and_order_and_does_not_archive_or_cancel() {
    let (mut app, mut queue) = app();
    open(&mut app, "b");
    commit(&mut app, Mutation::Reorder(vec![id("b"), id("a")]));
    let stamp = app.sidebar_organization.stamp().unwrap();
    assert!(
        app.sidebar_organization
            .propose(&stamp, Mutation::Trash)
            .is_none()
    );
    let _ = app.update(Message::Sidebar(Action::ReviewTrash(stamp)));
    commit(&mut app, Mutation::Trash);
    app.sidebar_organization.dismiss();
    assert!(
        !app.navigation_sessions()
            .iter()
            .any(|s| s.session_id == id("b"))
    );
    assert!(!app.management.registry.archived(&id("b")));
    assert_eq!(app.sidebar_organization.rank(&id("b")), Some(0));
    app.show_trash = true;
    app.workspace = Some(workspace("w2"));
    assert_eq!(
        app.navigation_sessions()
            .iter()
            .map(|s| s.session_id.clone())
            .collect::<Vec<_>>(),
        vec![id("b")]
    );
    open(&mut app, "b");
    commit(&mut app, Mutation::Restore);
    app.sidebar_organization.dismiss();
    app.show_trash = false;
    app.workspace = Some(workspace("w1"));
    assert_eq!(
        app.navigation_sessions()
            .iter()
            .map(|s| s.session_id.clone())
            .collect::<Vec<_>>(),
        vec![id("b"), id("a")]
    );
    assert!(queue.try_recv().is_err());
}
#[test]
fn trashing_current_running_session_hides_composer_but_preserves_stop_and_history() {
    let (mut app, mut queue) = app();
    app.sessions.get_mut(&id("a")).unwrap().running = true;
    open(&mut app, "a");
    let stamp = app.sidebar_organization.stamp().unwrap();
    app.sidebar_organization.review(&stamp);
    commit(&mut app, Mutation::Trash);
    app.sidebar_organization.dismiss();
    assert_eq!(app.selected, Some(id("a")));
    assert!(!app.composer_visible());
    assert!(app.panel_stop_visible());
    assert!(!app.send_allowed());
    let _ = app.view();
    assert!(queue.try_recv().is_err());
}
#[test]
fn persisted_metadata_is_not_pruned_by_roster_or_overwritten_by_workspace_follow() {
    let (mut app, mut queue) = app();
    let mut data = sidebar_organization::Data::new();
    data.insert(
        id("cold-not-loaded"),
        Entry {
            location: Location::Workspace(workspace("w2")),
            rank: Some(9),
            trashed: true,
        },
    );
    data.insert(
        id("b"),
        Entry {
            location: Location::Workspace(workspace("w2")),
            rank: Some(1),
            trashed: false,
        },
    );
    let ticket = app.sidebar_organization.start_load(true).unwrap();
    let _ = app.update(Message::SidebarLoaded {
        ticket,
        result: Ok(sidebar_organization::encode(&data).unwrap()),
    });
    let _ = app.receive(Event::Workspace(WorkspaceFollowFrame::Upsert {
        workspace: app.workspaces[1].clone(),
    }));
    assert!(app.sidebar_organization.trashed(&id("cold-not-loaded")));
    assert_eq!(app.sidebar_group(&id("b")), Some(workspace("w2")));
    assert!(queue.try_recv().is_err());
}
#[test]
fn sidebar_pending_io_is_joined_before_exit_and_close_cannot_rollback_it() {
    let (mut app, mut queue) = app();
    open(&mut app, "b");
    let stamp = app.sidebar_organization.stamp().unwrap();
    let write = app
        .sidebar_organization
        .propose(&stamp, Mutation::Move(Location::Ungrouped))
        .unwrap();
    let _ = app.update(Message::DismissPanel);
    assert!(!app.sidebar_organization.is_open());
    assert!(app.sidebar_organization.busy());
    app.draft_exit_requested = true;
    drop(app.finish_draft_exit());
    assert!(app.sidebar_organization.busy());
    let _ = app.update(Message::SidebarSaved {
        ticket: write.ticket,
        result: Err(draft_store::Failure::Indeterminate),
    });
    assert!(!app.sidebar_organization.busy());
    assert!(matches!(app.close, Close::Failed(_)));
    assert_eq!(
        app.sidebar_organization.location(&id("b")),
        Location::Original
    );
    assert!(queue.try_recv().is_err());
}

fn with_title(app: &mut App, name: &str, title: &str) {
    app.sessions.get_mut(&id(name)).unwrap().projections = Some(ProjectionHints {
        kind: ProjectionKind::Cached,
        as_of_seq: 3,
        values: [("title".to_string(), serde_json::json!(title))]
            .into_iter()
            .collect(),
    });
}
fn open_rename(app: &mut App, target: &str) -> crate::management::Ticket {
    let message = app.sidebar_open_message(&id(target));
    let _ = app.update(message);
    let stamp = app.sidebar_organization.stamp().unwrap();
    let _ = app.update(Message::Sidebar(Action::Rename(stamp)));
    assert!(!app.sidebar_organization.is_open());
    assert!(app.rename.is_open());
    app.rename.ticket().unwrap()
}
fn confirm_rename(
    app: &mut App,
    queue: &mut tokio::sync::mpsc::Receiver<Command>,
    title: &str,
) -> Command {
    let ticket = app.rename.ticket().unwrap();
    let _ = app.update(Message::Rename(crate::rename::Action::Edit {
        ticket: ticket.clone(),
        title: title.into(),
    }));
    let _ = app.update(Message::Rename(crate::rename::Action::Review(ticket)));
    let review = app.rename.review().expect("reviewed title").clone();
    let _ = app.update(Message::Rename(crate::rename::Action::Confirm(review)));
    queue.try_recv().expect("explicit rename command")
}
fn title_of(app: &App, name: &str) -> String {
    app.session_title(app.sessions.get(&id(name)).unwrap())
}
fn rename_ack(submission: crate::rename::Submission, title: &str, seq: i64) -> Event {
    Event::Renamed {
        submission,
        result: Ok(SessionRenameValue {
            title: title.into(),
            seq,
        }),
    }
}

#[test]
fn rename_row_action_prefills_the_visible_title_for_the_unselected_target_without_writing() {
    let (mut app, mut queue) = app();
    with_title(&mut app, "b", "PUBLIC row title");
    let ticket = open_rename(&mut app, "b");
    assert_eq!(ticket.target, id("b"));
    assert_eq!(app.selected, Some(id("a")));
    assert_eq!(app.rename.title(), "PUBLIC row title");
    assert_eq!(app.rename.notice(), &crate::rename::Notice::None);
    assert!(queue.try_recv().is_err());
}

#[test]
fn rename_local_validation_refuses_without_closing_the_editor_or_losing_the_typed_text() {
    let (mut app, mut queue) = app();
    with_title(&mut app, "b", "PUBLIC row title");
    let ticket = open_rename(&mut app, "b");
    let _ = app.update(Message::Rename(crate::rename::Action::Edit {
        ticket: ticket.clone(),
        title: "   ".into(),
    }));
    assert_eq!(
        app.rename.notice(),
        &crate::rename::Notice::Invalid(crate::rename::Invalid::Empty)
    );
    let _ = app.update(Message::Rename(crate::rename::Action::Review(
        ticket.clone(),
    )));
    assert!(app.rename.review().is_none());
    assert!(app.rename.is_open());
    assert_eq!(app.rename.title(), "   ");
    assert_eq!(title_of(&app, "b"), "PUBLIC row title");
    assert!(queue.try_recv().is_err());
    // Close leaves the previous title untouched and sends nothing.
    let _ = app.update(Message::Rename(crate::rename::Action::Close(ticket)));
    assert!(!app.rename.is_open());
    assert_eq!(title_of(&app, "b"), "PUBLIC row title");
    assert!(queue.try_recv().is_err());
}

#[test]
fn rename_ack_applies_only_the_acknowledged_title_and_never_becomes_a_chat_message() {
    let (mut app, mut queue) = app();
    with_title(&mut app, "b", "PUBLIC row title");
    let _ = open_rename(&mut app, "b");
    let before_rows = app.transcript.rows().len();
    let command = confirm_rename(&mut app, &mut queue, "PUBLIC typed title");
    let Command::Rename(submission) = command else {
        panic!("only an explicit rename command expected")
    };
    assert_eq!(submission.receipt.ticket.target, id("b"));
    assert_eq!(submission.title(), "PUBLIC typed title");
    // No optimistic rename: the sidebar still shows the old title until the ACK.
    assert_eq!(title_of(&app, "b"), "PUBLIC row title");
    assert_eq!(app.rename.notice(), &crate::rename::Notice::Pending);
    assert!(queue.try_recv().is_err());
    let _ = app.receive(rename_ack(submission, "PUBLIC normalized title", 9));
    assert_eq!(title_of(&app, "b"), "PUBLIC normalized title");
    assert_eq!(title_of(&app, "a"), "Untitled chat");
    assert_eq!(app.transcript.rows().len(), before_rows);
    assert!(
        !app.transcript
            .display_rows(true)
            .any(|row| row.text.contains("PUBLIC normalized title"))
    );
    assert_eq!(
        app.rename.notice(),
        &crate::rename::Notice::Renamed {
            title: "PUBLIC normalized title".into()
        }
    );
    assert!(queue.try_recv().is_err());
}

#[test]
fn rename_stale_target_epoch_generation_or_foreign_receipt_cannot_change_the_title() {
    let (mut app, mut queue) = app();
    with_title(&mut app, "b", "PUBLIC row title");
    let ticket = open_rename(&mut app, "b");
    // A bumped generation fences further edits from the still-open editor.
    app.generation += 1;
    let _ = app.update(Message::Rename(crate::rename::Action::Edit {
        ticket: ticket.clone(),
        title: "PUBLIC forged title".into(),
    }));
    assert_eq!(app.rename.title(), "PUBLIC row title");
    // A bumped transport epoch fences it too.
    app.generation -= 1;
    app.transport_epoch += 1;
    let _ = app.update(Message::Rename(crate::rename::Action::Edit {
        ticket: ticket.clone(),
        title: "PUBLIC forged title".into(),
    }));
    assert_eq!(app.rename.title(), "PUBLIC row title");
    app.transport_epoch -= 1;
    let command = confirm_rename(&mut app, &mut queue, "PUBLIC typed title");
    let Command::Rename(submission) = command else {
        panic!("only an explicit rename command expected")
    };
    // A foreign receipt (wrong attempt) cannot install a title.
    let mut foreign = submission.clone();
    foreign.receipt.attempt += 1;
    let _ = app.receive(rename_ack(foreign, "PUBLIC foreign title", 9));
    assert_eq!(title_of(&app, "b"), "PUBLIC row title");
    assert_eq!(app.rename.notice(), &crate::rename::Notice::Pending);
    let _ = app.receive(rename_ack(submission, "PUBLIC normalized title", 9));
    assert_eq!(title_of(&app, "b"), "PUBLIC normalized title");
    assert!(queue.try_recv().is_err());
}

#[test]
fn rename_stale_stamp_epoch_cannot_open_the_editor() {
    let (mut app, mut queue) = app();
    with_title(&mut app, "b", "PUBLIC row title");
    open(&mut app, "b");
    let stamp = app.sidebar_organization.stamp().unwrap();
    app.transport_epoch += 1;
    let _ = app.update(Message::Sidebar(Action::Rename(stamp)));
    assert!(!app.rename.is_open());
    assert!(queue.try_recv().is_err());
}

#[test]
fn rename_unknown_outcome_is_surfaced_without_retry_or_a_title_guess() {
    let (mut app, mut queue) = app();
    with_title(&mut app, "b", "PUBLIC row title");
    let _ = open_rename(&mut app, "b");
    let Command::Rename(submission) = confirm_rename(&mut app, &mut queue, "PUBLIC typed title")
    else {
        panic!("only an explicit rename command expected")
    };
    let _ = app.receive(Event::Renamed {
        submission: submission.clone(),
        result: Err(()),
    });
    assert_eq!(app.rename.notice(), &crate::rename::Notice::Indeterminate);
    assert!(app.rename.pending().is_none());
    assert_eq!(title_of(&app, "b"), "PUBLIC row title");
    assert_eq!(app.rename.title(), "PUBLIC typed title");
    // No automatic retry: nothing was re-sent and the consumed review is not re-armed.
    assert!(queue.try_recv().is_err());
    let _ = app.receive(rename_ack(submission, "PUBLIC late title", 9));
    assert_eq!(title_of(&app, "b"), "PUBLIC row title");
    assert!(queue.try_recv().is_err());
}

fn open_fork(app: &mut App, target: &str) -> crate::management::Ticket {
    let message = app.sidebar_open_message(&id(target));
    let _ = app.update(message);
    let stamp = app.sidebar_organization.stamp().unwrap();
    let _ = app.update(Message::Sidebar(Action::Fork(stamp)));
    assert!(!app.sidebar_organization.is_open());
    assert!(app.fork.is_open());
    app.fork.ticket().unwrap()
}
fn confirm_fork(app: &mut App, queue: &mut tokio::sync::mpsc::Receiver<Command>) -> Command {
    let ticket = app.fork.ticket().unwrap();
    let _ = app.update(Message::Fork(crate::fork::Action::Review(ticket)));
    let review = app.fork.review().expect("reviewed fork").clone();
    let _ = app.update(Message::Fork(crate::fork::Action::Confirm(review)));
    queue.try_recv().expect("explicit fork command")
}
fn fork_ack(submission: crate::fork::Submission, child: &str) -> Event {
    Event::Forked {
        submission,
        result: Ok(SessionForkValue {
            session_id: id(child),
        }),
    }
}

#[test]
fn fork_row_action_requires_review_and_cancel_changes_nothing() {
    let (mut app, mut queue) = app();
    let ticket = open_fork(&mut app, "b");
    assert_eq!(ticket.target, id("b"));
    assert_eq!(app.selected, Some(id("a")));
    assert_eq!(app.fork.notice(), &crate::fork::Notice::None);
    // Consent: Confirm is refused until this exact panel recorded a review.
    let forged = crate::fork::Review {
        ticket: ticket.clone(),
        cut: None,
    };
    let _ = app.update(Message::Fork(crate::fork::Action::Confirm(forged)));
    assert!(app.fork.review().is_none());
    assert!(app.fork.pending().is_none());
    assert!(queue.try_recv().is_err());
    // Cancel closes the panel and writes nothing.
    let _ = app.update(Message::Fork(crate::fork::Action::Close(ticket)));
    assert!(!app.fork.is_open());
    assert_eq!(app.selected, Some(id("a")));
    assert!(!app.sessions.contains_key(&id("PUBLIC-fork-child")));
    assert!(queue.try_recv().is_err());
}

#[test]
fn fork_ack_reveals_only_the_acknowledged_child_and_never_fabricates_a_roster_entry() {
    let (mut app, mut queue) = app();
    let before_rows = app.transcript.rows().len();
    let _ = open_fork(&mut app, "b");
    let Command::Fork(submission) = confirm_fork(&mut app, &mut queue) else {
        panic!("only an explicit fork command expected")
    };
    assert_eq!(submission.receipt.ticket.target, id("b"));
    assert_eq!(submission.source(), &id("b"));
    assert_eq!(app.fork.notice(), &crate::fork::Notice::Pending);
    // No optimistic child: the ACK id is not known before it arrives.
    assert!(!app.sessions.contains_key(&id("PUBLIC-fork-child")));
    assert!(queue.try_recv().is_err());
    let _ = app.receive(fork_ack(submission, "PUBLIC-fork-child"));
    // The exact acknowledged id is reported, but no roster row and no selection are invented.
    assert_eq!(
        app.fork.notice(),
        &crate::fork::Notice::Forked {
            child: id("PUBLIC-fork-child"),
            selected: false
        }
    );
    assert!(!app.sessions.contains_key(&id("PUBLIC-fork-child")));
    assert_eq!(app.selected, Some(id("a")));
    assert_eq!(app.transcript.rows().len(), before_rows);
    assert!(
        !app.transcript
            .display_rows(true)
            .any(|row| row.text.contains("PUBLIC-fork-child"))
    );
    assert!(queue.try_recv().is_err());
}

#[test]
fn fork_ack_selects_a_child_only_when_the_app_frames_already_contain_it() {
    let (mut app, mut queue) = app();
    app.sessions.insert(
        id("PUBLIC-fork-child"),
        SessionSummary {
            agent_available: true,
            session_id: id("PUBLIC-fork-child"),
            updated_at: 400,
            running: false,
            blank: true,
            parent_session_id: None,
            origin: None,
            cwd: None,
            projections: None,
        },
    );
    let _ = open_fork(&mut app, "b");
    let Command::Fork(submission) = confirm_fork(&mut app, &mut queue) else {
        panic!("only an explicit fork command expected")
    };
    let _ = app.receive(fork_ack(submission, "PUBLIC-fork-child"));
    assert_eq!(app.selected, Some(id("PUBLIC-fork-child")));
    assert_eq!(
        app.fork.notice(),
        &crate::fork::Notice::Forked {
            child: id("PUBLIC-fork-child"),
            selected: true
        }
    );
}

#[test]
fn fork_stale_target_epoch_generation_or_foreign_receipt_cannot_reveal_a_child() {
    let (mut app, mut queue) = app();
    let ticket = open_fork(&mut app, "b");
    app.generation += 1;
    let _ = app.update(Message::Fork(crate::fork::Action::Review(ticket.clone())));
    assert!(app.fork.review().is_none());
    app.generation -= 1;
    app.transport_epoch += 1;
    let _ = app.update(Message::Fork(crate::fork::Action::Review(ticket)));
    assert!(app.fork.review().is_none());
    app.transport_epoch -= 1;
    let Command::Fork(submission) = confirm_fork(&mut app, &mut queue) else {
        panic!("only an explicit fork command expected")
    };
    // A foreign receipt (wrong attempt) cannot reveal a child or retire the flight.
    let mut foreign = submission.clone();
    foreign.receipt.attempt += 1;
    let _ = app.receive(fork_ack(foreign, "PUBLIC-foreign-child"));
    assert_eq!(app.selected, Some(id("a")));
    assert!(!app.sessions.contains_key(&id("PUBLIC-foreign-child")));
    assert_eq!(app.fork.notice(), &crate::fork::Notice::Pending);
    let _ = app.receive(fork_ack(submission, "PUBLIC-fork-child"));
    assert_eq!(app.selected, Some(id("a")));
    assert!(!app.sessions.contains_key(&id("PUBLIC-fork-child")));
    assert!(queue.try_recv().is_err());
}

#[test]
fn fork_unavailable_is_definite_but_unknown_and_unattached_stay_indeterminate() {
    for (failure, expected) in [
        (
            crate::fork::Failure::Unavailable,
            crate::fork::Notice::Unavailable,
        ),
        (
            crate::fork::Failure::Unattached,
            crate::fork::Notice::Unattached,
        ),
        (
            crate::fork::Failure::Unknown,
            crate::fork::Notice::Indeterminate,
        ),
    ] {
        let (mut app, mut queue) = app();
        let _ = open_fork(&mut app, "b");
        let Command::Fork(submission) = confirm_fork(&mut app, &mut queue) else {
            panic!("only an explicit fork command expected")
        };
        let _ = app.receive(Event::Forked {
            submission: submission.clone(),
            result: Err(failure),
        });
        assert_eq!(app.fork.notice(), &expected);
        assert!(app.fork.pending().is_none());
        assert_eq!(app.selected, Some(id("a")));
        assert!(!app.sessions.contains_key(&id("PUBLIC-fork-child")));
        // No automatic retry, and a late possessive ACK stays fenced.
        assert!(queue.try_recv().is_err());
        let _ = app.receive(fork_ack(submission, "PUBLIC-late-child"));
        assert_eq!(app.selected, Some(id("a")));
        assert!(!app.sessions.contains_key(&id("PUBLIC-late-child")));
    }
}

#[test]
fn fork_in_flight_shares_the_single_management_rename_and_export_flight() {
    let (mut app, mut queue) = app();
    let _ = open_fork(&mut app, "b");
    let Command::Fork(_) = confirm_fork(&mut app, &mut queue) else {
        panic!("only an explicit fork command expected")
    };
    assert!(app.fork.pending().is_some());
    // A second mutation for any row is refused while the fork flight is unresolved.
    assert!(!app.sidebar_target_allowed(&id("b")));
    assert!(!app.sidebar_target_allowed(&id("a")));
    assert!(!app.management_context_for(Some(&id("b"))).allowed);
    assert!(!app.rename_context_for(Some(&id("b"))).allowed);
    assert!(!app.export_context_for(Some(&id("b"))).allowed);
    // The fork panel's own state machine refuses a second admission while pending.
    assert!(app.fork.pending().is_some());
    let ticket = app.fork.ticket().unwrap();
    let context = app.fork_context();
    assert!(
        app.fork
            .apply(crate::fork::Action::Review(ticket), &context)
            .is_none()
    );
    assert!(app.fork.pending().is_some());
    app.sidebar_organization
        .open(app.transport_epoch, app.generation, id("a"));
    let stamp = app.sidebar_organization.stamp().unwrap();
    let _ = app.update(Message::Sidebar(Action::Rename(stamp.clone())));
    let _ = app.update(Message::Sidebar(Action::Fork(stamp.clone())));
    let _ = app.update(Message::Sidebar(Action::Export(stamp.clone())));
    let _ = app.update(Message::Sidebar(Action::Registry(
        stamp,
        crate::management::Operation::Archive,
    )));
    assert!(app.management.ticket().is_none());
    assert!(!app.rename.is_open());
    assert!(app.exporter.ticket().is_none());
    assert!(queue.try_recv().is_err());
}

/// A new conversation is offered only where its press would be accepted: the command fence
/// drops `Message::NewSession` while observations are lost or a submission is in flight, so the
/// three controls that draw the action must agree with it.
#[test]
fn new_conversation_is_not_offered_where_its_press_is_discarded() {
    use dsh_native_transport::dto::{PromptContentPart, PromptMode};
    let (mut app, _queue) = app();
    assert!(app.can_create());
    app.observations_lost = true;
    assert!(!app.can_create(), "lost observations refuse the press");
    app.observations_lost = false;
    let session = id("a");
    assert!(app.pending.insert(
        7,
        crate::worker::prompt_recovery::Pending {
            session: session.clone(),
            text: "PUBLIC frozen submission".into(),
            draft_serial: 0,
            epoch: app.transport_epoch,
            file_request_id: None,
            file: Vec::new(),
            file_ticket: Vec::new(),
            request: dsh_native_transport::dto::SessionPromptRequest {
                session_id: session,
                request_id:
                    dsh_native_transport::dto::SessionRequestId::new("PUBLIC-request").unwrap(),
                mode: PromptMode::Queue,
                content: vec![PromptContentPart::Text {
                    text: "PUBLIC frozen submission".into(),
                }],
                client_time_zone: Some("UTC".into()),
            },
        }
    ));
    assert!(
        !app.can_create(),
        "an in-flight submission refuses the press"
    );
    assert!(app.pending.is_empty() || !app.can_create());
}

#[test]
fn a_session_row_marks_only_its_own_active_scheduled_tasks() {
    use dsh_native_transport::dto::ScheduleCatalogItem;
    let (mut app, _queue) = app();
    let item = |schedule: &str, session: &str, status: &str| ScheduleCatalogItem {
        id: schedule.into(),
        title: schedule.into(),
        kind: "daily".into(),
        session_id: id(session),
        status: status.into(),
        scheduled_at: None,
        last_delivery: None,
        record: serde_json::Map::new(),
    };
    let row = app
        .sessions
        .get(&id("a"))
        .cloned()
        .expect("the fixture's row");
    // A catalog read that has not settled yet advertises nothing, so no row flashes a mark.
    assert!(app.session_schedule_mark(&row, None).is_none());
    app.schedules = vec![
        item("PUBLIC-mark-one", "a", "active"),
        item("PUBLIC-mark-paused", "a", "inactive"),
        item("PUBLIC-mark-elsewhere", "b", "active"),
    ];
    assert!(
        app.session_schedule_mark(&row, None).is_some(),
        "an active task marks its Session's row"
    );
    let elsewhere = app.sessions.get(&id("b")).cloned().expect("the other row");
    assert!(app.session_schedule_mark(&elsewhere, None).is_some());
    let third = app.sessions.get(&id("c")).cloned().expect("the third row");
    assert!(
        app.session_schedule_mark(&third, None).is_none(),
        "a Session without an active task carries no mark"
    );
    // A refresh republishes over the records it already has, so the mark cannot blink mid-read.
    app.schedules_loading = true;
    assert!(app.session_schedule_mark(&row, None).is_some());
    app.schedules_loading = false;
    // An ended task runs no more and an inactive one never marks.
    app.schedules[0].status = "ended".into();
    assert!(app.session_schedule_mark(&row, None).is_none());
    app.schedules[0].status = "active".into();
    // The reference offers the row's leading cell only while its own primary state is idle
    // (`SessionScheduleMark.tsx:8-12`): live activity, a pending decision or an unread
    // completion keeps the cell for itself and the mark is not mounted.
    let mut running = row.clone();
    running.running = true;
    assert!(app.session_schedule_mark(&running, None).is_none());
    assert!(
        app.session_schedule_mark(&row, Some("Approval requested"))
            .is_none()
    );
    app.completion_unread.insert(id("a"));
    assert!(app.session_schedule_mark(&row, None).is_none());
}

#[test]
fn hovering_a_session_row_opens_its_card_after_the_delay_and_a_grace_close_lets_the_pointer_cross()
{
    let (mut app, _queue) = app();
    let session = id("a");
    // Crossing the row arms the dwell; nothing is open yet.
    drop(app.update(Message::SessionRowHover {
        session: session.clone(),
        entered: true,
    }));
    assert_eq!(app.session_hover.as_ref(), Some(&session));
    assert!(
        app.session_hover_open.is_none(),
        "a crossing cursor does not flash a preview"
    );
    // The 800ms dwell elapsing while the pointer is still there opens it.
    drop(app.update(Message::SessionRowHoverExpired(session.clone())));
    assert_eq!(app.session_hover_open.as_ref(), Some(&session));
    // Leaving arms the 200ms grace instead of closing at once, so the pointer can cross the gap.
    drop(app.update(Message::SessionRowHover {
        session: session.clone(),
        entered: false,
    }));
    assert_eq!(
        app.session_hover_open.as_ref(),
        Some(&session),
        "the grace keeps the card while the pointer crosses"
    );
    // A pointer resting on the card cancels that close.
    drop(app.update(Message::SessionHoverCard {
        session: session.clone(),
        entered: true,
    }));
    drop(app.update(Message::SessionRowHoverGrace(session.clone())));
    assert_eq!(
        app.session_hover_open.as_ref(),
        Some(&session),
        "resting on the card keeps it"
    );
    // Leaving the card re-arms the grace, and an elapsed grace closes it.
    drop(app.update(Message::SessionHoverCard {
        session: session.clone(),
        entered: false,
    }));
    drop(app.update(Message::SessionRowHoverGrace(session.clone())));
    assert!(app.session_hover_open.is_none());
    // A stale expiry for a row the pointer already left never opens a card.
    drop(app.update(Message::SessionRowHover {
        session: session.clone(),
        entered: true,
    }));
    drop(app.update(Message::SessionRowHover {
        session: session.clone(),
        entered: false,
    }));
    drop(app.update(Message::SessionRowHoverExpired(session.clone())));
    assert!(app.session_hover_open.is_none());
}

#[test]
fn the_hover_card_shows_two_ordered_active_tasks_and_counts_the_rest() {
    use dsh_native_transport::dto::ScheduleCatalogItem;
    let (mut app, _queue) = app();
    let item =
        |schedule: &str, session: &str, status: &str, at: Option<&str>| ScheduleCatalogItem {
            id: schedule.into(),
            title: schedule.into(),
            kind: "daily".into(),
            session_id: id(session),
            status: status.into(),
            scheduled_at: at.map(str::to_owned),
            last_delivery: None,
            record: serde_json::Map::new(),
        };
    app.schedules = vec![
        item(
            "PUBLIC-hover-later",
            "a",
            "active",
            Some("2026-12-02T08:00:00Z"),
        ),
        item(
            "PUBLIC-hover-overdue",
            "a",
            "active",
            Some("2026-10-16T09:00:00Z"),
        ),
        item(
            "PUBLIC-hover-next",
            "a",
            "active",
            Some("2026-12-01T17:30:00Z"),
        ),
        item("PUBLIC-hover-paused", "a", "inactive", None),
        item("PUBLIC-hover-elsewhere", "b", "active", None),
    ];
    let now = super::time_text::parse_instant("2026-10-20T00:00:00Z").expect("a fixed clock");
    let (shown, omitted) =
        crate::reducer::schedule_card::hover_tasks(app.schedules.iter(), &id("a"), now);
    assert_eq!(
        shown
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["PUBLIC-hover-overdue", "PUBLIC-hover-next"],
        "overdue first, then the soonest next run"
    );
    assert_eq!(omitted, 1, "the third active task is counted, not dropped");
    // The inactive task and another Session's task never enter this row's section.
    let (elsewhere, omitted) =
        crate::reducer::schedule_card::hover_tasks(app.schedules.iter(), &id("b"), now);
    assert_eq!(elsewhere.len(), 1);
    assert_eq!(elsewhere[0].id, "PUBLIC-hover-elsewhere");
    assert_eq!(omitted, 0);
}

#[test]
fn the_open_in_app_control_launches_the_selected_workspace_and_reports_a_failure() {
    let (mut app, mut queue) = app();
    app.sessions
        .get_mut(&id("a"))
        .expect("the selected row")
        .cwd = Some("/PUBLIC-workspace".into());
    app.open_in_app_apps = Some(vec!["vscode".into(), "kitty".into(), "unknown".into()]);
    app.open_in_app_choice = Some("kitty".into());
    // The control names the remembered app and offers only the ids the reference can name.
    assert!(app.open_in_app_action().is_some());
    drop(app.update(Message::OpenInAppLaunch("kitty".into())));
    assert!(app.open_in_app_busy);
    let Command::OpenInAppOpen { app: chosen, path } = queue.try_recv().expect("one launch") else {
        panic!("only an explicit launch expected")
    };
    assert_eq!(chosen, "kitty");
    assert_eq!(path, "/PUBLIC-workspace");
    // A second press while the first is in flight is ignored, not queued twice.
    drop(app.update(Message::OpenInAppLaunch("vscode".into())));
    assert!(queue.try_recv().is_err());
    // A failed launch keeps the control usable and states the reference's copy.
    drop(app.update(Message::Worker(Event::OpenInAppLaunched {
        app: "kitty".into(),
        result: Err("Open in app: HttpStatus(502)".into()),
    })));
    assert!(!app.open_in_app_busy);
    assert_eq!(
        app.open_in_app_error.as_deref(),
        Some("Could not open. Try again.")
    );
    // The next launch's receipt clears the failure and remembers the chosen app.
    drop(app.update(Message::OpenInAppLaunch("vscode".into())));
    let Command::OpenInAppOpen { app: chosen, .. } = queue.try_recv().expect("the retry") else {
        panic!("only an explicit launch expected")
    };
    assert_eq!(chosen, "vscode");
    drop(app.update(Message::Worker(Event::OpenInAppLaunched {
        app: "vscode".into(),
        result: Ok(()),
    })));
    assert_eq!(app.open_in_app_choice.as_deref(), Some("vscode"));
    assert_eq!(app.open_in_app_error, None);
    // A failed catalog read hides the control rather than drawing a broken one.
    drop(app.update(Message::Worker(Event::OpenInAppApps(Err(
        "Open in app: HttpStatus(404)".into(),
    )))));
    assert!(app.open_in_app_apps.is_none());
    assert!(app.open_in_app_action().is_none());
}

#[test]
fn the_plugin_manager_reads_once_and_adopts_the_hosts_profile_answer() {
    use super::plugin_manager::Action as Manager;
    let (mut app, mut queue) = app();
    drop(app.update(Message::PluginManager(Manager::Open)));
    assert!(app.plugin_manager.is_open());
    assert!(matches!(queue.try_recv(), Ok(Command::PluginManagerRead)));
    // The same first open also asks for the registry order the form offers, once.
    assert!(matches!(queue.try_recv(), Ok(Command::PluginRegistries)));
    // And once for the live rows the bundles declare.
    assert!(matches!(
        queue.try_recv(),
        Ok(Command::PluginManagerPlugins)
    ));
    // Opening again while the reads have settled queues nothing; Refresh is the explicit read.
    drop(app.update(Message::PluginManager(Manager::Open)));
    assert!(queue.try_recv().is_err());
    // A deployment without a manageable profile is its own answer, not an empty bundle list.
    drop(app.update(Message::Worker(Event::PluginManagerLoaded(Ok((
        false,
        Vec::new(),
    ))))));
    assert_eq!(app.plugin_manager.len(), 0);
    drop(app.update(Message::PluginManager(Manager::Refresh)));
    assert!(matches!(queue.try_recv(), Ok(Command::PluginManagerRead)));
    let bundle = dsh_native_transport::plugin::PluginBundle {
        name: "PUBLIC-bundle".into(),
        version: Some("1.0.0".into()),
        description: None,
        enabled: true,
        installed: true,
        optional: false,
        removable: true,
        rows: Vec::new(),
        overrides: 0,
        read_only_reason: None,
        error: None,
    };
    drop(app.update(Message::Worker(Event::PluginManagerLoaded(Ok((
        true,
        vec![bundle],
    ))))));
    assert_eq!(app.plugin_manager.len(), 1);
    // Escape closes the page like every other full-pane opener.
    drop(app.update(Message::DismissPanel));
    assert!(!app.plugin_manager.is_open());
}

#[test]
fn switching_a_plugin_bundle_needs_confirm_sends_one_write_and_rereads_the_list() {
    use super::plugin_manager::Action as Manager;
    use dsh_native_transport::plugin::{PluginApplication, PluginChange};
    let (mut app, mut queue) = app();
    let bundle = dsh_native_transport::plugin::PluginBundle {
        name: "PUBLIC-bundle".into(),
        version: Some("1.0.0".into()),
        description: None,
        enabled: true,
        installed: true,
        optional: false,
        removable: true,
        rows: Vec::new(),
        overrides: 0,
        read_only_reason: None,
        error: None,
    };
    app.plugin_manager.seed(true, vec![bundle]);
    // Asking arms the review; nothing is sent and no state changes optimistically.
    drop(app.update(Message::PluginManager(Manager::AskBundle {
        name: "PUBLIC-bundle".into(),
        enabled: false,
    })));
    assert!(queue.try_recv().is_err());
    // Cancel leaves no armed review and still sends nothing.
    drop(app.update(Message::PluginManager(Manager::CancelBundle)));
    drop(app.update(Message::PluginManager(Manager::ConfirmBundle)));
    assert!(queue.try_recv().is_err());
    // A confirmed review sends exactly the armed write.
    drop(app.update(Message::PluginManager(Manager::AskBundle {
        name: "PUBLIC-bundle".into(),
        enabled: false,
    })));
    drop(app.update(Message::PluginManager(Manager::ConfirmBundle)));
    let Command::PluginBundleEnabled { name, enabled } = queue.try_recv().expect("one write")
    else {
        panic!("only the bundle switch expected")
    };
    assert_eq!((name.as_str(), enabled), ("PUBLIC-bundle", false));
    // A second Confirm while the first is in flight is ignored.
    drop(app.update(Message::PluginManager(Manager::ConfirmBundle)));
    assert!(queue.try_recv().is_err());
    // The settlement states the restart requirement and re-reads the Host's list once.
    drop(app.update(Message::Worker(Event::PluginBundleChanged {
        name: "PUBLIC-bundle".into(),
        enabled: false,
        result: Ok(PluginChange {
            changed: true,
            application: PluginApplication::RestartRequired,
            target: "PUBLIC-bundle".into(),
            enabled: Some(false),
            error: None,
        }),
    })));
    assert!(matches!(queue.try_recv(), Ok(Command::PluginManagerRead)));
    // The re-read settles before another attempt; the page is then ready for a new review.
    drop(app.update(Message::Worker(Event::PluginManagerLoaded(Ok((
        true,
        Vec::new(),
    ))))));
    // A failed switch keeps the list and states the outcome without a silent retry.
    drop(app.update(Message::Worker(Event::PluginBundleChanged {
        name: "PUBLIC-bundle".into(),
        enabled: true,
        result: Ok(PluginChange {
            changed: false,
            application: PluginApplication::Failed,
            target: "PUBLIC-bundle".into(),
            enabled: None,
            error: Some("operation-error".into()),
        }),
    })));
    assert!(matches!(queue.try_recv(), Ok(Command::PluginManagerRead)));
}

#[test]
fn uninstalling_a_bundle_needs_confirm_and_sends_one_destructive_write() {
    use super::plugin_manager::Action as Manager;
    use dsh_native_transport::plugin::{PluginApplication, PluginChange};
    let (mut app, mut queue) = app();
    let bundle = dsh_native_transport::plugin::PluginBundle {
        name: "PUBLIC-bundle".into(),
        version: Some("1.0.0".into()),
        description: None,
        enabled: false,
        installed: true,
        optional: false,
        removable: true,
        rows: Vec::new(),
        overrides: 0,
        read_only_reason: None,
        error: None,
    };
    app.plugin_manager.seed(true, vec![bundle]);
    // Arming the removal sends nothing.
    drop(app.update(Message::PluginManager(Manager::AskRemove {
        name: "PUBLIC-bundle".into(),
    })));
    assert!(queue.try_recv().is_err());
    // Cancelling sends nothing and leaves nothing armed.
    drop(app.update(Message::PluginManager(Manager::CancelRemove)));
    drop(app.update(Message::PluginManager(Manager::ConfirmRemove)));
    assert!(queue.try_recv().is_err());
    // A confirmed removal sends exactly one destructive write.
    drop(app.update(Message::PluginManager(Manager::AskRemove {
        name: "PUBLIC-bundle".into(),
    })));
    drop(app.update(Message::PluginManager(Manager::ConfirmRemove)));
    let Command::PluginRemove { name } = queue.try_recv().expect("one removal") else {
        panic!("only the bundle removal expected")
    };
    assert_eq!(name, "PUBLIC-bundle");
    // A second Confirm while the first is in flight is ignored.
    drop(app.update(Message::PluginManager(Manager::ConfirmRemove)));
    assert!(queue.try_recv().is_err());
    // The settlement re-reads the Host's list.
    drop(app.update(Message::Worker(Event::PluginRemoved {
        name: "PUBLIC-bundle".into(),
        result: Ok(PluginChange {
            changed: true,
            application: PluginApplication::Applied,
            target: "PUBLIC-bundle".into(),
            enabled: None,
            error: None,
        }),
    })));
    assert!(matches!(queue.try_recv(), Ok(Command::PluginManagerRead)));
}

#[test]
fn inspecting_an_install_spec_sends_one_read_and_shows_the_answer() {
    use super::plugin_manager::Action as Manager;
    use dsh_native_transport::plugin::PluginInspection;
    let (mut app, mut queue) = app();
    app.plugin_manager.seed(true, Vec::new());
    // A blank spec sends no read.
    drop(app.update(Message::PluginManager(Manager::Inspect)));
    assert!(queue.try_recv().is_err());
    // The typed spec is trimmed for the Host and read exactly once.
    drop(app.update(Message::PluginManager(Manager::EditSpec(
        "  @PUBLIC/pkg  ".into(),
    ))));
    drop(app.update(Message::PluginManager(Manager::Inspect)));
    let Command::PluginInspect { spec, registry: _ } = queue.try_recv().expect("one inspect")
    else {
        panic!("only the spec inspection expected")
    };
    assert_eq!(spec, "@PUBLIC/pkg");
    // A second press while the read is in flight sends nothing.
    drop(app.update(Message::PluginManager(Manager::Inspect)));
    assert!(queue.try_recv().is_err());
    // An accepted answer is the preview; nothing is installed by it.
    drop(app.update(Message::Worker(Event::PluginInspected(Ok(
        PluginInspection::Accepted {
            kind: "registry".into(),
            name: Some("@PUBLIC/pkg".into()),
            version: Some("1.0.0".into()),
            description: Some("PUBLIC package".into()),
            bundle: Some(true),
            registry: Some("https://PUBLIC.invalid".into()),
            host: None,
        },
    )))));
    assert!(matches!(
        app.plugin_manager.inspection(),
        Some(PluginInspection::Accepted {
            bundle: Some(true),
            ..
        })
    ));
    // A refusal is a normal answer, and it replaces the preview.
    drop(app.update(Message::PluginManager(Manager::Inspect)));
    assert!(matches!(
        queue.try_recv(),
        Ok(Command::PluginInspect { .. })
    ));
    drop(app.update(Message::Worker(Event::PluginInspected(Ok(
        PluginInspection::Refused {
            problem: "not-found".into(),
            reason: "PUBLIC registry said no".into(),
            registries: 2,
        },
    )))));
    assert!(matches!(
        app.plugin_manager.inspection(),
        Some(PluginInspection::Refused { problem, .. }) if problem == "not-found"
    ));
}

#[test]
fn the_add_plugin_form_reads_the_registry_order_once_and_inspects_with_the_choice() {
    use super::plugin_manager::Action as Manager;
    use dsh_native_transport::plugin::{PluginInspection, PluginRegistries};
    let (mut app, mut queue) = app();
    // Opening reads the manager and, once, the registry order.
    drop(app.update(Message::PluginManager(Manager::Open)));
    assert!(matches!(queue.try_recv(), Ok(Command::PluginManagerRead)));
    assert!(matches!(queue.try_recv(), Ok(Command::PluginRegistries)));
    assert!(matches!(
        queue.try_recv(),
        Ok(Command::PluginManagerPlugins)
    ));
    // Opening again asks for none of the three reads a second time.
    drop(app.update(Message::PluginManager(Manager::Open)));
    assert!(queue.try_recv().is_err());
    drop(app.update(Message::Worker(Event::PluginRegistriesLoaded(Ok(
        PluginRegistries {
            registry: Some("https://first.invalid".into()),
            fallback: vec!["https://second.invalid".into()],
            resolved: Some("https://pnpm.invalid".into()),
        },
    )))));
    assert!(app.plugin_manager.registries().is_some());
    // Choosing the fallback makes the next inspect ask it first.
    drop(
        app.update(Message::PluginManager(Manager::SelectRegistry(Some(
            "https://second.invalid".into(),
        )))),
    );
    drop(app.update(Message::PluginManager(Manager::EditSpec(
        "@PUBLIC/pkg".into(),
    ))));
    drop(app.update(Message::PluginManager(Manager::Inspect)));
    let Command::PluginInspect { spec, registry } = queue.try_recv().expect("one inspect") else {
        panic!("only the spec inspection expected")
    };
    assert_eq!(spec, "@PUBLIC/pkg");
    assert_eq!(registry.as_deref(), Some("https://second.invalid"));
    // The answer still settles as the accepted preview; the registry choice changed only the ask.
    drop(app.update(Message::Worker(Event::PluginInspected(Ok(
        PluginInspection::Accepted {
            kind: "registry".into(),
            name: Some("@PUBLIC/pkg".into()),
            version: Some("1.0.0".into()),
            description: None,
            bundle: Some(true),
            registry: Some("https://second.invalid".into()),
            host: None,
        },
    )))));
    assert!(matches!(
        app.plugin_manager.inspection(),
        Some(PluginInspection::Accepted { .. })
    ));
}

#[test]
fn switching_a_plugin_row_needs_confirm_and_sends_one_write() {
    use super::plugin_manager::Action as Manager;
    use dsh_native_transport::plugin::{FiberPhase, PluginApplication, PluginChange, PluginRow};
    let (mut app, mut queue) = app();
    let bundle = dsh_native_transport::plugin::PluginBundle {
        name: "PUBLIC-bundle".into(),
        version: None,
        description: None,
        enabled: true,
        installed: true,
        optional: false,
        removable: true,
        rows: vec![dsh_native_transport::plugin::PluginBundleRow {
            row_id: "tools".into(),
            module_name: "PUBLIC-bundle/tools".into(),
            entry_id: Some("include:tools".into()),
        }],
        overrides: 0,
        read_only_reason: None,
        error: None,
    };
    app.plugin_manager.seed(true, vec![bundle]);
    app.plugin_manager.seed_rows(vec![PluginRow {
        entry_id: "include:tools".into(),
        module_name: "PUBLIC-bundle/tools".into(),
        enabled: true,
        fiber_phase: FiberPhase::Active,
        patch_id: Some("patch-tools".into()),
        read_only_reason: None,
    }]);
    // Arming sends nothing.
    drop(app.update(Message::PluginManager(Manager::AskRow {
        entry_id: "include:tools".into(),
        enabled: false,
    })));
    assert!(queue.try_recv().is_err());
    // Cancel sends nothing; a confirmed review sends exactly one row write.
    drop(app.update(Message::PluginManager(Manager::CancelRow)));
    drop(app.update(Message::PluginManager(Manager::ConfirmRow)));
    assert!(queue.try_recv().is_err());
    drop(app.update(Message::PluginManager(Manager::AskRow {
        entry_id: "include:tools".into(),
        enabled: false,
    })));
    drop(app.update(Message::PluginManager(Manager::ConfirmRow)));
    let Command::PluginEntryEnabled { entry_id, enabled } =
        queue.try_recv().expect("one row write")
    else {
        panic!("only the row switch expected")
    };
    assert_eq!((entry_id.as_str(), enabled), ("include:tools", false));
    // A second Confirm while the first is in flight is ignored.
    drop(app.update(Message::PluginManager(Manager::ConfirmRow)));
    assert!(queue.try_recv().is_err());
    // The settlement re-reads the live rows, not the bundles.
    drop(app.update(Message::Worker(Event::PluginEntryChanged {
        entry_id: "include:tools".into(),
        enabled: false,
        result: Ok(PluginChange {
            changed: true,
            application: PluginApplication::Applied,
            target: "include:tools".into(),
            enabled: Some(false),
            error: None,
        }),
    })));
    assert!(matches!(
        queue.try_recv(),
        Ok(Command::PluginManagerPlugins)
    ));
}
