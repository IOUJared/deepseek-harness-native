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
#[path = "cloudflare_fixture.rs"]
mod cloudflare;
#[path = "managed_accounts_fixture.rs"]
mod managed_accounts;
#[path = "runtime_preferences_fixture.rs"]
mod runtime_preferences;
#[path = "usage_preferences_fixture.rs"]
mod usage_preferences;

pub const PRESETS: &[&str] = &[
    "no-session",
    "subagent-trail",
    "hero-workspace-selected",
    "wide-welcome",
    "narrow-welcome",
    "tiny-welcome",
    "menu",
    "navigation",
    "long-model",
    "model-popup",
    "model-popup-tiny",
    "model-popup-empty",
    "model-popup-long",
    "model-submenu",
    "effort-submenu",
    "speed-submenu",
    "output-submenu",
    "model-submenu-tiny",
    "effort-submenu-tiny",
    "model-submenu-empty",
    "warnings",
    "conversation",
    "sidebar-options",
    "sidebar-options-tiny",
    "sidebar-trash-review",
    "sidebar-trash",
    "sidebar-moved",
    "title-controls",
    "title-controls-expanded",
    "model-metadata",
    "model-metadata-expanded",
    "working-conversation",
    "working-phase-6",
    "working-narrow",
    "chat-submitted",
    "chat-preparation-error",
    "chat-stream-live",
    "chat-context-rewrite",
    "long-user-message",
    "history-more",
    "chat-transcript",
    "chat-markdown",
    "chat-thoughts",
    "chat-edit-review",
    "chat-search",
    "chat-recovered",
    "chat-edited",
    "chat-search-narrow",
    "collapsed-wide",
    "tiny-long-model",
    "navigation-tiny",
    "settings-cloudflare-oauth-setup",
    "settings-cloudflare-oauth-waiting",
    "settings-cloudflare-oauth-connected",
    "settings-cloudflare-oauth-indeterminate",
    "settings-cloudflare-review",
    "settings-cloudflare-unknown",
    "settings-general-cloudflare-pending",
    "settings-general-cloudflare-unknown",
    "settings-general",
    "settings-models",
    "settings-codex",
    "settings-codex-managed",
    "settings-codex-usage",
    "settings-codex-usage-unknown",
    "settings-general-usage-unknown",
    "settings-general-usage-pending",
    "settings-codex-runtime-models",
    "settings-codex-runtime-images",
    "settings-codex-runtime-unknown",
    "settings-codex-review",
    "settings-codex-unknown",
    "settings-codex-models",
    "settings-codex-images",
    "settings-codex-maintenance",
    "settings-presets",
    "information",
    "information-expanded",
    "work-panel",
    "context-pressure",
    "context-pressure-risk",
    "context-management",
    "context-management-chat",
    "plan-chip",
    "command-menu",
    "plan-mode",
    "plan-mode-applying",
    "plan-mode-leaving",
    "tool-activity",
    "tool-activity-narrow",
    "tool-activity-tiny",
    "tool-activity-details",
    "tool-activity-expanded",
    "tool-activity-attachment",
    "tool-activity-edit-error",
    "context-injects",
    "assistant-tool-call",
    "assistant-tool-call-expanded",
    "context-injects-trajectory",
    "assistant-details-source",
    "assistant-details-formatted",
    "assistant-details-narrow",
    "assistant-details-fallback",
    "chat-density",
    "file-path-review",
    "file-path-tiny",
    "file-ready",
    "file-ready-two",
    "file-unknown",
    "inbox-canceled",
    "file-picker",
    "file-preview",
    "file-preview-narrow",
];
pub const PUBLIC_ASSISTANT_DETAILS: &str = "# PUBLIC native response\n\nThis is **bold**, *italic*, and `inline code` in a frozen PUBLIC display snapshot. No model or tool ran.\n\n## Readable steps\n\n1. Preserve the source.\n\n   PUBLIC continuation stays inside the first list item.\n\n2. Keep all controls local.\n   - No remote images.\n   - No code execution.\n\n```rust\n// PUBLIC example only; not executed.\nfn public_preview() {\n    let note = \"PUBLIC intentionally long code line demonstrates horizontal scrolling without wrapping or changing the preserved display source\";\n    println!(\"{note}\");\n}\n```\n\nPUBLIC content beyond the collapsed four-line chat preview remains visible here.\n\n> This is a read-only native presentation, not an approval or execution acknowledgement.\n\n[PUBLIC reference](https://example.invalid/PUBLIC) is inert text, not an opener.\n";
pub const PUBLIC_PROVIDER_ID: &str = "PUBLIC-layout-provider-NOT-A-ROUTE";
pub const PUBLIC_MODEL_ID: &str = "PUBLIC-layout-model-exact-id-世界-é-α";
pub const PUBLIC_EFFORT_ID: &str = "PUBLIC-layout-effort-exact-id-世界-é-α";

/// Requested App frame, not an independently measured compositor or pixel viewport.
pub fn preset_size(preset: &str) -> Option<(Size, f32)> {
    match preset {
        "sidebar-options" | "sidebar-trash-review" | "sidebar-trash" | "sidebar-moved" => {
            Some((Size::new(1040.0, 900.0), 1.0))
        }
        "sidebar-options-tiny" => Some((Size::new(608.0, 448.0), 1.25)),
        "file-picker" | "file-preview" => Some((Size::new(1040.0, 900.0), 1.0)),
        "file-preview-narrow" => Some((Size::new(760.0, 560.0), 1.0)),
        "settings-cloudflare-oauth-setup"
        | "settings-cloudflare-oauth-waiting"
        | "settings-cloudflare-oauth-connected"
        | "settings-cloudflare-oauth-indeterminate"
        | "settings-cloudflare-review"
        | "settings-cloudflare-unknown"
        | "settings-general-cloudflare-pending"
        | "settings-general-cloudflare-unknown"
        | "settings-models"
        | "settings-codex"
        | "settings-codex-managed"
        | "settings-codex-usage"
        | "settings-codex-usage-unknown"
        | "settings-general-usage-unknown"
        | "settings-general-usage-pending"
        | "settings-codex-runtime-models"
        | "settings-codex-runtime-images"
        | "settings-codex-runtime-unknown"
        | "settings-codex-review"
        | "settings-codex-unknown"
        | "settings-codex-models"
        | "settings-codex-images"
        | "settings-codex-maintenance"
        | "settings-presets" => Some((Size::new(1024.0, 900.0), 1.0)),
        "assistant-details-source"
        | "assistant-details-formatted"
        | "assistant-details-fallback" => Some((Size::new(1040.0, 900.0), 1.0)),
        "assistant-details-narrow" => Some((Size::new(760.0, 840.0), 1.0)),
        "chat-search-narrow" => Some((Size::new(760.0, 840.0), 1.5)),
        "tool-activity"
        | "tool-activity-expanded"
        | "tool-activity-attachment"
        | "tool-activity-edit-error" => Some((Size::new(1040.0, 1040.0), 1.0)),
        "context-injects" | "context-injects-trajectory" => Some((Size::new(1040.0, 900.0), 1.0)),
        "assistant-tool-call" | "assistant-tool-call-expanded" => {
            Some((Size::new(1040.0, 760.0), 1.0))
        }
        "chat-density" => Some((Size::new(1040.0, 1000.0), 1.0)),
        "tool-activity-narrow" => Some((Size::new(760.0, 840.0), 1.0)),
        "tool-activity-tiny" => Some((Size::new(608.0, 448.0), 1.25)),
        "no-session"
        | "wide-welcome"
        | "conversation"
        | "collapsed-wide"
        | "title-controls"
        | "title-controls-expanded"
        | "model-metadata"
        | "model-metadata-expanded"
        | "working-conversation"
        | "working-phase-6"
        | "subagent-trail"
        | "hero-workspace-selected" => Some((Size::new(1040.0, 700.0), 1.0)),
        "working-narrow" => Some((Size::new(760.0, 560.0), 1.0)),
        "chat-submitted"
        | "chat-preparation-error"
        | "chat-stream-live"
        | "chat-context-rewrite"
        | "history-more"
        | "long-user-message"
        | "chat-transcript"
        | "chat-markdown"
        | "chat-thoughts"
        | "chat-edit-review"
        | "chat-search"
        | "chat-recovered"
        | "chat-edited" => Some((Size::new(1040.0, 900.0), 1.0)),
        "model-popup"
        | "model-popup-empty"
        | "model-popup-long"
        | "model-submenu"
        | "effort-submenu"
        | "speed-submenu"
        | "output-submenu"
        | "model-submenu-empty" => Some((Size::new(1040.0, 700.0), 1.0)),
        "tiny-welcome"
        | "tiny-long-model"
        | "navigation-tiny"
        | "file-path-tiny"
        | "model-popup-tiny"
        | "model-submenu-tiny"
        | "effort-submenu-tiny" => Some((Size::new(608.0, 448.0), 1.25)),
        "narrow-welcome"
        | "menu"
        | "navigation"
        | "long-model"
        | "warnings"
        | "settings-general"
        | "information"
        | "information-expanded"
        | "work-panel"
        | "context-pressure"
        | "context-pressure-risk"
        | "context-management"
        | "context-management-chat"
        | "plan-chip"
        | "command-menu"
        | "plan-mode"
        | "plan-mode-applying"
        | "plan-mode-leaving"
        | "tool-activity-details"
        | "file-path-review"
        | "file-ready"
        | "file-ready-two"
        | "file-unknown"
        | "inbox-canceled" => Some((Size::new(760.0, 560.0), 1.0)),
        _ => None,
    }
}

