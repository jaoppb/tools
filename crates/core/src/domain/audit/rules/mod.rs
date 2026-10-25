//! The rules of repository issue audit, one module per theme.

pub mod cycle;
pub mod labels;
pub mod membership;
pub mod milestone;
pub mod parent;
pub mod references;
pub mod release;

use crate::config::RulesConfig;
use crate::domain::audit::survey::Survey;
use crate::domain::audit::violation::Violations;

/// Every rule broken in the survey according to the configured rule set, sorted.
#[must_use]
pub fn check(survey: &Survey, rules: &RulesConfig) -> Violations {
    let mut themes = Vec::new();
    if rules.labels {
        themes.push(labels::check(survey));
    }
    if rules.milestone {
        themes.push(milestone::check(survey));
    }
    if rules.parent {
        themes.push(parent::check(survey));
    }
    if rules.release {
        themes.push(release::check(survey));
    }
    if rules.cycle {
        themes.push(cycle::check(survey));
    }
    if rules.membership {
        themes.push(membership::check(survey));
    }
    if rules.references && rules.text {
        themes.push(references::check(survey));
    }
    Violations::new(themes.concat())
}
