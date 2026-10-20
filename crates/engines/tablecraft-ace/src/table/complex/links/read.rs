use super::super::{ComplexTableSchema, inspect_complex_schema, supported_key};
use super::{
    ComplexReferenceViolation,
    metadata::{ComplexColumnLinks, ComplexParentLink},
};
use crate::{InspectError, header::check, read_table, table::snapshot};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

/// Resolve one complex column and read its parent keys and raw flat children on
/// a stable/offline source. Assigned keys/child identities must be positive and
/// unique in their own scopes; absent keys retain NULL and empty membership.
/// Rejects orphan/ambiguous/NULL child references without partial publication.
/// No expressions, scripts, providers or attachment decompression execute.
pub fn read_complex_column_links<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ComplexColumnLinks, InspectError> {
    let _span = tracing::debug_span!("native.complex.links").entered();
    check(budget, cancelled)?;
    let schema = inspect_complex_schema(source, scope, table, budget, cancelled)?;
    let field_position = schema
        .owner()
        .columns()
        .iter()
        .position(|field| field.name().eq_ignore_ascii_case(column))
        .ok_or(InspectError::UnknownColumn)?;
    read_resolved(source, scope, schema, field_position, budget, cancelled)
}

pub(in crate::table::complex) fn read_resolved<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    schema: ComplexTableSchema,
    field_position: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ComplexColumnLinks, InspectError> {
    check(budget, cancelled)?;
    let field = schema
        .owner()
        .columns()
        .get(field_position)
        .ok_or(InspectError::Malformed)?;
    if !supported_key(field) {
        return Err(InspectError::Unsupported);
    }
    let position = schema
        .columns()
        .iter()
        .position(|complex| complex.number() == field.number())
        .ok_or(InspectError::Malformed)?;
    let carrier = snapshot::projected_column(field, ScalarType::Int32, budget)?;
    let rows = snapshot::read_records(
        source,
        schema.owner(),
        &[carrier],
        Some(&[field_position]),
        budget,
        cancelled,
    )?;
    let mut parents = reserved::<ComplexParentLink>(rows.len(), budget)?;
    let mut keys = reserved::<(i32, usize)>(rows.len(), budget)?;
    for (ordinal, row) in rows.into_iter().enumerate() {
        check(budget, cancelled)?;
        let key = match row.as_slice() {
            [Value::Int32(key)] if *key > 0 => {
                keys.push((*key, ordinal));
                Some(*key)
            }
            [Value::Int32(_)] => return Err(InspectError::Unsupported),
            [Value::Null] => None,
            _ => return Err(InspectError::Malformed),
        };
        parents.push(ComplexParentLink { key, range: 0..0 });
    }
    sort(&mut keys, budget, cancelled)?;
    for pair in keys.windows(2) {
        check(budget, cancelled)?;
        if pair[0].0 == pair[1].0 {
            return Err(ComplexReferenceViolation::DuplicateParent.into());
        }
    }
    let flat = schema.columns()[position].flat_table();
    let children = read_table(source, scope, flat.name(), budget, cancelled)?;
    if children.schema().object_id() != flat.object_id()
        || children.schema().definition_page() != flat.definition_page()
        || children.schema().declared_rows() != flat.declared_rows()
        || children.columns() != flat.scalar_columns_with_budget(budget)?
    {
        return Err(InspectError::Malformed);
    }
    let number_position = |number| {
        children
            .schema()
            .columns()
            .iter()
            .position(|field| field.number() == number)
            .ok_or(InspectError::Malformed)
    };
    let foreign_key = number_position(schema.columns()[position].foreign_key_number())?;
    let identity = number_position(schema.columns()[position].identity_number())?;
    let mut identities = reserved::<i32>(children.rows().len(), budget)?;
    let mut order = reserved::<(usize, usize)>(children.rows().len(), budget)?;
    let search_steps = u64::from(keys.len().checked_ilog2().unwrap_or(0)) + 1;
    for (ordinal, row) in children.rows().iter().enumerate() {
        check(budget, cancelled)?;
        let key = child_key(row.get(foreign_key))?;
        identities.push(child_key(row.get(identity))?);
        budget.charge(Resource::Steps, search_steps)?;
        let parent = keys
            .binary_search_by_key(&key, |(key, _)| *key)
            .map_err(|_| ComplexReferenceViolation::MissingParent)?;
        order.push((keys[parent].1, ordinal));
    }
    sort(&mut identities, budget, cancelled)?;
    for pair in identities.windows(2) {
        check(budget, cancelled)?;
        if pair[0] == pair[1] {
            return Err(ComplexReferenceViolation::DuplicateChild.into());
        }
    }
    sort(&mut order, budget, cancelled)?;
    let mut end = 0;
    for (ordinal, parent) in parents.iter_mut().enumerate() {
        check(budget, cancelled)?;
        let start = end;
        while order.get(end).is_some_and(|(owner, _)| *owner == ordinal) {
            check(budget, cancelled)?;
            end += 1;
        }
        parent.range = start..end;
    }
    if end != order.len() {
        return Err(InspectError::Malformed);
    }
    check(budget, cancelled)?;
    Ok(ComplexColumnLinks {
        schema,
        position,
        parents,
        children,
        order,
    })
}

fn child_key(value: Option<&Value>) -> Result<i32, InspectError> {
    match value {
        Some(Value::Int32(key)) if *key > 0 => Ok(*key),
        Some(Value::Int32(_)) => Err(InspectError::Unsupported),
        Some(Value::Null) => Err(ComplexReferenceViolation::NullChild.into()),
        _ => Err(InspectError::Malformed),
    }
}
fn reserved<T>(count: usize, budget: &mut ResourceBudget) -> Result<Vec<T>, InspectError> {
    budget.charge_allocation(count as u64, size_of::<T>() as u64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(values)
}
fn sort<T: Ord>(
    values: &mut [T],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    check(budget, cancelled)?;
    let levels = u64::from(values.len().checked_ilog2().unwrap_or(0)) + 1;
    budget.charge(
        Resource::Steps,
        values.len() as u64 * levels * size_of::<T>() as u64,
    )?;
    values.sort_unstable();
    check(budget, cancelled)
}
