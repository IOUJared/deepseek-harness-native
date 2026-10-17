//! PUBLIC zero-Host screenshot fixture embedding the actual production App view.
//! No worker::start, Node, network, account lookup, provider/catalog, prompt or ACK.
//! Feature opt-in only. The parent serializes builds/runs and inspects actual paint.
#![allow(dead_code)]
#[path = "../src/codex.rs"]
mod codex;
#[path = "../src/config.rs"]
mod config;
#[path = "../src/crash_log.rs"]
mod crash_log;
#[path = "../src/design.rs"]
mod design;
#[path = "../src/export_file.rs"]
mod export_file;
#[path = "../src/exporter.rs"]
mod exporter;
#[path = "../src/fork.rs"]
mod fork;
#[path = "../src/goal_ui.rs"]
mod goal_ui;
#[path = "../src/interactions.rs"]
mod interactions;
#[path = "../src/keybindings.rs"]
mod keybindings;
#[path = "../src/known.rs"]
mod known;
#[path = "../src/management.rs"]
mod management;
#[path = "../src/mcp.rs"]
mod mcp;
#[path = "../src/plugins.rs"]
mod plugins;
#[path = "../src/queue_dock.rs"]
mod queue_dock;
#[path = "../src/queue_update.rs"]
mod queue_update;
#[path = "../src/reducer.rs"]
mod reducer;
#[path = "../src/rename.rs"]
mod rename;
#[path = "../src/secret_input.rs"]
mod secret_input;
#[path = "../src/settings.rs"]
mod settings;
#[path = "../src/single_instance.rs"]
mod single_instance;
#[path = "../src/smoke.rs"]
mod smoke;
#[path = "../src/terminal.rs"]
mod terminal;
#[path = "../src/ui.rs"]
mod ui;
#[path = "../src/work_panel.rs"]
mod work_panel;
#[path = "../src/worker.rs"]
mod worker;

use iced::advanced::widget::{Id, Operation, operation::Outcome};
use iced::widget::{column, container};
use iced::{Element, Rectangle, Size, Subscription, Task, window};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    time::Duration,
};

const BANNER_HEIGHT: f32 = 32.0;
const KNOWN_IDS: &[&str] = &[
    "fixture-frame",
    "native-settings-entry",
    "native-api-key-draft",
    "native-stop-control",
    "native-send-control",
    "native-model-controls",
    "native-reasoning-control",
    "native-composer-rail",
    "native-hero-workspace",
    "native-hero-preset",
    "native-hero-preset-menu",
    "native-load-older",
    "native-plan-control",
    "native-editor-menu",
    "native-reference-summary",
    "native-permission-control",
    "native-permission-menu",
    "native-permission-confirm",
    "native-to-bottom",
    "native-width-handle-left",
    "native-width-handle-right",
    "native-header",
    "native-header-crumbs",
    "native-agent-preset",
    "native-jobs-action",
    "native-jobs-panel",
    "native-file-open",
    "native-file-draft",
    "native-file-picker",
    "native-file-preview",
    "native-detail-formatted",
    "native-detail-source",
    "native-detail-back",
    "native-detail-copy-source",
    "native-sidebar-options",
];

