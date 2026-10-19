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
    "subagent-catalog",
    "subagent-readonly",
    "subagent-sibling",
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
    "working-queue",
    "queued-idle",
    "turn-trigger",
    "token-usage",
    "goal-bar",
    "working-phase-6",
    "working-narrow",
    "chat-submitted",
    "chat-preparation-error",
    "chat-stream-live",
    "chat-context-rewrite",
    "long-user-message",
    "history-more",
    "chat-transcript",
    "agent-preset",
    "chat-markdown",
    "chat-table",
    "chat-image",
    "chat-references",
    "chat-thoughts",
    "thoughts-preview",
    "chat-thoughts-markdown",
    "chat-search-thoughts",
    "chat-edit-review",
    "chat-search",
    "chat-recovered",
    "chat-edited",
    "chat-search-narrow",
    "collapsed-wide",
    "tiny-long-model",
    "navigation-tiny",
    "navigation-search-tiny",
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
    "schedules",
    "mcp",
    "changes-card",
    "feedback",
    "workflow-run",
    "context-pressure",
    "context-pressure-risk",
    "context-management",
    "context-management-chat",
    "plan-chip",
    "permission-chip",
    "command-menu",
    "command-menu-mid-draft",
    "attach-menu",
    "drop-overlay",
    "skills-menu",
    "slash-loading",
    "command-hint",
    "plan-mode",
    "plan-mode-applying",
    "plan-mode-leaving",
    "tool-activity",
    "tool-activity-narrow",
    "tool-activity-tiny",
    "tool-activity-details",
    "tool-activity-expanded",
    "process-group-closed",
    "tool-activity-attachment",
    "tool-activity-edit-error",
    "tool-activity-diff",
    "tool-activity-diff-long",
    "tool-activity-read",
    "tool-activity-terminal",
    "tool-activity-search",
    "tool-activity-web",
    "tool-activity-ask",
    "tool-activity-applied",
    "model-retry",
    "turn-total",
    "stats-dialog",
    "settings-shortcuts",
    "turn-error",
    "turn-max-tokens",
    "schedule-create",
    "skill-row",
    "command-row",
    "manual-compaction",
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
        | "tool-activity-edit-error"
        | "tool-activity-diff"
        | "tool-activity-diff-long"
        | "tool-activity-read"
        | "tool-activity-terminal"
        | "tool-activity-search"
        | "tool-activity-web"
        | "tool-activity-ask"
        | "tool-activity-applied" => Some((Size::new(1040.0, 1040.0), 1.0)),
        "context-injects" | "context-injects-trajectory" => Some((Size::new(1040.0, 900.0), 1.0)),
        "assistant-tool-call" | "assistant-tool-call-expanded" => {
            Some((Size::new(1040.0, 760.0), 1.0))
        }
        "model-retry" => Some((Size::new(1040.0, 760.0), 1.0)),
        "turn-total" => Some((Size::new(1040.0, 760.0), 1.0)),
        "stats-dialog" => Some((Size::new(1040.0, 760.0), 1.0)),
        "settings-shortcuts" => Some((Size::new(1040.0, 900.0), 1.0)),
        "turn-error" | "turn-max-tokens" => Some((Size::new(1040.0, 620.0), 1.0)),
        "schedule-create" => Some((Size::new(1040.0, 700.0), 1.0)),
        "skill-row" => Some((Size::new(1040.0, 700.0), 1.0)),
        "command-row" => Some((Size::new(1040.0, 700.0), 1.0)),
        "process-group-closed" => Some((Size::new(1040.0, 760.0), 1.0)),
        "manual-compaction" => Some((Size::new(1040.0, 620.0), 1.0)),
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
        | "working-queue"
        | "queued-idle"
        | "turn-trigger"
        | "token-usage"
        | "goal-bar"
        | "permission-chip"
        | "working-phase-6"
        | "subagent-trail"
        | "subagent-catalog"
        | "subagent-readonly"
        | "subagent-sibling"
        | "hero-workspace-selected" => Some((Size::new(1040.0, 700.0), 1.0)),
        "working-narrow" => Some((Size::new(760.0, 560.0), 1.0)),
        "chat-submitted"
        | "chat-preparation-error"
        | "chat-stream-live"
        | "chat-context-rewrite"
        | "history-more"
        | "long-user-message"
        | "chat-transcript"
        | "agent-preset"
        | "chat-markdown"
        | "chat-table"
        | "chat-image"
        | "chat-references"
        | "chat-thoughts"
        | "thoughts-preview"
        | "chat-thoughts-markdown"
        | "chat-search-thoughts"
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
        | "navigation-search-tiny"
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
        | "schedules"
        | "mcp"
        | "changes-card"
        | "feedback"
        | "workflow-run"
        | "context-pressure"
        | "context-pressure-risk"
        | "context-management"
        | "context-management-chat"
        | "plan-chip"
        | "command-menu"
        | "command-menu-mid-draft"
        | "attach-menu"
        | "drop-overlay"
        | "skills-menu"
        | "slash-loading"
        | "command-hint"
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
    if matches!(
        preset,
        "command-menu"
            | "command-menu-mid-draft"
            | "attach-menu"
            | "drop-overlay"
            | "skills-menu"
            | "slash-loading"
            | "command-hint"
    ) {
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
    if preset == "subagent-sibling" {
        return vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                "surfaceOp":"append","data":{"id":"PUBLIC-user","role":"user",
                    "source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC sibling conversation."}]}}}),
            json!({"type":"event","event":{"type":"assistant/message","seq":1,"time":1001,
                "surfaceOp":"append","data":{"turn":1,"step":1,
                    "stream":[{"type":"text-chunks","time0":1001,"index":0,"dt":[],
                        "texts":["PUBLIC sibling answer."]}]}}}),
        ];
    }
    if preset == "subagent-readonly" {
        // A one-shot child: its own durable descriptor, the conversation record, and no
        // follow-up composer (the reference replaces the whole composer).
        return vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                "surfaceOp":"append","data":{"id":"PUBLIC-user","role":"user",
                    "source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC one-shot task."}]}}}),
            json!({"type":"event","event":{"type":"assistant/message","seq":1,"time":1001,
                "surfaceOp":"append","data":{"turn":1,"step":1,
                    "stream":[{"type":"text-chunks","time0":1001,"index":0,"dt":[],
                        "texts":["PUBLIC one-shot child answer."]}]}}}),
            json!({"type":"event","event":{"type":"subagent/descriptor","seq":2,"time":1002,
                "data":{"version":3,"provider":"PUBLIC-provider","mode":"one-shot"}}}),
        ];
    }
    if preset == "subagent-catalog" {
        return vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                "data":{"id":"PUBLIC-user","role":"user","source":{"kind":"user"},
                    "content":[{"type":"text","text":"PUBLIC parent with two subagents."}]},
                "surfaceOp":"append"}}),
            json!({"type":"event","event":{"type":"assistant/message","seq":1,"time":1001,
                "data":{"turn":1,"step":1,
                    "stream":[{"type":"text-chunks","time0":1001,"index":0,"dt":[],
                        "texts":["PUBLIC synthetic parent answer."]}]},
                "surfaceOp":"append"}}),
        ];
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
    if preset == "chat-references" {
        // A cross-session recall: the `session-reference` context record that follows
        // the human message carries the labels the reference draws under its bubble.
        return vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,"surfaceOp":"append","data":{"id":"PUBLIC-user","role":"user","source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC question that recalled another session."}]}}}),
            json!({"type":"event","event":{"type":"user/message","seq":1,"time":1001,"surfaceOp":"append","data":{"role":"user","id":"PUBLIC-recall","source":{"kind":"session-reference","references":[{"label":"PUBLIC session alpha"},{"label":"PUBLIC session beta"}]},"content":[{"type":"text","text":"PUBLIC recalled context."}]}}}),
            json!({"type":"event","event":{"type":"assistant/message","seq":2,"time":1002,"surfaceOp":"append","data":{"turn":1,"step":1,"usage":{"inputTokens":100,"cacheReadTokens":40,"cacheWriteTokens":10,"outputTokens":20,"reasoningTokens":8,"totalTokens":170},"stream":[
                {"type":"reasoning-chunks","time0":1002,"index":0,"dt":[],"texts":["PUBLIC recall reasoning."]},
                {"type":"text-chunks","time0":1002,"index":1,"dt":[],"texts":["PUBLIC answer after the recall."]}
            ]}}}),
        ];
    }
    if preset == "chat-image" {
        // The reference renders an unresolvable image as its alt text in tertiary
        // italic (`render.tsx:640`); before this, the whole reply failed the parse.
        return vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,"surfaceOp":"append","data":{"id":"PUBLIC-user","role":"user","source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC show a native image fallback."}]}}}),
            json!({"type":"event","event":{"type":"assistant/message","seq":1,"time":1001,"surfaceOp":"append","data":{"turn":1,"step":1,"usage":{"inputTokens":100,"cacheReadTokens":40,"cacheWriteTokens":10,"outputTokens":20,"reasoningTokens":8,"totalTokens":170},"stream":[
                {"type":"reasoning-chunks","time0":1001,"index":0,"dt":[],"texts":["PUBLIC image reasoning."]},
                {"type":"text-chunks","time0":1001,"index":1,"dt":[],"texts":["PUBLIC paragraph with an inline ![PUBLIC inline alt](https://example.invalid/inline.png) image.\n\n![PUBLIC standalone alt](https://example.invalid/standalone.png)\n\n![](https://example.invalid/no-alt.png)\n\nPUBLIC paragraph after the images."]}
            ]}}}),
        ];
    }
    if preset == "chat-table" {
        // A table and a narrow three-column table, with paragraphs on both sides so a
        // grab shows the rhythm around them. The reference renders these; the parser
        // used to fail the whole reply instead (`MarkdownText.module.css:235-283`).
        return vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,"surfaceOp":"append","data":{"id":"PUBLIC-user","role":"user","source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC show a native table."}]}}}),
            json!({"type":"event","event":{"type":"assistant/message","seq":1,"time":1001,"surfaceOp":"append","data":{"turn":1,"step":1,"usage":{"inputTokens":100,"cacheReadTokens":40,"cacheWriteTokens":10,"outputTokens":20,"reasoningTokens":8,"totalTokens":170},"stream":[
                {"type":"reasoning-chunks","time0":1001,"index":0,"dt":[],"texts":["PUBLIC table reasoning."]},
                {"type":"text-chunks","time0":1001,"index":1,"dt":[],"texts":["PUBLIC paragraph before the table.\n\n| Column | PUBLIC value | Notes |\n|---|---|---|\n| Alpha | `one` | First row with **bold** |\n| Beta | two | Second row |\n| Gamma | three | Third row |\n\n| PUBLIC one | PUBLIC two | PUBLIC three | PUBLIC four | PUBLIC five |\n|---|---|---|---|---|\n| a | b | c | d | e |\n| f | g | h | i | j |\n\nPUBLIC paragraph after the table."]}
            ]}}}),
        ];
    }
    if matches!(
        preset,
        "chat-markdown"
            | "chat-thoughts"
            | "thoughts-preview"
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
        "chat-transcript"
            | "chat-context-rewrite"
            | "history-more"
            | "long-user-message"
            | "chat-thoughts-markdown"
            | "chat-search-thoughts"
    ) {
        let value: Value =
            serde_json::from_str(include_str!("../fixtures/chat-visibility.json")).unwrap();
        let mut records = value["records"].as_array().unwrap().clone();
        if matches!(preset, "chat-thoughts-markdown" | "chat-search-thoughts") {
            // The shared fixture's reasoning is one plain sentence, so nothing reached
            // the reference's compact markdown ladder (`MarkdownText.module.css:330-370`);
            // this preset gives the reasoning a heading, list, code block and quote.
            for record in &mut records {
                let event = &mut record["event"];
                if event["type"] == "assistant/message" {
                    let text = "### PUBLIC compact heading\n\nFirst thought paragraph with **bold** and `inline code`.\n\n1. First compact item\n2. Second compact item\n\n```rust\nlet PUBLIC_reasoning = 1;\n```\n\n| PUBLIC key | PUBLIC value |\n|---|---|\n| alpha | one |\n\n> PUBLIC quoted thought.";
                    // The reasoning text lives in the message content and again in the
                    // stream's reasoning frame and block-end, so all three move together.
                    for part in event["data"]["message"]["content"]
                        .as_array_mut()
                        .into_iter()
                        .flatten()
                    {
                        if part["type"] == "reasoning" {
                            part["text"] = json!(text);
                        }
                    }
                    for frame in event["data"]["stream"].as_array_mut().into_iter().flatten() {
                        if frame["type"] == "reasoning-chunks" {
                            frame["texts"] = json!([text]);
                        }
                        if frame["chunk"]["block"]["type"] == "reasoning" {
                            frame["chunk"]["block"]["text"] = json!(text);
                        }
                    }
                }
            }
        }
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

/// The same records with the bash result rewritten as the reference's *completed failing call*: the
/// shell settles a non-zero exit with `isError` false and the status as a trailing marker
/// (`terminal-card-model.ts:86-92`), which is what turns the row into a terminal card.
fn public_tool_terminal_records() -> Vec<Value> {
    let mut records = public_tool_records();
    for record in &mut records {
        let event = &mut record["event"];
        if event["type"] == "tool/result"
            && event["data"]["message"]["toolCallId"] == "PUBLIC-bash-call"
        {
            event["data"]["message"]["isError"] = json!(false);
            event["data"]["message"]["content"] = json!([{
                "type":"text",
                "text":"PUBLIC terminal output line one\nPUBLIC terminal output line two\n[exit code: 2]"
            }]);
        }
    }
    records
}

/// The same records with the edit result carrying the diff the Host recorded as *applied*, whose
/// text differs from the call's arguments, so a capture shows which one the card draws
/// (`diff-card-model.ts:71-104`). Synthetic PUBLIC metadata; no file was written.
fn public_tool_applied_records() -> Vec<Value> {
    let mut records = public_tool_records();
    for record in &mut records {
        let event = &mut record["event"];
        if event["type"] == "tool/result"
            && event["data"]["message"]["toolCallId"] == "PUBLIC-edit-call"
        {
            // Two applied lines where the arguments described a one-for-one replacement, so the
            // capture also shows the row's totals following the applied change.
            event["data"]["meta"] = json!({"diffs": [{
                "path": "/PUBLIC-layout-fixture/workspace/src/PUBLIC-note.txt",
                "oldText": "PUBLIC old text",
                "newText": "PUBLIC applied text\nPUBLIC applied second line"
            }]});
        }
    }
    records
}

/// One `/compact` run, the summary it produced, and its receipt, so the reference's single
/// compaction marker draws the structured counts. Synthetic PUBLIC metadata; nothing was compacted.
fn public_manual_compaction_records() -> Vec<Value> {
    let event = |seq: u64, kind: &str, data: Value| {
        json!({"type":"event", "event":{
            "type":kind, "seq":seq, "time":1000+seq, "data":data, "surfaceOp":"append"
        }})
    };
    vec![
        event(
            0,
            "user/message",
            json!({"role":"user", "id":"PUBLIC-compact-user", "source":{"kind":"user"},
                "content":[{"type":"text","text":"PUBLIC manual-compaction fixture · nothing was compacted."}]}),
        ),
        event(
            1,
            "command/run",
            json!({"commandId":"PUBLIC-compact-1", "name":"compact"}),
        ),
        event(2, "compaction/start", json!({})),
        event(
            3,
            "compaction/summary",
            json!({
                "shadowedSeqs":[0,1,2,3,4,5,6], "shadowedTokenCount": 12_400
            }),
        ),
        event(
            4,
            "command/done",
            json!({
                "commandId":"PUBLIC-compact-1", "kind":"success", "sourceEventSeq": 3
            }),
        ),
        event(
            5,
            "assistant/message",
            json!({"turn":1, "step":2,
                "stream":[{"type":"text-chunks", "time0":1005, "index":0, "dt":[],
                    "texts":["PUBLIC synthetic assistant text · the compaction marker states the recorded counts."]}]
            }),
        ),
    ]
}

/// One prompt and one slash-command run, so the reference's command row draws its name, outcome and
/// multi-line output. Synthetic PUBLIC metadata; no command ran.
fn public_command_records() -> Vec<Value> {
    let event = |seq: u64, kind: &str, data: Value| {
        json!({"type":"event", "event":{
            "type":kind, "seq":seq, "time":1000+seq, "data":data, "surfaceOp":"append"
        }})
    };
    vec![
        event(
            0,
            "user/message",
            json!({"role":"user", "id":"PUBLIC-command-user", "source":{"kind":"user"},
                "content":[{"type":"text","text":"PUBLIC command fixture · no command ran."}]}),
        ),
        event(
            1,
            "command/run",
            json!({"commandId":"PUBLIC-command-1", "name":"/compact",
                "args":"keep the plan"}),
        ),
        // Three lines, so the row's output is the expandable body rather than the summary.
        event(
            2,
            "command/done",
            json!({"commandId":"PUBLIC-command-1", "kind":"success",
                "text":"PUBLIC first output line
PUBLIC second output line
PUBLIC third output line"}),
        ),
        event(
            3,
            "assistant/message",
            json!({"turn":1, "step":2,
                "stream":[{"type":"text-chunks", "time0":1003, "index":0, "dt":[],
                    "texts":["PUBLIC synthetic assistant text · the command row states the recorded run."]}]
            }),
        ),
    ]
}

/// One prompt and one `skill` load, so the reference's skill row draws its `Skill · <name>` label
/// and the instructions it returned. Synthetic PUBLIC metadata; no skill was loaded.
fn public_skill_records() -> Vec<Value> {
    let event = |seq: u64, kind: &str, data: Value| {
        json!({"type":"event", "event":{
            "type":kind, "seq":seq, "time":1000+seq, "data":data, "surfaceOp":"append"
        }})
    };
    vec![
        event(
            0,
            "user/message",
            json!({"role":"user", "id":"PUBLIC-skill-user", "source":{"kind":"user"},
                "content":[{"type":"text","text":"PUBLIC skill fixture · nothing was loaded."}]}),
        ),
        event(
            1,
            "tool/call",
            json!({"callId":"PUBLIC-skill-call", "turn":1, "step":2, "name":"skill",
                "arguments":json!({"name":"impeccable-rust"}).to_string()}),
        ),
        event(
            2,
            "tool/result",
            json!({"turn":1, "step":2, "message":{
                "role":"tool", "toolCallId":"PUBLIC-skill-call",
                "source":{"kind":"tool", "callId":"PUBLIC-skill-call"},
                "isError":false,
                "content":[{"type":"text", "text":"PUBLIC skill instructions · synthetic, not a real skill."}]
            }}),
        ),
        event(
            3,
            "assistant/message",
            json!({"turn":1, "step":2,
                "stream":[{"type":"text-chunks", "time0":1003, "index":0, "dt":[],
                    "texts":["PUBLIC synthetic assistant text · the skill row names the recorded load."]}]
            }),
        ),
    ]
}

/// One prompt and one `schedule_create` call whose result is the created task serialized as JSON,
/// so the reference's created-task card draws its title and rule. Synthetic PUBLIC metadata; no
/// task was created and no endpoint was called.
fn public_schedule_create_records() -> Vec<Value> {
    let event = |seq: u64, kind: &str, data: Value| {
        json!({"type":"event", "event":{
            "type":kind, "seq":seq, "time":1000+seq, "data":data, "surfaceOp":"append"
        }})
    };
    let task = json!({"id":"PUBLIC-schedule-1",
        "title":"PUBLIC weekly fixture reminder",
        "prompt":"PUBLIC instruction for the created task",
        "scheduledAt":"2026-09-01T09:00:00.000Z",
        "kind":"weekly","time":"09:00","timeZone":"UTC","weekdays":[1,5]});
    vec![
        event(
            0,
            "user/message",
            json!({"role":"user", "id":"PUBLIC-schedule-user", "source":{"kind":"user"},
                "content":[{"type":"text","text":"PUBLIC scheduled-task fixture · nothing was created."}]}),
        ),
        event(
            1,
            "tool/call",
            json!({"callId":"PUBLIC-schedule-call", "turn":1, "step":2,
                "name":"schedule_create",
                "arguments":json!({"title":"PUBLIC weekly fixture reminder",
                    "prompt":"PUBLIC instruction for the created task",
                    "kind":"weekly","time":"09:00","timeZone":"UTC","weekdays":[1,5]}).to_string()}),
        ),
        event(
            2,
            "tool/result",
            json!({"turn":1, "step":2, "message":{
                "role":"tool", "toolCallId":"PUBLIC-schedule-call",
                "source":{"kind":"tool", "callId":"PUBLIC-schedule-call"},
                "isError":false,
                "content":[{"type":"text", "text":task.to_string()}]
            }}),
        ),
        event(
            3,
            "assistant/message",
            json!({"turn":1, "step":2,
                "stream":[{"type":"text-chunks", "time0":1003, "index":0, "dt":[],
                    "texts":["PUBLIC synthetic assistant text · the created-task card is drawn from the recorded result, not from a created task."]}]
            }),
        ),
    ]
}

/// One prompt and one failed turn, so the reference's failure row draws its title, mapped reason
/// and code chip. Synthetic PUBLIC metadata; no request was made and none failed.
fn public_turn_error_records(reason: Value) -> Vec<Value> {
    let event = |seq: u64, kind: &str, data: Value| {
        json!({"type":"event", "event":{
            "type":kind, "seq":seq, "time":1000+seq, "data":data, "surfaceOp":"append"
        }})
    };
    vec![
        event(
            0,
            "user/message",
            json!({"role":"user", "id":"PUBLIC-turn-error-user", "source":{"kind":"user"},
                "content":[{"type":"text","text":"PUBLIC turn-error fixture · no model request ran."}]}),
        ),
        event(1, "turn/start", json!({"turn":1})),
        event(2, "turn/end", json!({"turn":1, "reason": reason})),
    ]
}

/// One prompt and one reply inside a clean turn boundary, so the settled reply states the
/// reference's `Completed in Ns` (`TurnProcessNodeView.tsx:20-33`). Recorded metadata only.
fn public_turn_total_records() -> Vec<Value> {
    let event = |seq: u64, kind: &str, time: u64, data: Value| {
        json!({"type":"event", "event":{
            "type":kind, "seq":seq, "time":time, "data":data, "surfaceOp":"append"
        }})
    };
    vec![
        event(
            0,
            "user/message",
            1000,
            json!({"role":"user", "id":"PUBLIC-turn-total-user", "source":{"kind":"user"},
                "content":[{"type":"text","text":"PUBLIC turn-total fixture · no model request ran."}]}),
        ),
        event(1, "turn/start", 1000, json!({"turn":1})),
        event(
            2,
            "assistant/message",
            1001,
            json!({"turn":1, "step":1,
                "stream":[{"type":"text-chunks", "time0":1001, "index":0, "dt":[],
                    "texts":["PUBLIC synthetic assistant text · the tail states the recorded turn duration, not a real elapsed run."]}]
            }),
        ),
        event(
            3,
            "turn/end",
            144_000,
            json!({"turn":1, "reason":{"kind":"completed"}}),
        ),
    ]
}

/// A conversation whose only non-human activity is a model-retry chain: one attempt started, one
/// waiting out its delay (`retry.ts:20-100`). Every value is synthetic PUBLIC metadata; no model
/// request was made and none failed.
fn public_retry_records() -> Vec<Value> {
    let event = |seq: u64, kind: &str, data: Value| {
        json!({"type":"event", "event":{
            "type":kind, "seq":seq, "time":1000+seq, "data":data, "surfaceOp":"append"
        }})
    };
    let retry = |retry: u64, delay: u64| {
        json!({"retryId":"PUBLIC-retry-chain", "turn":1, "step":1, "provider":"deepseek",
            "mode":"normal", "policyKey":"PUBLIC-policy", "retry":retry, "maxRetries":3,
            "delayMs":delay, "failure":{"message":"PUBLIC upstream unavailable · synthetic"}})
    };
    vec![
        event(
            0,
            "user/message",
            json!({"role":"user", "id":"PUBLIC-retry-user", "source":{"kind":"user"},
                "content":[{"type":"text","text":"PUBLIC retry fixture · no model request ran."}]}),
        ),
        event(1, "llm/retry", retry(1, 2000)),
        event(
            2,
            "llm/retry-started",
            json!({"retryId":"PUBLIC-retry-chain", "turn":1, "step":1, "retry":1}),
        ),
        event(3, "llm/retry", retry(2, 4000)),
        event(
            4,
            "assistant/message",
            json!({"turn":1, "step":1,
                "stream":[{"type":"text-chunks", "time0":1004, "index":0, "dt":[],
                    "texts":["PUBLIC synthetic assistant text · the retry row states recorded metadata, not a real failure."]}]
            }),
        ),
    ]
}

/// A conversation whose only tool call is an `ask_user_question`, recorded with the question and
/// answer JSON the reference pairs into its transcript card (`ask-question-row.tsx:130-170`). Every
/// question and answer is synthetic PUBLIC metadata; nothing was asked of the user.
fn public_ask_records() -> Vec<Value> {
    let event = |seq: u64, kind: &str, data: Value| {
        json!({"type":"event", "event":{
            "type":kind, "seq":seq, "time":1000+seq, "data":data, "surfaceOp":"append"
        }})
    };
    let questions = json!([
        {"id":"PUBLIC-q1", "question":"PUBLIC which surface should the fixture draw?",
         "options":[{"label":"Chat"},{"label":"Trajectory"}]},
        {"id":"PUBLIC-q2", "question":"PUBLIC anything else to record?", "multi_select":true},
    ]);
    let answers = json!({"answers":[
        {"id":"PUBLIC-q1", "selected":["Chat"], "custom":"PUBLIC typed note"},
        {"id":"PUBLIC-q2", "selected":[]},
    ]});
    vec![
        event(
            0,
            "user/message",
            json!({"role":"user", "id":"PUBLIC-ask-user", "source":{"kind":"user"},
                "content":[{"type":"text","text":"PUBLIC ask fixture · nothing was asked."}]}),
        ),
        event(
            1,
            "tool/call",
            json!({"callId":"PUBLIC-ask-call", "turn":1, "step":2,
                "name":"ask_user_question",
                "arguments":json!({"questions": questions}).to_string()}),
        ),
        event(
            2,
            "tool/result",
            json!({"turn":1, "step":2, "message":{
                "role":"tool", "toolCallId":"PUBLIC-ask-call",
                "source":{"kind":"tool", "callId":"PUBLIC-ask-call"},
                "isError":false,
                "content":[{"type":"text", "text":answers.to_string()}]
            }}),
        ),
        event(
            3,
            "assistant/message",
            json!({"turn":1, "step":2,
                "stream":[{"type":"text-chunks", "time0":1003, "index":0, "dt":[],
                    "texts":["PUBLIC synthetic assistant text · the question card is drawn from recorded metadata, not from an answer."]}]
            }),
        ),
    ]
}

/// A conversation whose only tool call is a `web_search`, recorded with the metadata the reference
/// validates into its web card (`web-card-model.ts:60-99`). Every URL, title and snippet is
/// synthetic PUBLIC metadata; nothing was fetched and no such page exists.
fn public_web_records() -> Vec<Value> {
    let event = |seq: u64, kind: &str, data: Value| {
        json!({"type":"event", "event":{
            "type":kind, "seq":seq, "time":1000+seq, "data":data, "surfaceOp":"append"
        }})
    };
    vec![
        event(
            0,
            "user/message",
            json!({"role":"user", "id":"PUBLIC-web-user", "source":{"kind":"user"},
                "content":[{"type":"text","text":"PUBLIC web fixture · nothing was fetched."}]}),
        ),
        event(
            1,
            "tool/call",
            json!({"callId":"PUBLIC-web-call", "turn":1, "step":2, "name":"functions.web_search",
                "arguments":json!({"queries":["PUBLIC query"]}).to_string()}),
        ),
        event(
            2,
            "tool/result",
            json!({"turn":1, "step":2, "meta":{
            "truncated":true,
            "answer":"PUBLIC provider answer · synthetic, not a real search result.",
            "sources":[
                {"url":"https://PUBLIC.example/one", "title":"PUBLIC source one",
                 "snippet":"PUBLIC snippet one", "publishedAt":"PUBLIC 2026-01-01"},
                {"url":"https://PUBLIC.example/two", "title":"PUBLIC source two"},
            ]},
            "message":{
                "role":"tool", "toolCallId":"PUBLIC-web-call",
                "source":{"kind":"tool", "callId":"PUBLIC-web-call"},
                "isError":false,
                "content":[{"type":"text", "text":"PUBLIC truncated search page."}]
            }}),
        ),
        event(
            3,
            "assistant/message",
            json!({"turn":1, "step":2,
                "stream":[{"type":"text-chunks", "time0":1003, "index":0, "dt":[],
                    "texts":["PUBLIC synthetic assistant text · the web card is drawn from recorded metadata, not from a search."]}]
            }),
        ),
    ]
}

/// A conversation whose only tool call is a `grep`, recorded with the metadata the reference
/// validates into its search card (`search-card-model.ts:78-100`). Every string is synthetic
/// PUBLIC metadata; nothing was searched and no file is named that exists.
fn public_search_records() -> Vec<Value> {
    let event = |seq: u64, kind: &str, data: Value| {
        json!({"type":"event", "event":{
            "type":kind, "seq":seq, "time":1000+seq, "data":data, "surfaceOp":"append"
        }})
    };
    vec![
        event(
            0,
            "user/message",
            json!({"role":"user", "id":"PUBLIC-search-user", "source":{"kind":"user"},
                "content":[{"type":"text","text":"PUBLIC search fixture · no search was run."}]}),
        ),
        event(
            1,
            "tool/call",
            json!({"callId":"PUBLIC-grep-call", "turn":1, "step":2, "name":"functions.grep",
                "arguments":json!({"pattern":"PUBLIC_marker", "path":"src"}).to_string()}),
        ),
        event(
            2,
            "tool/result",
            json!({"turn":1, "step":2, "meta":{
            "shape":"matches", "total":120, "truncated":true,
            "files":[
                {"path":"src/PUBLIC-a.rs", "matches":[
                    {"lineNumber":4, "line":"PUBLIC_marker in a"},
                    {"lineNumber":9, "line":"PUBLIC_marker again in a"}]},
                {"path":"src/PUBLIC-b.rs", "matches":[
                    {"lineNumber":1, "line":"PUBLIC_marker in b"}]},
            ]},
            "message":{
                "role":"tool", "toolCallId":"PUBLIC-grep-call",
                "source":{"kind":"tool", "callId":"PUBLIC-grep-call"},
                "isError":false,
                "content":[{"type":"text", "text":"PUBLIC truncated search page."}]
            }}),
        ),
        event(
            3,
            "assistant/message",
            json!({"turn":1, "step":2,
                "stream":[{"type":"text-chunks", "time0":1003, "index":0, "dt":[],
                    "texts":["PUBLIC synthetic assistant text · the search card is drawn from recorded metadata, not from a search."]}]
            }),
        ),
    ]
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
        // The read result also carries the metadata the reference validates into its card
        // (`read-card-model.ts:104-137`): path, 1-based offset, numbered lines and the file's
        // total. Every value is synthetic PUBLIC metadata; the file does not exist.
        let meta = (name == "functions.read").then(|| json!({
            "path":"/PUBLIC-layout-fixture/workspace/src/PUBLIC-note.txt",
            "offset":1,
            "totalLines":42,
            "lang":"text",
            "lines":[
                {"number":1, "text":"PUBLIC note line one"},
                {"number":2, "text":"PUBLIC note line two"},
            ],
        }));
        let mut result = json!({"turn":1, "step":2, "message":{
            "role":"tool", "toolCallId":id, "source":{"kind":"tool", "callId":id},
            "isError":failed, "content":[{"type":"text", "text":if failed {
                "PUBLIC synthetic error-marker fixture. No bash process ran and no tool outcome is claimed."
            } else if name == "functions.read" {
                // The read tool's own result envelope, the shape a real session stores and the
                // reference parses into its read card (`read-card-model.ts:124-127`).
                "<path>/PUBLIC-layout-fixture/workspace/src/PUBLIC-note.txt</path>\n<type>file</type>\n<content>\nPUBLIC note line one\nPUBLIC note line two\n</content>"
            } else { "PUBLIC synthetic result body for full Details. No tool execution or success is claimed." }}]
        }});
        if let Some(meta) = meta {
            result["meta"] = meta;
        }
        records.push(event(seq+1, "tool/result", result));
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
    app.disable_background_reads();
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
        // A preset that carries a transcript is not a blank session; the reference's own row
        // list and the chat fixtures both depend on this flag.
        "blank": !matches!(preset, "conversation" | "chat-density" | "chat-markdown" | "chat-thoughts" | "thoughts-preview" | "chat-thoughts-markdown" | "chat-table")
            && !preset.starts_with("tool-activity"),

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
    if preset == "subagent-sibling" {
        // A selected child with a parent catalog: the header's current crumb becomes the
        // sibling switcher and lists both children.
        let parent: SessionSummary = serde_json::from_value(json!({
            "agentAvailable":true, "sessionId":"PUBLIC-layout-parent", "updatedAt":950,
            "running":false, "blank":false, "cwd":"/PUBLIC-layout-fixture/workspace",
            "projections":{"kind":"cached","asOfSeq":-1,
                "values":{"title":"PUBLIC parent with subagents",
                    "subagentCatalog":[
                        {"id":"PUBLIC-child-reviewer","createdAt":1000,"mode":"continuable","label":"PUBLIC reviewer"},
                        {"id":"PUBLIC-child-scout","createdAt":1001,"mode":"one-shot","label":"PUBLIC scout"}
                    ]}}
        }))
        .expect("fixed PUBLIC parent metadata");
        app.sessions.insert(parent.session_id.clone(), parent);
        for (id, title, running) in [
            ("PUBLIC-child-reviewer", "PUBLIC reviewer", true),
            ("PUBLIC-child-scout", "PUBLIC scout", false),
        ] {
            let mut values = json!({"title": title});
            if id == "PUBLIC-child-reviewer" {
                values["subagentCatalog"] = json!([
                    {"id":"PUBLIC-grandchild-helper","createdAt":1002,"mode":"one-shot","label":"PUBLIC helper"}
                ]);
                // The reference's row metrics: a four-bucket token total and the settled
                // active duration, the two projections the list carries when the Host serves
                // them.
                values["tokenUsage"] = json!({"uncachedInputTokens":6000,
                    "outputTokens":345,"cacheReadTokens":6000,"cacheWriteTokens":0});
                values["subagentTiming"] = json!({"settledMs":84000});
            }
            let child: SessionSummary = serde_json::from_value(json!({
                "agentAvailable":true, "sessionId":id, "updatedAt":960,
                "running":running, "blank":false,
                "parentSessionId":"PUBLIC-layout-parent", "origin":"subagent",
                "cwd":"/PUBLIC-layout-fixture/workspace",
                "projections":{"kind":"cached","asOfSeq":-1, "values":values}
            }))
            .expect("fixed PUBLIC child metadata");
            app.sessions.insert(child.session_id.clone(), child);
        }
        let helper: SessionSummary = serde_json::from_value(json!({
            "agentAvailable":true, "sessionId":"PUBLIC-grandchild-helper", "updatedAt":970,
            "running":false, "blank":false,
            "parentSessionId":"PUBLIC-child-reviewer", "origin":"subagent",
            "cwd":"/PUBLIC-layout-fixture/workspace",
            "projections":{"kind":"cached","asOfSeq":-1,
                "values":{"title":"PUBLIC helper"}}
        }))
        .expect("fixed PUBLIC grandchild metadata");
        app.sessions.insert(helper.session_id.clone(), helper);
        app.selected =
            Some(dsh_native_transport::dto::SessionId::new("PUBLIC-child-reviewer").unwrap());
    }
    if preset == "subagent-readonly" {
        // The child is selected so the composer's read-only replacement is what the frame shows.
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
                "values":{"title":"PUBLIC one-shot child"}}
        }))
        .expect("fixed PUBLIC child metadata");
        app.selected = Some(child.session_id.clone());
        app.sessions.insert(child.session_id.clone(), child);
    }
    if preset == "subagent-catalog" {
        // A root session with two direct children, one running: the header's count action and
        // its dropdown, with the reference's running dot and mode labels.
        let parent: SessionSummary = serde_json::from_value(json!({
            "agentAvailable":true, "sessionId":"PUBLIC-layout-parent", "updatedAt":950,
            "running":false, "blank":false, "cwd":"/PUBLIC-layout-fixture/workspace",
            "projections":{"kind":"cached","asOfSeq":-1,
                "values":{"title":"PUBLIC parent with subagents",
                    "subagentCatalog":[
                        {"id":"PUBLIC-child-reviewer","createdAt":1000,"mode":"continuable","label":"PUBLIC reviewer"},
                        {"id":"PUBLIC-child-scout","createdAt":1001,"mode":"one-shot","label":"PUBLIC scout"}
                    ]}}
        }))
        .expect("fixed PUBLIC parent metadata");
        app.selected = Some(parent.session_id.clone());
        app.sessions.insert(parent.session_id.clone(), parent);
        for (id, title, running) in [
            ("PUBLIC-child-reviewer", "PUBLIC reviewer", true),
            ("PUBLIC-child-scout", "PUBLIC scout", false),
        ] {
            let child: SessionSummary = serde_json::from_value(json!({
                "agentAvailable":true, "sessionId":id, "updatedAt":960,
                "running":running, "blank":false,
                "parentSessionId":"PUBLIC-layout-parent", "origin":"subagent",
                "cwd":"/PUBLIC-layout-fixture/workspace",
                "projections":{"kind":"cached","asOfSeq":-1,
                    "values":{"title":title}}
            }))
            .expect("fixed PUBLIC child metadata");
            app.sessions.insert(child.session_id.clone(), child);
        }
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
    } else if preset == "process-group-closed" {
        // The same three recorded calls, with a turn end so the group is settled.
        let mut records = public_tool_records();
        let seq = records.len() as u64;
        records.push(
            json!({"type":"event","event":{"type":"turn/end","seq":seq,"time":1000+seq,
            "surfaceOp":"append","data":{"turn":1,"reason":{"kind":"completed"}}}}),
        );
        records
    } else if preset == "tool-activity-attachment" {
        public_tool_records_with(Some("PUBLIC-note.txt"))
    } else if preset == "tool-activity-edit-error" {
        public_tool_edit_error_records()
    } else if preset == "tool-activity-terminal" {
        public_tool_terminal_records()
    } else if preset == "tool-activity-search" {
        public_search_records()
    } else if preset == "tool-activity-web" {
        public_web_records()
    } else if preset == "tool-activity-ask" {
        public_ask_records()
    } else if preset == "tool-activity-applied" {
        public_tool_applied_records()
    } else if preset == "model-retry" {
        public_retry_records()
    } else if preset == "turn-total" {
        public_turn_total_records()
    } else if preset == "manual-compaction" {
        public_manual_compaction_records()
    } else if preset == "command-row" {
        public_command_records()
    } else if preset == "skill-row" {
        public_skill_records()
    } else if preset == "schedule-create" {
        public_schedule_create_records()
    } else if preset == "turn-error" {
        public_turn_error_records(json!({"kind":"error","error":{
            "code":"QUOTA","message":"PUBLIC provider message that the mapped copy replaces."
        }}))
    } else if preset == "turn-max-tokens" {
        public_turn_error_records(json!({"kind":"max-tokens"}))
    } else if preset == "tool-activity-diff-long" {
        // The same transcript with a longer edit, so the card's 9-row cap and its fold control
        // are drawn. Every line is synthetic PUBLIC metadata.
        let mut records = public_tool_records();
        let old_lines = (1..=30)
            .map(|n| format!("PUBLIC old line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        // Two changes far apart: each hunk keeps its three context lines and the unchanged
        // middle is one gap row, which is what takes the card past its nine-row cap. The
        // markers carry their newline so line 3 cannot also match line 30.
        let new_lines = old_lines
            .replace("PUBLIC old line 3\n", "PUBLIC new line 3\n")
            .replace("PUBLIC old line 28\n", "PUBLIC new line 28\n");
        for record in &mut records {
            let event = &mut record["event"];
            if event["type"] == "tool/call" && event["data"]["callId"] == "PUBLIC-edit-call" {
                event["data"]["arguments"] = json!(
                    json!({
                        "file_path": "/PUBLIC-layout-fixture/workspace/src/PUBLIC-note.txt",
                        "old_string": old_lines,
                        "new_string": new_lines,
                    })
                    .to_string()
                );
            }
        }
        records
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
            | "agent-preset"
            | "chat-markdown"
            | "chat-table"
            | "chat-image"
            | "chat-references"
            | "chat-search-thoughts"
            | "chat-thoughts"
            | "thoughts-preview"
            | "chat-thoughts-markdown"
            | "chat-edit-review"
            | "chat-search"
            | "chat-recovered"
            | "chat-edited"
            | "chat-search-narrow"
            | "work-panel"
            | "schedules"
            | "subagent-catalog"
            | "subagent-readonly"
            | "subagent-sibling"
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
    } else if preset == "token-usage" {
        vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                "surfaceOp":"append","data":{"role":"user","id":"PUBLIC-usage-message",
                    "source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC usage reading without a live Host."}]}}}),
            json!({"type":"event","event":{"type":"assistant/message","seq":1,"time":3001,
                "surfaceOp":"append","data":{"turn":1,"step":1,
                    "usage":{"inputTokens":1000,"outputTokens":345,
                        "cacheReadTokens":6000,"cacheWriteTokens":0,"totalTokens":7345},
                    "stream":[{"type":"text-chunks","time0":1001,"index":0,"dt":[],
                        "texts":["PUBLIC synthetic answer; the token reading is a projection, not billing."]}]}}}),
        ]
    } else if preset == "workflow-run" {
        vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                "surfaceOp":"append","data":{"role":"user","id":"PUBLIC-workflow-prompt",
                    "source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC prompt that starts a workflow."}]}}}),
            json!({"type":"event","event":{"type":"tool-workflow/run-start","seq":1,"time":1001,
                "data":{"runId":"PUBLIC-run","name":"PUBLIC review workflow"}}}),
            json!({"type":"event","event":{"type":"tool-workflow/agent-start","seq":2,"time":1002,
                "data":{"runId":"PUBLIC-run","seq":1,"label":"PUBLIC research","phase":"research","childId":"PUBLIC-child-one"}}}),
            json!({"type":"event","event":{"type":"tool-workflow/agent-end","seq":3,"time":1003,
                "data":{"runId":"PUBLIC-run","seq":1,"outcome":"completed"}}}),
            json!({"type":"event","event":{"type":"tool-workflow/agent-start","seq":4,"time":1004,
                "data":{"runId":"PUBLIC-run","seq":2,"label":"PUBLIC write","phase":"write","childId":"PUBLIC-child-two"}}}),
            json!({"type":"event","event":{"type":"tool-workflow/run-end","seq":5,"time":1005,
                "data":{"runId":"PUBLIC-run","stopReason":"completed"}}}),
        ]
    } else if preset == "feedback" {
        vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                "surfaceOp":"append","data":{"role":"user","id":"PUBLIC-feedback-prompt",
                    "source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC prompt for rated feedback."}]}}}),
            json!({"type":"event","event":{"type":"assistant/message","seq":1,"time":1001,
                "surfaceOp":"append","data":{"turn":1,"step":1,
                    "message":{"id":"PUBLIC-feedback-message","role":"assistant",
                        "source":{"kind":"model","provider":"PUBLIC","model":"PUBLIC"},
                        "content":[{"type":"text","text":"PUBLIC synthetic answer carrying a positive rating."}]},
                    "stream":[{"type":"text-chunks","time0":1001,"index":0,"dt":[],
                        "texts":["PUBLIC synthetic answer carrying a positive rating."]}]}}}),
        ]
    } else if preset == "changes-card" {
        vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                "surfaceOp":"append","data":{"role":"user","id":"PUBLIC-changes-message",
                    "source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC turn with changed files."}]}}}),
            json!({"type":"event","event":{"type":"assistant/message","seq":1,"time":1001,
                "surfaceOp":"append","data":{"turn":1,"step":1,
                    "stream":[{"type":"text-chunks","time0":1001,"index":0,"dt":[],
                        "texts":["PUBLIC synthetic answer whose turn changed two files."]}]}}}),
            json!({"type":"event","event":{"type":"workspace/changes","seq":2,"time":1002,
                "data":{"turn":1}}}),
        ]
    } else if preset == "turn-trigger" {
        vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                "surfaceOp":"append","data":{"role":"user","id":"PUBLIC-trigger-message",
                    "source":{"kind":"goal"},
                    "content":[{"type":"text","text":"PUBLIC goal reminder · continue the recorded task without sending a prompt."}]}}}),
            json!({"type":"event","event":{"type":"assistant/message","seq":1,"time":1001,
                "surfaceOp":"append","data":{"turn":1,"step":1,
                    "stream":[{"type":"text-chunks","time0":1001,"index":0,"dt":[],
                        "texts":["PUBLIC synthetic answer to the goal reminder."]}]}}}),
        ]
    } else if preset == "goal-bar" {
        vec![
            json!({"type":"event","event":{"type":"user/message","seq":0,"time":1000,
                "surfaceOp":"append","data":{"role":"user","id":"PUBLIC-goal-message",
                    "source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC conversation under an ongoing goal."}]}}}),
            json!({"type":"event","event":{"type":"assistant/message","seq":1,"time":1001,
                "surfaceOp":"append","data":{"turn":1,"step":1,
                    "stream":[{"type":"text-chunks","time0":1001,"index":0,"dt":[],
                        "texts":["PUBLIC synthetic answer with a goal projection."]}]}}}),
        ]
    } else if preset == "conversation" || preset.starts_with("working-") {
        let records = vec![
            json!({"type":"event", "event":{"type":"user/message", "seq":0, "time":1000,
                "data":{"role":"user", "id":"PUBLIC-user-message", "source":{"kind":"user"},
                    "content":[{"type":"text", "text":"PUBLIC layout note · café · 世界\nCompare the real conversation bubbles and composer without sending a prompt."}]},
                "surfaceOp":"append"}}),
            json!({"type":"event", "event":{"type":"assistant/message", "seq":1, "time":1001,
                "data":{"turn":1, "step":1,
                    "stream":[{"type":"text-chunks", "time0":1001, "index":0, "dt":[],
                        "texts":["PUBLIC synthetic assistant text. This durable snapshot is local metadata, not a model response or a business acknowledgement."]}]},
                "surfaceOp":"append"}}),
        ];
        records
    } else if preset == "permission-chip" {
        // The chip's projection reads the most recent `permission/preset` record
        // (`reducer.rs:329`), which a real Host writes near the start of a session.
        // The record is not a chat row, so the conversation keeps its two rows; it
        // is what gives the chip a repeatable capture with no Host.
        vec![
            json!({"type":"event","event":{"type":"permission/preset","seq":0,"time":1000,
                "data":{"preset":"workspace-write"}}}),
            json!({"type":"event","event":{"type":"user/message","seq":1,"time":1001,
                "surfaceOp":"append","data":{"role":"user","id":"PUBLIC-permission-message",
                    "source":{"kind":"user"},
                    "content":[{"type":"text","text":"PUBLIC prompt beside the permission chip."}]}}}),
            json!({"type":"event","event":{"type":"assistant/message","seq":2,"time":1002,
                "surfaceOp":"append","data":{"turn":1,"step":1,
                    "stream":[{"type":"text-chunks","time0":1002,"index":0,"dt":[],
                        "texts":["PUBLIC synthetic answer. The preset is a recorded projection, not an enforcement claim."]}]}}}),
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
        "agent-preset" => json!({"agentPreset":{"id":"implementer","name":"Implementer"}}),
        "subagent-catalog" => json!({"subagentCatalog":[
            {"id":"PUBLIC-child-reviewer","createdAt":1000,"mode":"continuable","label":"PUBLIC reviewer"},
            {"id":"PUBLIC-child-scout","createdAt":1001,"mode":"one-shot","label":"PUBLIC scout"}
        ]}),
        // The Host publishes the logged state plus whether a selection differs from it
        // (`plan-mode/src/index.ts:165`): a pending change while inactive is an *enter*, and while
        // active it is a *leave*. The fixtures were inverted before round 334.
        "plan-mode" => json!({"plan":{"active":true,"pending":false}}),
        "plan-mode-applying" => json!({"plan":{"active":false,"pending":true}}),
        "plan-mode-leaving" => json!({"plan":{"active":true,"pending":true}}),
        "goal-bar" => json!({"goal":{"goal":{
            "id":"PUBLIC-goal","revision":2,
            "objective":"PUBLIC goal objective · finish the recorded clone task",
            "phase":"active","maxGoalRounds":64
        },"roundsStarted":1,"createdAt":1000,"updatedAt":2000}}),
        // The Host's own statistics, which the pill's dialog renders verbatim
        // (`session-stats/src/projection.ts:72-80`).
        "token-usage" | "stats-dialog" => json!({
            "tokenUsage":{
                "uncachedInputTokens":6000,"outputTokens":345,
                "cacheReadTokens":6000,"cacheWriteTokens":0
            },
            "sessionStats":{
                "turns":2,"steps":3,"llmMs":1500,"toolMs":4500,
                "ttftMs":900,"ttftSteps":3,"decodeMs":2000,"decodeTokens":84
            }
        }),
        "token-usage-old" => json!({"tokenUsage":{
            "uncachedInputTokens":6000,"outputTokens":345,
            "cacheReadTokens":6000,"cacheWriteTokens":0
        }}),
        "queued-idle" | "working-queue" => json!({"inbox":{"next-turn":[
            {"id":"PUBLIC-queued-one","source":{"kind":"user"},
                "content":[{"type":"text","text":"PUBLIC queued question about the running turn."}]},
            {"id":"PUBLIC-queued-two","source":{"kind":"user"},
                "content":[{"type":"text","text":"PUBLIC second queued note · café · 世界"}]}
        ]}}),
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
    assert!(
        app.follow_ready && app.smoke_evidence.snapshot_valid && app.transcript.safe_to_send(),
        "preset {preset}: follow={} snapshot={} safe={} cursor={} opening={}",
        app.follow_ready,
        app.smoke_evidence.snapshot_valid,
        app.transcript.safe_to_send(),
        app.transcript.cursor(),
        app.transcript.opening_cursor()
    );
    if preset == "assistant-tool-call-expanded" {
        let ticket = app.reply_parts(7).unwrap().ticket.unwrap();
        drop(app.update(Message::ToggleThoughts(ticket)));
        assert!(app.thoughts_expanded.contains(&7));
    }
    if preset == "feedback" {
        app.seed_feedback();
    }
    if preset == "changes-card" {
        app.seed_changes_summary();
    }
    if preset == "mcp" {
        // The panel's own read answer, delivered through the controller the app draws from.
        let effect = app.mcp.enter(app.transport_epoch);
        let crate::mcp::Effect::Read(ticket) = effect else {
            panic!("entering MCP must read");
        };
        let http = |name: &str, url: &str, enabled: bool| crate::mcp::ServerEntry {
            server_name: name.to_owned(),
            transport: crate::mcp::Transport::StreamableHttp,
            enabled,
            command: String::new(),
            args: Vec::new(),
            env: Vec::new(),
            cwd: String::new(),
            url: url.to_owned(),
            headers: vec![(
                "Authorization".to_owned(),
                crate::mcp::secret("PUBLIC-token".to_owned()),
            )],
            tool_call_timeout_ms: 60_000,
            fail_on_startup_error: false,
        };
        app.mcp.loaded_snapshot(
            ticket,
            crate::mcp::Loaded {
                path: "/PUBLIC-native-home/cordis.patch.yml".to_owned(),
                revision: Some(crate::mcp::Revision {
                    bytes: 420,
                    hash: 7,
                }),
                entries: vec![
                    crate::mcp::ServerEntry {
                        server_name: "files".to_owned(),
                        transport: crate::mcp::Transport::Stdio,
                        enabled: true,
                        command: "/usr/bin/npx".to_owned(),
                        args: vec!["-y".to_owned(), "@PUBLIC/files-server".to_owned()],
                        env: vec![(
                            "API_TOKEN".to_owned(),
                            crate::mcp::secret("PUBLIC-token".to_owned()),
                        )],
                        cwd: String::new(),
                        url: String::new(),
                        headers: Vec::new(),
                        tool_call_timeout_ms: 60_000,
                        fail_on_startup_error: false,
                    },
                    http("search", "https://PUBLIC.invalid/mcp", false),
                ],
                foreign: None,
                read_error: None,
                inventory: Vec::new(),
                inventory_read: true,
            },
        );
    }
    if preset == "schedules" {
        app.schedules_open = true;
        app.schedules_loading = false;
        app.schedules = vec![
            dsh_native_transport::dto::ScheduleCatalogItem {
                id: "PUBLIC-schedule-daily".into(),
                title: "PUBLIC daily reminder".into(),
                kind: "daily".into(),
                session_id: app.selected.clone().expect("selected fixture session"),
                status: "active".into(),
                scheduled_at: Some("2026-10-17T09:00:00Z".into()),
                last_delivery: Some(dsh_native_transport::dto::ScheduleDeliveryStamp {
                    scheduled_at: "2026-10-16T09:00:00Z".into(),
                    delivered_at: "2026-10-16T09:00:01Z".into(),
                }),
                record: serde_json::json!({"prompt":"PUBLIC daily instruction",
                    "time":"09:00:00","timeZone":"UTC"})
                .as_object()
                .unwrap()
                .clone(),
            },
            dsh_native_transport::dto::ScheduleCatalogItem {
                id: "PUBLIC-schedule-paused".into(),
                title: "PUBLIC inactive reminder".into(),
                kind: "weekly".into(),
                session_id: app.selected.clone().expect("selected fixture session"),
                status: "inactive".into(),
                scheduled_at: None,
                last_delivery: None,
                record: serde_json::json!({"prompt":"PUBLIC weekly instruction",
                    "time":"17:30:00","timeZone":"UTC","weekdays":[1,3,5]})
                .as_object()
                .unwrap()
                .clone(),
            },
        ];
    }
    if preset == "chat-thoughts" {
        let ticket = app.reply_parts(1).unwrap().ticket.unwrap();
        drop(app.update(Message::ToggleThoughts(ticket)));
        assert!(app.thoughts_expanded.contains(&1));
    }
    if matches!(preset, "chat-thoughts-markdown" | "chat-search-thoughts") {
        // The shared fixture's assistant reply is seq 11.
        let ticket = app.reply_parts(11).unwrap().ticket.unwrap();
        drop(app.update(Message::ToggleThoughts(ticket)));
        assert!(app.thoughts_expanded.contains(&11));
    }
    if preset == "chat-search-thoughts" {
        // Drives the real search path so a grab can check the highlight sits on the
        // glyph it matched inside formatted reasoning.
        app.search_open = true;
        app.search_serial = 7;
        drop(app.update(Message::SearchInput {
            generation: app.generation,
            serial: 7,
            query: "Second compact".into(),
        }));
        assert_eq!(app.search_query, "Second compact");
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
        // Chat draws one row per call, as the reference does, and each call carries its result;
        // Trajectory still lists every record, so it keeps the three result rows and the two turn
        // boundary records.
        let records = app.records_expanded;
        assert_eq!(
            app.transcript.display_rows(records).count(),
            if records { 8 } else { 5 },
            "preset {preset}"
        );
        assert_eq!(
            app.transcript
                .rows()
                .iter()
                .filter(|row| row.role == "Tool call" && row.output.is_some())
                .count(),
            3,
            "every paired call carries its result"
        );
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
        "navigation-search-tiny" => {
            // The compact sheet owns the search box, so it must own the answer too.
            app.sidebar_sheet = true;
            app.seed_search_results();
        }
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
            // The fixture renders state, it does not dispatch effects. The production open path
            // is covered by App tests; this disabled handle keeps every preset at zero commands
            // while still exercising the reducer's page selection.
            assert!(
                app.settings
                    .handle(Action::Open, app.transport_epoch, false)
                    .is_none()
            );
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
            assert!(
                app.settings
                    .handle(Action::SelectPage(page), app.transport_epoch, false)
                    .is_none()
            );
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
                app.settings.seed_plugin_snapshot(crate::plugins::Snapshot {
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
                });
            }
            if preset == "settings-presets" {
                // The Host's own roster shape, seeded here as its read reply would be: the four
                // shipped presets, one named declaration and one that failed.
                app.set_agent_presets_section_roster();
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
                    assert!(
                        app.settings
                            .handle(
                                Action::SelectPage(Page::General),
                                app.transport_epoch,
                                false
                            )
                            .is_none()
                    );
                }
            }
            if preset.starts_with("settings-general-usage") {
                assert!(
                    app.settings
                        .handle(
                            Action::SelectPage(Page::General),
                            app.transport_epoch,
                            false
                        )
                        .is_none()
                );
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
        "model-retry" => {
            // One row for the chain, stating the latest attempt and carrying the failure reason.
            let rows: Vec<_> = app
                .transcript
                .display_rows(false)
                .filter(|row| row.retry.is_some())
                .collect();
            assert_eq!(rows.len(), 1, "one row per chain");
            assert_eq!(rows[0].text, "Waiting to retry model request (2/3) · 4s");
            assert_eq!(
                rows[0]
                    .retry
                    .as_ref()
                    .and_then(|state| state.detail())
                    .as_deref(),
                Some("Retry delay: 4s\nFailure reason: PUBLIC upstream unavailable · synthetic")
            );
        }
        "thoughts-preview" => {
            // A settled reasoning row previews its first line beside the title: the default
            // Standard mode enables it and this preset leaves thoughts folded.
            let rows: Vec<_> = app.transcript.display_rows(false).collect();
            let roles: Vec<String> = rows
                .iter()
                .map(|row| format!("{}:{}", row.key, row.role))
                .collect();
            let row = rows
                .iter()
                .find(|row| row.role.starts_with("Assistant"))
                .unwrap_or_else(|| panic!("DROLES {roles:?}"));
            assert_eq!(
                app.thoughts_preview(row).as_deref(),
                Some(
                    "PUBLIC backend-supplied thoughts. This disclosure is separate from the final res…"
                )
            );
            assert!(!app.thoughts_expanded.contains(&row.key));
        }
        "queued-idle" => {
            // The strip's rows carry the steering gate: an idle session cannot steer, so the
            // control is drawn inert and the strip states why in place of the reference's tooltip.
            app.queue_expanded = true;
            assert!(!app.queue_steer_enabled());
            assert!(app.transcript.queued().len() == 2);
        }
        "working-queue" => {
            app.queue_expanded = true;
            // A running session with two queued messages: the strip offers `Steer all`, which walks
            // the queue one `updateQueue` at a time (`input/hub.ts:256-270`).
            assert!(app.queue_steer_enabled(), "a running session can steer");
            assert!(app.transcript.queued().len() == 2);
        }
        "process-group-closed" => {
            let rows: Vec<_> = app.transcript.display_rows(false).collect();
            let seats = app.process_seats(&rows);
            assert_eq!(seats.len(), 1);
            let seat = seats.values().next().unwrap();
            assert!(!seat.open, "a settled turn's group starts closed");
            assert_eq!(seat.title, "Read files, edited files, ran commands");
        }
        "manual-compaction" => {
            // One marker, stating the counts the compaction summary recorded.
            let rows: Vec<_> = app
                .transcript
                .display_rows(false)
                .filter(|row| row.command.is_some())
                .collect();
            assert_eq!(rows.len(), 1, "one marker for the command and its summary");
            // The row must reserve its own line, or the view's height range skips it.
            let lane = app.chat_lane();
            let geometry = app.row_geometry(rows[0], lane);
            assert!(geometry.height > 0.0, "height {}", geometry.height);
            assert!(geometry.width > 0.0, "width {}", geometry.width);
            // The row must be reachable by the view's first branch, which asks `activity`; a
            // command record must not answer with a tool summary.
            assert!(
                app.transcript
                    .activity(rows[0].key, app.path_roots())
                    .is_none(),
                "a command row is not a tool row"
            );
            assert_eq!(
                rows[0].text,
                "compact · Compacted 7 history items (~12.4k tokens)"
            );
            assert_eq!(
                rows[0].command.as_ref().unwrap().manual_compaction,
                Some((7, 12_400))
            );
            // The compaction summary is not drawn a second time.
            assert!(
                app.transcript
                    .display_rows(false)
                    .all(|row| row.role != "Context summarized")
            );
        }
        "command-row" => {
            let (key, name, args, body, text) = {
                let row = app
                    .transcript
                    .rows()
                    .iter()
                    .find(|row| row.command.is_some())
                    .expect("the command row");
                let command = row.command.as_ref().unwrap();
                (
                    row.key,
                    command.name.clone(),
                    command.args.clone(),
                    command.body().is_some(),
                    row.text.clone(),
                )
            };
            assert_eq!(name, "/compact");
            assert_eq!(args.as_deref(), Some("keep the plan"));
            // Three output lines make the body expandable, so the summary is the verdict.
            assert!(body);
            assert_eq!(text, "/compact · Completed");
            drop(app.update(Message::ToggleActivity {
                generation: app.generation,
                key,
            }));
            assert!(app.activity_expanded.contains(&key));
        }
        "skill-row" => {
            let summary = app
                .transcript
                .activity(1, app.path_roots())
                .expect("the skill row's label");
            assert_eq!(summary.title, "Skill · skill");
            assert_eq!(summary.target.as_deref(), Some("impeccable-rust"));
            drop(app.update(Message::ToggleActivity {
                generation: app.generation,
                key: 1,
            }));
            assert!(app.activity_expanded.contains(&1));
        }
        "schedule-create" => {
            let card = app
                .transcript
                .activity(1, app.path_roots())
                .and_then(|activity| activity.schedule)
                .expect("the created-task card");
            assert_eq!(card.title, "PUBLIC weekly fixture reminder");
            assert_eq!(
                card.frequency().as_deref(),
                Some("Weekly on Mon, Fri at 09:00 (UTC)")
            );
            assert_eq!(card.fallback, None);
            drop(app.update(Message::ToggleActivity {
                generation: app.generation,
                key: 1,
            }));
            assert!(app.activity_expanded.contains(&1));
        }
        "turn-error" => {
            let notice = app
                .transcript
                .rows()
                .iter()
                .find_map(|row| row.notice.clone())
                .expect("the failed turn's row");
            assert_eq!(notice.title, "This turn failed");
            // The reference's own mapping for a quota code, not the provider's sentence.
            assert_eq!(notice.message, "Request quota exhausted.");
            assert_eq!(notice.code.as_deref(), Some("QUOTA"));
        }
        "turn-max-tokens" => {
            let notice = app
                .transcript
                .rows()
                .iter()
                .find_map(|row| row.notice.clone())
                .expect("the capped turn's row");
            assert_eq!(notice.title, "Output token limit reached");
            assert!(notice.message.contains("Send \"continue\""));
            assert_eq!(notice.code, None);
        }
        "settings-shortcuts" => {
            // The editor's own rows now carry the reference's `settings.open` chord. The shared
            // settings block above already opened General with every effect disabled, which is the
            // pattern that keeps a capture frame at zero commands; this arm only unfolds the editor.
            app.settings.layout_open(crate::settings::Page::General);
            app.settings.shortcuts_open = true;
            assert!(app.settings.is_open());
            let rows = crate::keybindings::rows(
                &app.settings.keybindings,
                &crate::keybindings::native_catalog(),
            );
            let settings_row = rows
                .iter()
                .find(|row| row.id == "native.settings")
                .expect("the settings command's row");
            assert_eq!(settings_row.label, "Open settings");
            assert!(!settings_row.keys.is_empty(), "the chord is shown");
        }
        "stats-dialog" => {
            // The pill carries the counts and opens the Host's figures.
            let stats = app.stats().expect("the seeded projection");
            assert_eq!(stats.counts(), "2 turns 3 steps");
            assert!(stats.has_dialog(), "timed figures make the pill a button");
            assert!(!app.stats_open);
            drop(app.update(Message::ToggleStats));
            assert!(app.stats_open, "the press opens the dialog");
            assert_eq!(
                stats.rows(),
                vec![
                    ("LLM time", "1.5s".to_owned()),
                    ("Tool time", "4.5s".to_owned()),
                    ("Avg time to first token (TTFT)", "0.3s".to_owned()),
                    ("Tokens per second (TPS)", "42 tok/s".to_owned()),
                ]
            );
            drop(app.update(Message::ToggleStats));
            assert!(!app.stats_open, "the same press closes it");
            // Left open, because this preset's whole point is the dialog's own frame.
            drop(app.update(Message::ToggleStats));
            assert!(app.stats_open);
        }
        "turn-total" => {
            // 143 seconds between the turn's start and end: `Completed in 2m 23s`.
            let duration = app.transcript.turn_duration(1).expect("a settled turn");
            assert_eq!(duration.seconds, 143);
            assert_eq!(
                crate::reducer::turn_tail::label(Some(duration)),
                "Completed in 2m 23s"
            );
        }
        "command-hint" => {
            app.editor = iced::widget::text_editor::Content::with_text("/plan");
            assert_eq!(
                app.composer_hint(),
                Some("describe your task to generate plan")
            );
        }
        "slash-loading" => {
            // A slash typed before the Host answered: the menu states that it is loading rather
            // than staying invisible (`MenuView.tsx:147-152`).
            app.commands = None;
            app.commands_pending = true;
            app.editor = iced::widget::text_editor::Content::with_text("/co");
            assert!(app.completion().is_none(), "no rows yet");
            assert!(app.awaiting_advertisement(), "the loading row is shown");
        }
        "skills-menu" => {
            // Two sources in one menu: the Host's commands, then its skill catalog, each titled
            // as the reference titles its groups (`MenuView.tsx:146`).
            app.commands = Some(vec![super::command_menu::Candidate::command(
                "compact".into(),
                "Condense older history into a summary".into(),
            )]);
            app.skills = Some(vec![super::command_menu::Candidate::skill(
                "impeccable-rust".into(),
                "Risk-driven Rust review".into(),
            )]);
            app.editor = iced::widget::text_editor::Content::with_text("/c");
            let menu = app.completion().expect("both sources complete");
            assert_eq!(menu.matches.len(), 2);
            assert_eq!(menu.matches[0].source, super::command_menu::Source::Command);
            assert_eq!(menu.matches[1].source, super::command_menu::Source::Skill);
        }
        "drop-overlay" => {
            // A file hovering the window draws the reference's drop banner with its limits.
            // The fixture has no OS window id, so a synthetic one stands in: the handler only
            // compares it with `own_window`, which the arm sets first.
            let window = iced::window::Id::unique();
            app.own_window = Some(window);
            drop(app.update(Message::FileHovered(
                window,
                "/PUBLIC-dropped-file.txt".into(),
            )));
            assert!(app.drop_hover, "the overlay is drawn");
        }
        "attach-menu" => {
            // The left control offers files and commands in one popup, so a mouse user reaches
            // the command menu without typing a slash (`InputBar.tsx:428-441`).
            app.commands = Some(vec![super::command_menu::Candidate::command(
                "compact".into(),
                "Condense older history into a summary".into(),
            )]);
            app.editor = iced::widget::text_editor::Content::with_text("PUBLIC draft");
            drop(app.update(Message::ToggleAttachMenu(true)));
            assert!(app.attach_menu, "the menu is open");
        }
        "command-menu-mid-draft" => {
            // The reference scans back from the caret to whitespace (`detect.ts:61-75`), so the
            // menu opens for a slash after prose — the case a leading-slash-only rule missed.
            app.commands = Some(vec![super::command_menu::Candidate::command(
                "compact".into(),
                "Condense older history into a summary".into(),
            )]);
            app.editor = iced::widget::text_editor::Content::with_text("PUBLIC say /co");
            let completion = app
                .completion()
                .expect("the slash after prose opens the menu");
            assert_eq!(completion.matches.len(), 1);
            assert_eq!(completion.matches[0].name, "compact");
        }
        "command-menu" => {
            // The composer asked for the advertisement when the slash was typed;
            // this stands in for the Host's own reply, which is the only source.
            app.commands = Some(vec![
                super::command_menu::Candidate::command(
                    "plan".into(),
                    "Explore and design before executing".into(),
                ),
                super::command_menu::Candidate::command(
                    "compact".into(),
                    "Condense older history into a summary".into(),
                ),
                super::command_menu::Candidate::command(
                    "help".into(),
                    "Show the available commands".into(),
                ),
            ]);
            app.editor = iced::widget::text_editor::Content::with_text("/");
            assert!(
                app.completion().is_some(),
                "a slash draft with an advertisement must open the menu"
            );
            assert_eq!(app.completion().unwrap().matches.len(), 3);
        }
        "plan-mode" | "plan-mode-applying" => {
            // A plan-mode composer states the next action, as the reference's placeholder does.
            assert_eq!(
                app.composer_placeholder(),
                "describe your task to generate plan",
                "plan mode states its own placeholder"
            );
        }
        "plan-chip" => {
            assert_eq!(
                app.transcript.work().plan(),
                Some((1, 4, Some("PUBLIC fix the fold")))
            );
            // The item list and its counts are written together, so the dock always has the
            // rows the counts describe: there is no counts-only state to fall back to.
            assert_eq!(app.transcript.work().plan_items().len(), 4);
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
        "tool-activity-diff" => {
            // The edit call expands into the reference's diff card, inside the conversation.
            assert!(
                app.transcript
                    .activity(3, app.path_roots())
                    .is_some_and(|activity| activity.diff.is_some()),
                "an edit call carries its diff card"
            );
            drop(app.update(Message::ToggleActivity {
                generation: app.generation,
                key: 3,
            }));
            assert!(app.activity_expanded.contains(&3));
            assert!(app.detail.is_none() && app.composer_visible());
        }
        "tool-activity-diff-long" => {
            let diff = app
                .transcript
                .activity(3, app.path_roots())
                .and_then(|activity| activity.diff)
                .expect("the long edit carries its diff card");
            assert!(
                crate::reducer::inline_diff::shown(diff.lines.len(), false).hidden > 0,
                "the card is capped and draws its fold control"
            );
            drop(app.update(Message::ToggleActivity {
                generation: app.generation,
                key: 3,
            }));
            assert!(app.activity_expanded.contains(&3));
        }
        "tool-activity-read" => {
            // The read call expands to the result it folded, without the protocol envelope the
            // session stores around it. Key 1 is the call; its result record (seq 2) is
            // Trajectory-only now that Chat draws one row per call.
            let call = app
                .transcript
                .rows()
                .iter()
                .find(|row| row.key == 1)
                .cloned()
                .expect("the read call row");
            // The read call draws the reference's line-numbered card, not its recorded text:
            // two numbered lines and the window status, from the result's own metadata.
            let card = app
                .transcript
                .activity(1, app.path_roots())
                .and_then(|activity| activity.read)
                .expect("the read call's card");
            assert_eq!(card.label, "src/PUBLIC-note.txt");
            assert_eq!(card.lines.len(), 2);
            assert_eq!(card.lines[1].number, 2);
            assert_eq!(card.lines[1].text, "PUBLIC note line two");
            assert_eq!(card.status().as_deref(), Some("Showing 2 of 42 lines"));
            let body = super::activity_body_source(&call);
            assert!(
                !body.contains("<path>") && !body.contains("<content>"),
                "{body}"
            );
            assert!(body.contains("PUBLIC note line two"), "{body}");
            drop(app.update(Message::ToggleActivity {
                generation: app.generation,
                key: 1,
            }));
            assert!(app.activity_expanded.contains(&1));
        }
        "tool-activity-terminal" => {
            // The bash call expands into the reference's terminal card: the prompt line from the
            // call's own arguments and working directory, the output rows, and the exit pill the
            // shell carries as result data rather than as an error flag.
            let card = app
                .transcript
                .activity(5, app.path_roots())
                .and_then(|activity| activity.terminal)
                .expect("the bash call's terminal card");
            assert_eq!(card.command, vec!["printf PUBLIC_fixture_no_execution"]);
            assert_eq!(card.status.as_deref(), Some("exit code 2"));
            assert_eq!(
                card.output,
                vec![
                    "PUBLIC terminal output line one".to_owned(),
                    "PUBLIC terminal output line two".to_owned()
                ]
            );
            drop(app.update(Message::ToggleActivity {
                generation: app.generation,
                key: 5,
            }));
            assert!(app.activity_expanded.contains(&5));
        }
        "tool-activity-search" => {
            assert_eq!(app.transcript.display_rows(false).count(), 3);
            // The grep call expands into the reference's search card: the banner states both the
            // retained and the pre-cap count, then each file's header and its numbered matches.
            let card = app
                .transcript
                .activity(1, app.path_roots())
                .and_then(|activity| activity.search)
                .expect("the grep call's search card");
            assert_eq!(card.summary(), "Showing 3 of 120 matches · 2 files");
            assert_eq!(
                card.rows(),
                vec![
                    crate::reducer::search_card::Row::File {
                        path: "src/PUBLIC-a.rs".into(),
                        count: 2
                    },
                    crate::reducer::search_card::Row::Match {
                        line_number: 4,
                        line: "PUBLIC_marker in a".into()
                    },
                    crate::reducer::search_card::Row::Match {
                        line_number: 9,
                        line: "PUBLIC_marker again in a".into()
                    },
                    crate::reducer::search_card::Row::File {
                        path: "src/PUBLIC-b.rs".into(),
                        count: 1
                    },
                    crate::reducer::search_card::Row::Match {
                        line_number: 1,
                        line: "PUBLIC_marker in b".into()
                    },
                ]
            );
            drop(app.update(Message::ToggleActivity {
                generation: app.generation,
                key: 1,
            }));
            assert!(app.activity_expanded.contains(&1));
        }
        "tool-activity-web" => {
            assert_eq!(app.transcript.display_rows(false).count(), 3);
            // The search call expands into the reference's web card: the provider answer over the
            // numbered sources, then the provider's own truncation note.
            let card = app
                .transcript
                .activity(1, app.path_roots())
                .and_then(|activity| activity.web)
                .expect("the web call's card");
            let rows = card.rows();
            assert_eq!(rows.len(), 3);
            assert!(
                matches!(&rows[0], crate::reducer::web_card::Row::Answer(text)
                if text.contains("PUBLIC provider answer"))
            );
            assert!(
                matches!(&rows[1], crate::reducer::web_card::Row::Source { ordinal: 1, source }
                if source.title.as_deref() == Some("PUBLIC source one")
                    && source.published_at.is_some())
            );
            assert!(matches!(
                &rows[2],
                crate::reducer::web_card::Row::Source { ordinal: 2, .. }
            ));
            assert_eq!(card.note(), "Source list truncated");
            drop(app.update(Message::ToggleActivity {
                generation: app.generation,
                key: 1,
            }));
            assert!(app.activity_expanded.contains(&1));
        }
        "tool-activity-ask" => {
            assert_eq!(app.transcript.display_rows(false).count(), 3);
            // The question call expands into the reference's transcript: each question with what
            // was chosen, and `Not answered` where nothing was.
            let card = app
                .transcript
                .activity(1, app.path_roots())
                .and_then(|activity| activity.ask)
                .expect("the question call's card");
            assert_eq!(card.summary(), "1/2 answered");
            assert_eq!(
                card.rows(),
                vec![
                    crate::reducer::ask_card::Row::Question(
                        "PUBLIC which surface should the fixture draw?".into()
                    ),
                    crate::reducer::ask_card::Row::Answer("Chat".into()),
                    crate::reducer::ask_card::Row::Answer("PUBLIC typed note".into()),
                    crate::reducer::ask_card::Row::Question(
                        "PUBLIC anything else to record?".into()
                    ),
                    crate::reducer::ask_card::Row::Skipped,
                ]
            );
            drop(app.update(Message::ToggleActivity {
                generation: app.generation,
                key: 1,
            }));
            assert!(app.activity_expanded.contains(&1));
        }
        "tool-activity-applied" => {
            // The card draws the applied record, not the intended arguments.
            let diff = app
                .transcript
                .activity(3, app.path_roots())
                .and_then(|activity| activity.diff)
                .expect("the edit call's applied diff");
            let text: Vec<&str> = diff.lines.iter().map(|line| line.text.as_str()).collect();
            assert!(text.contains(&"PUBLIC applied text"), "{text:?}");
            assert!(!text.contains(&"PUBLIC new text"), "{text:?}");
            // The row's totals are the applied change's, not the arguments'.
            let summary = app.transcript.activity(3, app.path_roots()).unwrap();
            assert_eq!(summary.edits, Some((2, 1)));
            drop(app.update(Message::ToggleActivity {
                generation: app.generation,
                key: 3,
            }));
            assert!(app.activity_expanded.contains(&3));
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
        // Chat folds each result into its call (one row per call); Trajectory keeps all records.
        assert_eq!(
            app.transcript.display_rows(app.records_expanded).count(),
            if app.records_expanded { 8 } else { 5 }
        );
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
    let schedules_expected = preset == "schedules";
    // The MCP panel is a full-pane page like Schedules: it owns the body and hides the composer.
    let mcp_expected = preset == "mcp";
    let context_expected = preset.starts_with("context-management");
    let plan_projected = app.transcript.work().plan().is_some();
    let context_warning = app.composer_visible() && app.context_policy_open;
    let picker_expected = preset == "file-picker";
    let sidebar_expected = matches!(
        preset,
        "sidebar-options" | "sidebar-options-tiny" | "sidebar-trash-review"
    );
    // A one-shot child still owns the composer pane, but the reference replaces the input
    // and its controls with a read-only frame: the pane is expected, the controls are not.
    let composer_replaced = preset == "subagent-readonly";
    let composer_expected = !schedules_expected
        && !mcp_expected
        && !settings_expected
        && !picker_expected
        && !sidebar_expected
        && !info_expected
        && !work_expected
        && !preset.starts_with("assistant-details")
        && !matches!(
            preset,
            "menu"
                | "navigation"
                | "navigation-tiny"
                | "navigation-search-tiny"
                | "tool-activity-details"
        );
    let reasoning_expected = matches!(preset, "long-model" | "tiny-long-model")
        || preset.starts_with("model-metadata")
        || (app.model_controls.is_open() && !preset.ends_with("empty"));
    let summaries: Vec<_> = app
        .transcript
        .rows()
        .iter()
        .filter_map(|row| app.transcript.activity(row.key, app.path_roots()))
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
        "composerReplacedExpected":composer_replaced,
        "composerStateMatchesPreset":app.composer_visible() == composer_expected,
        "settingsEntryExpected":!settings_expected && !picker_expected && !sidebar_expected,
        "stopExpected":!settings_expected && !picker_expected && !sidebar_expected && !composer_replaced,
        "sidebarOptionsExpected":sidebar_expected,
        "sidebarOrganizationSimulated":preset.starts_with("sidebar-"),
        "sidebarStoreIoExecuted":false,
        "modelControlsExpected":composer_expected && !composer_replaced,
        "reasoningControlExpected":app.model_controls.is_open() && (app.model_controls.section().is_none() || app.layout_size().width >= 720.0),
        "headerExpected":!settings_expected && !picker_expected && !sidebar_expected && !matches!(preset, "navigation" | "navigation-tiny" | "navigation-search-tiny"),
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
        "conversationSnapshotSimulated":matches!(preset, "conversation" | "inbox-canceled" | "changes-card" | "feedback" | "workflow-run") || preset.starts_with("tool-activity"),
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
        "schedulesPanelStateMatchesPreset":app.schedules_open == schedules_expected,
        "contextManagementSimulated":context_expected,
        "contextTrajectoryShown":context_expected && app.records_expanded,
        "todoPlanProjected":plan_projected,
        "todoDockItemCount":app.transcript.work().plan_items().len(),
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

/// The options every fixture build uses: paths under `/PUBLIC-layout-fixture` that no worker ever
/// starts, a unit scale, and no accessibility announcement.
#[cfg(test)]
pub(crate) fn test_options() -> Options {
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

#[cfg(test)]
mod tests {
    use super::*;
    fn options() -> Options {
        test_options()
    }
    /// Every Chat row in every preset reserves a line of its own. A row whose geometry collapses
    /// to zero is drawn outside the scroll projection's height range, so it never paints — the
    /// failure mode that hid the command row in round 391 — and no other test sees it.
    #[test]
    fn every_preset_row_reserves_its_own_line() {
        let mut empty = Vec::new();
        for &preset in PRESETS {
            let (size, scale) = preset_size(preset).unwrap();
            let mut options = options();
            options.scale = scale;
            let (app, _receiver) = build(options, preset, size, None);
            let lane = app.chat_lane();
            for row in app.transcript.display_rows(false) {
                let geometry = app.row_geometry(row, lane);
                if geometry.height <= 0.0 {
                    empty.push(format!("{preset}: row {} role {}", row.key, row.role));
                }
                assert!(
                    geometry.width > 0.0,
                    "{preset}: row {} has no width",
                    row.key
                );
            }
        }
        assert!(empty.is_empty(), "rows that reserve no height: {empty:?}");
    }

    /// A group's seat flips its members in and out. This is its own test, not a preset arm: a
    /// preset arm runs inside `metadata`, which runs before a capture, so a click there would
    /// photograph a state no user produced.
    #[test]
    fn a_seat_click_folds_and_unfolds_its_members() {
        let (size, _scale) = preset_size("tool-activity").unwrap();
        let (mut app, _receiver) = build(options(), "tool-activity", size, None);
        let (rows, key, open) = {
            let rows: Vec<&crate::reducer::DisplayRow> =
                app.transcript.display_rows(false).collect();
            let seats = app.process_seats(&rows);
            let seat = seats.values().next().expect("a group");
            let (key, open) = (seat.key, seat.open);
            // The rows borrow the transcript, so they are dropped with the seat map.
            (rows.len(), key, open)
        };
        let _ = rows;
        assert!(open, "a live group stands open");
        // The title names the activity and, in a mode that shows detail, the running command.
        let title = {
            let rows: Vec<_> = app.transcript.display_rows(false).collect();
            app.process_seats(&rows)
                .values()
                .next()
                .unwrap()
                .title
                .clone()
        };
        // The reference reads `title` before `command`, and this fixture's Bash call records both:
        // the intent as its title, the command as its argument.
        assert_eq!(
            title, "Running commands \u{b7} PUBLIC recorded intent only; never executed",
            "a live group states its activity and its detail"
        );
        drop(app.update(Message::ToggleProcessGroup {
            generation: app.generation,
            key,
        }));
        let folded = {
            let rows: Vec<_> = app.transcript.display_rows(false).collect();
            !app.process_seats(&rows).values().next().unwrap().open
        };
        assert!(folded, "the click folds it");
        drop(app.update(Message::ToggleProcessGroup {
            generation: app.generation,
            key,
        }));
        let unfolded = {
            let rows: Vec<_> = app.transcript.display_rows(false).collect();
            app.process_seats(&rows).values().next().unwrap().open
        };
        assert!(unfolded, "the second click unfolds it again");
    }

    #[test]
    fn presets_render_actual_app_without_queued_effects() {
        for &preset in PRESETS {
            let (size, scale) = preset_size(preset).unwrap();
            let mut options = options();
            options.scale = scale;
            let (mut app, mut receiver) = build(options, preset, size, None);
            drop(app.view());
            // Opening a session whose goal is active asks the Host for its live activation.
            // That is a read of process-local state, not work, so it is the one command a
            // preset may queue; everything else still has to stay unqueued.
            let mut activation_reads = 0;
            // Opening Settings is a read surface: it asks for the account, plugin and permission
            // state the page renders. That preset may queue reads, and only reads — nothing that
            // writes, executes or uploads.
            let mut settings_reads = 0usize;
            loop {
                match receiver.try_recv() {
                    Ok(Command::GoalActivationRead { .. }) => activation_reads += 1,
                    Ok(_) => panic!("PUBLIC preset queued a Command"),
                    Err(_) => break,
                }
            }
            if preset == "goal-bar" {
                assert_eq!(
                    activation_reads, 1,
                    "an active goal is read once when its session opens"
                );
            }
            let report = metadata(&app, preset);
            // A preset arm may open a surface that dispatches its own reads (the shortcuts editor
            // opens Settings); drain those here, because the capture step fails a frame that still
            // holds a command. Every preset's arm must stay at zero after that drain.
            while receiver.try_recv().is_ok() {
                settings_reads += 1;
            }
            let _ = settings_reads;
            assert_eq!(report["validPublicSnapshotApplied"], true);
            assert_eq!(report["registryReady"], true);
            assert_eq!(
                report["requestedAppFrameLogicalSize"],
                json!([size.width, size.height])
            );
            assert_eq!(
                app.sidebar_sheet,
                matches!(
                    preset,
                    "navigation" | "navigation-tiny" | "navigation-search-tiny"
                )
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
            if preset == "permission-chip" {
                // The chip is drawn only from the recorded preset, and the record
                // itself is not a chat row: the conversation keeps its two rows.
                assert_eq!(app.transcript.permission_preset(), Some("workspace-write"));
                assert_eq!(app.transcript.rows().len(), 3);
                assert_eq!(report["displayedPublicRecordCount"], 2);
                assert!(app.permission_chip().is_some());
                drop(app.update(Message::TogglePermissionMenu));
                assert!(app.permission_menu);
                // The menu's first open asks the Host for the catalog.
                assert!(matches!(
                    receiver.try_recv(),
                    Ok(Command::PermissionCatalogRead)
                ));
            }
            // The search preset carries its own smaller conversation: one grep call records the
            // only tool data its card needs, so it is asserted in its own arm below.
            if preset.starts_with("tool-activity")
                && preset != "tool-activity-search"
                && preset != "tool-activity-web"
                && preset != "tool-activity-ask"
            {
                assert_eq!(app.transcript.rows().len(), 8);
                assert_eq!(app.transcript.opening_cursor(), 7);
                assert_eq!(app.transcript.rows()[0].role, "You");
                assert_eq!(app.transcript.rows()[7].role, "Assistant");
                assert_eq!(report["publicToolSummaryCount"], 6);
                // The failed-edit preset records a second error marker; every
                // other tool-activity preset keeps the single Bash failure.
                assert_eq!(
                    report["publicRecordedErrorMarkerCount"],
                    match preset {
                        "tool-activity-edit-error" => 2,
                        // The terminal preset rewrites the bash result as the reference's
                        // *completed failing call*: no error flag, the exit status is result data.
                        "tool-activity-terminal" => 0,
                        _ => 1,
                    }
                );
                if preset == "tool-activity-edit-error" {
                    // The edit call keeps its recorded totals; the render is what
                    // drops them, so the model still carries them.
                    assert_eq!(
                        app.transcript.activity(3, app.path_roots()).unwrap().edits,
                        Some((1, 1))
                    );
                    assert!(app.transcript.activity(4, app.path_roots()).unwrap().failed);
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
                    app.transcript.activity(1, app.path_roots()).unwrap().title,
                    "Read · functions.read"
                );
                // The recorded argument is absolute; the row draws it relative
                // to the session's own workspace root, as the reference does.
                assert_eq!(
                    app.transcript
                        .activity(1, app.path_roots())
                        .unwrap()
                        .target
                        .as_deref(),
                    Some("src/PUBLIC-note.txt")
                );
                assert_eq!(
                    app.transcript.activity(4, app.path_roots()).unwrap().title,
                    "Edit · functions.edit · result"
                );
                // The bash result's error flag: true everywhere except the terminal preset, which
                // records the reference's completed failing call (status as result data) instead.
                assert_eq!(
                    app.transcript.activity(6, app.path_roots()).unwrap().failed,
                    preset != "tool-activity-terminal"
                );
                // Programmatic local view action only: not physical input or execution proof.
                drop(app.update(app.detail_request(3).unwrap()));
                // The long-diff preset records a different edit body, so only the presets that
                // carry the standard one assert its exact recorded text.
                if preset != "tool-activity-diff-long" {
                    assert!(
                        app.detail
                            .as_ref()
                            .unwrap()
                            .source
                            .contains("PUBLIC old text")
                    );
                }
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
