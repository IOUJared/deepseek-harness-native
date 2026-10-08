//! Opt-in, read-only real alpha plugin bridge qualification in exclusively new private homes.
//! No GUI, credentials, model/catalog, plugin activation changes or settings writes.
use dsh_native_core::{Backend, RustBackendOptions, StopPolicy};
use dsh_native_transport::Limits;
use serde_json::json;
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
    time::Duration,
};

fn run() -> Result<(), &'static str> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[0] != "--output" {
        return Err("invalid-arguments");
    }
    let output = PathBuf::from(&args[1]);
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let evidence = base
        .join("evidence")
        .canonicalize()
        .map_err(|_| "evidence-required")?;
    let parent = output
        .parent()
        .ok_or("output-parent-required")?
        .canonicalize()
        .map_err(|_| "existing-parent-required")?;
    if !output.is_absolute()
        || !parent.starts_with(&evidence)
        || output.exists()
        || output
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("new-isolated-output-required");
    }
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&output)
        .map_err(|_| "output-create-failed")?;
    for name in ["native-home", "user-home", "workspace"] {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(output.join(name))
            .map_err(|_| "home-create-failed")?;
    }
    let options = RustBackendOptions {
        runtime: base
            .parent()
            .ok_or("workspace-required")?
            .parent()
            .ok_or("workspace-required")?
            .join("deepseek-harness-linux/apps/cli"),
        expected_version: "0.2.1-alpha.1".into(),
        native_home: output.join("native-home"),
        user_home: output.join("user-home"),
        working_directory: output.join("workspace"),
        absolute_node_path: Some("/usr/bin/node".into()),
        startup_timeout: Duration::from_secs(20),
        stop_policy: StopPolicy::default(),
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| "runtime-start-failed")?;
    let (backend, _events) = Backend::start(options).map_err(|_| "core-start-failed")?;
    let mut report = json!({"status":"failed", "scope":"real-alpha-read-only-plugin-bridge",
        "modelPrompts":0,"modelCatalogRequested":false,"credentialOperations":0,
        "settingsMutations":0,"pluginEnableDisableInstallOperations":0,
        "actualSettingsPersistenceQualified":false,"nativeKeyboardPointerInputQualified":false});
    let checked = runtime.block_on(async {
        let ready = backend.wait_ready().map_err(|_| "core-ready-failed")?;
        if ready.version != "0.2.1-alpha.1" {
            return Err("wrong-alpha-version");
        }
        let client = backend
            .connect_transport(Limits {
                timeout: Duration::from_secs(10),
                ..Default::default()
            })
            .await
            .map_err(|_| "transport-connect-failed")?;
        let result = tokio::time::timeout(Duration::from_secs(25), async {
            let inventory = client
                .plugin_inventory()
                .await
                .map_err(|_| "plugin-inventory-failed")?;
            let settings = client
                .plugin_settings()
                .await
                .map_err(|_| "plugin-settings-failed")?;
            report["inventoryEntries"] = json!(inventory.entries.len());
            report["settingsNamespaces"] = json!(settings.namespaces.len());
            report["settingsWritable"] = json!(settings.writable);
            report["candidatePrimitiveFields"] = json!(
                settings
                    .namespaces
                    .iter()
                    .map(|ns| ns
                        .fields
                        .iter()
                        .filter(|field| field.value.is_some())
                        .count())
                    .sum::<usize>()
            );
            report["unsupportedFields"] = json!(
                settings
                    .namespaces
                    .iter()
                    .map(|ns| ns.unsupported_fields)
                    .sum::<usize>()
            );
            report["sensitiveFieldsExcluded"] = json!(
                settings
                    .namespaces
                    .iter()
                    .map(|ns| ns.secret_fields)
                    .sum::<usize>()
            );
            if inventory.entries.is_empty() || settings.namespaces.is_empty() {
                return Err("empty-alpha-composition");
            }
            if inventory.management_available {
                return Err("unexpected-management-authority");
            }
            if settings
                .namespaces
                .iter()
                .any(|ns| !ns.auto_generate && !ns.fields.is_empty())
            {
                return Err("custom-presentation-not-read-only");
            }
            Ok(())
        })
        .await
        .map_err(|_| "read-deadline")
        .and_then(|v| v);
        let stop = backend.stop_async();
        client.close().await;
        drop(stop);
        result
    });
    let stopped = backend.stop().map_err(|_| "core-stop-failed");
    if let Ok(stop) = &stopped {
        report["stop"] = json!({"exited":stop.exited,"graceful":stop.graceful,"exitCode":stop.exit_code,
            "containmentUnknown":stop.containment_unknown,"observedDescendantsRemaining":stop.observed_descendants_remaining});
    }
    let result = checked.and_then(|_| {
        stopped.and_then(|stop| {
            if stop.exited
                && stop.graceful
                && stop.exit_code == Some(0)
                && !stop.containment_unknown
                && stop.observed_descendants_remaining == 0
            {
                Ok(())
            } else {
                Err("owned-shutdown-not-clean")
            }
        })
    });
    match result {
        Ok(()) => report["status"] = json!("passed"),
        Err(code) => report["errorCode"] = json!(code),
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output.join("result.json"))
        .map_err(|_| "report-create-failed")?;
    writeln!(
        file,
        "{}",
        serde_json::to_string_pretty(&report).map_err(|_| "report-encode-failed")?
    )
    .map_err(|_| "report-write-failed")?;
    result
}
fn main() {
    if let Err(code) = run() {
        eprintln!("Plugin bridge qualification: {code}");
        std::process::exit(1);
    }
    println!("Read-only plugin bridge qualification passed");
}