#[derive(Clone)]
struct Options {
    mode: String,
    size: Size,
    scale: f32,
    screenshot: PathBuf,
    report: PathBuf,
    drop_file: Option<PathBuf>,
    drop_rejected: bool,
    content_size: Option<f32>,
    /// Dispatch the composer focus task, so a capture renders the focused editor.
    focus_composer: bool,
    /// Render every tool row as hovered, so a capture shows the chevron swap that
    /// otherwise needs a pointer.
    hover_tool_row: bool,
    /// Render both width handles' glow, so a capture shows the affordance that
    /// otherwise needs a pointer over a 10px strip.
    width_handle_glow: bool,
    /// Hold a plan toggle in flight, so a capture shows the chip's disabled state.
    plan_pending: bool,
    /// Render the transcript as scrolled away from its tail, so a capture shows
    /// the jump-to-latest control.
    scrolled_away: bool,
    /// Hold a history page in flight, so a capture shows the control's loading form.
    page_pending: bool,
    /// Open the editable-field context menu, so a capture shows it without a right
    /// press the fixture cannot deliver.
    editor_menu: bool,
    /// Open the permission chip's menu, so a capture shows it without a press.
    permission_menu: bool,
    /// Hold the full-access risk confirmation open for a capture.
    permission_confirm: bool,
    /// Tick the risk acknowledgement, so the confirmation's enabled form is captured too.
    permission_acknowledged: bool,
    /// Seed a Host-shaped catalog, so the menu's Host-driven list is captured.
    permission_catalog: bool,
    /// Seed a Host-shaped agent roster, so the label's display name is captured.
    agent_roster: bool,
    /// Open the hero preset seat's list, so its rows are captured.
    agent_seat: bool,
    /// Hold the seat's refusal state, so its announcement is captured.
    agent_seat_error: bool,
    /// Seed background jobs, so the header action is captured.
    jobs: bool,
    /// Open the settings modal, so its General page is captured.
    settings: bool,
    /// Customise one shortcut, so the disclosure's count line is captured.
    customised: bool,
    /// Seed the Host's advertised commands, so the disclosure's second group is captured.
    commands: bool,
    /// Arm the restore-all confirmation, so its copy is captured.
    reset_all: bool,
    /// Arm one shortcut row's recorder, so the capture-in-progress state is captured.
    capture: bool,
    /// Arm one row and capture a key another command holds, so the refusal is captured.
    capture_conflict: bool,
    /// Prefill the shortcuts search, so its filtered and empty states are captured.
    shortcuts_search: Option<String>,
    /// Seed the General page's session-log namespace, so its preference row is captured.
    session_log: bool,
    /// Complete one session-log write, so the reference's saved state is captured.
    session_log_saved: bool,
    /// Open the header's subagent catalog dropdown, so its rows are captured.
    subagent_menu: bool,
    /// Pre-expand the catalog's first branch, so a grandchild row is captured.
    subagent_expanded: bool,
    /// Register one pending approval, so the sidebar status is captured.
    pending_approval: bool,
    /// Set the busy-state Enter preference (`queue` or `steer`).
    busy_enter: Option<String>,
    /// Expand the queue dock's multi-item list for capture.
    queue_expanded: bool,
    /// Expand the first turn-trigger notice for capture.
    trigger_expanded: bool,
    /// Expand the first assistant row's usage detail.
    turn_usage_expanded: bool,
    /// Seed a background stop's unread marker on the notes row.
    completion_unread: bool,
    /// Begin editing the first queued item.
    queue_edit: bool,
    /// Arm the second queued item's removal confirmation.
    queue_remove: bool,
    /// Seed a content-search page under the session rows.
    search_results: bool,
    /// Begin editing the goal objective.
    goal_edit: bool,
    /// Arm the goal clear confirmation.
    goal_clear: bool,
    /// Arm the first scheduled task's delete confirmation.
    schedule_delete: bool,
    /// Open the first scheduled task's delivery history.
    schedule_history: bool,
    /// Begin editing the first scheduled task.
    schedule_edit: bool,
    /// Open the one-shot calendar in the schedule editor.
    schedule_at: bool,
    /// Expand the seeded changed-file list.
    changes_expanded: bool,
    /// Show the seeded `@` file-reference menu.
    reference_menu: bool,
    /// Expand the composer's todo dock.
    todo_dock: bool,
    /// Mark the goal's activation disarmed.
    goal_inactive: bool,
    /// Open the shortcuts disclosure, so its rows are captured.
    shortcuts: bool,
}
fn public_drop_file(value: String) -> Result<PathBuf, &'static str> {
    let path = PathBuf::from(value);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("public-file-required")?;
    if !path.is_absolute()
        || path.components().any(|p| matches!(p, Component::ParentDir))
        || !name.starts_with("PUBLIC-")
        || path.canonicalize().ok().as_ref() != Some(&path)
    {
        return Err("canonical-public-file-required");
    }
    let info = fs::symlink_metadata(&path).map_err(|_| "public-file-unavailable")?;
    let uid = fs::metadata("/proc/self")
        .map_err(|_| "ownership-unavailable")?
        .uid();
    let parent = fs::metadata(path.parent().ok_or("public-parent-required")?)
        .map_err(|_| "public-parent-unavailable")?;
    if !info.is_file()
        || info.uid() != uid
        || info.len() > 4 * 1024 * 1024
        || !parent.is_dir()
        || parent.uid() != uid
        || parent.permissions().mode() & 0o7777 != 0o700
    {
        return Err("owned-private-public-file-required");
    }
    Ok(path)
}
fn new_output(value: String) -> Result<PathBuf, &'static str> {
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || path.components().any(|p| matches!(p, Component::ParentDir))
        || path
            .to_str()
            .is_none_or(|s| s.bytes().any(|b| b < 32 || b == 127))
    {
        return Err("output-must-be-absolute-without-traversal");
    }
    let name = path.file_name().ok_or("output-filename-required")?;
    let parent = path
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .filter(|p| p.is_dir())
        .ok_or("output-parent-must-already-exist")?;
    let metadata = fs::metadata(&parent).map_err(|_| "output-parent-unavailable")?;
    let own_uid = fs::metadata("/proc/self")
        .map_err(|_| "process-ownership-unavailable")?
        .uid();
    if metadata.uid() != own_uid || metadata.permissions().mode() & 0o7777 != 0o700 {
        return Err("output-parent-must-be-owned-private-0700-directory");
    }
    let path = parent.join(name);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
        _ => Err("output-must-be-new"),
    }
}
fn options() -> Result<Options, &'static str> {
    let mut args = std::env::args().skip(1);
    let mut values = BTreeMap::new();
    while let Some(key) = args.next() {
        if ![
            "--mode",
            "--width",
            "--height",
            "--scale",
            "--screenshot",
            "--report",
            "--drop-file",
            "--drop-expect",
            "--content-size",
            "--focus-composer",
            "--hover-tool-row",
            "--width-handle-glow",
            "--plan-pending",
            "--editor-menu",
            "--permission-menu",
            "--permission-confirm",
            "--permission-acknowledged",
            "--permission-catalog",
            "--agent-roster",
            "--agent-seat",
            "--agent-seat-error",
            "--jobs",
            "--shortcuts",
            "--customised",
            "--commands",
            "--reset-all",
            "--capture",
            "--capture-conflict",
            "--shortcuts-search",
            "--session-log",
            "--session-log-saved",
            "--subagent-menu",
            "--subagent-expanded",
            "--pending-approval",
            "--busy-enter",
            "--queue-expanded",
            "--trigger-expanded",
            "--turn-usage-expanded",
            "--completion-unread",
            "--queue-edit",
            "--queue-remove",
            "--search-results",
            "--goal-edit",
            "--goal-clear",
            "--schedule-delete",
            "--schedule-history",
            "--schedule-edit",
            "--schedule-at",
            "--changes-expanded",
            "--reference-menu",
            "--todo-dock",
            "--goal-inactive",
            "--agent-seat-error",
            "--settings",
            "--scrolled-away",
            "--page-pending",
        ]
        .contains(&key.as_str())
        {
            return Err("unknown-argument");
        }
        let value = args.next().ok_or("missing-argument-value")?;
        if values.insert(key, value).is_some() {
            return Err("duplicate-argument");
        }
    }
    let mode = values
        .remove("--mode")
        .unwrap_or_else(|| "wide-welcome".into());
    let (mut size, mut scale) =
        ui::layout_fixture::preset_size(&mode).ok_or("unknown-public-layout-mode")?;
    for (key, target) in [
        ("--width", &mut size.width),
        ("--height", &mut size.height),
        ("--scale", &mut scale),
    ] {
        if let Some(value) = values.remove(key) {
            *target = value.parse().map_err(|_| "invalid-number")?;
        }
    }
    if !size.width.is_finite()
        || !size.height.is_finite()
        || !scale.is_finite()
        || !(280.0..=4096.0).contains(&size.width)
        || !(240.0..=4096.0).contains(&size.height)
        || !(0.75..=2.0).contains(&scale)
    {
        return Err("invalid-frame-or-application-scale");
    }
    let screenshot = new_output(values.remove("--screenshot").ok_or("screenshot-required")?)?;
    let report = new_output(values.remove("--report").ok_or("report-required")?)?;
    if screenshot == report {
        return Err("distinct-new-output-files-required");
    }
    let drop_file = values
        .remove("--drop-file")
        .map(public_drop_file)
        .transpose()?;
    let drop_rejected = match values.remove("--drop-expect").as_deref() {
        None | Some("accepted") => false,
        Some("rejected") => true,
        _ => return Err("unknown-drop-expectation"),
    };
    if (drop_rejected && drop_file.is_none()) || (drop_file.is_some() && mode != "narrow-welcome") {
        return Err("drop-check-requires-public-file-and-narrow-welcome");
    }
    Ok(Options {
        mode,
        size,
        scale,
        screenshot,
        report,
        drop_file,
        drop_rejected,
        content_size: match values.remove("--content-size") {
            Some(raw) => {
                let size: f32 = raw.parse().map_err(|_| "invalid-number")?;
                if !size.is_finite() || !(11.0..=20.0).contains(&size) {
                    return Err("invalid-content-size");
                }
                Some(size)
            }
            None => None,
        },
        focus_composer: match values.remove("--focus-composer") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        hover_tool_row: match values.remove("--hover-tool-row") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        width_handle_glow: match values.remove("--width-handle-glow") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        reset_all: match values.remove("--reset-all") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        capture: match values.remove("--capture") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        capture_conflict: match values.remove("--capture-conflict") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        shortcuts_search: values.remove("--shortcuts-search"),
        session_log: match values.remove("--session-log") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        session_log_saved: match values.remove("--session-log-saved") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        subagent_menu: match values.remove("--subagent-menu") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        subagent_expanded: match values.remove("--subagent-expanded") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        pending_approval: match values.remove("--pending-approval") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        busy_enter: values.remove("--busy-enter"),
        queue_expanded: match values.remove("--queue-expanded") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        trigger_expanded: match values.remove("--trigger-expanded") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        turn_usage_expanded: match values.remove("--turn-usage-expanded") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        completion_unread: match values.remove("--completion-unread") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        queue_edit: match values.remove("--queue-edit") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        queue_remove: match values.remove("--queue-remove") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        search_results: match values.remove("--search-results") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        goal_edit: match values.remove("--goal-edit") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        goal_clear: match values.remove("--goal-clear") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        schedule_delete: match values.remove("--schedule-delete") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        schedule_history: match values.remove("--schedule-history") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        schedule_edit: match values.remove("--schedule-edit") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        schedule_at: match values.remove("--schedule-at") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        changes_expanded: match values.remove("--changes-expanded") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        reference_menu: match values.remove("--reference-menu") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        todo_dock: match values.remove("--todo-dock") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        goal_inactive: match values.remove("--goal-inactive") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        commands: match values.remove("--commands") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        customised: match values.remove("--customised") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        shortcuts: match values.remove("--shortcuts") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        settings: match values.remove("--settings") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        jobs: match values.remove("--jobs") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        agent_seat_error: match values.remove("--agent-seat-error") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        agent_seat: match values.remove("--agent-seat") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        agent_roster: match values.remove("--agent-roster") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        permission_catalog: match values.remove("--permission-catalog") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        permission_acknowledged: match values.remove("--permission-acknowledged") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        permission_confirm: match values.remove("--permission-confirm") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        permission_menu: match values.remove("--permission-menu") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        editor_menu: match values.remove("--editor-menu") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        plan_pending: match values.remove("--plan-pending") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        scrolled_away: match values.remove("--scrolled-away") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
        page_pending: match values.remove("--page-pending") {
            Some(raw) => raw.parse().map_err(|_| "invalid-boolean")?,
            None => false,
        },
    })
}
fn public_options(scale: f32) -> config::Options {
    // These explicit fake private paths are inert metadata. Never invoke startup/parse or
    // consult ambient HOME, installed runtimes, profiles, credentials or actual workspaces.
    config::Options {
        backend: dsh_native_core::RustBackendOptions {
            runtime: "/PUBLIC-layout-fixture/NEVER-START/runtime".into(),
            expected_version: "0.2.1-alpha.1".into(),
            native_home: "/PUBLIC-layout-fixture/NEVER-START/native-home".into(),
            user_home: "/PUBLIC-layout-fixture/NEVER-START/user-home".into(),
            working_directory: "/PUBLIC-layout-fixture/workspace".into(),
            absolute_node_path: None,
            startup_timeout: Duration::from_secs(1),
            stop_policy: Default::default(),
        },
        scale,
        smoke: None,
        // A fixture renders a public frame; it never announces itself.
        accessibility: false,
    }
}

