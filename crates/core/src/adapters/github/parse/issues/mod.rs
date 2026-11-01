//! One page of the repository's issues, read into tree issues.

mod shapes;

use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::bad_reply;
use crate::adapters::github::parse::issues::shapes::{Node, Reply};
use crate::adapters::github::target::Target;
use crate::domain::{BlockedBy, IssueNumber, Label, Labels, Lifecycle, Milestone, Repo, TreeIssue};

/// The issues of one page, the cursor of the next page and how many issues the
/// repository says it has in all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuesPage {
    issues: Vec<TreeIssue>,
    next: Option<String>,
    total: u64,
}

impl IssuesPage {
    /// The issues of this page.
    #[must_use]
    pub fn issues(&self) -> &[TreeIssue] {
        &self.issues
    }

    /// The cursor to pass for the next page, if there is one.
    #[must_use]
    pub fn next(&self) -> Option<&str> {
        self.next.as_deref()
    }

    /// How many issues the repository has, open and closed.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.total
    }
}

/// Reads one page of the repository's issues. Cross-repository blockers are
/// retained.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the answer is not the expected JSON or an
/// issue has a state other than open or closed, and [`GhError::Incomplete`]
/// when an issue has more blockers than the page returned.
pub fn parse_issues(json: &str, target: &Target) -> Result<IssuesPage, GhError> {
    let repo = Repo::new(target.owner(), target.primary_repo());
    parse_issues_for(json, &repo)
}

/// Reads one page of issues for an explicit repository.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the answer is not the expected JSON or an
/// issue has a state other than open or closed, and [`GhError::Incomplete`]
/// when an issue has more blockers than the page returned.
pub fn parse_issues_for(json: &str, repo: &Repo) -> Result<IssuesPage, GhError> {
    let reply: Reply = serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let issues = reply.data.repository.issues;
    let next = issues.page_info.next();
    let read = issues
        .nodes
        .iter()
        .map(|node| issue_of(node, repo))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(IssuesPage {
        issues: read,
        next,
        total: issues.total_count,
    })
}

fn issue_of(node: &Node, repo: &Repo) -> Result<TreeIssue, GhError> {
    let number = IssueNumber::new(repo.clone(), node.head.number);
    let issue = TreeIssue::new(number, lifecycle_of(&node.head.state)?)
        .with_labels(labels_of(node))
        .with_blocked_by(blockers_of(node, repo)?);
    Ok(with_placement(issue, node, repo))
}

fn with_placement(issue: TreeIssue, node: &Node, repo: &Repo) -> TreeIssue {
    let issue = match &node.placed.milestone {
        Some(milestone) => issue.with_milestone(Milestone::new(&milestone.title)),
        None => issue,
    };
    match &node.placed.parent {
        Some(parent) => issue.with_parent(IssueNumber::new(repo.clone(), parent.number)),
        None => issue,
    }
}

fn lifecycle_of(state: &str) -> Result<Lifecycle, GhError> {
    match state {
        "OPEN" => Ok(Lifecycle::Open),
        "CLOSED" => Ok(Lifecycle::Closed),
        other => Err(GhError::BadReply {
            reason: format!("an issue has the state {other:?}"),
        }),
    }
}

fn labels_of(node: &Node) -> Labels {
    Labels::new(
        node.placed
            .labels
            .nodes
            .iter()
            .map(|named| Label::new(&named.name))
            .collect(),
    )
}

fn blockers_of(node: &Node, repo: &Repo) -> Result<BlockedBy, GhError> {
    let blocked_by = &node.blocked_by;
    let returned = u64::try_from(blocked_by.nodes.len()).unwrap_or(u64::MAX);
    if returned < blocked_by.total_count {
        return Err(GhError::Incomplete {
            what: format!(
                "issue {} has {returned} of {} blockers",
                IssueNumber::new(repo.clone(), node.head.number),
                blocked_by.total_count
            ),
        });
    }
    let numbers = blocked_by
        .nodes
        .iter()
        .map(|blocking| IssueNumber::new(blocking.repo(repo), blocking.number))
        .collect();
    Ok(BlockedBy::new(numbers))
}
