//! PUBLIC render-only metadata for the production App. Feature-gated by the parent ui module.
//! No worker start, Host, account lookup, catalog, model selection operation or business ACK.
use super::{App, Effort, InfoSection, Message, Model};
use crate::{
    config::Options,
    worker::{self, Command, Event},
};
use dsh_native_transport::dto::{
    ModelSelection, SessionFollowFrame, SessionSummary, WorkspaceView,
};
use iced::Size;
use serde_json::{Value, json};

pub const PRESETS: &[&str] = &[
    "wide-welcome",
    "narrow-welcome",
    "tiny-welcome",
    "menu",
    "navigation",
    "long-model",
    "warnings",
    "conversation",
    "title-controls", "title-controls-expanded",
    "collapsed-wide",
    "tiny-long-model",
    "navigation-tiny",
    "settings-general",
    "settings-codex",
    "information",
    "information-expanded",
    "tool-activity",
    "tool-activity-narrow",
    "tool-activity-tiny",
    "tool-activity-details",
    "assistant-details-source", "assistant-details-formatted", "assistant-details-narrow", "assistant-details-fallback",
    "chat-density",
    "file-path-review", "file-path-tiny", "file-ready", "file-unknown", "inbox-canceled",
];
pub const PUBLIC_ASSISTANT_DETAILS:&str="# PUBLIC native response\n\nThis is **bold**, *italic*, and `inline code` in a frozen PUBLIC display snapshot. No model or tool ran.\n\n## Readable steps\n\n1. Preserve the source.\n\n   PUBLIC continuation stays inside the first list item.\n\n2. Keep all controls local.\n   - No remote images.\n   - No code execution.\n\n```rust\n// PUBLIC example only; not executed.\nfn public_preview() {\n    let note = \"PUBLIC intentionally long code line demonstrates horizontal scrolling without wrapping or changing the preserved display source\";\n    println!(\"{note}\");\n}\n```\n\nPUBLIC content beyond the collapsed four-line chat preview remains visible here.\n\n> This is a read-only native presentation, not an approval or execution acknowledgement.\n\n[PUBLIC reference](https://example.invalid/PUBLIC) is inert text, not an opener.\n";
pub const PUBLIC_PROVIDER_ID: &str = "PUBLIC-layout-provider-NOT-A-ROUTE";
pub const PUBLIC_MODEL_ID: &str = "PUBLIC-layout-model-exact-id-世界-é-α";
pub const PUBLIC_EFFORT_ID: &str = "PUBLIC-layout-effort-exact-id-世界-é-α";

/// Requested App frame, not an independently measured compositor or pixel viewport.
pub fn preset_size(preset: &str) -> Option<(Size, f32)> {
    match preset {
        "assistant-details-source" | "assistant-details-formatted" | "assistant-details-fallback" => Some((Size::new(1040.0,900.0),1.0)),
        "assistant-details-narrow" => Some((Size::new(760.0,840.0),1.0)),
        "tool-activity" => Some((Size::new(1040.0, 1040.0), 1.0)),
        "chat-density" => Some((Size::new(1040.0, 1000.0), 1.0)),
        "tool-activity-narrow" => Some((Size::new(760.0, 840.0), 1.0)),
        "tool-activity-tiny" => Some((Size::new(608.0, 448.0), 1.25)),
        "wide-welcome" | "conversation" | "collapsed-wide" | "title-controls" | "title-controls-expanded" => Some((Size::new(1040.0, 700.0), 1.0)),
        "tiny-welcome" | "tiny-long-model" | "navigation-tiny" | "file-path-tiny" => {
            Some((Size::new(608.0, 448.0), 1.25))
        }
        "narrow-welcome"
        | "menu"
        | "navigation"
        | "long-model"
        | "warnings"
        | "settings-general"
        | "settings-codex"
        | "information"
        | "information-expanded"
        | "tool-activity-details" | "file-path-review" | "file-ready" | "file-unknown" | "inbox-canceled" => Some((Size::new(760.0, 560.0), 1.0)),
        _ => None,
    }
}

