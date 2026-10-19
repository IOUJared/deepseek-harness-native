//! PUBLIC App-state terminal-panel tests: no worker, Host, shell or clipboard.
//!
//! These assert the client-only contract: opening reads and spawns nothing, every
//! mutation is consent-gated and fenced, and only Host ACKs/frames become state.
use super::*;
use crate::terminal::{Action, Loaded, Notice, OUTPUT_CHUNKS, ReadTicket};
use dsh_native_transport::terminal::{
    TerminalFrame, TerminalShell, TerminalState, WebTerminalId, WebTerminalInfo,
};

fn app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
    let (handle, feed, receiver, _, _) = worker::test_channels();
    let options = crate::config::parse(
        [
            "--runtime",
            "/PUBLIC-never-start/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/PUBLIC-terminal-native-home",
            "--user-home",
            "/PUBLIC-terminal-user-home",
            "--cwd",
            "/tmp",
        ]
        .map(str::to_owned),
    )
    .unwrap()
    .unwrap();
    let (app, task) = App::boot(options, handle, feed);
    drop(task);
    (app, receiver)
}

fn session_id() -> SessionId {
    SessionId::new("PUBLIC-terminal-session").unwrap()
}

/// Put the App in the state the More menu requires: ready, a selected ordinary
/// Session and a ready registry baseline. No Host or worker is involved.
fn ready(app: &mut App) -> SessionId {
    app.ready = true;
    app.root_ready = true;
    app.roster_pending = false;
    app.management.baseline(vec![], vec![]);
    let id = session_id();
    app.sessions.insert(
        id.clone(),
        SessionSummary {
            agent_available: true,
            session_id: id.clone(),
            updated_at: 1000,
            running: false,
            blank: true,
            parent_session_id: None,
            origin: None,
            cwd: None,
            projections: None,
        },
    );
    app.selected = Some(id.clone());
    app.generation = 7;
    id
}

fn terminal_info(id: &str) -> WebTerminalInfo {
    WebTerminalInfo {
        id: WebTerminalId::new(id).unwrap(),
        title: "PUBLIC terminal".into(),
        shell: TerminalShell {
            path: "/bin/PUBLIC-shell".into(),
            args: vec!["-l".into()],
            name: "PUBLIC shell".into(),
        },
        cwd: "/PUBLIC/cwd".into(),
        cols: 80,
        rows: 24,
        state: TerminalState::Running,
        exit_code: None,
        error: None,
        controller_id: None,
    }
}

fn environment() -> dsh_native_transport::terminal::TerminalEnvironment {
    dsh_native_transport::terminal::TerminalEnvironment {
        cwd: "/PUBLIC/cwd".into(),
        max_input_bytes: 65536,
        max_cols: 500,
        max_rows: 200,
        scrollback: 1000,
    }
}

fn loaded(
    app: &mut App,
    queue: &mut tokio::sync::mpsc::Receiver<Command>,
    list: Vec<WebTerminalInfo>,
) -> ReadTicket {
    let view = app.terminal_context().view.unwrap();
    drop(app.update(Message::Terminal(Action::Open { view })));
    let Command::TerminalRead(ticket) = queue.try_recv().unwrap() else {
        panic!("opening must read the Host and produce no shell command");
    };
    drop(app.update(Message::Worker(worker::Event::TerminalLoaded {
        ticket: ticket.clone(),
        result: Ok(Loaded {
            list,
            shells: vec![TerminalShell {
                path: "/bin/PUBLIC-shell".into(),
                args: vec!["-l".into()],
                name: "PUBLIC shell".into(),
            }],
            shells_failed: false,
            environment: Some(environment()),
        }),
    })));
    ticket
}

fn ticket(app: &App) -> crate::management::Ticket {
    app.terminal.ticket().unwrap()
}

fn open_loaded() -> (App, tokio::sync::mpsc::Receiver<Command>, SessionId) {
    let (mut app, mut queue) = app();
    let id = ready(&mut app);
    loaded(
        &mut app,
        &mut queue,
        vec![terminal_info("PUBLIC-terminal-1")],
    );
    (app, queue, id)
}

fn select(app: &mut App) {
    let t = ticket(app);
    let id = WebTerminalId::new("PUBLIC-terminal-1").unwrap();
    drop(app.update(Message::Terminal(Action::Select { ticket: t, id })));
}

