//! Package one tested Git checkout as an external-runtime Linux development build.
//!
//! Port of `.github/scripts/package_release.py`. Calendar versions publish only
//! explicitly; a plain semantic version gets a CI dev tag.
use crate::launcher::{
    MAX_BINARY, binary_digest, copy_new, elf_requirements, licenses, release_identity,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn command(root: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(program)
        .args(args)
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("{program} failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn readme() -> &'static str {
    "# Harness Native — development build\n\nLinux x86_64 GNU/Linux binary; built on Ubuntu 24.04. Not an installer, AppImage,\nself-contained backend, generally portable Linux build or finished desktop release.\nSee build-info.json for measured glibc symbol requirements and dynamic libraries.\n\nRequires a separately built @deepseek-ai/dsh 0.2.1-alpha.1 backend with its base+web\ncomposition, Node 26 (26.10.0 tested), native Wayland, usable GPU/EGL/Vulkan drivers,\nxkbcommon and DejaVu system fonts.\nThe backend, Node, private profiles and credentials are NOT included.\n\nRun from this extracted directory, replacing every absolute path:\n\n./bin/dsh-native-app --runtime /absolute/deepseek-harness-linux/apps/cli \\\n  --expected-version 0.2.1-alpha.1 --home /absolute/separate-native-home \\\n  --user-home /absolute/user-home --cwd /absolute/workspace\n\nOptional: --node /absolute/node-executable (default /usr/bin/node).\nKeep the native home separate from existing Harness profiles.\n\nCI checks compilation, headless regressions and --help, not rendered Wayland,\nphysical input, accessibility, real model/login behavior or complete feature parity.\nRust license inventory includes the lockfile/target superset, not legal certification.\nSource snapshot comes from the exact tagged Git commit; no local evidence is included.\n"
}

fn notes(identity: &Value, glibc: &str) -> String {
    format!(
        "Development prerelease from `{commit}` after successful Linux build and headless checks.\n\n- Package version: `{version}`; CI build: `{run}`.\n- Ubuntu 24.04 / Linux x86_64; required glibc symbols: `{glibc}`.\n- Requires the separate compatible alpha backend and Node 26. Not self-contained.\n- Includes executable, exact-commit source, Rust license inventory and SHA-256 checksum.\n- No real-provider, account, physical-input or rendered-GUI validation in this workflow.\n",
        commit = identity["commit"].as_str().unwrap_or_default(),
        version = identity["packageVersion"].as_str().unwrap_or_default(),
        run = identity["runNumber"].as_str().unwrap_or_default(),
        glibc = glibc,
    )
}

fn write_archive(payload: &Path, name: &str, archive: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(archive)
        .map_err(|error| error.to_string())?;
    let encoder = flate2::GzBuilder::new()
        .filename("")
        .mtime(0)
        .write(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    fn collect(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
        for entry in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            paths.push(path.clone());
            if path.is_dir() {
                collect(&path, paths)?;
            }
        }
        Ok(())
    }
    let mut paths = vec![payload.to_path_buf()];
    collect(payload, &mut paths)?;
    let mut rest = paths.split_off(1);
    rest.sort();
    paths.extend(rest);
    for path in paths {
        let relative = path
            .strip_prefix(payload)
            .map_err(|_| "Payload outside the package")?;
        let info = path.symlink_metadata().map_err(|error| error.to_string())?;
        let mut header = tar::Header::new_gnu();
        let executable = info.is_dir() || info.permissions().mode() & 0o111 != 0;
        header.set_mode(if executable { 0o755 } else { 0o644 });
        header.set_uid(0);
        header.set_gid(0);
        header.set_username("").map_err(|error| error.to_string())?;
        header
            .set_groupname("")
            .map_err(|error| error.to_string())?;
        header.set_mtime(0);
        let arcname = if relative.as_os_str().is_empty() {
            name.to_string()
        } else {
            format!("{name}/{}", relative.to_string_lossy())
        };
        if info.is_dir() {
            header.set_entry_type(tar::EntryType::Directory);
            header.set_size(0);
            builder
                .append_data(&mut header, arcname, std::io::empty())
                .map_err(|error| error.to_string())?;
        } else {
            header.set_entry_type(tar::EntryType::Regular);
            header.set_size(info.len());
            let mut file = std::fs::File::open(&path).map_err(|error| error.to_string())?;
            builder
                .append_data(&mut header, arcname, &mut file)
                .map_err(|error| error.to_string())?;
        }
    }
    let encoder = builder.into_inner().map_err(|error| error.to_string())?;
    encoder.finish().map_err(|error| error.to_string())?;
    Ok(())
}

/// Build the release payload, archive and checksums.
pub fn package_run(
    root: &Path,
    binary: &Path,
    output: &Path,
    env: &BTreeMap<String, String>,
) -> i32 {
    let Ok(commit) = command(root, "git", &["rev-parse", "HEAD"]) else {
        eprintln!("package release failed: cannot resolve HEAD");
        return 1;
    };
    if env.get("GITHUB_SHA") != Some(&commit) {
        eprintln!("package release failed: checkout differs from the triggering commit");
        return 1;
    }
    let Ok(manifest) = std::fs::read_to_string(root.join("app/Cargo.toml")) else {
        eprintln!("package release failed: app manifest missing");
        return 1;
    };
    let run_number = env.get("GITHUB_RUN_NUMBER").cloned().unwrap_or_default();
    let Ok(identity) = release_identity(&manifest, &commit, &run_number) else {
        eprintln!("package release failed: invalid release identity");
        return 1;
    };
    if identity["automaticRelease"].as_bool() != Some(true) {
        println!(
            "{}",
            serde_json::json!({
                "status": "checks-only-explicit-calendar-publication",
                "packageVersion": identity["packageVersion"],
                "releaseTag": identity["releaseTag"],
                "commit": identity["commit"],
                "runNumber": identity["runNumber"],
                "automaticRelease": false,
            })
        );
        return 0;
    }
    let Ok(binary) = binary.canonicalize() else {
        eprintln!("package release failed: expected a built executable");
        return 1;
    };
    use std::os::unix::fs::PermissionsExt;
    let executable = std::fs::symlink_metadata(&binary)
        .map(|info| info.is_file() && info.permissions().mode() & 0o111 != 0)
        .unwrap_or(false);
    if !executable {
        eprintln!("package release failed: expected a built executable");
        return 1;
    }
    let output = if output.is_absolute() {
        output.to_path_buf()
    } else {
        root.join(output)
    };
    if std::fs::create_dir(&output).is_err() {
        eprintln!("package release failed: output already exists");
        return 1;
    }
    let tag = identity["releaseTag"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let name = format!("dsh-native-linux-x86_64-{tag}");
    let payload = output.join(&name);
    let result = (|| -> Result<Value, String> {
        std::fs::create_dir_all(payload.join("bin")).map_err(|error| error.to_string())?;
        copy_new(
            &binary,
            &payload.join("bin/dsh-native-app"),
            0o755,
            MAX_BINARY,
        )?;
        copy_new(
            &root.join("LICENSE"),
            &payload.join("LICENSE"),
            0o644,
            1024 * 1024,
        )?;
        let version_info = command(
            root,
            "readelf",
            &["--version-info", &binary.to_string_lossy()],
        )?;
        let dynamic_info = command(root, "readelf", &["-d", &binary.to_string_lossy()])?;
        let requirements = elf_requirements(&version_info, &dynamic_info)?;
        let cargo_home = env
            .get("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| env.get("HOME").map(|home| Path::new(home).join(".cargo")))
            .unwrap_or_else(|| PathBuf::from("/nonexistent"));
        let metadata: Value = serde_json::from_str(&command(
            root,
            "cargo",
            &[
                "metadata",
                "--locked",
                "--offline",
                "--format-version",
                "1",
                "--manifest-path",
                &root.join("app/Cargo.toml").to_string_lossy(),
            ],
        )?)
        .map_err(|_| "Invalid bounded Cargo metadata package list")?;
        let inventory = licenses(&metadata, &cargo_home.join("registry/src"), &payload)?;
        let source_archive = payload.join("source.tar.gz");
        let status = std::process::Command::new("git")
            .args([
                "archive",
                "--format=tar.gz",
                &format!("--output={}", source_archive.to_string_lossy()),
                &commit,
            ])
            .current_dir(root)
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err("git archive failed".into());
        }
        std::fs::write(payload.join("README.txt"), readme()).map_err(|error| error.to_string())?;
        let info = serde_json::json!({
            "packageVersion": identity["packageVersion"],
            "releaseTag": tag,
            "commit": commit,
            "runNumber": run_number,
            "automaticRelease": true,
            "minimumGlibcSymbols": requirements["minimumGlibcSymbols"],
            "dynamicDependencies": requirements["dynamicDependencies"],
            "target": "x86_64-unknown-linux-gnu",
            "runner": "ubuntu-24.04",
            "rust": command(root, "rustc", &["--version"])?,
            "node": command(root, "/usr/bin/node", &["--version"])?,
            "binarySha256": binary_digest(&payload.join("bin/dsh-native-app"))?,
            "rustLicensePackageCount": inventory["packageCount"],
            "developmentPrerelease": true,
            "externalBackendRequired": "0.2.1-alpha.1",
            "guiOrRealModelVerified": false,
        });
        std::fs::write(
            payload.join("build-info.json"),
            format!(
                "{}\n",
                serde_json::to_string_pretty(&info).map_err(|error| error.to_string())?
            ),
        )
        .map_err(|error| error.to_string())?;
        let archive = output.join(format!("{name}.tar.gz"));
        write_archive(&payload, &name, &archive)?;
        let digest = binary_digest(&archive)?;
        std::fs::write(
            output.join("SHA256SUMS"),
            format!(
                "{digest}  {}\n",
                archive.file_name().unwrap_or_default().to_string_lossy()
            ),
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            output.join("release-notes.md"),
            notes(
                &identity,
                requirements["minimumGlibcSymbols"]
                    .as_str()
                    .unwrap_or_default(),
            ),
        )
        .map_err(|error| error.to_string())?;
        Ok(info)
    })();
    match result {
        Ok(info) => {
            if let Some(path) = env.get("GITHUB_OUTPUT") {
                let mut append = std::fs::OpenOptions::new().append(true).open(path).ok();
                if let Some(file) = append.as_mut() {
                    use std::io::Write;
                    let _ = writeln!(file, "tag={tag}");
                }
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&info).unwrap_or_default()
            );
            0
        }
        Err(message) => {
            eprintln!("package release failed: {message}");
            1
        }
    }
}

/// `xtask release package --binary PATH --output NEW_DIR`.
pub fn run(args: &[String]) -> i32 {
    let mut binary = None;
    let mut output = None;
    let mut index = 0;
    while index < args.len() {
        let value = |index: &mut usize| -> Option<String> {
            *index += 1;
            args.get(*index).cloned()
        };
        match args[index].as_str() {
            "package" => {}
            "--binary" => binary = value(&mut index),
            "--output" => output = value(&mut index),
            other => {
                eprintln!("release: unknown argument {other}");
                return 1;
            }
        }
        index += 1;
    }
    let (Some(binary), Some(output)) = (binary, output) else {
        eprintln!("release package needs --binary and --output");
        return 1;
    };
    let root = crate::root();
    let env: BTreeMap<String, String> = std::env::vars().collect();
    package_run(&root, Path::new(&binary), Path::new(&output), &env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_release_readme_and_notes_state_the_unqualified_scope() {
        let text = readme();
        assert!(text.contains("Not an installer"));
        assert!(
            !text.contains("Python 3.11+"),
            "the archive no longer needs Python"
        );
        assert!(text.contains("Source snapshot comes from the exact tagged Git commit"));
        let identity = serde_json::json!({"commit": "a".repeat(40), "packageVersion": "0.2.1", "runNumber": "7"});
        let notes = notes(&identity, "2.34");
        assert!(notes.contains("`2.34`"));
        assert!(notes.contains("Not self-contained"));
    }
}
