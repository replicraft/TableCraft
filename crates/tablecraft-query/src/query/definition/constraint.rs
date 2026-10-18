use crate::select::Name;
/// Inline or table-level constraint syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    /// Explicit CONSTRAINT name, when supplied.
    pub name: Option<Name>,
    /// Restriction or relationship.
    pub kind: ConstraintKind,
}
/// Constraint kind; empty field lists refer to the enclosing inline column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintKind {
    /// PRIMARY KEY.
    PrimaryKey(Vec<Name>),
    /// UNIQUE.
    Unique(Vec<Name>),
    /// Named NOT NULL.
    NotNull(Vec<Name>),
    /// REFERENCES / FOREIGN KEY relationship.
    ForeignKey(Reference),
}
/// Foreign-key syntax; referenced fields may be omitted to select the primary key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// Local fields, empty for an inline reference.
    pub fields: Vec<Name>,
    /// FOREIGN KEY NO INDEX flag.
    pub no_index: bool,
    /// Referenced table.
    pub table: Name,
    /// Referenced fields in written order, or empty for the primary key.
    pub foreign_fields: Vec<Name>,
    /// ON UPDATE action.
    pub on_update: Option<ReferentialAction>,
    /// ON DELETE action.
    pub on_delete: Option<ReferentialAction>,
}
/// Access relationship actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferentialAction {
    /// CASCADE.
    Cascade,
    /// SET NULL.
    SetNull,
}
