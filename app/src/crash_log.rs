//! A durable record of a fatal start failure.
//!
//! The macOS shell writes `<logs>/crash-<iso>-<source>.log` for a failed Host
//! start or a renderer crash, caps the inspected error it embeds at 256 KiB and
//! keeps the newest ten reports (`main.js:7647-7742, 10989-11007`). Native had
//! no durable artifact at all: a failed Host start printed to stderr and exited,
//! so a launch from a desktop entry left no diagnosable trace.
//!
//! The record carries only what that stderr line already showed plus the
//! environment around it. Nothing here reads credentials, drafts or the Host's
//! stream, and the write is confined to the native home the run already owns.

use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The reference's cap on the inspected error text.
pub const MAX_DETAIL_BYTES: usize = 256 * 1024;
/// The reference keeps the newest ten reports.
pub const KEEP: usize = 10;
/// Bound on the collision suffix when two reports land in the same millisecond.
const MAX_ATTEMPTS: u32 = 20;

/// What one report says. `summary` and `detail` are the caller's own failure
/// text; the writer never adds process state beyond the header below.
pub struct Record<'a> {
    pub source: &'a str,
    pub summary: &'a str,
    pub detail: &'a str,
}

/// Write one report under `<home>/logs` and return its path.
///
/// Returns `None` when the home cannot hold a record; a failure to report must
/// never replace the failure being reported.
pub fn write(home: &Path, record: &Record<'_>) -> Option<PathBuf> {
    let directory = home.join("logs");
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory)
        .ok()?;
    let source = sanitized(record.source);
    let mut path = None;
    let mut file = None;
    for attempt in 0..MAX_ATTEMPTS {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let candidate = directory.join(format!("crash-{}-{source}{suffix}.log", stamp()));
        // `create_new` refuses an existing path, including a symlink planted
        // there, so a report can never overwrite or follow one.
        match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&candidate)
        {
            Ok(handle) => {
                path = Some(candidate);
                file = Some(handle);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    let path = path?;
    let mut file = file?;
    let mut text = String::new();
    let _ = writeln!(
        text,
        "DeepSeek Harness native {}",
        env!("CARGO_PKG_VERSION")
    );
    let _ = writeln!(text, "written: {}", stamp());
    let _ = writeln!(
        text,
        "platform: {}/{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let _ = writeln!(text, "source: {}", record.source);
    let _ = writeln!(text, "summary: {}", record.summary);
    let _ = writeln!(text, "detail ({} bytes max):", MAX_DETAIL_BYTES);
    text.push_str(&bounded(record.detail));
    text.push('\n');
    file.write_all(text.as_bytes()).ok()?;
    drop(file);
    prune(&directory);
    Some(path)
}

/// The newest [`KEEP`] reports survive; older ones are removed. A failure to
/// prune is not a failure to report.
fn prune(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with("crash-") && name.ends_with(".log"))
        .collect();
    if names.len() <= KEEP {
        return;
    }
    // Names carry the local stamp first, so lexical order is chronological.
    names.sort();
    for name in names.iter().take(names.len() - KEEP) {
        let _ = fs::remove_file(directory.join(name));
    }
}

/// The inspected error, cut on a character boundary at [`MAX_DETAIL_BYTES`].
fn bounded(detail: &str) -> String {
    if detail.len() <= MAX_DETAIL_BYTES {
        return detail.to_owned();
    }
    let mut end = MAX_DETAIL_BYTES;
    while end > 0 && !detail.is_char_boundary(end) {
        end -= 1;
    }
    let mut text = detail[..end].to_owned();
    text.push_str("\n[truncated]");
    text
}

/// A file-name-safe source label: ASCII alphanumerics, `-` and `_`, bounded.
fn sanitized(source: &str) -> String {
    let mut out: String = source
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(32)
        .collect();
    if out.is_empty() {
        out.push_str("unknown");
    }
    out
}

/// `YYYYMMDDTHHMMSS-mmm` in local time, the reference's sortable stamp.
fn stamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = now.as_secs();
    let millis = now.subsec_millis();
    let time = libc::time_t::try_from(seconds).unwrap_or(0);
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: `localtime_r` writes into the `tm` this frame owns and reads only
    // `time`; it retains no pointer to either.
    let filled = unsafe { libc::localtime_r(&time, tm.as_mut_ptr()) };
    let tm = unsafe { tm.assume_init() };
    if filled.is_null() {
        return format!("{seconds}-{millis:03}");
    }
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}-{millis:03}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fixture() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "dsh-crash-log-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn record_is_private_bounded_and_readable() {
        let base = fixture();
        let detail = "x".repeat(MAX_DETAIL_BYTES + 4096);
        let path = write(
            &base,
            &Record {
                source: "host-start",
                summary: "core: spawn refused",
                detail: &detail,
            },
        )
        .expect("record");
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with("crash-") && name.ends_with("-host-start.log"),
            "{name}"
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(base.join("logs"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("DeepSeek Harness native "));
        assert!(text.contains("source: host-start"));
        assert!(text.contains("summary: core: spawn refused"));
        assert!(text.contains("[truncated]"));
        // The header plus the cap, and nothing unbounded.
        assert!(text.len() < MAX_DETAIL_BYTES + 4096, "{}", text.len());
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn newest_ten_reports_survive_and_odd_sources_are_sanitized() {
        let base = fixture();
        for index in 0..(KEEP + 3) {
            let path = write(
                &base,
                &Record {
                    source: "../../evil source",
                    summary: "startup refused",
                    detail: "detail",
                },
            )
            .expect("record");
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            // Only ASCII alphanumerics, `-` and `_` survive sanitization, so a
            // path-shaped source label cannot escape the logs directory.
            assert!(name.ends_with("-evilsource.log"), "{name}");
            assert!(path.starts_with(base.join("logs")));
            assert!(!name.contains("..") && !name.contains('/'), "{name}");
            // Keep the loop's own order observable to the prune.
            std::thread::sleep(std::time::Duration::from_millis(2));
            let _ = index;
        }
        let remaining: Vec<String> = fs::read_dir(base.join("logs"))
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(remaining.len(), KEEP, "{remaining:?}");
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn an_existing_report_is_never_overwritten() {
        let base = fixture();
        let record = Record {
            source: "host-start",
            summary: "first",
            detail: "detail",
        };
        let first = write(&base, &record).expect("first record");
        let before = fs::read_to_string(&first).unwrap();
        // A planted symlink at the next candidate path is refused, not followed:
        // `create_new` fails and the writer moves to its collision suffix.
        let second = write(
            &base,
            &Record {
                source: "host-start",
                summary: "second",
                detail: "detail",
            },
        )
        .expect("second record");
        assert_ne!(first, second);
        assert_eq!(fs::read_to_string(&first).unwrap(), before);
        fs::remove_dir_all(&base).unwrap();
    }
}
