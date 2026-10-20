//! Development package launcher: path, environment and Node contract.
//!
//! First stage of the port of `scripts/development-launcher.py` — the only Python
//! file copied into the release. These pieces are pure and testable without a GUI:
//! absolute-path refusal, package overlap, the environment allow-list, the state
//! marker and the Node 26 version line.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const VERSION: &str = "0.2.1-alpha.1";
pub const APP: &str = "ai.deepseek.harness.native.development";
pub const MAX_BINARY: u64 = 64 * 1024 * 1024;

/// An explicitly absolute directory, free of control characters and `..`.
pub fn absolute(value: &str) -> Result<PathBuf, String> {
    if value.len() > 4096 || value.chars().any(|c| (c as u32) < 0x20 || c == '\u{7f}') {
        return Err("Invalid absolute directory".into());
    }
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || path
            .components()
            .any(|part| part == std::path::Component::ParentDir)
    {
        return Err("Explicit absolute directory required".into());
    }
    Ok(path)
}

/// One directory contains the other (or they are the same).
pub fn overlaps(a: &Path, b: &Path) -> bool {
    a == b || a.starts_with(b) || b.starts_with(a)
}

/// The inherited environment is an allow-list, never the caller's environment.
pub fn environment(
    user: &Path,
    inherited: &BTreeMap<String, String>,
    display: bool,
) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::from([
        ("HOME".to_string(), user.to_string_lossy().into_owned()),
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("LANG".to_string(), "C.UTF-8".to_string()),
    ]);
    for key in [
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "XDG_RUNTIME_DIR",
        "WAYLAND_DISPLAY",
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_SESSION_TYPE",
        "XDG_CURRENT_DESKTOP",
        "XDG_CONFIG_HOME",
        "XDG_CACHE_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
    ] {
        if let Some(value) = inherited.get(key) {
            if value.len() > 4096 || value.chars().any(|c| (c as u32) < 0x20 || c == '\u{7f}') {
                return Err("Invalid display or locale environment".into());
            }
            result.insert(key.to_string(), value.clone());
        }
    }
    // Preserve only this isolation exclusion; never use it to choose native storage.
    if let Some(home) = inherited.get("DSH_HOME") {
        result.insert(
            "DSH_HOME".to_string(),
            absolute(home)?.to_string_lossy().into_owned(),
        );
    }
    if display
        && (!result.contains_key("WAYLAND_DISPLAY") || !result.contains_key("XDG_RUNTIME_DIR"))
    {
        return Err("This development package requires the current Wayland session".into());
    }
    Ok(result)
}

/// What the native side must see before it reuses an existing data directory.
pub fn marker_for(manifest: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "application": APP,
        "layout": 1,
        "runtimeVersion": VERSION,
        "nativeSha256": manifest["nativeSha256"],
    })
}

/// Accept exactly one bounded `v26.x.y` line; never report stderr.
pub fn parse_node_version(stdout: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(stdout)
        .map_err(|_| "This development build requires Node version 26")?;
    let line = text.strip_suffix('\n').unwrap_or(text);
    let Some(rest) = line.strip_prefix("v26.") else {
        return Err("This development build requires Node version 26".into());
    };
    let mut parts = rest.split('.');
    let (Some(minor), Some(patch), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err("This development build requires Node version 26".into());
    };
    let bounded = |part: &str| {
        !part.is_empty() && part.len() <= 4 && part.bytes().all(|byte| byte.is_ascii_digit())
    };
    if !bounded(minor) || !bounded(patch) {
        return Err("This development build requires Node version 26".into());
    }
    Ok(line.to_string())
}

