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

/// Read a bounded package file without following a final symlink.
pub fn private_read(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| "Unexpected package file")?;
    // Safe: a NUL-terminated path and constant flags; the descriptor is closed below.
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err("Unexpected package file".into());
    }
    let mut info: libc::stat = unsafe { std::mem::zeroed() };
    // Safe: a live descriptor and a valid out-parameter.
    if unsafe { libc::fstat(fd, &mut info) } != 0
        || info.st_mode & libc::S_IFMT != libc::S_IFREG
        || info.st_size as u64 > limit
    {
        unsafe { libc::close(fd) };
        return Err("Unexpected package file".into());
    }
    use std::os::unix::io::FromRawFd;
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    let mut data = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut data).map_err(|error| error.to_string())?;
    if data.len() as u64 > limit || data.len() as u64 != info.st_size as u64 {
        return Err("Package file changed or exceeded its bound".into());
    }
    Ok(data)
}

/// Stream the native binary under a bound and reject a replaced or modified file.
pub fn binary_digest(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| "Unexpected native binary")?;
    // Safe: a NUL-terminated path and constant flags; the descriptor is closed below.
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err("Unexpected native binary".into());
    }
    let mut before: libc::stat = unsafe { std::mem::zeroed() };
    // Safe: a live descriptor and a valid out-parameter.
    if unsafe { libc::fstat(fd, &mut before) } != 0
        || before.st_mode & libc::S_IFMT != libc::S_IFREG
        || before.st_size <= 0
        || before.st_size as u64 > MAX_BINARY
    {
        unsafe { libc::close(fd) };
        return Err("Unexpected native binary".into());
    }
    use std::os::unix::io::FromRawFd;
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read =
            std::io::Read::read(&mut file, &mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        count += read as u64;
        if count > MAX_BINARY {
            return Err("Native binary exceeded its bound".into());
        }
        digest.update(&buffer[..read]);
    }
    let mut after: libc::stat = unsafe { std::mem::zeroed() };
    // Safe: a live descriptor and a valid out-parameter.
    if unsafe { libc::fstat(fd, &mut after) } != 0
        || count != before.st_size as u64
        || (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_mtime,
            before.st_mtime_nsec,
            before.st_ctime,
            before.st_ctime_nsec,
        ) != (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime,
            after.st_mtime_nsec,
            after.st_ctime,
            after.st_ctime_nsec,
        )
    {
        return Err("Native binary changed during validation".into());
    }
    Ok(format!("{:x}", digest.finalize()))
}

/// The running glibc as a comparable version tuple.
fn libc_version() -> Option<Vec<u64>> {
    // Safe: the returned pointer is a static NUL-terminated string for the process.
    let raw = unsafe { libc::gnu_get_libc_version() };
    if raw.is_null() {
        return None;
    }
    // Safe: the pointer is a valid NUL-terminated string owned by libc.
    let text = unsafe { std::ffi::CStr::from_ptr(raw) }.to_str().ok()?;
    parse_version(text)
}

fn parse_version(text: &str) -> Option<Vec<u64>> {
    text.split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<u64>>>()
}

/// Validate the package manifest and return it with the native binary path.
pub fn package(root: &Path) -> Result<(serde_json::Value, PathBuf), String> {
    let data = private_read(&root.join("development-manifest.json"), 65536)?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&data).map_err(|_| "Unsupported development package")?;
    if manifest["application"].as_str() != Some(APP)
        || manifest["runtimeVersion"].as_str() != Some(VERSION)
        || std::env::consts::OS != "linux"
        || std::env::consts::ARCH != "x86_64"
        || manifest["architecture"].as_str() != Some("x86_64")
        || manifest["runtimeMode"].as_str() != Some("external-explicit")
    {
        return Err("Unsupported development package".into());
    }
    let Some(minimum) = manifest["minimumGlibc"].as_str().and_then(parse_version) else {
        return Err("System glibc is older than this development build".into());
    };
    match libc_version() {
        Some(actual) if actual >= minimum => {}
        _ => return Err("System glibc is older than this development build".into()),
    }
    let Some(digest) = manifest["nativeSha256"].as_str() else {
        return Err("Invalid package identity".into());
    };
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("Invalid package identity".into());
    }
    let binary = root.join("bin/dsh-native-app");
    if binary_digest(&binary)? != digest {
        return Err("Native binary does not match package manifest".into());
    }
    use std::os::unix::fs::PermissionsExt;
    let executable = std::fs::metadata(&binary)
        .map(|info| info.permissions().mode() & 0o111 != 0)
        .unwrap_or(false);
    if !executable {
        return Err("Native binary is not executable".into());
    }
    Ok((manifest, binary))
}

