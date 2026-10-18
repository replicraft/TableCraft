mod encoding;

use super::{TableFeatures, inspect_table_features, snapshot};
use crate::{InspectError, NativeColumn, format::data_type, header::check};
use std::io::{Read, Seek};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::{Column, ScalarType};

pub(super) use encoding::{decode, supported};

/// Explicit stored Int32 calculation results, without recalculation or freshness
/// guarantees. This is not an ordinary scalar query snapshot or an edit handle.
pub struct StoredInt32Calculation {
    features: TableFeatures,
    position: usize,
    values: Vec<Option<i32>>,
}
impl StoredInt32Calculation {
    /// Original inert declarations, including expression/result-type properties.
    pub fn features(&self) -> &TableFeatures {
        &self.features
    }
    /// Exact native calculated column whose storage was inspected.
    pub fn column(&self) -> &NativeColumn {
        &self.features.schema().columns()[self.position]
    }
    /// Stored values in sequential live-root order; ordinals are snapshot-local.
    /// NULL is returned only for absent storage or the qualified empty wrapper.
    pub fn values(&self) -> &[Option<i32>] {
        &self.values
    }
}

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
    let name = field.column().name();
    budget.charge_allocation(name.len() as u64, 1)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(name);
    let column = Column::new(owned, ScalarType::Int32, true)?;
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
