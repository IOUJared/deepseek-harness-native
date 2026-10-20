//! Own native App/worker/full-alpha PUBLIC usage, runtime or local account lifecycle.
//!
//! Port of the document half of `scripts/qualify-composed-usage.py`. The runtime
//! patch is read with a small YAML subset reader for the pinned writer profile,
//! because the product does not ship a YAML codec and the fixture is ours.
use serde_json::Value;
use std::path::Path;

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
