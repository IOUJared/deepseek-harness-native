//! Development package launcher: path, environment and Node contract.
//!
//! First stage of the port of `scripts/development-launcher.py` — the only Python
//! file copied into the release. These pieces are pure and testable without a GUI:
//! absolute-path refusal, package overlap, the environment allow-list, the state
//! marker and the Node 26 version line.
use serde_json::Value;
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

/// A held data-directory lock; dropping it releases the descriptor.
pub struct Held {
    pub lock: std::fs::File,
}

fn openat(dir: i32, name: &str, flags: i32, mode: u32) -> Result<i32, String> {
    let c_name = std::ffi::CString::new(name).map_err(|_| "Unexpected native data entry")?;
    // Safe: a live descriptor, a NUL-terminated name and constant flags.
    let fd = unsafe { libc::openat(dir, c_name.as_ptr(), flags | libc::O_CLOEXEC, mode) };
    if fd < 0 {
        return Err("Unexpected native data entry".into());
    }
    Ok(fd)
}

fn stat_fd(fd: i32) -> Result<libc::stat, String> {
    let mut info: libc::stat = unsafe { std::mem::zeroed() };
    // Safe: a live descriptor and a valid out-parameter.
    if unsafe { libc::fstat(fd, &mut info) } != 0 {
        return Err("Unexpected native data entry".into());
    }
    Ok(info)
}

fn private_entry(info: &libc::stat, directory: bool) -> bool {
    let kind_ok = if directory {
        info.st_mode & libc::S_IFMT == libc::S_IFDIR
    } else {
        info.st_mode & libc::S_IFMT == libc::S_IFREG
    };
    kind_ok
        && info.st_uid == unsafe { libc::geteuid() }
        && info.st_mode & 0o777 == if directory { 0o700 } else { 0o600 }
}

