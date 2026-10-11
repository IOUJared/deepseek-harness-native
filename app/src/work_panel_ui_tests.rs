//! PUBLIC bounded/read-only work-panel tests; no worker, Host, model or clipboard.
use super::*;
use crate::work_panel::{SECTION_CAP, Section, Store, TOTAL_CAP};
use serde_json::{Value, json};
fn app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
    let (handle, feed, receiver, _, _) = worker::test_channels();
    let options = crate::config::parse(
        [
            "--runtime",
            "/PUBLIC-never-start/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/PUBLIC-work-native-home",
            "--user-home",
            "/PUBLIC-work-user-home",
            "--cwd",
            "/tmp",
        ]
        .map(str::to_owned),
    )
    .unwrap()
    .unwrap();
    let (mut app, task) = App::boot(options, handle, feed);
    drop(task);
    app.management.baseline(vec![], vec![]);
    (app, receiver)
}
/// Apply one dense snapshot, as `detail_ui_tests` does for focused record sets.
fn snapshot(app: &mut App, records: Vec<Value>) {
    let cursor = records.len() as u64 - 1;
    let frame = serde_json::from_value(json!({
        "type": "snapshot",
        "records": records,
        "cursor": cursor,
        "hasMore": false,
        "header": {"version":4,"id":"PUBLIC-work","createdAt":1000,"isSeeded":false},
        "projections": {"asOfSeq": cursor, "values": {}},
        "assistantStream": {"revision": 0}
    }))
    .unwrap();
    let _ = app.transcript.apply(app.generation, frame);
}
fn event(seq: u64, kind: &str, data: Value) -> Value {
    json!({"type":"event","event":{"type":kind,"seq":seq,"time":1000+seq,"data":data}})
}
fn goal(seq: u64, id: &str, objective: &str, phase: &str) -> Value {
    event(
        seq,
        "goal/change",
        json!({
            "kind":"goal/change","version":1,"operation":"create",
            "goal":{"id":id,"revision":1,"objective":objective,"phase":phase,"maxGoalRounds":4},
            "roundsStarted":0,"createdAt":1000,"updatedAt":1000
        }),
    )
}
fn todo(seq: u64, content: &str, status: &str) -> Value {
    event(
        seq,
        "todo/write",
        json!({"todos":[{"content":content,"status":status}]}),
    )
}
fn catalog(seq: u64, child: &str, label: &str) -> Value {
    event(
        seq,
        "subagent/catalog",
        json!({"version":1,"childId":child,"childCreatedAt":1000,"mode":"continuable","label":label}),
    )
}
fn schedule(seq: u64, id: &str, prompt: &str) -> Value {
    event(
        seq,
        "schedule/change",
        json!({"version":1,"operation":"create","schedule":{
            "id":id,"kind":"every","prompt":prompt,"everySeconds":300,
            "scheduledAt":"2026-08-31T00:00:00.000Z"
        }}),
    )
}
// --- Bounded retention -------------------------------------------------------

