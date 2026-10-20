//! Scripted actual native App/worker/Core reversible registry fixture.
//!
//! Port of the validation core of `scripts/qualify-composed-registry.py`:
//! receipts alone cannot pass — every planned state needs its independent
//! follow-up evidence, a strictly increasing ticket serial, and two distinct
//! public sessions that stay retained.
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::Path;

pub const APP_ID: &str = "ai.deepseek.harness.native.registry-composed-fixture";
pub const MAX_REPORT_BYTES: u64 = 65536;

/// `(name, operation, target slot, pin order, needs pin increment, needs archive increment)`
/// One planned registry transition and the evidence it must carry.
pub struct Stage {
    pub name: &'static str,
    pub operation: &'static str,
    pub target_slot: i64,
    pub pin_order: &'static [i64],
    pub needs_pin: bool,
    pub needs_archive: bool,
}

pub const STAGES: [Stage; 6] = [
    Stage {
        name: "pin-first",
        operation: "Pin",
        target_slot: 1,
        pin_order: &[1],
        needs_pin: true,
        needs_archive: false,
    },
    Stage {
        name: "pin-second",
        operation: "Pin",
        target_slot: 2,
        pin_order: &[2, 1],
        needs_pin: true,
        needs_archive: false,
    },
    Stage {
        name: "unpin-first",
        operation: "Unpin",
        target_slot: 1,
        pin_order: &[2],
        needs_pin: true,
        needs_archive: false,
    },
    Stage {
        name: "repin-first",
        operation: "Pin",
        target_slot: 1,
        pin_order: &[1, 2],
        needs_pin: true,
        needs_archive: false,
    },
    Stage {
        name: "archive-first",
        operation: "Archive",
        target_slot: 1,
        pin_order: &[2],
        needs_pin: true,
        needs_archive: true,
    },
    Stage {
        name: "restore-first",
        operation: "Restore",
        target_slot: 1,
        pin_order: &[2],
        needs_pin: false,
        needs_archive: true,
    },
];

const REQUIRED: [&str; 13] = [
    "publicFixtureOnly",
    "scriptedUiMessages",
    "realWorkerReady",
    "freshEmptyBaselineObserved",
    "ownedWorkspaceCreated",
    "twoDistinctPublicSessionsCreated",
    "ownedWorkspaceFeedMembershipObserved",
    "actualArchivedViewEntered",
    "restoredRecentViewVerified",
    "restoreDidNotRepin",
    "archiveAndUnpinCoupledFeedVerified",
    "allMatchingAcksAndIndependentFeedsVerified",
    "filesRetainedNotDeletionRollback",
];
const FORBIDDEN: [&str; 11] = [
    "realCredentialsUsed",
    "modelCatalogRequested",
    "browserSignInRequested",
    "apiKeySaveRequested",
    "stopActivityRequested",
    "sessionDeletionRequested",
    "forkOrRenameRequested",
    "manualBusinessMessagesForwarded",
    "rpcSetsInstalled",
    "nativeKeyboardPointerInputQualified",
    "liveHostDecisionQualified",
];

fn stop_ok(stop: &Value) -> bool {
    stop["exited"].as_bool() == Some(true)
        && stop["exitCode"].as_i64() == Some(0)
        && stop["graceful"].as_bool() == Some(true)
        && stop["containmentUnknown"].as_bool() == Some(false)
        && stop["observedDescendantsRemaining"].as_i64() == Some(0)
}

