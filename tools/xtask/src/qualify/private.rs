//! Bounded, owner-private artifact reads with strict JSON.
//!
//! Shared by the draft qualifier: a private artifact must be a single-link,
//! owner-only regular file read through `O_NOFOLLOW`; a JSON body must not repeat
//! a key, because "last one wins" would let a crafted report hide a second value.
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use std::ffi::CString;
use std::fs;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Read at most `limit` bytes from a private regular file and require the opened
/// identity to match the one observed before opening.
pub fn bytes(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let before = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !before.file_type().is_file()
        || before.mode() & 0o777 != 0o600
        || before.uid() != unsafe { libc::geteuid() }
        || before.nlink() != 1
        || before.size() == 0
        || before.size() > limit
    {
        return Err("private-regular-artifact-required".into());
    }
    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| "path-with-nul")?;
    // Safe: a NUL-terminated path and constant flags; the descriptor is closed below.
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err("private-artifact-not-openable".into());
    }
    let mut opened: libc::stat = unsafe { std::mem::zeroed() };
    // Safe: a live descriptor and a valid out-parameter.
    let stat_ok = unsafe { libc::fstat(fd, &mut opened) } == 0;
    let matches = stat_ok
        && opened.st_dev == before.dev()
        && opened.st_ino == before.ino()
        && opened.st_mode == before.mode()
        && opened.st_uid == before.uid()
        && opened.st_nlink == before.nlink()
        && opened.st_size as u64 == before.size();
    if !matches {
        unsafe { libc::close(fd) };
        return Err("private-artifact-changed".into());
    }
    use std::os::unix::io::FromRawFd;
    let mut file = unsafe { fs::File::from_raw_fd(fd) };
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|error| error.to_string())?;
    if data.len() as u64 != before.size() {
        return Err("private-artifact-size-changed".into());
    }
    Ok(data)
}

/// Parse a private JSON object, refusing duplicate keys at any depth.
pub fn json(path: &Path, limit: u64) -> Result<Value, String> {
    let data = bytes(path, limit)?;
    let parsed: NoDuplicates = serde_json::from_slice(&data).map_err(|error| error.to_string())?;
    if !parsed.0.is_object() {
        return Err("private-object-required".into());
    }
    Ok(parsed.0)
}

struct NoDuplicates(Value);

impl<'de> Deserialize<'de> for NoDuplicates {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(Strict)
    }
}

struct Strict;

impl<'de> Visitor<'de> for Strict {
    type Value = NoDuplicates;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value whose objects never repeat a key")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(NoDuplicates(Value::Bool(value)))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(NoDuplicates(Value::from(value)))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(NoDuplicates(Value::from(value)))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        Ok(NoDuplicates(Value::from(value)))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(NoDuplicates(Value::String(value.to_string())))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(NoDuplicates(Value::String(value)))
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicates(Value::Null))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicates(Value::Null))
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element::<NoDuplicates>()? {
            values.push(value.0);
        }
        Ok(NoDuplicates(Value::Array(values)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if object.contains_key(&key) {
                return Err(de::Error::custom("duplicate-private-json-key"));
            }
            object.insert(key, map.next_value::<NoDuplicates>()?.0);
        }
        Ok(NoDuplicates(Value::Object(object)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn private(path: &Path, body: &[u8]) {
        fs::write(path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn a_private_artifact_must_be_single_link_owner_only_and_bounded() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "private-").unwrap();
        let path = base.join("report.json");
        private(&path, b"{\"a\": 1}");
        assert!(bytes(&path, 1024).is_ok());
        // A public mode is refused.
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(bytes(&path, 1024).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        // A hard link (nlink 2) is refused.
        fs::hard_link(&path, base.join("other")).unwrap();
        assert!(bytes(&path, 1024).is_err());
        fs::remove_file(base.join("other")).unwrap();
        // A symlink, a FIFO, a directory and an oversize body are refused.
        std::os::unix::fs::symlink(&path, base.join("link")).unwrap();
        assert!(bytes(&base.join("link"), 1024).is_err());
        let fifo = CString::new(base.join("fifo").to_string_lossy().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(bytes(&base.join("fifo"), 1024).is_err());
        assert!(bytes(&base, 1024).is_err());
        assert!(bytes(&path, 4).is_err(), "a body over the bound is refused");
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_json_body_with_a_repeated_key_is_refused_at_any_depth() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "private-json-").unwrap();
        let path = base.join("doc.json");
        private(&path, b"{\"a\": 1, \"b\": {\"c\": 2}}");
        assert_eq!(json(&path, 4096).unwrap()["b"]["c"], Value::from(2));
        private(&path, b"{\"a\": 1, \"a\": 2}");
        assert!(
            json(&path, 4096).is_err(),
            "a top-level duplicate is refused"
        );
        private(&path, b"{\"b\": {\"c\": 1, \"c\": 2}}");
        assert!(json(&path, 4096).is_err(), "a nested duplicate is refused");
        private(&path, b"[1, 2]");
        assert!(json(&path, 4096).is_err(), "a non-object report is refused");
        fs::remove_dir_all(base).unwrap();
    }
}
