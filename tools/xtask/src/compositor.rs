//! The compositor's own view of one test pid's window.
//!
//! Only the owning process's entry is inspected, never other windows, and the
//! five reported fields are kept as raw JSON so integer sizes stay integers in
//! evidence. Shared by the benchmark runner and the component qualifiers.
use serde_json::{Map, Value};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// One compositor entry for the test pid, limited to the fields we report.
pub type Facts = Map<String, Value>;

/// Ask the compositor for this pid's window; `None` when it is absent, unreadable,
/// still unmapped, or the probe exceeds its bound.
pub fn window_facts(pid: i32) -> Option<Facts> {
    let mut child = Command::new("hyprctl")
        .args(["-j", "clients"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stream = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut raw = Vec::new();
        let _ = stream.read_to_end(&mut raw);
        raw
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait().ok()? {
            Some(status) => {
                if !status.success() {
                    return None;
                }
                break;
            }
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    let raw = reader.join().ok()?;
    if raw.len() > 1024 * 1024 {
        return None;
    }
    let entries: Vec<Value> = serde_json::from_slice(&raw).ok()?;
    let entry = entries
        .into_iter()
        .find(|entry| entry.get("pid").and_then(Value::as_i64) == Some(pid as i64))?;
    let mut facts = Facts::new();
    for key in ["pid", "class", "xwayland", "mapped", "size"] {
        facts.insert(
            key.to_string(),
            entry.get(key).cloned().unwrap_or(Value::Null),
        );
    }
    Some(facts)
}

/// The two facts a qualification insists on: mapped, native (not XWayland).
pub fn is_mapped_native(facts: Option<&Facts>) -> bool {
    facts.is_some_and(|facts| {
        facts.get("mapped").and_then(Value::as_bool) == Some(true)
            && facts.get("xwayland").and_then(Value::as_bool) == Some(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn facts(mapped: bool, xwayland: bool) -> Facts {
        let mut facts = Facts::new();
        facts.insert("mapped".into(), json!(mapped));
        facts.insert("xwayland".into(), json!(xwayland));
        facts
    }

    #[test]
    fn only_a_mapped_native_window_counts_as_owned() {
        assert!(is_mapped_native(Some(&facts(true, false))));
        assert!(!is_mapped_native(Some(&facts(false, false))));
        assert!(!is_mapped_native(Some(&facts(true, true))));
        assert!(!is_mapped_native(None));
    }
}
