//! Literal search of caller-supplied visible chat text, without fetching or retaining records.
use std::ops::Range;

pub const MAX_SOURCE_BYTES: usize = 32 * 1024;
pub const MAX_QUERY_BYTES: usize = 256;
pub const MAX_MATCHES: usize = 1024;

/// Find up to 1024 nonoverlapping matches as byte ranges into the unchanged source.
///
/// Matching uses Unicode scalar lowercase, not regex, normalization or full case folding.
/// A match must consume whole original characters: `İ` matches `i\u{307}`, not just `i`.
/// Empty queries, queries over 256 UTF-8 bytes and sources over 32 KiB return no matches.
/// The caller owns visible-text selection, highlighting, scrolling and session generation fences.
pub fn ranges(source: &str, query: &str) -> Vec<Range<usize>> {
    prepare(source, query)
        .map(|matches| matches.take(MAX_MATCHES).collect())
        .unwrap_or_default()
}

/// Test for a match under the same literal, Unicode and input limits as [`ranges`].
/// No original-byte mapping is allocated when the lowercase literal has no candidate.
pub fn contains(source: &str, query: &str) -> bool {
    prepare(source, query).is_some_and(|mut matches| matches.next().is_some())
}

fn lowercase(text: &str) -> String {
    text.chars().flat_map(char::to_lowercase).collect()
}

// No Debug implementation: search text must not enter diagnostics through formatting.
struct Matches {
    folded: String,
    needle: String,
    original: Vec<usize>,
    next: usize,
}

fn prepare(source: &str, query: &str) -> Option<Matches> {
    if source.len() > MAX_SOURCE_BYTES || query.is_empty() || query.len() > MAX_QUERY_BYTES {
        return None;
    }
    let needle = lowercase(query);
    let folded = lowercase(source);
    let next = folded.find(&needle)?;
    // Only scalar boundaries map back to the source; expansion interiors stay invalid.
    let mut original = vec![usize::MAX; folded.len() + 1];
    let mut offset = 0;
    for (index, scalar) in source.char_indices() {
        original[offset] = index;
        offset += scalar.to_lowercase().map(char::len_utf8).sum::<usize>();
    }
    original[offset] = source.len();
    Some(Matches {
        folded,
        needle,
        original,
        next,
    })
}

