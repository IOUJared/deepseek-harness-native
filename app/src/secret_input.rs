//! One-use secret drafts shared by the key, plugin and Codex editors.
//!
//! [`Editor`] owns the only plaintext copy the app keeps, overwrites its complete
//! allocation on drop and never prints it. [`Input`] is the widget-facing handle:
//! cloning duplicates the one-use slot, not the plaintext, and [`Input::take`]
//! drains it exactly once, so a stale or replayed action loses its secret
//! immediately. Validation stays with the caller that knows its own rule.
//!
//! Wiping covers owned Rust allocations only; widget, Node, kernel, clipboard and
//! swap copies are outside this guarantee.

use std::sync::{Arc, Mutex};
use zeroize::Zeroize;

/// A redacted `Debug` for every type that can transitively hold a secret.
macro_rules! redacted_debug {
    ($label:literal; $($ty:ty),*) => { $(impl std::fmt::Debug for $ty {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(concat!($label, "([REDACTED])"))
        }
    })* };
}
pub(crate) use redacted_debug;

/// The local plaintext buffer. Never `Clone`, `Display` or serializable.
#[derive(Default)]
pub struct Editor(String);
impl Editor {
    pub fn new(raw: String) -> Self {
        Self(raw)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Move the plaintext to the one consumer that must own it — the secret type
    /// that serializes the frame — leaving an empty buffer behind.
    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.0)
    }
    /// Zero the complete allocation, spare capacity included: `zeroize` wipes the
    /// uninitialized tail before the initialized bytes, so no byte of the draft
    /// survives the drop. `Drop` calls this; the fixture reads it before the drop
    /// to observe the guarantee while the allocation is still live.
    fn wipe(&mut self) {
        self.0.zeroize();
    }
}
impl Drop for Editor {
    fn drop(&mut self) {
        self.wipe();
    }
}
impl std::fmt::Debug for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretEditor([REDACTED])")
    }
}

/// One validated draft value, or the explicit refusal of an invalid one.
pub enum Value {
    Valid(Editor),
    Rejected,
}

/// A one-use holder for a validated draft. Cloning shares the single slot.
#[derive(Clone)]
pub struct Input(Arc<Mutex<Option<Value>>>);
impl Input {
    /// At most `max` bytes of printable non-whitespace ASCII, the same rule the
    /// Core frame enforces for an API key or callback. Anything else is refused
    /// here, with its plaintext wiped, before it can be reviewed.
    pub fn ascii(raw: String, max: usize) -> Self {
        if raw.len() > max || !raw.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
            drop(Editor::new(raw));
            return Self::rejected();
        }
        Self::valid(raw)
    }
    /// At most `max` bytes with no character restriction: an editor value that is
    /// not itself a credential.
    pub fn bounded(raw: String, max: usize) -> Self {
        if raw.len() > max {
            drop(Editor::new(raw));
            return Self::rejected();
        }
        Self::valid(raw)
    }
    fn valid(raw: String) -> Self {
        Self(Arc::new(Mutex::new(Some(Value::Valid(Editor::new(raw))))))
    }
    fn rejected() -> Self {
        Self(Arc::new(Mutex::new(Some(Value::Rejected))))
    }
    pub fn take(&self) -> Option<Value> {
        self.0.lock().ok()?.take()
    }
}
impl std::fmt::Debug for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretInput([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_editor_wipes_its_full_allocation_and_debug_is_redacted() {
        let mut raw = String::with_capacity(128);
        raw.push_str("PUBLIC_WIPE_FIXTURE");
        let mut editor = Editor::new(raw);
        assert_eq!(format!("{editor:?}"), "SecretEditor([REDACTED])");
        editor.wipe();
        // wipe initialized every owned byte, including spare capacity, before this observation.
        let bytes = unsafe { std::slice::from_raw_parts(editor.0.as_ptr(), editor.0.capacity()) };
        assert!(bytes.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn input_is_one_use_and_refuses_out_of_rule_drafts() {
        let ascii = Input::ascii("PUBLIC_FAKE_KEY".into(), 32);
        assert_eq!(format!("{ascii:?}"), "SecretInput([REDACTED])");
        let cloned = ascii.clone();
        assert!(matches!(ascii.take(), Some(Value::Valid(_))));
        // The clone shares the single slot, so a replayed holder has nothing left.
        assert!(cloned.take().is_none());
        assert!(matches!(
            Input::ascii("two\nlines".into(), 32).take(),
            Some(Value::Rejected)
        ));
        assert!(matches!(
            Input::ascii("x".repeat(33), 32).take(),
            Some(Value::Rejected)
        ));
        // `bounded` accepts any text the Host may store, up to its bound only.
        assert!(matches!(
            Input::bounded("two\nlines".into(), 32).take(),
            Some(Value::Valid(_))
        ));
        assert!(matches!(
            Input::bounded("x".repeat(33), 32).take(),
            Some(Value::Rejected)
        ));
    }
}
