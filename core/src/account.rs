//! DeepSeek account sign-in, as the Host bridge defines it.
//!
//! The Host owns the whole OAuth exchange, the credential store and the browser
//! destination: this module only names the three commands the bridge accepts,
//! validates exactly what it sends, and projects what comes back. Nothing here
//! reads, holds or forwards a token, and the account view a caller sees carries
//! no credential field at all.
//!
//! One attempt is described by an opaque Host-issued id - echoed back only to
//! cancel that attempt - and, while the Host waits for the browser callback, the
//! destination the user must open. That destination is Host configuration, so
//! this side checks its shape and the app checks it again before launching it.

/// The attempt id is opaque: bounded and free of control bytes and spaces, like
/// every other id the bridge carries, because it travels back in a command.
pub(crate) fn valid_attempt_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.bytes().any(|byte| byte <= 0x20 || byte == 0x7f)
}

/// A BCP 47-shaped tag, deliberately narrower than the whole grammar: the Host
/// forwards it to Platform as the UI language, so anything ambiguous is refused
/// rather than sent. This mirrors the bridge's own validation.
pub(crate) fn valid_locale(value: &str) -> bool {
    if value.is_empty() || value.len() > 64 {
        return false;
    }
    let mut segments = value.split(['-', '_']);
    let Some(primary) = segments.next() else {
        return false;
    };
    if !(2..=8).contains(&primary.len()) || !primary.bytes().all(|b| b.is_ascii_alphabetic()) {
        return false;
    }
    segments.all(|segment| {
        (1..=8).contains(&segment.len()) && segment.bytes().all(|b| b.is_ascii_alphanumeric())
    })
}

/// The offset the Host reports to Platform as part of the caller's identity.
pub(crate) fn valid_timezone_offset(seconds: i32) -> bool {
    // A range test, not `abs()`: the smallest i32 has no positive counterpart
    // and negating it panics.
    (-86_400..=86_400).contains(&seconds)
}

/// The UI identity the Host expects alongside an account command: this session's
/// locale and UTC offset, each already narrowed to what the bridge accepts, so a
/// caller cannot send a value the Host would refuse.
pub fn account_identity() -> (String, i32) {
    (ui_locale(), utc_offset_seconds())
}

/// This session's UTC offset in seconds, read from the C library rather than a
/// time crate: the value is only ever handed to the Host, and one number is not
/// worth a calendar dependency. A failure reports UTC, which is never worse than
/// refusing to sign in.
pub(crate) fn utc_offset_seconds() -> i32 {
    // SAFETY: `time` and `localtime_r` write only into this stack value, and the
    // pointer stays valid for the call.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut broken_down: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut broken_down).is_null() {
            return 0;
        }
        let offset = broken_down.tm_gmtoff;
        if valid_timezone_offset(offset as i32) {
            offset as i32
        } else {
            0
        }
    }
}

/// The UI language to report. The native UI is English only today, so a usable
/// environment tag is forwarded and anything else, including no locale at all,
/// reports `en` - the Host reduces the tag to `zh_CN` or `en_US` for Platform.
pub(crate) fn ui_locale() -> String {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(raw) = std::env::var_os(key) else {
            continue;
        };
        let Some(text) = raw.to_str() else {
            continue;
        };
        // `en_US.UTF-8@euro` and `C` are not the tag Platform wants.
        let tag = text.split(['.', '@']).next().unwrap_or_default();
        if valid_locale(tag) {
            return tag.to_owned();
        }
    }
    "en".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attempt_ids_are_bounded_and_carry_no_control_bytes() {
        assert!(valid_attempt_id("plain-id"));
        assert!(valid_attempt_id(&"a".repeat(512)));
        for invalid in ["", " ", "has space", "line\nbreak", &"a".repeat(513)] {
            assert!(!valid_attempt_id(invalid), "{invalid:?} must be refused");
        }
    }

    #[test]
    fn locales_are_narrow_bcp47_tags() {
        for valid in ["en", "en-US", "en_US", "zh-Hans-CN", "pt-BR"] {
            assert!(valid_locale(valid), "{valid} must be accepted");
        }
        for invalid in [
            "",
            "e",
            "englishlanguage",
            "en-",
            "-en",
            "en US",
            "en-US.UTF-8",
            "en/../../etc",
            &"a".repeat(65),
        ] {
            assert!(!valid_locale(invalid), "{invalid:?} must be refused");
        }
    }

    #[test]
    fn offsets_stay_inside_a_day() {
        for valid in [0, -3_600, 19_800, -43_200, 86_400] {
            assert!(valid_timezone_offset(valid));
        }
        for invalid in [86_401, -86_401, i32::MAX, i32::MIN] {
            assert!(!valid_timezone_offset(invalid));
        }
    }

    #[test]
    fn the_reported_identity_is_usable_even_without_a_session_locale() {
        let offset = utc_offset_seconds();
        assert!(
            valid_timezone_offset(offset),
            "the reported offset must always be acceptable, got {offset}"
        );
        let locale = ui_locale();
        assert!(
            valid_locale(&locale),
            "the reported locale must always be acceptable, got {locale:?}"
        );
    }
}
