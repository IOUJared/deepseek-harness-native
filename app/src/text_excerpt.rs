//! Four-line previews retain Unicode/ellipsis semantics without joining full source lines.
pub(super) fn excerpt(value: &str) -> String {
    let mut lines = value.lines();
    let mut result = String::new();
    let mut remaining = 240;
    for (index, line) in lines.by_ref().take(4).enumerate() {
        if index != 0 {
            if remaining == 0 {
                result.push('…');
                return result;
            }
            result.push('\n');
            remaining -= 1;
        }
        let mut chars = line.char_indices();
        let mut end = 0;
        let mut count = 0;
        for (offset, character) in chars.by_ref().take(remaining) {
            end = offset + character.len_utf8();
            count += 1;
        }
        // Keep marker headroom without geometrically doubling retained Unicode previews.
        result.reserve_exact(end + '…'.len_utf8());
        result.push_str(&line[..end]);
        remaining -= count;
        if chars.next().is_some() {
            result.push('…');
            return result;
        }
    }
    if lines.next().is_some() && !result.ends_with('…') {
        result.push('…');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::excerpt;

    // Retained original implementation is the exact-output oracle for this optimization.
    fn original(value: &str) -> String {
        let mut lines = value.lines();
        let first = lines.by_ref().take(4).collect::<Vec<_>>().join("\n");
        let mut chars = first.chars();
        let mut result: String = chars.by_ref().take(240).collect();
        if chars.next().is_some() {
            result = format!("{result}…");
        }
        if lines.next().is_some() && !result.ends_with('…') {
            result.push('…');
        }
        result
    }

    #[test]
    fn generated_previews_match_original_unicode_and_line_limits() {
        for token in ["a", "🌹", "é", "e\u{301}", "…", "\r", ""] {
            for count in [0, 1, 2, 59, 60, 239, 240, 241, 1024, 32768] {
                for separator in ["\n", "\r\n"] {
                    for lines in 0..=6 {
                        let value = vec![token.repeat(count); lines].join(separator);
                        for ending in ["", separator, "…"] {
                            let input = format!("{value}{ending}");
                            let preview = excerpt(&input);
                            assert!(preview.capacity() <= preview.len() + '…'.len_utf8());
                            assert_eq!(
                                preview,
                                original(&input),
                                "token={token:?}, count={count}, lines={lines}, ending={ending:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn newline_cutoff_and_natural_ellipsis_match_original() {
        for input in [
            format!("{}\nb\nc\nd\ne", "a".repeat(239)),
            format!("{}…x", "a".repeat(239)),
            format!("a\nb\nc\n…\n{}", "x".repeat(32768)),
            "\n\n\n\n\n".into(),
            "a\r\nb\rc\n".into(),
        ] {
            assert_eq!(excerpt(&input), original(&input));
        }
    }
}
