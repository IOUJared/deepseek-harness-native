//! Desktop notifications for work the user is not watching.
//!
//! The rule is narrow on purpose. A notification is only for a settle the user
//! would otherwise have to poll for, and only while this window is not focused:
//! announcing a result to someone already reading it is noise, and announcing a
//! still-running turn is worse than silence.
//!
//! Delivery is one bounded `notify-send` invocation with every field passed as a
//! separate argument, so no session text can become a shell word. That child is
//! this application's first, so it is spawned only when a notification is actually
//! due, is never waited on, and cannot outlive the call that started it.

use std::process::{Command, Stdio};

/// A settle worth interrupting for. Anything not listed here stays silent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The turn finished and an answer is waiting.
    Answered,
    /// The session update stream broke, so the conversation may have stopped
    /// receiving records while the window was not focused.
    Interrupted,
}

/// One due notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub kind: Kind,
    pub summary: String,
    pub body: String,
}

/// Whether this settle should be announced.
///
/// `focused` is the window's own focus state: while the user is looking at the
/// conversation every one of these is already on screen.
pub fn due(kind: Kind, focused: bool) -> bool {
    !focused && matches!(kind, Kind::Answered | Kind::Interrupted)
}

/// Bounded one-line text: a notification body is not a place for a transcript.
/// Control characters are dropped rather than collapsed, because `is_whitespace`
/// does not cover NUL, backspace or the other C0 codes a notification daemon
/// would render or truncate unpredictably.
fn bounded(text: &str, max: usize) -> String {
    let mut collapsed = String::new();
    let mut gap = false;
    for character in text.chars() {
        if character.is_control() || character.is_whitespace() {
            gap = !collapsed.is_empty();
            continue;
        }
        if gap {
            collapsed.push(' ');
            gap = false;
        }
        collapsed.push(character);
    }
    if collapsed.chars().count() <= max {
        return collapsed;
    }
    let mut out: String = collapsed.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// The notice for one settle, or `None` when it must stay silent.
pub fn notice(kind: Kind, focused: bool, title: &str, detail: &str) -> Option<Notice> {
    if !due(kind, focused) {
        return None;
    }
    let summary = match kind {
        Kind::Answered => "DeepSeek Harness · answer ready",
        Kind::Interrupted => "DeepSeek Harness · session interrupted",
    };
    let title = bounded(title, 60);
    let detail = bounded(detail, 160);
    let body = match (title.is_empty(), detail.is_empty()) {
        (true, _) => detail,
        (false, true) => title,
        (false, false) => format!("{title} · {detail}"),
    };
    Some(Notice {
        kind,
        summary: summary.to_owned(),
        body,
    })
}

/// Deliver one notice through the desktop. Never waits, never reads back, and
/// never lets a failure change what the app already showed.
pub fn deliver(notice: &Notice) {
    // One fixed program with fixed flags; the variable text is argument data.
    let _ = Command::new("notify-send")
        .arg("--app-name=DeepSeek Harness")
        .arg("--icon=ai.deepseek.harness.native.app")
        .arg("--")
        .arg(&notice.summary)
        .arg(&notice.body)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_unfocused_settle_is_announced() {
        // While the window has focus every one of these is already on screen.
        for kind in [Kind::Answered, Kind::Interrupted] {
            assert!(due(kind, false), "{kind:?} unfocused");
            assert!(!due(kind, true), "{kind:?} focused");
        }
    }

    #[test]
    fn an_announced_notice_names_the_outcome() {
        let answered = notice(Kind::Answered, false, "Refine the composer", "Done.")
            .expect("an unfocused answer is announced");
        assert_eq!(answered.kind, Kind::Answered);
        assert!(answered.summary.contains("answer ready"));
        assert!(answered.body.contains("Refine the composer"));
        assert!(answered.body.contains("Done."));

        let interrupted =
            notice(Kind::Interrupted, false, "", "the stream broke").expect("announced");
        assert!(interrupted.summary.contains("session interrupted"));
        assert_eq!(interrupted.body, "the stream broke");

        // A focused window gets nothing at all, not an empty notice.
        assert!(notice(Kind::Answered, true, "t", "d").is_none());
    }

    #[test]
    fn notification_text_is_bounded_and_single_line() {
        let long = "word ".repeat(200);
        let announced = notice(Kind::Answered, false, &long, &long).expect("announced");
        assert!(
            announced.body.chars().count() <= 60 + 3 + 160,
            "body length {}",
            announced.body.chars().count()
        );
        assert!(!announced.body.contains('\n'), "no newlines in a body");
        assert!(!announced.body.contains("  "), "whitespace collapsed");
        // Control characters cannot reach a desktop notification.
        let hostile =
            notice(Kind::Interrupted, false, "", "a\u{0}b\u{7}c\nnext").expect("announced");
        assert!(!hostile.body.contains('\u{0}'), "{:?}", hostile.body);
        assert!(!hostile.body.contains('\u{7}'), "{:?}", hostile.body);
        assert!(!hostile.body.contains('\n'));
        // A dropped control character must not become a word break: the letters
        // around it stay one word.
        assert_eq!(hostile.body, "a b c next");
    }
}
