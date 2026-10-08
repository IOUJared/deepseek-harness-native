//! Opt-in real-alpha Codex metadata check in new, isolated private homes.
//! Never starts authorization, launches a browser, submits callbacks, enables models or reads tokens.
use dsh_native_core::{Backend, CodexPhase, Command, PublicReply, RustBackendOptions, StopPolicy};
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
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| "runtime-create-failed")?;
    let _enter = runtime.enter();
    let options = RustBackendOptions {
        runtime: base
            .parent()
            .and_then(|p| p.parent())
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
    let (backend, _events) = Backend::start(options).map_err(|_| "core-start-failed")?;
    let checked = (|| {
        let ready = backend.wait_ready().map_err(|_| "core-ready-failed")?;
        if ready.version != "0.2.1-alpha.1" {
            return Err("wrong-alpha-version");
        }
        for _ in 0..2 {
            let PublicReply::Codex(meta) = backend
                .request(Command::CodexStatus, Duration::from_secs(10))
                .map_err(|_| "status-read-failed")?
            else {
                return Err("wrong-status-reply");
            };
            if !meta.available
                || meta.credential_stored
                || meta.route_configured
                || meta.phase != CodexPhase::Idle
                || meta.attempt.is_some()
                || meta.prompt.is_some()
                || meta.browser_available
                || meta.retry_blocked
                || meta.settings_revision.is_none()
            {
                return Err("unexpected-fresh-metadata");
            }
        }
        Ok(())
    })();
    let stopped = backend.stop().map_err(|_| "core-stop-failed");
    let mut report = json!({"status":"failed","scope":"real-alpha-isolated-read-only-codex-status",
        "alphaVersion":"0.2.1-alpha.1","metadataReads":2,"authorizationBegins":0,"callbacksSubmitted":0,
        "browserLaunches":0,"routeMutations":0,"modelCatalogRequests":0,"modelPrompts":0,
        "credentialPayloadExposed":false,"realOpenAiLoginQualified":false,"realGptEntitlementQualified":false,
        "nativeKeyboardPointerInputQualified":false});
    let result = checked.and_then(|_| stopped.map(|_| ()));
    match &result {
        Ok(()) => report["status"] = json!("passed"),
        Err(code) => report["failureCode"] = json!(code),
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output.join("result.json"))
        .map_err(|_| "report-create-failed")?;
    file.write_all(
        serde_json::to_string_pretty(&report)
            .map_err(|_| "report-encode-failed")?
            .as_bytes(),
    )
    .map_err(|_| "report-write-failed")?;
    result
}
fn main() {
    if let Err(code) = run() {
        eprintln!("Codex metadata fixture failed: {code}");
        std::process::exit(1);
    }
}
