use dsh_native_core::{RustBackendOptions, StopPolicy};
use std::{collections::HashMap, path::PathBuf, time::Duration};

/// Explicit isolated qualification inputs, subsequently validated by the real Core constructor.
pub fn parse() -> Result<RustBackendOptions, &'static str> {
    let mut fields = HashMap::new();
    let mut args = std::env::args().skip(1);
    while let Some(key) = args.next() {
        if ![
            "--runtime",
            "--expected-version",
            "--home",
            "--cwd",
            "--user-home",
            "--node",
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
    let mut take = |key: &str| fields.remove(key).ok_or("missing-required-argument");
    let value = RustBackendOptions {
        runtime: PathBuf::from(take("--runtime")?),
        expected_version: take("--expected-version")?,
        native_home: PathBuf::from(take("--home")?),
        working_directory: PathBuf::from(take("--cwd")?),
        user_home: PathBuf::from(take("--user-home")?),
        absolute_node_path: fields.remove("--node").map(PathBuf::from),
        startup_timeout: Duration::from_secs(15),
        stop_policy: StopPolicy::default(),
    };
    if [
        &value.runtime,
        &value.native_home,
        &value.working_directory,
        &value.user_home,
    ]
    .iter()
    .any(|p| !p.is_absolute() || !p.is_dir())
    {
        return Err("existing-absolute-directories-required");
    }
    Ok(value)
}