#[test]
fn opening_reads_the_host_and_produces_no_shell_command() {
    let (mut app, mut queue) = app();
    ready(&mut app);
    let view = app.terminal_context().view.unwrap();
    drop(app.update(Message::Terminal(Action::Open { view })));
    assert!(app.terminal.is_open());
    assert!(app.terminal.list().is_empty());
    // The only command the panel may emit on open is the read.
    assert!(matches!(queue.try_recv(), Ok(Command::TerminalRead(_))));
    assert!(queue.try_recv().is_err());
    drop(app.view());
    // Escape dismisses like the other auxiliary views.
    drop(app.update(Message::DismissPanel));
    assert!(!app.terminal.is_open());
    assert!(app.composer_visible());
    assert!(queue.try_recv().is_err());
}

#[test]
fn create_is_consent_gated_and_only_the_ack_installs_a_terminal() {
    let (mut app, mut queue, _id) = open_loaded();
    // A confirm without the review step sends nothing.
    let forged = crate::terminal::Review {
        ticket: ticket(&app),
    };
    drop(app.update(Message::Terminal(Action::ConfirmCreate(forged))));
    assert!(queue.try_recv().is_err());
    // Review then confirm produces exactly one create.
    let t = ticket(&app);
    drop(app.update(Message::Terminal(Action::ReviewCreate(t))));
    let review = crate::terminal::Review {
        ticket: ticket(&app),
    };
    drop(app.update(Message::Terminal(Action::ConfirmCreate(review))));
    let Command::TerminalCreate(submission) = queue.try_recv().unwrap() else {
        panic!("one create command");
    };
    assert!(queue.try_recv().is_err());
    assert_eq!(app.terminal.list().len(), 1, "no optimistic terminal");
    // A wrong-id ACK adds nothing and retires the exact receipt.
    drop(app.update(Message::Worker(worker::Event::TerminalCreated {
        submission,
        result: Ok(terminal_info("PUBLIC-other-terminal")),
    })));
    assert_eq!(app.terminal.list().len(), 1);
    assert_eq!(app.terminal.notice(), &Notice::CreatingIndeterminate);
    // A fresh reviewed create whose exact id comes back installs it and follows once.
    let t = ticket(&app);
    drop(app.update(Message::Terminal(Action::ReviewCreate(t))));
    let review = crate::terminal::Review {
        ticket: ticket(&app),
    };
    drop(app.update(Message::Terminal(Action::ConfirmCreate(review))));
    let Command::TerminalCreate(submission) = queue.try_recv().unwrap() else {
        panic!("second create command");
    };
    let created = submission.receipt.id.as_str().to_owned();
    drop(app.update(Message::Worker(worker::Event::TerminalCreated {
        submission,
        result: Ok(terminal_info(&created)),
    })));
    assert_eq!(app.terminal.list().len(), 2);
    assert_eq!(
        app.terminal.selected().map(WebTerminalId::as_str),
        Some(created.as_str())
    );
    assert!(matches!(queue.try_recv(), Ok(Command::TerminalFollow(_))));
    assert!(queue.try_recv().is_err());
}

#[test]
fn refused_create_reports_a_redacted_failure_and_adds_no_terminal() {
    let (mut app, mut queue, _id) = open_loaded();
    let t = ticket(&app);
    drop(app.update(Message::Terminal(Action::ReviewCreate(t))));
    let review = crate::terminal::Review {
        ticket: ticket(&app),
    };
    drop(app.update(Message::Terminal(Action::ConfirmCreate(review))));
    let Command::TerminalCreate(submission) = queue.try_recv().unwrap() else {
        panic!("create command");
    };
    drop(app.update(Message::Worker(worker::Event::TerminalCreated {
        submission: submission.clone(),
        result: Err(crate::terminal::CreateFailure::LimitReached),
    })));
    assert_eq!(app.terminal.notice(), &Notice::CreateLimit);
    assert_eq!(app.terminal.list().len(), 1);
    // No automatic retry: a replayed confirm produces no command.
    let t = ticket(&app);
    drop(app.update(Message::Terminal(Action::ReviewCreate(t))));
    let review = crate::terminal::Review {
        ticket: ticket(&app),
    };
    drop(app.update(Message::Terminal(Action::ConfirmCreate(review))));
    let Command::TerminalCreate(submission) = queue.try_recv().unwrap() else {
        panic!("second create");
    };
    drop(app.update(Message::Worker(worker::Event::TerminalCreated {
        submission,
        result: Err(crate::terminal::CreateFailure::Unknown),
    })));
    assert_eq!(app.terminal.notice(), &Notice::CreatingIndeterminate);
    assert!(queue.try_recv().is_err());
}

