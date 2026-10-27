//! Repository merge and branch settings, labels, and milestones enforced on GitHub.

mod label;
mod milestone;
mod plan;
mod settings;

pub use self::label::{LabelDiff, RepoLabel, RepoLabelsSpec};
pub use self::milestone::{
    MilestoneDiff, MilestoneState, RepoMilestone, RepoMilestoneSpec, RepoMilestonesSpec,
};
pub use self::plan::{DeleteReason, RepoEntities, RepoPlan, RepoState};
pub use self::settings::{RepoSettings, SettingDiff};