/// Synthetic records for the paper-informed context presets: a real window with
/// real occupancy, the harness's own context-management events, the loop guard's
/// reminder inject, and one standing plan. No Host, model or tool ran.
fn public_context_pressure_records(preset: &str) -> Option<Vec<Value>> {
    if preset.starts_with("context-management") {
        return Some(vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                "data":{"source":{"kind":"user","rpcId":"PUBLIC-context-rpc","promptFingerprint":"PUBLIC-opaque"},
                    "content":[{"type":"text","text":"PUBLIC question for the context-management preset."}]},
                "surfaceOp":"append"}}),
            json!({"type":"event","event":{"type":"tool/call","seq":1,"time":1001,
                "data":{"callId":"PUBLIC-call","turn":1,"step":1,"name":"bash",
                    "arguments":"{\"command\":\"PUBLIC synthetic command\"}"}}}),
            json!({"type":"event","event":{"type":"tool/result","seq":2,"time":1002,
                "data":{"turn":1,"step":1,"message":{"role":"tool","toolCallId":"PUBLIC-call",
                    "source":{"kind":"tool","callId":"PUBLIC-call"},"isError":false,
                    "content":[{"type":"text","text":"PUBLIC synthetic tool output."}]}}}}),
            // Staged management: elision first, summarization second, both logged.
            json!({"type":"event","event":{"type":"compaction/prune","seq":3,"time":1003,
                "data":{"shadowedRange":{"start":1,"end":2},"shadowedSeqs":[1,2],"shadowedTokenCount":42000}}}),
            json!({"type":"event","event":{"type":"compaction/start","seq":4,"time":1004,
                "data":{"compactionId":"PUBLIC-compaction","turn":1}}}),
            json!({"type":"event","event":{"type":"compaction/summary","seq":5,"time":1005,
                "data":{"compactionId":"PUBLIC-compaction",
                    "summary":[{"type":"text","text":"PUBLIC synthetic summary body."}],
                    "shadowedRange":{"start":3,"end":4},"shadowedSeqs":[3,4],"shadowedTokenCount":96000,
                    "provider":"PUBLIC","model":"PUBLIC"}}}),
            json!({"type":"event","event":{"type":"compaction/end","seq":6,"time":1006,
                "data":{"compactionId":"PUBLIC-compaction","turn":1}}}),
            // The loop guard's own reminder, marked by its producer source.
            json!({"type":"event","event":{"type":"user/message","seq":7,"time":1007,
                "data":{"content":[{"type":"text","text":"PUBLIC synthetic loop-guard reminder."}],
                    "source":{"kind":"repeat-tool-reminder","form":"notice","summary":"bash × 8"}}}}),
        ]);
    }
    if preset == "command-menu" {
        return Some(vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                "data":{"source":{"kind":"user","rpcId":"PUBLIC-menu-rpc","promptFingerprint":"PUBLIC-opaque"},
                    "content":[{"type":"text","text":"PUBLIC question for the command-menu preset."}]},
                "surfaceOp":"append"}}),
        ]);
    }
    if preset.starts_with("plan-mode") {
        // The same retained log for all three states; only the Host's published
        // projection differs, because that is what the indicator reads.
        return Some(vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                "data":{"source":{"kind":"user","rpcId":"PUBLIC-plan-mode-rpc","promptFingerprint":"PUBLIC-opaque"},
                    "content":[{"type":"text","text":"PUBLIC question for the plan-mode preset."}]},
                "surfaceOp":"append"}}),
            json!({"type":"event","event":{"type":"plan/mode","seq":1,"time":1001,
                "data":{"active":!preset.ends_with("leaving")}}}),
            json!({"type":"event","event":{"type":"todo/write","seq":2,"time":1002,
            "data":{"todos":[
                {"content":"PUBLIC explore the repository","status":"completed"},
                {"content":"PUBLIC write the plan","status":"in_progress"}
            ]}}}),
        ]);
    }
    if preset == "plan-chip" {
        return Some(vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                "data":{"source":{"kind":"user","rpcId":"PUBLIC-plan-rpc","promptFingerprint":"PUBLIC-opaque"},
                    "content":[{"type":"text","text":"PUBLIC question for the plan preset."}]},
                "surfaceOp":"append"}}),
            json!({"type":"event","event":{"type":"todo/write","seq":1,"time":1001,
            "data":{"todos":[
                {"content":"PUBLIC read the reducer","status":"completed"},
                {"content":"PUBLIC fix the fold","status":"in_progress"},
                {"content":"PUBLIC run the suite","status":"pending"},
                {"content":"PUBLIC write the note","status":"pending"}
            ]}}}),
        ]);
    }
    let (used, _capacity) = match preset {
        "context-pressure" => (108_000, 128_000),
        "context-pressure-risk" => (131_072, 128_000),
        _ => return None,
    };
    Some(vec![
        json!({"type":"event","event":{"type":"request/context","seq":0,"time":1000,
            "data":{"provider":"PUBLIC","model":"PUBLIC"}}}),
        json!({"type":"event","event":{"type":"assistant/message","seq":1,"time":1001,
            "data":{"turn":1,"step":1,
                "message":{"id":"PUBLIC-context","role":"assistant","source":{"kind":"model","provider":"PUBLIC","model":"PUBLIC"},
                    "content":[{"type":"text","text":"PUBLIC synthetic assistant text for the context preset."}]},
                "usage":{"inputTokens":used,"outputTokens":12,"cacheReadTokens":0,"cacheWriteTokens":0,"totalTokens":used+12},
                "stream":[{"type":"text-chunks","time0":1001,"index":0,"dt":[],
                    "texts":["PUBLIC synthetic assistant text for the context preset."]}]},
            "surfaceOp":"append"}}),
    ])
}

