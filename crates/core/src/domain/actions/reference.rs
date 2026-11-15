//! A `uses:` reference of a workflow file and how it pins the action.

use std::fmt;

/// A major version number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Major(pub u64);

impl Major {
    /// The major of `v7`, `7`, `v7.1` or `v7.1.2`; `None` for anything else (`main`, `v7-rc1`).
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('v').unwrap_or(text);
        let mut parts = digits.split('.');
        let major = parts.next()?;
        let rest: Vec<&str> = parts.collect();
        let numeric =
            |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
        if rest.len() > 2 || !numeric(major) || !rest.iter().all(|part| numeric(part)) {
            return None;
        }
        major.parse().ok().map(Self)
    }
}

impl fmt::Display for Major {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "v{}", self.0)
    }
}

/// The `owner/repo` an action lives in.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RepoId {
    /// The owner.
    pub owner: String,
    /// The repository name.
    pub name: String,
}

impl RepoId {
    /// Whether `pattern` (`owner/repo` or `owner/*`, case-insensitive) names this repository.
    #[must_use]
    pub fn matches(&self, pattern: &str) -> bool {
        let (owner, name) = pattern.split_once('/').unwrap_or((pattern, "*"));
        owner.eq_ignore_ascii_case(&self.owner)
            && (name == "*" || name.eq_ignore_ascii_case(&self.name))
    }
}

impl fmt::Display for RepoId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.owner, self.name)
    }
}

/// How a reference pins the action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pin {
    /// A version tag such as `v7` or `v7.1.2`.
    Version(Major),
    /// A full commit SHA, with the major its trailing `# vX` comment names, if any.
    Sha {
        /// The 40-hex commit.
        sha: String,
        /// The major of the comment.
        comment: Option<Major>,
    },
    /// Any other ref, such as a branch: it has no major.
    Mutable(String),
}

/// One `uses:` line of a workflow file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// The workflow file.
    pub file: String,
    /// The 1-based line.
    pub line: usize,
    /// The repository of the action.
    pub repo: RepoId,
    /// How the action is pinned.
    pub pin: Pin,
}

impl Reference {
    /// `file:line`, for reports.
    #[must_use]
    pub fn location(&self) -> String {
        format!("{}:{}", self.file, self.line)
    }

    /// The major the reference claims: its tag, or the comment of a SHA.
    #[must_use]
    pub const fn claimed(&self) -> Option<Major> {
        match &self.pin {
            Pin::Version(major) => Some(*major),
            Pin::Sha { comment, .. } => *comment,
            Pin::Mutable(_) => None,
        }
    }
}

/// The references of one workflow file; local (`./`) and `docker://` uses have no version.
#[must_use]
pub fn scan(file: &str, text: &str) -> Vec<Reference> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| parse_line(file, index + 1, line))
        .collect()
}

fn parse_line(file: &str, line: usize, text: &str) -> Option<Reference> {
    let trimmed = text.trim_start();
    let trimmed = trimmed.strip_prefix("- ").map_or(trimmed, str::trim_start);
    let value = trimmed.strip_prefix("uses:")?.trim();
    let (target, comment) = value
        .split_once('#')
        .map_or((value, ""), |(target, comment)| (target, comment));
    let target = target.trim().trim_matches(['"', '\'']);
    if target.starts_with("./") || target.starts_with("docker://") {
        return None;
    }
    let (path, pinned) = target.split_once('@')?;
    let mut segments = path.split('/');
    let (owner, name) = (segments.next()?, segments.next()?);
    let comment = comment.split_whitespace().next().and_then(Major::parse);
    Some(Reference {
        file: file.to_owned(),
        line,
        repo: RepoId {
            owner: owner.to_owned(),
            name: name.to_owned(),
        },
        pin: classify(pinned, comment),
    })
}

fn classify(pinned: &str, comment: Option<Major>) -> Pin {
    if pinned.len() == 40 && pinned.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Pin::Sha {
            sha: pinned.to_ascii_lowercase(),
            comment,
        };
    }
    // A bare number (`2024`, a short SHA) is more likely a branch than a version.
    let version = pinned
        .starts_with('v')
        .then(|| Major::parse(pinned))
        .flatten();
    version.map_or_else(|| Pin::Mutable(pinned.to_owned()), Pin::Version)
}