fn public_tool_records() -> Vec<Value> {
    // Alpha tool arguments are RAW JSON STRINGS. All paths, command text and results are
    // synthetic PUBLIC metadata: they assert neither tool execution nor a business ACK.
    let event = |seq: u64, kind: &str, data: Value| {
        json!({"type":"event", "event":{
            "type":kind, "seq":seq, "time":1000+seq, "data":data, "surfaceOp":"append"
        }})
    };
    let mut records = vec![event(
        0,
        "user/message",
        json!({
            "role":"user", "id":"PUBLIC-activity-user", "source":{"kind":"human"},
            "content":[{"type":"text", "text":"PUBLIC conversation · compare user, compact tool activity and assistant. No tools were executed."}]
        }),
    )];
    for (index, (id, name, arguments, failed)) in [
        ("PUBLIC-read-call", "functions.read", json!({"file_path":"/PUBLIC-layout-fixture/workspace/src/PUBLIC-note.txt"}), false),
        ("PUBLIC-edit-call", "functions.edit", json!({"file_path":"/PUBLIC-layout-fixture/workspace/src/PUBLIC-note.txt", "old_string":"PUBLIC old text", "new_string":"PUBLIC new text"}), false),
        ("PUBLIC-bash-call", "functions.bash", json!({"command":"printf PUBLIC_fixture_no_execution", "description":"PUBLIC recorded intent only; never executed"}), true),
    ].into_iter().enumerate() {
        let seq = 1 + index as u64 * 2;
        records.push(event(seq, "tool/call", json!({
            "callId":id, "turn":1, "step":2, "name":name, "arguments":arguments.to_string()
        })));
        records.push(event(seq+1, "tool/result", json!({"turn":1, "step":2, "message":{
            "role":"tool", "toolCallId":id, "source":{"kind":"tool", "callId":id},
            "isError":failed, "content":[{"type":"text", "text":if failed {
                "PUBLIC synthetic error-marker fixture. No bash process ran and no tool outcome is claimed."
            } else { "PUBLIC synthetic result body for full Details. No tool execution or success is claimed." }}]
        }})));
    }
    records.push(event(7, "assistant/message", json!({"turn":1, "step":2,
        "stream":[{"type":"text-chunks", "time0":1007, "index":0, "dt":[],
            "texts":["PUBLIC synthetic assistant text · tool summaries describe recorded metadata, not executed work. Open full Details in production to inspect stored text."]}]
    })));
    records
}

