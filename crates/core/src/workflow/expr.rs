//! A compiled CEL expression and the load-time checks over it.

use std::collections::BTreeSet;

use cel_interpreter::Program;

use crate::workflow::error::LoadError;
use crate::workflow::schema::HELPERS;

/// A CEL expression with the source it was compiled from.
#[derive(Debug)]
pub struct Expr {
    source: String,
    program: Program,
    reads: BTreeSet<String>,
}

impl Expr {
    /// Compiles `source`, then checks every identifier and function it uses
    /// against `known` (schema roots and fact names) and the fixed helpers.
    ///
    /// # Errors
    ///
    /// [`LoadError::Compile`] with the compiler's position, or
    /// [`LoadError::Unknown`] for a name the schema does not define.
    pub fn compile(
        origin: &str,
        source: &str,
        known: &BTreeSet<String>,
    ) -> Result<Self, LoadError> {
        let program = Program::compile(source).map_err(|errors| LoadError::Compile {
            origin: origin.to_owned(),
            message: errors.to_string(),
        })?;
        let bound = macro_variables(source);
        let references = program.references();
        let reads: BTreeSet<String> = references
            .variables()
            .into_iter()
            .filter(|name| !bound.contains(*name))
            .map(str::to_owned)
            .collect();
        if let Some(name) = reads.iter().find(|name| !known.contains(*name)) {
            return Err(unknown(origin, "identifier", name));
        }
        let unknown_function = references
            .functions()
            .into_iter()
            .find(|name| !is_builtin(name) && !HELPERS.contains(name));
        if let Some(name) = unknown_function {
            return Err(unknown(origin, "function", name));
        }
        Ok(Self {
            source: source.to_owned(),
            program,
            reads,
        })
    }

    /// The source text.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The compiled program.
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }

    /// The free identifiers the expression reads.
    #[must_use]
    pub const fn reads(&self) -> &BTreeSet<String> {
        &self.reads
    }
}

fn unknown(origin: &str, kind: &'static str, name: &str) -> LoadError {
    LoadError::Unknown {
        origin: origin.to_owned(),
        kind,
        name: name.to_owned(),
    }
}

/// Operators (`_&&_`, `!_`, `@in`) show up in `references()` as functions; so
/// do the functions and macros `cel-interpreter` ships and the schema allows.
fn is_builtin(name: &str) -> bool {
    if name.starts_with(['_', '!', '@']) {
        return true;
    }
    const BUILTINS: &[&str] = &[
        "has",
        "size",
        "startsWith",
        "endsWith",
        "contains",
        "exists",
        "all",
        "filter",
        "map",
        "exists_one",
        "int",
        "uint",
        "double",
        "string",
        "max",
        "min",
    ];
    BUILTINS.contains(&name)
}

/// The variables that comprehension macros bind in `source`.
///
/// `references()` cannot tell a bound `l` from a free identifier, and the AST
/// is private to `cel-interpreter`; reading the identifier after `.exists(` and
/// its siblings is the spike's stand-in for walking the AST.
fn macro_variables(source: &str) -> BTreeSet<String> {
    const MACROS: &[&str] = &[".exists(", ".exists_one(", ".all(", ".filter(", ".map("];
    let mut bound = BTreeSet::new();
    for opener in MACROS {
        for (at, _) in source.match_indices(opener) {
            let rest = source[at + opener.len()..].trim_start();
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() && rest[name.len()..].trim_start().starts_with(',') {
                bound.insert(name);
            }
        }
    }
    bound
}
