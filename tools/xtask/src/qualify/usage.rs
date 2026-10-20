//! Own native App/worker/full-alpha PUBLIC usage, runtime or local account lifecycle.
//!
//! Port of the document half of `scripts/qualify-composed-usage.py`. The runtime
//! patch is read with a small YAML subset reader for the pinned writer profile,
//! because the product does not ship a YAML codec and the fixture is ours.
use serde_json::Value;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use super::composed::{self, Key, Owned, cleanup_owned, identity, observe_descendants, pin_root};
use super::upload::sha256_hex;

/// A bounded YAML subset: block mappings, block lists of mappings, and scalars.
pub fn yaml(text: &str) -> Result<Value, String> {
    let mut lines: Vec<(usize, &str)> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        lines.push((indent, line.trim_start()));
    }
    let mut position = 0;
    parse_block(&lines, &mut position, 0)
}

fn scalar(text: &str) -> Value {
    let text = text.trim();
    if text == "true" {
        return Value::Bool(true);
    }
    if text == "false" {
        return Value::Bool(false);
    }
    if text == "null" || text == "~" {
        return Value::Null;
    }
    if let Ok(number) = text.parse::<i64>() {
        return Value::from(number);
    }
    if let Ok(number) = text.parse::<f64>() {
        return Value::from(number);
    }
    let unquoted = text
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            text.strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        })
        .unwrap_or(text);
    Value::String(unquoted.to_string())
}

fn parse_block(
    lines: &[(usize, &str)],
    position: &mut usize,
    indent: usize,
) -> Result<Value, String> {
    if *position >= lines.len() {
        return Err("empty yaml block".into());
    }
    if lines[*position].1.starts_with("- ") || lines[*position].1 == "-" {
        let mut items = Vec::new();
        while *position < lines.len()
            && lines[*position].0 == indent
            && lines[*position].1.starts_with('-')
        {
            let rest = lines[*position]
                .1
                .trim_start_matches('-')
                .trim_start()
                .to_string();
            let item_indent = indent + 2;
            *position += 1;
            if rest.is_empty() {
                items.push(parse_block(lines, position, item_indent)?);
                continue;
            }
            // A list item that starts a mapping: the first key shares the dash line.
            let mut fields = serde_json::Map::new();
            if let Some((key, value)) = rest.split_once(':') {
                fields.insert(
                    key.trim().to_string(),
                    parse_value(lines, position, item_indent, value)?,
                );
            } else {
                items.push(scalar(&rest));
                continue;
            }
            while *position < lines.len()
                && lines[*position].0 == item_indent
                && !lines[*position].1.starts_with("- ")
            {
                let (key, value) = lines[*position]
                    .1
                    .split_once(':')
                    .ok_or("unsupported yaml line")?;
                *position += 1;
                fields.insert(
                    key.trim().to_string(),
                    parse_value(lines, position, item_indent, value)?,
                );
            }
            items.push(Value::Object(fields));
        }
        return Ok(Value::Array(items));
    }
    let mut fields = serde_json::Map::new();
    while *position < lines.len() && lines[*position].0 == indent {
        let (key, value) = lines[*position]
            .1
            .split_once(':')
            .ok_or("unsupported yaml line")?;
        *position += 1;
        fields.insert(
            key.trim().to_string(),
            parse_value(lines, position, indent, value)?,
        );
    }
    Ok(Value::Object(fields))
}

fn parse_value(
    lines: &[(usize, &str)],
    position: &mut usize,
    indent: usize,
    value: &str,
) -> Result<Value, String> {
    let value = value.trim();
    if !value.is_empty() {
        return Ok(scalar(value));
    }
    if *position < lines.len() && lines[*position].0 > indent {
        return parse_block(lines, position, lines[*position].0);
    }
    Ok(Value::Null)
}

fn private_text(path: &Path, limit: u64) -> Result<(Vec<u8>, u32), String> {
    use std::os::unix::fs::MetadataExt;
    let data = crate::launcher::private_read(path, limit)?;
    let info = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    Ok((data, info.mode() & 0o777))
}

