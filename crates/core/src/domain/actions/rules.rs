//! The configurable rules and the findings they produce.

use std::fmt;

use super::reference::{Major, Pin, Reference};

/// The ref style `pin_format` demands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinFormat {
    /// A full commit SHA with a trailing `# vX` comment.
    ShaWithComment,
    /// A version tag (`@vX`).
    Tag,
}

/// The rules a repository opted into; the default checks nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rules {
    /// The referenced major must be the latest major.
    pub latest_major: bool,
    /// The demanded ref style.
    pub pin_format: Option<PinFormat>,
    /// A SHA must be a tag of the major its comment names.
    pub sha_matches_comment: bool,
    /// When set, only these repositories (`owner/repo`, `owner/*`) may be used.
    pub allow: Option<Vec<String>>,
    /// Repositories that may not be used.
    pub deny: Vec<String>,
    /// Repositories exempt from every rule.
    pub ignore: Vec<String>,
}

impl Rules {
    /// Whether any rule is enabled.
    #[must_use]
    pub fn any(&self) -> bool {
        self.latest_major
            || self.pin_format.is_some()
            || self.sha_matches_comment
            || self.allow.is_some()
            || !self.deny.is_empty()
    }

    /// Whether `repo` is exempt from every rule.
    #[must_use]
    pub fn is_ignored(&self, repo: &super::reference::RepoId) -> bool {
        self.ignore.iter().any(|pattern| repo.matches(pattern))
    }

    /// Whether a rule needs to know the versions GitHub has for the repository.
    #[must_use]
    pub const fn needs_versions(&self) -> bool {
        self.latest_major || self.sha_matches_comment
    }
}

/// What GitHub knows about the versions of one repository.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Versions {
    /// The tag of the latest release, when the repository publishes releases.
    pub latest_release: Option<String>,
    /// Every tag with the commit it points at.
    pub tags: Vec<(String, String)>,
}

impl Versions {
    /// The latest major: the highest of the latest release and every version tag. A backport
    /// published after a newer major can be marked "latest", so the release alone can name an
    /// older major.
    #[must_use]
    pub fn latest_major(&self) -> Option<Major> {
        self.latest_release
            .as_deref()
            .and_then(Major::parse)
            .into_iter()
            .chain(self.tags.iter().filter_map(|(tag, _)| Major::parse(tag)))
            .max()
    }

    fn majors_of(&self, sha: &str) -> Vec<Major> {
        self.tags
            .iter()
            .filter(|(_, commit)| commit.eq_ignore_ascii_case(sha))
            .filter_map(|(tag, _)| Major::parse(tag))
            .collect()
    }
}

/// One rule a reference breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// `file:line` of the reference.
    pub location: String,
    /// The rule that failed.
    pub rule: &'static str,
    /// What is wrong and how to fix it.
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: [{}] {}",
            self.location, self.rule, self.message
        )
    }
}

/// The findings of the rules that need nothing from GitHub: `allow`, `deny` and `pin_format`.
#[must_use]
pub fn judge_static(rules: &Rules, reference: &Reference) -> Vec<Finding> {
    let repo = &reference.repo;
    let finding = |rule, message: String| Finding {
        location: reference.location(),
        rule,
        message,
    };
    let mut findings = Vec::new();
    if let Some(allow) = &rules.allow
        && !allow.iter().any(|pattern| repo.matches(pattern))
    {
        findings.push(finding(
            "allow",
            format!("{repo} is not in the allowed list"),
        ));
    }
    if rules.deny.iter().any(|pattern| repo.matches(pattern)) {
        findings.push(finding("deny", format!("{repo} is denied")));
    }
    if let Some(format) = rules.pin_format {
        let ok = match (&reference.pin, format) {
            (Pin::Sha { comment, .. }, PinFormat::ShaWithComment) => comment.is_some(),
            (Pin::Version(_), PinFormat::Tag) => true,
            _ => false,
        };
        if !ok {
            let want = match format {
                PinFormat::ShaWithComment => "a full commit SHA with a trailing `# vX` comment",
                PinFormat::Tag => "a version tag (`@vX`)",
            };
            findings.push(finding(
                "pin-format",
                format!("{repo} must be pinned to {want}"),
            ));
        }
    }
    findings
}

/// The findings of the rules that compare the reference with the versions of its repository:
/// `latest_major` and `sha_matches_comment`.
#[must_use]
pub fn judge_versions(rules: &Rules, reference: &Reference, versions: &Versions) -> Vec<Finding> {
    let mut findings = Vec::new();
    if rules.latest_major {
        findings.extend(latest_major(reference, versions));
    }
    if rules.sha_matches_comment {
        findings.extend(sha_matches_comment(reference, versions));
    }
    findings
}

fn latest_major(reference: &Reference, versions: &Versions) -> Option<Finding> {
    let repo = &reference.repo;
    let finding = |message: String| Finding {
        location: reference.location(),
        rule: "latest-major",
        message,
    };
    let Some(latest) = versions.latest_major() else {
        return Some(finding(format!(
            "cannot determine the latest major of {repo}: no release and no version tag; add it to `ignore`"
        )));
    };
    match reference.claimed() {
        Some(major) if major == latest => None,
        Some(major) => Some(finding(format!(
            "{repo} is at {major} but the latest major is {latest}"
        ))),
        None => Some(finding(format!(
            "{repo} names no major (a branch ref, or a SHA without a `# vX` comment); latest is {latest}"
        ))),
    }
}

fn sha_matches_comment(reference: &Reference, versions: &Versions) -> Option<Finding> {
    let Pin::Sha { sha, comment } = &reference.pin else {
        return None;
    };
    let repo = &reference.repo;
    let finding = |message: String| Finding {
        location: reference.location(),
        rule: "sha-matches-comment",
        message,
    };
    let Some(comment) = comment else {
        return Some(finding(format!(
            "{repo}@{sha} has no `# vX` comment to check"
        )));
    };
    let majors = versions.majors_of(sha);
    if majors.is_empty() {
        return Some(finding(format!(
            "{sha} is not the commit of any tag of {repo}"
        )));
    }
    (!majors.contains(comment)).then(|| {
        finding(format!(
            "the comment says {comment} but {sha} is a tag of {}",
            majors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })
}
