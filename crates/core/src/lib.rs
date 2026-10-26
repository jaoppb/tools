//! Core library for repository management and issue workflow tools.
//!
//! Provides the GitHub adapters, domain models, rule audits, status sync logic,
//! and configuration parsing from `tools.toml`.

pub mod adapters;
pub mod application;
pub mod cli;
pub mod config;
pub mod domain;
pub mod graphql;