fn public_chat_records(preset: &str) -> Vec<Value> {
    if let Some(records) = public_context_pressure_records(preset) {
        return records;
    }
    if preset == "work-panel" {
        // Synthetic read-only work records: 15 goals and 12 schedules plus the
        // todo/subagent facts stay under both caps, so one capture shows every
        // section. The per-section overflow text is unit-tested separately.
        let mut records = Vec::new();
        for index in 0..15u64 {
            let phase = if index % 3 == 0 {
                "active"
            } else if index % 3 == 1 {
                "paused"
            } else {
                "complete"
            };
            records.push(json!({"type":"event","event":{"type":"goal/change","seq":index,"time":1000+index,"data":{
                "kind":"goal/change","version":1,"operation":"create",
                "goal":{"id":format!("PUBLIC-goal-{index}"),"revision":1,"objective":format!("PUBLIC synthetic goal {index}"),"phase":phase,"maxGoalRounds":4},
                "roundsStarted":index,"createdAt":1000,"updatedAt":1000}}}));
        }
        records.push(json!({"type":"event","event":{"type":"todo/write","seq":15,"time":1100,"data":{"todos":[
            {"content":"PUBLIC synthetic todo pending","status":"pending"},
            {"content":"PUBLIC synthetic todo in progress","status":"in_progress"},
            {"content":"PUBLIC synthetic todo completed","status":"completed"}]}}}));
        records.push(json!({"type":"event","event":{"type":"subagent/catalog","seq":16,"time":1101,"data":{"version":1,"childId":"PUBLIC-child","childCreatedAt":1000,"mode":"continuable","label":"PUBLIC synthetic child"}}}));
        records.push(json!({"type":"event","event":{"type":"subagent/descriptor","seq":17,"time":1102,"data":{"version":3,"provider":"PUBLIC-provider","mode":"continuable","label":"PUBLIC synthetic child","agentProvider":"PUBLIC-agent-provider","agentModel":"PUBLIC-agent-model"}}}));
        for index in 0..12u64 {
            let seq = 18 + index;
            records.push(json!({"type":"event","event":{"type":"schedule/change","seq":seq,"time":1200+index,"data":{
                "version":1,"operation":"create","schedule":{
                    "id":format!("PUBLIC-schedule-{index}"),"kind":"every","prompt":format!("PUBLIC synthetic schedule {index}"),
                    "everySeconds":300,"scheduledAt":"2026-08-31T00:00:00.000Z"}}}}));
        }
        return records;
    }
    if matches!(
        preset,
        "chat-markdown"
            | "chat-thoughts"
            | "chat-edit-review"
            | "chat-search"
            | "chat-recovered"
            | "chat-edited"
            | "chat-search-narrow"
    ) {
        return vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,"surfaceOp":"append","data":{"id":"PUBLIC-user","role":"user","source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC show a native formatted response."}]}}}),
            json!({"type":"event","event":{"type":"assistant/message","seq":1,"time":1001,"surfaceOp":"append","data":{"turn":1,"step":1,"usage":{"inputTokens":100,"cacheReadTokens":40,"cacheWriteTokens":10,"outputTokens":20,"reasoningTokens":8,"totalTokens":170},"stream":[
                {"type":"reasoning-chunks","time0":1001,"index":0,"dt":[],"texts":["PUBLIC backend-supplied thoughts. This disclosure is separate from the final response. No model ran."]},
                {"type":"text-chunks","time0":1001,"index":1,"dt":[],"texts":["## Native Markdown\n\n**Bold**, *italic*, and `inline code`.\n\n- PUBLIC native text\n- No webview or code execution\n\n```rust\n// PUBLIC example; not executed.\nfn main() {\n    let answer = 42;\n    println!(\"{answer}\");\n}\n```"]}
            ]}}}),
        ];
    }
    if matches!(
        preset,
        "chat-transcript" | "chat-context-rewrite" | "history-more" | "long-user-message"
    ) {
        let value: Value =
            serde_json::from_str(include_str!("../fixtures/chat-visibility.json")).unwrap();
        let mut records = value["records"].as_array().unwrap().clone();
        if preset == "long-user-message" {
            // The fixture's only user text is 22 characters, which never reaches
            // the reference's share-of-column bubble cap; this preset lengthens it
            // so a capture measures the cap rather than the content width.
            for record in &mut records {
                let event = &mut record["event"];
                if event["type"] == "user/message" {
                    event["data"]["content"] = json!([{
                        "type": "text",
                        "text": "PUBLIC long user input for the bubble cap: this sentence is deliberately far wider than the reference's 0.702 share of the conversation column, so the bubble must clamp to that share instead of growing with its own text."
                    }]);
                }
            }
        }
        if preset == "chat-context-rewrite" {
            records.push(json!({"type":"event","event":{"type":"user/message","seq":15,"time":1015,"surfaceOp":{"op":"replace","startSeq":8,"endSeq":13},"sourceEventSeqs":[8,11,13],"data":{"content":[{"type":"text","text":"PUBLIC model-only context summary: not a chat replacement"}]}}}));
        }
        return records;
    }
    let mut records = vec![
        json!({"type":"event","event":{"type":"model/selection","seq":0,"time":1000,"data":{"provider":"PUBLIC","model":"PUBLIC"}}}),
        json!({"type":"event","event":{"type":"session/title","seq":1,"time":1001,"data":{"title":"PUBLIC submitted conversation"}}}),
        json!({"type":"event","event":{"type":"agent/inbox/spliced","seq":2,"time":1002,"data":{"target":"next-turn","start":0,"inserted":[{"id":"PUBLIC-user","role":"user","source":{"kind":"user","rpcId":"PUBLIC-request","promptFingerprint":"PUBLIC-opaque"},"content":[{"type":"text","text":"PUBLIC submitted message stays in chat while the agent prepares its response."}]}]}}}),
    ];
    if preset == "chat-preparation-error" {
        records.extend([
            json!({"type":"event","event":{"type":"turn/start","seq":3,"time":1003,"data":{"turn":1}}}),
            json!({"type":"event","event":{"type":"agent/inbox/spliced","seq":4,"time":1004,"data":{"target":"next-turn","start":0,"removedCount":1,"inserted":[]}}}),
            json!({"type":"event","event":{"type":"turn/end","seq":5,"time":1005,"data":{"turn":1,"reason":{"kind":"error","error":{"code":"PUBLIC_ROUTE","message":"PUBLIC request preparation failed. The submitted message remains visible; no model ran."}}}}}),
        ]);
    }
    records
}

fn public_tool_records() -> Vec<Value> {
    public_tool_records_with(None)
}

/// The tool fixture with its Edit result recorded as an error, so the reference's
/// rule that a failed call drops its diff totals (`ToolRow.tsx:185-190`) has
/// fixture coverage. Only the recorded error flag and body change; no file was
/// written and no tool ran.
fn public_tool_edit_error_records() -> Vec<Value> {
    let mut records = public_tool_records();
    for record in &mut records {
        let event = &mut record["event"];
        if event["type"] == "tool/result"
            && event["data"]["message"]["toolCallId"] == "PUBLIC-edit-call"
        {
            event["data"]["message"]["isError"] = json!(true);
            event["data"]["message"]["content"] = json!([{
                "type":"text",
                "text":"PUBLIC synthetic edit failure. No file was written."
            }]);
        }
    }
    records
}

