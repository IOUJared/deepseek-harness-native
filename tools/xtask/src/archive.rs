//! Verify and extract an own development archive into a new evidence directory.
//!
//! Port of `scripts/verify-development-archive.py`. It is deliberately an
//! own-builder check, not adversarial streaming protection: every bound applies
//! after the archive is parsed into members. It verifies the generated payload,
//! extracts under explicit modes (independent of umask), then re-hashes the
//! extracted tree and its source inventory.
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use tar::Archive;

pub const MAX_MEMBERS: usize = 2048;
pub const MAX_EXPANDED: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    pub directory: bool,
    pub mode: u32,
    pub size: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub status: &'static str,
    pub archive_sha256: String,
    pub member_count: usize,
    pub expanded_bytes: u64,
    pub native_source_files: usize,
    pub complete_generated_payload_and_extracted_hashes_match: bool,
    pub source_inventory_complete: bool,
    pub no_links_special_files_duplicate_or_unsafe_members: bool,
    pub signature_or_adversarial_source_protection_claimed: bool,
    pub extracted_package: String,
}

fn unsafe_name(name: &str) -> bool {
    name.chars().any(|c| (c as u32) < 0x20 || c == '\u{7f}')
}

/// Reject anything that is not a bounded, canonical, regular directory tree under
/// `root`: wrong root, absolute paths, `..`, `//`, `./`, backslashes, control
/// characters, duplicates, links/special files, non-package modes and oversize
/// expansion.
pub fn validate_members(members: &[Member], root: &str) -> Result<u64, String> {
    if !(1..=MAX_MEMBERS).contains(&members.len()) {
        return Err("Archive member count exceeds bound".into());
    }
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    let mut root_directory = false;
    for member in members {
        let path = Path::new(&member.name);
        let canonical = path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
        let parts: Vec<&str> = member.name.split('/').collect();
        if path.is_absolute()
            || !canonical
            || parts.first() != Some(&root)
            || !parts
                .iter()
                .all(|part| !part.is_empty() && *part != "." && *part != "..")
            || member.name.contains('\\')
            || unsafe_name(&member.name)
            || member.name.ends_with('/')
            || !seen.insert(member.name.clone())
            || (member.directory && (member.mode != 0o755 || member.size != 0))
            || (!member.directory && member.mode != 0o644 && member.mode != 0o755)
            || (member.name == root && !member.directory)
            || member.size > MAX_EXPANDED
        {
            return Err("Unsafe or duplicate archive member".into());
        }
        root_directory |= member.name == root && member.directory;
        total += member.size;
        if total > MAX_EXPANDED {
            return Err("Expanded archive exceeds bound".into());
        }
    }
    if !root_directory {
        return Err("Missing expected archive root directory".into());
    }
    Ok(total)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub digest: Option<String>,
    pub mode: u32,
}

/// Map `<root.name>/relative` to digest and mode. `normalize_modes` mirrors the
/// builder's execute criterion (`os.access(X_OK)`), not just owner execute bits.
pub fn snapshot(root: &Path, normalize_modes: bool) -> Result<BTreeMap<String, Entry>, String> {
    let link = fs::symlink_metadata(root).map_err(|error| error.to_string())?;
    if link.is_symlink() || !link.is_dir() {
        return Err("Expected snapshot root directory".into());
    }
    let parent = root.parent().ok_or("snapshot root has no parent")?;
    let mut result = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("Unexpected snapshot object".into());
        }
        if !metadata.is_dir() && !metadata.is_file() {
            return Err("Unexpected snapshot object".into());
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(&path).map_err(|error| error.to_string())? {
                pending.push(entry.map_err(|error| error.to_string())?.path());
            }
        }
        let name = path
            .strip_prefix(parent)
            .map_err(|_| "snapshot entry outside its root")?
            .to_string_lossy()
            .into_owned();
        let digest = if metadata.is_file() {
            Some(crate::sha256_file(&path).map_err(|error| error.to_string())?)
        } else {
            None
        };
        let mode = if normalize_modes {
            if metadata.is_dir() || executable(&path) {
                0o755
            } else {
                0o644
            }
        } else {
            metadata.permissions().mode() & 0o777
        };
        result.insert(name, Entry { digest, mode });
    }
    Ok(result)
}

