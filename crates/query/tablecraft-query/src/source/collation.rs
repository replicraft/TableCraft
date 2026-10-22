//! Opaque host policy identity, independent of any native locale encoding.
use crate::QueryError;
use tablecraft_schema::{Column, ScalarType};

/// Host-issued text policy identity within one stable Source policy namespace.
///
/// Query code only retains and compares identity. It does not decode locale,
/// case/accent rules, native flags or choose a comparer from this value. A host
/// composing providers must remap identities into a consistent namespace; equal
/// keys must describe the same policy. Keys are not portable database identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextCollationId(u64);
impl TextCollationId {
    /// Retain an opaque caller-issued policy key. Every bit pattern is valid.
    pub const fn from_host(key: u64) -> Self {
        Self(key)
    }
    /// Recover the host key for resolution within the issuing policy namespace.
    pub const fn host_key(self) -> u64 {
        self.0
    }
}

pub(crate) fn validate(
    columns: &[Column],
    policies: &[Option<TextCollationId>],
) -> Result<(), QueryError> {
    validate_types(columns.iter().map(Column::data_type), policies)
}

pub(crate) fn validate_types(
    types: impl ExactSizeIterator<Item = ScalarType>,
    policies: &[Option<TextCollationId>],
) -> Result<(), QueryError> {
    if policies.is_empty() {
        return Ok(());
    }
    if types.len() != policies.len()
        || types
            .zip(policies)
            .any(|(column, policy)| policy.is_some() && column != ScalarType::Text)
    {
        return Err(QueryError::Source);
    }
    Ok(())
}

/// Static text-policy provenance of a bound expression or output column.
/// This records lineage, not a result type, comparer selection or native precedence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextPolicy {
    /// No explicit field policy is recorded at this boundary. Runtime-produced
    /// cells may enrich static Default provenance before choosing a comparer.
    Default,
    /// One field policy contributes, possibly through aliases/expressions.
    Column(TextCollationId),
    /// Distinct field identities contribute and require explicit resolution.
    Conflict,
}
impl TextPolicy {
    /// Combine contributing field provenance without guessing precedence.
    pub const fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Default, value) | (value, Self::Default) => value,
            (Self::Column(a), Self::Column(b)) if a.0 == b.0 => Self::Column(a),
            _ => Self::Conflict,
        }
    }
}

/// Explicit host mapping from stable source policy identities to comparers.
/// Providers must enforce cumulative work/cancellation bounds, preserve the
/// issuing namespace and fail unknown identities without a default fallback.
/// This callback performs no source discovery on behalf of the query layer.
pub trait CollationResolver {
    /// Borrow the comparer for exactly this host-issued identity.
    fn resolve(
        &self,
        policy: TextCollationId,
    ) -> Result<&dyn tablecraft_expressions::comparison::Collation, QueryError>;
}
