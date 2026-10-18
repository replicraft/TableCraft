use super::{ConstraintViolation, FieldCheck, RuleCheckStatus, rules};
use crate::{
    InspectError, PropertyGroup, PropertyGroupKind, TableProperties, TableSnapshot, header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::{FieldConstraints, ScalarType, SchemaName};

pub(super) fn check_fields(
    table: &TableSnapshot,
    properties: &TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<FieldCheck>, InspectError> {
    budget.charge_allocation(table.columns().len() as u64, size_of::<FieldCheck>() as u64)?;
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(table.columns().len())
        .map_err(|_| InspectError::Allocation)?;
    for (position, column) in table.columns().iter().enumerate() {
        check(budget, cancelled)?;
        let group = properties.group(PropertyGroupKind::Column, column.name());
        let required = boolean(group, "Required");
        let empty = boolean(group, "AllowZeroLength");
        let mut empty_status = status(empty);
        if empty.is_some() && column.data_type() != ScalarType::Text {
            empty_status = RuleCheckStatus::Unsupported;
        }
        for row in table.rows() {
            check(budget, cancelled)?;
            let value = row.get(position).ok_or(InspectError::Malformed)?;
            // Absent/unqualified declarations do not silently turn into checked
            // rules. Each supported declaration is tested independently.
            if let Some(Ok(required)) = required {
                FieldConstraints::new(required, true)
                    .check(value)
                    .map_err(ConstraintViolation::from)?;
            }
            if let Some(Ok(allow)) = empty.filter(|_| column.data_type() == ScalarType::Text) {
                FieldConstraints::new(false, allow)
                    .check(value)
                    .map_err(ConstraintViolation::from)?;
            }
        }
        fields.push(FieldCheck {
            name: copy_name(column.name(), budget)?,
            required: status(required),
            empty_text: empty_status,
            validation: rules::audit(table, group, Some(position), budget, cancelled)?,
        });
    }
    Ok(fields)
}

// Keep missing declarations distinct from unexpected type payloads.
fn boolean(group: Option<&PropertyGroup>, name: &str) -> Option<Result<bool, ()>> {
    group
        .and_then(|group| group.get(name))
        .map(|property| property.boolean().map_err(|_| ()))
}
fn status(value: Option<Result<bool, ()>>) -> RuleCheckStatus {
    match value {
        None => RuleCheckStatus::NotDeclared,
        Some(Ok(_)) => RuleCheckStatus::Checked,
        Some(Err(())) => RuleCheckStatus::Unsupported,
    }
}
pub(super) fn copy_name(
    name: &str,
    budget: &mut ResourceBudget,
) -> Result<SchemaName, InspectError> {
    budget.charge_allocation(name.len() as u64, 1)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(name);
    Ok(SchemaName::new(owned)?)
}
