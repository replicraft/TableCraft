use super::RelationshipRules;
use crate::{NameComparison, SchemaError, SchemaName};

/// Ordered correspondence between a referenced and dependent field.
pub struct RelationshipColumn {
    parent: SchemaName,
    child: SchemaName,
}

impl RelationshipColumn {
    /// Pair two validated names; field existence belongs to catalog binding.
    pub fn new(parent: SchemaName, child: SchemaName) -> Self {
        Self { parent, child }
    }
    /// Referenced field name.
    pub fn parent(&self) -> &SchemaName {
        &self.parent
    }
    /// Dependent field name.
    pub fn child(&self) -> &SchemaName {
        &self.child
    }
}

/// Validated relationship shape, independent of native flags and storage layout.
/// Names may be unresolved until a catalog consumer binds them. No Debug exposes
/// identities; constructor validation does not imply any records satisfy the rule.
pub struct Relationship {
    name: SchemaName,
    parent: SchemaName,
    child: SchemaName,
    columns: Vec<RelationshipColumn>,
    rules: Option<RelationshipRules>,
}

impl Relationship {
    /// Validate a nonempty ordered mapping without repeated fields on either
    /// side. Callers must bound width before this quadratic duplicate check.
    /// None means unqualified rule semantics, never a disabled constraint.
    pub fn new(
        name: SchemaName,
        parent: SchemaName,
        child: SchemaName,
        columns: Vec<RelationshipColumn>,
        rules: Option<RelationshipRules>,
        names: NameComparison,
    ) -> Result<Self, SchemaError> {
        if columns.is_empty() {
            return Err(SchemaError::Width);
        }
        for (position, column) in columns.iter().enumerate() {
            if columns[..position].iter().any(|other| {
                names.equals(other.parent.as_str(), column.parent.as_str())
                    || names.equals(other.child.as_str(), column.child.as_str())
            }) {
                return Err(SchemaError::Duplicate);
            }
        }
        Ok(Self {
            name,
            parent,
            child,
            columns,
            rules,
        })
    }
    /// Relationship identity.
    pub fn name(&self) -> &SchemaName {
        &self.name
    }
    /// Referenced table identity.
    pub fn parent_table(&self) -> &SchemaName {
        &self.parent
    }
    /// Dependent table identity.
    pub fn child_table(&self) -> &SchemaName {
        &self.child
    }
    /// Field pairs in declared key order.
    pub fn columns(&self) -> &[RelationshipColumn] {
        &self.columns
    }
    /// Qualified declaration semantics, without claiming enforcement/validity.
    pub const fn rules(&self) -> Option<RelationshipRules> {
        self.rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn name(value: &str) -> SchemaName {
        SchemaName::new(value.into()).unwrap()
    }
    #[test]
    fn self_references_and_unknown_rules_are_retained_but_bad_shapes_fail() {
        let make = |pairs| {
            Relationship::new(
                name("Hierarchy"),
                name("Nodes"),
                name("Nodes"),
                pairs,
                None,
                NameComparison::AsciiInsensitive,
            )
        };
        assert!(matches!(make(vec![]), Err(SchemaError::Width)));
        let valid = make(vec![RelationshipColumn::new(name("ID"), name("ParentID"))]).unwrap();
        assert!(valid.rules().is_none());
        assert_eq!(valid.columns()[0].child().as_str(), "ParentID");
        assert!(matches!(
            make(vec![
                RelationshipColumn::new(name("ID"), name("A")),
                RelationshipColumn::new(name("id"), name("B"))
            ]),
            Err(SchemaError::Duplicate)
        ));
        assert!(matches!(
            make(vec![
                RelationshipColumn::new(name("A"), name("ID")),
                RelationshipColumn::new(name("B"), name("id"))
            ]),
            Err(SchemaError::Duplicate)
        ));
    }
}