#[test]
fn section_overflow_keeps_newest_and_reports_the_omission() {
    let (mut app, _queue) = app();
    let records = (0..SECTION_CAP as u64 + 10)
        .map(|index| {
            goal(
                index,
                &format!("goal-{index}"),
                "PUBLIC objective",
                "active",
            )
        })
        .collect();
    snapshot(&mut app, records);
    let work = app.transcript.work();
    assert_eq!(work.entries(Section::Goals).len(), SECTION_CAP);
    assert_eq!(work.dropped(Section::Goals), 10);
    // Newest retained, oldest omitted: the omission is explicit, not silent.
    assert_eq!(work.entries(Section::Goals).first().unwrap().key, "goal-10");
    assert_eq!(work.entries(Section::Goals).last().unwrap().key, "goal-59");
    // A neighbouring section was never touched by the goals overflow.
    assert!(work.entries(Section::Todos).is_empty());
    assert_eq!(work.dropped(Section::Todos), 0);
}
#[test]
fn total_overflow_is_bounded_across_sections() {
    let mut store = Store::default();
    // 4 × 60 = 240 valid records, each section under its own cap.
    for index in 0..60u64 {
        store.ingest(
            "goal/change",
            &json!({"operation":"create","goal":{"id":format!("g{index}"),"objective":"o","phase":"active"}}),
        );
        store.ingest(
            "todo/write",
            &json!({"todos":[{"content":format!("t{index}"),"status":"pending"}]}),
        );
        store.ingest(
            "subagent/catalog",
            &json!({"version":1,"childId":format!("c{index}"),"mode":"one-shot"}),
        );
        store.ingest(
            "schedule/change",
            &json!({"version":1,"operation":"create","schedule":{"id":format!("s{index}"),"kind":"at","prompt":"p","scheduledAt":"2026-08-31T00:00:00.000Z"}}),
        );
    }
    assert!(store.total() <= TOTAL_CAP);
    assert_eq!(store.total(), TOTAL_CAP);
    assert_eq!(
        Section::ALL
            .iter()
            .map(|section| store.entries(*section).len())
            .sum::<usize>(),
        store.total()
    );
    // 240 valid records; every section keeps its newest `SECTION_CAP`, and the
    // 40 oldest records are explicitly omitted rather than silently dropped.
    assert_eq!(store.entries(Section::Goals)[0].key, "g10");
    assert_eq!(store.entries(Section::Todos)[0].key, "t10");
    assert_eq!(store.entries(Section::Subagents)[0].key, "c10");
    assert_eq!(store.entries(Section::Schedules)[0].key, "s10");
    for section in Section::ALL {
        assert_eq!(store.entries(section).len(), SECTION_CAP);
        assert_eq!(store.dropped(section), 10, "{}", section.name());
    }
    assert_eq!(store.dropped_total(), 40);
}
#[test]
fn identical_goal_updates_replace_instead_of_growing() {
    let mut store = Store::default();
    for phase in ["active", "paused", "complete"] {
        store.ingest(
            "goal/change",
            &json!({"operation":"edit","goal":{"id":"one","objective":"o","phase":phase}}),
        );
    }
    assert_eq!(store.entries(Section::Goals).len(), 1);
    assert_eq!(store.dropped(Section::Goals), 0);
}
#[test]
fn long_goal_objective_is_bounded_on_a_char_boundary() {
    let mut store = Store::default();
    // Multi-byte text must never be split mid-character by the bound.
    let objective = "世界".repeat(200);
    store.ingest(
        "goal/change",
        &json!({"operation":"create","goal":{"id":"one","objective":objective,"phase":"active"}}),
    );
    let line = &store.entries(Section::Goals)[0].lines[0];
    assert!(line.ends_with(" · active"), "{line}");
    assert!(line.len() <= 90 + " · active".len());
}
// --- Malformed and unknown records -------------------------------------------

#[test]
fn malformed_records_are_ignored_without_crashing_or_inventing() {
    let (mut app, _queue) = app();
    let malformed = vec![
        event(0, "goal/change", json!("not an object")),
        event(1, "goal/change", json!({"operation":7})),
        event(2, "goal/change", json!({"operation":"create"})),
        event(
            3,
            "goal/change",
            json!({"operation":"create","goal":{"id":"","phase":"active"}}),
        ),
        event(
            4,
            "goal/change",
            json!({"operation":"create","goal":{"id":"g","phase":"future"}}),
        ),
        event(5, "todo/write", json!({"todos":"not a list"})),
        event(
            6,
            "todo/write",
            json!({"todos":[{"content":"x","status":"future"},{"status":"pending"}]}),
        ),
        event(
            7,
            "subagent/catalog",
            json!({"version":9,"childId":"c","mode":"one-shot"}),
        ),
        event(
            8,
            "subagent/catalog",
            json!({"version":1,"childId":"c","mode":"continuable"}),
        ),
        event(
            9,
            "subagent/descriptor",
            json!({"version":3,"provider":"p","mode":"continuable"}),
        ),
        event(
            10,
            "schedule/change",
            json!({"version":0,"operation":"create"}),
        ),
        event(
            11,
            "schedule/change",
            json!({"version":1,"operation":"create","schedule":{"id":"s","kind":"future","prompt":"p"}}),
        ),
        event(
            12,
            "schedule/change",
            json!({"version":1,"operation":"delete","id":"missing"}),
        ),
        // An unknown kind is not this projection's business and stays in Trajectory.
        event(13, "future/required", json!({"anything":true})),
    ];
    snapshot(&mut app, malformed);
    assert!(app.transcript.work().is_empty());
    assert_eq!(app.transcript.work().total(), 0);
    // The reducer still exposes its own Trajectory rows; this projection invented none.
    assert!(
        !app.transcript
            .display_rows(true)
            .collect::<Vec<_>>()
            .is_empty()
    );
}
#[test]
fn unknown_kind_stays_a_trajectory_row_rather_than_a_panel_record() {
    let (mut app, _queue) = app();
    snapshot(
        &mut app,
        vec![event(0, "future/required", json!({"value":1}))],
    );
    assert!(app.transcript.work().is_empty());
    let rows: Vec<_> = app.transcript.display_rows(true).collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].role, "future/required");
}
// --- Empty state and valid records -------------------------------------------

