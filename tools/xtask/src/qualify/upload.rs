//! Owned real SDK/raw-HTTP upload with independent local byte/mode verification.
//!
//! Port of `scripts/qualify-file-upload.py`. The storage check opens only static
//! fixture-derived paths, never a path returned by the upload: it verifies the
//! canonical object and its alias are the same inode, mode 0400, with the exact
//! bytes this test generated.
use super::composed::{OwnedRun, run_owned};
use serde_json::{Value, json};
use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const CASES: [&str; 5] = ["raw", "empty", "ceiling", "alias", "cold"];

pub fn name(case: &str) -> &'static str {
    match case {
        "raw" => "PUBLIC report é__.bin",
        "empty" => "file",
        "ceiling" => "PUBLIC ceiling.bin",
        "alias" => "PUBLIC alias.bin",
        "cold" => "PUBLIC cold.bin",
        _ => "",
    }
}

/// The exact bytes each fixture case uploads.
pub fn source(case: &str) -> Vec<u8> {
    match case {
        "raw" | "alias" => (0..4097usize)
            .map(|index| (index * 17 % 256) as u8)
            .collect(),
        "ceiling" => (0..4 * 1024 * 1024usize)
            .map(|index| ((index * 13 + 7) % 256) as u8)
            .collect(),
        "empty" => Vec::new(),
        "cold" => vec![80, 0, 255, 128, 10],
        _ => Vec::new(),
    }
}

/// A canonical lowercase UUID with the version-4 and variant bits set.
pub fn canonical_uuid_v4(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if *byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase() {
            return false;
        }
    }
    bytes[14] == b'4' && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
}

fn digest(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(data))
}

/// Every upload receipt, name and file digest must match the generated fixture,
/// and the two epoch snapshots must be exactly the expected shape.
pub fn valid_sdk(report: &Value) -> bool {
    let Some(uploads) = report["uploads"].as_array() else {
        return false;
    };
    let base = json!(["permission/preset", "sandbox/mode", "approval/policy"]);
    let mut epoch = base.as_array().cloned().unwrap_or_default();
    epoch.push(json!("session/end-seed"));
    let stops_ok = report["stops"].as_array().is_some_and(|stops| {
        stops.len() == 2
            && stops.iter().all(|stop| {
                stop["exited"].as_bool() == Some(true)
                    && stop["graceful"].as_bool() == Some(true)
                    && stop["exitCode"].as_i64() == Some(0)
                    && stop["containmentUnknown"].as_bool() == Some(false)
                    && stop["observedDescendantsRemaining"].as_i64() == Some(0)
            })
    });
    if report["status"].as_str() != Some("passed")
        || report["versions"] != json!(["0.2.1-alpha.1", "0.2.1-alpha.1"])
        || report["explicitModelPrompts"].as_i64() != Some(0)
        || [
            "modelCatalogRequested",
            "processWideNetworkTrace",
            "uiExercised",
        ]
        .iter()
        .any(|key| report[*key].as_bool() != Some(false))
        || [
            "liveAgentObserved",
            "missingTargetRefused",
            "coldBeforeUpload",
            "resumedAfterUpload",
        ]
        .iter()
        .any(|key| report[*key].as_bool() != Some(true))
        || !stops_ok
        || report["postUploadSnapshots"] != json!([base, epoch])
        || uploads.len() != CASES.len()
    {
        return false;
    }
    let mut receipts = std::collections::BTreeSet::new();
    for (index, entry) in uploads.iter().enumerate() {
        let case = CASES[index];
        if entry["case"].as_str() != Some(case) {
            return false;
        }
        let value = &entry["value"];
        let Some(receipt) = value["receiptId"].as_str() else {
            return false;
        };
        if !canonical_uuid_v4(receipt) || !receipts.insert(receipt.to_string()) {
            return false;
        }
        let data = source(case);
        let file = &value["file"];
        if value.as_object().map(|object| object.len()) != Some(2)
            || value.get("receiptId").is_none()
            || value.get("file").is_none()
            || file.as_object().map(|object| object.len()) != Some(3)
            || file["name"].as_str() != Some(name(case))
            || file["bytes"].as_i64() != Some(data.len() as i64)
            || file["attachmentId"].as_str() != Some(&format!("sha256:{}", digest(&data)))
        {
            return false;
        }
    }
    true
}