/// Create or adopt the data directory and hold its exclusive lock.
pub fn acquire_state(data: &Path, marker: &serde_json::Value) -> Result<Held, String> {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::io::FromRawFd;
    let status = inspect_state(data, marker)?;
    if status == State::New {
        std::fs::create_dir(data).map_err(|_| "Native data directory could not be created")?;
        let _ = std::fs::set_permissions(data, std::fs::Permissions::from_mode(0o700));
    }
    let c_data = std::ffi::CString::new(data.to_string_lossy().as_bytes())
        .map_err(|_| "Unexpected native data entry")?;
    // Safe: a NUL-terminated path and constant flags; closed below.
    let directory = unsafe {
        libc::open(
            c_data.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if directory < 0 {
        return Err("Native data directory ownership changed".into());
    }
    let result = (|| -> Result<Held, String> {
        if !private_entry(&stat_fd(directory)?, true) {
            return Err("Native data directory ownership changed".into());
        }
        if status == State::New {
            let fd = openat(
                directory,
                ".native-development.json",
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW,
                0o600,
            )?;
            let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
            let body = format!(
                "{}\n",
                serde_json::to_string(marker).map_err(|_| "Marker write failed")?
            );
            file.write_all(body.as_bytes())
                .map_err(|_| "Native marker write incomplete")?;
            file.sync_all()
                .map_err(|_| "Native marker write incomplete")?;
            // Safe: a live directory descriptor.
            unsafe { libc::fsync(directory) };
        }
        // Read the held directory, not a replaced pathname, before taking its lock.
        let marker_fd = openat(
            directory,
            ".native-development.json",
            libc::O_RDONLY | libc::O_NOFOLLOW,
            0,
        )?;
        let recorded = {
            use std::io::Read;
            let mut file = unsafe { std::fs::File::from_raw_fd(marker_fd) };
            let mut body = Vec::new();
            file.read_to_end(&mut body)
                .map_err(|_| "Native marker changed")?;
            if body.len() > 1024 {
                return Err("Native marker changed".into());
            }
            serde_json::from_slice::<serde_json::Value>(&body)
                .map_err(|_| "Native marker changed")?
        };
        if &recorded != marker {
            return Err("Native marker changed".into());
        }
        let lock_fd = openat(
            directory,
            ".native-development.lock",
            libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW,
            0o600,
        )?;
        let lock = unsafe { std::fs::File::from_raw_fd(lock_fd) };
        // Safe: a live descriptor; the lock is exclusive and non-blocking.
        if unsafe { libc::flock(lock_fd, libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("This native data directory is already in use".into());
        }
        // Safe: the descriptor is held for the lifetime of `lock`.
        if !private_entry(&stat_fd(lock_fd)?, false) {
            return Err("Native lock is not private and owned".into());
        }
        let c_host = std::ffi::CString::new("host").unwrap();
        // Safe: a live directory descriptor and a constant name.
        unsafe { libc::mkdirat(directory, c_host.as_ptr(), 0o700) };
        let host_fd = openat(
            directory,
            "host",
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
            0,
        )?;
        let host_ok = private_entry(&stat_fd(host_fd)?, true);
        // Safe: each descriptor is closed exactly once here.
        unsafe { libc::close(host_fd) };
        if !host_ok {
            return Err("Native Host directory ownership changed".into());
        }
        let current = std::fs::symlink_metadata(data)
            .map_err(|_| "Native data pathname changed during acquisition")?;
        let pinned = stat_fd(directory)?;
        use std::os::unix::fs::MetadataExt;
        if !current.is_dir() || current.dev() != pinned.st_dev || current.ino() != pinned.st_ino {
            return Err("Native data pathname changed during acquisition".into());
        }
        inspect_state(data, marker)?;
        // Safe: a live directory descriptor.
        unsafe { libc::fsync(directory) };
        Ok(Held { lock })
    })();
    // Safe: the directory descriptor is no longer needed.
    unsafe { libc::close(directory) };
    result
}

/// Parsed launcher options, mirroring the shipped command line.
#[derive(Default)]
pub struct Options {
    pub workspace: String,
    pub runtime: Option<String>,
    pub user_home: Option<String>,
    pub data_home: Option<String>,
    pub node: Option<String>,
    pub scale: f64,
    pub check: bool,
    pub qualify_evidence: Option<String>,
}

#[derive(Debug)]
pub struct Plan {
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub data: PathBuf,
    pub marker: serde_json::Value,
    pub state: State,
    pub node_version: String,
    pub node_mode: &'static str,
}

fn canonical_dir(path: &Path) -> Option<PathBuf> {
    let resolved = path.canonicalize().ok()?;
    resolved.is_dir().then_some(resolved)
}

fn empty_private(path: &Path) -> bool {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let Ok(info) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !info.is_dir()
        || info.uid() != unsafe { libc::geteuid() }
        || info.permissions().mode() & 0o777 != 0o700
    {
        return false;
    }
    std::fs::read_dir(path)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
}

/// Validate every prerequisite and produce the native argv and environment.
pub fn plan(
    options: &Options,
    root: &Path,
    inherited: &BTreeMap<String, String>,
) -> Result<Plan, String> {
    let (manifest, binary) = package(root)?;
    let home = options
        .user_home
        .clone()
        .or_else(|| inherited.get("HOME").cloned())
        .unwrap_or_default();
    let user = canonical_dir(&absolute(&home)?)
        .ok_or("User home and workspace must be existing directories")?;
    let workspace = canonical_dir(&absolute(&options.workspace)?)
        .ok_or("User home and workspace must be existing directories")?;
    let Some(runtime_arg) = options.runtime.as_deref() else {
        return Err("This package requires an explicit isolated alpha --runtime".into());
    };
    let runtime = canonical_dir(&absolute(runtime_arg)?)
        .ok_or("Only the explicit isolated alpha CLI runtime is supported")?;
    let runtime_manifest: serde_json::Value = private_read(&runtime.join("package.json"), 65536)
        .ok()
        .and_then(|body| serde_json::from_slice(&body).ok())
        .ok_or("Only the explicit isolated alpha CLI runtime is supported")?;
    if runtime_manifest["name"].as_str() != Some("@deepseek-ai/dsh")
        || runtime_manifest["version"].as_str() != Some(VERSION)
        || !runtime.join("lib/bin.js").is_file()
    {
        return Err("Only the explicit isolated alpha CLI runtime is supported".into());
    }
    let data_raw = match &options.data_home {
        Some(value) => absolute(value)?,
        None => user.join(".local/share").join(format!(
            "dsh-native-dev-{}",
            manifest["nativeSha256"]
                .as_str()
                .unwrap_or_default()
                .get(..12)
                .unwrap_or_default()
        )),
    };
    if std::fs::symlink_metadata(&data_raw).is_ok_and(|info| info.file_type().is_symlink()) {
        return Err("Native data directory must not be a symlink".into());
    }
    let data = data_raw.canonicalize().unwrap_or(data_raw.clone());
    let mut forbidden = vec![user.join(".dsh"), root.to_path_buf(), runtime.clone()];
    if let Some(home) = inherited.get("DSH_HOME") {
        forbidden.push(
            absolute(home)?
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(home)),
        );
    }
    let local_share = user.join(".local/share");
    if data == Path::new("/")
        || data == user
        || data == local_share
        || data == local_share.join("dsh")
        || forbidden.iter().any(|path| overlaps(&data, path))
        || user.starts_with(&data)
    {
        return Err("Native data overlaps protected installation or storage".into());
    }
    if !options.scale.is_finite() || !(0.75..=2.0).contains(&options.scale) {
        return Err("Scale must be between 0.75 and 2.0".into());
    }
    let marker = marker_for(&manifest);
    let state = inspect_state(&data, &marker)?;
    let env = environment(&user, inherited, !options.check)?;
    let node = selected_node(options.node.as_deref())?;
    let node_version = probe_node(&environment(&user, inherited, false)?, Some(&node))?;
    let mut argv = vec![
        binary.to_string_lossy().into_owned(),
        "--runtime".into(),
        runtime.to_string_lossy().into_owned(),
        "--expected-version".into(),
        VERSION.into(),
        "--home".into(),
        data.join("host").to_string_lossy().into_owned(),
        "--user-home".into(),
        user.to_string_lossy().into_owned(),
        "--cwd".into(),
        workspace.to_string_lossy().into_owned(),
        "--scale".into(),
        options.scale.to_string(),
    ];
    if options.node.is_some() {
        argv.push("--node".into());
        argv.push(node.to_string_lossy().into_owned());
    }
    if let Some(evidence_arg) = &options.qualify_evidence {
        if options.check
            || options.data_home.is_none()
            || options.user_home.is_none()
            || state != State::New
        {
            return Err(
                "Qualification requires explicit fresh data and user homes, not --check".into(),
            );
        }
        let evidence = canonical_dir(&absolute(evidence_arg)?)
            .ok_or("Qualification requires separate owned private empty evidence, workspace and user directories")?;
        if [&evidence, &workspace, &user]
            .iter()
            .any(|path| !empty_private(path))
            || overlaps(&evidence, &data)
            || [&workspace, &user].iter().any(|path| {
                std::fs::read_dir(path)
                    .map(|mut d| d.next().is_some())
                    .unwrap_or(true)
            })
        {
            return Err("Qualification requires separate owned private empty evidence, workspace and user directories".into());
        }
        argv.extend([
            "--smoke-new-session".into(),
            "--exit-after-seconds".into(),
            "8".into(),
            "--smoke-evidence-root".into(),
            evidence.to_string_lossy().into_owned(),
            "--smoke-report".into(),
            evidence.join("app.json").to_string_lossy().into_owned(),
            "--smoke-screenshot".into(),
            evidence
                .join("own-window.png")
                .to_string_lossy()
                .into_owned(),
        ]);
    }
    Ok(Plan {
        argv,
        env,
        data,
        marker,
        state,
        node_version,
        node_mode: if options.node.is_some() {
            "explicit-absolute"
        } else {
            "system-default"
        },
    })
}

fn refused() -> i32 {
    eprintln!(
        "Native development launch refused: invalid package, runtime, private data, display or Node prerequisites. No installation or migration was attempted."
    );
    2
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        scale: 1.0,
        ..Options::default()
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 0;
    while index < args.len() {
        let token = &args[index];
        let (name, inline) = match token.split_once('=') {
            Some((name, value)) => (name.to_string(), Some(value.to_string())),
            None => (token.clone(), None),
        };
        if name.starts_with("--") && !seen.insert(name.clone()) {
            return Err("Duplicate launcher option".into());
        }
        let mut value = || -> Result<String, String> {
            if let Some(value) = inline.clone() {
                return Ok(value);
            }
            index += 1;
            args.get(index)
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match name.as_str() {
            "--workspace" => options.workspace = value()?,
            "--runtime" => options.runtime = Some(value()?),
            "--user-home" => options.user_home = Some(value()?),
            "--data-home" => options.data_home = Some(value()?),
            "--node" => options.node = Some(value()?),
            "--qualify-evidence" => options.qualify_evidence = Some(value()?),
            "--scale" => {
                options.scale = value()?
                    .parse()
                    .map_err(|_| "Scale must be between 0.75 and 2.0".to_string())?
            }
            "--check" => options.check = true,
            other => return Err(format!("Unknown launcher option {other}")),
        }
        index += 1;
    }
    if options.workspace.is_empty() {
        return Err("--workspace is required".into());
    }
    Ok(options)
}

/// The shipped launcher entry point: validate, optionally report, then exec.
pub fn main(args: &[String], root: Option<&Path>) -> i32 {
    let Ok(options) = parse_options(args) else {
        return refused();
    };
    let root = match root {
        Some(root) => root.to_path_buf(),
        None => match std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
        {
            Some(root) => root,
            None => return refused(),
        },
    };
    let inherited: BTreeMap<String, String> = std::env::vars().collect();
    let Ok(plan) = plan(&options, &root, &inherited) else {
        return refused();
    };
    if options.check {
        let manifest_mode = package(&root)
            .ok()
            .and_then(|(manifest, _)| manifest["runtimeMode"].as_str().map(str::to_string))
            .unwrap_or_default();
        println!(
            "{}",
            serde_json::json!({
                "status": "prerequisites-valid",
                "runtimeVersion": VERSION,
                "nativeDataStatus": if plan.state == State::New { "new" } else { "owned" },
                "runtimeDependencyResolutionValidated": false,
                "runtimeStartupValidated": false,
                "guiStarted": false,
                "hostStarted": false,
                "nativeDataCreated": false,
                "runtimeMode": manifest_mode,
                "nodeVersionObserved": plan.node_version,
                "nodeMajorAccepted": 26,
                "nodeSelectionMode": plan.node_mode,
            })
        );
        return 0;
    }
    let held = match acquire_state(&plan.data, &plan.marker) {
        Ok(held) => held,
        Err(_) => return refused(),
    };
    // The lock must survive the exec so the native process owns the directory.
    use std::os::unix::io::AsRawFd;
    // Safe: a live descriptor and a constant command.
    unsafe { libc::fcntl(held.lock.as_raw_fd(), libc::F_SETFD, 0) };
    use std::os::unix::process::CommandExt;
    let error = std::process::Command::new(&plan.argv[0])
        .args(&plan.argv[1..])
        .env_clear()
        .envs(&plan.env)
        .exec();
    eprintln!("Native development launch failed: {error}");
    2
}

/// Copy a regular, non-symlinked source under a bound and refuse one that changes.
pub fn copy_new(source: &Path, target: &Path, mode: u32, limit: u64) -> Result<(), String> {
    use std::io::{Read, Write};
    use std::os::unix::fs::MetadataExt;
    let before = std::fs::symlink_metadata(source).map_err(|_| "Unexpected package source file")?;
    if before.file_type().is_symlink() {
        return Err("Source symlinks are not packaged".into());
    }
    if !before.file_type().is_file() || before.size() > limit {
        return Err("Unexpected package source file".into());
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut reader = std::fs::File::open(source).map_err(|error| error.to_string())?;
    let mut writer = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|_| "Package target already exists".to_string())?;
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        writer
            .write_all(&buffer[..read])
            .map_err(|error| error.to_string())?;
    }
    writer.sync_all().map_err(|error| error.to_string())?;
    let after =
        std::fs::symlink_metadata(source).map_err(|_| "Package source changed during snapshot")?;
    if (
        before.dev(),
        before.ino(),
        before.size(),
        before.mtime(),
        before.mtime_nsec(),
        before.ctime(),
        before.ctime_nsec(),
    ) != (
        after.dev(),
        after.ino(),
        after.size(),
        after.mtime(),
        after.mtime_nsec(),
        after.ctime(),
        after.ctime_nsec(),
    ) {
        return Err("Package source changed during snapshot".into());
    }
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(target, std::fs::Permissions::from_mode(mode))
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn node_major_line(value: &Value) -> bool {
    let Some(text) = value.as_str() else {
        return true;
    };
    crate::launcher::parse_node_version(text.as_bytes()).is_ok()
}

/// Check the selected PUBLIC UI qualification; not signatures or all backend source.
pub fn qualified_native(
    path: &Path,
    expected_digest: &str,
    app_root: &Path,
) -> Result<Value, String> {
    if app_root
        .symlink_metadata()
        .is_ok_and(|info| info.file_type().is_symlink())
    {
        return Err("Native app root symlink refused".into());
    }
    let body = crate::launcher::private_read(path, 256 * 1024)
        .map_err(|_| "Selected native qualification does not match this release")?;
    let value: Value = serde_json::from_slice(&body)
        .map_err(|_| "Selected native qualification does not match this release")?;
    let tests_ok = [
        "defaultAppTestsPassed",
        "featureAppTestsPassed",
        "fixtureExampleTestsPassed",
    ]
    .iter()
    .all(|field| value[*field].as_i64().is_some_and(|count| count > 0));
    let cases_ok = value["publicCases"].as_array().is_some_and(|cases| {
        !cases.is_empty()
            && cases.len() <= 128
            && cases.iter().all(|case| {
                case["passed"].as_bool() == Some(true) && case["exitCode"].as_i64() == Some(0)
            })
    }) && value["realKeyless"]["passed"].as_bool() == Some(true)
        && value["realKeyless"]["exitCode"].as_i64() == Some(0);
    let sources_ok = value["sourceSha256"].as_object().is_some_and(|sources| {
        !sources.is_empty() && sources.len() <= 512 && sources.contains_key("src/ui.rs")
    });
    let format_ok = if value["formatCheckPassed"].as_bool() == Some(true) {
        true
    } else {
        value["formatCheckPassed"].as_bool() == Some(false)
            && value["scopedFormatCheck"]["passed"].as_bool() == Some(true)
            && value["scopedFormatCheck"]["scope"].as_str() == Some("explicit-app-files")
            && value["scopedFormatCheck"]["exitCode"].as_i64() == Some(0)
    };
    if value["scope"].as_str() != Some("native-control-layout-and-Harness-style-conversation")
        || value["defaultReleaseSha256"].as_str() != Some(expected_digest)
        || value["parentPaintInspectionComplete"].as_bool() != Some(true)
        || value["checkAllTargetsFeaturePassed"].as_bool() != Some(true)
        || !tests_ok
        || !cases_ok
        || !node_major_line(&value["systemNodeVersionObserved"])
        || !sources_ok
        || !format_ok
    {
        return Err("Selected native qualification does not match this release".into());
    }
    Ok(value)
}

pub const NATIVE_MODULES: [&str; 3] = ["app", "core", "transport"];
pub const NATIVE_FOLDERS: [&str; 5] = ["src", "runtime", "tests", "examples", "fixtures"];
pub const MAX_NATIVE_SOURCE_FILES: usize = 4096;
pub const MAX_NATIVE_SOURCE_ENTRIES: usize = 8192;
pub const MAX_NATIVE_SOURCE_BYTES: u64 = 128 * 1024 * 1024;

/// The dedicated App vendor tree carries its own notices and provenance.
pub fn native_folders(module: &str) -> Vec<&'static str> {
    let mut folders = NATIVE_FOLDERS.to_vec();
    if module == "app" {
        folders.push("vendor");
    }
    folders
}

/// Require a canonical, owner, non-symlinked native input path.
pub fn native_path(value: &Path, directory: bool) -> Result<PathBuf, String> {
    use std::os::unix::fs::MetadataExt;
    let raw = value.to_string_lossy();
    if raw.len() > 4096 || raw.chars().any(|c| (c as u32) < 0x20 || c == '\u{7f}') {
        return Err("Invalid native input path".into());
    }
    if !value.is_absolute()
        || value
            .components()
            .any(|part| part == std::path::Component::ParentDir)
    {
        return Err("Native inputs require absolute paths without parents".into());
    }
    let mut current = Some(value);
    while let Some(item) = current {
        if item
            .symlink_metadata()
            .is_ok_and(|info| info.file_type().is_symlink())
        {
            return Err("Native input symlink refused".into());
        }
        current = item.parent();
    }
    let canonical = value.canonicalize().map_err(|_| {
        "Native input must be canonical, own and the expected file type".to_string()
    })?;
    let info = canonical.symlink_metadata().map_err(|_| {
        "Native input must be canonical, own and the expected file type".to_string()
    })?;
    if raw.as_ref() != canonical.to_string_lossy()
        || info.uid() != unsafe { libc::geteuid() }
        || info.is_dir() != directory
    {
        return Err("Native input must be canonical, own and the expected file type".into());
    }
    Ok(canonical)
}

/// The source root and native binary must be supplied together or both by default.
pub fn native_inputs(
    base: &Path,
    binary: &Path,
    source_root: Option<&Path>,
    native_binary: Option<&Path>,
) -> Result<(PathBuf, PathBuf), String> {
    if source_root.is_some() != native_binary.is_some() {
        return Err("Source root and native binary must be supplied together".into());
    }
    let base = native_path(source_root.unwrap_or(base), true)?;
    let binary = native_path(native_binary.unwrap_or(binary), false)?;
    for module in NATIVE_MODULES {
        let root = native_path(&base.join(module), true)?;
        for name in ["Cargo.toml", "Cargo.lock"] {
            native_path(&root.join(name), false)?;
        }
        native_path(&root.join("src"), true)?;
    }
    Ok((base, binary))
}

fn walk(directory: &Path, paths: &mut Vec<PathBuf>, entries: &mut usize) -> Result<(), String> {
    for entry in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        *entries += 1;
        if *entries > MAX_NATIVE_SOURCE_ENTRIES {
            return Err("Native source entry count exceeded bound".into());
        }
        let path = entry.path();
        let info = entry.metadata().map_err(|error| error.to_string())?;
        if info.is_dir() {
            walk(&path, paths, entries)?;
        } else {
            paths.push(path);
        }
    }
    Ok(())
}

/// Single bounded source selection, deterministic by path.
pub fn native_source_files(base: &Path) -> Result<Vec<(PathBuf, u64)>, String> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut entries = 0usize;
    for module in NATIVE_MODULES {
        let root = base.join(module);
        if root
            .symlink_metadata()
            .is_ok_and(|info| info.file_type().is_symlink())
        {
            return Err("Native module root symlink refused".into());
        }
        for name in ["Cargo.toml", "Cargo.lock", "LICENSE"] {
            paths.push(root.join(name));
        }
        for folder in native_folders(module) {
            let directory = root.join(folder);
            if directory
                .symlink_metadata()
                .is_ok_and(|info| info.file_type().is_symlink())
            {
                return Err("Native source root symlink refused".into());
            }
            if directory.is_dir() {
                walk(&directory, &mut paths, &mut entries)?;
            }
        }
    }
    paths.sort();
    let mut chosen = Vec::new();
    for source in paths {
        if source
            .symlink_metadata()
            .is_ok_and(|info| info.file_type().is_symlink())
        {
            return Err("Native source symlink refused".into());
        }
        if !source.exists() || source.is_dir() {
            continue;
        }
        use std::os::unix::fs::MetadataExt;
        let info = source
            .symlink_metadata()
            .map_err(|error| error.to_string())?;
        let name = source
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let relative = source
            .strip_prefix(base)
            .unwrap_or(&source)
            .to_string_lossy()
            .into_owned();
        if !info.is_file()
            || info.uid() != unsafe { libc::geteuid() }
            || name.starts_with(".env")
            || name == ".credentials.yaml"
            || name == ".npmrc"
            || relative.chars().any(|c| (c as u32) < 0x20 || c == '\u{7f}')
        {
            return Err("Private or unexpected native source file refused".into());
        }
        let limit =
            if name.ends_with("Cargo.toml") || name.ends_with("Cargo.lock") || name == "LICENSE" {
                64 * 1024 * 1024
            } else {
                8 * 1024 * 1024
            };
        chosen.push((source, limit));
        if chosen.len() > MAX_NATIVE_SOURCE_FILES {
            return Err("Native source file count exceeded bound".into());
        }
    }
    Ok(chosen)
}