#[derive(Clone, Default)]
struct LayoutObservation {
    rects: BTreeMap<&'static str, Vec<Rectangle>>,
}
impl Operation<LayoutObservation> for LayoutObservation {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<LayoutObservation>)) {
        operate(self);
    }
    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        if let Some(name) = KNOWN_IDS
            .iter()
            .copied()
            .find(|name| id.is_some_and(|id| id == &Id::from(*name)))
        {
            self.rects.entry(name).or_default().push(bounds);
        }
    }
    // No text, text-input contents, custom state or arbitrary widget IDs are observed.
    fn finish(&self) -> Outcome<LayoutObservation> {
        Outcome::Some(self.clone())
    }
}
fn rect_value(r: &Rectangle) -> Value {
    json!({"x":r.x,"y":r.y,"width":r.width,"height":r.height})
}
fn valid_rect(r: &Rectangle) -> bool {
    [r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite()) && r.width > 0.0 && r.height > 0.0
}
fn inside(inner: &Rectangle, outer: &Rectangle) -> bool {
    const EPS: f32 = 0.5;
    valid_rect(inner)
        && valid_rect(outer)
        && inner.x >= outer.x - EPS
        && inner.y >= outer.y - EPS
        && inner.x + inner.width <= outer.x + outer.width + EPS
        && inner.y + inner.height <= outer.y + outer.height + EPS
}
impl LayoutObservation {
    fn count(&self, name: &str) -> usize {
        self.rects.get(name).map_or(0, Vec::len)
    }
    fn report(&self, requested: Size, viewport: Option<Size>, metadata: &Value) -> Value {
        let frame = self
            .rects
            .get("fixture-frame")
            .filter(|rs| rs.len() == 1)
            .and_then(|rs| rs.first());
        let composer = metadata["composerExpected"].as_bool().unwrap_or(false)
            && metadata["composerReplacedExpected"] != true;
        let frame_matches = frame.is_some_and(|r| {
            valid_rect(r)
                && (r.width - requested.width).abs() <= 0.5
                && (r.height - requested.height).abs() <= 0.5
        });
        let frame_in_viewport = frame
            .zip(viewport)
            .is_some_and(|(r, size)| inside(r, &Rectangle::with_size(size)));
        // A popup is a floating layer rather than layout, so it may legitimately overlap the
        // fixture's own banner band, which occupies the top of every capture and is not product
        // chrome. Every layout container still has to lie inside the frame.
        const FLOATING: [&str; 3] = [
            "native-jobs-panel",
            "native-permission-menu",
            "native-hero-preset-menu",
        ];
        let controls_in_frame = frame.is_some_and(|frame| {
            self.rects
                .iter()
                .filter(|(name, _)| {
                    **name != "fixture-frame" && !FLOATING.iter().any(|id| **name == *id)
                })
                .all(|(_, rects)| rects.iter().all(|r| inside(r, frame)))
        });
        let counts = self.count("native-sidebar-options")
            == usize::from(metadata["sidebarOptionsExpected"] == true)
            && self.count("native-settings-entry")
                == usize::from(metadata["settingsEntryExpected"] == true)
            && self.count("native-stop-control") == usize::from(metadata["stopExpected"] == true)
            && self.count("native-send-control") == usize::from(composer)
            && self.count("native-model-controls")
                == usize::from(metadata["modelControlsExpected"] == true)
            && self.count("native-reasoning-control")
                == usize::from(metadata["reasoningControlExpected"] == true)
            && self.count("native-header") == usize::from(metadata["headerExpected"] == true)
            && self.count("native-detail-formatted")
                == usize::from(
                    metadata["detailVisible"] == true
                        && metadata["detailFormattedAvailable"] == true,
                )
            && self.count("native-detail-source")
                == usize::from(
                    metadata["detailVisible"] == true
                        && metadata["detailFormattedAvailable"] == true,
                )
            && self.count("native-detail-back") == usize::from(metadata["detailVisible"] == true)
            && self.count("native-detail-copy-source")
                == usize::from(metadata["detailVisible"] == true)
            && self.count("native-file-picker")
                == usize::from(metadata["filePickerExpected"] == true)
            && self.count("native-file-preview")
                == usize::from(
                    metadata["localPreviewSnapshotSimulated"] == true
                        || metadata["localPreviewReadFromDroppedFile"] == true,
                );
        let states_match = metadata["composerStateMatchesPreset"] == true
            && metadata["reasoningStateMatchesPreset"] == true
            && metadata["validPublicSnapshotApplied"] == true
            && metadata["registryReady"] == true
            && metadata["settingsStateMatchesPreset"] == true
            && metadata["informationStateMatchesPreset"] == true;
        let frame_origin_matches =
            frame.is_some_and(|r| r.x.abs() <= 0.5 && (r.y - BANNER_HEIGHT).abs() <= 0.5);
        let rects: BTreeMap<_, _> = KNOWN_IDS
            .iter()
            .map(|&name| {
                (
                    name,
                    self.rects
                        .get(name)
                        .map(|rs| rs.iter().map(rect_value).collect::<Vec<_>>())
                        .unwrap_or_default(),
                )
            })
            .collect();
        json!({
            "source":"actual-Iced-runtime-widget-operation-after-layout; known container IDs only",
            "coordinateSpace":"Iced application-logical widget layout bounds, not pixel/paint bounds",
            "knownContainerRects":rects,
            "measuredAppFrameBounds":frame.map(rect_value),
            "settingsCount":self.count("native-settings-entry"),
            "settingsRects":self.rects.get("native-settings-entry").map(|rs| rs.iter().map(rect_value).collect::<Vec<_>>()).unwrap_or_default(),
            "stopCount":self.count("native-stop-control"), "sendCount":self.count("native-send-control"),
            "frameMatchesRequestedSize":frame_matches, "frameWithinMeasuredWindowViewport":frame_in_viewport,
            "knownRenderedControlsWithinFrame":controls_in_frame, "expectedControlCountsMatch":counts,
            "presetStatesMatch":states_match, "frameOriginMatchesOutsideBanner":frame_origin_matches,
            "passed":frame_matches && frame_in_viewport && controls_in_frame && counts && states_match && frame_origin_matches,
            "allTextCollected":false, "paintVisibilityQualified":false,
            "scope":"known fixture-tagged container layout only; parent inspects actual screenshot paint"
        })
    }
}

