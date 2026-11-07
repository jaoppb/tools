//! Fact schema v1: the names a rule may read and the helpers it may call.

/// The schema version this build implements.
pub const VERSION: i64 = 1;

/// The status keyword for "leave the card where it is".
pub const CURRENT: &str = "@current";

/// Top-level names every expression sees besides the derived facts.
pub const ROOTS: &[&str] = &["issue", "params", "repo"];

/// The fixed helper set of schema v1. Helpers are generic string functions.
pub const HELPERS: &[&str] = &["fold"];
