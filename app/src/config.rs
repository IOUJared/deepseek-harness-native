use dsh_native_core::{RustBackendOptions, StopPolicy};
use std::{
    path::{Component, PathBuf},
    time::Duration,
};
pub const USAGE: &str = "dsh-native-app --runtime ABS_FORK/apps/cli --expected-version 0.2.1-alpha.1 --home ABS_NATIVE_HOME --cwd EXISTING_ABSOLUTE_DIR [--user-home ABS_USER_HOME] [--node ABS_NODE_EXECUTABLE] [--scale 1.0]\n\nStarts an isolated owned native Host; no installed-runtime fallback. Omitting --node selects /usr/bin/node, never PATH. Explicit Node paths are validated by Core before startup; selecting an interpreter does not bundle its libraries or the Host runtime. Load models and Send require explicit clicks. --help does not start a Host or GUI.\n\nOwn keyless GUI smoke (explicit opt-in): --smoke-new-session --exit-after-seconds 8 --smoke-report ABS_NEW_FILE_INSIDE_APP/evidence/report.json. Requires an empty/new native home; opens only --cwd and creates one REAL blank session, without catalog or prompt requests. Optional --smoke-screenshot ABS_NEW_PNG_INSIDE_APP/evidence/own-window.png captures exactly one own Iced render after the REAL snapshot; no desktop capture. A relocated development binary may explicitly select --smoke-evidence-root ABS_EXISTING_OWNED_PRIVATE_0700_DIRECTORY; omission preserves the source-local app/evidence restriction.";
#[derive(Clone)]
pub struct Options {
    pub backend: RustBackendOptions,
    pub scale: f32,
    pub smoke: Option<Smoke>,
}
#[derive(Clone)]
pub struct Smoke {
    pub exit_after: Duration,
    pub report: PathBuf,
    pub screenshot: Option<PathBuf>,
}
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Option<Options>, String> {
    let mut values = std::collections::BTreeMap::new();
    let mut args = args.into_iter();
    while let Some(key) = args.next() {
        if matches!(key.as_str(), "--help" | "-h") {
            return Ok(None);
        }
        if key == "--smoke-new-session" {
            if values.insert(key, "true".into()).is_some() {
                return Err("Duplicate smoke argument".into());
            }
            continue;
        }
        if ![
            "--exit-after-seconds",
            "--smoke-report",
            "--smoke-screenshot",
            "--smoke-evidence-root",
            "--runtime",
            "--expected-version",
            "--home",
            "--cwd",
            "--user-home",
            "--node",
            "--scale",
        ]
        .contains(&key.as_str())
        {
            return Err("Unknown native application argument".into());
        }
        let value = args
            .next()
            .ok_or("Missing native application argument value")?;
        if values.insert(key, value).is_some() {
            return Err("Duplicate native application argument".into());
        }
    }
    let version = values
        .remove("--expected-version")
        .ok_or("--expected-version is required")?;
    if version != "0.2.1-alpha.1" {
        return Err(
            "Only the explicitly supplied alpha fork version 0.2.1-alpha.1 is supported".into(),
        );
    }
    fn path(
        values: &mut std::collections::BTreeMap<String, String>,
        key: &str,
    ) -> Result<PathBuf, String> {
        let value = values
            .remove(key)
            .ok_or_else(|| format!("{key} is required"))?;
        let path = PathBuf::from(value);
        if !path.is_absolute()
            || path.components().any(|c| matches!(c, Component::ParentDir))
            || path
                .to_str()
                .is_none_or(|s| s.bytes().any(|b| b < 32 || b == 127))
        {
            return Err(format!(
                "{key} must be an absolute path without parent traversal"
            ));
        }
        Ok(path)
    }
    // Syntax only here; Core owns canonical file/executable validation and spawn.
    let absolute_node_path = if values.contains_key("--node") {
        Some(path(&mut values, "--node")?)
    } else {
        None
    };
    let runtime = path(&mut values, "--runtime")?;
    let native_home = path(&mut values, "--home")?;
    let working_directory = path(&mut values, "--cwd")?;
    let user_home = if values.contains_key("--user-home") {
        path(&mut values, "--user-home")?
    } else {
        let home = std::env::var("HOME")
            .map_err(|_| "--user-home is required when HOME is unavailable")?;
        values.insert("--user-home".into(), home);
        path(&mut values, "--user-home")?
    };
    if native_home == user_home {
        return Err("Native home must be distinct from user home".into());
    }
    if !working_directory.is_dir() {
        return Err("--cwd must name an existing directory".into());
    }
    let scale = values
        .remove("--scale")
        .map_or(Ok(1.0), |v| v.parse::<f32>())
        .map_err(|_| "Invalid --scale")?;
    if !scale.is_finite() || !(0.75..=2.0).contains(&scale) {
        return Err("--scale must be between 0.75 and 2.0".into());
    }
    let smoke = if values.remove("--smoke-new-session").is_some() {
        let seconds = values
            .remove("--exit-after-seconds")
            .ok_or("Smoke mode requires --exit-after-seconds")?
            .parse::<u64>()
            .map_err(|_| "Invalid smoke deadline")?;
        if !(3..=60).contains(&seconds) {
            return Err("Smoke deadline must be 3–60 seconds".into());
        }
        let report = path(&mut values, "--smoke-report")?;
        let root = if values.contains_key("--smoke-evidence-root") {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            let root = path(&mut values, "--smoke-evidence-root")?
                .canonicalize()
                .map_err(|_| "Explicit smoke evidence directory unavailable")?;
            let metadata = std::fs::metadata(&root)
                .map_err(|_| "Explicit smoke evidence directory unavailable")?;
            let own_uid = std::fs::metadata("/proc/self")
                .map_err(|_| "Linux process ownership unavailable")?
                .uid();
            if !metadata.is_dir()
                || metadata.uid() != own_uid
                || metadata.permissions().mode() & 0o7777 != 0o700
            {
                return Err(
                    "Explicit smoke evidence root must be an owned private 0700 directory".into(),
                );
            }
            root
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("evidence")
                .canonicalize()
                .map_err(|_| "Smoke evidence directory unavailable")?
        };
        let parent = report
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .ok_or("Smoke report parent must exist")?;
        if !parent.starts_with(&root) || report.exists() {
            return Err("Smoke report must be a NEW file inside the selected evidence root".into());
        }
        let report = parent.join(report.file_name().ok_or("Missing report filename")?);
        let screenshot = if values.contains_key("--smoke-screenshot") {
            let output = path(&mut values, "--smoke-screenshot")?;
            let parent = output
                .parent()
                .and_then(|p| p.canonicalize().ok())
                .ok_or("Screenshot parent must exist")?;
            if !parent.starts_with(&root)
                || output.exists()
                || output.extension().is_none_or(|e| e != "png")
            {
                return Err(
                    "Screenshot must be a NEW .png file inside the selected evidence root".into(),
                );
            }
            let output = parent.join(output.file_name().ok_or("Missing screenshot filename")?);
            if output == report {
                return Err("Screenshot and report must be distinct files".into());
            }
            Some(output)
        } else {
            None
        };
        if native_home.exists()
            && std::fs::read_dir(&native_home)
                .map_err(|_| "Smoke home unavailable")?
                .next()
                .is_some()
        {
            return Err("Smoke home must be new or empty; never reuse user data".into());
        }
        Some(Smoke {
            exit_after: Duration::from_secs(seconds),
            report,
            screenshot,
        })
    } else {
        if values.contains_key("--smoke-report")
            || values.contains_key("--exit-after-seconds")
            || values.contains_key("--smoke-screenshot")
            || values.contains_key("--smoke-evidence-root")
        {
            return Err("Smoke options require --smoke-new-session".into());
        }
        None
    };
    Ok(Some(Options {
        scale,
        smoke,
        backend: RustBackendOptions {
            runtime,
            expected_version: version,
            native_home,
            user_home,
            working_directory,
            absolute_node_path,
            startup_timeout: Duration::from_secs(15),
            stop_policy: StopPolicy::default(),
        },
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args() -> Vec<String> {
        [
            "--runtime",
            "/explicit-fork/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/isolated-native-home",
            "--user-home",
            "/user-home",
            "--cwd",
            "/tmp",
        ]
        .map(str::to_owned)
        .to_vec()
    }
    #[test]
    fn explicit_paths_parse_without_launch() {
        assert!(parse(args()).unwrap().is_some());
    }
    #[test]
    fn omitted_node_preserves_core_system_default_without_path_lookup() {
        let options = parse(args()).unwrap().unwrap();
        assert!(options.backend.absolute_node_path.is_none());
    }
    #[test]
    fn explicit_node_preserves_absolute_spaces_and_unicode_without_probing() {
        let selected = "/PUBLIC nonexistent directory/node copy · 世界";
        let mut values = args();
        values.extend(["--node".into(), selected.into()]);
        let options = parse(values).unwrap().unwrap();
        assert_eq!(
            options.backend.absolute_node_path,
            Some(PathBuf::from(selected))
        );
        assert_eq!(
            options.backend.runtime,
            PathBuf::from("/explicit-fork/apps/cli")
        );
        assert_eq!(options.backend.expected_version, "0.2.1-alpha.1");
        assert_eq!(
            options.backend.native_home,
            PathBuf::from("/isolated-native-home")
        );
        assert_eq!(options.backend.user_home, PathBuf::from("/user-home"));
    }
    #[test]
    fn malformed_node_paths_refuse_without_echoing_caller_values() {
        for selected in [
            "",
            "PUBLIC_private_node",
            "/PUBLIC_private/../node",
            "/PUBLIC_private/node\n",
            "/PUBLIC_private/node\0",
            "/PUBLIC_private/node\u{7f}",
        ] {
            let mut values = args();
            values.extend(["--node".into(), selected.into()]);
            let error = match parse(values) {
                Err(error) => error,
                Ok(_) => panic!("invalid Node path admitted"),
            };
            assert!(error.contains("--node"));
            assert!(!error.contains("PUBLIC_private"));
        }
    }
    #[test]
    fn duplicate_or_missing_node_flag_refuses_without_launch() {
        let mut values = args();
        values.extend([
            "--node".into(),
            "/PUBLIC_private/first".into(),
            "--node".into(),
            "/PUBLIC_private/second".into(),
        ]);
        let error = match parse(values) {
            Err(error) => error,
            Ok(_) => panic!("duplicate Node admitted"),
        };
        assert_eq!(error, "Duplicate native application argument");
        let mut values = args();
        values.push("--node".into());
        assert!(parse(values).is_err());
    }
    #[test]
    fn help_describes_node_default_and_requires_no_runtime_or_node_probe() {
        assert!(USAGE.contains("--node ABS_NODE_EXECUTABLE"));
        assert!(USAGE.contains("/usr/bin/node, never PATH"));
        assert!(parse(["--help".into()]).unwrap().is_none());
    }
    #[test]
    fn no_runtime_fallback() {
        assert!(parse(["--expected-version".into(), "0.2.1-alpha.1".into()]).is_err());
    }
    #[test]
    fn version_mismatch_refuses() {
        let mut a = args();
        a[3] = "0.2.0-rc.2".into();
        assert!(parse(a).is_err());
    }
    #[test]
    fn duplicate_and_parent_traversal_refuse() {
        let mut a = args();
        a.extend(["--home".into(), "/other".into()]);
        assert!(parse(a).is_err());
        let mut a = args();
        a[1] = "/a/../runtime".into();
        assert!(parse(a).is_err());
    }
    fn private_test_root() -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("evidence")
            .join(format!(
                "portable-smoke-parser-{}-{nonce}",
                std::process::id()
            ));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        root
    }
    fn smoke_args(root: &std::path::Path) -> Vec<String> {
        let mut values = args();
        values.extend([
            "--smoke-new-session".into(),
            "--exit-after-seconds".into(),
            "8".into(),
            "--smoke-evidence-root".into(),
            root.to_string_lossy().into_owned(),
            "--smoke-report".into(),
            root.join("PUBLIC-app.json").to_string_lossy().into_owned(),
            "--smoke-screenshot".into(),
            root.join("PUBLIC-own-window.png")
                .to_string_lossy()
                .into_owned(),
        ]);
        values
    }
    #[test]
    fn explicit_private_smoke_root_is_portable_and_creates_nothing() {
        let root = private_test_root();
        let options = parse(smoke_args(&root)).unwrap().unwrap();
        let smoke = options.smoke.unwrap();
        assert_eq!(smoke.report, root.join("PUBLIC-app.json"));
        assert_eq!(
            smoke.screenshot.unwrap(),
            root.join("PUBLIC-own-window.png")
        );
        assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    }
    #[test]
    fn explicit_smoke_root_requires_opt_in_private_existing_directory() {
        use std::os::unix::fs::PermissionsExt;
        let root = private_test_root();
        let mut orphan = args();
        orphan.extend([
            "--smoke-evidence-root".into(),
            root.to_string_lossy().into_owned(),
        ]);
        assert!(parse(orphan).is_err());
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(parse(smoke_args(&root)).is_err());
        assert!(parse(smoke_args(&root.join("missing"))).is_err());
        let file = root.join("PUBLIC-not-directory");
        std::fs::write(&file, b"PUBLIC").unwrap();
        assert!(parse(smoke_args(&file)).is_err());
    }
    #[test]
    fn portable_smoke_outputs_stay_new_and_inside_selected_root() {
        let root = private_test_root();
        let mut outside = smoke_args(&root);
        let index = outside.iter().position(|s| s == "--smoke-report").unwrap() + 1;
        outside[index] = "/tmp/PUBLIC-unselected-report.json".into();
        assert!(parse(outside).is_err());
        std::fs::write(root.join("PUBLIC-app.json"), b"PUBLIC-preserve").unwrap();
        assert!(parse(smoke_args(&root)).is_err());
        assert_eq!(
            std::fs::read(root.join("PUBLIC-app.json")).unwrap(),
            b"PUBLIC-preserve"
        );
    }
    #[test]
    fn screenshot_requires_smoke_and_own_new_png() {
        let mut a = args();
        a.extend([
            "--smoke-screenshot".into(),
            "/tmp/not-authorized.png".into(),
        ]);
        assert!(parse(a).is_err());
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evidence");
        let mut base = args();
        base.extend([
            "--smoke-new-session".into(),
            "--exit-after-seconds".into(),
            "12".into(),
            "--smoke-report".into(),
            root.join("test-new-report-never-written.json")
                .to_string_lossy()
                .into_owned(),
        ]);
        let mut a = base.clone();
        a.extend([
            "--smoke-screenshot".into(),
            "/tmp/not-authorized.png".into(),
        ]);
        assert!(parse(a).is_err());
        let mut a = base.clone();
        a.extend([
            "--smoke-screenshot".into(),
            root.join("test-new-own-window-never-written.png")
                .to_string_lossy()
                .into_owned(),
        ]);
        assert!(
            parse(a)
                .unwrap()
                .unwrap()
                .smoke
                .unwrap()
                .screenshot
                .is_some()
        );
        base.extend([
            "--smoke-screenshot".into(),
            root.join("tests.log").to_string_lossy().into_owned(),
        ]);
        assert!(parse(base).is_err());
    }
}
