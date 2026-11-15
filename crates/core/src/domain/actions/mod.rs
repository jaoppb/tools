//! The rules for the `uses:` references of GitHub workflows, over parsed references and the
//! versions GitHub knows. This layer imports only `std`: it never sees `gh`, JSON or TOML.

mod reference;
mod rules;

pub use reference::{Major, Pin, Reference, RepoId, scan};
pub use rules::{Finding, PinFormat, Rules, Versions, judge};
