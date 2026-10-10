//! PUBLIC render-only runtime receipt. No Settings service, storage or provider runs.
use super::App;
use crate::{codex, settings};
use dsh_native_core::{
    CodexCacheRetention, CodexIdleMillis, CodexImageBudget, CodexRuntimeNatural,
    CodexRuntimeSnapshot, CodexRuntimeTransport, CodexRuntimeValues,
};

pub(super) fn seed(app: &mut App) {
    let ticket = app.settings.codex_ticket();
    let effect = app.settings.handle(
        settings::Action::Codex(codex::Action::Runtime {
            ticket,
            action: codex::RuntimeAction::Refresh,
        }),
        app.transport_epoch,
        true,
    ).expect("PUBLIC runtime read, never dispatched");
    let settings::Effect::Codex(codex::Effect::RuntimeRead { ticket }) = effect else {
        panic!("PUBLIC expected runtime read");
    };
    let base_values = CodexRuntimeValues {
        stream_idle_timeout_ms: Some(CodexIdleMillis::new(300000.0).unwrap()),
        request_image_pixel_budget: Some(CodexImageBudget::new(4194304).unwrap()),
        request_image_max_bytes: Some(CodexImageBudget::new(1048576).unwrap()),
        max_request_image_bytes: Some(CodexImageBudget::new(20971520).unwrap()),
        ..CodexRuntimeValues::default()
    };
    let overrides = CodexRuntimeValues {
        transport: Some(CodexRuntimeTransport::Auto),
        cache_retention: Some(CodexCacheRetention::Short),
        timeout_ms: Some(CodexRuntimeNatural::new(0).unwrap()),
        websocket_connect_timeout_ms: Some(CodexRuntimeNatural::new(9000).unwrap()),
        stream_idle_timeout_ms: Some(CodexIdleMillis::new(45000.5).unwrap()),
        request_image_pixel_budget: Some(CodexImageBudget::new(1048576).unwrap()),
        request_image_max_bytes: Some(CodexImageBudget::new(524288).unwrap()),
        max_request_image_bytes: Some(CodexImageBudget::new(8388608).unwrap()),
    };
    app.settings.codex_completed(ticket, codex::Outcome::Runtime(CodexRuntimeSnapshot {
        version: 1,
        revision: 4,
        configured: true,
        writable: true,
        values: overrides,
        base_values,
        overrides,
    }));
}

pub(super) fn fail_save(app: &mut App) {
    let ticket = app.settings.codex_ticket();
    let effect = app.settings.handle(
        settings::Action::Codex(codex::Action::Runtime { ticket, action: codex::RuntimeAction::Save }),
        app.transport_epoch,
        true,
    ).expect("PUBLIC runtime save, never dispatched");
    let settings::Effect::Codex(codex::Effect::RuntimeSave { ticket, .. }) = effect else {
        panic!("PUBLIC expected runtime save");
    };
    app.settings.codex_completed(ticket, codex::Outcome::RuntimeIndeterminate);
}
