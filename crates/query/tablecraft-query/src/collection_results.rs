//! Structured query results preserve stored multivalue lists as lists.
use crate::QueryResult;
use tablecraft_core::values::{ExpressionValue, Value};

/// One output cell in an explicitly collection-aware query result.
/// Collections are not scalar expression values and have no implicit spelling.
/// Equality is representational and does not define SQL collection equality.
#[derive(Clone, Debug, PartialEq)]
pub enum QueryValue {
    /// An ordinary scalar or expression-only result state.
    Scalar(ExpressionValue),
    /// A complete parent-associated list of exact stored scalar values.
    Multivalue(MultivalueValue),
}

/// One complete stored list, preserving encounter order, duplicates and NULLs.
/// An empty list is distinct from a list containing one stored NULL. This type
/// does not invent an absent-parent marker or expose native child identities.
#[derive(Clone, Debug, PartialEq)]
pub struct MultivalueValue {
    pub(crate) values: Vec<Value>,
}

impl MultivalueValue {
    /// Borrow the complete ordered stored values without coercion.
    pub fn values(&self) -> &[Value] {
        &self.values
    }

    /// Number of stored children, including duplicate and NULL entries.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the parent has no stored children.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Buffered scalar and structured collection outputs from one explicit query.
pub type CollectionQueryResult = QueryResult<QueryValue>;
