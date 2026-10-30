//! Presence-bearing INSERT intent; supplied NULL is never an omission marker.
use super::InsertableTable;
use tablecraft_core::{limits::ResourceBudget, values::Value};

/// Original per-column intent in the complete exposed backend schema order.
/// This enum carries no expression, native property or default evaluator.
pub enum InsertValue {
    /// An explicitly supplied stored carrier, including explicit NULL.
    Supplied(Value),
    /// No value was supplied; the backend decides its supported default/NULL law.
    Omitted,
}

/// Explicit opt-in beyond the unchanged complete-row insertion capability.
pub trait OmittingInsertableTable: InsertableTable {
    /// Consume complete-width presence intent once. A generated companion schema
    /// excludes only its backend-bound generated field, never a guessed NULL slot.
    /// No durable write or ambient clock is authorized. The backend resolves
    /// only needed defaults under its explicit policy.
    fn plan_insert_with_omissions(
        self,
        values: Vec<InsertValue>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error>;
}
