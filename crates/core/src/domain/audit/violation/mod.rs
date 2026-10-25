//! What the audit found: the rule an issue breaks, in a form that sorts and
//! prints the same way on every run.

use std::fmt;

use crate::domain::audit::text::Surface;
use crate::domain::issue::IssueNumber;

/// The issues of one blocked-by cycle, lowest number first.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cycle(Vec<IssueNumber>);

impl Cycle {
    /// A cycle of these issues, already in order.
    #[must_use]
    pub const fn new(members: Vec<IssueNumber>) -> Self {
        Self(members)
    }
}

impl fmt::Display for Cycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let members: Vec<String> = self.0.iter().map(ToString::to_string).collect();
        formatter.write_str(&members.join(", "))
    }
}

/// A rule of `AGENTS.md` section 10 that an issue can break.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    /// The issue does not have exactly one `type:` label.
    NotOneType,
    /// The issue has no `area:` label.
    NoArea,
    /// The open issue has no milestone.
    NoMilestone,
    /// The open issue's milestone title is not a release such as `v1`.
    UnreadableMilestone,
    /// The open issue is not an epic and has no parent.
    NoParent,
    /// The open issue sits under this closed parent.
    ClosedParent(IssueNumber),
    /// The open issue is in a later release than this parent.
    ParentEarlierRelease(IssueNumber),
    /// The open issue blocks this issue, which is in an earlier release.
    BlocksEarlierRelease(IssueNumber),
    /// The issue is on this blocked-by cycle.
    Cycle(Cycle),
    /// The issue is not an item of the Project.
    NotInProject,
    /// This surface of the issue or pull request names an ADR by an old number.
    OldAdrNumber(Surface),
    /// This surface of the issue or pull request names a numbered `spdd/` file.
    DeletedSpddPath(Surface),
}

impl Rule {
    /// The short name of the rule, as the report prints it.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::NotOneType => "not-one-type",
            Self::NoArea => "no-area",
            Self::NoMilestone => "no-milestone",
            Self::UnreadableMilestone => "unreadable-milestone",
            Self::NoParent => "no-parent",
            Self::ClosedParent(_) => "closed-parent",
            Self::ParentEarlierRelease(_) => "parent-earlier-release",
            Self::BlocksEarlierRelease(_) => "blocks-earlier-release",
            Self::Cycle(_) => "blocked-by-cycle",
            Self::NotInProject => "not-in-project",
            Self::OldAdrNumber(_) => "old-adr-number",
            Self::DeletedSpddPath(_) => "deleted-spdd-path",
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        describe(self, formatter)
    }
}

fn describe(rule: &Rule, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match rule {
        Rule::NotOneType => formatter.write_str("not exactly one type label"),
        Rule::NoArea => formatter.write_str("no area label"),
        Rule::NoMilestone => formatter.write_str("no milestone"),
        Rule::UnreadableMilestone => {
            formatter.write_str("milestone is not a release such as v1 or v1.1")
        }
        Rule::NoParent => formatter.write_str("no parent and not an epic"),
        Rule::ClosedParent(parent) => {
            write!(formatter, "open under the closed parent {parent}")
        }
        Rule::ParentEarlierRelease(parent) => {
            write!(formatter, "in a later release than its parent {parent}")
        }
        Rule::BlocksEarlierRelease(blocked) => {
            write!(
                formatter,
                "blocks {blocked}, which is in an earlier release"
            )
        }
        Rule::Cycle(cycle) => write!(formatter, "blocked-by cycle: {cycle}"),
        Rule::NotInProject => formatter.write_str("not an item of the Project"),
        Rule::OldAdrNumber(surface) => old_number(*surface, formatter),
        Rule::DeletedSpddPath(surface) => deleted_path(*surface, formatter),
    }
}

fn old_number(surface: Surface, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
        formatter,
        "old ADR number in the {surface}; link the ADR by its title (only formerly ADR-NNNN may keep one)"
    )
}

fn deleted_path(surface: Surface, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
        formatter,
        "spdd file named in the {surface}; canvases are not committed, name the issue as #<n> or link the file at a commit"
    )
}

/// One finding: an issue and the rule it breaks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Violation {
    number: IssueNumber,
    rule: Rule,
}

impl Violation {
    /// The issue `number` breaks `rule`.
    #[must_use]
    pub const fn new(number: IssueNumber, rule: Rule) -> Self {
        Self { number, rule }
    }

    /// One violation of `number` for each rule that is present.
    #[must_use]
    pub fn each(number: IssueNumber, rules: impl IntoIterator<Item = Option<Rule>>) -> Vec<Self> {
        rules
            .into_iter()
            .flatten()
            .map(|rule| Self::new(number, rule))
            .collect()
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} {}: {}",
            self.number,
            self.rule.name(),
            self.rule
        )
    }
}

/// Every finding of a run, sorted by issue number and then by rule.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Violations(Vec<Violation>);

impl Violations {
    /// These findings, sorted.
    #[must_use]
    pub fn new(mut found: Vec<Violation>) -> Self {
        found.sort();
        Self(found)
    }

    /// How many findings there are.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether nothing was found.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The findings in order.
    pub fn iter(&self) -> impl Iterator<Item = &Violation> {
        self.0.iter()
    }
}

impl fmt::Display for Violations {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .iter()
            .try_for_each(|violation| writeln!(formatter, "{violation}"))
    }
}
