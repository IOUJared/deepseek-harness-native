//! Owned scripted native UI/worker/isolated Host export.
//!
//! Port of the report and capture half of `scripts/qualify-composed-export.py`.
//! The blank-session archive is checked independently (bounded inflate, CRC and
//! UTF-8/object JSONL) before this report can pass.
use super::registry::{MAX_PNG_BYTES, png_dimensions, private_regular};
use serde_json::Value;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::Path;

pub const APP_ID: &str = "ai.deepseek.harness.native.export-composed-fixture";
pub const ARCHIVE: &str = "PUBLIC-session-export.zip";
pub const MAX_ZIP: u64 = 4 * 1024 * 1024;
pub const MAX_ROOT: u64 = 1024 * 1024;
const CAPTURES: [(&str, &str); 2] = [
    ("exportReviewCapture", "export-review.png"),
    ("exportSavedCapture", "export-saved.png"),
];

const REQUIRED: [&str; 17] = [
    "publicFixtureOnly",
    "scriptedUiMessages",
    "realWorkerReady",
    "freshEmptyBaselineObserved",
    "ownedWorkspaceCreated",
    "oneOrdinaryPublicSessionCreated",
    "selectedBaselineObserved",
    "matchingSavedReceiptObserved",
    "rootOnlyExportConfirmed",
    "noOverwriteReceiptObserved",
    "privateZipVerified",
    "exactBytesUnchanged",
    "samePathBytesAndMetadataUnchanged",
    "twoExactReceiptsObserved",
    "gracefulStopped",
    "actualCloseUiDispatched",
    "actualCloseInspectionObserved",
];
const ABSENT: [&str; 10] = [
    "modelCatalogRequested",
    "browserSignInRequested",
    "apiKeySaveRequested",
    "pluginWritesRequested",
    "settingsWritesRequested",
    "manualBusinessMessagesForwarded",
    "sessionDeletionRequested",
    "stopActivityRequested",
    "includeDescendants",
    "realCredentialsUsed",
];

fn positive(value: &Value) -> bool {
    value.as_i64().is_some_and(|number| number >= 1)
}

/// Two exact save receipts: a new archive, then the same path refusing overwrite.
pub fn valid_app(app: &Value, pid: u64, scale: f64) -> bool {
    if app["status"].as_str() != Some("passed")
        || app["pid"].as_u64() != Some(pid)
        || app["applicationScale"].as_f64() != Some(scale)
        || app["windowBackend"].as_str() != Some("wayland")
        || app["targetCount"].as_i64() != Some(1)
        || app["zipArtifact"].as_str() != Some(ARCHIVE)
        || REQUIRED.iter().any(|key| app[*key].as_bool() != Some(true))
        || app["modelPrompts"].as_i64() != Some(0)
        || ABSENT.iter().any(|key| app[*key].as_bool() != Some(false))
    {
        return false;
    }
    let Some(stages) = app["stages"].as_array() else {
        return false;
    };
    if stages.len() != 2 {
        return false;
    }
    for stage in stages {
        if [
            "matchingExportedReceipt",
            "controllerPendingRetired",
            "enteredPathCleared",
        ]
        .iter()
        .any(|key| stage[*key].as_bool() != Some(true))
        {
            return false;
        }
        let receipt = &stage["receipt"];
        if !["epoch", "generation", "serial", "editor", "attempt"]
            .iter()
            .all(|key| positive(&receipt[*key]))
        {
            return false;
        }
    }
    let (first, second) = (&stages[0], &stages[1]);
    let (a, b) = (&first["receipt"], &second["receipt"]);
    first["name"].as_str() == Some("save-new-public-archive")
        && first["saved"].as_bool() == Some(true)
        && first["notCreated"].as_bool() == Some(false)
        && first["savedBytes"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0 && bytes <= MAX_ZIP)
        && second["name"].as_str() == Some("same-path-no-overwrite")
        && second["saved"].as_bool() == Some(false)
        && second["notCreated"].as_bool() == Some(true)
        && second["savedBytes"].is_null()
        && a["epoch"] == b["epoch"]
        && a["generation"] == b["generation"]
        && b["serial"].as_i64() > a["serial"].as_i64()
        && b["editor"].as_i64() > a["editor"].as_i64()
        && b["attempt"].as_i64() > a["attempt"].as_i64()
}