/// Run `<node> --version` under a five-second bound and parse its single line.
pub fn probe_node(env: &BTreeMap<String, String>, node: Option<&Path>) -> Result<String, String> {
    use std::process::{Command, Stdio};
    let node = node
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/usr/bin/node"));
    let mut child = Command::new(&node)
        .arg("--version")
        .env_clear()
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "This development build requires Node version 26")?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                return Err("This development build requires Node version 26".into());
            }
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|_| "This development build requires Node version 26")?;
    if output.stdout.len() > 32 || output.stderr.len() > 1024 {
        return Err("This development build requires Node version 26".into());
    }
    parse_node_version(&output.stdout)
}

/// Whether a native data directory is new or an owned one we may reuse.
#[derive(Debug, PartialEq, Eq)]
pub enum State {
    New,
    Owned,
}

fn owned_directory(path: &Path) -> Result<bool, String> {
    use std::os::unix::fs::MetadataExt;
    let info =
        std::fs::symlink_metadata(path).map_err(|_| "Native data parent must already exist")?;
    Ok(info.is_dir() && info.uid() == unsafe { libc::geteuid() } && info.mode() & 0o777 == 0o700)
}

/// Refuse any directory or marker this launcher did not create.
pub fn inspect_state(data: &Path, marker: &serde_json::Value) -> Result<State, String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let info = match std::fs::symlink_metadata(data) {
        Ok(info) => info,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = data
                .parent()
                .ok_or("Native data parent must already exist")?;
            if !parent.is_dir() {
                return Err("Native data parent must already exist".into());
            }
            return Ok(State::New);
        }
        Err(_) => return Err("Existing native data directory is not private and owned".into()),
    };
    if !info.is_dir()
        || info.uid() != unsafe { libc::geteuid() }
        || info.permissions().mode() & 0o777 != 0o700
    {
        return Err("Existing native data directory is not private and owned".into());
    }
    let marker_path = data.join(".native-development.json");
    let marker_info = std::fs::symlink_metadata(&marker_path)
        .map_err(|_| "Refusing to adopt or migrate an unknown native data directory")?;
    let recorded = private_read(&marker_path, 1024)
        .ok()
        .and_then(|body| serde_json::from_slice::<serde_json::Value>(&body).ok());
    if !marker_info.is_file()
        || marker_info.uid() != unsafe { libc::geteuid() }
        || marker_info.permissions().mode() & 0o777 != 0o600
        || recorded.as_ref() != Some(marker)
    {
        return Err("Refusing to adopt or migrate an unknown native data directory".into());
    }
    let host = data.join("host");
    if std::fs::symlink_metadata(&host).is_ok() && !owned_directory(&host)? {
        return Err("Native Host directory is not private and owned".into());
    }
    Ok(State::Owned)
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

