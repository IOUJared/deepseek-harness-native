//! PUBLIC synthetic text only: geometry microbenchmark, no Host/worker executor.
//! The shared source modules are included only so `chat_geometry`'s inline helpers resolve.
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
// The probe calls the cache from the crate root, but `chat_geometry` lives inside
// `ui` in the App so its inline helpers can read the UI's private modules. Include
// the same file inside a wrapper and re-export only the cache the probe drives.
mod ui {
    include!("../src/ui.rs");
    pub(crate) use chat_geometry::Cache;
}
#[path = "../src/work_panel.rs"]
mod work_panel;
#[path = "../src/worker.rs"]
mod worker;

fn main() {
    let cache = ui::Cache::default();
    let rows: Vec<_> = (0..4096_u64)
        .map(|key| (key, format!("PUBLIC synthetic preview {key}: native text measurement, not a model response. café · 世界")))
        .collect();
    let cold = std::time::Instant::now();
    let total: f32 = rows
        .iter()
        .map(|(key, text)| {
            cache
                .geometry(
                    *key,
                    if key % 2 == 0 { "You" } else { "Assistant" },
                    text,
                    0.0,
                    740.0,
                )
                .height
        })
        .sum();
    let cold_ms = cold.elapsed().as_secs_f64() * 1000.0;
    let warm = std::time::Instant::now();
    for _ in 0..10 {
        cache.retain_keys(rows.iter().map(|(key, _)| *key));
        let again: f32 = rows
            .iter()
            .map(|(key, text)| {
                cache
                    .geometry(
                        *key,
                        if key % 2 == 0 { "You" } else { "Assistant" },
                        text,
                        0.0,
                        740.0,
                    )
                    .height
            })
            .sum();
        assert_eq!(total, again);
    }
    let warm_ms = warm.elapsed().as_secs_f64() * 1000.0 / 10.0;
    println!(
        "{}",
        serde_json::json!({"scope":"PUBLIC geometry-only 4096-row release microbenchmark; no GUI/Host or integrated performance claim", "rows":rows.len(), "coldMsIncludingFontInitialization":cold_ms, "warmMeanMs":warm_ms, "heightStable":true})
    );
}
