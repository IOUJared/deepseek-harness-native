//! Explicit raw-byte upload using alpha's Session-addressed route, which may resume a cold Agent.
//! Receipt success means storage/staging, not prompt admission, image validation or a model call.
use crate::{
    Error, NativeClient, Result,
    dto::{FileUploadValue, SessionId},
    http::read_response,
};
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, COOKIE, ORIGIN};
use serde_json::Value;
use std::fmt;
use tokio_util::task::AbortOnDropHandle;

/// Native in-memory intake ceiling, not a Host/provider file-size limit.
pub const MAX_UPLOAD_BYTES: usize = 4 * 1024 * 1024;
/// Per-generation active plus queued upload workers; excess admission returns QueueFull.
pub const MAX_UPLOAD_JOBS: usize = 8;
const MAX_NAME_BYTES: usize = 1024;
const MAX_RECEIPT_BYTES: usize = 16 * 1024;

/// One-use bounded file body. No Clone, serialization or public bytes/name getter.
/// The allocation moves into reqwest; caller, HTTP, kernel and Host copies are not wiped.
/// ```compile_fail
/// use dsh_native_transport::upload::FileUpload;
/// fn copy(file: &FileUpload) { let _: FileUpload = file.clone(); }
/// ```
/// ```compile_fail
/// use dsh_native_transport::upload::FileUpload;
/// fn publish(file: &FileUpload) { serde_json::to_string(file).unwrap(); }
/// ```
pub struct FileUpload {
    bytes: Vec<u8>,
    name: Option<String>,
}
impl FileUpload {
    /// Validate the complete body before network admission. Empty ordinary files are allowed.
    /// Names are optional display leaves, not storage destinations; no trimming or path resolution.
    pub fn new(bytes: Vec<u8>, name: Option<String>) -> Result<Self> {
        if bytes.len() > MAX_UPLOAD_BYTES {
            return Err(Error::Oversize);
        }
        if name.as_deref().is_some_and(|name| !valid_name(name)) {
            return Err(Error::InvalidDto);
        }
        Ok(Self { bytes, name })
    }
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}
impl fmt::Debug for FileUpload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileUpload")
            .field("bytes", &self.len())
            .finish_non_exhaustive()
    }
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && name != "."
        && name != ".."
        && !name
            .chars()
            .any(|ch| ch.is_control() || ch == '/' || ch == '\\')
}
fn valid_receipt_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 4096
        && !id.chars().any(|ch| ch.is_control() || ch.is_whitespace())
}
fn receipt(value: Value, expected_bytes: usize) -> Result<FileUploadValue> {
    let obj = value.as_object().ok_or(Error::InvalidFrame)?;
    match obj.get("ok").and_then(Value::as_bool) {
        Some(false) => {
            let error = obj
                .get("error")
                .and_then(Value::as_object)
                .ok_or(Error::InvalidFrame)?;
            if obj.len() != 2
                || error.len() != 3
                || !error.get("code").is_some_and(Value::is_string)
                || !error.get("message").is_some_and(Value::is_string)
                || !error.get("details").is_some_and(Value::is_object)
            {
                return Err(Error::InvalidFrame);
            }
            Err(Error::RemoteFailure)
        }
        Some(true) => {
            let value = obj
                .get("value")
                .and_then(Value::as_object)
                .ok_or(Error::InvalidFrame)?;
            let file = value
                .get("file")
                .and_then(Value::as_object)
                .ok_or(Error::InvalidDto)?;
            if obj.len() != 2 || value.len() != 2 || file.len() != 3 {
                return Err(Error::InvalidDto);
            }
            let found: FileUploadValue = serde_json::from_value(Value::Object(value.clone()))
                .map_err(|_| Error::InvalidDto)?;
            if !valid_receipt_id(found.receipt_id.as_str())
                || !valid_receipt_id(found.file.attachment_id.as_str())
                || !valid_name(&found.file.name)
                || found.file.bytes != expected_bytes as u64
            {
                return Err(Error::InvalidDto);
            }
            Ok(found)
        }
        None => Err(Error::InvalidFrame),
    }
}
impl NativeClient {
    /// Consume a bounded body through POST /api/session/uploadFileBinary, Cookie/Origin and no redirects.
    /// Alpha addresses a Session and may resume its ordinary Agent; it does not require an AgentId query.
    /// One deadline includes semaphore queueing, send and complete bounded JSON receipt decoding.
    /// Owner invalidation or dropping this future cancels the local carrier, not a Host rollback.
    /// Errors after calling this method are unconfirmed outcomes: never automatically retry or submit a prompt.
    pub async fn upload_file(
        &self,
        session_id: &SessionId,
        file: FileUpload,
    ) -> Result<FileUploadValue> {
        self.inner.authority()?;
        let slot = self
            .inner
            .upload_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                if self.inner.lifetime.is_cancelled() {
                    Error::Closed
                } else {
                    Error::QueueFull
                }
            })?;
        let deadline = tokio::time::Instant::now() + self.inner.limits.timeout;
        let owner = self.clone();
        let selected = session_id.clone();
        // The tracked worker owns admission, so close can drain even a parked caller future.
        let task = self.inner.tasks.spawn(async move {
            let _slot = slot;
            if owner.inner.lifetime.is_cancelled() {
                return Err(Error::Closed);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Timeout);
            }
            tokio::select! {
                biased;
                _ = owner.inner.lifetime.cancelled() => Err(Error::Closed),
                _ = tokio::time::sleep_until(deadline) => Err(Error::Timeout),
                result = owner.upload_active(&selected, file, deadline) => result,
            }
        });
        let result = AbortOnDropHandle::new(task)
            .await
            .unwrap_or(Err(Error::Network));
        if self.inner.lifetime.is_cancelled() {
            Err(Error::Closed)
        } else {
            result
        }
    }
    async fn upload_active(
        &self,
        session_id: &SessionId,
        file: FileUpload,
        deadline: tokio::time::Instant,
    ) -> Result<FileUploadValue> {
        let _permit = self
            .inner
            .rpc_slots
            .acquire()
            .await
            .map_err(|_| Error::Closed)?;
        let auth = self.inner.authority()?;
        let mut url = auth
            .origin
            .join("api/session/uploadFileBinary")
            .map_err(|_| Error::InvalidLaunch)?;
        url.query_pairs_mut()
            .append_pair("sessionId", session_id.as_str());
        if let Some(name) = &file.name {
            url.query_pairs_mut().append_pair("name", name);
        }
        let expected_bytes = file.len();
        if tokio::time::Instant::now() >= deadline {
            return Err(Error::Timeout);
        }
        let response = self
            .inner
            .http
            .post(url)
            .header(ORIGIN, auth.origin.origin().ascii_serialization())
            .header(COOKIE, auth.cookie)
            .header(CONTENT_TYPE, "application/octet-stream")
            .header(CONTENT_LENGTH, expected_bytes.to_string())
            .body(file.bytes)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    Error::Timeout
                } else {
                    Error::Network
                }
            })?;
        let mut limits = self.inner.limits;
        limits.max_result_bytes = limits.max_result_bytes.min(MAX_RECEIPT_BYTES);
        limits.max_json_items = limits.max_json_items.min(128);
        let result = read_response(response, limits)
            .await
            .and_then(|value| receipt(value, expected_bytes));
        if tokio::time::Instant::now() >= deadline {
            Err(Error::Timeout)
        } else {
            result
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/upload_wire.rs"]
mod wire_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn complete_body_name_bounds_and_debug_are_private() {
        assert!(FileUpload::new(vec![0; MAX_UPLOAD_BYTES], None).is_ok());
        assert!(matches!(
            FileUpload::new(vec![0; MAX_UPLOAD_BYTES + 1], None),
            Err(Error::Oversize)
        ));
        assert!(FileUpload::new(Vec::new(), None).unwrap().is_empty());
        assert!(FileUpload::new(vec![1], Some("é".repeat(512))).is_ok());
        for name in ["", ".", "..", "a/b", "a\\b", "a\n", "a\u{0085}"] {
            assert!(matches!(
                FileUpload::new(vec![1], Some(name.into())),
                Err(Error::InvalidDto)
            ));
        }
        assert!(matches!(
            FileUpload::new(vec![1], Some("é".repeat(513))),
            Err(Error::InvalidDto)
        ));
        let file = FileUpload::new(
            b"PUBLIC_FILE_BODY".to_vec(),
            Some("PUBLIC_PRIVATE_NAME".into()),
        )
        .unwrap();
        let debug = format!("{file:?}");
        assert!(!debug.contains("PUBLIC_FILE_BODY"));
        assert!(!debug.contains("PUBLIC_PRIVATE_NAME"));
    }
    #[test]
    fn receipt_requires_complete_matching_metadata_without_error_text() {
        let good = json!({"ok":true,"value":{"receiptId":"receipt","file":{"attachmentId":"sha256:PUBLIC","name":"PUBLIC file.txt","bytes":3}}});
        assert_eq!(receipt(good.clone(), 3).unwrap().file.bytes, 3);
        assert!(matches!(receipt(good.clone(), 4), Err(Error::InvalidDto)));
        for field in ["receiptId", "file"] {
            let mut bad = good.clone();
            bad["value"].as_object_mut().unwrap().remove(field);
            assert!(receipt(bad, 3).is_err());
        }
        let mut bad = good.clone();
        bad["value"]["file"]["extra"] = json!("PUBLIC_SECRET");
        assert!(receipt(bad, 3).is_err());
        let mut bad = good.clone();
        bad["value"]["file"]["name"] = json!("../escape");
        assert!(receipt(bad, 3).is_err());
        assert_eq!(receipt(json!({"ok":false,"error":{"code":"session/attachment-invalid","message":"PUBLIC_SECRET","details":{"secret":"PUBLIC_SECRET"}}}), 3).unwrap_err(), Error::RemoteFailure);
        assert!(receipt(json!({"ok":false,"error":{}}), 3).is_err());
    }
}