#[cfg(test)]
mod package_tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn fixture() -> (PathBuf, serde_json::Value) {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "launcher-").unwrap();
        let bin = base.as_path().join("bin/dsh-native-app");
        fs::create_dir_all(bin.parent().unwrap()).unwrap();
        fs::write(&bin, b"PUBLIC native binary").unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        let digest = binary_digest(&bin).unwrap();
        let manifest = serde_json::json!({
            "application": APP, "runtimeVersion": VERSION, "architecture": "x86_64",
            "runtimeMode": "external-explicit", "minimumGlibc": "2.0",
            "nativeSha256": digest,
        });
        fs::write(
            base.as_path().join("development-manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        fs::set_permissions(
            base.as_path().join("development-manifest.json"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        (base, manifest)
    }

    #[test]
    fn a_matching_manifest_and_binary_are_accepted_and_mismatches_refused() {
        let (base, _) = fixture();
        let (manifest, binary) = package(base.as_path()).expect("a valid package");
        assert_eq!(manifest["application"], APP);
        assert_eq!(binary, base.as_path().join("bin/dsh-native-app"));
        // A binary that does not match the manifest digest is refused.
        fs::write(binary.as_path(), b"tampered").unwrap();
        assert_eq!(
            package(base.as_path()).unwrap_err(),
            "Native binary does not match package manifest"
        );
        // A non-executable binary is refused.
        let (base, _) = fixture();
        fs::set_permissions(
            base.as_path().join("bin/dsh-native-app"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert_eq!(
            package(base.as_path()).unwrap_err(),
            "Native binary is not executable"
        );
        // A wrong application, a malformed digest and an unsupported mode are refused.
        for (key, value, expected) in [
            (
                "application",
                serde_json::json!("other.app"),
                "Unsupported development package",
            ),
            (
                "runtimeMode",
                serde_json::json!("installed"),
                "Unsupported development package",
            ),
            (
                "nativeSha256",
                serde_json::json!("XYZ"),
                "Invalid package identity",
            ),
        ] {
            let (base, mut manifest) = fixture();
            manifest[key] = value;
            fs::write(
                base.as_path().join("development-manifest.json"),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            assert_eq!(package(base.as_path()).unwrap_err(), expected, "{key}");
        }
        // private_read refuses a symlinked manifest.
        let (base, _) = fixture();
        let manifest = base.as_path().join("development-manifest.json");
        let elsewhere = base.as_path().join("elsewhere.json");
        fs::rename(&manifest, &elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &manifest).unwrap();
        assert!(private_read(&manifest, 65536).is_err());
    }
}

#[cfg(test)]
mod state_tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn marker() -> serde_json::Value {
        serde_json::json!({"application": APP, "layout": 1, "runtimeVersion": VERSION, "nativeSha256": "a".repeat(64)})
    }

    fn data_dir() -> PathBuf {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "launcher-state-").unwrap();
        let data = base.join("native-container");
        fs::create_dir(&data).unwrap();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o700)).unwrap();
        let marker_path = data.join(".native-development.json");
        fs::write(&marker_path, serde_json::to_vec(&marker()).unwrap()).unwrap();
        fs::set_permissions(&marker_path, fs::Permissions::from_mode(0o600)).unwrap();
        data
    }

    #[test]
    fn a_new_directory_is_created_and_an_owned_one_is_reused() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "launcher-new-").unwrap();
        assert_eq!(
            inspect_state(&base.join("native-container"), &marker()).unwrap(),
            State::New
        );
        let data = data_dir();
        assert_eq!(inspect_state(&data, &marker()).unwrap(), State::Owned);
        // A host directory must keep the same ownership rules.
        fs::create_dir(data.join("host")).unwrap();
        fs::set_permissions(data.join("host"), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(inspect_state(&data, &marker()).is_err());
        // A public data directory, a wrong marker and an unknown marker are refused.
        let data = data_dir();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(inspect_state(&data, &marker()).is_err());
        let data = data_dir();
        fs::write(
            data.join(".native-development.json"),
            b"{\"application\":\"other\"}",
        )
        .unwrap();
        assert!(inspect_state(&data, &marker()).is_err());
        // A missing parent is refused rather than creating a tree.
        assert!(inspect_state(&data.join("gone/child"), &marker()).is_err());
    }
}
