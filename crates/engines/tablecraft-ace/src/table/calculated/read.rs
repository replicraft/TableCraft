use super::super::{inspect_table_features, snapshot};
use super::{StoredCalculation, StoredInt32Calculation, encoding};
use crate::{InspectError, format::data_type, header::check};
use std::io::{Read, Seek};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

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
    let StoredCalculation {
        features,
        position,
        values,
        ..
    } = read_values(
        source,
        scope,
        table,
        column,
        Some(data_type::INT32),
        budget,
        cancelled,
    )?;
    budget.charge_allocation(values.len() as u64, size_of::<Option<i32>>() as u64)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(values.len())
        .map_err(|_| InspectError::Allocation)?;
    for value in values {
        check(budget, cancelled)?;
        output.push(match value {
            Value::Null => None,
            Value::Int32(value) => Some(value),
            _ => return Err(InspectError::Malformed),
        });
    }
    check(budget, cancelled)?;
    Ok(StoredInt32Calculation {
        features,
        position,
        values: output,
    })
}

/// Read qualified ACE 16-family stored Boolean, Int32, Double, Currency,
/// legacy DateTime or Short Text results, with exact matching typed ResultType
/// and nonempty Expression declarations. No formula or active content executes;
/// values may be stale. Failure returns no partial result, including empty-table
/// declaration failures. Ordinary scalar query eligibility is unchanged.
pub fn read_stored_calculation<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<StoredCalculation, InspectError> {
    read_values(source, scope, table, column, None, budget, cancelled)
}

fn read_values<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    expected: Option<u8>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<StoredCalculation, InspectError> {
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
    let kind = encoding::kind(field.column()).ok_or(InspectError::Unsupported)?;
    if expected.is_some_and(|expected| expected != field.column().native_type())
        || field.result_native_type()? != Some(field.column().native_type())
        || field.expression()?.is_none_or(str::is_empty)
    {
        return Err(InspectError::Unsupported);
    }
    let column = snapshot::projected_column(field.column(), kind, budget)?;
    let rows = snapshot::read_records(
        source,
        features.schema(),
        &[column],
        Some(&[position]),
        budget,
        cancelled,
    )?;
    budget.charge_allocation(rows.len() as u64, size_of::<Value>() as u64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(rows.len())
        .map_err(|_| InspectError::Allocation)?;
    for mut row in rows {
        check(budget, cancelled)?;
        if row.len() != 1 {
            return Err(InspectError::Malformed);
        }
        values.push(row.pop().ok_or(InspectError::Malformed)?);
    }
    check(budget, cancelled)?;
    Ok(StoredCalculation {
        features,
        position,
        kind,
        values,
    })
}
