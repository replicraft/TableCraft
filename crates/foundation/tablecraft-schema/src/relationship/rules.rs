/// Declared referential action; this metadata does not execute a cascade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferentialAction {
    /// Reject a referenced-key change that would leave dependent records.
    Restrict,
    /// Propagate a referenced-key update or delete dependent records.
    Cascade,
    /// Set dependent keys to NULL, subject to field constraints.
    SetNull,
}

/// Backend-neutral relationship declarations with explicitly known semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelationshipRules {
    /// Whether referential integrity is declared enforced.
    pub enforced: bool,
    /// Whether the declaration is one-to-one rather than one-to-many.
    pub one_to_one: bool,
    /// Declared behavior for referenced-key updates.
    pub on_update: ReferentialAction,
    /// Declared behavior for referenced-row deletion.
    pub on_delete: ReferentialAction,
}
