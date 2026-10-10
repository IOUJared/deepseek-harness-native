//! Original navigation is retained as the order/filter oracle and release workload baseline.
use super::*;
use std::hint::black_box;

fn reference(app: &App) -> Vec<&SessionSummary> {
    let filter = app.filter.to_lowercase();
    let ids = app
        .workspace
        .as_ref()
        .and_then(|id| {
            app.workspaces
                .iter()
                .find(|workspace| &workspace.workspace_id == id)
        })
        .map(|workspace| &workspace.session_ids);
    let mut sessions: Vec<_> = app
        .sessions
        .values()
        .filter(|session| {
            app.management.registry.archived(&session.session_id) == app.show_archived
                && ids.is_none_or(|ids| ids.contains(&session.session_id))
                && (filter.is_empty()
                    || session.session_id.as_str().to_lowercase().contains(&filter)
                    || app.session_title(session).to_lowercase().contains(&filter))
        })
        .collect();
    sessions.sort_by(|a, b| app.navigation_order(a, b));
    sessions
}
fn fixture(
    members: usize,
    count: usize,
    scoped: bool,
) -> (App, tokio::sync::mpsc::Receiver<Command>) {
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
    let id = |i| SessionId::new(format!("PUBLIC-session-{:08x}", i * 7919)).unwrap();
    for i in 0..count {
        let session_id = id(i);
        app.sessions.insert(
            session_id.clone(),
            SessionSummary {
                session_id,
                agent_available: true,
                updated_at: (i % 11) as i64,
                running: i % 5 == 0,
                blank: false,
                parent_session_id: None,
                origin: None,
                cwd: None,
                projections: None,
            },
        );
    }
    let workspace_id = WorkspaceId::new("PUBLIC-workspace").unwrap();
    app.workspaces.push(WorkspaceView {
        workspace_id: workspace_id.clone(),
        path: "/PUBLIC".into(),
        title: "PUBLIC".into(),
        session_ids: (0..members).map(id).collect(),
        created_at: "PUBLIC".into(),
        updated_at: "PUBLIC".into(),
    });
    if scoped {
        app.workspace = Some(workspace_id);
    }
    (app, queue)
}
fn ids(rows: Vec<&SessionSummary>) -> Vec<&SessionId> {
    rows.into_iter().map(|row| &row.session_id).collect()
}

#[test]
fn indexed_navigation_preserves_original_order_filters_and_workspace_changes() {
    for (members, count, scoped) in [
        (0, 12, false),
        (0, 12, true),
        (12, 24, true),
        (64, 128, true),
        (512, 1024, true),
    ] {
        let (mut app, mut queue) = fixture(members, count, scoped);
        let archives = app.sessions.keys().step_by(7).cloned().collect();
        let pins = app.sessions.keys().rev().take(3).cloned().collect();
        assert!(app.management.baseline(archives, pins));
        for filter in [
            "",
            "PUBLIC-session",
            "public-session",
            "not-present",
            "00000000",
        ] {
            app.filter = filter.into();
            for archived in [false, true] {
                app.show_archived = archived;
                assert_eq!(ids(app.navigation_sessions()), ids(reference(&app)));
            }
        }
        app.show_archived = false;
        app.filter.clear();
        if let Some(first) = app.workspaces[0].session_ids.first().cloned() {
            app.workspaces[0].session_ids.push(first);
        }
        assert_eq!(ids(app.navigation_sessions()), ids(reference(&app)));
        app.workspaces[0].session_ids.reverse();
        app.workspaces[0].session_ids.truncate(1);
        assert_eq!(ids(app.navigation_sessions()), ids(reference(&app)));
        app.workspace = Some(WorkspaceId::new("PUBLIC-missing").unwrap());
        assert_eq!(ids(app.navigation_sessions()), ids(reference(&app)));
        assert!(queue.try_recv().is_err());
    }
}

fn elapsed(f: fn(&App) -> Vec<&SessionSummary>, app: &App, repeats: usize) -> u128 {
    let start = Instant::now();
    for _ in 0..repeats {
        black_box(f(black_box(app)));
    }
    start.elapsed().as_nanos()
}
#[test]
#[ignore = "controlled release navigation benchmark, not a correctness gate"]
fn production_navigation_release_matrix() {
    for (name, members, count, scope) in [
        ("all", 12, 24, false),
        ("empty", 0, 24, true),
        ("typical", 12, 24, true),
        ("medium", 64, 128, true),
        ("large", 512, 1024, true),
        ("maximum", 2048, 4096, true),
        ("large-small-roster", 4096, 8, true),
        ("large-one-session", 4096, 1, true),
    ] {
        let (app, _queue) = fixture(members, count, scope);
        assert_eq!(ids(reference(&app)), ids(app.navigation_sessions()));
        let repeats = if count > 1000 {
            10
        } else if count > 100 {
            100
        } else {
            10000
        };
        for _ in 0..10 {
            black_box(reference(&app));
            black_box(app.navigation_sessions());
        }
        for sample in 0..11 {
            let (old, new) = if sample % 2 == 0 {
                (
                    elapsed(reference, &app, repeats),
                    elapsed(App::navigation_sessions, &app, repeats),
                )
            } else {
                let new = elapsed(App::navigation_sessions, &app, repeats);
                (elapsed(reference, &app, repeats), new)
            };
            println!("NAV-BENCH,{name},{sample},{repeats},{old},{new}");
        }
    }
}
