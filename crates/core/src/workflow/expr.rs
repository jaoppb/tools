//! A compiled CEL expression and the load-time checks over it.

use std::collections::BTreeSet;

use cel_interpreter::Program;
use cel_parser::Parser;
use cel_parser::ast::{EntryExpr, Expr as Node, IdedExpr};

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
        // `Program` keeps its AST private, so the source is parsed a second
        // time here to walk it; expressions are short and this runs at load.
        let program = Program::compile(source).map_err(|errors| compile_error(origin, &errors))?;
        let tree = Parser::default()
            .parse(source)
            .map_err(|errors| compile_error(origin, &errors))?;
        let mut walk = Walk::default();
        walk.visit(&tree, &mut Vec::new());
        let Walk { reads, functions } = walk;
        if let Some(name) = reads.iter().find(|name| !known.contains(*name)) {
            return Err(unknown(origin, "identifier", name));
        }
        let unknown_function = functions
            .iter()
            .find(|name| !is_builtin(name) && !HELPERS.contains(&name.as_str()));
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
    pub(crate) const fn reads(&self) -> &BTreeSet<String> {
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

fn compile_error(origin: &str, errors: &impl std::fmt::Display) -> LoadError {
    LoadError::Compile {
        origin: origin.to_owned(),
        message: errors.to_string(),
    }
}

/// Operators (`_&&_`, `!_`, `@in`) are calls in the AST; so are the functions
/// `cel-interpreter` ships and the schema allows. Comprehension macros
/// (`exists`, `all`, `filter`, `map`, `exists_one`) and `has()` are expanded by
/// the parser and never show up as calls.
fn is_builtin(name: &str) -> bool {
    const BUILTINS: &[&str] = &[
        // Operators, as the parser names their calls.
        "_?_:_",
        "_&&_",
        "_||_",
        "!_",
        "-_",
        "_-_",
        "_+_",
        "_*_",
        "_/_",
        "_%_",
        "_==_",
        "_!=_",
        "_>=_",
        "_<=_",
        "_>_",
        "_<_",
        "_[_]",
        "@in",
        "@not_strictly_false",
        // Functions of the interpreter.
        "size",
        "startsWith",
        "endsWith",
        "contains",
        "int",
        "uint",
        "double",
        "string",
        "max",
        "min",
    ];
    BUILTINS.contains(&name)
}

/// What an expression uses: the free identifiers and the functions it calls.
///
/// The walk follows scopes: a comprehension binds its iteration variables
/// (and the macro accumulator) for its loop and result expressions only, so a
/// name bound in one place does not hide a free identifier spelled the same
/// somewhere else.
#[derive(Default)]
struct Walk {
    reads: BTreeSet<String>,
    functions: BTreeSet<String>,
}

impl Walk {
    fn visit(&mut self, node: &IdedExpr, scope: &mut Vec<String>) {
        match &node.expr {
            Node::Unspecified | Node::Literal(_) => {}
            Node::Ident(name) => {
                if !scope.contains(name) {
                    self.reads.insert(name.clone());
                }
            }
            Node::Select(select) => self.visit(&select.operand, scope),
            Node::Call(call) => {
                self.functions.insert(call.func_name.clone());
                if let Some(target) = &call.target {
                    self.visit(target, scope);
                }
                for argument in &call.args {
                    self.visit(argument, scope);
                }
            }
            Node::List(list) => {
                for element in &list.elements {
                    self.visit(element, scope);
                }
            }
            Node::Map(map) => {
                for entry in &map.entries {
                    self.entry(&entry.expr, scope);
                }
            }
            Node::Struct(literal) => {
                for entry in &literal.entries {
                    self.entry(&entry.expr, scope);
                }
            }
            Node::Comprehension(comprehension) => {
                self.visit(&comprehension.iter_range, scope);
                self.visit(&comprehension.accu_init, scope);
                let depth = scope.len();
                scope.push(comprehension.iter_var.clone());
                scope.extend(comprehension.iter_var2.clone());
                scope.push(comprehension.accu_var.clone());
                self.visit(&comprehension.loop_cond, scope);
                self.visit(&comprehension.loop_step, scope);
                self.visit(&comprehension.result, scope);
                scope.truncate(depth);
            }
        }
    }

    fn entry(&mut self, entry: &EntryExpr, scope: &mut Vec<String>) {
        match entry {
            EntryExpr::StructField(field) => self.visit(&field.value, scope),
            EntryExpr::MapEntry(pair) => {
                self.visit(&pair.key, scope);
                self.visit(&pair.value, scope);
            }
        }
    }
}
