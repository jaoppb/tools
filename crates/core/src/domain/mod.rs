//! The rules of the issue lifecycle, over a snapshot of the Project.
//!
//! This layer imports only `std`: it never sees `gh`, JSON, TOML or the network.

pub mod audit;
pub mod card;
pub mod facts;
pub mod issue;
pub mod links;
pub mod repo;
pub mod snapshot;

pub use crate::domain::audit::{
    BlockedBy, Cycle, IssueTree, Members, Placement, Release, Rule, Surface, Survey, Text,
    TextItem, Texts, TreeIssue, Violation, Violations,
};
pub use crate::domain::card::{Card, Cards, Standing};
pub use crate::domain::facts::{Effort, Facts, Label, Labels, Milestone, Missing, Priority};
pub use crate::domain::issue::{
    IssueMigrationAction, IssueMigrationPlan, IssueNumber, Lifecycle, Role, StatusName, Statuses,
    StatusesError, UnknownStatus,
};
pub use crate::domain::links::{Blocker, Blockers, Links, Parent};
pub use crate::domain::repo::{
    DeleteReason, LabelDiff, MilestoneDiff, MilestoneState, Repo, RepoLabel, RepoLabelsSpec,
    RepoMilestone, RepoMilestoneSpec, RepoMilestonesSpec, RepoPlan, RepoPlanError, RepoSettings,
    RepoSpec, RepoState, SettingDiff,
};
pub use crate::domain::snapshot::{Closes, Closing, Snapshot};
