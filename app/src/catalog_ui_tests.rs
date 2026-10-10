// Included by the existing UI production reducer tests; all channels are inert PUBLIC fixtures.
fn catalog_fixture() -> ModelCatalog {
    serde_json::from_value(serde_json::json!({
        "groups":[{"id":"PUBLIC-provider","name":"PUBLIC provider","models":[
            {"id":"PUBLIC-model","name":"PUBLIC current model","reasoning":null}
        ]}],"routableProviders":["PUBLIC-provider"],"failures":[],"default":{"provider":"PUBLIC-provider","model":"PUBLIC-model"}
    })).unwrap()
}
fn catalog_request(
    app: &mut App,
    queue: &mut tokio::sync::mpsc::Receiver<Command>,
) -> worker::catalog_read::Ticket {
    drop(app.update(Message::LoadModels));
    match queue.try_recv().unwrap() {
        Command::Catalog(ticket) => ticket,
        _ => panic!("expected only the owned catalog read"),
    }
}
#[test]
fn catalog_stale_success_error_and_duplicate_cannot_replace_newer_flight_or_cache() {
    let (mut app, mut queue) = managed_app();
    let old = catalog_request(&mut app, &mut queue);
    drop(app.receive(Event::Fault("PUBLIC lost owner".into())));
    assert!(!app.catalog_read.pending());
    drop(app.receive(Event::Ready("0.2.1-alpha.1".into())));
    let Command::Catalog(current) = queue.try_recv().unwrap() else {
        panic!("startup catalog missing");
    };
    assert_ne!(old, current); // Same fixed worker epoch after retirement, distinct serial.
    let status = app.status.clone();
    for result in [Ok(catalog_fixture()), Err("PUBLIC stale error".into())] {
        drop(app.receive(Event::Catalog {
            ticket: old,
            result,
        }));
        assert_eq!(app.catalog_read.current(), Some(current));
        assert!(app.models.is_empty());
        assert!(app.model_controls.catalog_default.is_none());
        assert_eq!(app.status, status);
        assert!(!app.model_controls.catalog_is_failed());
    }
    drop(app.receive(Event::Catalog {
        ticket: current,
        result: Ok(catalog_fixture()),
    }));
    assert!(!app.catalog_read.pending());
    assert_eq!(app.models[0].name, "PUBLIC current model");
    let status = app.status.clone();
    for ticket in [old, current] {
        for result in [
            Ok(ModelCatalog {
                groups: vec![],
                ..catalog_fixture()
            }),
            Err("PUBLIC duplicate error".into()),
        ] {
            drop(app.receive(Event::Catalog { ticket, result }));
            assert_eq!(app.status, status);
            assert_eq!(app.models[0].name, "PUBLIC current model");
            assert_eq!(
                app.model_controls.catalog_default.as_ref().unwrap().model,
                "PUBLIC-model"
            );
            assert!(!app.catalog_read.pending());
        }
    }
    assert!(queue.try_recv().is_err());
}
#[test]
fn catalog_owned_error_keeps_cache_and_stale_success_error_do_not_clear_failure() {
    let (mut app, mut queue) = managed_app();
    let old = catalog_request(&mut app, &mut queue);
    drop(app.receive(Event::Catalog { ticket: old, result: Ok(catalog_fixture()) }));
    let current = catalog_request(&mut app, &mut queue);
    drop(app.receive(Event::Catalog { ticket: current, result: Err("PUBLIC current error".into()) }));
    assert!(app.model_controls.catalog_is_failed());
    assert!(!app.catalog_read.pending());
    let status = app.status.clone();
    for ticket in [old, current] {
        for result in [Ok(ModelCatalog { groups: vec![], ..catalog_fixture() }), Err("PUBLIC stale cached error".into())] {
            drop(app.receive(Event::Catalog { ticket, result }));
            assert!(app.model_controls.catalog_is_failed());
            assert_eq!(app.status, status);
            assert_eq!(app.models[0].name, "PUBLIC current model");
            assert_eq!(app.model_controls.catalog_default.as_ref().unwrap().model, "PUBLIC-model");
            assert!(!app.catalog_read.pending());
        }
    }
    assert!(queue.try_recv().is_err());
}
#[test]
fn catalog_unsolicited_and_wrong_epoch_receipts_leave_state_untouched() {
    let (mut app, mut queue) = managed_app();
    let unsolicited = worker::catalog_read::State::default()
        .begin(worker::TRANSPORT_EPOCH)
        .unwrap();
    let status = app.status.clone();
    for result in [Ok(catalog_fixture()), Err("PUBLIC unsolicited".into())] {
        drop(app.receive(Event::Catalog {
            ticket: unsolicited,
            result,
        }));
        assert!(app.models.is_empty());
        assert_eq!(app.status, status);
        assert!(!app.catalog_read.pending());
    }
    let current = catalog_request(&mut app, &mut queue);
    let foreign = worker::catalog_read::State::default()
        .begin(worker::TRANSPORT_EPOCH + 1)
        .unwrap();
    let status = app.status.clone();
    for result in [Ok(catalog_fixture()), Err("PUBLIC wrong epoch".into())] {
        drop(app.receive(Event::Catalog {
            ticket: foreign,
            result,
        }));
        assert_eq!(app.catalog_read.current(), Some(current));
        assert_eq!(app.status, status);
        assert!(!app.model_controls.catalog_is_failed());
        assert!(app.models.is_empty());
    }
}
#[test]
fn catalog_all_terminal_and_observation_loss_events_retire_success_and_error() {
    let stop = dsh_native_core::StopResult {
        exited: true,
        graceful: true,
        containment_unknown: false,
        observed_descendants_remaining: 0,
        exit_code: Some(0),
    };
    for terminal in [
        Event::Fault("PUBLIC fault".into()),
        Event::Core(PublicEvent::Exited(stop)),
        Event::Stopped(Ok(stop)),
        Event::NoBackendStarted,
        Event::ObservationsLost,
    ] {
        let (mut app, mut queue) = managed_app();
        let ticket = catalog_request(&mut app, &mut queue);
        drop(app.receive(terminal));
        assert!(!app.catalog_read.pending());
        let status = app.status.clone();
        for result in [
            Ok(catalog_fixture()),
            Err("PUBLIC late terminal error".into()),
        ] {
            drop(app.receive(Event::Catalog { ticket, result }));
            assert_eq!(app.status, status);
            assert!(app.models.is_empty());
            assert!(app.model_controls.catalog_default.is_none());
            assert!(!app.catalog_read.pending());
        }
        drop(app.update(Message::LoadModels));
        assert!(queue.try_recv().is_err()); // No loss/restart auto read, including ready-but-unverified observations.
    }
}
#[test]
fn catalog_global_flight_is_not_bound_to_session_or_popup_and_does_not_add_reads() {
    let (mut app, mut queue) = managed_app();
    let ticket = catalog_request(&mut app, &mut queue);
    app.generation += 1;
    app.selected = None;
    app.model_controls.dismiss();
    drop(app.update(Message::LoadModels));
    assert_eq!(app.catalog_read.current(), Some(ticket));
    assert!(queue.try_recv().is_err());
    drop(app.receive(Event::Catalog {
        ticket,
        result: Ok(catalog_fixture()),
    }));
    assert_eq!(app.models[0].name, "PUBLIC current model");
    assert!(!app.catalog_read.pending());
    drop(app.view());
    assert!(queue.try_recv().is_err());
}
#[test]
fn catalog_queue_refusal_consumes_serial_and_stale_enqueue_cannot_touch_current_status() {
    let (mut app, mut queue) = managed_app();
    let first = worker::catalog_read::State::default()
        .begin(worker::TRANSPORT_EPOCH)
        .unwrap();
    while app.handle.send(Command::Inspect).is_ok() {}
    drop(app.update(Message::LoadModels));
    assert!(!app.catalog_read.pending());
    assert!(app.status.contains("operation not sent"));
    assert!(app.model_controls.catalog_is_failed());
    while queue.try_recv().is_ok() {}
    let next = catalog_request(&mut app, &mut queue);
    assert_ne!(first, next);
    let status = app.status.clone();
    while app.handle.send(Command::Inspect).is_ok() {}
    app.command(Command::Catalog(first));
    assert_eq!(app.status, status);
    assert_eq!(app.catalog_read.current(), Some(next));
    drop(app.receive(Event::Catalog {
        ticket: first,
        result: Err("PUBLIC old rejection".into()),
    }));
    assert_eq!(app.status, status);
    assert_eq!(app.catalog_read.current(), Some(next));
}
#[test]
fn catalog_exhaustion_shutdown_and_closing_never_admit_or_apply_metadata() {
    let (mut app, mut queue) = managed_app();
    app.catalog_read = worker::catalog_read::State::exhausted();
    drop(app.update(Message::LoadModels));
    assert!(app.status.contains("exhausted"));
    assert!(!app.catalog_read.pending());
    assert!(queue.try_recv().is_err());
    drop(app.receive(Event::Ready("0.2.1-alpha.1".into())));
    assert!(queue.try_recv().is_err());
    for closing in [Close::Inspecting, Close::Stopping] {
        let (mut app, mut queue) = managed_app();
        let ticket = catalog_request(&mut app, &mut queue);
        app.close = closing;
        app.status = "PUBLIC critical close status".into();
        drop(app.receive(Event::Catalog {
            ticket,
            result: Ok(catalog_fixture()),
        }));
        assert!(app.models.is_empty());
        assert_eq!(app.status, "PUBLIC critical close status");
        assert!(!app.catalog_read.pending());
    }
    let (mut app, _) = managed_app();
    // Keep the existing inert fixture state, but retain this handle's shutdown watch.
    // Production Shutdown bypasses the business queue even when that queue is full.
    let (handle, _feed, mut queue, _events, closing) = worker::test_channels();
    app.handle = handle;
    let ticket = catalog_request(&mut app, &mut queue);
    assert!(!*closing.borrow());
    app.shutdown();
    assert!(*closing.borrow()); // The real App dispatch reached Handle's priority watch.
    assert!(matches!(app.close, Close::Stopping));
    assert!(!app.ready);
    assert!(app.draft_exit_requested);
    assert!(!app.catalog_read.pending());
    assert!(matches!(
        queue.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));
    let status = app.status.clone();
    for result in [
        Ok(catalog_fixture()),
        Err("PUBLIC late shutdown error".into()),
    ] {
        drop(app.receive(Event::Catalog { ticket, result }));
        assert_eq!(app.status, status);
        assert!(app.models.is_empty());
        assert!(app.model_controls.catalog_default.is_none());
        assert!(!app.catalog_read.pending());
        assert!(matches!(app.close, Close::Stopping));
        assert!(*closing.borrow());
    }
    drop(app.update(Message::LoadModels));
    assert!(!app.catalog_read.pending());
    assert!(matches!(
        queue.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));
}