/// The packaged native source snapshot inventory.
pub fn native_source_inventory(base: &Path) -> Result<serde_json::Value, String> {
    let mut records = Vec::new();
    let mut total = 0u64;
    for (source, limit) in native_source_files(base)? {
        let data = crate::launcher::private_read(&source, limit)?;
        total += data.len() as u64;
        if total > MAX_NATIVE_SOURCE_BYTES {
            return Err("Native source bytes exceeded bound".into());
        }
        let relative = source
            .strip_prefix(base)
            .map_err(|_| "Native source outside its root")?;
        records.push(serde_json::json!({
            "path": format!("source/native/{}", relative.to_string_lossy()),
            "bytes": data.len(),
            "sha256": crate::sha256_file(&source).map_err(|error| error.to_string())?,
        }));
    }
    records.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    Ok(serde_json::json!({
        "scope": "Packaged native source snapshot; not an exact reproducible-build or complete third-party source certificate",
        "fileCount": records.len(),
        "files": records,
    }))
}

fn package_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn package_version_ok(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 100
        && version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'_' | b'-'))
}

fn license_file_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    for prefix in ["LICENSE", "COPYING", "NOTICE", "COPYRIGHT"] {
        if let Some(rest) = upper.strip_prefix(prefix)
            && (rest.is_empty() || rest.starts_with(['.', '_', '-']))
        {
            return true;
        }
    }
    false
}

