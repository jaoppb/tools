//! Why a `[workflow]` block does not load.

/// A load-time failure. Every variant names what is wrong; TOML errors carry
/// their own line and column.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// Another error, found at this place of the TOML source.
    #[error("line {line}, column {column}: {error}")]
    At {
        /// 1-based line.
        line: usize,
        /// 1-based column.
        column: usize,
        /// What is wrong there.
        error: Box<LoadError>,
    },
    /// The TOML does not parse or has an unknown key.
    #[error("invalid [workflow] block: {0}")]
    Toml(#[from] toml::de::Error),
    /// `schema` is not a version this build implements.
    #[error("unsupported workflow schema {0}, this build implements 1")]
    Schema(i64),
    /// `extends` names no preset.
    #[error("unknown preset {0:?}")]
    Preset(String),
    /// A rule sets a status that is not declared.
    #[error("rule {rule:?} sets undeclared status {status:?}")]
    UndeclaredStatus {
        /// The rule id.
        rule: String,
        /// The status it sets.
        status: String,
    },
    /// Neither the block nor its preset declares a status.
    #[error("no statuses are declared: list them in `statuses` or extend a preset")]
    NoStatuses,
    /// `default` is not a declared status.
    #[error("default status {0:?} is not declared")]
    UndeclaredDefault(String),
    /// Two local rules share an id.
    #[error("duplicate rule id {0:?}")]
    DuplicateRule(String),
    /// An expression does not compile.
    #[error("{origin}: {message}")]
    Compile {
        /// Where the expression is: `fact "x"` or `rule "x"`.
        origin: String,
        /// The compiler's message with its position.
        message: String,
    },
    /// An expression names something the schema does not define.
    #[error("{origin}: unknown {kind} {name:?}")]
    Unknown {
        /// Where the expression is.
        origin: String,
        /// `identifier` or `function`.
        kind: &'static str,
        /// The unknown name.
        name: String,
    },
    /// A fact is named like a root of the schema.
    #[error("fact {0:?} would hide a name of the schema")]
    ReservedFact(String),
    /// Facts refer to each other in a circle.
    #[error("facts depend on each other in a cycle through {0:?}")]
    FactCycle(String),
    /// A param holds a value CEL cannot carry.
    #[error("param {0:?} holds a value that is neither a scalar nor a list of scalars")]
    Param(String),
}

impl LoadError {
    /// The error without its location.
    #[must_use]
    pub fn root(&self) -> &Self {
        match self {
            Self::At { error, .. } => error.root(),
            other => other,
        }
    }

    /// The 1-based `(line, column)` of the TOML source, when known.
    #[must_use]
    pub const fn location(&self) -> Option<(usize, usize)> {
        match self {
            Self::At { line, column, .. } => Some((*line, *column)),
            _ => None,
        }
    }
}
