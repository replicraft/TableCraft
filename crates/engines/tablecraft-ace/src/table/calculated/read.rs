use super::super::{inspect_table_features, snapshot};
use super::{StoredInt32Calculation, encoding::supported};
use crate::{InspectError, format::data_type, header::check};
use std::io::{Read, Seek};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

/// Inspect stored calculation results on an explicit stable/offline source.
/// Only code-5/code-6 Int32 variable calculated wrappers with explicit matching
/// ResultType and nonempty Expression declarations are qualified. No expression,
/// VBA, provider or active content runs; values can be stale. Failure returns no
/// partial snapshot, including for malformed/unknown wrappers and empty tables.
pub fn read_stored_int32_calculation<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<StoredInt32Calculation, InspectError> {
    let _span = tracing::debug_span!("native.calculation.stored").entered();
    check(budget, cancelled)?;
    let features = inspect_table_features(source, scope, table, budget, cancelled)?;
    let position = features
        .schema()
        .columns()
        .iter()
        .position(|field| field.name().eq_ignore_ascii_case(column))
        .ok_or(InspectError::UnknownColumn)?;
    let field = features
        .columns()
        .nth(position)
        .ok_or(InspectError::Malformed)?;
    if !supported(field.column())
        || field.result_native_type()? != Some(data_type::INT32)
        || field.expression()?.is_none_or(str::is_empty)
    {
        return Err(InspectError::Unsupported);
    }
    let column = snapshot::projected_column(field.column(), ScalarType::Int32, budget)?;
    let rows = snapshot::read_records(
        source,
        features.schema(),
        &[column],
        Some(&[position]),
        budget,
        cancelled,
    )?;
    budget.charge_allocation(rows.len() as u64, size_of::<Option<i32>>() as u64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(rows.len())
        .map_err(|_| InspectError::Allocation)?;
    for row in rows {
        check(budget, cancelled)?;
        values.push(match row.as_slice() {
            [Value::Int32(value)] => Some(*value),
            [Value::Null] => None,
            _ => return Err(InspectError::Malformed),
        });
    }
    check(budget, cancelled)?;
    Ok(StoredInt32Calculation {
        features,
        position,
        values,
    })
}
