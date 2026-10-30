//! Qualified generated-column binding for an explicitly selected increment policy.
use super::{IncrementingAutoNumber, counter};
use crate::{
    EditError, InspectError, NativeImage,
    header::check,
    table::mutation::{gates, preview},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

#[cfg(test)]
mod tests;

/// Captured LAST precondition and actual generated field, not source identity or
/// stored-mode qualification. Native layout and flags remain engine-owned.
pub struct Ace16IncrementingInsertBinding {
    policy: IncrementingAutoNumber,
    position: usize,
    name: String,
}

impl Ace16IncrementingInsertBinding {
    /// Original LAST counter under explicitly selected increment-by-one behavior.
    pub const fn policy(&self) -> IncrementingAutoNumber {
        self.policy
    }
    /// Declaration position to omit from complete native-order companion values.
    pub const fn generated_position(&self) -> usize {
        self.position
    }
    /// Actual generated field spelling, independent of its name or physical slot.
    pub fn generated_name(&self) -> &str {
        &self.name
    }
}

impl NativeImage {
    /// Bind the actual generated field and LAST in this captured image under an
    /// explicitly selected increment policy. All source/property gates apply;
    /// no live MAX or raw stored Increment/Random mode is inferred. The existing
    /// INSERT preview revalidates the returned policy and complete companion row.
    pub fn prepare_ace16_incrementing_insert_binding(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16IncrementingInsertBinding, EditError> {
        let _span = tracing::debug_span!("native.insert.increment_binding").entered();
        check(budget, cancelled)?;
        preview::qualify(self)?;
        let checked = gates::before_incrementing_insert(self, table, budget, cancelled)?;
        let source = counter::source(self, &checked, budget, cancelled)?;
        let column = checked
            .schema()
            .columns()
            .get(source.column)
            .ok_or(InspectError::Malformed)?;
        budget
            .charge_allocation(column.name().len() as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, column.name().len() as u64)
            .map_err(InspectError::from)?;
        let mut name = String::new();
        name.try_reserve_exact(column.name().len())
            .map_err(|_| InspectError::Allocation)?;
        name.push_str(column.name());
        check(budget, cancelled)?;
        Ok(Ace16IncrementingInsertBinding {
            policy: IncrementingAutoNumber::new(source.last),
            position: source.column,
            name,
        })
    }
}
