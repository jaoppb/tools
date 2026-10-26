//! Generic Abstract Syntax Tree (AST) and builder for GraphQL queries.
//!
//! Provides strongly-typed models for GraphQL documents, operations, selection sets,
//! fields, arguments, values, variable definitions, and fragments, with clean formatting
//! via [`std::fmt::Display`].

use std::fmt;

/// A top-level GraphQL document containing operations and fragments.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Document {
    definitions: Vec<Definition>,
}

impl Document {
    /// Creates an empty GraphQL document.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            definitions: Vec::new(),
        }
    }

    /// Creates a document with a single query operation.
    #[must_use]
    pub fn query(name: Option<&str>, selection_set: SelectionSet) -> Self {
        let op = OperationDefinition::new(OperationType::Query, name, selection_set);
        Self {
            definitions: vec![Definition::Operation(op)],
        }
    }

    /// Creates a document with a single mutation operation.
    #[must_use]
    pub fn mutation(name: Option<&str>, selection_set: SelectionSet) -> Self {
        let op = OperationDefinition::new(OperationType::Mutation, name, selection_set);
        Self {
            definitions: vec![Definition::Operation(op)],
        }
    }

    /// Adds a definition (operation or fragment) to this document.
    pub fn add_definition(&mut self, definition: impl Into<Definition>) {
        self.definitions.push(definition.into());
    }

    /// Builder method to append a definition.
    #[must_use]
    pub fn with_definition(mut self, definition: impl Into<Definition>) -> Self {
        self.add_definition(definition);
        self
    }

    /// Returns a slice of the definitions in this document.
    #[must_use]
    pub fn definitions(&self) -> &[Definition] {
        &self.definitions
    }
}

/// A definition inside a GraphQL document: either an operation or a fragment.
#[derive(Debug, Clone, PartialEq)]
pub enum Definition {
    /// An executable operation (query, mutation, subscription).
    Operation(OperationDefinition),
    /// A reusable fragment definition.
    Fragment(FragmentDefinition),
}

impl From<OperationDefinition> for Definition {
    fn from(op: OperationDefinition) -> Self {
        Self::Operation(op)
    }
}

impl From<FragmentDefinition> for Definition {
    fn from(fragment: FragmentDefinition) -> Self {
        Self::Fragment(fragment)
    }
}

/// GraphQL operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    /// Query operation.
    Query,
    /// Mutation operation.
    Mutation,
    /// Subscription operation.
    Subscription,
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Query => write!(f, "query"),
            Self::Mutation => write!(f, "mutation"),
            Self::Subscription => write!(f, "subscription"),
        }
    }
}

/// Definition of an operation.
#[derive(Debug, Clone, PartialEq)]
pub struct OperationDefinition {
    operation_type: OperationType,
    name: Option<String>,
    variable_definitions: Vec<VariableDefinition>,
    selection_set: SelectionSet,
}

impl OperationDefinition {
    /// Creates a new operation definition.
    #[must_use]
    pub fn new(
        operation_type: OperationType,
        name: Option<&str>,
        selection_set: SelectionSet,
    ) -> Self {
        Self {
            operation_type,
            name: name.map(ToOwned::to_owned),
            variable_definitions: Vec::new(),
            selection_set,
        }
    }

    /// Adds a variable definition to this operation.
    pub fn add_variable(&mut self, variable: VariableDefinition) {
        self.variable_definitions.push(variable);
    }

    /// Builder method to append a variable definition.
    #[must_use]
    pub fn with_variable(mut self, variable: VariableDefinition) -> Self {
        self.add_variable(variable);
        self
    }
}

/// Definition of a variable declared on an operation.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableDefinition {
    name: String,
    type_name: String,
    default_value: Option<Value>,
}

impl VariableDefinition {
    /// Creates a new variable definition (e.g. `$owner: String!`).
    #[must_use]
    pub fn new(name: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_name: type_name.into(),
            default_value: None,
        }
    }

    /// Attaches a default value to this variable.
    #[must_use]
    pub fn with_default(mut self, value: Value) -> Self {
        self.default_value = Some(value);
        self
    }
}

/// Definition of a named fragment: `fragment Name on TypeName { selectionSet }`.
#[derive(Debug, Clone, PartialEq)]
pub struct FragmentDefinition {
    name: String,
    type_condition: String,
    selection_set: SelectionSet,
}

impl FragmentDefinition {
    /// Creates a new fragment definition.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        type_condition: impl Into<String>,
        selection_set: SelectionSet,
    ) -> Self {
        Self {
            name: name.into(),
            type_condition: type_condition.into(),
            selection_set,
        }
    }
}

/// A set of selections on a GraphQL type.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SelectionSet {
    items: Vec<Selection>,
}

impl SelectionSet {
    /// Creates an empty selection set.
    #[must_use]
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Creates a selection set from a collection of selections.
    #[must_use]
    pub fn from_items(items: impl IntoIterator<Item = Selection>) -> Self {
        Self {
            items: items.into_iter().collect(),
        }
    }

