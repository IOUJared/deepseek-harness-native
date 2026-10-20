//! `/proc` measurement shared by the benchmark runner and the idle sampler.
//!
//! Every reader takes a *proc root*, normally `/proc`. Tests point it at a
//! fixture directory, so the parsing and tree-building rules are exercised
//! without depending on the machine's live process table.
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

/// Monotonic seconds since the first call in this process; the float the Python
/// tools got from `time.monotonic()`.
pub fn monotonic() -> f64 {
    static BASE: OnceLock<Instant> = OnceLock::new();
    BASE.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// `os.sysconf("SC_CLK_TCK")`, the kernel's scheduler ticks per second.
pub fn clock_ticks() -> f64 {
    // Safe: a constant sysconf selector with no pointer arguments.
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if ticks > 0 { ticks as f64 } else { 100.0 }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stat {
    pub pid: i32,
    pub ppid: i32,
    pub pgrp: i32,
    pub cpu_ticks: u64,
    pub start_ticks: u64,
}

/// Fields after the parenthesised `comm`; the comm itself may contain spaces and
/// parentheses, which is why parsing starts at the last `)`.
fn after_comm(text: &str) -> Option<Vec<&str>> {
    let close = text.rfind(')')?;
    Some(text.get(close + 2..)?.split_whitespace().collect())
}

pub fn stat(proc_root: &Path, pid: i32) -> Option<Stat> {
    let text = fs::read_to_string(proc_root.join(pid.to_string()).join("stat")).ok()?;
    let fields = after_comm(&text)?;
    let number = |index: usize| fields.get(index)?.parse::<i64>().ok();
    Some(Stat {
        pid,
        ppid: number(1)? as i32,
        pgrp: number(2)? as i32,
        cpu_ticks: (number(11)? + number(12)?) as u64,
        start_ticks: number(19)? as u64,
    })
}

/// `(utime, stime)` only, as the idle sampler records them.
pub fn stat_ticks(text: &str) -> Option<(u64, u64)> {
    let fields = after_comm(text)?;
    let utime = fields.get(11)?.parse::<u64>().ok()?;
    let stime = fields.get(12)?.parse::<u64>().ok()?;
    Some((utime, stime))
}

/// The parent pid from one `stat` body.
pub fn ppid(text: &str) -> Option<i32> {
    after_comm(text)?.get(1)?.parse().ok()
}

/// The process or thread start time from one `stat` body.
pub fn start_ticks(text: &str) -> Option<u64> {
    after_comm(text)?.get(19)?.parse().ok()
}

/// `(Rss, Pss)` from one `smaps_rollup` body; both are required.
pub fn smaps_rollup(text: &str) -> Option<(u64, u64)> {
    let mut rss = None;
    let mut pss = None;
    for line in text.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        if key != "Rss" && key != "Pss" {
            continue;
        }
        let value = rest.split_whitespace().next()?.parse::<u64>().ok()?;
        if key == "Rss" {
            rss = Some(value);
        } else {
            pss = Some(value);
        }
    }
    Some((rss?, pss?))
}

/// The root's descendants by parent link, bounded so a pathological tree cannot
/// make sampling unbounded. An empty answer means the root pid was reused: the
/// observed start time no longer matches the identity captured at spawn.
pub fn process_tree(proc_root: &Path, root_pid: i32, root_start: u64) -> Vec<Stat> {
    let Some(root) = stat(proc_root, root_pid) else {
        return Vec::new();
    };
    if root.start_ticks != root_start {
        return Vec::new();
    }
    let mut processes: BTreeMap<i32, Stat> = BTreeMap::new();
    if let Ok(entries) = fs::read_dir(proc_root) {
        for entry in entries.flatten() {
            let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else {
                continue;
            };
            if let Some(found) = stat(proc_root, pid) {
                processes.insert(pid, found);
            }
        }
    }
    let mut owned: Vec<i32> = vec![root_pid];
    loop {
        let mut added = Vec::new();
        for process in processes.values() {
            if owned.contains(&process.ppid) && !owned.contains(&process.pid) {
                added.push(process.pid);
            }
        }
        if added.is_empty() {
            break;
        }
        owned.extend(added);
    }
    owned
        .into_iter()
        .filter_map(|pid| processes.get(&pid).cloned())
        .collect()
}

/// VRAM held by one process's DRM clients, keyed by driver, PCI device and client id.
pub fn drm_clients(proc_root: &Path, pid: i32) -> BTreeMap<(String, String, String), u64> {
    let mut clients = BTreeMap::new();
    let Ok(entries) = fs::read_dir(proc_root.join(pid.to_string()).join("fdinfo")) else {
        return clients;
    };
    for entry in entries.flatten() {
        let Ok(text) = fs::read_to_string(entry.path()) else {
            continue;
        };
        let mut fields = BTreeMap::new();
        for line in text.lines() {
            if let Some((key, value)) = line.split_once(':') {
                fields.insert(key, value);
            }
        }
        let identifier = fields.get("drm-client-id").map_or("", |value| value.trim());
        if identifier.is_empty() {
            continue;
        }
        let key = (
            fields
                .get("drm-driver")
                .map_or(String::new(), |v| v.trim().to_string()),
            fields
                .get("drm-pdev")
                .map_or(String::new(), |v| v.trim().to_string()),
            identifier.to_string(),
        );
        if let Some(value) = fields.get("drm-memory-vram") {
            let parts: Vec<&str> = value.split_whitespace().collect();
            if let [number, "KiB"] = parts.as_slice()
                && let Ok(kib) = number.parse::<u64>()
            {
                clients.insert(key, kib);
            }
        }
    }
    clients
}

/// The three system load averages, from `<proc>/loadavg` (`os.getloadavg`).
pub fn loadavg(proc_root: &Path) -> [f64; 3] {
    fs::read_to_string(proc_root.join("loadavg"))
        .ok()
        .and_then(|text| {
            let mut values = [0.0; 3];
            let mut fields = text.split_whitespace();
            for value in &mut values {
                *value = fields.next()?.parse().ok()?;
            }
            Some(values)
        })
        .unwrap_or([0.0; 3])
}

#[derive(Clone, Debug)]
pub struct TreeSample {
    pub at: f64,
    pub loadavg: [f64; 3],
    pub process_count: usize,
    pub rss_kib: u64,
    /// `None` when any process's `smaps_rollup` was unreadable; an unknown total
    /// must not be reported as zero.
    pub pss_kib: Option<u64>,
    pub drm_vram_kib: Option<u64>,
    /// `pid:start_ticks` to CPU ticks, so a reused pid cannot masquerade as the same process.
    pub ticks: BTreeMap<String, u64>,
}

/// One sample of the owned process tree: memory, GPU, per-identity CPU ticks.
pub fn sample_tree(proc_root: &Path, root_pid: i32, root_start: u64) -> TreeSample {
    let processes = process_tree(proc_root, root_pid, root_start);
    let mut rss = 0u64;
    let mut pss = 0u64;
    let mut pss_complete = !processes.is_empty();
    let mut ticks = BTreeMap::new();
    let mut gpu = BTreeMap::new();
    for process in &processes {
        let Some(current) = stat(proc_root, process.pid) else {
            pss_complete = false;
            continue;
        };
        if current.start_ticks != process.start_ticks {
            pss_complete = false;
            continue;
        }
        ticks.insert(
            format!("{}:{}", process.pid, process.start_ticks),
            process.cpu_ticks,
        );
        gpu.extend(drm_clients(proc_root, process.pid));
        if let Ok(text) = fs::read_to_string(proc_root.join(process.pid.to_string()).join("status"))
            && let Some(rss_line) = text.lines().find(|line| line.starts_with("VmRSS:"))
            && let Some(value) = rss_line
                .split_whitespace()
                .nth(1)
                .and_then(|value| value.parse::<u64>().ok())
        {
            rss += value;
        }
        match fs::read_to_string(proc_root.join(process.pid.to_string()).join("smaps_rollup"))
            .ok()
            .and_then(|text| smaps_rollup(&text))
        {
            Some((_, value)) => pss += value,
            None => pss_complete = false,
        }
    }
    TreeSample {
        at: monotonic(),
        loadavg: loadavg(proc_root),
        process_count: processes.len(),
        rss_kib: rss,
        pss_kib: pss_complete.then_some(pss),
        drm_vram_kib: (!gpu.is_empty()).then(|| gpu.values().sum()),
        ticks,
    }
}

/// Endpoint CPU over one interval, as percent of one logical core. A process that
/// disappeared, or tick counters that went backwards, answer `None` rather than a
/// false zero.
pub fn cpu_percent(previous: &TreeSample, current: &TreeSample, hz: f64) -> Option<f64> {
    let elapsed = current.at - previous.at;
    if elapsed <= 0.0 {
        return None;
    }
    if !previous
        .ticks
        .keys()
        .all(|key| current.ticks.contains_key(key))
    {
        return None;
    }
    let delta: i64 = current
        .ticks
        .iter()
        .map(|(key, value)| {
            *value as i64 - previous.ticks.get(key).copied().unwrap_or(*value) as i64
        })
        .sum();
    Some(delta.max(0) as f64 / hz / elapsed * 100.0)
}

/// A signal is only safe when the pid is still the captured root and leads its group.
pub fn group_is_owned(proc_root: &Path, pid: i32, start: u64) -> bool {
    stat(proc_root, pid).is_some_and(|current| current.start_ticks == start && current.pgrp == pid)
}

/// Median of a value list; integer inputs stay integral, as Python's did.
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut ordered = values.to_vec();
    ordered.sort_by(|a, b| a.total_cmp(b));
    let middle = ordered.len() / 2;
    Some(if ordered.len().is_multiple_of(2) {
        (ordered[middle - 1] + ordered[middle]) / 2.0
    } else {
        ordered[middle]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempdir::make_temp_dir;

    fn fixture(lines: &[(&str, &str)]) -> std::path::PathBuf {
        let root = make_temp_dir(&std::env::temp_dir(), "proc-").unwrap();
        for (path, text) in lines {
            let target = root.join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, text).unwrap();
        }
        root
    }

    /// `pid (comm) state ppid pgrp ... utime stime ... starttime ...`
    fn stat_line(pid: i32, ppid: i32, pgrp: i32, utime: u64, stime: u64, start: u64) -> String {
        let mut fields = vec!["S".to_string(), ppid.to_string(), pgrp.to_string()];
        fields.resize(11, "0".to_string());
        fields.push(utime.to_string());
        fields.push(stime.to_string());
        fields.resize(19, "0".to_string());
        fields.push(start.to_string());
        format!("{pid} (owned) {}\n", fields.join(" "))
    }

    #[test]
    fn a_comm_with_spaces_and_parentheses_does_not_shift_the_fields() {
        let text =
            "42 (weird ) name) R 1 42 42 0 -1 4194304 10 0 0 0 50 25 0 0 20 0 1 0 0 0 0 0 0 0";
        assert_eq!(stat_ticks(text), Some((50, 25)));
        let root = fixture(&[("42/stat", text)]);
        let parsed = stat(&root, 42).unwrap();
        assert_eq!(parsed.ppid, 1);
        assert_eq!(parsed.pgrp, 42);
        assert_eq!(parsed.cpu_ticks, 75);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_reused_root_pid_yields_no_tree() {
        let line = stat_line(100, 1, 100, 0, 0, 20);
        let root = fixture(&[("100/stat", &line)]);
        assert!(
            process_tree(&root, 100, 10).is_empty(),
            "start time differs"
        );
        assert_eq!(process_tree(&root, 100, 20).len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn descendants_are_collected_through_the_parent_link() {
        let root_line = stat_line(100, 1, 100, 0, 0, 10);
        let child = stat_line(200, 100, 100, 0, 0, 11);
        let grandchild = stat_line(300, 200, 100, 0, 0, 12);
        let sibling = stat_line(400, 1, 400, 0, 0, 13);
        let root = fixture(&[
            ("100/stat", &root_line),
            ("200/stat", &child),
            ("300/stat", &grandchild),
            ("400/stat", &sibling),
        ]);
        let mut pids: Vec<i32> = process_tree(&root, 100, 10).iter().map(|p| p.pid).collect();
        pids.sort();
        assert_eq!(pids, [100, 200, 300]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cpu_is_a_percentage_of_one_core() {
        let previous = TreeSample {
            at: 10.0,
            loadavg: [0.0; 3],
            process_count: 1,
            rss_kib: 0,
            pss_kib: None,
            drm_vram_kib: None,
            ticks: [("100:1".to_string(), 100)].into(),
        };
        let mut current = previous.clone();
        current.at = 12.0;
        current.ticks.insert("100:1".into(), 100 + 200);
        assert_eq!(cpu_percent(&previous, &current, 100.0), Some(100.0));
        current.ticks.insert("100:1".into(), 100 + 100);
        assert_eq!(cpu_percent(&previous, &current, 100.0), Some(50.0));
    }

    #[test]
    fn a_disappeared_process_does_not_claim_zero_cpu() {
        let previous = TreeSample {
            at: 10.0,
            loadavg: [0.0; 3],
            process_count: 2,
            rss_kib: 0,
            pss_kib: None,
            drm_vram_kib: None,
            ticks: [("100:1".into(), 100), ("200:2".into(), 50)].into(),
        };
        let mut current = previous.clone();
        current.at = 12.0;
        current.ticks.remove("200:2");
        assert_eq!(cpu_percent(&previous, &current, 100.0), None);
    }

    #[test]
    fn a_new_descendant_is_not_charged_for_cpu_before_its_first_sample() {
        let previous = TreeSample {
            at: 10.0,
            loadavg: [0.0; 3],
            process_count: 1,
            rss_kib: 0,
            pss_kib: None,
            drm_vram_kib: None,
            ticks: [("100:1".into(), 100)].into(),
        };
        let mut current = previous.clone();
        current.at = 12.0;
        current.ticks.insert("200:2".into(), 10_000);
        assert_eq!(cpu_percent(&previous, &current, 100.0), Some(0.0));
    }

    #[test]
    fn a_signal_target_requires_the_captured_start_and_its_own_group() {
        let line = stat_line(100, 1, 100, 0, 0, 10);
        let root = fixture(&[("100/stat", &line)]);
        assert!(group_is_owned(&root, 100, 10));
        assert!(!group_is_owned(&root, 100, 11));
        fs::remove_dir_all(root).unwrap();
        let foreign_line = stat_line(100, 1, 50, 0, 0, 10);
        let other = fixture(&[("100/stat", &foreign_line)]);
        assert!(!group_is_owned(&other, 100, 10));
        fs::remove_dir_all(other).unwrap();
        let absent = fixture(&[]);
        assert!(!group_is_owned(&absent, 100, 10));
        fs::remove_dir_all(absent).unwrap();
    }

    #[test]
    fn smaps_rollup_requires_both_totals() {
        assert_eq!(smaps_rollup("Rss: 100 kB\nPss: 80 kB\n"), Some((100, 80)));
        assert_eq!(smaps_rollup("Rss: 100 kB\n"), None);
        assert_eq!(smaps_rollup(""), None);
    }

    #[test]
    fn a_tree_sample_sums_memory_and_reports_unknown_pss() {
        let first = stat_line(100, 1, 100, 0, 0, 10);
        let second = stat_line(200, 100, 100, 0, 0, 11);
        let root = fixture(&[
            ("100/stat", &first),
            ("100/status", "Name:\towned\nVmRSS:\t  1024 kB\n"),
            ("100/smaps_rollup", "Rss: 1000 kB\nPss: 800 kB\n"),
            (
                "100/fdinfo/7",
                "drm-client-id:\t3\ndrm-driver:\tamdgpu\ndrm-pdev:\t0000:03:00.0\ndrm-memory-vram:\t 2048 KiB\n",
            ),
            ("200/stat", &second),
            ("200/status", "VmRSS:\t  512 kB\n"),
            ("200/smaps_rollup", "Pss: 400 kB\n"),
            ("loadavg", "0.50 0.25 0.10 1/100 1234\n"),
        ]);
        let sample = sample_tree(&root, 100, 10);
        assert_eq!(sample.process_count, 2);
        assert_eq!(sample.rss_kib, 1536);
        assert_eq!(
            sample.pss_kib, None,
            "200 has no Rss line, so the total is unknown"
        );
        assert_eq!(sample.drm_vram_kib, Some(2048));
        assert_eq!(sample.loadavg, [0.5, 0.25, 0.1]);
        assert_eq!(sample.ticks.len(), 2);
        // Repair the second smaps and the total becomes known.
        fs::write(root.join("200/smaps_rollup"), "Rss: 400 kB\nPss: 300 kB\n").unwrap();
        assert_eq!(sample_tree(&root, 100, 10).pss_kib, Some(1100));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_median_handles_odd_even_and_empty() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }
}