/// Every planned stage, its ticket ordering and the retained sessions.
pub fn valid_app(app: &Value, child_pid: u64, scale: f64) -> bool {
    if app["status"].as_str() != Some("passed")
        || app["pid"].as_u64() != Some(child_pid)
        || app["applicationScale"].as_f64() != Some(scale)
        || app["windowBackend"].as_str() != Some("wayland")
        || app["modelPrompts"].as_i64() != Some(0)
        || REQUIRED.iter().any(|key| app[*key].as_bool() != Some(true))
        || FORBIDDEN
            .iter()
            .any(|key| app[*key].as_bool() != Some(false))
    {
        return false;
    }
    let Some(ids) = app["publicSessionIds"].as_array() else {
        return false;
    };
    if ids.len() != 2
        || ids.iter().any(|value| {
            value
                .as_str()
                .is_none_or(|value| value.is_empty() || value.len() > 4096)
        })
        || ids[0] == ids[1]
    {
        return false;
    }
    let Some(stages) = app["stages"].as_array() else {
        return false;
    };
    if stages.len() != STAGES.len() {
        return false;
    }
    let mut previous_serial = 0i64;
    for (observed, stage) in stages.iter().zip(STAGES.iter()) {
        if observed["name"].as_str() != Some(stage.name)
            || observed["operation"].as_str() != Some(stage.operation)
            || observed["targetSlot"].as_i64() != Some(stage.target_slot)
            || observed["pinOrderSlots"] != serde_json::json!(stage.pin_order)
        {
            return false;
        }
        for key in [
            "matchingSubmissionAcknowledged",
            "requiredFeedIncrementsObserved",
            "authoritativeRegistryVerified",
            "actualNavigationVerified",
            "bothSessionsAndAccountingRetained",
        ] {
            if observed[key].as_bool() != Some(true) {
                return false;
            }
        }
        if stage.needs_pin && observed["pinIncrementObserved"].as_bool() != Some(true) {
            return false;
        }
        if stage.needs_archive && observed["archiveIncrementObserved"].as_bool() != Some(true) {
            return false;
        }
        let ticket = &observed["ticket"];
        let Some(generation) = ticket["generation"].as_i64() else {
            return false;
        };
        let Some(serial) = ticket["serial"].as_i64() else {
            return false;
        };
        if ticket["epoch"].as_i64() != Some(1) || generation <= 0 || serial <= previous_serial {
            return false;
        }
        previous_serial = serial;
    }
    stop_ok(&app["stop"])
}

pub const MAX_PNG_BYTES: u64 = 64 * 1024 * 1024;
const CAPTURES: [(&str, &str); 2] = [
    ("archiveReviewCapture", "archive-review.png"),
    ("archivedRestoreReviewCapture", "restore-review.png"),
];

fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for part in parts {
        for &byte in *part {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
    }
    !crc
}

struct Inflater {
    stream: flate2::Decompress,
    ended: bool,
    supplied: u64,
}

fn inflate_into(inflater: &mut Inflater, payload: &[u8], remaining: u64) -> Option<Vec<u8>> {
    let room = usize::try_from(remaining.checked_add(1)?).ok()?;
    let before = inflater.stream.total_in();
    let mut output = vec![0u8; room];
    let before_out = inflater.stream.total_out();
    let status = inflater
        .stream
        .decompress(payload, &mut output, flate2::FlushDecompress::None)
        .ok()?;
    inflater.supplied += payload.len() as u64;
    let produced = ((inflater.stream.total_out() - before_out) as usize).min(output.len());
    output.truncate(produced);
    if status == flate2::Status::StreamEnd {
        inflater.ended = true;
    } else if inflater.stream.total_in() != before + payload.len() as u64 {
        // Unconsumed input means the output bound cut the stream short.
        return None;
    }
    Some(output)
}