/// The receiver MUST be retained, inspected and drained locally; never run its Commands.
/// Tasks returned by boot/local setup are discarded; the screenshot runner owns its tasks.
pub fn build(
    mut options: Options,
    preset: &str,
    size: Size,
) -> (App, tokio::sync::mpsc::Receiver<Command>) {
    assert!(PRESETS.contains(&preset), "unknown PUBLIC layout preset");
    assert!(
        size.width.is_finite() && size.height.is_finite() && size.width > 0.0 && size.height > 0.0
    );
    options.smoke = None;
    let (handle, feed, receiver, events, closing) = worker::test_channels();
    // No stream subscription or execution exists. Dropping these endpoints cannot start a worker.
    drop((events, closing));
    let (mut app, boot_task) = App::boot(options, handle, feed);
    drop(boot_task);
    app.ready = true;
    app.root_ready = true;
    app.roster_pending = false;
    app.transport_epoch = worker::TRANSPORT_EPOCH;
    app.account = "PUBLIC local metadata · no account lookup".into();

    let selected: SessionSummary = serde_json::from_value(json!({
        "agentAvailable":true, "sessionId":"PUBLIC-layout-conversation", "updatedAt":1000,
        "running":matches!(preset, "menu" | "navigation" | "navigation-tiny"),
        "blank":preset != "conversation" && preset != "chat-density" && !preset.starts_with("tool-activity"),
        "cwd":"/PUBLIC-layout-fixture/workspace",
        "projections":{"kind":"cached", "asOfSeq":-1,
            "values":{"title":"PUBLIC local conversation"}}
    }))
    .expect("fixed PUBLIC session metadata");
    let selected_id = selected.session_id.clone();
    app.selected = Some(selected_id.clone());
    app.sessions.insert(selected_id.clone(), selected);
    for (id, title, updated) in [
        ("PUBLIC-layout-notes", "PUBLIC notes · café · 世界", 900),
        ("PUBLIC-layout-review", "PUBLIC review · native layout", 800),
    ] {
        let summary: SessionSummary = serde_json::from_value(json!({
            "agentAvailable":true, "sessionId":id, "updatedAt":updated,
            "running":false, "blank":false, "cwd":"/PUBLIC-layout-fixture/workspace",
            "projections":{"kind":"cached", "asOfSeq":-1, "values":{"title":title}}
        }))
        .expect("fixed PUBLIC navigation metadata");
        app.sessions.insert(summary.session_id.clone(), summary);
    }
    if preset == "navigation-tiny" {
        // Fixed PUBLIC synthetic history makes the real session-list scrolling meaningful.
        for index in 0..12 {
            let summary: SessionSummary = serde_json::from_value(json!({
                "agentAvailable":true, "sessionId":format!("PUBLIC-layout-scroll-{index}"),
                "updatedAt":700-index, "running":false, "blank":false,
                "cwd":"/PUBLIC-layout-fixture/workspace",
                "projections":{"kind":"cached", "asOfSeq":-1,
                    "values":{"title":format!("PUBLIC scroll row {} · café · 世界", index+1)}}
            }))
            .expect("fixed PUBLIC scroll metadata");
            app.sessions.insert(summary.session_id.clone(), summary);
        }
    }
    let workspace: WorkspaceView = serde_json::from_value(json!({
        "workspaceId":"PUBLIC-layout-workspace", "path":"/PUBLIC-layout-fixture/workspace",
        "title":"PUBLIC workspace", "sessionIds":["PUBLIC-layout-conversation", "PUBLIC-layout-notes", "PUBLIC-layout-review"],
        "createdAt":"1970-01-01T00:00:00Z", "updatedAt":"1970-01-01T00:00:01Z"
    })).expect("fixed PUBLIC workspace metadata");
    app.workspaces = vec![workspace];
    assert!(app.management.baseline(vec![], vec![selected_id.clone()]));
    let records = if preset.starts_with("assistant-details") {
        let source=if preset=="assistant-details-fallback" {"# PUBLIC unsupported source\n\n![PUBLIC no image loaded](file:///PUBLIC/no-image.png)\n\n<p>PUBLIC HTML shown literally; never interpreted.</p>"}else{PUBLIC_ASSISTANT_DETAILS};
        vec![json!({"type":"event","event":{"type":"assistant/message","seq":0,"time":1001,"surfaceOp":"append","data":{"turn":1,"step":1,"stream":[{"type":"text-chunks","time0":1001,"index":0,"dt":[],"texts":[source]}]}}})]
    } else if preset.starts_with("tool-activity") || preset.starts_with("title-controls") {
        public_tool_records()
    } else if preset == "inbox-canceled" {
        vec![
            json!({"type":"event","event":{"type":"sandbox/mode","seq":0,"time":1000,"data":{"mode":"PUBLIC-render-only"}}}),
            json!({"type":"event","event":{"type":"agent/inbox/spliced","seq":1,"time":1001,"data":{"target":"next-turn","start":0,"removedCount":1,"inserted":[]}}}),
            json!({"type":"event","event":{"type":"agent/inbox/spliced","seq":2,"time":1002,"data":{"target":"next-turn","start":0,"removedCount":1,"inserted":[],"outcome":"canceled"}}}),
        ]
    } else if preset == "chat-density" {
        ["PUBLIC · Hello", "PUBLIC · Hi. How can I help?", "PUBLIC · Please check this function.", "PUBLIC · I'll show the file and command names in the tool disclosures.", "PUBLIC · café · 世界\nA short second line.\nKeep every stored record.\nFourth line stays visible.", "PUBLIC · This is synthetic text, not a model response. Details remain available without executing commands."].iter().enumerate().map(|(index,text)| {
            let seq = index as u64;
            if index % 2 == 0 {
                json!({"type":"event","event":{"type":"user/message","seq":seq,"time":1000+seq,"surfaceOp":"append","data":{"content":[{"type":"text","text":text}]}}})
            } else {
                json!({"type":"event","event":{"type":"assistant/message","seq":seq,"time":1000+seq,"surfaceOp":"append","data":{"turn":1,"step":1,"stream":[{"type":"text-chunks","time0":1000+seq,"index":0,"dt":[],"texts":[text]}]}}})
            }
        }).collect()
    } else if preset == "conversation" {
        vec![
            json!({"type":"event", "event":{"type":"user/message", "seq":0, "time":1000,
                "data":{"role":"user", "id":"PUBLIC-user-message", "source":{"kind":"human"},
                    "content":[{"type":"text", "text":"PUBLIC layout note · café · 世界\nCompare the real conversation bubbles and composer without sending a prompt."}]},
                "surfaceOp":"append"}}),
            json!({"type":"event", "event":{"type":"assistant/message", "seq":1, "time":1001,
                "data":{"turn":1, "step":1,
                    "stream":[{"type":"text-chunks", "time0":1001, "index":0, "dt":[],
                        "texts":["PUBLIC synthetic assistant text. This durable snapshot is local metadata, not a model response or a business acknowledgement."]}]},
                "surfaceOp":"append"}}),
        ]
    } else {
        vec![]
    };
    let cursor = records.len() as i64 - 1;
    let snapshot: SessionFollowFrame = serde_json::from_value(json!({
        "type":"snapshot", "records":records, "cursor":cursor, "hasMore":false,
        "header":{"version":4, "id":"PUBLIC-layout-conversation", "createdAt":1000, "isSeeded":false},
        "projections":{"asOfSeq":cursor, "values":{}}, "assistantStream":{"revision":0}
    })).expect("fixed PUBLIC valid follow snapshot");
    drop(app.update(Message::Worker(Event::Selected {
        generation: app.generation,
        frame: snapshot,
    })));
    assert!(app.follow_ready && app.smoke_evidence.snapshot_valid && app.transcript.safe_to_send());
    if preset.starts_with("title-controls") {
        for (seq,kind,data) in [
            (8,"session/title",json!({"title":"PUBLIC earlier title"})),
            (9,"session/title-llm-request",json!({"messages":[{"content":[{"type":"text","text":"PUBLIC opaque title prompt"}]}]})),
            (10,"session/title",json!({"title":"PUBLIC updated chat title"})),
        ] {
            let frame=serde_json::from_value(json!({"type":"event","event":{"type":kind,"seq":seq,"time":1000+seq,"data":data}})).unwrap();
            drop(app.update(Message::Worker(Event::Selected{generation:app.generation,frame})));
        }
        app.records_expanded=preset=="title-controls-expanded";
        assert_eq!(app.transcript.display_rows(app.records_expanded).count(),8);
        assert_eq!(app.session_title(app.sessions.get(&selected_id).unwrap()),"PUBLIC updated chat title");
    }
    if preset == "collapsed-wide" {
        app.sidebar_expanded = false;
    }
    drop(app.update(Message::Resized(size)));

    match preset {
        "file-path-review" | "file-path-tiny" | "file-ready" | "file-unknown" => {
            // Pure local render metadata only: never execute StageFile/PromptFile or read the path.
            let context=app.file_context();assert!(app.files.open(&context));let stamp=app.files.editor().unwrap().0;
            assert!(app.files.edit(stamp,"/PUBLIC-layout-fixture/PUBLIC review file.bin".into(),&context));
            if matches!(preset,"file-ready"|"file-unknown") {
                let stamp=app.files.editor().unwrap().0;let submission=app.files.stage(stamp,&context).unwrap();let ticket=submission.ticket.clone();drop(submission);
                app.files.complete(&ticket,worker::attachments::Outcome::Staged(worker::attachments::Metadata{name:"PUBLIC review file.bin".into(),bytes:5003}),&context);
                if preset=="file-unknown" {let request=dsh_native_transport::dto::SessionRequestId::new("PUBLIC-render-only-not-transmitted").unwrap();assert!(app.files.begin_prompt(&context,request.clone()).is_some());app.files.prompt_complete(&ticket,&request,worker::attachments::PromptOutcome::Unknown,&context);}
            }
        }
        "menu" => app.conversation_menu = true,
        "navigation" | "navigation-tiny" => app.sidebar_sheet = true,
        "settings-general" | "settings-codex" => {
            use crate::settings::{Action, Page};
            // Default ApiLogin would read metadata. Seed General locally with disabled effects,
            // then exercise actual App Open/navigation. No production defaults/guards change.
            assert!(
                app.settings
                    .handle(Action::Open, app.transport_epoch, false)
                    .is_none()
            );
            assert!(
                app.settings
                    .handle(
                        Action::SelectPage(Page::General),
                        app.transport_epoch,
                        false
                    )
                    .is_none()
            );
            app.settings.close();
            drop(app.update(Message::Settings(Action::Open)));
            let page = if preset == "settings-codex" {
                Page::Codex
            } else {
                Page::General
            };
            drop(app.update(Message::Settings(Action::SelectPage(page))));
            assert!(app.settings.is_open());
        }
        "information" | "information-expanded" => {
            drop(app.update(Message::Capabilities));
            if preset == "information-expanded" {
                drop(app.update(Message::ToggleInfoDetails(InfoSection::Features)));
                drop(app.update(Message::ToggleInfoDetails(InfoSection::Diagnostics)));
            }
            assert!(app.capabilities);
            assert_eq!(
                app.info_expanded,
                if preset == "information-expanded" {
                    3
                } else {
                    0
                }
            );
        }
        "assistant-details-source" | "assistant-details-formatted" | "assistant-details-narrow" | "assistant-details-fallback" => {
            drop(app.update(app.detail_request(0).unwrap()));
            if matches!(preset,"assistant-details-formatted"|"assistant-details-narrow") {
                let stamp=app.detail.as_ref().unwrap().ticket.unwrap();
                drop(app.update(Message::DetailMode(stamp,super::detail_panel::Mode::Formatted)));
                assert_eq!(app.detail.as_ref().unwrap().mode,super::detail_panel::Mode::Formatted);
            }
            assert!(app.detail.is_some() && !app.composer_visible());
        }
        "tool-activity-details" => {
            drop(app.update(app.detail_request(3).unwrap()));
            assert!(
                app.detail
                    .as_ref()
                    .is_some_and(|detail| detail.source.contains("PUBLIC old text"))
            );
        }
        "long-model" | "tiny-long-model" => {
            let effort = Effort {
                id: PUBLIC_EFFORT_ID.into(),
                name: "PUBLIC extended reasoning · raisonnement approfondi · 深入推理 · εκτενής συλλογισμός".into(),
            };
            let model = Model {
                provider: PUBLIC_PROVIDER_ID.into(), id: PUBLIC_MODEL_ID.into(),
                label: "PUBLIC multilingual model · modèle de démonstration très long · 多语言布局示例模型 · μοντέλο δοκιμής".into(),
                efforts: vec![effort.clone()], default_effort: Some(effort.id.clone()),
            };
            // Private fixture assignment only: no Catalog/SelectModel/Model ACK is simulated.
            let selection: ModelSelection = serde_json::from_value(json!({
                "provider":PUBLIC_PROVIDER_ID, "model":PUBLIC_MODEL_ID,
                "reasoningEffort":PUBLIC_EFFORT_ID
            }))
            .expect("fixed PUBLIC model metadata");
            app.model_selections.insert(selected_id, selection);
            app.models = vec![model.clone()];
            app.chosen_model = Some(model);
            app.chosen_effort = Some(effort);
        }
        "warnings" => {
            // A valid empty baseline followed by a PUBLIC local sequence gap exercises the
            // existing read-only warning. No invalid baseline, reducer/guard mutation or ACK.
            let gap: SessionFollowFrame = serde_json::from_value(json!({
                "type":"event", "event":{"type":"user/message", "seq":1, "time":1000,
                    "data":{"content":[{"type":"text", "text":"PUBLIC gap fixture · never accepted"}]},
                    "surfaceOp":"append"}
            })).expect("fixed PUBLIC sequence-gap frame");
            drop(app.update(Message::Worker(Event::Selected {
                generation: app.generation,
                frame: gap,
            })));
            assert!(!app.transcript.safe_to_send());
        }
        _ => {}
    }
    app.status = if preset == "warnings" {
        "PUBLIC local warning simulation · sequence gap after valid snapshot; read-only until reload · no Host exists".into()
    } else {
        "Live real session; UTC timestamps; no generation started".into()
    };
    (app, receiver)
}