/// The exact PUBLIC "usage off" document and its file facts.
pub fn usage_document(root: &Path) -> Result<Value, String> {
    let path = root.join("harness/codex-usage/usage.v1.json");
    let (data, mode) = private_text(&path, 65536)?;
    if mode != 0o600 {
        return Err("public usage document must be private".into());
    }
    let value: Value = serde_json::from_slice(&data).map_err(|error| error.to_string())?;
    let expected = serde_json::json!({
        "version": 1, "revision": 5,
        "preferences": {"composerDisplay": "off", "usageAlert": "off",
                        "customRemainingPercent": 17.5, "retryAfterReset": false,
                        "experimentalQuota": false},
    });
    if value != expected {
        return Err("public usage document differs from the pinned off document".into());
    }
    Ok(serde_json::json!({
        "exactPublicOffDocument": true,
        "sha256": crate::sha256_file(&path).map_err(|error| error.to_string())?,
        "bytes": data.len(),
        "mode": "0o600",
    }))
}

/// The usage owner lock must be free once the phase is over.
pub fn lock_released(root: &Path) -> Result<bool, String> {
    use std::os::unix::ffi::OsStrExt;
    let path = root.join("harness/codex-usage/owner.lock");
    let info = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
    if !info.is_file() || info.file_type().is_symlink() {
        return Ok(false);
    }
    let c_path =
        std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| "path-with-nul")?;
    // Safe: a NUL-terminated path and constant flags; the descriptor is closed below.
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDWR | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Ok(false);
    }
    // Safe: a live descriptor; the lock is exclusive and non-blocking.
    let acquired = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } == 0;
    if acquired {
        // Safe: release the lock this call took.
        unsafe { libc::flock(fd, libc::LOCK_UN) };
    }
    // Safe: the probe descriptor is closed exactly once.
    unsafe { libc::close(fd) };
    Ok(acquired)
}

const PIN_KEYS: [&str; 8] = [
    "transport",
    "cacheRetention",
    "timeoutMs",
    "websocketConnectTimeoutMs",
    "streamIdleTimeoutMs",
    "requestImagePixelBudget",
    "requestImageMaxBytes",
    "maxRequestImageBytes",
];

fn pin_value(key: &str) -> Value {
    match key {
        "transport" => Value::String("websocket-cached".into()),
        "cacheRetention" => Value::String("long".into()),
        "timeoutMs" | "websocketConnectTimeoutMs" => Value::from(0),
        "streamIdleTimeoutMs" => Value::from(12.5),
        "requestImagePixelBudget" | "requestImageMaxBytes" => Value::from(524288),
        "maxRequestImageBytes" => Value::from(8388608),
        _ => Value::Null,
    }
}

/// The runtime patch: eight exact PUBLIC pins, or all of them removed.
pub fn runtime_document(root: &Path, pinned: bool) -> Result<Value, String> {
    let path = root.join("harness/profiles/desktop/cordis.patch.yml");
    let (data, mode) = private_text(&path, 65536)?;
    if mode != 0o600 {
        return Err("public runtime patch must be private".into());
    }
    let text = String::from_utf8(data.clone()).map_err(|_| "runtime patch is not UTF-8")?;
    let rows = yaml(&text)?;
    let rows = rows
        .as_array()
        .filter(|rows| rows.len() == 1)
        .ok_or("runtime patch must hold one row")?;
    let row = &rows[0];
    if row["id"].as_str() != Some("llm-pi-ai")
        || row["name"].as_str() != Some("@deepseek-ai/dsh-llm-pi-ai")
    {
        return Err("runtime patch row is not the pinned plugin".into());
    }
    let config = &row["config"];
    let vault = root.join("harness/codex-accounts");
    if config["nativeCodexAccounts"].as_bool() != Some(true)
        || config["nativeCodexAccountVault"].as_str() != Some(&vault.to_string_lossy())
    {
        return Err("runtime patch account flags are not the pinned ones".into());
    }
    let providers = config["providers"]
        .as_object()
        .ok_or("runtime patch has no providers")?;
    if providers.len() != 1 || !providers.contains_key("openai-codex") {
        return Err("runtime patch must list only the pinned provider".into());
    }
    let route = &providers["openai-codex"];
    if route["displayName"].as_str() != Some("PUBLIC preserved route label") {
        return Err("runtime patch route label changed".into());
    }
    let present: Vec<&str> = PIN_KEYS
        .iter()
        .copied()
        .filter(|key| route.get(*key).is_some())
        .collect();
    if pinned {
        if present.len() != PIN_KEYS.len()
            || PIN_KEYS.iter().any(|key| route[*key] != pin_value(key))
        {
            return Err("runtime patch does not carry exactly the eight pins".into());
        }
    } else if !present.is_empty() {
        return Err("runtime patch still carries a pin".into());
    }
    let mut other = rows[0].clone();
    if let Some(route) = other["config"]["providers"]["openai-codex"].as_object_mut() {
        for key in PIN_KEYS {
            route.remove(key);
        }
    }
    Ok(serde_json::json!({
        "exactEightPublicPins": pinned,
        "allEightPinsRemoved": !pinned,
        "labelAndNativeFlagsPreserved": true,
        "unrelatedFieldsSha256": sha256_hex(canonical_json(&other).as_bytes()),
        "sha256": sha256_hex(&data),
        "bytes": data.len(),
        "mode": "0o600",
        "yamlReaderVersion": "xtask-yaml-subset",
    }))
}