/// Validate a bounded RGBA8/noninterlaced writer-profile PNG: CRCs and chunk
/// order, one complete zlib stream, exact row lengths and valid filter tags.
/// Pixels are not reconstructed; ancillary chunks are CRC-checked only.
pub fn png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() as u64 > MAX_PNG_BYTES || data.len() < 33 || data[..8] != *b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let mut position = 8usize;
    let mut dimensions: Option<(u32, u32)> = None;
    let mut inflater: Option<Inflater> = None;
    let (mut expected, mut inflated, mut stride) = (0u64, 0u64, 0u64);
    let (mut idat_seen, mut idat_ended, mut palette_seen) = (false, false, false);
    while position < data.len() {
        if data.len() - position < 12 {
            return None;
        }
        let length = u32::from_be_bytes(data[position..position + 4].try_into().ok()?) as u64;
        let kind = &data[position + 4..position + 8];
        let end = position as u64 + 12 + length;
        if kind.iter().any(|byte| !byte.is_ascii_alphabetic()) || kind[2] & 32 != 0 {
            return None;
        }
        if end > data.len() as u64 {
            return None;
        }
        let end = end as usize;
        let payload = &data[position + 8..position + 8 + length as usize];
        if u32::from_be_bytes(data[end - 4..end].try_into().ok()?) != crc32(&[kind, payload]) {
            return None;
        }
        if dimensions.is_none() {
            if kind != b"IHDR" || length != 13 {
                return None;
            }
            let width = u32::from_be_bytes(payload[0..4].try_into().ok()?);
            let height = u32::from_be_bytes(payload[4..8].try_into().ok()?);
            let (depth, color, compression, filtering, interlace) = (
                payload[8],
                payload[9],
                payload[10],
                payload[11],
                payload[12],
            );
            stride = width as u64 * 4 + 1;
            expected = stride * height as u64;
            if width == 0
                || height == 0
                || expected > MAX_PNG_BYTES
                || (depth, color, compression, filtering, interlace) != (8, 6, 0, 0, 0)
            {
                return None;
            }
            dimensions = Some((width, height));
            inflater = Some(Inflater {
                stream: flate2::Decompress::new(true),
                ended: false,
                supplied: 0,
            });
        } else if kind == b"IHDR" {
            return None;
        } else if kind == b"PLTE" {
            if palette_seen || idat_seen || length == 0 || !length.is_multiple_of(3) || length > 768
            {
                return None;
            }
            palette_seen = true;
        } else if kind == b"IDAT" {
            if idat_ended {
                return None;
            }
            idat_seen = true;
            let remaining = expected - inflated;
            let rows = inflate_into(inflater.as_mut()?, payload, remaining)?;
            if rows.len() as u64 > remaining {
                return None;
            }
            let first_filter = ((stride - inflated % stride) % stride) as usize;
            if rows
                .iter()
                .skip(first_filter)
                .step_by(stride as usize)
                .any(|filter| *filter > 4)
            {
                return None;
            }
            inflated += rows.len() as u64;
        } else if kind == b"IEND" {
            let inflater = inflater.as_ref()?;
            if length != 0
                || end != data.len()
                || !idat_seen
                || inflated != expected
                || !inflater.ended
                || inflater.supplied != inflater.stream.total_in()
            {
                return None;
            }
            return dimensions;
        } else {
            // An unknown critical chunk changes image interpretation.
            if kind[0] & 32 == 0 {
                return None;
            }
            if idat_seen {
                idat_ended = true;
            }
        }
        position = end;
    }
    None
}

pub(super) fn private_regular(path: &Path, maximum: u64) -> Option<Vec<u8>> {
    let info = fs::symlink_metadata(path).ok()?;
    if !info.file_type().is_file() || info.mode() & 0o777 != 0o600 || info.size() > maximum {
        return None;
    }
    let c_path =
        std::ffi::CString::new(std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str())).ok()?;
    // Safe: a NUL-terminated path and constant flags; the descriptor is closed below.
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return None;
    }
    let mut opened: libc::stat = unsafe { std::mem::zeroed() };
    // Safe: a live descriptor and a valid out-parameter.
    if unsafe { libc::fstat(fd, &mut opened) } != 0
        || opened.st_dev != info.dev()
        || opened.st_ino != info.ino()
        || opened.st_mode & libc::S_IFMT != libc::S_IFREG
        || opened.st_mode & 0o777 != 0o600
    {
        unsafe { libc::close(fd) };
        return None;
    }
    use std::os::unix::io::FromRawFd;
    let mut file = unsafe { fs::File::from_raw_fd(fd) };
    let mut data = Vec::new();
    file.read_to_end(&mut data).ok()?;
    if data.len() as u64 > maximum {
        None
    } else {
        Some(data)
    }
}