/// Read one file with `O_NOFOLLOW`, requiring a private 0400 regular file whose
/// body is exactly `data`; return its device and inode.
pub fn verify_file(path: &Path, data: &[u8]) -> Result<(u64, u64), String> {
    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| "path-with-nul")?;
    // Safe: a NUL-terminated path and constant flags; the descriptor is closed below.
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err("file-not-readable-without-following-links".into());
    }
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // Safe: a live descriptor and a valid out-parameter.
    if unsafe { libc::fstat(fd, &mut stat) } != 0 {
        unsafe { libc::close(fd) };
        return Err("cannot-stat-file".into());
    }
    let mode = stat.st_mode & 0o777;
    let regular = stat.st_mode & libc::S_IFMT == libc::S_IFREG;
    if !regular || mode != 0o400 || stat.st_size as usize != data.len() {
        unsafe { libc::close(fd) };
        return Err("file-size-or-mode".into());
    }
    let mut offset = 0usize;
    let mut buffer = vec![0u8; 65536];
    loop {
        // Safe: a live descriptor, a writable buffer and its length.
        let read = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
        if read < 0 {
            unsafe { libc::close(fd) };
            return Err("short-read".into());
        }
        if read == 0 {
            break;
        }
        let chunk = &buffer[..read as usize];
        if data.get(offset..offset + chunk.len()) != Some(chunk) {
            unsafe { libc::close(fd) };
            return Err("independent-byte-mismatch".into());
        }
        offset += chunk.len();
    }
    unsafe { libc::close(fd) };
    if offset != data.len() {
        return Err("incomplete-independent-read".into());
    }
    Ok((stat.st_dev, stat.st_ino))
}

fn private_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        use std::os::unix::fs::MetadataExt;
        metadata.file_type().is_dir() && metadata.mode() & 0o777 == 0o700
    })
}

/// Verify every fixture's canonical object and alias under the private store.
pub fn verify_local_cases(
    output: &Path,
    fixtures: &[(&str, Vec<u8>, &str)],
) -> Result<Vec<Value>, String> {
    let root = output.join("harness/attachments/v1");
    let mut verified = Vec::new();
    for (case, data, name) in fixtures {
        let digest = digest(data);
        let canonical = root.join("file-objects").join(&digest[..2]).join(&digest);
        let alias = root
            .join("files")
            .join(&digest[..2])
            .join(&digest)
            .join(name);
        let mut directories: Vec<PathBuf> = vec![
            output.to_path_buf(),
            output.join("harness"),
            output.join("harness/attachments"),
            root.clone(),
        ];
        for relative in [canonical.strip_prefix(&root), alias.strip_prefix(&root)] {
            let Ok(relative) = relative else {
                return Err("fixture-path-outside-store".into());
            };
            let mut current = root.clone();
            for component in relative.parent().into_iter().flat_map(Path::components) {
                current = current.join(component);
                directories.push(current.clone());
            }
        }
        for directory in &directories {
            if !private_directory(directory) {
                return Err("private-directory-mode-or-link".into());
            }
        }
        let canonical_identity = verify_file(&canonical, data)?;
        if verify_file(&alias, data)? != canonical_identity {
            return Err("alias-not-hardlinked-to-canonical".into());
        }
        verified.push(json!({
            "case": case,
            "sha256": digest,
            "bytes": data.len(),
            "rawBytesVerified": true,
            "canonicalAndAliasMode": "0400",
            "privateDirectories": true,
            "sameInode": true,
        }));
    }
    Ok(verified)
}

