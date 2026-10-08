//! Explicit developer smoke: PUBLIC fake key in a new private test home only.
//! No model/catalog, browser sign-in, real credential, installed profile or external API request.
use dsh_native_core::{
    Backend, Command, PublicReply, RustBackendOptions, SecretApiKey, StopPolicy,
};
use serde_json::{Value, json};
use std::{collections::HashMap, fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};

const PUBLIC_FIXTURE: &str = "PUBLIC_NATIVE_API_KEY_SMOKE_ONLY";
fn options() -> Result<RustBackendOptions, &'static str> {
    let mut fields = HashMap::new();
    let mut args = std::env::args().skip(1);
    while let Some(key) = args.next() {
        if ![
            "--runtime",
            "--expected-version",
            "--home",
            "--cwd",
            "--user-home",
        ]
        .contains(&key.as_str())
        {
            return Err("invalid-arguments");
        }
        if fields
            .insert(key, args.next().ok_or("missing-value")?)
            .is_some()
        {
            return Err("duplicate-argument");
        }
    }
    let mut take = |key: &str| fields.remove(key).ok_or("missing-argument");
    let value = RustBackendOptions {
        runtime: PathBuf::from(take("--runtime")?),
        expected_version: take("--expected-version")?,
        native_home: PathBuf::from(take("--home")?),
        working_directory: PathBuf::from(take("--cwd")?),
        user_home: PathBuf::from(take("--user-home")?),
        absolute_node_path: None,
        startup_timeout: Duration::from_secs(15),
        stop_policy: StopPolicy::default(),
    };
    let boundary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../smoke-home")
        .canonicalize()
        .map_err(|_| "test-boundary-unavailable")?;
    for directory in [
        &value.native_home,
        &value.working_directory,
        &value.user_home,
    ] {
        let metadata = fs::symlink_metadata(directory).map_err(|_| "test-directory-unavailable")?;
        let canonical = directory
            .canonicalize()
            .map_err(|_| "test-directory-unavailable")?;
        if !directory.is_absolute()
            || !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || !canonical.starts_with(&boundary)
            || metadata.permissions().mode() & 0o777 != 0o700
            || fs::read_dir(directory)
                .map_err(|_| "test-directory-unavailable")?
                .next()
                .is_some()
        {
            return Err("new-empty-private-smoke-directories-required");
        }
    }
    Ok(value)
}
fn checks(backend: &Backend, home: &PathBuf, report: &mut Value) -> Result<(), String> {
    backend.wait_ready().map_err(|e| e.to_string())?;
    report["coreReady"] = json!(true);
    let PublicReply::Onboarding(before) = backend
        .request(Command::OnboardingRead, Duration::from_secs(5))
        .map_err(|e| e.to_string())?
    else {
        return Err("onboarding-reply-required".into());
    };
    if before.has_api_key || before.logged_in || !before.writable {
        return Err("empty-writable-test-provider-required".into());
    }
    report["initialCredentialAbsent"] = json!(true);
    let secret = SecretApiKey::new(PUBLIC_FIXTURE.to_owned()).map_err(|e| e.to_string())?;
    if !backend
        .save_api_key(secret, Duration::from_secs(5))
        .map_err(|e| e.to_string())?
    {
        return Err("fixture-write-refused".into());
    }
    report["persistenceAcknowledged"] = json!(true);
    let PublicReply::Onboarding(after) = backend
        .request(Command::OnboardingRead, Duration::from_secs(5))
        .map_err(|e| e.to_string())?
    else {
        return Err("onboarding-reply-required".into());
    };
    if !after.has_api_key || after.logged_in {
        return Err("metadata-after-fixture-write-invalid".into());
    }
    report["credentialMetadataPresent"] = json!(true);
    report["browserAccountStillSignedOut"] = json!(true);
    let file = home.join(".credentials.yaml");
    let metadata = fs::symlink_metadata(&file).map_err(|_| "fixture-file-missing")?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o777 != 0o600
        || file
            .canonicalize()
            .map_err(|_| "fixture-file-unavailable")?
            .parent()
            != Some(
                home.canonicalize()
                    .map_err(|_| "test-home-unavailable")?
                    .as_path(),
            )
    {
        return Err("fixture-file-not-private-local".into());
    }
    let bytes = fs::read(&file).map_err(|_| "fixture-file-unavailable")?;
    if !bytes
        .windows(PUBLIC_FIXTURE.len())
        .any(|part| part == PUBLIC_FIXTURE.as_bytes())
    {
        return Err("fixture-value-not-persisted".into());
    }
    report["defaultProviderPrivateLocalFile"] = json!(true);
    report["fixtureValueVerifiedWithoutPrinting"] = json!(true);
    Ok(())
}
fn main() {
    let mut report = json!({"scope":"real-owned-keyless-default-provider-public-fixture-persistence", "publicFixtureOnly":true, "modelPrompts":0, "modelCatalogRequested":false, "browserSignInRequested":false, "realCredentialsUsed":false});
    let options = match options() {
        Ok(value) => value,
        Err(error) => {
            report["errorCode"] = json!(error);
            report["status"] = json!("failed");
            println!("{report}");
            std::process::exit(1);
        }
    };
    let home = options.native_home.clone();
    let (backend, _events) = match Backend::start(options) {
        Ok(value) => value,
        Err(error) => {
            report["errorCode"] = json!(error.to_string());
            report["status"] = json!("failed");
            println!("{report}");
            std::process::exit(1);
        }
    };
    let result = checks(&backend, &home, &mut report);
    if let Err(error) = &result {
        report["errorCode"] = json!(error);
    }
    let stop = backend.stop();
    let clean = match stop {
        Ok(stop) => {
            report["stop"] = json!({"exited":stop.exited,"exitCode":stop.exit_code,"graceful":stop.graceful,"observedDescendantsRemaining":stop.observed_descendants_remaining,"containmentUnknown":stop.containment_unknown});
            stop.exited
                && stop.exit_code == Some(0)
                && stop.graceful
                && stop.observed_descendants_remaining == 0
                && !stop.containment_unknown
        }
        Err(error) => {
            report["stopError"] = json!(error.to_string());
            false
        }
    };
    let passed = result.is_ok() && clean;
    report["status"] = json!(if passed { "passed" } else { "failed" });
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    if !passed {
        std::process::exit(1);
    }
}
