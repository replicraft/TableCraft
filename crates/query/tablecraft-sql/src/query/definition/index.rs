use crate::select::Name;
/// CREATE INDEX and its ordered fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateIndex {
    /// Index name.
    pub name: Name,
    /// Owning table.
    pub table: Name,
    /// Explicit UNIQUE flag.
    pub unique: bool,
    /// Indexed fields with direction.
    pub fields: Vec<IndexField>,
    /// Optional WITH policy.
    pub policy: Option<IndexPolicy>,
}
/// Index field syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexField {
    /// Field name.
    pub name: Name,
    /// DESC when true; implicit/explicit ASC otherwise.
    pub descending: bool,
}
/// Mutually exclusive CREATE INDEX WITH policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexPolicy {
    /// PRIMARY.
    Primary,
    /// DISALLOW NULL.
    DisallowNull,
    /// IGNORE NULL.
    IgnoreNull,
}