fn executable(path: &Path) -> bool {
    let Ok(c_path) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) else {
        return false;
    };
    // Safe: a NUL-terminated path and a constant access mode.
    unsafe { libc::access(c_path.as_ptr(), libc::X_OK) == 0 }
}

/// Validate, extract, and re-verify one archive against the staged payload.
pub fn verify(
    archive: &Path,
    destination: &Path,
    expected: &Path,
    evidence_root: &Path,
) -> Result<Report, String> {
    let destination = if destination.is_absolute() {
        destination.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(destination))
            .unwrap_or_else(|_| destination.to_path_buf())
    };
    let allowed = evidence_root
        .canonicalize()
        .unwrap_or_else(|_| evidence_root.to_path_buf());
    if fs::symlink_metadata(&destination).is_ok()
        || fs::symlink_metadata(archive)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        || fs::symlink_metadata(expected)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        || !destination
            .parent()
            .is_some_and(|parent| parent.starts_with(&allowed))
    {
        return Err("Only a new owned evidence extraction is accepted".into());
    }
    let expected_tree = snapshot(expected, true)?;
    let root_name = expected
        .file_name()
        .ok_or("expected payload has no name")?
        .to_string_lossy()
        .into_owned();
    for name in ["launch.py", "bin/dsh-native-app"] {
        let key = format!("{root_name}/{name}");
        let entry = expected_tree.get(&key);
        if entry.is_none_or(|entry| entry.digest.is_none() || entry.mode != 0o755) {
            return Err("Required package entry point is not executable".into());
        }
    }

    let file = fs::File::open(archive).map_err(|error| error.to_string())?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut tar = Archive::new(decoder);

    // First pass: validate the member list and hash every body.
    let mut members: Vec<Member> = Vec::new();
    let mut actual: BTreeMap<String, Entry> = BTreeMap::new();
    for entry in tar.entries().map_err(|error| error.to_string())? {
        let mut entry = entry.map_err(|error| error.to_string())?;
        let header = entry.header().clone();
        let raw = String::from_utf8(entry.path_bytes().to_vec())
            .map_err(|_| "Archive member name is not UTF-8".to_string())?;
        let directory = header.entry_type().is_dir();
        let regular = header.entry_type().is_file();
        // `tarfile` strips the directory marker; keep the same canonical names.
        let name = if directory {
            raw.trim_end_matches('/').to_string()
        } else {
            raw
        };
        if !directory && !regular {
            return Err("Unsafe or duplicate archive member".into());
        }
        let mode = header.mode().map_err(|error| error.to_string())?;
        let size = header.size().map_err(|error| error.to_string())?;
        members.push(Member {
            name: name.clone(),
            directory,
            mode,
            size,
        });
        let digest = if directory {
            None
        } else {
            let mut hasher = Sha256::new();
            let mut counted = 0u64;
            let mut chunk = [0u8; 64 * 1024];
            loop {
                let read = entry.read(&mut chunk).map_err(|error| error.to_string())?;
                if read == 0 {
                    break;
                }
                counted += read as u64;
                hasher.update(&chunk[..read]);
            }
            if counted != size {
                return Err("Incomplete archive member".into());
            }
            Some(format!("{:x}", hasher.finalize()))
        };
        actual.insert(name, Entry { digest, mode });
    }
    let total = validate_members(&members, &root_name)?;
    if actual != expected_tree {
        return Err("Archive differs from own generated payload".into());
    }

    // Second pass: extract under explicit modes, independent of the umask.
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&destination)
        .map_err(|error| error.to_string())?;
    let file = fs::File::open(archive).map_err(|error| error.to_string())?;
    let mut tar = Archive::new(flate2::read::GzDecoder::new(file));
    for entry in tar.entries().map_err(|error| error.to_string())? {
        let mut entry = entry.map_err(|error| error.to_string())?;
        let header = entry.header().clone();
        let raw = String::from_utf8(entry.path_bytes().to_vec())
            .map_err(|_| "Archive member name is not UTF-8".to_string())?;
        let name = if header.entry_type().is_dir() {
            raw.trim_end_matches('/').to_string()
        } else {
            raw
        };
        let target = destination.join(&name);
        if header.entry_type().is_dir() {
            fs::create_dir_all(&target).map_err(|error| error.to_string())?;
            fs::set_permissions(&target, fs::Permissions::from_mode(0o755))
                .map_err(|error| error.to_string())?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            let mut stream = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&target)
                .map_err(|error| error.to_string())?;
            io::copy(&mut entry, &mut stream).map_err(|error| error.to_string())?;
            let mode = if header.mode().unwrap_or(0o644) == 0o755 {
                0o755
            } else {
                0o644
            };
            fs::set_permissions(&target, fs::Permissions::from_mode(mode))
                .map_err(|error| error.to_string())?;
        }
    }

    let extracted = destination.join(&root_name);
    let extracted_tree = snapshot(&extracted, false)?;
    if extracted_tree != expected_tree {
        return Err("Extracted payload differs".into());
    }
    let records = validate_inventory(&extracted, &extracted_tree, &root_name)?;
    let archive_sha256 = crate::sha256_file(archive).map_err(|error| error.to_string())?;
    let report = Report {
        status: "passed",
        archive_sha256,
        member_count: members.len(),
        expanded_bytes: total,
        native_source_files: records,
        complete_generated_payload_and_extracted_hashes_match: true,
        source_inventory_complete: true,
        no_links_special_files_duplicate_or_unsafe_members: true,
        signature_or_adversarial_source_protection_claimed: false,
        extracted_package: extracted.to_string_lossy().into_owned(),
    };
    let rendered = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
    let path = destination.join("extraction-result.json");
    let mut writer = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|error| error.to_string())?;
    writer
        .write_all((rendered + "\n").as_bytes())
        .map_err(|error| error.to_string())?;
    println!("{}", serde_json::to_string(&report).unwrap_or_default());
    Ok(report)
}