/// Copy the license texts of every registry package and write the inventory.
pub fn licenses(metadata: &Value, cargo_src: &Path, payload: &Path) -> Result<Value, String> {
    let Some(packages) = metadata["packages"].as_array() else {
        return Err("Invalid bounded Cargo metadata package list".into());
    };
    if packages.is_empty() || packages.len() > 1024 {
        return Err("Invalid bounded Cargo metadata package list".into());
    }
    let cargo_src = cargo_src
        .canonicalize()
        .map_err(|_| "Cargo license source is not the expected cached crate")?;
    for package in packages {
        if !package.is_object() {
            return Err("Invalid Cargo package metadata".into());
        }
        if package["source"].is_null() {
            continue;
        }
        let (Some(source), Some(name), Some(version)) = (
            package["source"].as_str(),
            package["name"].as_str(),
            package["version"].as_str(),
        ) else {
            return Err("Unsupported Cargo package identity".into());
        };
        if !source.starts_with("registry+")
            || !package_name_ok(name)
            || !package_version_ok(version)
        {
            return Err("Unsupported Cargo package identity".into());
        }
        let Some(manifest) = package["manifest_path"].as_str().map(Path::new) else {
            return Err("Cargo license source is not the expected cached crate".into());
        };
        let manifest = manifest
            .canonicalize()
            .map_err(|_| "Cargo license source is not the expected cached crate")?;
        let parent = manifest
            .parent()
            .ok_or("Cargo license source is not the expected cached crate")?;
        let relative = parent
            .strip_prefix(&cargo_src)
            .map_err(|_| "Cargo license source is not the expected cached crate")?;
        if manifest
            .file_name()
            .is_some_and(|name| name != "Cargo.toml")
            || relative.components().count() != 2
            || parent
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                != Some(format!("{name}-{version}"))
        {
            return Err("Cargo license source is not the expected cached crate".into());
        }
    }
    let mut ordered: Vec<&Value> = packages
        .iter()
        .filter(|package| !package["source"].is_null())
        .collect();
    ordered.sort_by(|a, b| {
        (a["name"].as_str(), a["version"].as_str())
            .cmp(&(b["name"].as_str(), b["version"].as_str()))
    });
    let mut records = Vec::new();
    for package in ordered {
        let name = package["name"].as_str().unwrap_or_default();
        let version = package["version"].as_str().unwrap_or_default();
        let root = Path::new(package["manifest_path"].as_str().unwrap_or_default())
            .canonicalize()
            .map_err(|_| "Cargo license source is not the expected cached crate")?
            .parent()
            .ok_or("Cargo license source is not the expected cached crate")?
            .to_path_buf();
        let mut chosen: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
        for entry in std::fs::read_dir(&root).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if path.is_file() && license_file_name(&file_name) {
                chosen.insert(path);
            }
        }
        if let Some(explicit) = package["license_file"].as_str() {
            let explicit = PathBuf::from(explicit);
            let explicit = if explicit.is_absolute() {
                explicit
            } else {
                root.join(explicit)
            };
            if let Ok(resolved) = explicit.canonicalize()
                && resolved.starts_with(&root)
            {
                chosen.insert(resolved);
            }
        }
        let mut copied = Vec::new();
        for path in chosen {
            if path
                .symlink_metadata()
                .is_ok_and(|info| info.file_type().is_symlink())
                || !path.is_file()
                || path
                    .metadata()
                    .map(|info| info.len() > 1024 * 1024)
                    .unwrap_or(true)
            {
                continue;
            }
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let relative = Path::new("licenses/rust")
                .join(format!("{name}-{version}"))
                .join(&file_name);
            copy_new(&path, &payload.join(&relative), 0o644, 1024 * 1024)?;
            copied.push(relative.to_string_lossy().into_owned());
        }
        records.push(serde_json::json!({
            "name": name,
            "version": version,
            "declaredLicense": package["license"],
            "licenseTexts": copied,
            "source": format!("https://crates.io/crates/{name}/{version}"),
        }));
    }
    let with_texts = records
        .iter()
        .filter(|record| {
            record["licenseTexts"]
                .as_array()
                .is_some_and(|texts| !texts.is_empty())
        })
        .count();
    let inventory = serde_json::json!({
        "scope": "Cargo registry metadata lockfile/target superset; source=None path dependencies excluded (notices retained in selected native sources); not exact compiled-link closure or legal certification",
        "packageCount": records.len(),
        "packagesWithCopiedLicenseTexts": with_texts,
        "packages": records,
    });
    std::fs::create_dir_all(payload.join("licenses")).map_err(|error| error.to_string())?;
    std::fs::write(
        payload.join("licenses/rust-inventory.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&inventory).map_err(|error| error.to_string())?
        ),
    )
    .map_err(|error| error.to_string())?;
    Ok(inventory)
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