/// Recursively key-sorted JSON, so a digest is stable across phases.
pub fn canonical_json(value: &Value) -> String {
    fn sorted(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let mut ordered = std::collections::BTreeMap::new();
                for (key, value) in map {
                    ordered.insert(key.clone(), sorted(value));
                }
                Value::Object(ordered.into_iter().collect())
            }
            Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
            other => other.clone(),
        }
    }
    serde_json::to_string(&sorted(value)).unwrap_or_default()
}

const ACCOUNT_IDS: [&str; 2] = [
    "account-11111111-1111-4111-8111-111111111111",
    "account-22222222-2222-4222-8222-222222222222",
];

/// The owned synthetic PUBLIC account document, selected or emptied.
pub fn account_document(root: &Path, selected: bool) -> Result<Value, String> {
    let path = root.join("harness/codex-accounts/accounts.v1.json");
    let (data, mode) = private_text(&path, 65536)?;
    if mode != 0o600 {
        return Err("public account document must be private".into());
    }
    let document: Value = serde_json::from_slice(&data).map_err(|error| error.to_string())?;
    let accounts: Vec<Value> = ACCOUNT_IDS
        .iter()
        .zip(["first", "second"])
        .map(|(id, label)| {
            serde_json::json!({"opaqueId": id, "label": format!("PUBLIC {label}"),
                               "email": Value::Null, "plan": Value::Null})
        })
        .collect();
    let manifest = serde_json::json!({
        "accounts": if selected { Value::Array(accounts) } else { Value::Array(Vec::new()) },
        "selected": if selected { Value::String(ACCOUNT_IDS[1].into()) } else { Value::Null },
        "epoch": if selected { 2 } else { 4 },
    });
    if document["version"].as_i64() != Some(1) || document["manifest"] != manifest {
        return Err("public account manifest differs from the pinned one".into());
    }
    let records: Vec<Value> = ACCOUNT_IDS
        .iter()
        .enumerate()
        .map(|(index, id)| {
            serde_json::json!({
                "key": format!("llm-pi-ai-codex-accounts/{id}"),
                "record": {"kind": "grant", "payload": {
                    "type": "oauth",
                    "access": format!("PUBLIC-access-{index}"),
                    "refresh": format!("PUBLIC-refresh-{index}"),
                    "expires": 4102444800000i64,
                    "accountId": format!("PUBLIC-issuer-{index}"),
                    "extra": {"untouched": ["PUBLIC-extra", Value::Null]},
                }},
            })
        })
        .collect();
    let expected_records = if selected {
        Value::Array(records)
    } else {
        Value::Array(Vec::new())
    };
    if document["records"] != expected_records {
        return Err("public account records differ from the pinned ones".into());
    }
    Ok(serde_json::json!({
        "version": 1,
        "epoch": document["manifest"]["epoch"],
        "publicAccountCount": document["manifest"]["accounts"].as_array().map(Vec::len).unwrap_or(0),
        "selectedPublicSecond": selected,
        "publicStoredPayloadsUnchanged": selected,
        "allLocalRecordsDeleted": !selected,
        "sha256": sha256_hex(&data),
        "bytes": data.len(),
        "mode": "0o600",
    }))
}

/// The accounts owner lock must be free.
pub fn account_lock_released(root: &Path) -> Result<bool, String> {
    use std::os::unix::ffi::OsStrExt;
    let path = root.join("harness/codex-accounts/owner.lock");
    let info = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
    if !info.is_file() || info.file_type().is_symlink() {
        return Ok(false);
    }
    let c_path =
        std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| "path-with-nul")?;
    // Safe: a NUL-terminated path and constant flags; the descriptor is closed below.
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDWR | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Ok(false);
    }
    // Safe: a live descriptor; the lock is exclusive and non-blocking.
    let acquired = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } == 0;
    if acquired {
        // Safe: release the lock this call took.
        unsafe { libc::flock(fd, libc::LOCK_UN) };
    }
    // Safe: the probe descriptor is closed exactly once.
    unsafe { libc::close(fd) };
    Ok(acquired)
}