/// The same records with an optional `file` content part on the first user
/// message, so a preset can render the attachment card the reference stacks
/// above the bubble (`MessageItem.tsx:194-221`). A macro body cannot hold the
/// conditional, so the parts are built first and interpolated.
fn public_tool_records_with(attachment: Option<&str>) -> Vec<Value> {
    // Alpha tool arguments are RAW JSON STRINGS. All paths, command text and results are
    // synthetic PUBLIC metadata: they assert neither tool execution nor a business ACK.
    let event = |seq: u64, kind: &str, data: Value| {
        json!({"type":"event", "event":{
            "type":kind, "seq":seq, "time":1000+seq, "data":data, "surfaceOp":"append"
        }})
    };
    let mut parts = vec![
        json!({"type":"text", "text":"PUBLIC conversation · compare user, compact tool activity and assistant. No tools were executed."}),
    ];
    if let Some(name) = attachment {
        parts.push(json!({"type":"file", "attachment":{"name":name, "bytes":12}}));
    }
    let mut records = vec![event(
        0,
        "user/message",
        json!({
            "role":"user", "id":"PUBLIC-activity-user", "source":{"kind":"user"}, "content":parts
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

/// The record shapes a real session stores: the system prompt, the harness
/// context injects, one human prompt, a delivery log and the assistant reply.
/// Every string is synthetic PUBLIC metadata; no model request was made.
fn public_context_records() -> Vec<Value> {
    let event = |seq: u64, kind: &str, data: Value| {
        json!({"type":"event", "event":{
            "type":kind, "seq":seq, "time":1000+seq, "data":data, "surfaceOp":"append"
        }})
    };
    let injected = |seq: u64, kind: &str, form: &str, text: &str| {
        event(
            seq,
            "user/message",
            json!({
                "role":"user",
                "source":{"kind":kind, "form":form},
                "content":[{"type":"text","text":text}]
            }),
        )
    };
    vec![
        event(
            0,
            "system/message",
            json!({"message":{"role":"system","content":[{"type":"text","text":"PUBLIC system prompt · You are an AI agent powered by DeepSeek Harness."}]}}),
        ),
        injected(
            1,
            "runtime-context",
            "snapshot",
            "Current runtime context. This snapshot supersedes earlier runtime-context snapshots.\n\nPUBLIC file policy line.",
        ),
        injected(
            2,
            "time-context",
            "snapshot",
            "Time sampled while preparing turn 1, step 1: PUBLIC timestamp[UTC]",
        ),
        injected(
            3,
            "skill-catalog",
            "catalog",
            "<system-reminder>\nA skill is a reusable set of task-specific instructions.\n</system-reminder>",
        ),
        event(
            4,
            "user/message",
            json!({
                "role":"user",
                "source":{"kind":"user","rpcId":"PUBLIC-rpc","promptFingerprint":"PUBLIC-fingerprint"},
                "content":[{"type":"text","text":"PUBLIC real question · only this stays in Chat with the answer."}]
            }),
        ),
        event(
            5,
            "session-log-deepseek/delivery-accepted",
            json!({"sessionFormatVersion":4,"sessionId":"PUBLIC-session","throughSeq":15}),
        ),
        event(
            6,
            "assistant/message",
            json!({"turn":1,"step":1,
                "stream":[{"type":"text-chunks","time0":1006,"index":0,"dt":[],
                    "texts":["PUBLIC assistant answer · the conversation this app actually shows."]}]
            }),
        ),
    ]
}

/// One real session's settled assistant stream: thinking and a Bash tool call,
/// with no answer prose at all. The tool call must group with the thinking, never render as
/// prose. Built on the known-good tool fixture so only the stream differs.
/// Every string is synthetic PUBLIC metadata; nothing was executed.
fn public_tool_call_records() -> Vec<Value> {
    let mut records = public_tool_records();
    // The last record is the settled assistant message (seq 7).
    if let Some(last) = records.last_mut() {
        *last = json!({"type":"event", "event":{
            "type":"assistant/message", "seq":7, "time":1007, "surfaceOp":"append",
            "data":{"turn":1, "step":2, "stream":[
                {"type":"reasoning-chunks", "time0":1000, "index":0, "dt":[], "texts":["PUBLIC thinking about the request."]},
                {"type":"tool-call-chunks", "time0":1001, "index":1, "id":"PUBLIC-bash-call", "name":"bash", "dt":[],
                    "args":["{\"description\":\"Show current date and time\",\"command\":\"date -u\"}"]}
            ]}
        }});
    }
    records
}

/// The receiver MUST be retained, inspected and drained locally; never run its Commands.
/// Tasks returned by boot/local setup are discarded; the screenshot runner owns its tasks.
pub fn build(
    mut options: Options,
    preset: &str,
    size: Size,
    content_size: Option<f32>,
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
    // A fixture may render the transcript at any content size the control offers,
    // so a scaled run is reproducible without writing a preference file.
    if let Some(content_size) = content_size {
        app.set_content_font_size(content_size);
    }
    app.ready = true;
    app.root_ready = true;
    app.roster_pending = false;
    app.transport_epoch = worker::TRANSPORT_EPOCH;
    app.account = "PUBLIC local metadata · no account lookup".into();

    let selected: SessionSummary = serde_json::from_value(json!({
        "agentAvailable":true, "sessionId":"PUBLIC-layout-conversation", "updatedAt":1000,
        "running":preset.starts_with("working-") || matches!(preset, "menu" | "navigation" | "navigation-tiny"),
        "blank":preset != "conversation" && preset != "chat-density" && !preset.starts_with("tool-activity"),
        "cwd":"/PUBLIC-layout-fixture/workspace",
        "projections":{"kind":"cached", "asOfSeq":-1,
            "values":{"title":"PUBLIC local conversation"}}
    }))
    .expect("fixed PUBLIC session metadata");
    let selected_id = selected.session_id.clone();
    // `no-session` renders the empty conversation state: the header has no tabs,
    // which is the branch ConversationRoot.module.css:27-30 describes.
    if preset != "no-session" {
        app.selected = Some(selected_id.clone());
    }
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
    if preset == "subagent-trail" {
        // A subagent child under its parent, so the header's breadcrumb trail
        // renders an ancestor crumb and its `/` separator
        // (`ConversationSession.tsx:25-43`).
        let parent: SessionSummary = serde_json::from_value(json!({
            "agentAvailable":true, "sessionId":"PUBLIC-layout-parent", "updatedAt":950,
            "running":false, "blank":false, "cwd":"/PUBLIC-layout-fixture/workspace",
            "projections":{"kind":"cached","asOfSeq":-1,
                "values":{"title":"PUBLIC parent conversation"}}
        }))
        .expect("fixed PUBLIC parent metadata");
        app.sessions.insert(parent.session_id.clone(), parent);
        let child: SessionSummary = serde_json::from_value(json!({
            "agentAvailable":true, "sessionId":"PUBLIC-layout-child", "updatedAt":960,
            "running":false, "blank":false,
            "parentSessionId":"PUBLIC-layout-parent", "origin":"subagent",
            "cwd":"/PUBLIC-layout-fixture/workspace",
            "projections":{"kind":"cached","asOfSeq":-1,
                "values":{"title":"PUBLIC subagent child"}}
        }))
        .expect("fixed PUBLIC child metadata");
        app.selected = Some(child.session_id.clone());
        app.sessions.insert(child.session_id.clone(), child);
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
    if preset == "hero-workspace-selected" {
        // The hero with a chosen workspace, so the chip renders its labelled
        // form instead of the `Choose workspace` placeholder.
        app.workspace = Some(workspace.workspace_id.clone());
    }
    app.workspaces = vec![workspace];
    assert!(app.management.baseline(vec![], vec![selected_id.clone()]));
    let records = if preset.starts_with("assistant-details") {
        let source = if preset == "assistant-details-fallback" {
            "# PUBLIC unsupported source\n\n![PUBLIC no image loaded](file:///PUBLIC/no-image.png)\n\n<p>PUBLIC HTML shown literally; never interpreted.</p>"
        } else {
            PUBLIC_ASSISTANT_DETAILS
        };
        vec![
            json!({"type":"event","event":{"type":"assistant/message","seq":0,"time":1001,"surfaceOp":"append","data":{"turn":1,"step":1,"stream":[{"type":"text-chunks","time0":1001,"index":0,"dt":[],"texts":[source]}]}}}),
        ]
    } else if preset == "tool-activity-attachment" {
        public_tool_records_with(Some("PUBLIC-note.txt"))
    } else if preset == "tool-activity-edit-error" {
        public_tool_edit_error_records()
    } else if preset.starts_with("tool-activity") || preset.starts_with("title-controls") {
        public_tool_records()
    } else if preset.starts_with("assistant-tool-call") {
        public_tool_call_records()
    } else if preset.starts_with("context-injects") {
        public_context_records()
    } else if preset.starts_with("model-metadata") {
        let mut records = public_tool_records();
        for seq in 8..10 {
            records.push(json!({"type":"event","event":{"type":"model/selection","seq":seq,"time":1000+seq,"data":{"provider":PUBLIC_PROVIDER_ID,"model":PUBLIC_MODEL_ID,"reasoningEffort":PUBLIC_EFFORT_ID}}}));
        }
        records
    } else if matches!(
        preset,
        "chat-submitted"
            | "chat-preparation-error"
            | "chat-stream-live"
            | "chat-context-rewrite"
            | "history-more"
            | "long-user-message"
            | "chat-transcript"
            | "chat-markdown"
            | "chat-thoughts"
            | "chat-edit-review"
            | "chat-search"
            | "chat-recovered"
            | "chat-edited"
            | "chat-search-narrow"
            | "work-panel"
    ) || preset.starts_with("context-")
        || preset.starts_with("plan-mode")
        || preset == "plan-chip"
    {
        public_chat_records(preset)
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
    } else if preset == "conversation" || preset.starts_with("working-") {
        vec![
            json!({"type":"event", "event":{"type":"user/message", "seq":0, "time":1000,
                "data":{"role":"user", "id":"PUBLIC-user-message", "source":{"kind":"user"},
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
    // The context presets carry a real window and occupancy, because the
    // composer's tier reads only Host-reported projections.
    let projection_values = match preset {
        "context-pressure" => json!({"contextPressure":{"pressureTokens":108000,
            "projectedTokens":108000,"contextWindow":128000}}),
        "context-pressure-risk" => json!({"contextPressure":{"pressureTokens":131072,
            "projectedTokens":131072,"contextWindow":128000}}),
        "plan-mode" => json!({"plan":{"active":true,"pending":false}}),
        "plan-mode-applying" => json!({"plan":{"active":true,"pending":true}}),
        "plan-mode-leaving" => json!({"plan":{"active":false,"pending":true}}),
        _ => json!({}),
    };
    let snapshot: SessionFollowFrame = serde_json::from_value(json!({
        "type":"snapshot", "records":records, "cursor":cursor,
        "hasMore":preset == "history-more",
        "header":{"version":4, "id":"PUBLIC-layout-conversation", "createdAt":1000, "isSeeded":false},
        "projections":{"asOfSeq":cursor, "values":projection_values}, "assistantStream":{"revision":0}
    })).expect("fixed PUBLIC valid follow snapshot");
    drop(app.update(Message::Worker(Event::Selected {
        generation: app.generation,
        frame: snapshot,
    })));
    assert!(app.follow_ready && app.smoke_evidence.snapshot_valid && app.transcript.safe_to_send());
    if preset == "assistant-tool-call-expanded" {
        let ticket = app.reply_parts(7).unwrap().ticket.unwrap();
        drop(app.update(Message::ToggleThoughts(ticket)));
        assert!(app.thoughts_expanded.contains(&7));
    }
    if preset == "chat-thoughts" {
        let ticket = app.reply_parts(1).unwrap().ticket.unwrap();
        drop(app.update(Message::ToggleThoughts(ticket)));
        assert!(app.thoughts_expanded.contains(&1));
    }
    if preset == "chat-edit-review" {
        app.editor = iced::widget::text_editor::Content::with_text(
            "PUBLIC unsent draft preserved until explicit replacement.",
        );
        drop(app.update(Message::EditReply {
            generation: app.generation,
            key: 0,
        }));
        assert!(app.edit_review.is_some());
    }
    if matches!(preset, "chat-search" | "chat-search-narrow") {
        drop(app.update(Message::SearchOpen));
        drop(app.update(Message::SearchInput {
            generation: app.generation,
            serial: app.search_serial,
            query: "native".into(),
        }));
        assert_eq!(app.search_hits(), vec![0, 1]);
    }
    if preset == "chat-recovered" {
        // PUBLIC callback projection only: actual private Store IO is separately unit-tested.
        let ticket = app.draft_state.start_load().unwrap();
        let session = app.selected.clone().unwrap();
        drop(
            app.update(Message::DraftOutcome(
                super::draft_recovery::Outcome::Loaded {
                    ticket,
                    result: Ok([(
                        session,
                        "PUBLIC recovered unsent text — never automatically sent.".into(),
                    )]
                    .into()),
                },
            )),
        );
        assert!(app.editor.text().contains("recovered unsent"));
    }
    if preset == "chat-edited" {
        app.editor =
            iced::widget::text_editor::Content::with_text("PUBLIC replace only after consent.");
        drop(app.update(Message::EditReply {
            generation: app.generation,
            key: 0,
        }));
        let (generation, key, draft_serial) = app.edit_review.unwrap();
        drop(app.update(Message::ConfirmEdit {
            generation,
            key,
            draft_serial,
        }));
        assert_eq!(
            app.editor.text(),
            "PUBLIC show a native formatted response."
        );
    }
    if preset == "chat-stream-live" {
        for value in [
            json!({"type":"assistant-stream","frame":{"type":"start","revision":1,"attemptId":"PUBLIC-live","startedAfterSeq":2,"turn":1,"step":1}}),
            json!({"type":"assistant-stream","frame":{"type":"chunk","revision":2,"attemptId":"PUBLIC-live","index":0,"time":1003,"chunk":{"type":"reasoning-delta","index":0,"text":"PUBLIC backend-supplied reasoning: checking the conversation before answering."}}}),
            json!({"type":"assistant-stream","frame":{"type":"chunk","revision":3,"attemptId":"PUBLIC-live","index":1,"time":1004,"chunk":{"type":"text-delta","index":1,"text":"PUBLIC streamed assistant output appears here as it arrives. This is synthetic fixture data, not a live model response."}}}),
        ] {
            let frame = serde_json::from_value(value).unwrap();
            drop(app.update(Message::Worker(Event::Selected {
                generation: app.generation,
                frame,
            })));
        }
        assert!(app.transcript.partial().is_some());
    }
    if preset.starts_with("title-controls") {
        for (seq, kind, data) in [
            (8, "session/title", json!({"title":"PUBLIC earlier title"})),
            (
                9,
                "session/title-llm-request",
                json!({"messages":[{"content":[{"type":"text","text":"PUBLIC opaque title prompt"}]}]}),
            ),
            (
                10,
                "session/title",
                json!({"title":"PUBLIC updated chat title"}),
            ),
        ] {
            let frame = serde_json::from_value(
                json!({"type":"event","event":{"type":kind,"seq":seq,"time":1000+seq,"data":data}}),
            )
            .unwrap();
            drop(app.update(Message::Worker(Event::Selected {
                generation: app.generation,
                frame,
            })));
        }
        app.records_expanded = preset == "title-controls-expanded";
        assert_eq!(app.transcript.display_rows(app.records_expanded).count(), 8);
        assert_eq!(
            app.session_title(app.sessions.get(&selected_id).unwrap()),
            "PUBLIC updated chat title"
        );
    }
    if preset == "collapsed-wide" {
        app.sidebar_expanded = false;
    }
    drop(app.update(Message::Resized(size)));

    match preset {
        "file-picker" | "file-preview" | "file-preview-narrow" => {
            app.public_attachment_fixture(preset == "file-picker")
        }
        "file-path-review" | "file-path-tiny" | "file-ready" | "file-ready-two"
        | "file-unknown" => {
            // Pure local render metadata only: never execute StageFile/PromptFile or read the path.
            let context = app.file_context();
            assert!(app.files.open(&context));
            let stamp = app.files.editor().unwrap().0;
            assert!(app.files.edit(
                stamp,
                "/PUBLIC-layout-fixture/PUBLIC review file.bin".into(),
                &context
            ));
            if matches!(preset, "file-ready" | "file-ready-two" | "file-unknown") {
                let stamp = app.files.editor().unwrap().0;
                let submission = app.files.stage(stamp, &context).unwrap();
                let ticket = submission.ticket.clone();
                drop(submission);
                app.files.complete(
                    &ticket,
                    worker::attachments::Outcome::Staged(worker::attachments::Metadata {
                        name: "PUBLIC review file.bin".into(),
                        bytes: 5003,
                    }),
                    &context,
                );
                if preset == "file-ready-two" {
                    // Second attachment proves the bounded set renders and stages independently.
                    let context = app.file_context();
                    assert!(app.files.open(&context));
                    let stamp = app.files.editor().unwrap().0;
                    assert!(app.files.edit(
                        stamp,
                        "/PUBLIC-layout-fixture/PUBLIC second file.txt".into(),
                        &context
                    ));
                    let stamp = app.files.editor().unwrap().0;
                    let submission = app.files.stage(stamp, &context).unwrap();
                    let ticket = submission.ticket.clone();
                    drop(submission);
                    app.files.complete(
                        &ticket,
                        worker::attachments::Outcome::Staged(worker::attachments::Metadata {
                            name: "PUBLIC second file.txt".into(),
                            bytes: 2048,
                        }),
                        &context,
                    );
                }
                if preset == "file-unknown" {
                    let request = dsh_native_transport::dto::SessionRequestId::new(
                        "PUBLIC-render-only-not-transmitted",
                    )
                    .unwrap();
                    assert!(app.files.begin_prompt(&context, request.clone()).is_some());
                    app.files.prompt_complete(
                        &ticket,
                        &request,
                        worker::attachments::PromptOutcome::Unknown,
                        &context,
                    );
                }
            }
        }
        "menu" => app.conversation_menu = true,
        "navigation" | "navigation-tiny" => app.sidebar_sheet = true,
        "settings-cloudflare-oauth-setup"
        | "settings-cloudflare-oauth-waiting"
        | "settings-cloudflare-oauth-connected"
        | "settings-cloudflare-oauth-indeterminate"
        | "settings-cloudflare-review"
        | "settings-cloudflare-unknown"
        | "settings-general-cloudflare-pending"
        | "settings-general-cloudflare-unknown"
        | "settings-general"
        | "settings-models"
        | "settings-codex"
        | "settings-codex-managed"
        | "settings-codex-usage"
        | "settings-codex-usage-unknown"
        | "settings-general-usage-unknown"
        | "settings-general-usage-pending"
        | "settings-codex-runtime-models"
        | "settings-codex-runtime-images"
        | "settings-codex-runtime-unknown"
        | "settings-codex-review"
        | "settings-codex-unknown"
        | "settings-codex-models"
        | "settings-codex-images"
        | "settings-codex-maintenance"
        | "settings-presets" => {
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
            let page = if preset.starts_with("settings-cloudflare")
                || preset.starts_with("settings-general-cloudflare")
            {
                Page::Cloudflare
            } else if preset.starts_with("settings-codex")
                || preset.starts_with("settings-general-usage")
            {
                Page::Codex
            } else if preset == "settings-presets" {
                Page::AgentPresets
            } else if preset == "settings-models" {
                Page::ApiLogin
            } else {
                Page::General
            };
            drop(app.update(Message::Settings(Action::SelectPage(page))));
            if preset == "settings-models" {
                // Exercise a real read request and supply its reply locally, so
                // the Models page renders its ready state without a Host.
                let ticket = app.settings.ticket();
                if app
                    .settings
                    .handle(Action::Refresh(ticket), app.transport_epoch, true)
                    .is_some()
                {
                    let reading = app.settings.ticket();
                    app.settings.metadata(
                        reading,
                        Ok(dsh_native_core::OnboardingMetadata {
                            logged_in: false,
                            has_api_key: false,
                            writable: true,
                        }),
                    );
                }
                // PUBLIC DeepSeek catalog rows: the page renders real route
                // facts instead of an invented endpoint or model.
                let row = |id: &str, name: &str| Model {
                    provider: "deepseek-official".into(),
                    id: id.into(),
                    name: name.into(),
                    label: name.into(),
                    efforts: Vec::new(),
                    default_effort: None,
                    service_tiers: Vec::new(),
                    verbosity_levels: Vec::new(),
                };
                app.models = vec![
                    row("deepseek-flash", "DeepSeek-V41-Flash"),
                    row("deepseek-v4-pro", "DeepSeek-V4-Pro"),
                ];
                // PUBLIC live-settings snapshot for the one namespace the
                // DeepSeek page renders. No endpoint or credential is invented.
                use dsh_native_transport::plugin::{
                    PluginInventorySnapshot, SettingsDescribeValue, SettingsField,
                    SettingsFieldKind, SettingsNamespaceView, SettingsScalar,
                };
                let field = |path: &str, value: &str| SettingsField {
                    path: vec![path.into()],
                    label: path.into(),
                    kind: SettingsFieldKind::String { max_length: 200 },
                    value: Some(SettingsScalar::String(value.into())),
                    overridden: false,
                };
                let ticket = app.settings.plugin_read_ticket();
                app.settings.plugins_loaded(
                    ticket,
                    Ok(crate::plugins::Snapshot {
                        inventory: PluginInventorySnapshot {
                            entries: Vec::new(),
                            management_available: false,
                        },
                        settings: SettingsDescribeValue {
                            writable: true,
                            namespaces: vec![SettingsNamespaceView {
                                ns: "llm-deepseek".into(),
                                revision: 3,
                                auto_generate: true,
                                unsupported_fields: 1,
                                secret_fields: 0,
                                fields: vec![
                                    field("baseURL", "https://api.deepseek.com/anthropic"),
                                    field("reasoningEffort", "high"),
                                ],
                            }],
                        },
                    }),
                );
            }
            if preset.starts_with("settings-codex") || preset.starts_with("settings-general-usage")
            {
                use crate::codex::Tab;
                let tab = match preset {
                    "settings-codex-models"
                    | "settings-codex-runtime-models"
                    | "settings-codex-runtime-unknown" => Tab::Models,
                    "settings-codex-images" | "settings-codex-runtime-images" => Tab::Images,
                    "settings-codex-maintenance" => Tab::Maintenance,
                    _ => Tab::Account,
                };
                app.settings.layout_codex_tab(tab);
                if matches!(
                    preset,
                    "settings-codex-managed" | "settings-codex-review" | "settings-codex-unknown"
                ) {
                    managed_accounts::seed(&mut app, preset);
                }
                if matches!(
                    preset,
                    "settings-codex-usage"
                        | "settings-codex-usage-unknown"
                        | "settings-general-usage-unknown"
                        | "settings-general-usage-pending"
                ) {
                    managed_accounts::seed(&mut app, "settings-codex-managed");
                    usage_preferences::seed(&mut app);
                    if preset != "settings-codex-usage" {
                        usage_preferences::save(
                            &mut app,
                            preset != "settings-general-usage-pending",
                        );
                    }
                }
                if matches!(
                    preset,
                    "settings-codex-runtime-models"
                        | "settings-codex-runtime-images"
                        | "settings-codex-runtime-unknown"
                ) {
                    runtime_preferences::seed(&mut app);
                    if preset == "settings-codex-runtime-unknown" {
                        runtime_preferences::fail_save(&mut app);
                    }
                }
            }
            if preset.starts_with("settings-cloudflare")
                || preset.starts_with("settings-general-cloudflare")
            {
                cloudflare::seed(&mut app, preset);
                if preset.starts_with("settings-general-cloudflare") {
                    drop(app.update(Message::Settings(Action::SelectPage(Page::General))));
                }
            }
            if preset.starts_with("settings-general-usage") {
                drop(app.update(Message::Settings(Action::SelectPage(Page::General))));
            }
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
        // Chat is the default clean view; the trajectory variant shows the log,
        // exactly as the existing context-inject presets do.
        "context-management" => {
            app.records_expanded = true;
        }
        "context-management-chat" => {
            app.records_expanded = false;
        }
        "context-pressure" => {
            assert_eq!(app.transcript.work().plan(), None);
        }
        "context-pressure-risk" => {
            // Reaching the window is stated as well as tinted.
            drop(app.update(Message::ToggleContextPolicy));
            assert!(app.context_policy_open);
        }
        // Plan mode is reported only by the Host's projection; no control here
        // can change it, so the fixture asserts the presented label alone.
        "plan-mode" | "plan-mode-applying" | "plan-mode-leaving" => {
            let mode = super::plan_mode::mode(app.transcript.projections())
                .expect("PUBLIC plan projection");
            let expected = match preset {
                "plan-mode" => super::plan_mode::Mode::Active,
                "plan-mode-applying" => super::plan_mode::Mode::Applying,
                _ => super::plan_mode::Mode::Leaving,
            };
            assert_eq!(mode, expected);
        }
        "command-menu" => {
            // The composer asked for the advertisement when the slash was typed;
            // this stands in for the Host's own reply, which is the only source.
            app.commands = Some(vec![
                super::command_menu::Candidate {
                    name: "plan".into(),
                    description: "Explore and design before executing".into(),
                },
                super::command_menu::Candidate {
                    name: "compact".into(),
                    description: "Condense older history into a summary".into(),
                },
                super::command_menu::Candidate {
                    name: "help".into(),
                    description: "Show the available commands".into(),
                },
            ]);
            app.editor = iced::widget::text_editor::Content::with_text("/");
            assert!(
                app.completion().is_some(),
                "a slash draft with an advertisement must open the menu"
            );
            assert_eq!(app.completion().unwrap().matches.len(), 3);
        }
        "plan-chip" => {
            assert_eq!(
                app.transcript.work().plan(),
                Some((1, 4, Some("PUBLIC fix the fold")))
            );
        }
        "work-panel" => {
            // Reached through the existing More menu, exactly as a user would.
            drop(app.update(Message::ToggleConversationMenu));
            drop(app.update(Message::Work(super::work_panel_ui::Action::Open)));
            assert!(app.work_open && !app.conversation_menu);
            assert!(app.transcript.work().total() > 0);
            assert_eq!(app.transcript.work().dropped_total(), 0);
        }
        "assistant-details-source"
        | "assistant-details-formatted"
        | "assistant-details-narrow"
        | "assistant-details-fallback" => {
            drop(app.update(app.detail_request(0).unwrap()));
            if matches!(
                preset,
                "assistant-details-formatted" | "assistant-details-narrow"
            ) {
                let stamp = app.detail.as_ref().unwrap().ticket.unwrap();
                drop(app.update(Message::DetailMode(
                    stamp,
                    super::detail_panel::Mode::Formatted,
                )));
                assert_eq!(
                    app.detail.as_ref().unwrap().mode,
                    super::detail_panel::Mode::Formatted
                );
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
        "tool-activity-expanded" => {
            // The recorded Bash call expands inside the conversation; the detail
            // panel stays closed and no tool is executed.
            drop(app.update(Message::ToggleActivity {
                generation: app.generation,
                key: 5,
            }));
            assert!(app.activity_expanded.contains(&5));
            assert!(app.detail.is_none() && app.composer_visible());
        }
        "long-model" | "tiny-long-model" => {
            let effort = Effort {
                id: PUBLIC_EFFORT_ID.into(),
                name: "PUBLIC extended reasoning · raisonnement approfondi · 深入推理 · εκτενής συλλογισμός".into(),
            };
            let model = Model {
                provider: PUBLIC_PROVIDER_ID.into(), id: PUBLIC_MODEL_ID.into(),
                name: "PUBLIC multilingual model · modèle de démonstration très long · 多语言布局示例模型 · μοντέλο δοκιμής".into(),
                label: "PUBLIC multilingual model · modèle de démonstration très long · 多语言布局示例模型 · μοντέλο δοκιμής".into(),
                efforts: vec![effort.clone()], default_effort: Some(effort.id.clone()),
                service_tiers: vec![], verbosity_levels: vec![],
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
    let submenu = if preset.starts_with("model-submenu") {
        Some(super::model_controls::Section::Model)
    } else if preset.starts_with("effort-submenu") {
        Some(super::model_controls::Section::Effort)
    } else if preset == "speed-submenu" {
        Some(super::model_controls::Section::Speed)
    } else if preset == "output-submenu" {
        Some(super::model_controls::Section::OutputDetail)
    } else {
        None
    };
    if (preset.starts_with("model-popup") || submenu.is_some()) && !preset.ends_with("empty") {
        let effort = Effort {
            id: PUBLIC_EFFORT_ID.into(),
            name: "High".into(),
        };
        let efforts = ["Low", "Medium", "High", "Xhigh", "Max"]
            .iter()
            .map(|name| {
                if *name == "High" {
                    effort.clone()
                } else {
                    Effort {
                        id: format!("PUBLIC_{name}"),
                        name: (*name).into(),
                    }
                }
            })
            .collect();
        let model = Model {
            provider: PUBLIC_PROVIDER_ID.into(),
            id: PUBLIC_MODEL_ID.into(),
            name: "Demo model".into(),
            label: "PUBLIC / Demo model".into(),
            efforts,
            default_effort: Some(effort.id.clone()),
            service_tiers: super::model_controls::Choice::from_catalog(
                ["default", "priority"]
                    .iter()
                    .map(|id| dsh_native_transport::dto::RequestControlChoice {
                        id: (*id).into(),
                        name: (*id).into(),
                        description: None,
                    })
                    .collect(),
                super::model_controls::Section::Speed,
            ),
            verbosity_levels: super::model_controls::Choice::from_catalog(
                ["low", "medium", "high"]
                    .iter()
                    .map(|id| dsh_native_transport::dto::RequestControlChoice {
                        id: (*id).into(),
                        name: (*id).into(),
                        description: None,
                    })
                    .collect(),
                super::model_controls::Section::OutputDetail,
            ),
        };
        app.models = vec![model.clone()];
        app.chosen_model = Some(model.clone());
        app.chosen_effort = Some(effort.clone());
        app.model_selections.insert(
            app.selected.clone().unwrap(),
            ModelSelection {
                provider: model.provider.clone(),
                model: model.id.clone(),
                reasoning_effort: Some(effort.id),
                service_tier: Some("priority".into()),
                text_verbosity: Some("medium".into()),
            },
        );
        app.chosen_service_tier = Some("priority".into());
        app.chosen_text_verbosity = Some("medium".into());
        for index in 0..8 {
            let mut extra = model.clone();
            extra.provider = format!("PUBLIC_provider_{}", index / 4);
            extra.id = format!("PUBLIC_model_{index}");
            extra.name = format!("PUBLIC model {}", index + 1);
            extra.label = format!("PUBLIC provider {} / {}", index / 4 + 1, extra.name);
            app.models.push(extra);
        }
    }
    if preset == "model-popup-long" {
        let model = app.chosen_model.as_mut().unwrap();
        model.name = "WWWWWWWWWWWWWWWWWWWW世界世界世界 · PUBLIC long name".into();
        app.chosen_effort.as_mut().unwrap().name =
            "WWWWWWWWWWWWWWWWWWWW世界 · PUBLIC long effort".into();
    }
    if preset.starts_with("model-popup") || submenu.is_some() {
        app.model_controls
            .seed_menu(app.transport_epoch, app.generation);
        if let Some(section) = submenu {
            let stamp = app.model_controls.stamp().unwrap();
            drop(app.update(Message::ModelControls(
                super::model_controls::Action::Browse(stamp, Some(section)),
            )));
        }
        assert!(app.model_controls.is_open());
    }
    if preset.starts_with("context-injects") {
        // Chat is the default clean view; the trajectory variant shows the log.
        app.records_expanded = preset.ends_with("trajectory");
        assert!(app.transcript.rows().len() >= 7);
    }
    if preset.starts_with("model-metadata") {
        app.records_expanded = preset.ends_with("expanded");
        let effort = Effort {
            id: PUBLIC_EFFORT_ID.into(),
            name: "Medium".into(),
        };
        let model = Model {
            provider: PUBLIC_PROVIDER_ID.into(),
            id: PUBLIC_MODEL_ID.into(),
            name: "Demo model".into(),
            label: "PUBLIC / Demo model".into(),
            efforts: vec![effort.clone()],
            default_effort: Some(effort.id.clone()),
            service_tiers: vec![],
            verbosity_levels: vec![],
        };
        app.model_selections.insert(
            app.selected.clone().unwrap(),
            ModelSelection {
                provider: model.provider.clone(),
                model: model.id.clone(),
                reasoning_effort: Some(effort.id.clone()),
                service_tier: None,
                text_verbosity: None,
            },
        );
        app.models = vec![model.clone()];
        app.chosen_model = Some(model);
        app.chosen_effort = Some(effort);
        assert_eq!(app.transcript.rows().len(), 10);
        assert_eq!(app.transcript.display_rows(app.records_expanded).count(), 8);
    }
    if preset.starts_with("working-") {
        app.window_active = true; // PUBLIC focused eligibility, not a visibility measurement.
        app.working_phase = if preset == "working-phase-6" { 6 } else { 0 };
        let background = dsh_native_transport::dto::SessionId::new("PUBLIC-layout-notes").unwrap();
        app.sessions.get_mut(&background).unwrap().running = true;
        assert!(app.working_animation_active());
    }
    if preset.starts_with("sidebar-") {
        use super::sidebar_organization::{Location, Mutation};
        let io = app.sidebar_organization.start_load(false).unwrap();
        app.sidebar_organization.loaded(io, Ok(Default::default()));
        let target = dsh_native_transport::dto::SessionId::new("PUBLIC-layout-notes").unwrap();
        app.workspaces.push(serde_json::from_value(json!({"workspaceId":"PUBLIC-sidebar-group","path":"/PUBLIC-other-directory",
            "title":"PUBLIC sidebar group","sessionIds":[],"createdAt":"1970-01-01T00:00:00Z","updatedAt":"1970-01-01T00:00:00Z"})).unwrap());
        app.sidebar_organization
            .open(app.transport_epoch, app.generation, target);
        if matches!(preset, "sidebar-trash-review" | "sidebar-trash") {
            let stamp = app.sidebar_organization.stamp().unwrap();
            app.sidebar_organization.review(&stamp);
            if preset == "sidebar-trash" {
                let stamp = app.sidebar_organization.stamp().unwrap();
                let write = app
                    .sidebar_organization
                    .propose(&stamp, Mutation::Trash)
                    .unwrap();
                app.sidebar_organization.saved(write.ticket, Ok(()));
                app.sidebar_organization.dismiss();
                app.show_trash = true;
            }
        } else if preset == "sidebar-moved" {
            let group =
                dsh_native_transport::dto::WorkspaceId::new("PUBLIC-sidebar-group").unwrap();
            let stamp = app.sidebar_organization.stamp().unwrap();
            let write = app
                .sidebar_organization
                .propose(&stamp, Mutation::Move(Location::Workspace(group.clone())))
                .unwrap();
            app.sidebar_organization.saved(write.ticket, Ok(()));
            app.sidebar_organization.dismiss();
            app.workspace = Some(group);
        }
    }
    app.status = if preset == "warnings" {
        "PUBLIC local warning simulation · sequence gap after valid snapshot; read-only until reload · no Host exists".into()
    } else {
        "Live real session; UTC timestamps; no generation started".into()
    };
    (app, receiver)
}

/// A fixture-only local clock tick; never subscribe the App's worker feed.
pub fn advance_working(app: &mut App) -> u8 {
    drop(app.update(Message::WorkingTick));
    app.working_phase
}

/// Observe actual local attachment state without serializing paths or upload authority.
/// Used only by the opt-in native pointer/drop fixture; no worker or Host exists.
pub fn attachment_drop_state(app: &App, expected: &std::path::Path) -> Value {
    json!({
        "reviewPathMatches":app.files.editor().is_some_and(|(_,path)|std::path::Path::new(path)==expected),
        "editorOpen":app.files.editor_open(),
        "localImagePreviewReady":app.public_local_preview_ready(),
        "localReadBusy":app.local_files.busy(),
        "uploadPending":app.files.pending(),
        "uploadReceiptPresent":app.files.ticket().is_some(),
        "cancelNoticeVisible":app.status.starts_with("Drop canceled or unsupported.") && !super::routine_status(&app.status),
        "nativeWindowOwned":app.own_window.is_some(),
    })
}

/// Only known PUBLIC metadata and fixture-owned state; never serialize arbitrary App state.
pub fn metadata(app: &App, preset: &str) -> Value {
    let settings_expected = preset.starts_with("settings-");
    let info_expected = matches!(preset, "information" | "information-expanded");
    let work_expected = preset == "work-panel";
    let context_expected = preset.starts_with("context-management");
    let plan_expected = preset == "plan-chip";
    let plan_projected = app.transcript.work().plan().is_some();
    let context_warning = app.composer_visible() && app.context_policy_open;
    let picker_expected = preset == "file-picker";
    let sidebar_expected = matches!(
        preset,
        "sidebar-options" | "sidebar-options-tiny" | "sidebar-trash-review"
    );
    let composer_expected = !settings_expected
        && !picker_expected
        && !sidebar_expected
        && !info_expected
        && !work_expected
        && !preset.starts_with("assistant-details")
        && !matches!(
            preset,
            "menu" | "navigation" | "navigation-tiny" | "tool-activity-details"
        );
    let reasoning_expected = matches!(preset, "long-model" | "tiny-long-model")
        || preset.starts_with("model-metadata")
        || (app.model_controls.is_open() && !preset.ends_with("empty"));
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
        "settingsEntryExpected":!settings_expected && !picker_expected && !sidebar_expected,
        "stopExpected":!settings_expected && !picker_expected && !sidebar_expected,
        "sidebarOptionsExpected":sidebar_expected,
        "sidebarOrganizationSimulated":preset.starts_with("sidebar-"),
        "sidebarStoreIoExecuted":false,
        "modelControlsExpected":composer_expected,
        "reasoningControlExpected":app.model_controls.is_open() && (app.model_controls.section().is_none() || app.layout_size().width >= 720.0),
        "headerExpected":!settings_expected && !picker_expected && !sidebar_expected && !matches!(preset, "navigation" | "navigation-tiny"),
        "settingsStateMatchesPreset":app.settings.is_open() == settings_expected,
        "informationStateMatchesPreset":app.capabilities == info_expected
            && app.info_expanded == if preset == "information-expanded" { 3 } else { 0 },
        "informationExpandedBits":app.info_expanded,
        "settingsModalExpected":settings_expected,
        "settingsSeedWithoutEffects":settings_expected,
        "modalBackgroundOperationExcludedExpected":settings_expected || picker_expected || sidebar_expected,
        "filePickerExpected":picker_expected,
        "filePickerStateMatchesPreset":app.local_files.picker.is_open()==picker_expected,
        "localPreviewSnapshotSimulated":preset.starts_with("file-preview"),
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
        "toolExecutionOutcomeClaimed":false
    });
    let local_metadata = json!({
        "managedAccountMetadataSimulated":matches!(preset, "settings-codex-managed" | "settings-codex-usage" | "settings-codex-usage-unknown" | "settings-general-usage-unknown" | "settings-general-usage-pending" | "settings-codex-review" | "settings-codex-unknown"),
        "usagePreferencesSimulated":matches!(preset, "settings-codex-usage" | "settings-codex-usage-unknown" | "settings-general-usage-unknown" | "settings-general-usage-pending"),
        "cloudflareConfigurationSimulated":preset.contains("cloudflare"),
        "runtimePreferencesSimulated":matches!(preset, "settings-codex-runtime-models" | "settings-codex-runtime-images" | "settings-codex-runtime-unknown"),
        "accountWritesDispatched":false,
        "accountIdentityOrEntitlementVerified":false,
        "fileDraftMetadataSimulated":preset.starts_with("file-"),
        "fileUploadRequested":false,
        "filePromptRequested":false,
        "filePreviewQualified":false,
        "filePathReviewVisible":app.files.editor_open(),
        "fileDraftReady":app.files.ready().is_some(),
        "fileDraftBlocksSend":!app.files.can_send(&app.file_context()),
        "workPanelStateMatchesPreset":app.work_open == work_expected,
        "contextManagementSimulated":context_expected,
        "contextTrajectoryShown":context_expected && app.records_expanded,
        "planChipSimulated":plan_expected,
        "planChipProjected":plan_projected,
        "planModeSimulated":preset.starts_with("plan-mode"),
        "planModeLabel":super::plan_mode::mode(app.transcript.projections()).map(super::plan_mode::Mode::label),
        "planControlPressable":app.plan_control().visible(),
        "planControlPress":app.plan_control().press(),
        "commandMenuOpen":app.completion().is_some(),
        "commandMenuRows":app.completion().map_or(0,|menu|menu.matches.len()),
        "commandMenuSelected":app.completion().map_or(0,|menu|menu.selected),
        "contextDisclosureOpen":context_warning,
        "workPanelReadOnlyTotal":app.transcript.work().total(),
        "workPanelGoalsOmitted":app.transcript.work().dropped(crate::work_panel::Section::Goals)
    });
    report
        .as_object_mut()
        .unwrap()
        .extend(local_metadata.as_object().unwrap().clone());
    let safety = json!({
        "modelPopupExpected":preset.starts_with("model-popup") || preset.contains("-submenu"),
        "modelSubmenu":app.model_controls.section().map(|section|match section {super::model_controls::Section::Model=>"model",super::model_controls::Section::Effort=>"effort",super::model_controls::Section::Speed=>"speed",super::model_controls::Section::OutputDetail=>"output-detail"}),
        "unsupportedSettingsEnabled":false,
        "modelPopupOpen":app.model_controls.is_open(),
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
        "workingPhase":app.working_phase,
        "workingAnimationEligible":app.working_animation_active(),
        "workingFocusedEligibilitySimulated":preset.starts_with("working-"),
        "publicRunningSessionCount":app.sessions.values().filter(|session|session.running).count(),
        "modelMetadataSuppressionSimulated":preset.starts_with("model-metadata"),
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
            // A fixture renders a public frame; it never announces itself.
            accessibility: false,
        }
    }
    #[test]
    fn presets_render_actual_app_without_queued_effects() {
        for &preset in PRESETS {
            let (size, scale) = preset_size(preset).unwrap();
            let mut options = options();
            options.scale = scale;
            let (mut app, mut receiver) = build(options, preset, size, None);
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
            let sidebar_modal = matches!(
                preset,
                "sidebar-options" | "sidebar-options-tiny" | "sidebar-trash-review"
            );
            assert_eq!(app.sidebar_organization.is_open(), sidebar_modal);
            assert_eq!(report["sidebarOptionsExpected"], sidebar_modal);
            assert_eq!(
                report["modalBackgroundOperationExcludedExpected"],
                preset.starts_with("settings-") || preset == "file-picker" || sidebar_modal
            );
            if sidebar_modal {
                assert_eq!(report["headerExpected"], false);
                assert_eq!(report["settingsEntryExpected"], false);
                assert_eq!(report["stopExpected"], false);
            }
            assert_eq!(report["composerStateMatchesPreset"], true);
            assert_eq!(report["settingsStateMatchesPreset"], true);
            assert_eq!(report["informationStateMatchesPreset"], true);
            assert_eq!(app.settings.is_open(), preset.starts_with("settings-"));
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
                assert_eq!(report["conversationSnapshotSimulated"], true);
                assert_eq!(report["displayedPublicRecordCount"], 1);
                assert_eq!(report["canceledInboxHeadingExpected"], true);
                assert_eq!(app.transcript.rows().len(), 3);
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
                // The failed-edit preset records a second error marker; every
                // other tool-activity preset keeps the single Bash failure.
                assert_eq!(
                    report["publicRecordedErrorMarkerCount"],
                    if preset == "tool-activity-edit-error" {
                        2
                    } else {
                        1
                    }
                );
                if preset == "tool-activity-edit-error" {
                    // The edit call keeps its recorded totals; the render is what
                    // drops them, so the model still carries them.
                    assert_eq!(app.transcript.activity(3).unwrap().edits, Some((1, 1)));
                    assert!(app.transcript.activity(4).unwrap().failed);
                }
                // The attachment preset is the only one whose user message
                // carries a file part; every other tool-activity preset keeps
                // the file text inside the joined body, so this also pins that
                // the split did not leak into them.
                let files = &app.transcript.rows()[0].files;
                if preset == "tool-activity-attachment" {
                    assert_eq!(files.len(), 1);
                    assert_eq!(files[0].name, "PUBLIC-note.txt");
                    assert_eq!(files[0].meta, "TXT 12B");
                    assert!(
                        !app.transcript.rows()[0].text.contains("PUBLIC-note.txt"),
                        "the file part must leave the bubble body"
                    );
                } else {
                    assert!(files.is_empty());
                }
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
                assert!(
                    app.detail
                        .as_ref()
                        .unwrap()
                        .source
                        .contains("PUBLIC old text")
                );
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
        let (app, mut receiver) = build(options(), "long-model", Size::new(760.0, 560.0), None);
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