#[cfg(test)]
mod acquire_tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn acquiring_creates_the_marker_and_host_and_holds_an_exclusive_lock() {
        let base =
            crate::tempdir::make_temp_dir(&std::env::temp_dir(), "launcher-acquire-").unwrap();
        let data = base.join("native-container");
        let marker = serde_json::json!({"application": APP, "layout": 1, "runtimeVersion": VERSION,
                                        "nativeSha256": "a".repeat(64)});
        let held = acquire_state(&data, &marker).expect("a new directory is acquired");
        let marker_path = data.join(".native-development.json");
        let info = fs::symlink_metadata(&marker_path).unwrap();
        assert_eq!(info.permissions().mode() & 0o777, 0o600);
        let host = fs::symlink_metadata(data.join("host")).unwrap();
        assert_eq!(host.permissions().mode() & 0o777, 0o700);
        assert!(fs::symlink_metadata(data.join(".native-development.lock")).is_ok());
        // A second acquisition while the lock is held is refused.
        assert_eq!(
            acquire_state(&data, &marker).err().unwrap(),
            "This native data directory is already in use"
        );
        drop(held);
        // A different marker is refused rather than adopted.
        let other = serde_json::json!({"application": "other"});
        assert!(acquire_state(&data, &other).is_err());
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod plan_tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    struct Fixture {
        base: PathBuf,
        package: PathBuf,
        runtime: PathBuf,
        user: PathBuf,
        workspace: PathBuf,
        node: PathBuf,
    }