/// The `source-snapshot.json` inventory must describe exactly the extracted
/// `source/native` files, with matching sizes and digests.
pub fn validate_inventory(
    extracted: &Path,
    extracted_tree: &BTreeMap<String, Entry>,
    root_name: &str,
) -> Result<usize, String> {
    let text = fs::read_to_string(extracted.join("source-snapshot.json"))
        .map_err(|error| error.to_string())?;
    let inventory: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let count = inventory
        .get("fileCount")
        .and_then(Value::as_u64)
        .ok_or("Invalid source snapshot inventory")?;
    let records = inventory
        .get("files")
        .and_then(Value::as_array)
        .ok_or("Invalid source snapshot inventory")?;
    let prefix = format!("{root_name}/");
    let source_prefix = format!("{prefix}source/native/");
    let source_paths: BTreeSet<String> = extracted_tree
        .iter()
        .filter(|(name, entry)| name.starts_with(&source_prefix) && entry.digest.is_some())
        .map(|(name, _)| name[prefix.len()..].to_string())
        .collect();
    if records.len() > MAX_MEMBERS
        || records.len() as u64 != count
        || records.len() != source_paths.len()
    {
        return Err("Incomplete source snapshot inventory".into());
    }
    let mut seen = BTreeSet::new();
    for record in records {
        let name = record
            .get("path")
            .and_then(Value::as_str)
            .ok_or("Invalid source snapshot path")?;
        let parts: Vec<&str> = name.split('/').collect();
        let canonical = Path::new(name)
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
        let bytes = record.get("bytes").and_then(Value::as_u64);
        let digest = record.get("sha256").and_then(Value::as_str);
        if parts.len() < 3
            || parts[0] != "source"
            || parts[1] != "native"
            || !canonical
            || parts.iter().any(|part| *part == ".." || *part == ".")
            || name.contains('\\')
            || unsafe_name(name)
            || !source_paths.contains(name)
            || !seen.insert(name.to_string())
            || bytes.is_none()
            || digest.is_none()
            || extracted_tree
                .get(&format!("{prefix}{name}"))
                .and_then(|entry| entry.digest.clone())
                != digest.map(str::to_string)
        {
            return Err("Source inventory mismatch".into());
        }
        let data = fs::read(extracted.join(name)).map_err(|error| error.to_string())?;
        if data.len() as u64 != bytes.unwrap_or_default()
            || format!("{:x}", Sha256::digest(&data)) != digest.unwrap_or_default()
        {
            return Err("Source inventory mismatch".into());
        }
    }
    if seen != source_paths {
        return Err("Incomplete source snapshot inventory".into());
    }
    Ok(records.len())
}

