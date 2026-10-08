use crate::{Error, StopPolicy};
use std::{
    os::unix::fs::PermissionsExt,
    path::{Component, Path, PathBuf},
    time::Duration,
};

/// Caller-owned configuration. No runtime, Node, credentials, or home defaults come from ambient DSH variables.
#[derive(Clone, Debug)]
pub struct RustBackendOptions {
    pub runtime: PathBuf,
    pub expected_version: String,
    pub native_home: PathBuf,
    pub user_home: PathBuf,
    pub working_directory: PathBuf,
    /// None selects /usr/bin/node, never PATH or the working directory.
    pub absolute_node_path: Option<PathBuf>,
    pub startup_timeout: Duration,
    pub stop_policy: StopPolicy,
}

pub(crate) struct ValidatedOptions {
    pub runtime: PathBuf,
    pub version: String,
    pub home: PathBuf,
    pub user_home: PathBuf,
    pub cwd: PathBuf,
    pub node: PathBuf,
    pub startup_timeout: Duration,
    pub stop: StopPolicy,
}

pub(crate) fn absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .to_str()
            .is_some_and(|s| !s.bytes().any(|b| b < 32 || b == 127))
        && !path.components().any(|c| matches!(c, Component::ParentDir))
}

fn future_path(path: &Path) -> Result<PathBuf, Error> {
    match path.canonicalize() {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or(Error::InvalidOptions)?;
            Ok(future_path(parent)?.join(path.file_name().ok_or(Error::InvalidOptions)?))
        }
        Err(_) => Err(Error::InvalidOptions),
    }
}

impl RustBackendOptions {
    pub(crate) fn validate(&self) -> Result<ValidatedOptions, Error> {
        let node = self
            .absolute_node_path
            .as_deref()
            .unwrap_or(Path::new("/usr/bin/node"));
        if [
            &self.runtime,
            &self.native_home,
            &self.user_home,
            &self.working_directory,
            node,
        ]
        .iter()
        .any(|p| !absolute(p))
            || self.expected_version != "0.2.1-alpha.1"
            || self.startup_timeout.is_zero()
            || self.startup_timeout > Duration::from_secs(30)
            || !self.stop_policy.valid()
        {
            return Err(Error::InvalidOptions);
        }
        let runtime = self
            .runtime
            .canonicalize()
            .map_err(|_| Error::InvalidOptions)?;
        let cwd = self
            .working_directory
            .canonicalize()
            .map_err(|_| Error::InvalidOptions)?;
        let user_home = self
            .user_home
            .canonicalize()
            .map_err(|_| Error::InvalidOptions)?;
        let home = future_path(&self.native_home)?;
        if !runtime.is_dir()
            || !cwd.is_dir()
            || !user_home.is_dir()
            || home == Path::new("/")
            || home == user_home
            || user_home.starts_with(&home)
            || home == user_home.join(".local/share")
        {
            return Err(Error::InvalidOptions);
        }
        let mut forbidden = vec![future_path(&user_home.join(".dsh"))?];
        // Inherited DSH_HOME is only an isolation exclusion, never a configuration fallback.
        if let Some(ambient) = std::env::var_os("DSH_HOME") {
            let ambient = PathBuf::from(ambient);
            if !absolute(&ambient) {
                return Err(Error::InvalidOptions);
            }
            forbidden.push(future_path(&ambient)?);
        }
        if forbidden
            .iter()
            .any(|p| home.starts_with(p) || p.starts_with(&home))
        {
            return Err(Error::InvalidOptions);
        }
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(runtime.join("package.json")).map_err(|_| Error::InvalidOptions)?,
        )
        .map_err(|_| Error::InvalidOptions)?;
        if manifest["name"] != "@deepseek-ai/dsh" || manifest["version"] != self.expected_version {
            return Err(Error::InvalidOptions);
        }
        let node = node.canonicalize().map_err(|_| Error::InvalidOptions)?;
        let metadata = node.metadata().map_err(|_| Error::InvalidOptions)?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
            return Err(Error::InvalidOptions);
        }
        Ok(ValidatedOptions {
            runtime,
            version: self.expected_version.clone(),
            home,
            user_home,
            cwd,
            node,
            startup_timeout: self.startup_timeout,
            stop: self.stop_policy,
        })
    }
}