    /// Adds a selection to this set.
    pub fn add(&mut self, selection: impl Into<Selection>) {
        self.items.push(selection.into());
    }

    /// Builder method to append a selection.
    #[must_use]
    pub fn with(mut self, selection: impl Into<Selection>) -> Self {
        self.add(selection);
        self
    }

    /// Appends a field by name.
    #[must_use]
    pub fn with_field(mut self, name: impl Into<String>) -> Self {
        self.add(Field::new(name));
        self
    }

    /// Whether the selection set has no selections.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns a slice of the items.
    #[must_use]
    pub fn items(&self) -> &[Selection] {
        &self.items
    }
}

/// One selection in a selection set: a field, fragment spread, or inline fragment.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    /// A queried field.
    Field(Field),
    /// A named fragment spread (`...FragmentName`).
    FragmentSpread(String),
    /// An inline fragment (`... on TypeName { selectionSet }`).
    InlineFragment(InlineFragment),
}

impl From<Field> for Selection {
    fn from(field: Field) -> Self {
        Self::Field(field)
    }
}

/// An inline fragment: `... on TypeCondition { selectionSet }`.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineFragment {
    type_condition: Option<String>,
    selection_set: SelectionSet,
}

impl InlineFragment {
    /// Creates an inline fragment on `type_condition`.
    #[must_use]
    pub fn on_type(type_condition: impl Into<String>, selection_set: SelectionSet) -> Self {
        Self {
            type_condition: Some(type_condition.into()),
            selection_set,
        }
    }

    /// Creates an untyped inline fragment.
    #[must_use]
    pub const fn untyped(selection_set: SelectionSet) -> Self {
        Self {
            type_condition: None,
            selection_set,
        }
    }
}

/// A queried field, optionally with alias, arguments, and sub-selection set.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    alias: Option<String>,
    name: String,
    arguments: Vec<Argument>,
    selection_set: Option<SelectionSet>,
}

impl Field {
    /// Creates a simple field with `name`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            alias: None,
            name: name.into(),
            arguments: Vec::new(),
            selection_set: None,
        }
    }

    /// Creates a field with an `alias` and `name`.
    #[must_use]
    pub fn with_alias(alias: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            alias: Some(alias.into()),
            name: name.into(),
            arguments: Vec::new(),
            selection_set: None,
        }
    }

    /// Adds an argument to this field.
    pub fn add_argument(&mut self, name: impl Into<String>, value: impl Into<Value>) {
        self.arguments.push(Argument::new(name, value));
    }

    /// Builder method to append an argument.
    #[must_use]
    pub fn with_arg(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.add_argument(name, value);
        self
    }

    /// Sets the selection set for this field.
    #[must_use]
    pub fn with_selection_set(mut self, selection_set: SelectionSet) -> Self {
        self.selection_set = Some(selection_set);
        self
    }
}

/// An argument on a field or directive.
#[derive(Debug, Clone, PartialEq)]
pub struct Argument {
    name: String,
    value: Value,
}

impl Argument {
    /// Creates a new argument.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<Value>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// A GraphQL literal or variable value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Variable reference (e.g. `$owner`).
    Variable(String),
    /// Integer literal.
    Int(i64),
    /// Floating point literal.
    Float(f64),
    /// String literal.
    String(String),
    /// Boolean literal.
    Boolean(bool),
    /// Unquoted enum literal or identifier (e.g. `OPEN`).
    Enum(String),
    /// List of values: `[a, b, c]`.
    List(Vec<Value>),
    /// Input object: `{ key: value }`.
    Object(Vec<(String, Value)>),
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::String(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Self::Int(n)
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Self::Int(i64::from(n))
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Boolean(b)
    }
}

// ---------------------------------------------------------------------------
// Formatting implementations
// ---------------------------------------------------------------------------

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (idx, def) in self.definitions.iter().enumerate() {
            if idx > 0 {
                writeln!(f)?;
            }
            write!(f, "{def}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Definition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Operation(op) => write!(f, "{op}"),
            Self::Fragment(frag) => write!(f, "{frag}"),
        }
    }
}

impl fmt::Display for OperationDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.operation_type)?;
        if let Some(ref name) = self.name {
            write!(f, " {name}")?;
        }
        if !self.variable_definitions.is_empty() {
            write!(f, "(")?;
            for (idx, var) in self.variable_definitions.iter().enumerate() {
                if idx > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{var}")?;
            }
            write!(f, ")")?;
        }
        write!(f, " ")?;
        format_selection_set(&self.selection_set, f, 0)
    }
}

impl fmt::Display for VariableDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}: {}", self.name, self.type_name)?;
        if let Some(ref def) = self.default_value {
            write!(f, " = {def}")?;
        }
        Ok(())
    }
}