fn private_capture(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::symlink_metadata(path)
        .map(|info| {
            info.is_file()
                && !info.file_type().is_symlink()
                && info.permissions().mode() & 0o777 == 0o600
        })
        .unwrap_or(false)
}

/// The runtime phase must expose its own renderer capture.
pub fn runtime_valid(app: &Value, output: &Path) -> bool {
    app["configurationScope"].as_str() == Some("runtime") && output.join("own-images.png").is_file()
}

/// The accounts phases: maintenance, consent captures and readbacks.
pub fn accounts_valid(app: &Value, name: &str, output: &Path) -> bool {
    let expected_commands = match name {
        "initial" => 8,
        "restored" => 6,
        "inherited" => 2,
        _ => return false,
    };
    let consent: &[&str] = match name {
        "initial" => &[
            "own-review-cancel-select.png",
            "own-review-select-first.png",
            "own-review-select-second.png",
        ],
        "restored" => &["own-review-local-logout-second.png", "own-review-all.png"],
        "inherited" => &[],
        _ => return false,
    };
    if app["configurationScope"].as_str() != Some("accounts")
        || !output.join("own-maintenance.png").is_file()
        || app["maintenanceStatusMatched"].as_bool() != Some(true)
        || app["maintenanceAccountActionsRejected"].as_bool() != Some(true)
        || app["accountScreenshotSaved"].as_bool() != Some(true)
        || app["accountOrAuthorizationCommands"].as_i64() != Some(expected_commands)
        || app["accountConsentCaptures"] != serde_json::json!(consent)
        || !consent
            .iter()
            .all(|file| private_capture(&output.join(file)))
        || app["publicStoredFixturesOnly"].as_bool() != Some(true)
        || app["oauthOrBrowserOrEnableModelsAdmitted"].as_bool() != Some(false)
    {
        return false;
    }
    if name != "initial" {
        return true;
    }
    let final_account = &app["accountFinal"];
    app["localReviewCancelAndNoReviewConfirmNotAdmitted"].as_bool() == Some(true)
        && app["localReviewCancelEpochAndRosterReadbackMatched"].as_bool() == Some(true)
        && app["localReviewCancelReadback"] == app["accountLoaded"]
        && app["selectedSwitchActionsRejectedByActualApp"].as_bool() == Some(true)
        && app["selectedSwitchEpochAndSelectionUnchangedAfterExplicitReadback"].as_bool()
            == Some(true)
        && app["selectedSwitchRejection"]
            == serde_json::json!({"epoch": 1, "selected": ACCOUNT_IDS[0], "ticketUnchangedAfterEachAction": true})
        && {
            let mut readback = final_account.clone();
            if let Some(object) = readback.as_object_mut() {
                object.insert("epoch".into(), serde_json::json!(1));
                object.insert("selected".into(), serde_json::json!(ACCOUNT_IDS[0]));
            }
            app["selectedSwitchReadback"] == readback
        }
}

const USAGE_APP_ID: &str = "ai.deepseek.harness.native.usage-composed-fixture";

struct PhaseOutcome {
    passed: bool,
    report: Value,
    app: Value,
    durable_document: Option<Value>,
    durable_account: Option<Value>,
    durable_runtime: Option<Value>,
}

fn phase(
    binary: &Path,
    root: &Path,
    runtime: &Path,
    name: &str,
    scope: &str,
    seconds: u64,
) -> PhaseOutcome {
    let output = root.join(name);
    let report = match std::fs::create_dir(&output) {
        Ok(()) => {
            let _ = std::fs::set_permissions(
                &output,
                std::os::unix::fs::PermissionsExt::from_mode(0o700),
            );
            None
        }
        Err(_) => Some("phase-output-exists"),
    };
    let Some(failure) = report else {
        return run_phase(binary, root, runtime, name, scope, seconds, &output);
    };
    PhaseOutcome {
        passed: false,
        report: serde_json::json!({"status": "failed", "phase": name, "errorCode": failure}),
        app: serde_json::json!({"status": "missing"}),
        durable_document: None,
        durable_account: None,
        durable_runtime: None,
    }
}

