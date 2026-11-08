//! Repository merge and branch settings, labels, milestones, and project enforced on GitHub.

mod coord;
mod label;
mod milestone;
mod plan;
mod project;
mod reconcile;
mod settings;
mod unmanaged;

pub(crate) use self::reconcile::{
    ReconcileHandler, overlay_by_key, reconcile_by_key, union_names, validate_collection_spec,
};

pub use self::coord::Repo;
pub use self::label::{LabelDiff, RepoLabel, RepoLabelsOverlay, RepoLabelsSpec};
pub use self::milestone::{
    MilestoneDiff, MilestoneState, RepoMilestone, RepoMilestoneSpec, RepoMilestonesOverlay,
    RepoMilestonesSpec,
};
pub use self::plan::{DeleteReason, RepoPlan, RepoPlanError, RepoSpec, RepoState};
pub use self::project::{ProjectDiff, RepoProject, RepoProjectSpec};
pub use self::settings::{MergeCommitMessage, MergeCommitTitle, RepoSettings, SettingDiff};
pub use self::unmanaged::{Severity, Unmanaged};
