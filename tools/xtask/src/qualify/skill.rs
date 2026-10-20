//! Pinned skill loading through the owned alpha profile runner.
//!
//! Port of `scripts/qualify-impeccable-skill.py`. Rejects partial loads, wrong
//! scope, wakes and metadata-only evidence: each fact must show the catalog,
//! the full body and the tool loaded, with the instruction digest of the file it
//! claims to have read.
use super::composed::{OwnedRun, run_owned};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

/// Scope to skill name; only these three are accepted.
pub const EXPECTED: [(&str, &str); 3] = [
    ("native-repository", "impeccable-rust"),
    ("desktop-workspace", "impeccable-rust"),
    ("backend-repository", "harness-verification"),
];

/// The instruction digest each skill name must report.
pub fn expected_hashes(root: &Path) -> BTreeMap<String, String> {
    let mut hashes = BTreeMap::new();
    let native = root.join(".agents/skills/impeccable-rust/SKILL.md");
    if let Ok(digest) = crate::sha256_file(&native) {
        hashes.insert("impeccable-rust".to_string(), digest);
    }
    let backend = root.parent().map(|parent| {
        parent.join("deepseek-harness-linux/.agents/skills/harness-verification/SKILL.md")
    });
    if let Some(digest) = backend
        .as_deref()
        .and_then(|path| crate::sha256_file(path).ok())
    {
        hashes.insert("harness-verification".to_string(), digest);
    }
    hashes
}

pub fn valid_skill(report: &Value, hashes: &BTreeMap<String, String>) -> bool {
    let (Some(counts), Some(facts)) = (report["counts"].as_object(), report["facts"].as_array())
    else {
        return false;
    };
    let counts_ok = counts
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>()
        == ["turnStarts", "stepStarts", "requests", "unexpectedWake"]
            .into_iter()
            .collect()
        && counts.values().all(|value| value.as_i64() == Some(0));
    let mut scopes = BTreeMap::new();
    for fact in facts {
        let (Some(name), Some(skill)) = (fact["name"].as_str(), fact["skill"].as_str()) else {
            return false;
        };
        scopes.insert(name, skill);
    }
    let scope_ok = scopes.len() == EXPECTED.len()
        && EXPECTED
            .iter()
            .all(|(name, skill)| scopes.get(name) == Some(skill));
    let facts_ok = facts.iter().all(|fact| {
        fact["catalog"].as_bool() == Some(true)
            && fact["bodyLoaded"].as_bool() == Some(true)
            && fact["toolLoaded"].as_bool() == Some(true)
            && fact["skill"]
                .as_str()
                .and_then(|skill| hashes.get(skill))
                .is_some_and(|expected| fact["instructionSha256"].as_str() == Some(expected))
    });
    report["status"].as_str() == Some("passed")
        && report["phase"].as_str() == Some("complete")
        && report["fatal"].as_bool() == Some(false)
        && report["runtime"].as_str() == Some("0.2.1-alpha.1")
        && report["providerSubstituted"].as_bool() == Some(false)
        && report["modelPromptsRequested"].as_bool() == Some(false)
        && counts_ok
        && scope_ok
        && facts_ok
}

/// `xtask qualify impeccable-skill`.
pub fn run(root: &Path, output: &Path) -> i32 {
    let script = root.join("scripts/qualify-impeccable-skill.mjs");
    let hashes = expected_hashes(root);
    if hashes.len() != 2 {
        eprintln!("impeccable skill failed: the pinned skill files are missing");
        return 1;
    }
    let spec = OwnedRun {
        script: &script,
        report_name: "skill.json",
        report_key: "skill",
        evidence_env: "DSH_NATIVE_SKILL_EVIDENCE",
        scope: Some(
            "Owned exact-alpha profile, real scoped catalog and full skill tool loading; no GUI/provider execution",
        ),
        command: {
            let script = script.to_string_lossy().into_owned();
            Box::new(move |_| vec!["/usr/bin/node".to_string(), script.clone()])
        },
        capture_stdout: false,
        timeout: Duration::from_secs(35),
    };
    match run_owned(root, output, &spec, |report| valid_skill(report, &hashes)) {
        Ok(result) => {
            if result["status"].as_str() == Some("passed") {
                0
            } else {
                1
            }
        }
        Err(message) => {
            eprintln!("impeccable skill failed: {message}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hashes() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("impeccable-rust".to_string(), "a".repeat(64)),
            ("harness-verification".to_string(), "b".repeat(64)),
        ])
    }

    fn report() -> Value {
        let fact = |name: &str, skill: &str| {
            json!({"name": name, "skill": skill, "catalog": true, "bodyLoaded": true,
                   "toolLoaded": true, "instructionSha256": hashes()[skill]})
        };
        json!({
            "status": "passed", "phase": "complete", "fatal": false,
            "runtime": "0.2.1-alpha.1", "providerSubstituted": false,
            "modelPromptsRequested": false,
            "counts": {"turnStarts": 0, "stepStarts": 0, "requests": 0, "unexpectedWake": 0},
            "facts": [
                fact("native-repository", "impeccable-rust"),
                fact("desktop-workspace", "impeccable-rust"),
                fact("backend-repository", "harness-verification"),
            ],
        })
    }

    #[test]
    fn a_skill_load_needs_all_three_scopes_with_full_bodies_and_matching_digests() {
        assert!(valid_skill(&report(), &hashes()));
        // A partial load (metadata only) is not a qualification.
        let mut partial = report();
        partial["facts"][0]["bodyLoaded"] = json!(false);
        assert!(!valid_skill(&partial, &hashes()));
        // A wrong scope mapping is rejected even if every fact otherwise passes.
        let mut foreign = report();
        foreign["facts"][2]["skill"] = json!("impeccable-rust");
        assert!(!valid_skill(&foreign, &hashes()));
        // A digest that does not match the file it claims is rejected.
        let mut stale = report();
        stale["facts"][1]["instructionSha256"] = json!("c".repeat(64));
        assert!(!valid_skill(&stale, &hashes()));
        // A wake or a turn is not a quiet load.
        let mut woke = report();
        woke["counts"]["unexpectedWake"] = json!(1);
        assert!(!valid_skill(&woke, &hashes()));
        assert!(
            !valid_skill(&json!({}), &hashes()),
            "a truncated report is not valid"
        );
    }
}