fn usage() -> &'static str {
    "usage: xtask archive verify --archive PATH --destination NEW_DIR --expected-payload DIR"
}

fn rejected(message: &str) -> i32 {
    eprintln!("{}\n{message}", usage());
    2
}

/// `xtask archive verify ...`.
pub fn run(arguments: &[String]) -> i32 {
    if arguments.first().map(String::as_str) != Some("verify") {
        return rejected("expected the `verify` command");
    }
    let mut archive: Option<PathBuf> = None;
    let mut destination: Option<PathBuf> = None;
    let mut expected: Option<PathBuf> = None;
    let mut arguments = arguments[1..].iter();
    while let Some(argument) = arguments.next() {
        let mut value = |name: &str| {
            arguments
                .next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        let parsed: Result<(), String> = (|| match argument.as_str() {
            "--archive" => {
                archive = Some(PathBuf::from(value("--archive")?));
                Ok(())
            }
            "--destination" => {
                destination = Some(PathBuf::from(value("--destination")?));
                Ok(())
            }
            "--expected-payload" => {
                expected = Some(PathBuf::from(value("--expected-payload")?));
                Ok(())
            }
            other => Err(format!("unexpected argument: {other}")),
        })();
        if let Err(message) = parsed {
            return rejected(&message);
        }
    }
    let (Some(archive), Some(destination), Some(expected)) = (archive, destination, expected)
    else {
        return rejected("--archive, --destination and --expected-payload are required");
    };
    let evidence = crate::root().join("app/evidence");
    match verify(&archive, &destination, &expected, &evidence) {
        Ok(_) => 0,
        Err(message) => {
            eprintln!("archive verification failed: {message}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempdir::make_temp_dir;
    use tar::{Builder, EntryType, Header};

    const ROOT: &str = "dsh-native-linux-x86_64-development";

    /// One archive member: `None` is a directory, `Some` a regular file.
    struct File {
        name: String,
        body: Option<Vec<u8>>,
        mode: u32,
    }

    fn member(name: &str, directory: bool, mode: u32) -> Member {
        Member {
            name: name.to_string(),
            directory,
            mode,
            size: if directory { 0 } else { 4 },
        }
    }

    fn payload() -> Vec<File> {
        let source = b"PUBLIC source\n".to_vec();
        let digest = format!("{:x}", Sha256::digest(&source));
        let inventory = format!(
            "{{\"fileCount\": 1, \"files\": [{{\"path\": \"source/native/app/main.rs\", \"bytes\": {}, \"sha256\": \"{digest}\"}}]}}\n",
            source.len()
        );
        vec![
            File {
                name: ROOT.to_string(),
                body: None,
                mode: 0o755,
            },
            File {
                name: format!("{ROOT}/bin"),
                body: None,
                mode: 0o755,
            },
            File {
                name: format!("{ROOT}/bin/dsh-native-app"),
                body: Some(b"#!public binary\n".to_vec()),
                mode: 0o755,
            },
            File {
                name: format!("{ROOT}/launch.py"),
                body: Some(b"#!public launcher\n".to_vec()),
                mode: 0o755,
            },
            File {
                name: format!("{ROOT}/source"),
                body: None,
                mode: 0o755,
            },
            File {
                name: format!("{ROOT}/source/native"),
                body: None,
                mode: 0o755,
            },
            File {
                name: format!("{ROOT}/source/native/app"),
                body: None,
                mode: 0o755,
            },
            File {
                name: format!("{ROOT}/source/native/app/main.rs"),
                body: Some(source),
                mode: 0o644,
            },
            File {
                name: format!("{ROOT}/source-snapshot.json"),
                body: Some(inventory.into_bytes()),
                mode: 0o644,
            },
        ]
    }

    /// Materialize the same listing as a directory tree and as a .tar.gz.
    fn fixture(base: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let staged = base.join(ROOT);
        let archive = base.join("package.tar.gz");
        let mut builder = Builder::new(flate2::write::GzEncoder::new(
            fs::File::create(&archive).unwrap(),
            flate2::Compression::default(),
        ));
        for file in payload() {
            let path = base.join(&file.name);
            let relative = path.strip_prefix(base).unwrap();
            match file.body {
                None => {
                    fs::create_dir_all(&path).unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(file.mode)).unwrap();
                    let mut header = Header::new_gnu();
                    header.set_entry_type(EntryType::Directory);
                    header.set_size(0);
                    header.set_mode(file.mode);
                    header.set_cksum();
                    builder
                        .append_data(&mut header, relative, io::empty())
                        .unwrap();
                }
                Some(body) => {
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    fs::write(&path, &body).unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(file.mode)).unwrap();
                    let mut header = Header::new_gnu();
                    header.set_size(body.len() as u64);
                    header.set_mode(file.mode);
                    header.set_cksum();
                    builder
                        .append_data(&mut header, relative, body.as_slice())
                        .unwrap();
                }
            }
        }
        builder.into_inner().unwrap().finish().unwrap();
        let evidence = base.join("evidence");
        fs::create_dir_all(&evidence).unwrap();
        (staged, archive, evidence)
    }

    #[test]
    fn unsafe_or_duplicate_member_lists_are_rejected() {
        let valid = vec![
            member(ROOT, true, 0o755),
            member(&format!("{ROOT}/bin/dsh-native-app"), false, 0o755),
        ];
        assert_eq!(validate_members(&valid, ROOT), Ok(4));
        // Wrong root, absolute path, parent escape, duplicate, link-like mode, directory with size.
        for broken in [
            member("another-root", true, 0o755),
            member(&format!("/{ROOT}/bin"), true, 0o755),
            member(&format!("{ROOT}/../escape"), false, 0o644),
            member(&format!("{ROOT}/bin/dsh-native-app"), false, 0o755),
        ] {
            let mut members = valid.clone();
            members.push(broken);
            assert!(validate_members(&members, ROOT).is_err());
        }
        let mut duplicate = valid.clone();
        duplicate.push(member(&format!("{ROOT}/bin/dsh-native-app"), false, 0o755));
        assert!(validate_members(&duplicate, ROOT).is_err());
        let mut wrong_mode = valid.clone();
        wrong_mode.push(member(&format!("{ROOT}/README.txt"), false, 0o600));
        assert!(validate_members(&wrong_mode, ROOT).is_err());
        let mut directory_size = valid.clone();
        let mut sized = member(&format!("{ROOT}/bin"), true, 0o755);
        sized.size = 1;
        directory_size.push(sized);
        assert!(validate_members(&directory_size, ROOT).is_err());
        let mut missing_root = vec![member(&format!("{ROOT}/bin"), true, 0o755)];
        missing_root.push(member(&format!("{ROOT}/README.txt"), false, 0o644));
        assert!(validate_members(&missing_root, ROOT).is_err());
    }

    #[test]
    fn an_empty_or_over_full_member_list_is_rejected() {
        assert!(validate_members(&[], ROOT).is_err());
        let mut many = vec![member(ROOT, true, 0o755)];
        for index in 0..MAX_MEMBERS {
            many.push(member(&format!("{ROOT}/f{index}"), false, 0o644));
        }
        assert!(validate_members(&many, ROOT).is_err());
    }

    #[test]
    fn a_matching_archive_round_trips_and_reports_its_inventory() {
        let base = make_temp_dir(&std::env::temp_dir(), "archive-").unwrap();
        let (staged, archive, evidence) = fixture(&base);
        let destination = evidence.join("extracted");
        let report = verify(&archive, &destination, &staged, &evidence).unwrap();
        assert_eq!(report.status, "passed");
        assert_eq!(report.member_count, 9);
        assert_eq!(report.native_source_files, 1);
        assert!(destination.join(ROOT).join("bin/dsh-native-app").exists());
        let written = fs::read_to_string(destination.join("extraction-result.json")).unwrap();
        assert!(
            written.contains("\"sourceInventoryComplete\": true"),
            "{written}"
        );
        assert_eq!(
            fs::metadata(destination.join("extraction-result.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        // Extraction normalizes modes even under a restrictive umask.
        assert_eq!(
            fs::metadata(destination.join(ROOT).join("bin/dsh-native-app"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn an_archive_that_differs_from_the_staged_payload_is_rejected() {
        let base = make_temp_dir(&std::env::temp_dir(), "archive-diff-").unwrap();
        let (staged, archive, evidence) = fixture(&base);
        fs::write(base.join(ROOT).join("launch.py"), b"#!different\n").unwrap();
        assert_eq!(
            verify(&archive, &evidence.join("out"), &staged, &evidence).unwrap_err(),
            "Archive differs from own generated payload"
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_non_executable_entry_point_is_rejected() {
        let base = make_temp_dir(&std::env::temp_dir(), "archive-entry-").unwrap();
        let (staged, archive, evidence) = fixture(&base);
        fs::set_permissions(
            base.join(ROOT).join("launch.py"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert_eq!(
            verify(&archive, &evidence.join("out"), &staged, &evidence).unwrap_err(),
            "Required package entry point is not executable"
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn an_incomplete_source_inventory_is_rejected() {
        let base = make_temp_dir(&std::env::temp_dir(), "archive-inventory-").unwrap();
        let (staged, archive, evidence) = fixture(&base);
        let short = "{\"fileCount\": 0, \"files\": []}\n";
        fs::write(base.join(ROOT).join("source-snapshot.json"), short).unwrap();
        // Rebuild the archive from the changed staged tree so only the inventory differs.
        let mut builder = Builder::new(flate2::write::GzEncoder::new(
            fs::File::create(&archive).unwrap(),
            flate2::Compression::default(),
        ));
        for file in payload() {
            let path = base.join(&file.name);
            let relative = path.strip_prefix(&base).unwrap();
            match file.body {
                None => {
                    let mut header = Header::new_gnu();
                    header.set_entry_type(EntryType::Directory);
                    header.set_size(0);
                    header.set_mode(file.mode);
                    header.set_cksum();
                    builder
                        .append_data(&mut header, relative, io::empty())
                        .unwrap();
                }
                Some(_) => {
                    let body = fs::read(&path).unwrap();
                    let mut header = Header::new_gnu();
                    header.set_size(body.len() as u64);
                    header.set_mode(file.mode);
                    header.set_cksum();
                    builder
                        .append_data(&mut header, relative, body.as_slice())
                        .unwrap();
                }
            }
        }
        builder.into_inner().unwrap().finish().unwrap();
        let error = verify(&archive, &evidence.join("out"), &staged, &evidence).unwrap_err();
        assert!(
            error.contains("Incomplete source snapshot inventory"),
            "{error}"
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_destination_outside_the_evidence_root_is_rejected() {
        let base = make_temp_dir(&std::env::temp_dir(), "archive-outside-").unwrap();
        let (staged, archive, evidence) = fixture(&base);
        let outside = base.join("elsewhere");
        assert_eq!(
            verify(&archive, &outside, &staged, &evidence).unwrap_err(),
            "Only a new owned evidence extraction is accepted"
        );
        // An existing destination is refused too.
        fs::create_dir_all(evidence.join("taken")).unwrap();
        assert_eq!(
            verify(&archive, &evidence.join("taken"), &staged, &evidence).unwrap_err(),
            "Only a new owned evidence extraction is accepted"
        );
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod inventory_tests {
    use super::*;
    use crate::tempdir::make_temp_dir;

    const ROOT: &str = "dsh-native-linux-x86_64-development";

    fn member(name: &str, directory: bool, mode: u32) -> Member {
        Member {
            name: name.to_string(),
            directory,
            mode,
            size: if directory { 0 } else { 4 },
        }
    }

    fn extracted(base: &Path) -> PathBuf {
        let root = base.join(ROOT);
        fs::create_dir_all(root.join("source/native")).unwrap();
        fs::write(root.join("source/native/a.rs"), b"PUBLIC a\n").unwrap();
        fs::write(root.join("source/native/b.rs"), b"PUBLIC b\n").unwrap();
        root
    }

    fn record(name: &str, body: &[u8]) -> Value {
        serde_json::json!({
            "path": name,
            "bytes": body.len(),
            "sha256": format!("{:x}", Sha256::digest(body)),
        })
    }

    fn write_inventory(root: &Path, count: u64, files: &[Value]) {
        fs::write(
            root.join("source-snapshot.json"),
            serde_json::to_string(&serde_json::json!({"fileCount": count, "files": files}))
                .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn a_complete_inventory_is_accepted_and_a_duplicate_or_boolean_count_is_not() {
        let base = make_temp_dir(&std::env::temp_dir(), "inventory-").unwrap();
        let root = extracted(&base);
        let a = record("source/native/a.rs", b"PUBLIC a\n");
        let b = record("source/native/b.rs", b"PUBLIC b\n");
        write_inventory(&root, 2, &[a.clone(), b.clone()]);
        let tree = snapshot(&root, false).unwrap();
        assert_eq!(validate_inventory(&root, &tree, ROOT), Ok(2));

        // A duplicated record hides behind set equality only if lengths are ignored.
        write_inventory(&root, 2, &[a.clone(), a.clone()]);
        assert_eq!(
            validate_inventory(&root, &tree, ROOT),
            Err("Source inventory mismatch".into())
        );

        // JSON `true` cannot impersonate a one-byte count.
        let mut boolean = a.clone();
        boolean["bytes"] = Value::Bool(true);
        write_inventory(&root, 2, &[boolean, b.clone()]);
        assert_eq!(
            validate_inventory(&root, &tree, ROOT),
            Err("Source inventory mismatch".into())
        );

        // A record for a file that is not in the extracted tree is rejected.
        write_inventory(&root, 2, &[a, record("source/native/missing.rs", b"x")]);
        assert_eq!(
            validate_inventory(&root, &tree, ROOT),
            Err("Source inventory mismatch".into())
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_backslash_control_character_or_trailing_slash_member_is_rejected() {
        for name in [
            format!("{ROOT}/bad\\name"),
            format!("{ROOT}/bad\u{1}name"),
            format!("{ROOT}/dir/"),
        ] {
            let members = vec![
                member(ROOT, true, 0o755),
                Member {
                    name,
                    directory: false,
                    mode: 0o644,
                    size: 4,
                },
            ];
            assert!(
                validate_members(&members, ROOT).is_err(),
                "accepted a bad name"
            );
        }
    }
}