fn run_phase(
    binary: &Path,
    root: &Path,
    runtime: &Path,
    name: &str,
    scope: &str,
    seconds: u64,
    output: &Path,
) -> PhaseOutcome {
    use std::os::unix::process::CommandExt;
    let mut environment = super::environment(root);
    environment.insert("PATH".into(), "/usr/bin:/bin".into());
    let log_path = root.join(format!("{name}-stderr.log"));
    let Ok(log) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&log_path)
    else {
        return PhaseOutcome {
            passed: false,
            report: serde_json::json!({"status": "failed", "phase": name, "errorCode": "log-open-failed"}),
            app: serde_json::json!({"status": "missing"}),
            durable_document: None,
            durable_account: None,
            durable_runtime: None,
        };
    };
    let spawned = std::process::Command::new(binary)
        .arg("--output")
        .arg(output)
        .arg("--state-root")
        .arg(root)
        .arg("--runtime")
        .arg(runtime)
        .arg("--phase")
        .arg(name)
        .arg("--scope")
        .arg(scope)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(
            log.try_clone()
                .unwrap_or_else(|_| std::fs::File::open("/dev/null").unwrap()),
        ))
        .stderr(std::process::Stdio::from(log))
        .env_clear()
        .envs(&environment)
        .process_group(0)
        .spawn();
    let Ok(child) = spawned else {
        return PhaseOutcome {
            passed: false,
            report: serde_json::json!({"status": "failed", "phase": name, "errorCode": "spawn-failed"}),
            app: serde_json::json!({"status": "missing"}),
            durable_document: None,
            durable_account: None,
            durable_runtime: None,
        };
    };
    let pid = child.id();
    let mut owned = Owned {
        child,
        status: None,
    };
    let owner = std::process::id() as i32;
    let mut pidfds: std::collections::BTreeMap<Key, i32> = std::collections::BTreeMap::new();
    let mut observed: std::collections::BTreeSet<Key> = std::collections::BTreeSet::new();
    let mut facts: Option<Value> = None;
    let mut forced = false;
    let mut failure: Option<&'static str> = None;
    let mut observation_failures = 0u64;
    let mut root_pinned = false;
    let mut pinned_root: Option<crate::proc::Stat> = None;
    match pin_root(&mut owned, owner) {
        Err(()) => {
            failure = Some("root-pin-failed");
            forced = true;
        }
        Ok((pinned, fd)) => {
            root_pinned = true;
            pidfds.insert(identity(&pinned), fd);
            observed.insert(identity(&pinned));
            pinned_root = Some(pinned);
        }
    }
    if let Some(pinned) = pinned_root.as_ref() {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
        loop {
            if owned.poll().ok().flatten().is_some() {
                break;
            }
            observation_failures +=
                observe_descendants(&mut owned, pinned, &mut pidfds, &mut observed, owner);
            if facts.is_none()
                && let Some(candidate) = crate::compositor::window_facts(pid as i32)
                && candidate["mapped"].as_bool() == Some(true)
            {
                facts = Some(Value::Object(candidate));
            }
            if composed::INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst)
                || std::time::Instant::now() >= deadline
            {
                forced = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    let cleanup = cleanup_owned(&mut owned, &pidfds, root_pinned);
    for fd in pidfds.values() {
        // Safe: each descriptor was opened for one identity this run and closed once.
        unsafe {
            libc::close(*fd);
        }
    }
    let forced = forced || cleanup.root_was_running_at_cleanup;
    let app: Value = std::fs::read(output.join("app.json"))
        .ok()
        .and_then(|body| serde_json::from_slice(&body).ok())
        .unwrap_or_else(|| serde_json::json!({"status": "missing"}));
    let passed = owned.code() == 0
        && !forced
        && failure.is_none()
        && observation_failures == 0
        && cleanup.observed_cleanup_complete
        && cleanup.remaining_before_external_cleanup.is_empty()
        && app["status"].as_str() == Some("passed")
        && facts.as_ref().is_some_and(|facts| {
            facts["class"].as_str() == Some(USAGE_APP_ID)
                && facts["xwayland"].as_bool() == Some(false)
        })
        && output.join("own-window.png").is_file();
    let mut report = serde_json::json!({
        "status": if passed { "passed" } else { "failed" },
        "phase": name,
        "scope": scope,
        "exitCode": owned.code(),
        "forced": forced,
        "errorCode": failure,
        "ownedWindow": facts,
        "observedOwnedProcessCount": observed.len(),
        "ownershipObservationFailures": observation_failures,
        "cleanup": &cleanup,
        "app": app,
        "parentPaintInspected": false,
        "nativeKeyboardPointerInputQualified": false,
    });
    let mut durable_document = None;
    let mut durable_account = None;
    let mut durable_runtime = None;
    if passed {
        match scope {
            "usage" => {
                durable_document = usage_document(root).ok();
                report["durableDocument"] = durable_document.clone().unwrap_or(Value::Null);
                report["kernelUsageLockReleased"] =
                    serde_json::json!(lock_released(root).unwrap_or(false));
            }
            "runtime" => {
                let valid = runtime_valid(&app, output);
                if !valid {
                    report["status"] = serde_json::json!("failed");
                }
                durable_runtime = runtime_document(root, name == "initial").ok();
                report["durableRuntimePatch"] = durable_runtime.clone().unwrap_or(Value::Null);
            }
            _ => {
                let valid = accounts_valid(&app, name, output);
                if !valid {
                    report["status"] = serde_json::json!("failed");
                }
                durable_account = account_document(root, name == "initial").ok();
                report["durableAccountDocument"] = durable_account.clone().unwrap_or(Value::Null);
                report["kernelAccountVaultLockReleased"] =
                    serde_json::json!(account_lock_released(root).unwrap_or(false));
            }
        }
    }
    let passed = report["status"].as_str() == Some("passed");
    if let Err(error) = super::private_json(&output.join("result.json"), &report) {
        report["errorCode"] = serde_json::json!(format!("result-write-failed: {error}"));
    }
    PhaseOutcome {
        passed,
        report,
        app,
        durable_document,
        durable_account,
        durable_runtime,
    }
}

fn projected(value: &Value) -> Value {
    let mut projected = serde_json::Map::new();
    for key in ["version", "configured", "values", "baseValues", "overrides"] {
        projected.insert(key.to_string(), value[key].clone());
    }
    Value::Object(projected)
}

/// `xtask qualify composed-usage --binary PATH --runtime DIR --output NEW [--scope]`.
pub fn run(
    root: &Path,
    binary_arg: &Path,
    runtime_arg: &Path,
    requested: &Path,
    scope: &str,
) -> i32 {
    let Ok(binary) = binary_arg.canonicalize() else {
        eprintln!("composed usage failed: native fixture");
        return 1;
    };
    let Ok(runtime) = runtime_arg.canonicalize() else {
        eprintln!("composed usage failed: runtime");
        return 1;
    };
    let Ok(before_hash) = crate::sha256_file(&binary) else {
        eprintln!("composed usage failed: binary digest");
        return 1;
    };
    let output = match super::prepare(root, requested) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("composed usage failed: {message}");
            return 1;
        }
    };
    let names: &[&str] = match scope {
        "usage" => &["initial", "restored"],
        _ => &["initial", "restored", "inherited"],
    };
    let mut outcomes: Vec<PhaseOutcome> = Vec::new();
    for name in names {
        let outcome = phase(&binary, &output, &runtime, name, scope, 40);
        let passed = outcome.passed;
        outcomes.push(outcome);
        if !passed {
            break;
        }
    }
    let passed = outcomes.len() == names.len() && outcomes.iter().all(|outcome| outcome.passed);
    let stable = crate::sha256_file(&binary).is_ok_and(|hash| hash == before_hash);
    let durable = if scope == "usage" {
        passed && outcomes[0].durable_document == outcomes[1].durable_document
    } else if scope == "accounts" {
        passed
            && outcomes[1].durable_account == outcomes[2].durable_account
            && outcomes[0].app["accountFinal"] == outcomes[1].app["accountLoaded"]
            && outcomes[1].app["accountFinal"] == outcomes[2].app["accountLoaded"]
            && outcomes[1].app["selectedLogoutHasNoReplacement"].as_bool() == Some(true)
            && outcomes[2].app["emptySignoutAllRejectedByActualApp"].as_bool() == Some(true)
    } else {
        let unrelated: std::collections::BTreeSet<String> = outcomes
            .iter()
            .filter_map(|outcome| {
                outcome.durable_runtime.as_ref()?["unrelatedFieldsSha256"]
                    .as_str()
                    .map(str::to_string)
            })
            .collect();
        passed
            && unrelated.len() == 1
            && outcomes[1]
                .durable_runtime
                .as_ref()
                .map(|value| &value["sha256"])
                == outcomes[2]
                    .durable_runtime
                    .as_ref()
                    .map(|value| &value["sha256"])
            && projected(&outcomes[0].app["runtimeAcknowledged"])
                == projected(&outcomes[1].app["runtimeLoaded"])
            && projected(&outcomes[1].app["runtimeAcknowledged"])
                == projected(&outcomes[2].app["runtimeLoaded"])
    };
    let status = if passed && stable && durable {
        "passed"
    } else {
        "failed"
    };
    let results: Vec<Value> = outcomes
        .iter()
        .map(|outcome| outcome.report.clone())
        .collect();
    let report = serde_json::json!({
        "status": status,
        "scope": "Own native App/worker/full-alpha PUBLIC usage, runtime or local account lifecycle.",
        "configurationScope": scope,
        "binarySha256": before_hash,
        "binaryUnchanged": stable,
        "results": results,
        "persistedConfigurationVerifiedAcrossIndependentProcesses": durable,
        "persistedDocumentUnchangedAcrossIndependentProcesses": if scope == "usage" { serde_json::json!(durable) } else { Value::Null },
        "runtimePinnedAndInheritedRestartsMatched": if scope == "runtime" { serde_json::json!(durable) } else { Value::Null },
        "installedRuntimeProfilesAccountsOrDesktopChanged": false,
        "actualProviderRequests": 0,
        "zeroRequestsEvidence": "reviewed commands/source scope; NOT network instrumentation",
        "accountCounterScope": "admitted native UI effects, NOT underlying private commands; each account/status refresh issues two local reads",
        "realQuotaOrNativeAlertsOrResetRetryQualified": false,
        "parentPaintInspectionRequired": true,
    });
    if let Err(error) = super::private_json(&output.join("qualification.json"), &report) {
        eprintln!("cannot write the usage report: {error}");
        return 1;
    }
    println!(
        "{}",
        serde_json::json!({"status": report["status"], "report": output.join("qualification.json").to_string_lossy()})
    );
    if status == "passed" { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn the_yaml_subset_reads_the_pinned_patch_shape() {
        let text = "\
- id: llm-pi-ai
  name: '@deepseek-ai/dsh-llm-pi-ai'
  config:
    nativeCodexAccounts: true
    nativeCodexAccountVault: /tmp/vault
    providers:
      openai-codex:
        displayName: PUBLIC preserved route label
        transport: websocket-cached
        streamIdleTimeoutMs: 12.5
        requestImagePixelBudget: 524288
";
        let value = yaml(text).expect("a supported document");
        let row = &value[0];
        assert_eq!(row["id"], "llm-pi-ai");
        assert_eq!(row["name"], "@deepseek-ai/dsh-llm-pi-ai");
        assert_eq!(row["config"]["nativeCodexAccounts"], true);
        assert_eq!(
            row["config"]["providers"]["openai-codex"]["displayName"],
            "PUBLIC preserved route label"
        );
        assert_eq!(
            row["config"]["providers"]["openai-codex"]["streamIdleTimeoutMs"],
            12.5
        );
        assert_eq!(
            row["config"]["providers"]["openai-codex"]["requestImagePixelBudget"],
            524288
        );
    }

    #[test]
    fn a_usage_document_must_be_private_and_exactly_the_pinned_off_document() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "usage-doc-").unwrap();
        let folder = base.join("harness/codex-usage");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("usage.v1.json");
        let document = serde_json::json!({
            "version": 1, "revision": 5,
            "preferences": {"composerDisplay": "off", "usageAlert": "off",
                            "customRemainingPercent": 17.5, "retryAfterReset": false,
                            "experimentalQuota": false},
        });
        fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let facts = usage_document(&base).expect("the pinned document");
        assert_eq!(facts["exactPublicOffDocument"], true);
        // A changed revision or a public mode is refused.
        let mut changed = document.clone();
        changed["revision"] = serde_json::json!(4);
        fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(usage_document(&base).is_err());
        fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(usage_document(&base).is_err());
        // A free lock is reported as released; an absent lock is not.
        assert!(!lock_released(&base).unwrap_or(false));
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod account_tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn document(selected: bool) -> Value {
        let ids = ACCOUNT_IDS;
        let accounts: Vec<Value> = ids
            .iter()
            .zip(["first", "second"])
            .map(|(id, label)| {
                serde_json::json!({"opaqueId": id, "label": format!("PUBLIC {label}"),
                                   "email": Value::Null, "plan": Value::Null})
            })
            .collect();
        let records: Vec<Value> = ids
            .iter()
            .enumerate()
            .map(|(index, id)| {
                serde_json::json!({
                    "key": format!("llm-pi-ai-codex-accounts/{id}"),
                    "record": {"kind": "grant", "payload": {
                        "type": "oauth", "access": format!("PUBLIC-access-{index}"),
                        "refresh": format!("PUBLIC-refresh-{index}"),
                        "expires": 4102444800000i64,
                        "accountId": format!("PUBLIC-issuer-{index}"),
                        "extra": {"untouched": ["PUBLIC-extra", Value::Null]},
                    }},
                })
            })
            .collect();
        serde_json::json!({
            "version": 1,
            "manifest": {"accounts": if selected { Value::Array(accounts) } else { Value::Array(Vec::new()) },
                          "selected": if selected { Value::String(ids[1].into()) } else { Value::Null },
                          "epoch": if selected { 2 } else { 4 }},
            "records": if selected { Value::Array(records) } else { Value::Array(Vec::new()) },
        })
    }

    #[test]
    fn the_account_document_is_either_the_pinned_pair_or_emptied() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "usage-accounts-").unwrap();
        let folder = base.join("harness/codex-accounts");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("accounts.v1.json");
        fs::write(&path, serde_json::to_vec(&document(true)).unwrap()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let selected = account_document(&base, true).expect("the selected document");
        assert_eq!(selected["selectedPublicSecond"], true);
        assert_eq!(selected["publicAccountCount"], 2);
        assert_eq!(selected["epoch"], 2);
        fs::write(&path, serde_json::to_vec(&document(false)).unwrap()).unwrap();
        let emptied = account_document(&base, false).expect("the emptied document");
        assert_eq!(emptied["allLocalRecordsDeleted"], true);
        assert_eq!(emptied["epoch"], 4);
        // A mixed document (manifest emptied but records kept) is refused.
        let mut mixed = document(false);
        mixed["records"] = document(true)["records"].clone();
        fs::write(&path, serde_json::to_vec(&mixed).unwrap()).unwrap();
        assert!(account_document(&base, false).is_err());
        // A selected read of an emptied document is refused, and so is a public mode.
        fs::write(&path, serde_json::to_vec(&document(false)).unwrap()).unwrap();
        assert!(account_document(&base, true).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(account_document(&base, false).is_err());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn the_accounts_phase_needs_maintenance_counts_consent_and_readbacks() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "usage-phase-").unwrap();
        for file in [
            "own-maintenance.png",
            "own-review-cancel-select.png",
            "own-review-select-first.png",
            "own-review-select-second.png",
        ] {
            let path = base.join(file);
            fs::write(&path, b"PUBLIC capture").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let account = serde_json::json!({"opaqueId": ACCOUNT_IDS[0], "label": "PUBLIC first"});
        let mut app = serde_json::json!({
            "configurationScope": "accounts",
            "maintenanceStatusMatched": true,
            "maintenanceAccountActionsRejected": true,
            "accountScreenshotSaved": true,
            "accountOrAuthorizationCommands": 8,
            "accountConsentCaptures": ["own-review-cancel-select.png",
                                        "own-review-select-first.png",
                                        "own-review-select-second.png"],
            "publicStoredFixturesOnly": true,
            "oauthOrBrowserOrEnableModelsAdmitted": false,
            "accountLoaded": account.clone(),
            "accountFinal": {"opaqueId": ACCOUNT_IDS[1], "label": "PUBLIC second"},
            "localReviewCancelAndNoReviewConfirmNotAdmitted": true,
            "localReviewCancelEpochAndRosterReadbackMatched": true,
            "localReviewCancelReadback": account.clone(),
            "selectedSwitchActionsRejectedByActualApp": true,
            "selectedSwitchEpochAndSelectionUnchangedAfterExplicitReadback": true,
            "selectedSwitchRejection": {"epoch": 1, "selected": ACCOUNT_IDS[0],
                                         "ticketUnchangedAfterEachAction": true},
        });
        let mut readback = app["accountFinal"].clone();
        readback["epoch"] = serde_json::json!(1);
        readback["selected"] = serde_json::json!(ACCOUNT_IDS[0]);
        app["selectedSwitchReadback"] = readback;
        assert!(accounts_valid(&app, "initial", &base));
        // A wrong command count, a missing consent capture and a foreign rejection fail.
        let mut commands = app.clone();
        commands["accountOrAuthorizationCommands"] = serde_json::json!(6);
        assert!(!accounts_valid(&commands, "initial", &base));
        let mut consent = app.clone();
        consent["accountConsentCaptures"] = serde_json::json!(["own-review-all.png"]);
        assert!(!accounts_valid(&consent, "initial", &base));
        let mut rejection = app.clone();
        rejection["selectedSwitchRejection"]["selected"] = serde_json::json!(ACCOUNT_IDS[1]);
        assert!(!accounts_valid(&rejection, "initial", &base));
        // The inherited phase has no consent captures.
        let mut inherited = app.clone();
        inherited["accountOrAuthorizationCommands"] = serde_json::json!(2);
        inherited["accountConsentCaptures"] = serde_json::json!([]);
        assert!(accounts_valid(&inherited, "inherited", &base));
        fs::remove_dir_all(base).unwrap();
    }
}
