//! Paths of `spdd/` files found in a text.
//!
//! Canvases are gitignored working files and are never committed, so any path
//! that names one is a reference, whether or not the file exists on a disk. A
//! path is judged by the number of its issue, not by its exact name, so a
//! wildcard (`GH-64-*`) or an ellipsis (`GH-6-…`) counts. A path with no digits
//! after `GH-` is a placeholder and is ignored.

const MARKER: &str = "spdd/";
const DIRECTORIES: [&str; 2] = ["spdd/analysis/GH-", "spdd/prompt/GH-"];

/// Whether `text` names a numbered `spdd/` file.
pub(super) fn names_file(text: &str) -> bool {
    text.match_indices(MARKER)
        .any(|(offset, _)| has_issue_at(text, offset))
}

fn has_issue_at(text: &str, offset: usize) -> bool {
    issue_at(text, offset).is_some()
}

fn issue_at(text: &str, offset: usize) -> Option<u64> {
    if is_glued(text.get(..offset)?) {
        return None;
    }
    let rest = text.get(offset..)?;
    let after = DIRECTORIES
        .iter()
        .find_map(|directory| rest.strip_prefix(directory))?;
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<u64>().ok()
}

/// Whether the marker is the tail of a longer word or path, such as a link to
/// a file at a commit, which is a permalink and not a reference.
fn is_glued(before: &str) -> bool {
    before
        .chars()
        .next_back()
        .is_some_and(|character| character.is_alphanumeric() || character == '/')
}