/// Both captures must be private PNGs whose reported size matches the header.
pub fn valid_captures(output: &Path, app: &Value) -> bool {
    for (key, filename) in CAPTURES {
        let report = &app[key];
        let Some(data) = private_regular(&output.join(filename), MAX_PNG_BYTES) else {
            return false;
        };
        if report["saved"].as_bool() != Some(true)
            || report["manualInputProof"].as_bool() != Some(false)
        {
            return false;
        }
        let Some((width, height)) = png_dimensions(&data) else {
            return false;
        };
        if report["physicalSize"] != serde_json::json!([width, height]) {
            return false;
        }
    }
    true
}

/// The private app report, or a status stub when it is missing or malformed.
pub fn read_app(output: &Path) -> Value {
    let Some(data) = private_regular(&output.join("app.json"), MAX_REPORT_BYTES) else {
        return serde_json::json!({"status": "missing-or-invalid-private-app-report"});
    };
    serde_json::from_slice(&data)
        .ok()
        .filter(Value::is_object)
        .unwrap_or_else(|| serde_json::json!({"status": "invalid-app-report"}))
}

/// `xtask qualify composed-registry --output NEW_DIR [--scale 1.0|1.25]`.
pub fn run(root: &Path, output: &Path, scale: f64) -> i32 {
    let started = std::time::Instant::now();
    if scale != 1.0 && scale != 1.25 {
        eprintln!("composed-registry failed: invalid scale");
        return 1;
    }
    let output = match super::prepare(root, output) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("composed-registry failed: {message}");
            return 1;
        }
    };
    let Ok(binary) = root
        .join("app/target/release/examples/registry_composed_smoke")
        .canonicalize()
    else {
        eprintln!("composed-registry failed: release fixture not ready");
        return 1;
    };
    let before_hash = match crate::sha256_file(&binary) {
        Ok(hash) => hash,
        Err(error) => {
            eprintln!("composed-registry failed: {error}");
            return 1;
        }
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
        eprintln!("composed-registry failed: cannot spawn the fixture");
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
    let app = read_app(&output);
    let app_valid = valid_app(&app, child_pid, scale);
    let captures_valid = valid_captures(&output, &app);
    let binary_unchanged = crate::sha256_file(&binary).is_ok_and(|hash| hash == before_hash);
    let passed = run.owned.code() == 0
        && !run.forced
        && run.failure.is_none()
        && run.cleanup.observed_cleanup_complete
        && run.cleanup.remaining_before_external_cleanup.is_empty()
        && run.observation_failures == 0
        && app_valid
        && captures_valid
        && run.facts.as_ref().is_some_and(|facts| {
            facts["class"].as_str() == Some(APP_ID) && facts["xwayland"].as_bool() == Some(false)
        })
        && binary_unchanged;
    let report = serde_json::json!({
        "status": if passed { "passed" } else { "failed" },
        "scope": "Scripted actual native App/worker/Core reversible registry fixture in a NEW private home only.",
        "binarySha256": before_hash,
        "binaryUnchanged": binary_unchanged,
        "exitCode": run.owned.code(),
        "forced": run.forced,
        "interrupted": run.interrupted,
        "failureCode": run.failure,
        "ownedWindow": run.facts,
        "observedOwnedProcessCount": run.observed,
        "ownershipObservationFailures": run.observation_failures,
        "qualificationBudgetSeconds": super::composed::QUALIFICATION_SECONDS,
        "fixtureShutdownRequestSeconds": 35,
        "cleanup": &run.cleanup,
        "elapsedWallSeconds": started.elapsed().as_secs_f64(),
        "wallClockIncludesPreparationObservationCleanup": true,
        "hardTotalWallClockBoundClaimed": false,
        "containmentScope": "observed pinned identities only",
        "remainingBeforeExternalCleanup": &run.cleanup.remaining_before_external_cleanup,
        "remainingAfterExternalCleanup": &run.cleanup.remaining_after_external_cleanup,
        "app": app,
        "appEvidenceVerified": app_valid,
        "privateOwnRendererCapturesVerified": captures_valid,
        "paintInspected": false,
        "nativeKeyboardPointerInputQualified": false,
        "liveHostDecisionQualified": false,
        "desktopSettingsChanged": false,
        "installedRuntimeChanged": false,
        "realCredentialsUsed": false,
        "mutationsReversible": true,
        "filesRetainedNotDeletionRollback": true,
    });
    if let Err(error) = super::private_json(&output.join("result.json"), &report) {
        eprintln!("cannot write the registry report: {error}");
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
        let stages: Vec<Value> = STAGES
            .iter()
            .enumerate()
            .map(|(index, stage)| {
                json!({
                    "name": stage.name, "operation": stage.operation, "targetSlot": stage.target_slot,
                    "pinOrderSlots": stage.pin_order,
                    "matchingSubmissionAcknowledged": true,
                    "requiredFeedIncrementsObserved": true,
                    "authoritativeRegistryVerified": true,
                    "actualNavigationVerified": true,
                    "bothSessionsAndAccountingRetained": true,
                    "pinIncrementObserved": true,
                    "archiveIncrementObserved": true,
                    "ticket": {"epoch": 1, "generation": index + 1, "serial": index + 1},
                })
            })
            .collect();
        let mut value = json!({
            "status": "passed", "pid": 6100, "applicationScale": 1.0,
            "windowBackend": "wayland", "modelPrompts": 0,
            "publicSessionIds": ["PUBLIC-session-a", "PUBLIC-session-b"],
            "stages": stages,
            "stop": {"exited": true, "exitCode": 0, "graceful": true,
                     "containmentUnknown": false, "observedDescendantsRemaining": 0},
        });
        for key in REQUIRED {
            value[key] = json!(true);
        }
        for key in FORBIDDEN {
            value[key] = json!(false);
        }
        value
    }

    #[test]
    fn a_complete_registry_run_passes_and_any_stage_gap_fails() {
        assert!(valid_app(&app(), 6100, 1.0));
        // A receipt without its follow-up evidence is not a pass.
        let mut unverified = app();
        unverified["stages"][2]["requiredFeedIncrementsObserved"] = json!(false);
        assert!(!valid_app(&unverified, 6100, 1.0));
        // The pin order and target slot are part of the proof.
        let mut order = app();
        order["stages"][1]["pinOrderSlots"] = json!([1, 2]);
        assert!(!valid_app(&order, 6100, 1.0));
        let mut slot = app();
        slot["stages"][3]["targetSlot"] = json!(2);
        assert!(!valid_app(&slot, 6100, 1.0));
        // A restore must not claim a pin increment requirement.
        let mut repinned = app();
        repinned["stages"][5]["pinIncrementObserved"] = json!(false);
        assert!(valid_app(&repinned, 6100, 1.0), "restore needs no pin");
        // Two sessions must be distinct and non-empty.
        let mut same = app();
        same["publicSessionIds"] = json!(["PUBLIC-session-a", "PUBLIC-session-a"]);
        assert!(!valid_app(&same, 6100, 1.0));
        let mut empty = app();
        empty["publicSessionIds"] = json!(["PUBLIC-session-a", ""]);
        assert!(!valid_app(&empty, 6100, 1.0));
        // The ticket serial must increase and the epoch stay 1.
        let mut serial = app();
        serial["stages"][2]["ticket"]["serial"] = json!(2);
        assert!(!valid_app(&serial, 6100, 1.0));
        let mut epoch = app();
        epoch["stages"][0]["ticket"]["epoch"] = json!(2);
        assert!(!valid_app(&epoch, 6100, 1.0));
        // Overclaims and a substituted pid are refused.
        let mut deletion = app();
        deletion["sessionDeletionRequested"] = json!(true);
        assert!(!valid_app(&deletion, 6100, 1.0));
        assert!(!valid_app(&app(), 6101, 1.0));
        assert!(
            !valid_app(&json!({}), 6100, 1.0),
            "a truncated report is not valid"
        );
    }
}

