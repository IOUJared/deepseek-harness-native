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