/// Both captures must be private PNGs reporting the file they saved.
pub fn valid_captures(output: &Path, app: &Value) -> bool {
    for (key, filename) in CAPTURES {
        let capture = &app[key];
        let Some(data) = private_regular(&output.join(filename), MAX_PNG_BYTES) else {
            return false;
        };
        let Some((width, height)) = png_dimensions(&data) else {
            return false;
        };
        if capture["saved"].as_bool() != Some(true)
            || capture["artifact"].as_str() != Some(filename)
            || capture["physicalSize"] != serde_json::json!([width, height])
        {
            return false;
        }
    }
    true
}

const EOCD: u32 = 0x0605_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const LOCAL: u32 = 0x0403_4b50;

fn u16le(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32le(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn find_eocd(data: &[u8]) -> Option<usize> {
    let floor = data.len().saturating_sub(65557);
    (floor..data.len().saturating_sub(3))
        .rev()
        .find(|position| u32le(data, *position) == Some(EOCD))
}

fn failure(code: &str) -> Value {
    serde_json::json!({"passed": false, "failureCode": code})
}

/// Inflate one raw deflate stream with an explicit output budget.
fn raw_inflate(compressed: &[u8], limit: u64) -> Option<Vec<u8>> {
    let mut stream = flate2::Decompress::new(false);
    let mut output = vec![0u8; usize::try_from(limit + 1).ok()?];
    let status = stream
        .decompress(compressed, &mut output, flate2::FlushDecompress::Finish)
        .ok()?;
    if status != flate2::Status::StreamEnd || stream.total_in() as usize != compressed.len() {
        return None;
    }
    output.truncate(stream.total_out() as usize);
    Some(output)
}

/// Bounded independent inflate+CRC+UTF8/object-JSONL check; never an extraction.
pub fn inspect_archive(output: &Path, app: &Value) -> Value {
    let Some(data) = private_regular(&output.join(ARCHIVE), MAX_ZIP) else {
        return failure("private-archive-count");
    };
    let expected = app["privateArchiveFileVerified"]["bytes"].as_u64();
    if data.is_empty() || Some(data.len() as u64) != expected {
        return failure("private-archive-count");
    }
    let Some(eocd) = find_eocd(&data) else {
        return failure("root-zip-crc-or-jsonl-invalid");
    };
    let Some(count) = u16le(&data, eocd + 10) else {
        return failure("root-zip-crc-or-jsonl-invalid");
    };
    let Some(directory) = u32le(&data, eocd + 16).map(|offset| offset as usize) else {
        return failure("root-zip-crc-or-jsonl-invalid");
    };
    if count != 1 || u32le(&data, directory) != Some(CENTRAL) {
        return failure("blank-root-entry-profile");
    }
    let Some(flags) = u16le(&data, directory + 8) else {
        return failure("root-metadata-bound");
    };
    let Some(method) = u16le(&data, directory + 10) else {
        return failure("root-metadata-bound");
    };
    let Some(crc) = u32le(&data, directory + 16) else {
        return failure("root-metadata-bound");
    };
    let Some(compressed_size) = u32le(&data, directory + 20).map(|size| size as u64) else {
        return failure("root-metadata-bound");
    };
    let Some(uncompressed_size) = u32le(&data, directory + 24).map(|size| size as u64) else {
        return failure("root-metadata-bound");
    };
    let Some(name_length) = u16le(&data, directory + 28).map(|length| length as usize) else {
        return failure("root-metadata-bound");
    };
    let Some(central_extra) = u16le(&data, directory + 30) else {
        return failure("root-metadata-bound");
    };
    let Some(comment_length) = u16le(&data, directory + 32).map(|length| length as usize) else {
        return failure("root-metadata-bound");
    };
    let Some(local_offset) = u32le(&data, directory + 42).map(|offset| offset as usize) else {
        return failure("root-metadata-bound");
    };
    let name = data.get(directory + 46..directory + 46 + name_length);
    let end_of_central = directory + 46 + name_length + central_extra as usize + comment_length;
    if end_of_central > data.len() {
        return failure("root-metadata-bound");
    }
    if flags & 1 != 0 || method != 0 && method != 8 || uncompressed_size > MAX_ROOT {
        return failure("root-metadata-bound");
    }
    if name != Some(b"session.v4.jsonl".as_slice()) {
        return failure("blank-root-entry-profile");
    }
    if u32le(&data, local_offset) != Some(LOCAL) || local_offset + 30 > data.len() {
        return failure("root-local-header");
    }
    let Some(local_name_length) = u16le(&data, local_offset + 26).map(|length| length as usize)
    else {
        return failure("root-local-data-bound");
    };
    let Some(local_extra) = u16le(&data, local_offset + 28) else {
        return failure("root-local-data-bound");
    };
    let start = local_offset + 30 + local_name_length + local_extra as usize;
    let end = start + compressed_size as usize;
    if local_extra != 0
        || end > data.len()
        || data.get(local_offset + 30..local_offset + 30 + local_name_length)
            != Some(b"session.v4.jsonl".as_slice())
    {
        return failure("root-local-data-bound");
    }
    let compressed = &data[start..end];
    let root = match method {
        8 => match raw_inflate(compressed, MAX_ROOT) {
            Some(bytes) => bytes,
            None => return failure("root-inflate-bound-or-eof"),
        },
        0 => compressed.to_vec(),
        _ => return failure("root-compression-method"),
    };
    if root.len() as u64 > MAX_ROOT
        || root.len() as u64 != uncompressed_size
        || crate::qualify::registry::crc32(&[&root]) != crc
    {
        return failure("root-inflate-count-or-crc");
    }
    if !root.ends_with(b"\n") {
        return failure("root-jsonl-bound");
    }
    let Some(text) = std::str::from_utf8(&root).ok() else {
        return failure("root-zip-crc-or-jsonl-invalid");
    };
    let lines: Vec<&str> = text.split('\n').filter(|line| !line.is_empty()).collect();
    if lines.is_empty() || lines.len() > 256 || lines.iter().any(|line| line.len() > 65536) {
        return failure("root-jsonl-bound");
    }
    let mut records = Vec::new();
    for line in &lines {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            return failure("root-zip-crc-or-jsonl-invalid");
        };
        if !record.is_object() {
            return failure("root-jsonl-object-profile");
        }
        records.push(record);
    }
    let header = &records[0];
    if header["type"].as_str() != Some("session")
        || header["version"].as_i64() != Some(4)
        || header["id"]
            .as_str()
            .is_none_or(|id| id.is_empty() || id.len() > 512)
        || header["createdAt"]
            .as_i64()
            .is_none_or(|created| created < 0)
        || header["isSeeded"].as_bool() != Some(false)
        || header["delegationDepth"].as_i64() != Some(0)
        || header.get("origin").is_some()
        || header.get("parentSession").is_some()
    {
        return failure("ordinary-v4-header-profile");
    }
    serde_json::json!({
        "passed": true,
        "archiveBytes": data.len(),
        "archiveSha256": crate::sha256_file(&output.join(ARCHIVE)).unwrap_or_default(),
        "entryCount": 1,
        "rootUncompressedBytes": root.len(),
        "jsonLineCount": lines.len(),
        "headerLineCount": 1,
        "eventRecordCount": lines.len() - 1,
        "crcAndBoundedInflateChecked": true,
        "utf8ObjectJsonlChecked": true,
        "sessionIdentityAuthenticated": false,
        "sessionFormatSemanticsFullyChecked": false,
        "richReferencedAttachmentsQualified": false,
        "extracted": false,
        "rawContentReported": false,
    })
}

/// Stream the fixture binary under a bound, rejecting a replaced inode.
fn binary_digest(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let before = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !before.file_type().is_file() || before.size() > 64 * 1024 * 1024 {
        return Err("invalid-fixture-binary".into());
    }
    let c_path = std::ffi::CString::new(std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str()))
        .map_err(|_| "path-with-nul")?;
    // Safe: a NUL-terminated path and constant flags; the descriptor is closed below.
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err("invalid-fixture-binary".into());
    }
    let mut opened: libc::stat = unsafe { std::mem::zeroed() };
    // Safe: a live descriptor and a valid out-parameter.
    if unsafe { libc::fstat(fd, &mut opened) } != 0
        || opened.st_dev != before.dev()
        || opened.st_ino != before.ino()
    {
        unsafe { libc::close(fd) };
        return Err("fixture-binary-changed".into());
    }
    use std::os::unix::io::FromRawFd;
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let read =
            std::io::Read::read(&mut file, &mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        count += read as u64;
        if count > 64 * 1024 * 1024 {
            return Err("fixture-binary-oversize".into());
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

/// `xtask qualify composed-export --output NEW_DIR [--scale 1.0|1.25]`.
pub fn run(root: &Path, output: &Path, scale: f64) -> i32 {
    let started = std::time::Instant::now();
    let output = match super::prepare(root, output) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("composed-export failed: {message}");
            return 1;
        }
    };
    let Ok(binary) = root
        .join("app/target/release/examples/export_composed_smoke")
        .canonicalize()
    else {
        eprintln!("composed-export failed: release fixture not ready");
        return 1;
    };
    let Ok(before_hash) = binary_digest(&binary) else {
        eprintln!("composed-export failed: invalid fixture binary");
        return 1;
    };
    // Safe: constant signals and a handler that only stores an atomic flag.
    let previous_int = unsafe {
        libc::signal(
            libc::SIGINT,
            super::composed::interrupt as *const () as libc::sighandler_t,
        )
    };
    let previous_term = unsafe {
        libc::signal(
            libc::SIGTERM,
            super::composed::interrupt as *const () as libc::sighandler_t,
        )
    };
    super::composed::INTERRUPTED.store(false, std::sync::atomic::Ordering::SeqCst);
    let spawned = std::process::Command::new(&binary)
        .arg("--output")
        .arg(&output)
        .arg("--scale")
        .arg(format!("{scale}"))
        .env_clear()
        .envs(super::environment(&output))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn();
    let Ok(child) = spawned else {
        eprintln!("composed-export failed: cannot spawn the fixture");
        return 1;
    };
    let child_pid = child.id() as u64;
    let mut run = super::composed::guard(super::composed::Owned {
        child,
        status: None,
    });
    // Safe: restore the handlers this run installed.
    unsafe {
        libc::signal(libc::SIGINT, previous_int);
        libc::signal(libc::SIGTERM, previous_term);
    }
    if run.cleanup.root_was_running_at_cleanup {
        run.forced = true;
    }
    let app = super::registry::read_app(&output);
    let app_valid = valid_app(&app, child_pid, scale);
    let captures_valid = valid_captures(&output, &app);
    let archive = inspect_archive(&output, &app);
    let unchanged = binary_digest(&binary).as_deref() == Ok(before_hash.as_str());
    let passed = run.owned.code() == 0
        && !run.forced
        && run.failure.is_none()
        && app_valid
        && captures_valid
        && archive["passed"].as_bool() == Some(true)
        && unchanged
        && run.cleanup.observed_cleanup_complete
        && run.cleanup.remaining_before_external_cleanup.is_empty()
        && run.observation_failures == 0
        && run.facts.as_ref().is_some_and(|facts| {
            facts["class"].as_str() == Some(APP_ID) && facts["xwayland"].as_bool() == Some(false)
        });
    let report = serde_json::json!({
        "status": if passed { "passed" } else { "failed" },
        "scope": "Owned scripted native UI/worker/isolated Host export; public blank session only, no physical-input proof.",
        "binarySha256": before_hash,
        "binaryUnchanged": unchanged,
        "exitCode": run.owned.code(),
        "forced": run.forced,
        "failureCode": run.failure,
        "interrupted": run.interrupted,
        "ownedWindow": run.facts,
        "ownershipObservationFailures": run.observation_failures,
        "observedOwnedProcessCount": run.observed,
        "qualificationBudgetSeconds": super::composed::QUALIFICATION_SECONDS,
        "fixtureShutdownRequestSeconds": 35,
        "cleanup": &run.cleanup,
        "elapsedWallSeconds": started.elapsed().as_secs_f64(),
        "hardTotalWallClockBoundClaimed": false,
        "containmentScope": "observed pinned identities only",
        "app": app,
        "appEvidenceVerified": app_valid,
        "privateOwnRendererCapturesVerified": captures_valid,
        "independentArchiveInspection": archive,
        "paintInspected": false,
        "nativeKeyboardPointerInputQualified": false,
        "realCredentialsUsed": false,
        "desktopSettingsChanged": false,
        "installedRuntimeChanged": false,
        "partialFailureOrUnhealthyFilesystemLiveQualified": false,
        "fullDesktopParity": false,
    });
    if let Err(error) = super::private_json(&output.join("result.json"), &report) {
        eprintln!("cannot write the export report: {error}");
        return 1;
    }
    println!(
        "{}",
        serde_json::json!({"status": report["status"], "report": output.join("result.json").to_string_lossy()})
    );
    if passed { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn app() -> Value {
        json!({
            "status": "passed", "pid": 7100, "applicationScale": 1.0,
            "windowBackend": "wayland", "targetCount": 1, "zipArtifact": ARCHIVE,
            "modelPrompts": 0,
            "stages": [
                {"name": "save-new-public-archive", "saved": true, "notCreated": false,
                 "savedBytes": 1024, "matchingExportedReceipt": true,
                 "controllerPendingRetired": true, "enteredPathCleared": true,
                 "receipt": {"epoch": 3, "generation": 5, "serial": 7, "editor": 2, "attempt": 1}},
                {"name": "same-path-no-overwrite", "saved": false, "notCreated": true,
                 "savedBytes": null, "matchingExportedReceipt": true,
                 "controllerPendingRetired": true, "enteredPathCleared": true,
                 "receipt": {"epoch": 3, "generation": 5, "serial": 9, "editor": 3, "attempt": 2}},
            ],
        })
    }

    fn with_flags(mut value: Value) -> Value {
        for key in REQUIRED {
            value[key] = json!(true);
        }
        for key in ABSENT {
            value[key] = json!(false);
        }
        value
    }

    #[test]
    fn two_exact_save_receipts_are_required_and_no_overwrite_must_not_save() {
        let good = with_flags(app());
        assert!(valid_app(&good, 7100, 1.0));
        // An overwrite is not the refused second stage.
        let mut overwritten = good.clone();
        overwritten["stages"][1]["saved"] = json!(true);
        assert!(!valid_app(&overwritten, 7100, 1.0));
        let mut created = good.clone();
        created["stages"][1]["notCreated"] = json!(false);
        assert!(!valid_app(&created, 7100, 1.0));
        // The two receipts must advance serial, editor and attempt on one epoch.
        for (key, value) in [("serial", 7), ("editor", 2), ("attempt", 1)] {
            let mut replay = good.clone();
            replay["stages"][1]["receipt"][key] = json!(value);
            assert!(!valid_app(&replay, 7100, 1.0), "{key} did not advance");
        }
        let mut epoch = good.clone();
        epoch["stages"][1]["receipt"]["epoch"] = json!(4);
        assert!(!valid_app(&epoch, 7100, 1.0));
        // A saved byte count outside the bound is refused.
        let mut huge = good.clone();
        huge["stages"][0]["savedBytes"] = json!(MAX_ZIP + 1);
        assert!(!valid_app(&huge, 7100, 1.0));
        // A retired pending controller is part of the proof.
        let mut pending = good.clone();
        pending["stages"][0]["controllerPendingRetired"] = json!(false);
        assert!(!valid_app(&pending, 7100, 1.0));
        // Overclaims and a substituted pid are refused.
        let mut descendants = good.clone();
        descendants["includeDescendants"] = json!(true);
        assert!(!valid_app(&descendants, 7100, 1.0));
        assert!(!valid_app(&good, 7101, 1.0));
        assert!(!valid_app(&json!({}), 7100, 1.0));
    }
}

#[cfg(test)]
mod archive_tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn crc(data: &[u8]) -> u32 {
        crate::qualify::registry::crc32(&[data])
    }

    fn deflate(data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    /// Build a one-entry archive; `flags`, `method` and `crc_value` are injectable.
    pub(super) fn archive(root: &[u8], method: u16, flags: u16, crc_value: Option<u32>) -> Vec<u8> {
        let body = if method == 8 {
            deflate(root)
        } else {
            root.to_vec()
        };
        let checksum = crc_value.unwrap_or_else(|| crc(root));
        let name = b"session.v4.jsonl";
        let mut out = Vec::new();
        // Local header.
        out.extend_from_slice(&LOCAL.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&checksum.to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(root.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&body);
        let central = out.len();
        // Central directory.
        out.extend_from_slice(&CENTRAL.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&checksum.to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(root.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(name);
        let directory_size = out.len() - central;
        // End of central directory.
        out.extend_from_slice(&EOCD.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&(directory_size as u32).to_le_bytes());
        out.extend_from_slice(&(central as u32).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    fn root() -> Vec<u8> {
        let mut root = Vec::new();
        root.extend_from_slice(
            b"{\"type\":\"session\",\"version\":4,\"id\":\"PUBLIC-session-1\",\"createdAt\":0,\"isSeeded\":false,\"delegationDepth\":0}\n",
        );
        root.extend_from_slice(b"{\"type\":\"event\"}\n");
        root
    }

    pub(super) fn inspect(output: &std::path::Path, bytes: &[u8]) -> Value {
        fs::write(output.join(ARCHIVE), bytes).unwrap();
        fs::set_permissions(output.join(ARCHIVE), fs::Permissions::from_mode(0o600)).unwrap();
        let app = serde_json::json!({"privateArchiveFileVerified": {"bytes": bytes.len()}});
        inspect_archive(output, &app)
    }

    #[test]
    fn a_blank_session_archive_is_checked_independently() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "export-zip-").unwrap();
        let stored = inspect(&base, &archive(&root(), 0, 0, None));
        assert_eq!(stored["passed"].as_bool(), Some(true), "{stored}");
        assert_eq!(stored["eventRecordCount"].as_i64(), Some(1));
        // A deflated entry is accepted through the bounded inflate.
        let deflated = inspect(&base, &archive(&root(), 8, 0, None));
        assert_eq!(deflated["passed"].as_bool(), Some(true), "{deflated}");
        // A bad CRC, a wrong header profile and a missing trailing newline fail.
        assert_eq!(
            inspect(&base, &archive(&root(), 0, 0, Some(0)))["failureCode"],
            "root-inflate-count-or-crc"
        );
        let mut wrong = root();
        wrong.truncate(wrong.len() - 1);
        assert_eq!(
            inspect(&base, &archive(&wrong, 0, 0, None))["failureCode"],
            "root-jsonl-bound"
        );
        let mut old = root();
        let text = String::from_utf8(old.clone())
            .unwrap()
            .replace("\"version\":4", "\"version\":3");
        old = text.into_bytes();
        assert_eq!(
            inspect(&base, &archive(&old, 0, 0, None))["failureCode"],
            "ordinary-v4-header-profile"
        );
        // An encrypted entry is refused, and a mismatched byte count is refused.
        assert_eq!(
            inspect(&base, &archive(&root(), 0, 1, None))["failureCode"],
            "root-metadata-bound"
        );
        fs::write(base.join(ARCHIVE), archive(&root(), 0, 0, None)).unwrap();
        let wrong_count = serde_json::json!({"privateArchiveFileVerified": {"bytes": 1}});
        assert_eq!(
            inspect_archive(&base, &wrong_count)["failureCode"],
            "private-archive-count"
        );
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    fn png(width: u32, height: u32) -> Vec<u8> {
        fn chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut out = Vec::new();
            out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            out.extend_from_slice(kind);
            out.extend_from_slice(payload);
            let mut crc = 0xffff_ffffu32;
            for byte in kind.iter().chain(payload) {
                crc ^= *byte as u32;
                for _ in 0..8 {
                    crc = if crc & 1 != 0 {
                        (crc >> 1) ^ 0xedb8_8320
                    } else {
                        crc >> 1
                    };
                }
            }
            out.extend_from_slice(&(!crc).to_be_bytes());
            out
        }
        let mut rows = vec![0u8; (width as usize * 4 + 1) * height as usize];
        for row in rows.chunks_mut(width as usize * 4 + 1) {
            row[0] = 0;
        }
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&rows).unwrap();
        let idat = encoder.finish().unwrap();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        out.extend(chunk(b"IHDR", &ihdr));
        out.extend(chunk(b"IDAT", &idat));
        out.extend(chunk(b"IEND", &[]));
        out
    }

    #[test]
    fn a_jsonl_line_must_be_a_bounded_object() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "export-jsonl-").unwrap();
        use super::archive_tests::{archive, inspect};
        // A scalar line is not an object record.
        assert_eq!(
            inspect(&base, &archive(b"1\n", 0, 0, None))["failureCode"],
            "root-jsonl-object-profile"
        );
        // A line over the bound is refused.
        let mut huge = Vec::new();
        huge.extend_from_slice(b"{\"type\":\"session\",\"version\":4,\"id\":\"PUBLIC-session-1\",\"createdAt\":0,\"isSeeded\":false,\"delegationDepth\":0}\n");
        huge.extend(std::iter::repeat_n(b'x', 70000));
        huge.push(b'\n');
        assert_eq!(
            inspect(&base, &archive(&huge, 0, 0, None))["failureCode"],
            "root-jsonl-bound"
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn both_captures_must_report_the_private_png_they_saved() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "export-capture-").unwrap();
        let body = png(3, 2);
        for name in ["export-review.png", "export-saved.png"] {
            let path = base.join(name);
            fs::write(&path, &body).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let app = serde_json::json!({
            "exportReviewCapture": {"saved": true, "artifact": "export-review.png", "physicalSize": [3, 2]},
            "exportSavedCapture": {"saved": true, "artifact": "export-saved.png", "physicalSize": [3, 2]},
        });
        assert!(valid_captures(&base, &app));
        // The reported artifact must be the file inspected.
        let mut swapped = app.clone();
        swapped["exportSavedCapture"]["artifact"] = serde_json::json!("export-review.png");
        assert!(!valid_captures(&base, &swapped));
        // A size mismatch is refused.
        let mut size = app.clone();
        size["exportReviewCapture"]["physicalSize"] = serde_json::json!([2, 3]);
        assert!(!valid_captures(&base, &size));
        // A missing file is refused.
        fs::remove_file(base.join("export-saved.png")).unwrap();
        assert!(!valid_captures(&base, &app));
        fs::remove_dir_all(base).unwrap();
    }
}
