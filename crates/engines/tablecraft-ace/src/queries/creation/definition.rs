//! Captured constant SELECT metadata, never SQL or mutation authority.
use super::QueryCreationError;
use crate::{InspectError, header::check, identifier};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::LegacyDateTime,
};

/// Owned native constant-SELECT request with an explicit creation time.
///
/// This first declaration represents one signed LONG constant and an explicit
/// result alias. It does not evaluate SQL or qualify a native source.
pub struct QueryCreationDefinition {
    name: String,
    alias: String,
    value: i32,
    captured_at: LegacyDateTime,
}

impl QueryCreationDefinition {
    /// Capture native names, a constant and exact caller-supplied DATE bits.
    ///
    /// Both catalog creation/update dates use this captured fact. The engine
    /// never reads a clock or derives it from an existing object's timestamps.
    ///
    /// # Errors
    /// Refuses invalid native names, cancellation, allocation and caller limits.
    pub fn capture(
        destination: &str,
        alias: &str,
        value: i32,
        captured_at: LegacyDateTime,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, QueryCreationError> {
        check(budget, cancelled)?;
        if !identifier::valid_name(destination, budget, cancelled)?
            || !identifier::valid_name(alias, budget, cancelled)?
        {
            return Err(QueryCreationError::Definition);
        }

        let name = copy_text(destination, budget, cancelled)?;
        let alias = copy_text(alias, budget, cancelled)?;
        Ok(Self {
            name,
            alias,
            value,
            captured_at,
        })
    }

    /// Exact destination object spelling.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Explicit result-column alias; separate from the expression body.
    pub fn alias(&self) -> &str {
        &self.alias
    }

    /// Captured signed LONG literal; never a computed result.
    pub const fn value(&self) -> i32 {
        self.value
    }

    /// Exact native DATE fact retained for both new catalog date fields.
    pub const fn captured_at(&self) -> LegacyDateTime {
        self.captured_at
    }
}

fn copy_text(
    value: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, QueryCreationError> {
    check(budget, cancelled)?;
    budget.charge_allocation(value.len() as u64, 1)?;
    budget.charge(Resource::Steps, value.len() as u64)?;
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| InspectError::Allocation)?;
    result.push_str(value);
    Ok(result)
}