/// Only known PUBLIC metadata and fixture-owned state; never serialize arbitrary App state.
pub fn metadata(app: &App, preset: &str) -> Value {
    let settings_expected = matches!(preset, "settings-general" | "settings-codex");
    let info_expected = matches!(preset, "information" | "information-expanded");
    let composer_expected = !settings_expected
        && !info_expected && !preset.starts_with("assistant-details")
        && !matches!(
            preset,
            "menu" | "navigation" | "navigation-tiny" | "tool-activity-details"
        );
    let reasoning_expected = matches!(preset, "long-model" | "tiny-long-model");
    let summaries: Vec<_> = app
        .transcript
        .rows()
        .iter()
        .filter_map(|row| app.transcript.activity(row.key))
        .collect();
    let mut report = json!({
        "publicSource":true,
        "scope":"embedded-native-actual-production-App-view-PUBLIC-local-metadata-simulation",
        "sourceScope":"cfg(feature=public-layout-fixture)-only; default production binary excludes helper",
        "renderedName":"PUBLIC local conversation",
        "mode":preset,
        "requestedAppFrameLogicalSize":[app.window_size.width, app.window_size.height],
        "applicationScale":app.options.scale,
        "validPublicSnapshotApplied":app.smoke_evidence.snapshot_valid,
        "registryReady":app.management.registry.ready(),
        "composerExpected":composer_expected,
        "composerStateMatchesPreset":app.composer_visible() == composer_expected,
        "settingsEntryExpected":!settings_expected,
        "stopExpected":!settings_expected,
        "modelControlsExpected":composer_expected,
        "reasoningControlExpected":reasoning_expected,
        "headerExpected":!settings_expected && !matches!(preset, "navigation" | "navigation-tiny"),
        "settingsStateMatchesPreset":app.settings.is_open() == settings_expected,
        "informationStateMatchesPreset":app.capabilities == info_expected
            && app.info_expanded == if preset == "information-expanded" { 3 } else { 0 },
        "informationExpandedBits":app.info_expanded,
        "settingsModalExpected":settings_expected,
        "settingsSeedWithoutEffects":settings_expected,
        "modalBackgroundOperationExcludedExpected":settings_expected,
        "reasoningStateMatchesPreset":app.chosen_model.as_ref().is_some_and(|m| !m.efforts.is_empty()) == reasoning_expected,
        "durablePublicRecordCount":app.transcript.rows().len(),
        "durablePublicSnapshotCursor":app.transcript.opening_cursor(),
        "conversationSnapshotSimulated":matches!(preset, "conversation" | "inbox-canceled") || preset.starts_with("tool-activity"),
        "displayedPublicRecordCount":app.transcript.display_rows(app.records_expanded).count(),
        "canceledInboxHeadingExpected":preset == "inbox-canceled",
        "publicToolSnapshotSimulated":preset.starts_with("tool-activity"),
        "publicToolSummaryCount":summaries.len(),
        "publicRecordedErrorMarkerCount":summaries.iter().filter(|summary| summary.failed).count(),
        "toolExecutionRequested":false,
        "toolExecutionOutcomeClaimed":false,
        "fileDraftMetadataSimulated":preset.starts_with("file-"),
        "fileUploadRequested":false,
        "filePromptRequested":false,
        "filePreviewQualified":false,
        "filePathReviewVisible":app.files.editor_open(),
        "fileDraftReady":app.files.ready().is_some(),
        "fileDraftBlocksSend":!app.files.can_send(&app.file_context())
    });
    let safety = json!({
        "assistantDetailsSnapshotSimulated":preset.starts_with("assistant-details"),
        "detailVisible":app.detail_visible(),
        "detailFormattedAvailable":app.detail.as_ref().is_some_and(|detail|detail.formatted_available()),
        "detailMode":app.detail.as_ref().map(|detail|match detail.mode {super::detail_panel::Mode::Formatted=>"formatted",super::detail_panel::Mode::Source=>"source"}),
        "detailSourceBytes":app.detail.as_ref().map(|detail|detail.source.len()),
        "detailsLinksOrImagesOpened":false,
        "detailsCodeExecuted":false,
        "detailsClipboardEffectRequested":false,
        "healthyRoutineStatusSimulated":preset != "warnings",
        "sidebarExpanded":app.sidebar_expanded,
        "sidebarSheet":app.sidebar_sheet,
        "conversationMenu":app.conversation_menu,
        "readOnlySequenceGapSimulated":preset == "warnings",
        "transcriptSafeToSend":app.transcript.safe_to_send(),
        "followReady":app.follow_ready,
        "publicSessionCount":app.sessions.len(),
        "publicModel":app.chosen_model.as_ref().map(|m| json!({"providerId":m.provider, "modelId":m.id, "displayLabel":m.label})),
        "publicEffort":app.chosen_effort.as_ref().map(|e| json!({"id":e.id, "displayLabel":e.name})),
        "backendStarted":false, "nodeStarted":false, "liveHost":false,
        "accountLookupIssued":false, "modelCatalogRequested":false, "modelPrompts":0,
        "modelSelectionOperationIssued":false, "businessAcknowledgementSimulated":false,
        "nativeKeyboardPointerInputQualified":false,
        "pixelGeometryQualified":false, "parentPaintInspectionRequired":true,
        "privatePathsRedacted":true
    });
    report
        .as_object_mut()
        .unwrap()
        .extend(safety.as_object().unwrap().clone());
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options() -> Options {
        Options {
            backend: dsh_native_core::RustBackendOptions {
                runtime: "/PUBLIC-layout-fixture/NEVER-START/runtime".into(),
                expected_version: "0.2.1-alpha.1".into(),
                native_home: "/PUBLIC-layout-fixture/NEVER-START/native-home".into(),
                user_home: "/PUBLIC-layout-fixture/NEVER-START/user-home".into(),
                working_directory: "/PUBLIC-layout-fixture/workspace".into(),
                absolute_node_path: None,
                startup_timeout: std::time::Duration::from_secs(1),
                stop_policy: Default::default(),
            },
            scale: 1.0,
            smoke: None,
        }
    }
    #[test]
    fn presets_render_actual_app_without_queued_effects() {
        for &preset in PRESETS {
            let (size, scale) = preset_size(preset).unwrap();
            let mut options = options();
            options.scale = scale;
            let (mut app, mut receiver) = build(options, preset, size);
            drop(app.view());
            assert!(
                receiver.try_recv().is_err(),
                "PUBLIC preset queued a Command"
            );
            let report = metadata(&app, preset);
            assert_eq!(report["validPublicSnapshotApplied"], true);
            assert_eq!(report["registryReady"], true);
            assert_eq!(
                report["requestedAppFrameLogicalSize"],
                json!([size.width, size.height])
            );
            assert_eq!(
                app.sidebar_sheet,
                matches!(preset, "navigation" | "navigation-tiny")
            );
            assert_eq!(app.conversation_menu, preset == "menu");
            assert_eq!(report["composerStateMatchesPreset"], true);
            assert_eq!(report["settingsStateMatchesPreset"], true);
            assert_eq!(report["informationStateMatchesPreset"], true);
            assert_eq!(
                app.settings.is_open(),
                matches!(preset, "settings-general" | "settings-codex")
            );
            assert_eq!(
                app.capabilities,
                matches!(preset, "information" | "information-expanded")
            );
            assert_eq!(
                app.info_expanded,
                if preset == "information-expanded" {
                    3
                } else {
                    0
                }
            );
            assert_eq!(app.sidebar_expanded, preset != "collapsed-wide");
            if preset == "conversation" {
                assert_eq!(app.transcript.rows().len(), 2);
                assert_eq!(app.transcript.opening_cursor(), 1);
                assert_eq!(app.transcript.rows()[0].role, "You");
                assert_eq!(app.transcript.rows()[1].role, "Assistant");
            }
            if preset == "inbox-canceled" {
                assert_eq!(report["conversationSnapshotSimulated"],true);
                assert_eq!(report["displayedPublicRecordCount"],1);
                assert_eq!(report["canceledInboxHeadingExpected"],true);
                assert_eq!(app.transcript.rows().len(),3);
                assert!(app.transcript.inbox_canceled(2));
                assert!(app.transcript.safe_to_send());
                drop(app.update(app.detail_request(2).unwrap()));
                assert!(app.detail.as_ref().unwrap().source.contains("canceled"));
                assert!(receiver.try_recv().is_err());
            }
            if preset.starts_with("tool-activity") {
                assert_eq!(app.transcript.rows().len(), 8);
                assert_eq!(app.transcript.opening_cursor(), 7);
                assert_eq!(app.transcript.rows()[0].role, "You");
                assert_eq!(app.transcript.rows()[7].role, "Assistant");
                assert_eq!(report["publicToolSummaryCount"], 6);
                assert_eq!(report["publicRecordedErrorMarkerCount"], 1);
                assert_eq!(
                    app.transcript.activity(1).unwrap().title,
                    "Read · functions.read"
                );
                assert_eq!(
                    app.transcript.activity(4).unwrap().title,
                    "Edit · functions.edit · result"
                );
                assert!(app.transcript.activity(6).unwrap().failed);
                // Programmatic local view action only: not physical input or execution proof.
                drop(app.update(app.detail_request(3).unwrap()));
                assert!(app.detail.as_ref().unwrap().source.contains("PUBLIC old text"));
                drop(app.update(app.detail_back().unwrap()));
                assert!(receiver.try_recv().is_err());
            }
            if preset == "warnings" {
                assert!(!app.follow_ready && !app.transcript.safe_to_send());
                assert!(app.transcript.rows().is_empty());
            } else {
                assert_eq!(
                    app.status,
                    "Live real session; UTC timestamps; no generation started"
                );
            }
        }
    }
    #[test]
    fn public_model_assignment_preserves_exact_ids_and_multilingual_labels() {
        let (app, mut receiver) = build(options(), "long-model", Size::new(760.0, 560.0));
        let model = app.chosen_model.as_ref().unwrap();
        assert_eq!(model.id, PUBLIC_MODEL_ID);
        assert_eq!(model.provider, PUBLIC_PROVIDER_ID);
        assert!(model.label.contains("PUBLIC") && model.label.contains("多语言"));
        assert_eq!(app.chosen_effort.as_ref().unwrap().id, PUBLIC_EFFORT_ID);
        let selection = app
            .model_selections
            .get(app.selected.as_ref().unwrap())
            .unwrap();
        assert_eq!(selection.model, PUBLIC_MODEL_ID);
        assert_eq!(
            selection.reasoning_effort.as_deref(),
            Some(PUBLIC_EFFORT_ID)
        );
        assert!(receiver.try_recv().is_err());
    }
}