impl fmt::Display for FragmentDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fragment {} on {} ",
            self.name, self.type_condition
        )?;
        format_selection_set(&self.selection_set, f, 0)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Variable(name) => write!(f, "${name}"),
            Self::Int(val) => write!(f, "{val}"),
            Self::Float(val) => write!(f, "{val}"),
            Self::String(val) => {
                // Escape string contents
                write!(f, "\"")?;
                for ch in val.chars() {
                    match ch {
                        '"' => write!(f, "\\\"")?,
                        '\\' => write!(f, "\\\\")?,
                        '\n' => write!(f, "\\n")?,
                        '\r' => write!(f, "\\r")?,
                        '\t' => write!(f, "\\t")?,
                        other => write!(f, "{other}")?,
                    }
                }
                write!(f, "\"")
            }
            Self::Boolean(val) => write!(f, "{val}"),
            Self::Enum(val) => write!(f, "{val}"),
            Self::List(items) => {
                write!(f, "[")?;
                for (idx, item) in items.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Self::Object(fields) => {
                write!(f, "{{")?;
                for (idx, (k, v)) in fields.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

impl fmt::Display for Argument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.value)
    }
}

fn format_selection_set(
    set: &SelectionSet,
    f: &mut fmt::Formatter<'_>,
    indent: usize,
) -> fmt::Result {
    writeln!(f, "{{")?;
    let next_indent = indent + 2;
    for item in &set.items {
        write_indent(f, next_indent)?;
        match item {
            Selection::Field(field) => {
                if let Some(ref alias) = field.alias {
                    write!(f, "{alias}: ")?;
                }
                write!(f, "{}", field.name)?;
                if !field.arguments.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in field.arguments.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{arg}")?;
                    }
                    write!(f, ")")?;
                }
                if let Some(ref sub_set) = field.selection_set {
                    write!(f, " ")?;
                    format_selection_set(sub_set, f, next_indent)?;
                } else {
                    writeln!(f)?;
                }
            }
            Selection::FragmentSpread(name) => {
                writeln!(f, "...{name}")?;
            }
            Selection::InlineFragment(inline) => {
                write!(f, "...")?;
                if let Some(ref cond) = inline.type_condition {
                    write!(f, " on {cond}")?;
                }
                write!(f, " ")?;
                format_selection_set(&inline.selection_set, f, next_indent)?;
            }
        }
    }
    write_indent(f, indent)?;
    write!(f, "}}")
}

fn write_indent(f: &mut fmt::Formatter<'_>, count: usize) -> fmt::Result {
    for _ in 0..count {
        write!(f, " ")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_simple_query() {
        let doc = Document::query(
            Some("GetViewer"),
            SelectionSet::new().with_field("viewer"),
        );
        let text = doc.to_string();
        assert!(text.contains("query GetViewer {"));
        assert!(text.contains("  viewer"));
    }

    #[test]
    fn formats_query_with_variables_arguments_and_fragments() {
        let labels_frag = FragmentDefinition::new(
            "LabelFields",
            "Label",
            SelectionSet::new()
                .with_field("id")
                .with_field("name")
                .with_field("color")
                .with_field("description"),
        );

        let mut op = OperationDefinition::new(
            OperationType::Query,
            Some("RepoState"),
            SelectionSet::new().with(
                Field::new("repository")
                    .with_arg("owner", Value::Variable("owner".to_owned()))
                    .with_arg("name", Value::Variable("repo".to_owned()))
                    .with_selection_set(
                        SelectionSet::new()
                            .with_field("id")
                            .with_field("mergeCommitAllowed")
                            .with(
                                Field::new("labels")
                                    .with_arg("first", 100)
                                    .with_selection_set(
                                        SelectionSet::new().with(
                                            Field::new("nodes").with_selection_set(
                                                SelectionSet::new().with(
                                                    Selection::FragmentSpread(
                                                        "LabelFields".to_owned(),
                                                    ),
                                                ),
                                            ),
                                        ),
                                    ),
                            ),
                    ),
            ),
        );
        op.add_variable(VariableDefinition::new("owner", "String!"));
        op.add_variable(VariableDefinition::new("repo", "String!"));

        let doc = Document::new()
            .with_definition(op)
            .with_definition(labels_frag);

        let query_text = doc.to_string();
        assert!(query_text.contains("query RepoState($owner: String!, $repo: String!) {"));
        assert!(query_text.contains("repository(owner: $owner, name: $repo) {"));
        assert!(query_text.contains("...LabelFields"));
        assert!(query_text.contains("fragment LabelFields on Label {"));
        assert!(query_text.contains("color"));
    }

    #[test]
    fn formats_inline_fragment() {
        let inline = InlineFragment::on_type(
            "Issue",
            SelectionSet::new().with_field("number").with_field("title"),
        );
        let doc = Document::query(
            None,
            SelectionSet::new().with(Selection::InlineFragment(inline)),
        );
        let text = doc.to_string();
        assert!(text.contains("... on Issue {"));
        assert!(text.contains("number"));
        assert!(text.contains("title"));
    }
}
