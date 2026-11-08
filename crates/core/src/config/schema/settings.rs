//! `[github.repository.settings]`.

use crate::domain::repo::{MergeCommitMessage, MergeCommitTitle, RepoSettings};
use serde::Deserialize;

const fn default_true() -> bool {
    true
}

/// Default merge commit title format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MergeCommitTitleConfig {
    /// Use the pull request title.
    #[default]
    PrTitle,
    /// Use the classic merge message.
    MergeMessage,
}

impl From<MergeCommitTitleConfig> for MergeCommitTitle {
    fn from(config: MergeCommitTitleConfig) -> Self {
        match config {
            MergeCommitTitleConfig::PrTitle => Self::PrTitle,
            MergeCommitTitleConfig::MergeMessage => Self::MergeMessage,
        }
    }
}

/// Default merge commit message format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MergeCommitMessageConfig {
    /// Use the pull request body.
    #[default]
    PrBody,
    /// Use the pull request title.
    PrTitle,
    /// Default to blank.
    Blank,
}

impl From<MergeCommitMessageConfig> for MergeCommitMessage {
    fn from(config: MergeCommitMessageConfig) -> Self {
        match config {
            MergeCommitMessageConfig::PrBody => Self::PrBody,
            MergeCommitMessageConfig::PrTitle => Self::PrTitle,
            MergeCommitMessageConfig::Blank => Self::Blank,
        }
    }
}

/// The repository settings to enforce.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RepositorySettingsConfig {
    /// Whether merge commits are allowed.
    #[serde(default = "default_true")]
    allow_merge_commit: bool,
    /// Whether squash merges are allowed.
    #[serde(default)]
    allow_squash_merge: bool,
    /// Whether rebase merges are allowed.
    #[serde(default)]
    allow_rebase_merge: bool,
    /// Whether head branches are deleted upon merge.
    #[serde(default = "default_true")]
    delete_branch_on_merge: bool,
    /// Default merge commit title format.
    #[serde(default)]
    merge_commit_title: MergeCommitTitleConfig,
    /// Default merge commit message format.
    #[serde(default)]
    merge_commit_message: MergeCommitMessageConfig,
}

impl From<RepositorySettingsConfig> for RepoSettings {
    fn from(config: RepositorySettingsConfig) -> Self {
        Self {
            allow_merge_commit: config.allow_merge_commit,
            allow_squash_merge: config.allow_squash_merge,
            allow_rebase_merge: config.allow_rebase_merge,
            delete_branch_on_merge: config.delete_branch_on_merge,
            merge_commit_title: config.merge_commit_title.into(),
            merge_commit_message: config.merge_commit_message.into(),
        }
    }
}
