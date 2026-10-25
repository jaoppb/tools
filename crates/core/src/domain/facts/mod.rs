//! What triage sets on an issue: labels, milestone, Priority and Effort.

use std::fmt;

/// A GitHub label, such as `type:feat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label(String);

impl Label {
    /// The label with this text.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self(text.to_owned())
    }

    fn has_prefix(&self, prefix: &str) -> bool {
        self.0.starts_with(prefix)
    }

    fn is(&self, text: &str) -> bool {
        self.0 == text
    }
}

/// The labels of one issue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Labels(Vec<Label>);

impl Labels {
    /// The labels of an issue.
    #[must_use]
    pub const fn new(labels: Vec<Label>) -> Self {
        Self(labels)
    }

    /// Whether exactly one label starts with `type:`.
    #[must_use]
    pub fn has_one_type(&self) -> bool {
        self.0
            .iter()
            .filter(|label| label.has_prefix("type:"))
            .count()
            == 1
    }

    /// Whether at least one label starts with `area:`.
    #[must_use]
    pub fn has_area(&self) -> bool {
        self.0.iter().any(|label| label.has_prefix("area:"))
    }

    /// Whether the issue is an epic.
    #[must_use]
    pub fn is_epic(&self) -> bool {
        self.0.iter().any(|label| label.is("epic"))
    }
}

/// The release an issue belongs to, such as `v1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Milestone(String);

impl Milestone {
    /// The milestone with this title.
    #[must_use]
    pub fn new(title: &str) -> Self {
        Self(title.to_owned())
    }
}

impl fmt::Display for Milestone {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The Priority option of an issue, such as `P1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Priority(String);

impl Priority {
    /// The Priority with this option name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self(name.to_owned())
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The Effort of an issue, in points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effort(u8);

impl Effort {
    const READY_LIMIT: u8 = 8;

    /// An effort of `points`.
    #[must_use]
    pub const fn new(points: u8) -> Self {
        Self(points)
    }

    /// Whether the effort is small enough for an issue to be Ready.
    #[must_use]
    pub const fn fits_ready(self) -> bool {
        self.0 <= Self::READY_LIMIT
    }
}

/// A field that triage has not set correctly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// The issue does not have exactly one `type:` label.
    Type,
    /// The issue has no `area:` label.
    Area,
    /// The issue has no milestone.
    Milestone,
    /// The issue has no Priority.
    Priority,
    /// The issue has no Effort.
    Effort,
    /// The Effort is above the limit for Ready: the story must be split.
    EffortAboveReady,
}

impl Missing {
    const ALL: [Self; 6] = [
        Self::Type,
        Self::Area,
        Self::Milestone,
        Self::Priority,
        Self::Effort,
        Self::EffortAboveReady,
    ];

    fn applies_to(self, facts: &Facts) -> bool {
        match self {
            Self::Type => !facts.labels.has_one_type(),
            Self::Area => !facts.labels.has_area(),
            Self::Milestone => facts.milestone.is_none(),
            Self::Priority => facts.priority.is_none(),
            Self::Effort => facts.effort.is_none(),
            Self::EffortAboveReady => facts.effort.is_some_and(|effort| !effort.fits_ready()),
        }
    }
}

impl fmt::Display for Missing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Type => "not exactly one type label",
            Self::Area => "no area label",
            Self::Milestone => "no milestone",
            Self::Priority => "no Priority",
            Self::Effort => "no Effort",
            Self::EffortAboveReady => "Effort above 8, split the story",
        };
        formatter.write_str(text)
    }
}

/// Everything triage sets on an issue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    labels: Labels,
    milestone: Option<Milestone>,
    priority: Option<Priority>,
    effort: Option<Effort>,
}

impl Facts {
    /// Facts with these labels and nothing else set.
    #[must_use]
    pub const fn new(labels: Labels) -> Self {
        Self {
            labels,
            milestone: None,
            priority: None,
            effort: None,
        }
    }

    /// These facts with a milestone.
    #[must_use]
    pub fn with_milestone(self, milestone: Milestone) -> Self {
        Self {
            milestone: Some(milestone),
            ..self
        }
    }

    /// These facts with a Priority.
    #[must_use]
    pub fn with_priority(self, priority: Priority) -> Self {
        Self {
            priority: Some(priority),
            ..self
        }
    }

    /// These facts with an Effort.
    #[must_use]
    pub fn with_effort(self, effort: Effort) -> Self {
        Self {
            effort: Some(effort),
            ..self
        }
    }

    /// Whether the issue is an epic.
    #[must_use]
    pub fn is_epic(&self) -> bool {
        self.labels.is_epic()
    }

    /// What triage still has to set, in a fixed order.
    #[must_use]
    pub fn missing(&self) -> Vec<Missing> {
        Missing::ALL
            .into_iter()
            .filter(|missing| missing.applies_to(self))
            .collect()
    }
}