/// `xtask qualify file-upload`.
pub fn run(root: &Path, output: &Path) -> i32 {
    let binary = root.join("core/target/release/examples/file_upload_smoke");
    let runtime = root
        .parent()
        .map(|parent| parent.join("deepseek-harness-linux/apps/cli"))
        .unwrap_or_default();
    let spec = OwnedRun {
        script: &binary,
        report_name: "sdk.json",
        report_key: "sdk",
        evidence_env: "DSH_NATIVE_UPLOAD_EVIDENCE",
        scope: Some(
            "owned two-epoch public alpha composition and Rust SDK/raw HTTP; post-upload snapshots only, no process-wide trace or GUI",
        ),
        command: {
            let binary = binary.to_string_lossy().into_owned();
            let runtime = runtime.to_string_lossy().into_owned();
            Box::new(move |output: &Path| {
                vec![
                    binary.clone(),
                    "--runtime".into(),
                    runtime.clone(),
                    "--expected-version".into(),
                    "0.2.1-alpha.1".into(),
                    "--home".into(),
                    output.join("harness").to_string_lossy().into_owned(),
                    "--cwd".into(),
                    output.join("workspace").to_string_lossy().into_owned(),
                    "--user-home".into(),
                    output.join("user").to_string_lossy().into_owned(),
                ]
            })
        },
        capture_stdout: true,
        timeout: Duration::from_secs(35),
    };
    let owned = match run_owned(root, output, &spec, valid_sdk) {
        Ok(owned) => owned,
        Err(message) => {
            eprintln!("file-upload qualification failed: {message}");
            return 1;
        }
    };
    let resolved = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|d| d.join(output))
            .unwrap_or_else(|_| output.to_path_buf())
    };
    let fixtures: Vec<(&str, Vec<u8>, &str)> = CASES
        .iter()
        .map(|case| (*case, source(case), name(case)))
        .collect();
    let mut report = json!({
        "status": "failed",
        "ownedSdkStatus": owned["status"],
        "storage": [],
        "scope": "independent private default LocalAttachmentStore bytes, names, deduplication, modes; not other providers, receipt prompt binding, model/image validation, GUI or complete rollback",
    });
    if owned["status"].as_str() == Some("passed") {
        match verify_local_cases(&resolved, &fixtures) {
            Ok(storage) => {
                report["storage"] = json!(storage);
                report["status"] = json!("passed");
            }
            Err(error) => {
                report["error"] = json!(error);
            }
        }
    } else {
        report["error"] = json!("owned-sdk-failed");
    }
    if let Err(error) = super::private_json(&resolved.join("upload-qualification.json"), &report) {
        eprintln!("cannot write the storage report: {error}");
        return 1;
    }
    println!(
        "{}",
        json!({"status": report["status"], "report": resolved.join("upload-qualification.json").to_string_lossy()})
    );
    if report["status"].as_str() == Some("passed") {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempdir::make_temp_dir;

    fn uploads() -> Value {
        let entries: Vec<Value> = CASES
            .iter()
            .enumerate()
            .map(|(index, case)| {
                let data = source(case);
                let receipt = format!("12345678-1234-4abc-8def-{:012x}", index + 1);
                json!({
                    "case": case,
                    "value": {
                        "receiptId": receipt,
                        "file": {
                            "attachmentId": format!("sha256:{}", digest(&data)),
                            "name": name(case),
                            "bytes": data.len(),
                        }
                    }
                })
            })
            .collect();
        json!({
            "status": "passed", "versions": ["0.2.1-alpha.1", "0.2.1-alpha.1"],
            "explicitModelPrompts": 0, "modelCatalogRequested": false,
            "processWideNetworkTrace": false, "uiExercised": false,
            "liveAgentObserved": true, "missingTargetRefused": true,
            "coldBeforeUpload": true, "resumedAfterUpload": true,
            "stops": [{"exited": true, "graceful": true, "exitCode": 0,
                       "containmentUnknown": false, "observedDescendantsRemaining": 0},
                      {"exited": true, "graceful": true, "exitCode": 0,
                       "containmentUnknown": false, "observedDescendantsRemaining": 0}],
            "postUploadSnapshots": [
                ["permission/preset", "sandbox/mode", "approval/policy"],
                ["permission/preset", "sandbox/mode", "approval/policy", "session/end-seed"]
            ],
            "uploads": entries,
        })
    }

    #[test]
    fn an_upload_report_needs_every_fixture_receipt_name_and_digest() {
        assert!(valid_sdk(&uploads()));
        let mut duplicate = uploads();
        let first = duplicate["uploads"][0]["value"]["receiptId"].clone();
        duplicate["uploads"][1]["value"]["receiptId"] = first;
        assert!(!valid_sdk(&duplicate), "receipts must be unique");
        let mut wrong_name = uploads();
        wrong_name["uploads"][0]["value"]["file"]["name"] = json!("other.bin");
        assert!(!valid_sdk(&wrong_name));
        let mut wrong_bytes = uploads();
        wrong_bytes["uploads"][2]["value"]["file"]["bytes"] = json!(1);
        assert!(!valid_sdk(&wrong_bytes));
        let mut reordered = uploads();
        let second = reordered["uploads"][1].clone();
        reordered["uploads"] = json!([second]);
        assert!(!valid_sdk(&reordered));
        let mut bad_uuid = uploads();
        bad_uuid["uploads"][0]["value"]["receiptId"] = json!("not-a-uuid");
        assert!(!valid_sdk(&bad_uuid));
    }

    #[test]
    fn canonical_uuids_are_lowercase_version_four_only() {
        assert!(canonical_uuid_v4("12345678-1234-4abc-8def-000000000001"));
        assert!(!canonical_uuid_v4("12345678-1234-5abc-8def-000000000001"));
        assert!(!canonical_uuid_v4("12345678-1234-4abc-7def-000000000001"));
        assert!(!canonical_uuid_v4("12345678-1234-4ABC-8def-000000000001"));
        assert!(!canonical_uuid_v4("1234567812344abc8def000000000001"));
    }

    #[test]
    fn storage_verification_requires_private_directories_and_a_hardlinked_alias() {
        let base = make_temp_dir(&std::env::temp_dir(), "upload-store-").unwrap();
        let data = b"PUBLIC fixture".to_vec();
        let digest = digest(&data);
        let root = base.join("harness/attachments/v1");
        use std::os::unix::fs::PermissionsExt;
        for directory in [
            base.clone(),
            base.join("harness"),
            base.join("harness/attachments"),
            root.clone(),
            root.join("file-objects"),
            root.join(format!("file-objects/{}", &digest[..2])),
            root.join("files"),
            root.join(format!("files/{}", &digest[..2])),
            root.join(format!("files/{}/{}", &digest[..2], digest)),
        ] {
            fs::create_dir_all(&directory).unwrap();
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let canonical = root.join(format!("file-objects/{}/{}", &digest[..2], digest));
        fs::write(&canonical, &data).unwrap();
        fs::set_permissions(&canonical, fs::Permissions::from_mode(0o400)).unwrap();
        let alias = root.join(format!("files/{}/{}/{}", &digest[..2], digest, "file"));
        fs::hard_link(&canonical, &alias).unwrap();
        let fixtures = [("cold", data.clone(), "file")];
        assert!(
            verify_local_cases(&base, &fixtures).is_ok(),
            "private store passes"
        );
        // A non-hardlinked alias is rejected.
        fs::remove_file(&alias).unwrap();
        fs::write(&alias, &data).unwrap();
        fs::set_permissions(&alias, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(verify_local_cases(&base, &fixtures).is_err());
        fs::remove_dir_all(base).unwrap();
    }
}
