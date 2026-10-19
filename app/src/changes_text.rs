//! The changed-files card's own copy, taken from the reference: one title for a turn that edited one
//! file and another for a turn that edited many, with additions and deletions grouped and signed
//! `+N` / `-N` (`ui-deliverables/src/client/locales.ts:100-110`, `ChangedFiles.tsx:25-26,58-60`).
//!
//! The card's counts came from a single muted string with a typographic minus and no grouping, so a
//! turn that edited `report.md` alone read "Changed files · 1 files" and a thousand-line change read
//! `+1000` in the same ink as everything else.

/// The reference's thousands grouping: `1,234`.
pub fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// The card's title: `Edited <name>` for one file, `Edited <n> files` for any other count
/// (`locales.ts:100-101`). The name is the file's basename, as the reference passes `basename(path)`.
pub fn title(total: u64, single: Option<&str>) -> String {
    match single {
        Some(path) if total <= 1 => format!("Edited {}", basename(path)),
        _ => format!("Edited {} files", grouped(total)),
    }
}

/// The last path segment, which is what the reference shows for a single edited file.
pub fn basename(path: &str) -> String {
    path.rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or(path)
        .to_owned()
}

/// One file row's counts, or the reference's own word for a comparison it cannot show
/// (`locales.ts:110,125-126`).
pub fn file_counts(binary: bool, oversized: bool, added: u64, deleted: u64) -> String {
    if binary {
        "binary".to_owned()
    } else if oversized {
        "too large".to_owned()
    } else {
        format!("+{} -{}", grouped(added), grouped(deleted))
    }
}

#[cfg(test)]
mod tests {
    //! The card's copy, at the counts where the shapes differ.
    use super::*;

    #[test]
    fn one_file_names_itself_and_more_count_themselves() {
        assert_eq!(title(1, Some("src/report.md")), "Edited report.md");
        assert_eq!(title(2, None), "Edited 2 files");
        assert_eq!(title(1_234, None), "Edited 1,234 files");
        // A single file whose path cannot be read still counts rather than guessing a name.
        assert_eq!(title(1, None), "Edited 1 files");
    }

    #[test]
    fn counts_are_grouped_and_signed_the_way_the_reference_writes_them() {
        assert_eq!(file_counts(false, false, 0, 0), "+0 -0");
        assert_eq!(file_counts(false, false, 12, 3), "+12 -3");
        assert_eq!(
            file_counts(false, false, 1_500, 1_000_000),
            "+1,500 -1,000,000"
        );
        // The refusal words are the reference's, not "oversized".
        assert_eq!(file_counts(true, false, 0, 0), "binary");
        assert_eq!(file_counts(false, true, 0, 0), "too large");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000), "1,000");
        assert_eq!(grouped(1_234_567), "1,234,567");
    }
}
