//! What triage sets on an issue: labels, milestone, Priority and Effort.

use std::fmt;

use crate::config::RulesConfig;

/// A GitHub label, such as `type:feat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label(String);

impl Label {
    /// The label with this text.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self(text.to_owned())
    }

    /// The label text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn has_prefix(&self, prefix: &str) -> bool {
        self.0.starts_with(prefix)
    }

    fn has_prefix_folded(&self, prefix: &str) -> bool {
        if prefix.is_empty() {
            return false;
        }
        if self.0.starts_with(prefix) {
            return true;
        }
        let deunicoded = deunicode::deunicode(&self.0);
        let normalized_prefix = if prefix.is_ascii() {
            prefix.to_ascii_lowercase()
        } else {
            deunicode::deunicode(prefix).to_ascii_lowercase()
        };
        deunicoded
            .to_ascii_lowercase()
            .starts_with(&normalized_prefix)
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

    /// Creates labels from string names.
    #[must_use]
    pub fn from_names(names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(names.into_iter().map(|n| Label(n.into())).collect())
    }

    /// The slice of labels.
    #[must_use]
    pub fn as_slice(&self) -> &[Label] {
        &self.0
    }

    /// An iterator over label name strings.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(Label::as_str)
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

    /// Whether at least one label starts with `area:` (accent-insensitive).
    #[must_use]
    pub fn has_area(&self) -> bool {
        self.has_area_matching(&["area:"])
    }

    /// Whether at least one label matches any of the given area prefixes (accent-insensitive).
    #[must_use]
    pub fn has_area_matching<S: AsRef<str>>(&self, prefixes: &[S]) -> bool {
        self.0.iter().any(|label| {
            prefixes
                .iter()
                .any(|prefix| label.has_prefix_folded(prefix.as_ref()))
        })
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
    fn applies_to(self, facts: &Facts, rules: &RulesConfig) -> bool {
        match self {
            Self::Type => !facts.labels.has_one_type(),
            Self::Area => !facts.labels.has_area_matching(&rules.area_prefixes),
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

    /// What triage still has to set under the given rule configuration.
    #[must_use]
    pub fn missing_for(&self, rules: &RulesConfig) -> Vec<Missing> {
        let mut missing = Vec::new();
        if Missing::Type.applies_to(self, rules) {
            missing.push(Missing::Type);
        }
        if Missing::Area.applies_to(self, rules) {
            missing.push(Missing::Area);
        }
        if Missing::Milestone.applies_to(self, rules) {
            missing.push(Missing::Milestone);
        }
        if rules.priority && Missing::Priority.applies_to(self, rules) {
            missing.push(Missing::Priority);
        }
        if rules.effort {
            if Missing::Effort.applies_to(self, rules) {
                missing.push(Missing::Effort);
            } else if Missing::EffortAboveReady.applies_to(self, rules) {
                missing.push(Missing::EffortAboveReady);
            }
        }
        missing
    }

    /// What triage still has to set, under default rules.
    #[must_use]
    pub fn missing(&self) -> Vec<Missing> {
        self.missing_for(&RulesConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_matches_accented_and_standard_area_prefix() {
        let standard = Label::new("area:core");
        let accented = Label::new("área:segurança");
        let uppercase = Label::new("Área:Arquitetura");
        let other = Label::new("type:feat");

        assert!(standard.has_prefix_folded("area:"));
        assert!(accented.has_prefix_folded("area:"));
        assert!(uppercase.has_prefix_folded("area:"));
        assert!(!other.has_prefix_folded("area:"));
        assert!(!standard.has_prefix_folded(""));
    }

    #[test]
    fn label_matches_configured_prefix_alias() {
        let label = Label::new("domain:auth");
        assert!(label.has_prefix_folded("domain:"));
        assert!(!label.has_prefix_folded("area:"));
    }

    #[test]
    fn labels_has_area_and_has_area_matching() {
        let default_labels = Labels::from_names(["type:feat", "área:segurança"]);
        assert!(default_labels.has_area());

        let custom_labels = Labels::from_names(["type:feat", "domain:auth"]);
        assert!(!custom_labels.has_area());
        assert!(custom_labels.has_area_matching(&["domain:"]));
        assert!(custom_labels.has_area_matching(&["area:", "domain:"]));
    }

    #[test]
    fn facts_missing_with_custom_prefixes() {
        let facts = Facts::new(Labels::from_names(["type:feat", "área:segurança"]))
            .with_milestone(Milestone::new("v0.1.0"));
        let default_rules = RulesConfig::default();
        assert!(facts.missing_for(&default_rules).is_empty());

        let custom_rules = RulesConfig {
            area_prefixes: vec!["custom:".to_string()],
            ..RulesConfig::default()
        };
        assert_eq!(facts.missing_for(&custom_rules), vec![Missing::Area]);
    }
}
