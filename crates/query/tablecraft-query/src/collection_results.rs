//! Structured query results preserve stored multivalue lists as lists.
use crate::QueryResult;
use std::{fmt, sync::Arc};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_db::OwnedRecordset;

/// One output cell in an explicitly collection-aware query result.
/// Collections are not scalar expression values and have no implicit spelling.
/// Equality is representational and does not define SQL collection equality.
// NO_CLONE: Recordset copies share immutable validated children across aliases
// and retained results; they never duplicate the owned child rows.
#[derive(Clone)]
pub enum QueryValue {
    /// An ordinary scalar or expression-only result state.
    Scalar(ExpressionValue),
    /// A complete parent-associated list of exact stored scalar values.
    Multivalue(MultivalueValue),
    /// No parent exists on this outer-joined side. This is neither a stored
    /// child NULL nor a present empty list, and has no scalar SQL coercion.
    AbsentMultivalue,
    /// Complete parent-associated child metadata, including explicitly uninspected members.
    Recordset(Arc<OwnedRecordset>),
    /// This outer-joined side has no parent, distinct from any present empty recordset.
    AbsentRecordset,
}

impl fmt::Debug for QueryValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scalar(value) => formatter.debug_tuple("Scalar").field(value).finish(),
            Self::Multivalue(value) => formatter.debug_tuple("Multivalue").field(value).finish(),
            Self::AbsentMultivalue => formatter.write_str("AbsentMultivalue"),
            Self::Recordset(value) => formatter
                .debug_struct("Recordset")
                .field("members", &value.schema().columns().len())
                .field("children", &value.len())
                .field("presence", &value.presence())
                .finish_non_exhaustive(),
            Self::AbsentRecordset => formatter.write_str("AbsentRecordset"),
        }
    }
}

impl PartialEq for QueryValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Scalar(left), Self::Scalar(right)) => left == right,
            (Self::Multivalue(left), Self::Multivalue(right)) => left == right,
            (Self::AbsentMultivalue, Self::AbsentMultivalue)
            | (Self::AbsentRecordset, Self::AbsentRecordset) => true,
            (Self::Recordset(left), Self::Recordset(right)) => {
                left.schema().columns() == right.schema().columns()
                    && left.schema().availability() == right.schema().availability()
                    && left.schema().names() == right.schema().names()
                    && left.presence() == right.presence()
                    && left.rows() == right.rows()
            }
            _ => false,
        }
    }
}

/// One complete stored list, preserving encounter order, duplicates and NULLs.
/// An empty list is distinct from a list containing one stored NULL. This type
/// always represents a present parent; `QueryValue::AbsentMultivalue` carries
/// outer-join absence separately. Native child identities are not exposed.
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
