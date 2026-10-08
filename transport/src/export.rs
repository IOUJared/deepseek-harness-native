//! Root-only binary Session export; no destination, filename, or extraction authority.
//! The default Host flushes the live persisted log and includes its referenced attachments.
//! Export is not a pure storage read: abort does not roll back flush effects, and arbitrary
//! Host flush listeners may have other effects, including external network activity.
//! The download bound does not bound the Host's complete root-log read/serialization.
//! This checks a conservative classic-ZIP container, not DEFLATE, CRC, log contents,
//! attachment integrity, or extraction safety. ZIP64, split, encrypted, prefixed,
//! extra-field-bearing, and noncontiguous archives are rejected. The pinned alpha profile
//! requires a first, unique `session.v4.jsonl`, then unique UTF-8 `media/…` or `files/…`
//! regular-file names without backslashes, controls, empty, dot, or dot-dot components.
//! Advertised uncompressed sizes are capped at 64 MiB per entry / 256 MiB total; these
//! are metadata-only checks, not bounds on actual DEFLATE output. The 32 MiB download
//! ceiling intentionally refuses some otherwise legitimate large exports.
//! Use a maintained external ZIP reader for content checks; never extract entry paths
//! merely because this transport accepted them. Attachment naming/content and Session
//! identity are not authenticated by this structural/profile guard.

use crate::{Error, NativeClient, Result, dto::SessionId};
use reqwest::header::{CONTENT_ENCODING, CONTENT_TYPE, COOKIE, ORIGIN};
use std::{
    collections::HashSet,
    fmt, io,
    sync::atomic::{Ordering, compiler_fence},
};
use tokio::time::{Instant, timeout_at};

const MAX_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
const MAX_ADVERTISED_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ADVERTISED_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
// Pinned alpha exports SESSION_FORMAT_VERSION = 4 through sessionFormatLogFilename.
const ROOT_LOG: &str = "session.v4.jsonl";

/// Owned, sensitive export with container structure checked but entry contents unverified.
/// No Clone, Serialize, byte accessor, or adopted remote filename is provided.
/// Consuming writes and all drops best-effort wipe the owned downloaded allocation.
/// This does not guarantee erasure of allocator reallocation, reqwest, kernel, writer,
/// or other copies. `write_to` is not atomic and an I/O error may leave partial output.
/// ```compile_fail
/// use dsh_native_transport::export::SessionArchive;
/// fn copy(archive: &SessionArchive) { let _: SessionArchive = archive.clone(); }
/// ```
/// ```compile_fail
/// use dsh_native_transport::export::SessionArchive;
/// fn publish(archive: &SessionArchive) { serde_json::to_string(archive).unwrap(); }
/// ```
pub struct SessionArchive {
    bytes: Vec<u8>,
}
impl SessionArchive {
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
    /// Consume the archive into an owner-selected writer, then wipe the owned buffer.
    pub fn write_to(self, writer: &mut impl io::Write) -> io::Result<usize> {
        writer.write_all(&self.bytes)?;
        Ok(self.len())
    }
}
impl fmt::Debug for SessionArchive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionArchive")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}
impl Drop for SessionArchive {
    fn drop(&mut self) {
        // black_box + fence discourage dead-store removal without adding unsafe code.
        std::hint::black_box(self.bytes.as_mut_slice()).fill(0);
        compiler_fence(Ordering::SeqCst);
    }
}

impl NativeClient {
    /// GET only `/api/session.export?sessionId=…&includeDescendants=false` using the
    /// existing private Cookie/Origin client with redirects disabled. Root attachments
    /// remain included; no model prompt, Agent resume, activity stop, or deletion is sent.
    /// A single deadline covers semaphore queueing, headers, and the complete download.
    /// Generation cancellation revokes queued/in-flight requests and stale clones.
    /// Non-200 => HttpStatus, wrong type/encoding/container => InvalidFrame, size =>
    /// Oversize, read failure => Network/Timeout. Remote bodies/headers are never exposed.
    pub async fn export_session(&self, session_id: &SessionId) -> Result<SessionArchive> {
        let deadline = Instant::now() + self.inner.limits.timeout;
        tokio::select! {
            biased;
            _ = self.inner.lifetime.cancelled() => Err(Error::Closed),
            result = timeout_at(deadline, self.export_active(session_id, deadline)) => {
                result.unwrap_or(Err(Error::Timeout))
            }
        }
    }

