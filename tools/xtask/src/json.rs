//! The one piece of JSON the reports compare across the Python and Rust tools.
//!
//! `verify-native.py` hashed `json.dumps(files, sort_keys=True)`, and evidence
//! reports quote that aggregate digest. Reproducing `json.dumps` exactly (default
//! separators, `ensure_ascii=True`, sorted keys) keeps a digest taken before the
//! port comparable with one taken after it, so no report needs a footnote.
use std::collections::BTreeMap;

/// Serialize a string map the way `json.dumps(mapping, sort_keys=True)` does.
pub fn dump_sorted_object(map: &BTreeMap<String, String>) -> String {
    let mut out = String::from("{");
    for (index, (key, value)) in map.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        write_string(&mut out, key);
        out.push_str(": ");
        write_string(&mut out, value);
    }
    out.push('}');
    out
}

/// One JSON string body with Python's `ensure_ascii=True` escaping: every
/// non-ASCII scalar becomes UTF-16 `\uXXXX` escapes (a surrogate pair above the
/// basic plane), and only the two required characters plus the named controls
/// are shortened.
fn write_string(out: &mut String, value: &str) {
    out.push('"');
    for scalar in value.chars() {
        match scalar {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            scalar if (scalar as u32) < 0x20 || !scalar.is_ascii() => {
                let mut units = [0u16; 2];
                for unit in scalar.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
            scalar => out.push(scalar),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn a_sorted_object_is_serialized_the_way_json_dumps_writes_it() {
        // Expected bytes produced by python3 -c 'import json;print(json.dumps({...},sort_keys=True))'
        let rendered = dump_sorted_object(&map(&[("b", "x"), ("a", "é\n"), ("c", "\u{1f600}")]));
        assert_eq!(
            rendered,
            "{\"a\": \"\\u00e9\\n\", \"b\": \"x\", \"c\": \"\\ud83d\\ude00\"}"
        );
    }

    #[test]
    fn an_empty_object_is_two_braces() {
        assert_eq!(dump_sorted_object(&map(&[])), "{}");
    }

    #[test]
    fn the_required_escapes_are_quotes_and_backslashes() {
        assert_eq!(
            dump_sorted_object(&map(&[("k", "a\"b\\c")])),
            "{\"k\": \"a\\\"b\\\\c\"}"
        );
    }

    #[test]
    fn a_control_character_below_the_printable_range_becomes_an_escape() {
        assert_eq!(
            dump_sorted_object(&map(&[("k", "\u{1}\u{7}\u{1f}")])),
            "{\"k\": \"\\u0001\\u0007\\u001f\"}"
        );
    }
}