#[cfg(test)]
mod png_tests {
    use super::*;
    use std::io::Write;

    fn chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out.extend_from_slice(&crc32(&[kind, payload]).to_be_bytes());
        out
    }

    fn ihdr(width: u32, height: u32, depth: u8) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&width.to_be_bytes());
        payload.extend_from_slice(&height.to_be_bytes());
        payload.extend_from_slice(&[depth, 6, 0, 0, 0]);
        chunk(b"IHDR", &payload)
    }

    fn png(width: u32, height: u32, filter: u8) -> Vec<u8> {
        let stride = width as usize * 4 + 1;
        let mut rows = vec![0u8; stride * height as usize];
        for row in rows.chunks_mut(stride) {
            row[0] = filter;
        }
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&rows).unwrap();
        let idat = encoder.finish().unwrap();
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        out.extend(ihdr(width, height, 8));
        out.extend(chunk(b"IDAT", &idat));
        out.extend(chunk(b"IEND", &[]));
        out
    }

    #[test]
    fn a_writer_profile_png_needs_chunk_order_crcs_and_filter_tags() {
        assert_eq!(png_dimensions(&png(2, 3, 0)), Some((2, 3)));
        assert_eq!(png_dimensions(&png(1, 1, 4)), Some((1, 1)));
        // A filter tag above 4 is invalid.
        assert_eq!(png_dimensions(&png(1, 1, 5)), None);
        // A corrupted CRC is refused (the first chunk's CRC sits at 29..33).
        let mut corrupted = png(1, 1, 0);
        corrupted[29] ^= 1;
        assert_eq!(png_dimensions(&corrupted), None);
        // Only RGBA8 non-interlaced is the writer profile.
        let mut deep = b"\x89PNG\r\n\x1a\n".to_vec();
        deep.extend(ihdr(1, 1, 16));
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&[0, 0, 0, 0, 0]).unwrap();
        deep.extend(chunk(b"IDAT", &encoder.finish().unwrap()));
        deep.extend(chunk(b"IEND", &[]));
        assert_eq!(png_dimensions(&deep), None);
        // A palette after image data is not allowed.
        let mut late = b"\x89PNG\r\n\x1a\n".to_vec();
        late.extend(ihdr(1, 1, 8));
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&[0, 0, 0, 0, 0]).unwrap();
        late.extend(chunk(b"IDAT", &encoder.finish().unwrap()));
        late.extend(chunk(b"PLTE", &[0, 0, 0]));
        late.extend(chunk(b"IEND", &[]));
        assert_eq!(png_dimensions(&late), None);
        // An unknown critical chunk changes interpretation.
        let mut critical = png(1, 1, 0);
        let iend = critical.len() - 12;
        critical.splice(iend..iend, chunk(b"ABCD", &[]));
        assert_eq!(png_dimensions(&critical), None);
        // Truncation and trailing bytes are refused.
        let good = png(1, 1, 0);
        assert_eq!(png_dimensions(&good[..good.len() - 4]), None);
        let mut trailing = good.clone();
        trailing.push(0);
        assert_eq!(png_dimensions(&trailing), None);
        assert_eq!(png_dimensions(b""), None);
    }

    #[test]
    fn a_capture_needs_a_private_png_and_a_matching_reported_size() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "registry-png-").unwrap();
        let body = png(2, 3, 0);
        for name in ["archive-review.png", "restore-review.png"] {
            let path = base.join(name);
            fs::write(&path, &body).unwrap();
            fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))
                .unwrap();
        }
        let app = serde_json::json!({
            "archiveReviewCapture": {"saved": true, "manualInputProof": false, "physicalSize": [2, 3]},
            "archivedRestoreReviewCapture": {"saved": true, "manualInputProof": false, "physicalSize": [2, 3]},
        });
        assert!(valid_captures(&base, &app));
        // A public mode, a manual-input claim and a size mismatch are refused.
        fs::set_permissions(
            base.join("archive-review.png"),
            std::os::unix::fs::PermissionsExt::from_mode(0o644),
        )
        .unwrap();
        assert!(!valid_captures(&base, &app));
        fs::set_permissions(
            base.join("archive-review.png"),
            std::os::unix::fs::PermissionsExt::from_mode(0o600),
        )
        .unwrap();
        let mut claim = app.clone();
        claim["archiveReviewCapture"]["manualInputProof"] = serde_json::json!(true);
        assert!(!valid_captures(&base, &claim));
        let mut size = app.clone();
        size["archivedRestoreReviewCapture"]["physicalSize"] = serde_json::json!([3, 2]);
        assert!(!valid_captures(&base, &size));
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod registry_extra_tests {
    use super::*;
    use std::io::Write;

    fn chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out.extend_from_slice(&crc32(&[kind, payload]).to_be_bytes());
        out
    }

    fn ihdr(width: u32, height: u32, depth: u8, color: u8, interlace: u8) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&width.to_be_bytes());
        payload.extend_from_slice(&height.to_be_bytes());
        payload.extend_from_slice(&[depth, color, 0, 0, interlace]);
        chunk(b"IHDR", &payload)
    }

    fn deflate(rows: &[u8]) -> Vec<u8> {
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(rows).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn split_idat_and_ancillary_chunks_are_accepted() {
        let stream = deflate(&[0, 0, 0, 0, 0]);
        let (first, second) = stream.split_at(stream.len() / 2);
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend(ihdr(1, 1, 8, 6, 0));
        png.extend(chunk(b"tEXt", b"PUBLIC-comment"));
        png.extend(chunk(b"IDAT", first));
        png.extend(chunk(b"IDAT", second));
        png.extend(chunk(b"IEND", &[]));
        assert_eq!(png_dimensions(&png), Some((1, 1)));
    }

    #[test]
    fn unsupported_profiles_and_extra_expansion_are_refused() {
        for (depth, color, interlace) in [(16, 6, 0), (8, 2, 0), (8, 6, 1)] {
            let stream = deflate(&[0, 0, 0, 0, 0]);
            let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
            png.extend(ihdr(1, 1, depth, color, interlace));
            png.extend(chunk(b"IDAT", &stream));
            png.extend(chunk(b"IEND", &[]));
            assert_eq!(png_dimensions(&png), None, "{depth}/{color}/{interlace}");
        }
        // A stream that expands past the exact row budget is refused.
        let bomb = deflate(&vec![0u8; 5000]);
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend(ihdr(1, 1, 8, 6, 0));
        png.extend(chunk(b"IDAT", &bomb));
        png.extend(chunk(b"IEND", &[]));
        assert_eq!(png_dimensions(&png), None);
    }

    #[test]
    fn a_missing_capture_or_a_symlinked_report_is_refused() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "registry-app-").unwrap();
        let app =
            serde_json::json!({"archiveReviewCapture": {}, "archivedRestoreReviewCapture": {}});
        assert!(!valid_captures(&base, &app));
        let body = {
            let stream = deflate(&[0, 0, 0, 0, 0]);
            let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
            png.extend(ihdr(1, 1, 8, 6, 0));
            png.extend(chunk(b"IDAT", &stream));
            png.extend(chunk(b"IEND", &[]));
            png
        };
        fs::write(base.join("app.json"), &body).unwrap();
        fs::set_permissions(
            base.join("app.json"),
            std::os::unix::fs::PermissionsExt::from_mode(0o600),
        )
        .unwrap();
        // A non-JSON private body parses to the invalid stub, not a missing one.
        assert_eq!(read_app(&base)["status"], "invalid-app-report");
        fs::write(base.join("app.json"), b"{\"status\": \"passed\"}").unwrap();
        assert_eq!(read_app(&base)["status"], "passed");
        fs::remove_file(base.join("app.json")).unwrap();
        std::os::unix::fs::symlink(base.join("elsewhere.json"), base.join("app.json")).unwrap();
        assert_eq!(
            read_app(&base)["status"],
            "missing-or-invalid-private-app-report"
        );
        fs::remove_dir_all(base).unwrap();
    }
}