#[derive(Clone)]
enum Message {
    App(ui::Message),
    WorkingFrame,
    Opened(window::Id),
    Native {
        backend: &'static str,
        scale: f32,
        size: Size,
    },
    Observe(window::Id),
    Observed(window::Id, Size, LayoutObservation),
    Screenshot(window::Screenshot),
    Saved(bool, [u32; 2]),
    Deadline,
    Close,
    Closed,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PublicLayoutFixtureMessage(redacted)")
    }
}
struct Fixture {
    options: Options,
    app: ui::App,
    commands: tokio::sync::mpsc::Receiver<worker::Command>,
    report: Value,
    window: Option<window::Id>,
    viewport: Option<Size>,
    capture_started: bool,
    finished: bool,
    captured_count: usize,
    ignored_app_messages: usize,
}
impl Fixture {
    fn boot(options: Options) -> (Self, Task<Message>) {
        let deadline = if options.drop_file.is_some() { 45 } else { 8 };
        let (mut app, commands) = ui::layout_fixture::build(
            public_options(options.scale),
            &options.mode,
            options.size,
            options.content_size,
        );
        // States that need a pointer or a focused editor are forced here, so a
        // capture can show them.
        if options.hover_tool_row {
            app.set_hover_all_activities(true);
        }
        if options.width_handle_glow {
            app.set_width_handle_glow(true);
        }
        if options.plan_pending {
            app.set_plan_pending(true);
        }
        if options.editor_menu {
            app.set_editor_menu(true);
        }
        if options.permission_menu {
            app.set_permission_menu(true);
        }
        if options.permission_confirm {
            app.set_permission_confirm(true);
        }
        if options.permission_acknowledged {
            app.set_permission_acknowledged(true);
        }
        if options.permission_catalog {
            app.set_permission_catalog();
        }
        if options.agent_roster {
            app.set_agent_roster();
        }
        if options.agent_seat {
            app.set_agent_seat_open();
        }
        if options.agent_seat_error {
            app.set_agent_seat_error("Agent preset: This session has already started");
        }
        if options.jobs {
            app.set_jobs();
        }
        if options.settings {
            app.set_settings_open();
        }
        if options.shortcuts {
            // The disclosure is opened through the app's own action, so the fixture exercises the
            // same path a press does rather than poking private state.
            app.set_shortcuts_open(true);
        }
        if options.customised {
            // One binding cleared through the store's own edit path, so the count line reads a
            // real modified count rather than a fixture-injected number.
            app.customise_one_shortcut();
        }
        if options.commands {
            app.seed_commands();
        }
        if options.reset_all {
            app.arm_reset_all();
        }
        if options.capture {
            app.arm_shortcut_capture();
        }
        if options.capture_conflict {
            // The production chain, not a fixture-injected string: arm the row, then press the
            // key Navigation already holds and let the reducer refuse it.
            app.arm_shortcut_capture();
            let _ = app.update(ui::Message::KeyPressed(
                iced::keyboard::Key::Character("b".into()),
                iced::keyboard::Modifiers::CTRL,
            ));
        }
        if let Some(query) = &options.shortcuts_search {
            app.set_shortcuts_query(query);
        }
        if options.session_log {
            app.seed_session_log();
        }
        if options.session_log_saved {
            app.seed_session_log_saved();
        }
        if options.subagent_menu {
            app.set_subagent_menu(true);
        }
        if options.subagent_expanded {
            app.expand_subagent_branch("PUBLIC-child-reviewer");
        }
        if options.pending_approval {
            app.seed_pending_approval();
        }
        if let Some(raw) = &options.busy_enter
            && let Some(value) = ui::settings_busy_enter(raw)
        {
            app.set_busy_enter(value);
        }
        if options.queue_expanded {
            app.expand_queue();
        }
        if options.trigger_expanded {
            app.expand_trigger(0);
        }
        if options.turn_usage_expanded {
            app.expand_turn_usage(1);
        }
        if options.completion_unread {
            app.mark_completion_unread("PUBLIC-layout-notes");
        }
        if options.queue_edit {
            app.begin_queue_edit("PUBLIC-queued-one");
        }
        if options.queue_remove {
            app.arm_queue_remove("PUBLIC-queued-two");
        }
        if options.search_results {
            app.seed_search_results();
        }
        if options.goal_edit {
            app.begin_goal_edit();
        }
        if options.goal_clear {
            app.arm_goal_clear();
        }
        if options.schedule_delete {
            app.arm_schedule_delete();
        }
        if options.schedule_history {
            app.open_schedule_history();
        }
        if options.schedule_edit {
            app.begin_schedule_edit();
        }
        if options.schedule_at {
            app.open_at_picker();
        }
        if options.changes_expanded {
            app.expand_seeded_changes();
        }
        if options.reference_menu {
            app.seed_reference_menu();
        }
        if options.todo_dock {
            app.expand_todo_dock();
        }
        if options.goal_inactive {
            app.disarmed_goal_activation();
        }
        if options.scrolled_away {
            app.set_scrolled_away(true);
        }
        if options.page_pending {
            app.set_page_pending(true);
        }
        let mut report = ui::layout_fixture::metadata(&app, &options.mode);
        report["pid"] = json!(std::process::id());
        report["nativeDropCheckOptIn"] = json!(options.drop_file.is_some());
        report["nativeDropEvents"] = json!(0);
        report["nativeDropCancelEvents"] = json!(0);
        report["unexpectedDropPaths"] = json!(0);
        report["dropExpectedRejected"] = json!(options.drop_rejected);
        report["physicalMouseQualified"] = json!(false);
        report["rendererRequested"] = json!("wgpu");
        report["font"] = json!("DejaVu Sans");
        report["theme"] = json!("RosePine");
        report["capturedCommandKinds"] = json!([]);
        report["workingTickCount"] = json!(0);
        report["workingFramePhases"] = json!([]);
        report["publicFixtureBannerOutsideAppFrame"] = json!(true);
        report["fixtureBannerLogicalHeight"] = json!(BANNER_HEIGHT);
        report["privateOwnedOutputParentsValidated"] = json!(true);
        report["capture"] = json!({"source":"own-Iced-window-renderer-buffer", "publicSource":true,
            "screenshotPath":"[REDACTED explicit new output path]", "reportPath":"[REDACTED explicit new output path]",
            "desktopCapture":false, "oneShot":true});
        // Read before `options` moves into the fixture.
        let focus_composer = options.focus_composer;
        let mut fixture = Self {
            options,
            app,
            commands,
            report,
            window: None,
            viewport: None,
            capture_started: false,
            finished: false,
            captured_count: 0,
            ignored_app_messages: 0,
        };
        fixture.inspect_commands();
        (
            fixture,
            Task::batch([
                if focus_composer {
                    ui::App::focus_composer_task().map(Message::App)
                } else {
                    Task::none()
                },
                Task::perform(
                    async move { tokio::time::sleep(Duration::from_secs(deadline)).await },
                    |_| Message::Deadline,
                ),
            ]),
        )
    }
    fn inspect_commands(&mut self) {
        // Exhaustively inspect only command variants; never serialize content or execute effects.
        while let Ok(command) = self.commands.try_recv() {
            let kind = match &command {
                worker::Command::StageFile(_) => "StageFile",
                worker::Command::DiscardFile(_) => "DiscardFile",
                worker::Command::PromptFile { .. } => "PromptFile",
                worker::Command::Account(_) => "Account",
                worker::Command::Codex(_) => "Codex",
                worker::Command::Cloudflare(_) => "Cloudflare",
                worker::Command::Export(_) => "Export",
                worker::Command::PluginsRead(_) => "PluginsRead",
                worker::Command::PermissionCatalogRead => "PermissionCatalogRead",
                worker::Command::AgentPresetsRead => "AgentPresetsRead",
                worker::Command::AgentPresetSelect { .. } => "AgentPresetSelect",
                worker::Command::JobKill { .. } => "JobKill",
                worker::Command::JobOutput { .. } => "JobOutput",
                worker::Command::PermissionSettingsRead => "PermissionSettingsRead",
                worker::Command::PermissionDefaultSet { .. } => "PermissionDefaultSet",
                worker::Command::PluginSave { .. } => "PluginSave",
                worker::Command::Manage(_) => "Manage",
                worker::Command::Rename(_) => "Rename",
                worker::Command::Fork(_) => "Fork",
                worker::Command::UpdateQueue(_) => "UpdateQueue",
                worker::Command::Search { .. } => "Search",
                worker::Command::Goal { .. } => "Goal",
                worker::Command::ScheduleCatalog => "ScheduleCatalog",
                worker::Command::ChangesSummary { .. } => "ChangesSummary",
                worker::Command::ChangesDiff { .. } => "ChangesDiff",
                worker::Command::FileReferences { .. } => "FileReferences",
                worker::Command::SessionReferences { .. } => "SessionReferences",
                worker::Command::OpenWorkspacePath { .. } => "OpenWorkspacePath",
                worker::Command::FeedbackPut { .. } => "FeedbackPut",
                worker::Command::FeedbackDelete { .. } => "FeedbackDelete",
                worker::Command::ScheduleDelete { .. } => "ScheduleDelete",
                worker::Command::ScheduleHistory { .. } => "ScheduleHistory",
                worker::Command::ScheduleUpdate { .. } => "ScheduleUpdate",
                worker::Command::CommandsList(_) => "CommandsList",
                worker::Command::CommandsExecute { .. } => "CommandsExecute",
                worker::Command::TerminalRead(_) => "TerminalRead",
                worker::Command::TerminalFollow(_) => "TerminalFollow",
                worker::Command::TerminalCreate(_) => "TerminalCreate",
                worker::Command::TerminalClose(_) => "TerminalClose",
                worker::Command::TerminalWrite(_) => "TerminalWrite",
                worker::Command::McpRead(_) => "McpRead",
                worker::Command::McpWrite(_) => "McpWrite",
                worker::Command::KeyMetadata(_) => "KeyMetadata",
                worker::Command::SaveKey { .. } => "SaveKey",
                worker::Command::Decision(_) => "Decision",
                worker::Command::Select { .. } => "Select",
                worker::Command::Create { .. } => "Create",
                worker::Command::OpenWorkspace(_) => "OpenWorkspace",
                worker::Command::Catalog(_) => "Catalog",
                worker::Command::SelectModel { .. } => "SelectModel",
                worker::Command::Prompt { .. } => "Prompt",
                worker::Command::CheckSubmission(_) => "CheckSubmission",
                worker::Command::RejoinObservations { .. } => "RejoinObservations",
                worker::Command::Cancel { .. } => "Cancel",
                worker::Command::Page { .. } => "Page",
                worker::Command::Inspect => "Inspect",
                worker::Command::Shutdown => "Shutdown",
            };
            if self.report["capturedCommandKinds"].is_null() {
                self.report["capturedCommandKinds"] = json!([]);
            }
            self.report["capturedCommandKinds"]
                .as_array_mut()
                .unwrap()
                .push(json!(kind));
            self.captured_count += 1;
            drop(command);
        }
        self.report["capturedCommandCount"] = json!(self.captured_count);
        self.report["commandsInspectedAndDroppedNeverExecuted"] = json!(true);
        self.report["effectsDispatched"] = json!(0);
        self.report["workerStarted"] = json!(false);
    }
    fn finish(&mut self, passed: bool) -> Task<Message> {
        self.inspect_commands();
        self.finished = true;
        self.report["status"] = json!(if passed && self.captured_count == 0 {
            "passed"
        } else {
            "failed"
        });
        self.report["fixturePassed"] = json!(passed && self.captured_count == 0);
        self.report["ignoredAppWidgetMessages"] = json!(self.ignored_app_messages);
        self.report["explicitWindowCloseRequested"] = json!(self.window.is_some());
        let saved = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&self.options.report)
            .and_then(|file| {
                serde_json::to_writer_pretty(&file, &self.report).map_err(std::io::Error::other)?;
                file.sync_all()
            })
            .is_ok();
        if !saved {
            eprintln!("PUBLIC layout report could not be created");
        }
        match self.window {
            Some(id) => window::close(id),
            None => iced::exit(),
        }
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WorkingFrame
                if self.options.mode == "working-conversation"
                    && !self.capture_started
                    && !self.finished =>
            {
                let phase = ui::layout_fixture::advance_working(&mut self.app);
                let count = self.report["workingTickCount"].as_u64().unwrap_or(0);
                self.report["workingTickCount"] = json!(count + 1);
                self.report["workingPhase"] = json!(phase);
                if count < 128 {
                    self.report["workingFramePhases"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!(phase));
                }
                self.inspect_commands();
            }
            Message::App(message) => {
                let allowed = if let Some(expected) = &self.options.drop_file {
                    match &message {
                        ui::Message::FileDropped(id, path) if Some(*id) == self.window => {
                            let key = if path == expected {
                                "nativeDropEvents"
                            } else {
                                "unexpectedDropPaths"
                            };
                            self.report[key] = json!(self.report[key].as_u64().unwrap_or(0) + 1);
                            path == expected
                        }
                        ui::Message::FileDropCancelled(id) if Some(*id) == self.window => {
                            self.report["nativeDropCancelEvents"] = json!(
                                self.report["nativeDropCancelEvents"].as_u64().unwrap_or(0) + 1
                            );
                            true
                        }
                        ui::Message::FilePreviewRead(..) => true,
                        _ => false,
                    }
                } else {
                    false
                };
                if allowed && !self.finished {
                    // These messages originate ONLY from the real Iced native window subscription
                    // or the production local preview Task. Never construct a synthetic FileDropped.
                    let task = self.app.update(message).map(Message::App);
                    self.inspect_commands();
                    let state = ui::layout_fixture::attachment_drop_state(
                        &self.app,
                        self.options.drop_file.as_ref().unwrap(),
                    );
                    self.report["localPreviewReadFromDroppedFile"] =
                        state["localImagePreviewReady"].clone();
                    self.report["filePreviewQualified"] = state["localImagePreviewReady"].clone();
                    self.report["filePathReviewVisible"] = state["editorOpen"].clone();
                    let complete = if self.options.drop_rejected {
                        self.report["nativeDropCancelEvents"].as_u64().unwrap_or(0) > 0
                            && state["cancelNoticeVisible"] == true
                            && state["editorOpen"] == false
                    } else {
                        self.report["nativeDropEvents"] == 1
                            && state["reviewPathMatches"] == true
                            && state["localImagePreviewReady"] == true
                    };
                    self.report["actualAttachmentState"] = state.clone();
                    self.report["actualProductionNativeDropQualified"] = json!(
                        complete
                            && state["localReadBusy"] == false
                            && state["uploadPending"] == false
                            && state["uploadReceiptPresent"] == false
                            && self.captured_count == 0
                    );
                    if self.report["actualProductionNativeDropQualified"] == true
                        && !self.capture_started
                        && let Some(id) = self.window
                    {
                        return Task::batch([
                            task,
                            Task::perform(
                                async {
                                    tokio::time::sleep(Duration::from_millis(150)).await;
                                },
                                move |_| Message::Observe(id),
                            ),
                        ]);
                    }
                    return task;
                }
                drop(message);
                self.ignored_app_messages += 1;
                self.inspect_commands();
            }
            Message::Opened(id) if self.window.is_none() && !self.finished => {
                self.window = Some(id);
                if self.options.drop_file.is_some() {
                    drop(self.app.update(ui::Message::NativeOpened(id)));
                }
                return window::run(id, |w| match w.display_handle().map(|h| h.as_raw()) {
                    Ok(raw_window_handle::RawDisplayHandle::Wayland(_)) => "wayland",
                    Ok(
                        raw_window_handle::RawDisplayHandle::Xlib(_)
                        | raw_window_handle::RawDisplayHandle::Xcb(_),
                    ) => "x11",
                    _ => "other-or-unavailable",
                })
                .then(move |backend| {
                    window::scale_factor(id).then(move |scale| {
                        window::size(id).map(move |size| Message::Native {
                            backend,
                            scale,
                            size,
                        })
                    })
                });
            }
            Message::Native {
                backend,
                scale,
                size,
            } if !self.finished => {
                self.report["windowBackend"] = json!(backend);
                self.report["nativeScaleReportedByIced"] = json!(scale);
                self.report["measuredWindowLogicalViewport"] = json!([size.width, size.height]);
                self.viewport = Some(size);
                // Do NOT feed outer window geometry into App: its requested inner frame is fixed.
                if self.options.drop_file.is_some() {
                    self.report["nativeInputEventSource"] =
                        json!("actual-Iced-window-events-and-production-local-preview-Task");
                    let ready = json!({"publicOnly":true,"pid":std::process::id(),"nativeWayland":backend=="wayland","capturedCommandCount":self.captured_count});
                    let parent = self.options.report.parent().unwrap();
                    let saved = fs::OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .mode(0o600)
                        .open(parent.join("drop-ready.json"))
                        .and_then(|file| {
                            serde_json::to_writer(&file, &ready).map_err(std::io::Error::other)
                        })
                        .is_ok();
                    if !saved {
                        return self.finish(false);
                    }
                    return Task::none();
                }
                if let Some(id) = self.window {
                    let scroll = if matches!(
                        self.options.mode.as_str(),
                        "settings-codex-usage"
                            | "settings-codex-usage-unknown"
                            | "settings-codex-runtime-unknown"
                    ) {
                        self.report["settingsScrollFixtureRequested"] = json!("end");
                        iced::widget::operation::snap_to_end("native-settings-body")
                    } else {
                        Task::none()
                    };
                    return Task::batch([
                        scroll,
                        Task::perform(
                            async { tokio::time::sleep(Duration::from_millis(1000)).await },
                            move |_| Message::Observe(id),
                        ),
                    ]);
                }
            }
            Message::Observe(id) if !self.capture_started && !self.finished => {
                self.capture_started = true;
                return window::size(id).then(move |size| {
                    iced::advanced::widget::operate(LayoutObservation::default())
                        .map(move |o| Message::Observed(id, size, o))
                });
            }
            Message::Observed(id, size, observation) if !self.finished => {
                self.viewport = Some(size);
                self.report["measuredWindowLogicalViewport"] = json!([size.width, size.height]);
                self.report["layoutObservation"] =
                    observation.report(self.options.size, self.viewport, &self.report);
                // Iced 0.14 rebuilds layout before running this handler's tasks,
                // invalidating the last draw's weak editor reference. Allow the
                // requested redraw, then capture directly without another App
                // message/rebuild. This bounded scheduling assumption is not an
                // atomic frame-identity or deterministic paint-barrier guarantee.
                return Task::future(async {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                })
                .then(move |()| window::screenshot(id))
                .map(Message::Screenshot);
            }
            Message::Screenshot(screenshot) if !self.finished => {
                let path = self.options.screenshot.clone();
                let size = [screenshot.size.width, screenshot.size.height];
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            smoke::save_screenshot(&path, &screenshot).is_ok()
                        })
                        .await
                        .unwrap_or(false)
                    },
                    move |saved| Message::Saved(saved, size),
                );
            }
            Message::Saved(saved, size) if !self.finished => {
                self.report["capture"]["saved"] = json!(saved);
                self.report["capture"]["physicalBufferSize"] = json!(size);
                let progressed = self.options.mode != "working-conversation"
                    || (self.report["workingTickCount"].as_u64().unwrap_or(0) >= 2
                        && self.report["workingFramePhases"]
                            .as_array()
                            .is_some_and(|phases| {
                                phases
                                    .first()
                                    .is_some_and(|first| phases.iter().any(|phase| phase != first))
                            }));
                self.report["workingClockProgressionRequired"] =
                    json!(self.options.mode == "working-conversation");
                self.report["workingClockProgressionPassed"] = json!(progressed);
                let passed = saved
                    && self.report["windowBackend"] == "wayland"
                    && self.report["layoutObservation"]["passed"] == true
                    && progressed
                    && (self.options.drop_file.is_none()
                        || (self.report["actualProductionNativeDropQualified"] == true
                            && self.report["unexpectedDropPaths"] == 0));
                return self.finish(passed);
            }
            Message::Deadline | Message::Close if !self.finished => {
                self.report["deadlineOrManualCloseBeforeEvidence"] = json!(true);
                return self.finish(false);
            }
            Message::Closed => return iced::exit(),
            _ => {}
        }
        Task::none()
    }
    fn view(&self) -> Element<'_, Message> {
        // The PUBLIC banner is outside the fixed App frame; it never consumes App space.
        // The clipped frame constrains the *actual* production main shell, not a pixel mock.
        let banner = container(
            ui::label(
                "PUBLIC LOCAL FIXTURE · zero Host · synthetic metadata",
                ui::DANGER,
            )
            .size(11),
        )
        .padding([0, 12])
        .width(self.options.size.width)
        .center_y(BANNER_HEIGHT)
        .clip(true)
        .style(|_| ui::panel(ui::SURFACE));
        let frame = container(self.app.view().map(Message::App))
            .id("fixture-frame")
            .width(self.options.size.width)
            .height(self.options.size.height)
            .clip(true)
            .style(|_| ui::panel(ui::BASE));
        column![banner, frame]
            .width(self.options.size.width)
            .height(self.options.size.height + BANNER_HEIGHT)
            .into()
    }
    fn subscription(&self) -> Subscription<Message> {
        // Omit App.subscription(): no worker stream or keyboard/business dispatch.
        let drop_check = self.options.drop_file.is_some();
        // Only this explicit PUBLIC working preset owns a bounded local presentation clock.
        Subscription::batch([
            window::events()
                .with(drop_check)
                .filter_map(|(drop_check, (id, event))| match event {
                    window::Event::FileDropped(path) if drop_check => {
                        Some(Message::App(ui::Message::FileDropped(id, path)))
                    }
                    window::Event::FilesHoveredLeft if drop_check => {
                        Some(Message::App(ui::Message::FileDropCancelled(id)))
                    }
                    window::Event::Opened { .. } => Some(Message::Opened(id)),
                    // A compositor may tile or fullscreen the window regardless of
                    // the requested size. Dropping this left the app laid out for
                    // the old size, so a grabbed frame showed a stale layout with
                    // unpainted space beside it.
                    window::Event::Resized(size) => Some(Message::App(ui::Message::Resized(size))),
                    window::Event::CloseRequested => Some(Message::Close),
                    window::Event::Closed => Some(Message::Closed),
                    _ => None,
                }),
            if self.options.mode == "working-conversation"
                && !self.capture_started
                && !self.finished
            {
                iced::time::every(Duration::from_millis(100)).map(|_| Message::WorkingFrame)
            } else {
                Subscription::none()
            },
        ])
    }
}
fn report_passed(path: &Path) -> bool {
    fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .is_some_and(|v| {
            v["status"] == "passed" && v["publicSource"] == true && v["effectsDispatched"] == 0
        })
}
fn main() -> iced::Result {
    // Before any Iced/Tokio/owner threads, matching the production renderer selection.
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    let options = options().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2)
    });
    let report = options.report.clone();
    let window_size = Size::new(
        options.size.width * options.scale,
        (options.size.height + BANNER_HEIGHT) * options.scale,
    );
    let result = iced::application(
        move || Fixture::boot(options.clone()),
        Fixture::update,
        Fixture::view,
    )
    .title("Harness actual App layout · PUBLIC LOCAL FIXTURE · zero Host")
    .window(window::Settings {
        size: window_size,
        resizable: false,
        platform_specific: window::settings::PlatformSpecific {
            application_id: "ai.deepseek.harness.native.public-layout-fixture".into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .exit_on_close_request(false)
    .settings(iced::Settings {
        default_text_size: iced::Pixels(14.0),
        ..Default::default()
    })
    .default_font(ui::FONT)
    .theme(|_: &Fixture| ui::native_theme())
    .scale_factor(|f: &Fixture| f.options.scale)
    .subscription(Fixture::subscription)
    .run();
    if result.is_ok() && !report_passed(&report) {
        eprintln!("PUBLIC actual-App layout fixture failed; inspect its report and paint");
        std::process::exit(1);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observer_collects_only_known_ids_and_no_text() {
        let mut observation = LayoutObservation::default();
        observation.container(
            Some(&Id::from("private-unlisted-id")),
            Rectangle::with_size(Size::new(10.0, 10.0)),
        );
        observation.text(
            None,
            Rectangle::with_size(Size::new(10.0, 10.0)),
            "unobserved text",
        );
        assert!(observation.rects.is_empty());
        observation.container(
            Some(&Id::from("fixture-frame")),
            Rectangle::with_size(Size::new(760.0, 560.0)),
        );
        assert_eq!(observation.count("fixture-frame"), 1);
    }
    #[test]
    fn rectangle_check_rejects_off_frame_or_nonfinite_bounds() {
        let frame = Rectangle::with_size(Size::new(760.0, 560.0));
        assert!(inside(
            &Rectangle {
                x: 10.0,
                y: 10.0,
                width: 60.0,
                height: 30.0
            },
            &frame
        ));
        assert!(!inside(
            &Rectangle {
                x: 740.0,
                y: 10.0,
                width: 60.0,
                height: 30.0
            },
            &frame
        ));
        assert!(!inside(
            &Rectangle {
                x: f32::NAN,
                y: 10.0,
                width: 60.0,
                height: 30.0
            },
            &frame
        ));
    }
}
