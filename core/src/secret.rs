use crate::{Error, protocol::MAX_LINE};
use std::sync::atomic::{Ordering, compiler_fence};

const MAX_API_KEY_BYTES: usize = 16_384;

// Only this private borrowed wire packet is serializable; the public secret wrapper is not.
#[derive(serde::Serialize)]
struct SaveApiKeyPacket<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(rename = "requestId")]
    request_id: &'a str,
    #[serde(rename = "apiKey")]
    api_key: &'a str,
}

/// Explicit native-worker input, never a widget property or serialized message.
/// Consumes 1..=16,384 printable non-whitespace ASCII bytes, matching the private adapter.
/// No Clone, Display, Serialize, or public plaintext accessor; Debug is always redacted.
/// Owned Rust input and encoded-frame allocations are wiped on Drop. Caller, kernel, and Node
/// copies are outside this guarantee. Invalid input is consumed and wiped too.
///
/// ```compile_fail
/// let key = dsh_native_core::SecretApiKey::new("PUBLIC_FAKE_KEY".into()).unwrap();
/// serde_json::to_string(&key).unwrap();
/// ```
///
/// ```compile_fail
/// let key = dsh_native_core::SecretApiKey::new("PUBLIC_FAKE_KEY".into()).unwrap();
/// format!("{key}");
/// ```
pub struct SecretApiKey {
    bytes: SecretBytes,
}
impl SecretApiKey {
    pub fn new(raw: String) -> Result<Self, Error> {
        let bytes = SecretBytes(raw.into_bytes());
        if bytes.0.is_empty()
            || bytes.0.len() > MAX_API_KEY_BYTES
            || !bytes.0.iter().all(|byte| (0x21..=0x7e).contains(byte))
        {
            return Err(Error::InvalidCommand);
        }
        Ok(Self { bytes })
    }

    pub(crate) fn frame(&self, id: &str) -> Result<SecretBytes, Error> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err(Error::InvalidCommand);
        }
        let packet = SaveApiKeyPacket {
            kind: "save-api-key",
            request_id: id,
            api_key: std::str::from_utf8(&self.bytes.0).map_err(|_| Error::InvalidCommand)?,
        };
        // Allocate the whole bounded frame once. The writer rejects overflow before extending,
        // so serialization cannot reallocate/free a partially populated secret allocation.
        let mut frame = SecretBytes(Vec::with_capacity(MAX_LINE));
        serde_json::to_writer(&mut frame, &packet).map_err(|_| Error::CommandTooLarge)?;
        std::io::Write::write_all(&mut frame, b"\n").map_err(|_| Error::CommandTooLarge)?;
        Ok(frame)
    }
}
impl std::fmt::Debug for SecretApiKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretApiKey([REDACTED])")
    }
}

/// One-use pasted Codex response. Never cloned, displayed, serialized or publicly readable.
/// Consumes 1..=16,384 printable non-whitespace ASCII bytes without trimming.
/// Owned input and private encoded frames are wiped; GUI, Node and kernel copies are not covered.
/// ```compile_fail
/// let value = dsh_native_core::SecretCodexCallback::new("PUBLIC_CODE".into()).unwrap();
/// serde_json::to_string(&value).unwrap();
/// ```
/// ```compile_fail
/// let value = dsh_native_core::SecretCodexCallback::new("PUBLIC_CODE".into()).unwrap();
/// format!("{value}");
/// ```
pub struct SecretCodexCallback(SecretBytes);
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexCallbackPacket<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    request_id: &'a str,
    attempt: u64,
    prompt: u64,
    response: &'a str,
}
impl SecretCodexCallback {
    pub fn new(raw: String) -> Result<Self, Error> {
        let bytes = SecretBytes(raw.into_bytes());
        if bytes.0.is_empty()
            || bytes.0.len() > MAX_API_KEY_BYTES
            || !bytes.0.iter().all(|byte| (0x21..=0x7e).contains(byte))
        {
            return Err(Error::InvalidCommand);
        }
        Ok(Self(bytes))
    }
    pub(crate) fn frame(&self, id: &str, attempt: u64, prompt: u64) -> Result<SecretBytes, Error> {
        if !crate::codex::valid_id(attempt)
            || !crate::codex::valid_id(prompt)
            || id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(Error::InvalidCommand);
        }
        let packet = CodexCallbackPacket {
            kind: "codex-callback",
            request_id: id,
            attempt,
            prompt,
            response: std::str::from_utf8(&self.0.0).map_err(|_| Error::InvalidCommand)?,
        };
        let mut frame = SecretBytes(Vec::with_capacity(MAX_LINE));
        serde_json::to_writer(&mut frame, &packet).map_err(|_| Error::CommandTooLarge)?;
        std::io::Write::write_all(&mut frame, b"\n").map_err(|_| Error::CommandTooLarge)?;
        Ok(frame)
    }
}
impl std::fmt::Debug for SecretCodexCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretCodexCallback([REDACTED])")
    }
}

// No zeroize crate is available in the required offline shared cache. Keep the fallback local,
// non-serializable and non-Debug; volatile stores plus a compiler fence prevent dead-store removal.
pub(crate) struct SecretBytes(Vec<u8>);
impl SecretBytes {
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.0
    }
    fn wipe(&mut self) {
        for offset in 0..self.0.capacity() {
            // Vec owns every byte in its allocation, including spare capacity. Write only;
            // never read uninitialized spare bytes. The allocation remains live until Drop ends.
            unsafe {
                self.0.as_mut_ptr().add(offset).write_volatile(0);
            }
        }
        compiler_fence(Ordering::SeqCst);
    }
}
impl std::io::Write for SecretBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_LINE.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other("Secret frame exceeds bound"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.wipe();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_writer_rejects_overflow_before_reallocation() {
        let mut buffer = SecretBytes(Vec::with_capacity(MAX_LINE));
        std::io::Write::write_all(&mut buffer, &vec![b'A'; MAX_LINE]).unwrap();
        let pointer = buffer.0.as_ptr();
        assert!(std::io::Write::write_all(&mut buffer, b"PUBLIC_FAKE_OVERFLOW").is_err());
        assert_eq!(buffer.0.len(), MAX_LINE);
        assert_eq!(buffer.0.as_ptr(), pointer);
        assert_eq!(buffer.0.capacity(), MAX_LINE);
    }
    #[test]
    fn wipe_clears_initialized_and_spare_allocation_bytes() {
        let mut buffer = SecretBytes(Vec::with_capacity(128));
        buffer.0.extend_from_slice(b"PUBLIC_FAKE_WIPE");
        buffer.wipe();
        // wipe initialized every allocated byte, so observing the spare capacity is safe now.
        let bytes = unsafe { std::slice::from_raw_parts(buffer.0.as_ptr(), buffer.0.capacity()) };
        assert!(bytes.iter().all(|byte| *byte == 0));
    }
}
