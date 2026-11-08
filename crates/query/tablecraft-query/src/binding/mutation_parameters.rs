//! Bounded declaration admission shared by explicitly parameterized mutations.
use crate::{
    QueryError,
    profile::{MAX_PARAMETERS, SOURCE_SCHEMA_POLICY},
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::query::Parameter;

// Copy, constructor validation, schema-policy validation and scope preparation.
const PARAMETER_NAME_PASSES: usize = 4;

pub(super) fn admit(
    declarations: &[Parameter],
    mut budget: Option<&mut ResourceBudget>,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    if declarations.len() > MAX_PARAMETERS {
        return Err(QueryError::StructureLimit);
    }
    let mut preceding_name_bytes = 0_usize;
    for (position, declaration) in declarations.iter().enumerate() {
        if cancelled() {
            return Err(QueryError::Cancelled);
        }
        let name_bytes = declaration.name.as_str().len();
        if name_bytes > SOURCE_SCHEMA_POLICY.max_name_bytes {
            return Err(QueryError::StructureLimit);
        }
        // Prepay declaration validation and every prior-name duplicate
        // comparison. Retained binder bytes remain covered by MAX_BIND_BYTES.
        let work = name_bytes
            .checked_mul(position + PARAMETER_NAME_PASSES)
            .and_then(|work| work.checked_add(preceding_name_bytes))
            .and_then(|work| work.checked_add(declaration.data_type.name.len()))
            .and_then(|work| work.checked_add(declaration.data_type.arguments.len()))
            .and_then(|work| work.checked_add(1))
            .ok_or(QueryError::StructureLimit)?;
        if let Some(budget) = budget.as_deref_mut() {
            budget.charge(Resource::Steps, work as u64)?;
        }
        preceding_name_bytes = preceding_name_bytes
            .checked_add(name_bytes)
            .ok_or(QueryError::StructureLimit)?;
    }
    Ok(())
}
