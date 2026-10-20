//! Command-line predicates: the plan is printable without running anything, and
//! rejected arguments exit 2 rather than starting a partial run.
use std::process::Command;

fn xtask() -> Command {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
}

#[test]
fn the_plan_prints_commands_without_running_them() {
    let output = xtask()
        .args(["verify", "--suite", "examples", "--plan"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let plan: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let checks = plan["checks"].as_array().unwrap();
    assert_eq!(checks.len(), 2, "{plan}");
    let names: Vec<&str> = checks
        .iter()
        .map(|check| check["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["layout_smoke", "scroll_app_smoke"]);
    assert_eq!(plan["suite"], "examples");
    assert_eq!(plan["offline"], true);
}

#[test]
fn the_freshness_guard_is_planned_for_the_reporting_suites_only() {
    for suite in ["test", "all"] {
        let output = xtask()
            .args(["verify", "--suite", suite, "--plan"])
            .output()
            .unwrap();
        let plan: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let first = &plan["checks"][0];
        assert_eq!(first["name"], "binary-freshness", "{suite}");
        assert_eq!(
            first["command"],
            serde_json::json!(["internal", "binary-freshness"])
        );
    }
    let output = xtask()
        .args(["verify", "--suite", "examples", "--plan"])
        .output()
        .unwrap();
    let plan: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let names: Vec<&str> = plan["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|check| check["name"].as_str().unwrap())
        .collect();
    assert!(!names.contains(&"binary-freshness"), "{names:?}");
}

#[test]
fn the_lint_plan_starts_with_the_in_process_format_guard() {
    let output = xtask()
        .args(["verify", "--suite", "lint", "--plan"])
        .output()
        .unwrap();
    let plan: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(plan["checks"][0]["name"], "format-debt");
    assert_eq!(
        plan["checks"][0]["command"],
        serde_json::json!(["internal", "format-debt"])
    );
    assert_eq!(plan["checks"].as_array().unwrap().len(), 5);
}

#[test]
fn invalid_arguments_exit_two() {
    let cases: [&[&str]; 6] = [
        &["verify", "--timeout", "0", "--plan"],
        &["verify", "--timeout", "3601", "--plan"],
        &["verify", "--cargo-home", "relative-cache", "--plan"],
        &["verify", "--suite", "live-model", "--plan"],
        &["verify", "--unknown"],
        &["definitely-not-a-command"],
    ];
    for arguments in cases {
        let output = xtask().args(arguments).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
    }
}

#[test]
fn help_names_the_commands_it_replaced() {
    let output = xtask().arg("help").output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("verify"), "{text}");
    assert!(text.contains("format-debt"), "{text}");
}