    fn fixture() -> Fixture {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "launcher-plan-").unwrap();
        let package = base.join("package");
        let bin = package.join("bin/dsh-native-app");
        fs::create_dir_all(bin.parent().unwrap()).unwrap();
        fs::write(&bin, b"PUBLIC native binary").unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        let digest = binary_digest(&bin).unwrap();
        let manifest = serde_json::json!({
            "application": APP, "runtimeVersion": VERSION, "architecture": "x86_64",
            "runtimeMode": "external-explicit", "minimumGlibc": "2.0", "nativeSha256": digest,
        });
        fs::write(
            package.join("development-manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        fs::set_permissions(
            package.join("development-manifest.json"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let runtime = base.join("runtime");
        fs::create_dir_all(runtime.join("lib")).unwrap();
        fs::write(
            runtime.join("package.json"),
            serde_json::to_vec(
                &serde_json::json!({"name": "@deepseek-ai/dsh", "version": VERSION}),
            )
            .unwrap(),
        )
        .unwrap();
        fs::set_permissions(
            runtime.join("package.json"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        fs::write(runtime.join("lib/bin.js"), b"// PUBLIC").unwrap();
        let user = base.join("user");
        let workspace = base.join("workspace");
        fs::create_dir(&user).unwrap();
        fs::create_dir(&workspace).unwrap();
        let node = base.join("node");
        fs::write(&node, b"#!/bin/sh\necho v26.1.2\n").unwrap();
        fs::set_permissions(&node, fs::Permissions::from_mode(0o755)).unwrap();
        Fixture {
            base,
            package,
            runtime,
            user,
            workspace,
            node,
        }
    }

    fn options(fixture: &Fixture) -> Options {
        Options {
            workspace: fixture.workspace.to_string_lossy().into_owned(),
            runtime: Some(fixture.runtime.to_string_lossy().into_owned()),
            user_home: Some(fixture.user.to_string_lossy().into_owned()),
            data_home: Some(fixture.base.join("data").to_string_lossy().into_owned()),
            node: Some(fixture.node.to_string_lossy().into_owned()),
            scale: 1.0,
            check: true,
            qualify_evidence: None,
        }
    }

    #[test]
    fn a_complete_prerequisite_set_produces_the_native_argv() {
        let fixture = fixture();
        let launched =
            plan(&options(&fixture), &fixture.package, &BTreeMap::new()).expect("a valid plan");
        assert_eq!(launched.node_version, "v26.1.2");
        assert_eq!(launched.node_mode, "explicit-absolute");
        assert_eq!(launched.state, State::New);
        assert!(launched.argv.iter().any(|arg| arg == "--runtime"));
        assert!(launched.argv.iter().any(|arg| arg.ends_with("data/host")));
        assert_eq!(launched.env["HOME"], fixture.user.to_string_lossy());
        // A scale outside the bound, a missing or foreign runtime, an overlapping
        // data home and a qualification run in --check mode are refused.
        let mut scaled = options(&fixture);
        scaled.scale = 0.5;
        assert_eq!(
            plan(&scaled, &fixture.package, &BTreeMap::new()).unwrap_err(),
            "Scale must be between 0.75 and 2.0"
        );
        let mut missing = options(&fixture);
        missing.runtime = None;
        assert!(plan(&missing, &fixture.package, &BTreeMap::new()).is_err());
        fs::write(
            fixture.runtime.join("package.json"),
            serde_json::to_vec(&serde_json::json!({"name": "other", "version": VERSION})).unwrap(),
        )
        .unwrap();
        assert!(plan(&options(&fixture), &fixture.package, &BTreeMap::new()).is_err());
        fs::write(
            fixture.runtime.join("package.json"),
            serde_json::to_vec(
                &serde_json::json!({"name": "@deepseek-ai/dsh", "version": VERSION}),
            )
            .unwrap(),
        )
        .unwrap();
        let mut overlap = options(&fixture);
        overlap.data_home = Some(fixture.user.to_string_lossy().into_owned());
        assert_eq!(
            plan(&overlap, &fixture.package, &BTreeMap::new()).unwrap_err(),
            "Native data overlaps protected installation or storage"
        );
        let mut qualified = options(&fixture);
        qualified.qualify_evidence = Some(fixture.base.to_string_lossy().into_owned());
        assert!(plan(&qualified, &fixture.package, &BTreeMap::new()).is_err());
        fs::remove_dir_all(fixture.base).unwrap();
    }
}

#[cfg(test)]
mod entry_tests {
    use super::*;

    fn args(tokens: &[&str]) -> Vec<String> {
        tokens.iter().map(|token| token.to_string()).collect()
    }

    #[test]
    fn the_entry_point_parses_flags_and_refuses_bad_ones() {
        let options = parse_options(&args(&[
            "--workspace",
            "/tmp/ws",
            "--runtime=/tmp/rt",
            "--check",
            "--scale",
            "1.25",
            "--user-home",
            "/tmp/user",
        ]))
        .expect("a valid command line");
        assert_eq!(options.workspace, "/tmp/ws");
        assert_eq!(options.runtime.as_deref(), Some("/tmp/rt"));
        assert_eq!(options.user_home.as_deref(), Some("/tmp/user"));
        assert!(options.check);
        assert_eq!(options.scale, 1.25);
        // A duplicated option, an unknown option, a missing value and a bad scale fail.
        assert_eq!(
            parse_options(&args(&["--workspace", "/a", "--workspace", "/b"]))
                .err()
                .unwrap(),
            "Duplicate launcher option"
        );
        assert!(parse_options(&args(&["--workspace", "/a", "--other"])).is_err());
        assert!(parse_options(&args(&["--workspace"])).is_err());
        assert!(parse_options(&args(&["--scale", "nope", "--workspace", "/a"])).is_err());
        assert!(
            parse_options(&args(&["--check"])).is_err(),
            "workspace is required"
        );
        // A refusal exits 2 without touching the filesystem.
        assert_eq!(
            main(
                &args(&["--workspace", "/tmp", "--check"]),
                Some(Path::new("/nonexistent-package"))
            ),
            2
        );
    }
}

#[cfg(test)]
mod packaging_tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_snapshot_copy_is_bounded_and_refuses_symlinks_and_existing_targets() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "package-copy-").unwrap();
        let source = base.join("source.bin");
        fs::write(&source, b"PUBLIC package payload").unwrap();
        let target = base.join("out/source.bin");
        copy_new(&source, &target, 0o600, 1024).expect("a bounded copy");
        assert_eq!(fs::read(&target).unwrap(), b"PUBLIC package payload");
        assert_eq!(
            fs::symlink_metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // An existing target is never overwritten.
        assert!(copy_new(&source, &target, 0o600, 1024).is_err());
        // A symlinked source and an over-limit source are refused.
        std::os::unix::fs::symlink(&source, base.join("link.bin")).unwrap();
        assert_eq!(
            copy_new(
                &base.join("link.bin"),
                &base.join("out/link.bin"),
                0o600,
                1024
            )
            .unwrap_err(),
            "Source symlinks are not packaged"
        );
        assert!(copy_new(&source, &base.join("out/big.bin"), 0o600, 4).is_err());
        fs::remove_dir_all(base).unwrap();
    }

    fn report() -> Value {
        serde_json::json!({
            "scope": "native-control-layout-and-Harness-style-conversation",
            "defaultReleaseSha256": "a".repeat(64),
            "parentPaintInspectionComplete": true,
            "checkAllTargetsFeaturePassed": true,
            "defaultAppTestsPassed": 10,
            "featureAppTestsPassed": 20,
            "fixtureExampleTestsPassed": 5,
            "publicCases": [{"passed": true, "exitCode": 0}],
            "realKeyless": {"passed": true, "exitCode": 0},
            "systemNodeVersionObserved": "v26.1.2",
            "sourceSha256": {"src/ui.rs": "b".repeat(64)},
            "formatCheckPassed": true,
        })
    }

    fn qualified(base: &Path, value: &Value) -> Result<Value, String> {
        let path = base.join("qualification.json");
        fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        qualified_native(&path, &"a".repeat(64), base)
    }

    #[test]
    fn a_native_qualification_must_match_the_release_and_carry_its_evidence() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "package-qual-").unwrap();
        assert!(qualified(&base, &report()).is_ok());
        for (key, value) in [
            ("defaultReleaseSha256", serde_json::json!("c".repeat(64))),
            ("parentPaintInspectionComplete", serde_json::json!(false)),
            ("defaultAppTestsPassed", serde_json::json!(0)),
            ("formatCheckPassed", serde_json::json!(false)),
            ("systemNodeVersionObserved", serde_json::json!("v25.1.0")),
        ] {
            let mut broken = report();
            broken[key] = value;
            assert!(qualified(&base, &broken).is_err(), "{key} was accepted");
        }
        // A missing app-source pin is refused.
        let mut unpinned = report();
        unpinned["sourceSha256"] = serde_json::json!({});
        assert!(qualified(&base, &unpinned).is_err());
        // A scoped format check is accepted only with its exact scope and exit code.
        let mut scoped = report();
        scoped["formatCheckPassed"] = serde_json::json!(false);
        scoped["scopedFormatCheck"] =
            serde_json::json!({"passed": true, "scope": "explicit-app-files", "exitCode": 0});
        assert!(qualified(&base, &scoped).is_ok());
        scoped["scopedFormatCheck"]["scope"] = serde_json::json!("all");
        assert!(qualified(&base, &scoped).is_err());
        // A failing public case is refused.
        let mut failing = report();
        failing["publicCases"] = serde_json::json!([{"passed": false, "exitCode": 1}]);
        assert!(qualified(&base, &failing).is_err());
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    use std::fs;

    fn tree() -> PathBuf {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "package-native-").unwrap();
        for module in NATIVE_MODULES {
            let root = base.join(module);
            fs::create_dir_all(root.join("src")).unwrap();
            fs::write(
                root.join("Cargo.toml"),
                format!("[package]\nname = \"{module}\"\n"),
            )
            .unwrap();
            fs::write(root.join("Cargo.lock"), b"# PUBLIC lock\n").unwrap();
            fs::write(root.join("src/lib.rs"), format!("// {module}\n")).unwrap();
        }
        fs::create_dir_all(base.join("app/vendor/crate")).unwrap();
        fs::write(base.join("app/vendor/crate/Cargo.toml"), b"[package]\n").unwrap();
        base
    }

