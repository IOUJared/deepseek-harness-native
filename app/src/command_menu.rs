//! Slash-command completion for the composer.
//!
//! The composer sends whatever the user typed, so completion is an editing aid
//! and never an execution path: accepting a candidate rewrites the draft and
//! stops there. Only the leading token completes — the first line of a command
//! line, before any space — which matches how the Host reads a command, and
//! keeps a `/` inside prose from opening a menu.
//!
//! The candidate list is the Host's own advertisement. Nothing is synthesised
//! locally, so a command the Host does not offer can never be completed, and the
//! menu is empty rather than wrong while the advertisement is still on its way.

/// One completable command, as the Host advertised it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub name: String,
    pub description: String,
}

/// [`MAX_MATCHES`]: bounded like every other list this app renders.
pub const MAX_MATCHES: usize = 12;

/// What the composer should show and act on: the ranked rows for the current
/// draft, the highlighted one, and the token those rows are completing.
#[derive(Debug, PartialEq, Eq)]
pub struct Completion<'a> {
    pub matches: Vec<&'a Candidate>,
    pub selected: usize,
    pub query: String,
}

impl Completion<'_> {
    /// The highlighted candidate.
    pub fn candidate(&self) -> Option<&Candidate> {
        self.matches.get(self.selected).copied()
    }
}

/// The token being completed, when the draft is a leading command line.
///
/// Returns the text after the slash, which may be empty (the user just typed
/// `/`). A slash anywhere else, a second slash, or any whitespace before the end
/// ends completion: the user has moved on to arguments or prose.
pub fn query(draft: &str) -> Option<&str> {
    let rest = draft.strip_prefix('/')?;
    if rest.contains([' ', '\t', '\n']) {
        return None;
    }
    Some(rest)
}

/// Candidates matching the query, ranked: prefix matches first, then substring
/// matches, each in the Host's advertised order. An empty query offers all.
pub fn matches<'a>(candidates: &'a [Candidate], query: &str) -> Vec<&'a Candidate> {
    let needle = query.to_ascii_lowercase();
    if needle.is_empty() {
        return candidates.iter().take(MAX_MATCHES).collect();
    }
    let mut prefix = Vec::new();
    let mut contains = Vec::new();
    for candidate in candidates {
        let name = candidate.name.to_ascii_lowercase();
        if name.starts_with(&needle) {
            prefix.push(candidate);
        } else if name.contains(&needle) {
            contains.push(candidate);
        }
        if prefix.len() >= MAX_MATCHES {
            break;
        }
    }
    prefix.extend(contains);
    prefix.truncate(MAX_MATCHES);
    prefix
}

/// The selected row after moving `delta` places, wrapping at both ends so a
/// held key cannot run off the list.
pub fn advance(selected: usize, len: usize, delta: i32) -> usize {
    if len == 0 {
        return 0;
    }
    let len = len as i32;
    let current = (selected.min(len as usize - 1)) as i32;
    (current + delta).rem_euclid(len) as usize
}

/// The draft after accepting a candidate: the command token, with the caller's
/// trailing text preserved and normalised to a single separating space so an
/// accepted command never absorbs the next word.
pub fn accepted(draft: &str, name: &str) -> String {
    let rest = draft
        .split_once(' ')
        .map_or("", |(_, rest)| rest.trim_start());
    if rest.is_empty() {
        format!("/{name} ")
    } else {
        format!("/{name} {rest}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates(names: &[(&str, &str)]) -> Vec<Candidate> {
        names
            .iter()
            .map(|(name, description)| Candidate {
                name: (*name).to_owned(),
                description: (*description).to_owned(),
            })
            .collect()
    }
    fn names(list: &[&Candidate]) -> Vec<String> {
        list.iter().map(|c| c.name.clone()).collect()
    }

    #[test]
    fn only_a_leading_token_completes() {
        assert_eq!(query("/"), Some(""));
        assert_eq!(query("/pl"), Some("pl"));
        assert_eq!(query("/plan"), Some("plan"));
        // Arguments and prose end completion.
        assert_eq!(query("/plan "), None);
        assert_eq!(query("/plan off"), None);
        assert_eq!(query("tell me /about it"), None);
        assert_eq!(query(""), None);
        assert_eq!(query("plan"), None);
        // Only the first line is a command position.
        assert_eq!(query("/plan\nmore"), None);
    }

    #[test]
    fn an_empty_query_offers_the_advertisement_in_order() {
        let list = candidates(&[("plan", "a"), ("compact", "b"), ("help", "c")]);
        assert_eq!(names(&matches(&list, "")), ["plan", "compact", "help"]);
    }

    #[test]
    fn matching_is_case_insensitive_and_ranks_prefixes_first() {
        let list = candidates(&[("compact", "a"), ("replan", "b"), ("plan", "c")]);
        // `plan` matches by prefix, `replan` only by substring, so order flips.
        assert_eq!(names(&matches(&list, "plan")), ["plan", "replan"]);
        assert_eq!(names(&matches(&list, "PLAN")), ["plan", "replan"]);
        assert_eq!(names(&matches(&list, "comp")), ["compact"]);
        assert!(matches(&list, "zzz").is_empty());
    }

    #[test]
    fn the_menu_is_bounded() {
        let many: Vec<_> = (0..(MAX_MATCHES * 3))
            .map(|index| Candidate {
                name: format!("c{index:03}"),
                description: String::new(),
            })
            .collect();
        assert_eq!(matches(&many, "").len(), MAX_MATCHES);
        assert_eq!(matches(&many, "c").len(), MAX_MATCHES);
    }

    #[test]
    fn selection_wraps_at_both_ends() {
        assert_eq!(advance(0, 3, -1), 2);
        assert_eq!(advance(2, 3, 1), 0);
        assert_eq!(advance(1, 3, 1), 2);
        // An empty list and an out-of-range selection are both safe.
        assert_eq!(advance(0, 0, 1), 0);
        // An out-of-range selection clamps to the last row, then wraps.
        assert_eq!(advance(9, 3, 1), 0);
        assert_eq!(advance(9, 3, -1), 1);
    }

    #[test]
    fn accepting_replaces_the_token_and_keeps_arguments() {
        let list = candidates(&[("plan", "a"), ("compact", "b")]);
        let matched = matches(&list, "pl");
        assert_eq!(names(&[matched[0]]), ["plan"]);
        assert_eq!(accepted("/pl", "plan"), "/plan ");
        assert_eq!(accepted("/plan", "plan"), "/plan ");
        // A half-typed argument survives, separated by exactly one space.
        assert_eq!(accepted("/pl   off", "plan"), "/plan off");
        assert_eq!(accepted("/", "compact"), "/compact ");
        // Accepting never leaves a bare command without a separator, so the next
        // word typed cannot join the command name.
        assert!(!accepted("/", "help").is_empty());
        assert!(accepted("/", "help").ends_with(' '));
    }
}
