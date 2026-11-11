//! Actual unadorned scalar declarations, separate from frozen Currency13 originals.
use super::{CreateColumn, CreateDatabaseError, column::Declaration, profile};
use crate::{InspectError, header::check, identifier};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// One ordinary scalar declaration with every optional field policy absent.
/// Capture retains data only, not native source or persistence authority.
pub struct UnadornedFieldAdditionDefinition {
    name: String,
    declaration: Declaration,
}

impl UnadornedFieldAdditionDefinition {
    /// Retain an explicit ordinary scalar field without adding gallery policies.
    ///
    /// # Errors
    /// Refuses unsupported types, annotations, invalid qualified ASCII names, allocation,
    /// cancellation and caller budget exhaustion.
    pub fn capture(
        column: CreateColumn<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreateDatabaseError> {
        check(budget, cancelled)?;
        if matches!(column.declaration(), Declaration::ShortBinary { .. })
            || column.required().is_some()
            || column.allow_zero_length().is_some()
            || column.default_literal().is_some()
            || column.description().is_some()
            || column.primary_key_name().is_some()
            || column.incrementing_autonumber()
            || column.unicode_compression()
        {
            return Err(CreateDatabaseError::Declaration);
        }
        let declaration = column.declaration();
        profile::bind(column.name(), declaration, 0)?;
        if !column.name().is_ascii() || !identifier::valid_name(column.name(), budget, cancelled)? {
            return Err(CreateDatabaseError::InvalidName);
        }
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
        Ok(Self { name, declaration })
    }

    /// Exact requested field spelling.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Original explicit field, retaining absent Required, AZL and default facts.
    pub fn column(&self) -> CreateColumn<'_> {
        CreateColumn::unadorned(&self.name, self.declaration)
    }

    pub(crate) fn field(
        &self,
        fixed_offset: usize,
    ) -> Result<profile::Field<'_>, CreateDatabaseError> {
        profile::bind(&self.name, self.declaration, fixed_offset)
    }
}
