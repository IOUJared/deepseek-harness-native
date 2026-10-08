use super::*;
use crate::plugins::{ReadFailure, ReadTicket, SaveOutcome, WriteTicket};

fn read() -> ReadTicket {
    ReadTicket {
        epoch: TRANSPORT_EPOCH,
        panel: 3,
        serial: 4,
    }
}
fn write() -> WriteTicket {
    WriteTicket {
        epoch: TRANSPORT_EPOCH,
        panel: 3,
        serial: 5,
        description: 6,
        namespace: 0,
        expected_revision: 2,
        draft_revision: 7,
    }
}
#[test]
fn plugin_admission_is_epoch_smoke_shutdown_capacity_and_key_save_fenced() {
    let op = PluginOperation::Read(read());
    for (candidate, smoke, stopping, tasks, key_saving) in [
        (op, true, false, 0, false),
        (op, false, true, 0, false),
        (op, false, false, MAX_OPERATIONS, false),
        (op, false, false, 0, true),
        (
            PluginOperation::Read(ReadTicket {
                epoch: 99,
                ..read()
            }),
            false,
            false,
            0,
            false,
        ),
    ] {
        let mut active = None;
        assert!(!admit_plugin(
            &mut active,
            candidate,
            smoke,
            stopping,
            tasks,
            key_saving
        ));
        assert!(active.is_none());
    }
    let mut active = None;
    assert!(admit_plugin(&mut active, op, false, false, 0, false));
    assert!(!admit_plugin(
        &mut active,
        PluginOperation::Save(write()),
        false,
        false,
        0,
        false
    ));
    assert_eq!(active, Some(op));
}
#[test]
fn plugin_settlement_requires_full_matching_ticket_and_operation_kind() {
    let op = PluginOperation::Read(read());
    let mut active = Some(op);
    settle_plugin(&mut active, &Event::Ready("PUBLIC".into()));
    settle_plugin(
        &mut active,
        &Event::PluginSaved {
            ticket: write(),
            result: SaveOutcome::NotSent,
        },
    );
    settle_plugin(
        &mut active,
        &Event::PluginsLoaded {
            ticket: ReadTicket {
                serial: 9,
                ..read()
            },
            result: Err(ReadFailure::Unavailable),
        },
    );
    assert_eq!(active, Some(op));
    settle_plugin(
        &mut active,
        &Event::PluginsLoaded {
            ticket: read(),
            result: Err(ReadFailure::Malformed),
        },
    );
    assert!(active.is_none());
    active = Some(PluginOperation::Save(write()));
    settle_plugin(
        &mut active,
        &Event::PluginSaved {
            ticket: WriteTicket {
                draft_revision: 99,
                ..write()
            },
            result: SaveOutcome::Indeterminate,
        },
    );
    assert!(active.is_some());
    settle_plugin(
        &mut active,
        &Event::PluginSaved {
            ticket: write(),
            result: SaveOutcome::Indeterminate,
        },
    );
    assert!(active.is_none());
}
#[test]
fn plugin_capacity_rejection_and_metadata_debug_never_retain_error_bodies() {
    let event = rejected(Command::PluginsRead(read()));
    assert!(
        matches!(event, Event::PluginsLoaded { ticket, result: Err(ReadFailure::Unavailable) } if ticket == read())
    );
    assert_eq!(format!("{event:?}"), "NativeWorkerEvent(redacted)");
    assert_eq!(
        plugin_read_failure(dsh_native_transport::Error::InvalidDto),
        ReadFailure::Malformed
    );
    assert_eq!(
        plugin_read_failure(dsh_native_transport::Error::SettingsRejected),
        ReadFailure::Refused
    );
    assert_eq!(
        plugin_read_failure(dsh_native_transport::Error::Timeout),
        ReadFailure::Unavailable
    );
}
#[test]
fn admitted_plugin_errors_are_indeterminate_except_fixed_backend_diagnoses() {
    use dsh_native_transport::Error;
    for error in [
        Error::RemoteFailure,
        Error::Network,
        Error::Timeout,
        Error::Closed,
        Error::InvalidDto,
        Error::InvalidJson,
        Error::Correlation,
        Error::Oversize,
    ] {
        assert!(matches!(
            plugin_save_outcome(Err(error)),
            SaveOutcome::Indeterminate
        ));
    }
    assert!(matches!(
        plugin_save_outcome(Err(Error::SettingsConflict)),
        SaveOutcome::Conflict
    ));
    assert!(matches!(
        plugin_save_outcome(Err(Error::SettingsRejected)),
        SaveOutcome::Refused
    ));
}