#[test]
fn selected_output_is_bounded_with_explicit_truncation() {
    let (mut app, mut queue, _id) = open_loaded();
    select(&mut app);
    let Command::TerminalFollow(_) = queue.try_recv().unwrap() else {
        panic!("follow command");
    };
    let follow = app.terminal.follow().cloned().expect("live follow");
    for sequence in 1..=(OUTPUT_CHUNKS as i64 + 20) {
        drop(app.update(Message::Worker(worker::Event::TerminalFrame {
            ticket: follow.clone(),
            frame: TerminalFrame::Output {
                sequence,
                data: format!("PUBLIC line {sequence}\n"),
            },
        })));
    }
    assert!(app.terminal.output().dropped() > 0, "overflow is counted");
    assert!(app.terminal.output().chunks().len() <= OUTPUT_CHUNKS);
    // The rendered panel states the truncation rather than silently hiding it.
    let _ = app.view();
    // A frame for a foreign follow is ignored.
    let mut foreign = follow.clone();
    foreign.serial += 1;
    let before = app.terminal.output().text();
    drop(app.update(Message::Worker(worker::Event::TerminalFrame {
        ticket: foreign,
        frame: TerminalFrame::Output {
            sequence: 9999,
            data: "PUBLIC injected".into(),
        },
    })));
    assert_eq!(app.terminal.output().text(), before);
}

#[test]
fn input_write_is_blocked_until_a_terminal_is_selected() {
    let (mut app, mut queue, _id) = open_loaded();
    let t = ticket(&app);
    drop(app.update(Message::Terminal(Action::EditInput {
        ticket: t.clone(),
        text: "PUBLIC input".into(),
    })));
    drop(app.update(Message::Terminal(Action::Write(t))));
    assert!(queue.try_recv().is_err(), "no selection means no write");
    assert_eq!(app.terminal.notice(), &Notice::InputRefused);
    // After selecting and attaching, the same write is admitted once.
    select(&mut app);
    let Command::TerminalFollow(_) = queue.try_recv().unwrap() else {
        panic!("follow");
    };
    let t = ticket(&app);
    drop(app.update(Message::Terminal(Action::Write(t.clone()))));
    let Command::TerminalWrite(submission) = queue.try_recv().unwrap() else {
        panic!("write command");
    };
    // A second write is refused while the first receipt is outstanding.
    drop(app.update(Message::Terminal(Action::Write(t))));
    assert!(queue.try_recv().is_err());
    // Only a Host acceptance clears the typed line.
    drop(app.update(Message::Worker(worker::Event::TerminalWritten {
        submission,
        result: Err(crate::terminal::WriteFailure::Unknown),
    })));
    assert_eq!(app.terminal.notice(), &Notice::WriteIndeterminate);
    assert_eq!(app.terminal.input(), "PUBLIC input");
}

#[test]
fn close_requires_confirmation_and_removes_only_a_confirmed_terminal() {
    let (mut app, mut queue, _id) = open_loaded();
    let id = WebTerminalId::new("PUBLIC-terminal-1").unwrap();
    // A confirm without review sends nothing.
    drop(app.update(Message::Terminal(Action::ConfirmClose(
        crate::terminal::CloseReview {
            ticket: ticket(&app),
            id: id.clone(),
        },
    ))));
    assert!(queue.try_recv().is_err());
    let t = ticket(&app);
    drop(app.update(Message::Terminal(Action::ReviewClose {
        ticket: t,
        id: id.clone(),
    })));
    let t = ticket(&app);
    drop(app.update(Message::Terminal(Action::ConfirmClose(
        crate::terminal::CloseReview {
            ticket: t,
            id: id.clone(),
        },
    ))));
    let Command::TerminalClose(submission) = queue.try_recv().unwrap() else {
        panic!("close command");
    };
    assert_eq!(app.terminal.list().len(), 1, "no optimistic removal");
    let receipt = submission.receipt.clone();
    drop(app.update(Message::Worker(worker::Event::TerminalClosed {
        submission,
        result: Ok(()),
    })));
    assert!(app.terminal.list().is_empty());
    assert_eq!(app.terminal.notice(), &Notice::Closed);
    // A duplicate ACK for the retired receipt changes nothing.
    drop(app.update(Message::Worker(worker::Event::TerminalClosed {
        submission: crate::terminal::CloseSubmission { receipt },
        result: Ok(()),
    })));
    assert!(app.terminal.list().is_empty());
}

