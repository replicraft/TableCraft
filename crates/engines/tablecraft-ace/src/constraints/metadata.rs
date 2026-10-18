use crate::NativeRelationship;
use tablecraft_schema::SchemaName;

/// Coverage of one declarative field/table rule in an explicit audit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleCheckStatus {
    /// No nonempty declaration was present; no rule was invented.
    NotDeclared,
    /// Declared rule checked against every applicable record.
    Checked,
    /// A declaration exists but its semantics are unqualified.
    Unsupported,
}

/// Field identity and individual rule coverage; no value payloads are retained.
pub struct FieldCheck {
    pub(super) name: SchemaName,
    pub(super) required: RuleCheckStatus,
    pub(super) empty_text: RuleCheckStatus,
    pub(super) validation: RuleCheckStatus,
}
impl FieldCheck {
    /// Original field name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    /// Coverage of the explicit Required declaration.
    pub const fn required(&self) -> RuleCheckStatus {
        self.required
    }
    /// Coverage of the explicit AllowZeroLength declaration.
    pub const fn empty_text(&self) -> RuleCheckStatus {
        self.empty_text
    }
    /// Coverage of the explicit ValidationRule expression.
    pub const fn validation(&self) -> RuleCheckStatus {
        self.validation
    }
}

/// Record-integrity coverage for one associated native relationship.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationshipCheckStatus {
    /// Qualified one-to-ten-column integer tuple uniqueness and membership checked.
    Checked,
    /// Qualified declaration explicitly disables referential integrity.
    NotEnforced,
    /// Rule flags, endpoints, key shape/carriers or parent unique index unqualified.
    Unsupported,
}

/// Declaration plus actual record-check coverage.
pub struct RelationshipCheck {
    pub(super) relationship: NativeRelationship,
    pub(super) status: RelationshipCheckStatus,
}
impl RelationshipCheck {
    /// Original native declaration.
    pub fn relationship(&self) -> &NativeRelationship {
        &self.relationship
    }
    /// Whether record integrity was checked or explicitly left unqualified.
    pub const fn status(&self) -> RelationshipCheckStatus {
        self.status
    }
}

/// Completed field/relationship audit. It does not qualify skipped rules, index
/// structure, macros, writes or native shared-file isolation.
pub struct ConstraintReport {
    pub(super) name: SchemaName,
    pub(super) rows: usize,
    pub(super) fields: Vec<FieldCheck>,
    pub(super) table_validation: RuleCheckStatus,
    pub(super) relationships: Vec<RelationshipCheck>,
}
impl ConstraintReport {
    /// Original checked table name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    /// Complete scalar rows checked in the explicit table.
    pub const fn rows(&self) -> usize {
        self.rows
    }
    /// Per-field rule declarations and coverage.
    pub fn fields(&self) -> &[FieldCheck] {
        &self.fields
    }
    /// Table-level ValidationRule coverage.
    pub const fn table_validation(&self) -> RuleCheckStatus {
        self.table_validation
    }
    /// Relationships touching the table and their actual record coverage.
    pub fn relationships(&self) -> &[RelationshipCheck] {
        &self.relationships
    }
}