#[test]
fn empty_state_is_a_short_line_and_reads_no_section() {
    let (mut app, mut queue) = app();
    drop(app.update(Message::Work(work_panel_ui::Action::Open)));
    assert!(app.work_open);
    assert!(app.transcript.work().is_empty());
    drop(app.view()); // Renders the bounded empty line, not an empty box.
    assert!(queue.try_recv().is_err());
}
#[test]
fn valid_records_populate_each_section() {
    let (mut app, _queue) = app();
    snapshot(
        &mut app,
        vec![
            goal(0, "g1", "PUBLIC goal", "active"),
            todo(1, "PUBLIC todo", "in_progress"),
            catalog(2, "c1", "PUBLIC child"),
            schedule(3, "s1", "PUBLIC prompt"),
        ],
    );
    let work = app.transcript.work();
    assert_eq!(work.entries(Section::Subagents).len(), 1);
    assert_eq!(work.entries(Section::Subagents)[0].key, "c1");
    assert_eq!(
        work.entries(Section::Subagents)[0].lines[0],
        "label: PUBLIC child"
    );
    assert_eq!(
        work.entries(Section::Subagents)[0].lines[1],
        "mode: continuable"
    );
    for section in Section::ALL {
        assert!(!work.entries(section).is_empty(), "{}", section.name());
        assert_eq!(work.dropped(section), 0, "{}", section.name());
    }
    // The objective and phase share one compact scannable row.
    assert_eq!(
        work.entries(Section::Goals)[0].lines[0],
        "PUBLIC goal · active"
    );
    assert_eq!(
        work.entries(Section::Todos)[0].lines[0],
        "PUBLIC todo · in progress"
    );
    assert!(
        work.entries(Section::Schedules)[0]
            .lines
            .contains(&"prompt: PUBLIC prompt".into())
    );
}
#[test]
fn continuable_descriptor_is_its_own_keyed_row() {
    let mut store = Store::default();
    store.ingest(
        "subagent/descriptor",
        &json!({"version":3,"provider":"p","mode":"continuable","label":"PUBLIC explorer","agentProvider":"anthropic","agentModel":"claude"}),
    );
    let entries = store.entries(Section::Subagents);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "PUBLIC explorer");
    assert_eq!(entries[0].lines[0], "label: PUBLIC explorer");
    assert_eq!(entries[0].lines[1], "mode: continuable");
    assert_eq!(entries[0].lines[2], "agent: anthropic / claude");
}
// --- Panel routing -----------------------------------------------------------

#[test]
fn more_menu_opens_and_dismisses_the_read_only_panel() {
    let (mut app, mut queue) = app();
    assert!(app.composer_visible());
    drop(app.update(Message::ToggleConversationMenu));
    drop(app.update(Message::Work(work_panel_ui::Action::Open)));
    assert!(app.work_open && !app.conversation_menu && !app.composer_visible());
    drop(app.view());
    // Escape/close behaves like the other auxiliary views.
    drop(app.update(Message::DismissPanel));
    assert!(!app.work_open && app.composer_visible());
    // The explicit Back control is equivalent and equally inert.
    drop(app.update(Message::ToggleConversationMenu));
    drop(app.update(Message::Work(work_panel_ui::Action::Open)));
    drop(app.update(Message::Work(work_panel_ui::Action::Close)));
    assert!(!app.work_open);
    assert!(queue.try_recv().is_err());
}
#[test]
fn opening_the_panel_is_view_only_and_preserves_draft_content() {
    let (mut app, mut queue) = app();
    app.editor = text_editor::Content::with_text("PUBLIC unsent draft");
    snapshot(&mut app, vec![goal(0, "g1", "PUBLIC goal", "active")]);
    let identity = (app.generation, app.follow_bottom, app.records_expanded);
    drop(app.update(Message::Work(work_panel_ui::Action::Open)));
    assert!(app.work_open);
    assert_eq!(app.editor.text(), "PUBLIC unsent draft");
    assert_eq!(
        (app.generation, app.follow_bottom, app.records_expanded),
        identity
    );
    drop(app.view());
    drop(app.update(Message::Work(work_panel_ui::Action::Close)));
    assert_eq!(app.editor.text(), "PUBLIC unsent draft");
    assert!(queue.try_recv().is_err());
}
#[test]
fn information_and_work_panels_remain_mutually_exclusive() {
    let (mut app, mut queue) = app();
    drop(app.update(Message::Capabilities));
    drop(app.update(Message::Work(work_panel_ui::Action::Open)));
    assert!(app.work_open && !app.capabilities);
    drop(app.update(Message::Capabilities));
    assert!(app.capabilities && !app.work_open);
    assert!(queue.try_recv().is_err());
}
#[test]
fn closing_state_rejects_panel_open() {
    let (mut app, mut queue) = app();
    app.close = Close::Stopping;
    drop(app.update(Message::Work(work_panel_ui::Action::Open)));
    assert!(!app.work_open);
    assert!(queue.try_recv().is_err());
}