    async fn export_active(
        &self,
        session_id: &SessionId,
        deadline: Instant,
    ) -> Result<SessionArchive> {
        let _permit = self
            .inner
            .rpc_slots
            .acquire()
            .await
            .map_err(|_| Error::Closed)?;
        let auth = self.inner.authority()?;
        let mut url = auth
            .origin
            .join("api/session.export")
            .map_err(|_| Error::InvalidLaunch)?;
        url.query_pairs_mut()
            .append_pair("sessionId", session_id.as_str())
            .append_pair("includeDescendants", "false");
        let mut response = self
            .inner
            .http
            .get(url)
            .header(ORIGIN, auth.origin.origin().ascii_serialization())
            .header(COOKIE, auth.cookie)
            .send()
            .await
            .map_err(network_error)?;
        if response.status().as_u16() != 200 {
            return Err(Error::HttpStatus(response.status().as_u16()));
        }
        // Media type is ASCII case-insensitive; parameters do not change the ZIP type.
        let mut types = response.headers().get_all(CONTENT_TYPE).iter();
        let is_zip = types
            .next()
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.split(';').next())
            .is_some_and(|s| s.trim().eq_ignore_ascii_case("application/zip"));
        if !is_zip
            || types.next().is_some()
            || response
                .headers()
                .get_all(CONTENT_ENCODING)
                .iter()
                .any(|h| {
                    h.to_str()
                        .map_or(true, |s| !s.trim().eq_ignore_ascii_case("identity"))
                })
        {
            return Err(Error::InvalidFrame);
        }
        let limit = self.inner.limits.max_result_bytes.min(MAX_ARCHIVE_BYTES);
        let expected = response.content_length();
        if expected.is_some_and(|n| n > limit as u64) {
            return Err(Error::Oversize);
        }
        // Own even incomplete downloads in the wiping holder, including timeout/drop paths.
        let mut archive = SessionArchive { bytes: Vec::new() };
        while let Some(chunk) = response.chunk().await.map_err(network_error)? {
            if chunk.len() > limit.saturating_sub(archive.len()) {
                return Err(Error::Oversize);
            }
            let next = archive.len() + chunk.len();
            if next > archive.bytes.capacity() {
                let capacity = next
                    .max(archive.bytes.capacity().saturating_mul(2))
                    .min(limit);
                archive
                    .bytes
                    .try_reserve_exact(capacity - archive.len())
                    .map_err(|_| Error::Oversize)?;
            }
            archive.bytes.extend_from_slice(&chunk);
        }
        if expected.is_some_and(|n| n != archive.len() as u64) {
            return Err(Error::InvalidFrame);
        }
        validate_container(&archive.bytes)?;
        if self.inner.lifetime.is_cancelled() {
            return Err(Error::Closed);
        }
        if Instant::now() >= deadline {
            return Err(Error::Timeout);
        }
        Ok(archive)
    }
}

fn network_error(error: reqwest::Error) -> Error {
    if error.is_timeout() {
        Error::Timeout
    } else {
        Error::Network
    }
}

