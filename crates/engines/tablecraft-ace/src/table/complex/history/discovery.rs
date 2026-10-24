//! Metadata-only declarations for explicitly requested stored-history views.
use super::{VersionHistoryMemberDeclaration, schema};
use crate::{
    InspectError,
    header::check,
    inspect_complex_schema,
    table::complex::{encoding::OBSERVED_HISTORY_EXT_FLAG, supported_key},
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_schema::{CollectionMember, Column, SchemaName};

/// Inspect stored Memo history members without reading parent/child records.
/// Names belong to the actual hidden field and checked flat member declarations.
/// This engineering view does not establish native Access SQL member syntax.
/// Unsupported histories remain unavailable; corrupt resolved mappings fail.
pub fn inspect_version_history_declarations<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<VersionHistoryMemberDeclaration>, InspectError> {
    let _span = tracing::debug_span!("native.complex.history.schema").entered();
    let complex = inspect_complex_schema(source, scope, table, budget, cancelled)?;
    if !matches!(
        complex.owner().header.read_profile()?,
        crate::ReadProfile::Ace2007 | crate::ReadProfile::Ace2016 | crate::ReadProfile::Ace2019
    ) {
        return Err(InspectError::Unsupported);
    }

    let mut members = Vec::new();
    for column in complex.columns() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, complex.owner().columns().len() as u64)?;
        let owner = complex
            .owner()
            .columns()
            .iter()
            .find(|field| field.number() == column.number())
            .ok_or(InspectError::Malformed)?;
        if !supported_key(owner) || owner.extended_flags() != OBSERVED_HISTORY_EXT_FLAG {
            continue;
        }
        let (value, _) = match schema::type_fields(column.type_table()) {
            Ok(fields) => fields,
            Err(InspectError::Unsupported) => continue,
            Err(error) => return Err(error),
        };
        let (position, memo) = match schema::select(&complex, value.name(), budget, cancelled) {
            Ok(binding) => binding,
            Err(InspectError::Unsupported) => continue,
            Err(error) => return Err(error),
        };
        if complex.owner().columns()[position].number() != column.number() {
            return Err(InspectError::Malformed);
        }
        let fields = match schema::qualify(column, column.flat_table()) {
            Ok(fields) => fields,
            Err(InspectError::Unsupported) => continue,
            Err(error) => return Err(error),
        };
        for position in [fields.value, fields.modified] {
            check(budget, cancelled)?;
            let flat = &column.flat_table().columns()[position];
            let kind = flat.scalar_type().ok_or(InspectError::Unsupported)?;
            budget.charge_allocation(1, size_of::<VersionHistoryMemberDeclaration>() as u64)?;
            members
                .try_reserve_exact(1)
                .map_err(|_| InspectError::Allocation)?;
            let collection = owned(owner.name(), budget)?;
            let name = owned(flat.name(), budget)?;
            let memo = SchemaName::new(owned(complex.owner().columns()[memo].name(), budget)?)?;
            let field = Column::with_type(name, flat.column_type(kind)?, true)?;
            members.push(VersionHistoryMemberDeclaration {
                member: CollectionMember::new(collection, field)?,
                memo,
                order: flat.text_sort_order(),
            });
        }
    }
    check(budget, cancelled)?;
    Ok(members)
}

fn owned(value: &str, budget: &mut ResourceBudget) -> Result<String, InspectError> {
    budget.charge_allocation(value.len() as u64, 1)?;
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| InspectError::Allocation)?;
    output.push_str(value);
    Ok(output)
}