    #[test]
    fn native_sources_are_selected_deterministically_and_inventoried() {
        let base = tree();
        let canonical = base.canonicalize().unwrap();
        let files = native_source_files(&canonical).unwrap();
        assert!(
            files
                .iter()
                .any(|(path, _)| path.ends_with("app/vendor/crate/Cargo.toml"))
        );
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(files, sorted, "the selection is ordered by path");
        let inventory = native_source_inventory(&canonical).unwrap();
        assert_eq!(
            inventory["fileCount"].as_u64().unwrap() as usize,
            files.len()
        );
        assert!(
            inventory["files"][0]["path"]
                .as_str()
                .unwrap()
                .starts_with("source/native/")
        );
        // The vendor tree is only admitted for the app module.
        assert_eq!(native_folders("app").len(), 6);
        assert_eq!(native_folders("core").len(), 5);
        // A private environment file and a symlink are refused.
        fs::write(base.join("core/src/.env.local"), b"SECRET=1").unwrap();
        assert_eq!(
            native_source_files(&canonical).unwrap_err(),
            "Private or unexpected native source file refused"
        );
        fs::remove_file(base.join("core/src/.env.local")).unwrap();
        std::os::unix::fs::symlink(base.join("app/src/lib.rs"), base.join("app/src/link.rs"))
            .unwrap();
        assert_eq!(
            native_source_files(&canonical).unwrap_err(),
            "Native source symlink refused"
        );
        // Relative and non-canonical inputs are refused.
        assert_eq!(
            native_path(Path::new("relative"), true).unwrap_err(),
            "Native inputs require absolute paths without parents"
        );
        assert!(
            native_path(&base.join("."), true).is_err(),
            "a non-canonical path is refused"
        );
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod license_tests {
    use super::*;
    use std::fs;

    fn fixture() -> (PathBuf, PathBuf, Value) {
        let base =
            crate::tempdir::make_temp_dir(&std::env::temp_dir(), "package-licenses-").unwrap();
        let src = base.join("registry/src");
        // The cache nests crates under an index directory, so the crate is two parts deep.
        let crate_dir = src.join("index.crates.io-1949cf8c6b5b557f/public-crate-1.2.3");
        fs::create_dir_all(&crate_dir).unwrap();
        fs::write(crate_dir.join("Cargo.toml"), b"[package]\n").unwrap();
        fs::write(crate_dir.join("LICENSE-MIT"), b"PUBLIC MIT text").unwrap();
        fs::write(crate_dir.join("NOTICE"), b"PUBLIC notice").unwrap();
        let payload = base.join("payload");
        fs::create_dir_all(&payload).unwrap();
        let metadata = serde_json::json!({"packages": [
            {"name": "public-crate", "version": "1.2.3", "source": "registry+https://crates.io",
             "manifest_path": crate_dir.join("Cargo.toml").to_string_lossy(),
             "license": "MIT OR Apache-2.0"},
            {"name": "local-path", "version": "0.1.0", "source": null,
             "manifest_path": crate_dir.join("Cargo.toml").to_string_lossy(), "license": null},
        ]});
        (base, payload, metadata)
    }

    #[test]
    fn registry_license_texts_are_copied_and_inventoried() {
        let (base, payload, metadata) = fixture();
        let inventory =
            licenses(&metadata, &base.join("registry/src"), &payload).expect("licenses");
        assert_eq!(
            inventory["packageCount"], 1,
            "a path dependency is not a registry package"
        );
        assert_eq!(inventory["packagesWithCopiedLicenseTexts"], 1);
        let texts = inventory["packages"][0]["licenseTexts"].as_array().unwrap();
        assert_eq!(texts.len(), 2);
        let copied = payload.join(texts[0].as_str().unwrap());
        assert_eq!(fs::read(&copied).unwrap(), b"PUBLIC MIT text");
        assert!(payload.join("licenses/rust-inventory.json").is_file());
        // A non-registry source, an invalid name and a mismatched crate directory are refused.
        for (packages, expected) in [
            (
                serde_json::json!([{"name": "x", "version": "1.0.0", "source": "git+https://example",
                "manifest_path": base.join("registry/src/index.crates.io-1949cf8c6b5b557f/public-crate-1.2.3/Cargo.toml").to_string_lossy(), "license": null}]),
                "Unsupported Cargo package identity",
            ),
            (
                serde_json::json!([{"name": "bad name", "version": "1.0.0", "source": "registry+x",
                "manifest_path": base.join("registry/src/index.crates.io-1949cf8c6b5b557f/public-crate-1.2.3/Cargo.toml").to_string_lossy(), "license": null}]),
                "Unsupported Cargo package identity",
            ),
            (
                serde_json::json!([{"name": "other", "version": "9.9.9", "source": "registry+x",
                "manifest_path": base.join("registry/src/index.crates.io-1949cf8c6b5b557f/public-crate-1.2.3/Cargo.toml").to_string_lossy(), "license": null}]),
                "Cargo license source is not the expected cached crate",
            ),
        ] {
            let metadata = serde_json::json!({"packages": packages});
            assert_eq!(
                licenses(&metadata, &base.join("registry/src"), &payload).unwrap_err(),
                expected
            );
        }
        fs::remove_dir_all(base).unwrap();
    }
}