/// The composer's plan chip reads the *newest* write, unlike the cumulative
/// Todos section: a plan the model replaced must not keep stale items.
#[test]
fn standing_plan_is_the_newest_write_and_replaces_the_previous_list() {
    let mut store = Store::default();
    assert!(store.plan().is_none());
    store.ingest(
        "todo/write",
        &json!({"todos":[
            {"content":"read the reducer","status":"completed"},
            {"content":"fix the fold","status":"in_progress"},
            {"content":"run the suite","status":"pending"}
        ]}),
    );
    assert_eq!(store.plan(), Some((1, 3, Some("fix the fold"))));
    // The cumulative panel still knows every item ever written.
    assert_eq!(store.entries(Section::Todos).len(), 3);

    // A later write that drops two items replaces the plan entirely.
    store.ingest(
        "todo/write",
        &json!({"todos":[{"content":"run the suite","status":"in_progress"}]}),
    );
    assert_eq!(store.plan(), Some((0, 1, Some("run the suite"))));
    assert_eq!(store.entries(Section::Todos).len(), 3);

    // With nothing in progress the next step is unknown, not invented.
    store.ingest(
        "todo/write",
        &json!({"todos":[
            {"content":"run the suite","status":"completed"},
            {"content":"write the note","status":"pending"}
        ]}),
    );
    assert_eq!(store.plan(), Some((1, 2, None)));
}
#[test]
fn standing_plan_ignores_malformed_writes_and_bounds_the_next_item() {
    let mut store = Store::default();
    for bad in [
        json!({}),
        json!({"todos":[]}),
        json!({"todos":"not-a-list"}),
        json!({"todos":[{"content":"","status":"pending"}]}),
        json!({"todos":[{"content":"x","status":"unknown"}]}),
        json!({"todos":[{"status":"pending"}]}),
        json!({"todos":[7]}),
    ] {
        store.ingest("todo/write", &bad);
        assert!(store.plan().is_none(), "{bad}");
    }
    let long = "a".repeat(4096);
    store.ingest(
        "todo/write",
        &json!({"todos":[{"content":long,"status":"in_progress"}]}),
    );
    let (done, total, next) = store.plan().unwrap();
    assert_eq!((done, total), (0, 1));
    let next = next.unwrap();
    // `bounded` keeps at most `max - 1` bytes of text and appends a three-byte
    // ellipsis, so the retained item stays within the section bound.
    assert_eq!(next.len(), 511 + '…'.len_utf8());
    assert!(next.ends_with('…'));
    // A session with no todo write keeps no plan, and the projected plan the
    // composer reads comes from the same retained records as the panel.
    let (mut app, _queue) = app();
    snapshot(
        &mut app,
        vec![goal(0, "PUBLIC-goal", "PUBLIC objective", "active")],
    );
    assert!(app.transcript.work().plan().is_none());
    snapshot(
        &mut app,
        vec![
            goal(0, "PUBLIC-goal", "PUBLIC objective", "active"),
            // One write carries the whole list, as `todo_write` sends it.
            event(
                1,
                "todo/write",
                json!({"todos":[
                    {"content":"fix the fold","status":"in_progress"},
                    {"content":"run the suite","status":"pending"}
                ]}),
            ),
        ],
    );
    assert_eq!(
        app.transcript.work().plan(),
        Some((0, 2, Some("fix the fold")))
    );
}

/// The composer's context disclosure is presentation only: it never survives a
/// hidden composer, never reaches a settings write, and starts no request.
#[test]
fn context_policy_disclosure_follows_the_composer_and_dispatches_nothing() {
    let (mut app, mut queue) = app();
    snapshot(
        &mut app,
        vec![
            goal(0, "PUBLIC-goal", "PUBLIC objective", "active"),
            todo(1, "fix the fold", "in_progress"),
        ],
    );
    assert!(app.composer_visible());
    assert!(!app.context_policy_open);
    drop(app.update(Message::ToggleContextPolicy));
    assert!(app.context_policy_open);
    drop(app.update(Message::ToggleContextPolicy));
    assert!(!app.context_policy_open);

    // A surface that hides the composer also retires the expansion.
    drop(app.update(Message::ToggleContextPolicy));
    assert!(app.context_policy_open);
    drop(app.update(Message::Work(work_panel_ui::Action::Open)));
    assert!(!app.composer_visible());
    drop(app.update(Message::ToggleContextPolicy));
    assert!(!app.context_policy_open);
    drop(app.update(Message::Work(work_panel_ui::Action::Close)));
    assert!(app.composer_visible());
    assert!(!app.context_policy_open);
    // No command, effect or prompt was produced by any disclosure transition.
    assert!(queue.try_recv().is_err());
    // The same records still render the plan the chip states.
    assert_eq!(
        app.transcript.work().plan(),
        Some((0, 1, Some("fix the fold")))
    );
}