/// The default is fixed; an explicit path must resolve to a regular executable.
pub fn selected_node(value: Option<&str>) -> Result<PathBuf, String> {
    let Some(value) = value else {
        return Ok(PathBuf::from("/usr/bin/node"));
    };
    let node = absolute(value)?
        .canonicalize()
        .map_err(|_| "Selected Node is not an executable regular file".to_string())?;
    absolute(&node.to_string_lossy())
        .map_err(|_| "Selected Node is not an executable regular file".to_string())?;
    let info = std::fs::metadata(&node)
        .map_err(|_| "Selected Node is not an executable regular file".to_string())?;
    use std::os::unix::fs::PermissionsExt;
    if !info.is_file() || info.permissions().mode() & 0o111 == 0 {
        return Err("Selected Node is not an executable regular file".into());
    }
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicit_absolute_directories_are_accepted() {
        assert!(absolute("/tmp/public").is_ok());
        assert!(absolute("relative/path").is_err());
        assert!(
            absolute("/tmp/../etc").is_err(),
            "a parent component is refused"
        );
        assert!(
            absolute("/tmp/\u{7f}").is_err(),
            "a control character is refused"
        );
        let long = format!("/{}", "x".repeat(5000));
        assert!(absolute(&long).is_err(), "an over-long value is refused");
    }

    #[test]
    fn overlap_means_one_directory_contains_the_other() {
        let a = PathBuf::from("/tmp/a/b");
        assert!(overlaps(&a, &PathBuf::from("/tmp/a")));
        assert!(overlaps(&PathBuf::from("/tmp/a"), &a));
        assert!(overlaps(&a, &a));
        assert!(!overlaps(&a, &PathBuf::from("/tmp/a/c")));
        assert!(!overlaps(&a, &PathBuf::from("/tmp/other")));
        // A prefix that is not a component boundary is not containment.
        assert!(!overlaps(
            &PathBuf::from("/tmp/ab"),
            &PathBuf::from("/tmp/a")
        ));
    }

    #[test]
    fn the_environment_keeps_only_the_allow_list() {
        let inherited = BTreeMap::from([
            ("WAYLAND_DISPLAY".to_string(), "wayland-1".to_string()),
            ("XDG_RUNTIME_DIR".to_string(), "/run/user/1000".to_string()),
            ("OPENAI_API_KEY".to_string(), "secret".to_string()),
            ("DSH_HOME".to_string(), "/tmp/dsh".to_string()),
        ]);
        let env = environment(Path::new("/tmp/user"), &inherited, true).expect("wayland");
        assert_eq!(env["HOME"], "/tmp/user");
        assert_eq!(env["PATH"], "/usr/bin:/bin");
        assert_eq!(env["WAYLAND_DISPLAY"], "wayland-1");
        assert_eq!(env["DSH_HOME"], "/tmp/dsh");
        assert!(
            !env.contains_key("OPENAI_API_KEY"),
            "a secret is not inherited"
        );
        // Without a display, a display-requiring package is refused.
        assert!(environment(Path::new("/tmp/user"), &BTreeMap::new(), true).is_err());
        // A relative DSH_HOME is refused rather than silently dropped.
        let relative = BTreeMap::from([("DSH_HOME".to_string(), "relative".to_string())]);
        assert!(environment(Path::new("/tmp/user"), &relative, false).is_err());
    }

    #[test]
    fn a_node_version_is_one_bounded_v26_line() {
        assert_eq!(parse_node_version(b"v26.1.2\n").unwrap(), "v26.1.2");
        assert_eq!(parse_node_version(b"v26.10.0").unwrap(), "v26.10.0");
        for bad in [
            &b"v25.1.2\n"[..],
            b"v26\n",
            b"v26.1.2.3\n",
            b"v26.a.2\n",
            b"v26.12345.1\n",
            b"v26.1.2\nextra\n",
            b"\xff",
        ] {
            assert!(parse_node_version(bad).is_err(), "{bad:?} was accepted");
        }
    }

    #[test]
    fn a_marker_carries_the_package_identity() {
        let manifest = serde_json::json!({"nativeSha256": "a".repeat(64)});
        let marker = marker_for(&manifest);
        assert_eq!(marker["application"], APP);
        assert_eq!(marker["layout"], 1);
        assert_eq!(marker["runtimeVersion"], VERSION);
        assert_eq!(marker["nativeSha256"], "a".repeat(64));
    }
}
