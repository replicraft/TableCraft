use thiserror::Error;

/// Schema rejection, without names or value contents.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SchemaError {
    /// Empty/control-containing name, or a name outside an explicit profile.
    #[error("invalid schema name")]
    Name,
    /// Repeated column names under the selected comparison policy.
    #[error("duplicate schema column")]
    Duplicate,
    /// Empty schema or width outside an explicit profile.
    #[error("invalid schema width")]
    Width,
    /// Invalid declared type constraint.
    #[error("invalid schema type constraint")]
    TypeConstraint,
    /// Value violates the declared carrier or constraints.
    #[error("value does not match declared type")]
    Value,
    /// Row does not match column count, declared types or nullability.
    #[error("row does not match schema")]
    Row,
}
