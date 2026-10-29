//! Repository merge and branch settings, labels, milestones, and project enforced on GitHub.

mod label;
mod milestone;
mod plan;
mod project;
mod reconcile;
mod settings;

pub(crate) use self::reconcile::{ReconcileHandler, reconcile_by_key, validate_collection_spec};

pub use self::label::{LabelDiff, RepoLabel, RepoLabelsSpec};
pub use self::milestone::{
    MilestoneDiff, MilestoneState, RepoMilestone, RepoMilestoneSpec, RepoMilestonesSpec,
};
pub use self::plan::{DeleteReason, RepoPlan, RepoPlanError, RepoSpec, RepoState};
pub use self::project::{ProjectDiff, RepoProject, RepoProjectSpec};
pub use self::settings::{MergeCommitMessage, MergeCommitTitle, RepoSettings, SettingDiff};