#[test]
fn unknown_write_outcome_is_indeterminate_and_never_retried() {
    let (mut app, mut queue, _id) = open_loaded();
    select(&mut app);
    let Command::TerminalFollow(_) = queue.try_recv().unwrap() else {
        panic!("follow");
    };
    let t = ticket(&app);
    drop(app.update(Message::Terminal(Action::EditInput {
        ticket: t.clone(),
        text: "PUBLIC input".into(),
    })));
    drop(app.update(Message::Terminal(Action::Write(t))));
    let Command::TerminalWrite(submission) = queue.try_recv().unwrap() else {
        panic!("write");
    };
    drop(app.update(Message::Worker(worker::Event::TerminalWritten {
        submission,
        result: Err(crate::terminal::WriteFailure::Unknown),
    })));
    assert_eq!(app.terminal.notice(), &Notice::WriteIndeterminate);
    assert!(queue.try_recv().is_err(), "no automatic retry");
    assert_eq!(app.terminal.input(), "PUBLIC input", "the line is kept");
}

#[test]
fn opening_the_panel_preserves_the_chat_draft_and_hides_the_composer() {
    let (mut app, mut queue) = app();
    ready(&mut app);
    app.editor = text_editor::Content::with_text("PUBLIC unsent draft");
    let generation = app.generation;
    let view = app.terminal_context().view.unwrap();
    drop(app.update(Message::Terminal(Action::Open { view })));
    assert!(app.terminal.is_open() && !app.composer_visible());
    assert_eq!(app.editor.text(), "PUBLIC unsent draft");
    assert_eq!(app.generation, generation);
    // Only the read command is produced; chat and trajectory are untouched.
    assert!(matches!(queue.try_recv(), Ok(Command::TerminalRead(_))));
    assert!(queue.try_recv().is_err());
    drop(app.view());
    drop(app.update(Message::DismissPanel));
    assert_eq!(app.editor.text(), "PUBLIC unsent draft");
    assert!(app.composer_visible());
}

#[test]
fn a_disconnected_generation_fences_late_terminal_receipts() {
    let (mut app, mut queue, _id) = open_loaded();
    let t = ticket(&app);
    drop(app.update(Message::Terminal(Action::ReviewCreate(t))));
    let review = crate::terminal::Review {
        ticket: ticket(&app),
    };
    drop(app.update(Message::Terminal(Action::ConfirmCreate(review))));
    let Command::TerminalCreate(submission) = queue.try_recv().unwrap() else {
        panic!("create");
    };
    let created = submission.receipt.id.as_str().to_owned();
    // The owned Core exits before the ACK: nothing may be installed afterwards.
    drop(app.update(Message::Worker(worker::Event::Fault(
        "PUBLIC owned Core exited".into(),
    ))));
    assert!(!app.terminal.is_open());
    drop(app.update(Message::Worker(worker::Event::TerminalCreated {
        submission,
        result: Ok(terminal_info(&created)),
    })));
    assert!(
        !app.terminal
            .list()
            .iter()
            .any(|info| info.id.as_str() == created),
        "a late ACK after disconnect installs no terminal"
    );
    assert!(matches!(
        app.terminal.notice(),
        Notice::CreatingIndeterminate
    ));
}

#[test]
fn close_outcome_unknown_leaves_the_terminal_listed() {
    let (mut app, mut queue, _id) = open_loaded();
    let id = WebTerminalId::new("PUBLIC-terminal-1").unwrap();
    let t = ticket(&app);
    drop(app.update(Message::Terminal(Action::ReviewClose {
        ticket: t,
        id: id.clone(),
    })));
    let t = ticket(&app);
    drop(app.update(Message::Terminal(Action::ConfirmClose(
        crate::terminal::CloseReview {
            ticket: t,
            id: id.clone(),
        },
    ))));
    let Command::TerminalClose(submission) = queue.try_recv().unwrap() else {
        panic!("close");
    };
    drop(app.update(Message::Worker(worker::Event::TerminalClosed {
        submission,
        result: Err(crate::terminal::CloseFailure::Unknown),
    })));
    assert_eq!(app.terminal.notice(), &Notice::ClosingIndeterminate);
    assert_eq!(app.terminal.list().len(), 1);
}