// Narrow structural check of the audited fflate classic ZIP output, without reading
// compressed contents or constructing filenames. Every entry must cover consecutive
// local headers/data/(optional signed descriptor), followed by the complete directory.
fn validate_container(bytes: &[u8]) -> Result<()> {
    let invalid = Error::InvalidFrame;
    if bytes.len() < 22 || !bytes.starts_with(b"PK\x03\x04") {
        return Err(invalid);
    }
    let footer = (bytes.len().saturating_sub(22 + u16::MAX as usize)..=bytes.len() - 22)
        .rev()
        .find(|&p| {
            bytes.get(p..p + 4) == Some(b"PK\x05\x06")
                && p + 22 + word(bytes, p + 20).unwrap_or(0) as usize == bytes.len()
        })
        .ok_or(invalid)?;
    let count = word(bytes, footer + 10)?;
    let directory_size = dword(bytes, footer + 12)? as usize;
    let directory = dword(bytes, footer + 16)? as usize;
    if word(bytes, footer + 4)? != 0
        || word(bytes, footer + 6)? != 0
        || count == 0
        || count == u16::MAX
        || word(bytes, footer + 8)? != count
        || directory.checked_add(directory_size) != Some(footer)
    {
        return Err(invalid);
    }
    let mut central = directory;
    let mut local = 0usize;
    let mut advertised_total = 0u64;
    let mut names = HashSet::new();
    for index in 0..count {
        let header = bytes
            .get(central..central.checked_add(46).ok_or(invalid)?)
            .ok_or(invalid)?;
        if !header.starts_with(b"PK\x01\x02") {
            return Err(invalid);
        }
        let flags = word(header, 8)?;
        let method = word(header, 10)?;
        let crc = dword(header, 16)?;
        let compressed = dword(header, 20)?;
        let uncompressed = dword(header, 24)?;
        let name_len = word(header, 28)? as usize;
        let extra_len = word(header, 30)? as usize;
        let comment_len = word(header, 32)? as usize;
        let version = word(header, 6)?;
        let attributes = dword(header, 38)?;
        let unix_type = (attributes >> 16) & 0o170000;
        if !matches!(version, 10 | 20)
            || flags & !0x080e != 0
            || !matches!(method, 0 | 8)
            || compressed == u32::MAX
            || uncompressed == u32::MAX
            || extra_len != 0
            || word(header, 34)? != 0
            || dword(header, 42)? as usize != local
            || name_len == 0
            || attributes & 0x18 != 0
            || !matches!(unix_type, 0 | 0o100000)
            || (method == 0 && (compressed != uncompressed || flags & 6 != 0))
            || (method == 8 && version != 20)
        {
            return Err(invalid);
        }
        let central_end = central
            .checked_add(46 + name_len + extra_len + comment_len)
            .ok_or(invalid)?;
        if central_end > footer {
            return Err(invalid);
        }
        let name = std::str::from_utf8(
            bytes
                .get(central + 46..central + 46 + name_len)
                .ok_or(invalid)?,
        )
        .map_err(|_| invalid)?;
        if !safe_export_name(name, index == 0)
            || names.contains(name)
            || (!name.is_ascii() && flags & 0x0800 == 0)
        {
            return Err(invalid);
        }
        names.try_reserve(1).map_err(|_| Error::Oversize)?;
        names.insert(name);
        advertised_total = advertised_total
            .checked_add(uncompressed as u64)
            .ok_or(Error::Oversize)?;
        if uncompressed as u64 > MAX_ADVERTISED_ENTRY_BYTES
            || advertised_total > MAX_ADVERTISED_TOTAL_BYTES
        {
            return Err(Error::Oversize);
        }
        let local_header = bytes
            .get(local..local.checked_add(30).ok_or(invalid)?)
            .ok_or(invalid)?;
        if !local_header.starts_with(b"PK\x03\x04")
            || word(local_header, 4)? != version
            || word(local_header, 6)? != flags
            || word(local_header, 8)? != method
            || word(local_header, 26)? as usize != name_len
            || word(local_header, 28)? != 0
        {
            return Err(invalid);
        }
        let data = local
            .checked_add(30 + name_len + word(local_header, 28)? as usize)
            .ok_or(invalid)?;
        let mut end = data.checked_add(compressed as usize).ok_or(invalid)?;
        if end > directory
            || bytes.get(local + 30..local + 30 + name_len)
                != bytes.get(central + 46..central + 46 + name_len)
        {
            return Err(invalid);
        }
        if flags & 8 != 0 {
            if dword(local_header, 14)? != 0
                || dword(local_header, 18)? != 0
                || dword(local_header, 22)? != 0
            {
                return Err(invalid);
            }
            let descriptor = bytes
                .get(end..end.checked_add(16).ok_or(invalid)?)
                .ok_or(invalid)?;
            if !descriptor.starts_with(b"PK\x07\x08")
                || dword(descriptor, 4)? != crc
                || dword(descriptor, 8)? != compressed
                || dword(descriptor, 12)? != uncompressed
            {
                return Err(invalid);
            }
            end += 16;
        } else if dword(local_header, 14)? != crc
            || dword(local_header, 18)? != compressed
            || dword(local_header, 22)? != uncompressed
        {
            return Err(invalid);
        }
        if end > directory {
            return Err(invalid);
        }
        local = end;
        central = central_end;
    }
    if local != directory || central != footer {
        return Err(invalid);
    }
    Ok(())
}
fn safe_export_name(name: &str, root: bool) -> bool {
    if root {
        return name == ROOT_LOG;
    }
    if !(name.starts_with("media/") || name.starts_with("files/"))
        || name.chars().any(|ch| ch == '\\' || ch.is_control())
    {
        return false;
    }
    name.split('/').all(|part| !matches!(part, "" | "." | ".."))
}
fn word(bytes: &[u8], offset: usize) -> Result<u16> {
    let value = bytes.get(offset..offset + 2).ok_or(Error::InvalidFrame)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}
fn dword(bytes: &[u8], offset: usize) -> Result<u32> {
    let value = bytes.get(offset..offset + 4).ok_or(Error::InvalidFrame)?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn debug_redacts_and_consuming_write_preserves_only_writer_copy() {
        let archive = SessionArchive {
            bytes: b"PRIVATE_LOG_BYTES".to_vec(),
        };
        assert_eq!(format!("{archive:?}"), "SessionArchive { len: 17, .. }");
        assert!(!archive.is_empty());
        let mut output = Vec::new();
        assert_eq!(archive.write_to(&mut output).unwrap(), 17);
        assert_eq!(output, b"PRIVATE_LOG_BYTES");
    }
    #[test]
    fn consuming_writer_failure_is_returned_without_archive_recovery() {
        struct Refused;
        impl io::Write for Refused {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::PermissionDenied.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let archive = SessionArchive {
            bytes: vec![1, 2, 3],
        };
        assert_eq!(
            archive.write_to(&mut Refused).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
    }
    #[test]
    fn hostile_short_containers_fail_without_panics() {
        for len in 0..256 {
            let mut bytes = vec![0xff; len];
            if len >= 4 {
                bytes[..4].copy_from_slice(b"PK\x03\x04");
            }
            assert_eq!(validate_container(&bytes), Err(Error::InvalidFrame));
        }
    }
}