impl Iterator for Matches {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let start = self.next + self.folded[self.next..].find(&self.needle)?;
            let end = start + self.needle.len();
            let first = self.original[start];
            let last = self.original[end];
            if first != usize::MAX && last != usize::MAX {
                self.next = end;
                return Some(first..last);
            }
            // A rejected partial expansion must not skip an overlapping valid candidate.
            self.next = start + self.folded[start..].chars().next()?.len_utf8();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slices<'a>(source: &'a str, query: &str) -> Vec<&'a str> {
        ranges(source, query)
            .into_iter()
            .map(|range| &source[range])
            .collect()
    }

    #[test]
    fn ascii_is_case_insensitive_literal_and_nonoverlapping() {
        assert_eq!(ranges("Hello hELLo", "hello"), [0..5, 6..11]);
        assert!(contains("Hello hELLo", "HELLO"));
        assert_eq!(ranges("ababa", "aba"), [0..3]);
        assert_eq!(ranges("aaaaa", "aa"), [0..2, 2..4]);
        assert!(!contains("PUBLIC supplied text", "absent"));
    }

    #[test]
    fn unicode_ranges_slice_original_bytes_not_lowercase_bytes() {
        let source = "É世界 é世界";
        assert_eq!(ranges(source, "é世界"), [0..8, 9..17]);
        assert_eq!(slices(source, "é世界"), ["É世界", "é世界"]);
        assert!(contains(source, "É世界"));
        assert_eq!(ranges("界世界", "世界"), [3..9]);
        // No normalization or full Unicode case folding is implied.
        assert!(!contains("é", "e\u{301}"));
        assert!(!contains("Straße", "STRASSE"));
    }

    #[test]
    fn lowercase_expansions_match_only_at_whole_original_scalar_boundaries() {
        let source = "İ i\u{307} I";
        assert_eq!(ranges(source, "İ"), [0..2, 3..6]);
        assert_eq!(ranges(source, "i\u{307}"), [0..2, 3..6]);
        assert_eq!(slices(source, "İ"), ["İ", "i\u{307}"]);
        assert_eq!(ranges(source, "i"), [3..4, 7..8]);
        assert_eq!(ranges(source, "\u{307}"), [4..6]);
        assert!(!contains("İ", "i"));
        assert!(!contains("İ", "\u{307}"));
        assert!(contains("İ", "i\u{307}"));
        assert!(contains("i\u{307}", "İ"));
    }

    #[test]
    fn regex_metacharacters_are_only_literal_characters() {
        let source = "PUBLIC [a-z]+ .* (hello)? \\d{2} ^$";
        for query in ["[a-z]+", ".*", "(hello)?", "\\d{2}", "^$"] {
            assert_eq!(slices(source, query), [query]);
            assert!(contains(source, query));
        }
        assert!(!contains("anything", ".*"));
        assert!(!contains("12345", "\\d+"));
        assert_eq!(ranges("literal [", "["), [8..9]);
    }

    #[test]
    fn empty_and_over_budget_inputs_fail_closed_without_truncating_a_query() {
        for source in ["", "PUBLIC text"] {
            assert!(ranges(source, "").is_empty());
            assert!(!contains(source, ""));
        }
        let source = "x".repeat(MAX_SOURCE_BYTES + 1);
        assert!(ranges(&source, "x").is_empty());
        assert!(!contains(&source, "x"));
        let query = "x".repeat(MAX_QUERY_BYTES + 1);
        assert!(ranges("x", &query).is_empty());
        assert!(!contains("x", &query));
        let multibyte_query = "é".repeat(MAX_QUERY_BYTES / 2 + 1);
        assert!(ranges(&multibyte_query, &multibyte_query).is_empty());
        assert!(!contains(&multibyte_query, &multibyte_query));
        let multibyte_source = "界".repeat(MAX_SOURCE_BYTES / 3 + 1);
        assert!(ranges(&multibyte_source, "界").is_empty());
        assert!(!contains(&multibyte_source, "界"));
    }

    #[test]
    fn exact_budgets_and_match_capacity_remain_usable() {
        let source = "x".repeat(MAX_SOURCE_BYTES);
        let query = "x".repeat(MAX_QUERY_BYTES);
        let found = ranges(&source, &query);
        assert_eq!(found.len(), MAX_SOURCE_BYTES / MAX_QUERY_BYTES);
        assert_eq!(found.first(), Some(&(0..MAX_QUERY_BYTES)));
        assert!(contains(&source, &query));
        let found = ranges(&source, "x");
        assert_eq!(found.len(), MAX_MATCHES);
        assert_eq!(found.last(), Some(&(MAX_MATCHES - 1..MAX_MATCHES)));
        let unicode = "İ".repeat(MAX_SOURCE_BYTES / 2);
        let found = ranges(&unicode, "i\u{307}");
        assert_eq!(found.len(), MAX_MATCHES);
        assert_eq!(
            found.last(),
            Some(&(2 * (MAX_MATCHES - 1)..2 * MAX_MATCHES))
        );
        assert!(!contains(&unicode, "i"));
    }

    #[test]
    fn ranges_reconstruct_the_retained_text_exactly_and_never_mutate_it() {
        let source = String::from("PUBLIC É世界\nİ thought [.*] é世界\nPUBLIC answer");
        let retained = source.clone();
        for query in ["é世界", "i\u{307}", "[.*]", "PUBLIC", "absent", "\n"] {
            let found = ranges(&source, query);
            assert_eq!(contains(&source, query), !found.is_empty());
            let mut reconstructed = String::new();
            let mut before = 0;
            for range in found {
                assert!(range.start >= before);
                assert!(range.start < range.end);
                assert!(source.is_char_boundary(range.start));
                assert!(source.is_char_boundary(range.end));
                reconstructed.push_str(&source[before..range.start]);
                reconstructed.push_str(&source[range.clone()]);
                before = range.end;
            }
            reconstructed.push_str(&source[before..]);
            assert_eq!(reconstructed, retained);
            assert_eq!(source, retained);
        }
    }
}
