//! Bind actual original presence before any provider callback or candidate bytes.
use crate::table::mutation::insert_defaults::{
    clock, declaration,
    literal::{self, Bound},
    values::copy_value,
};
use crate::{
    EditError, InsertFieldValue, InspectError, NativeImage, OmittedRowInsert, RowInsert,
    TableSchema, WriteBlocker, header::check, inspect_table_properties, limits::MAX_LIVE_ROWS,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{LegacyDateTime, Value},
};
use tablecraft_expressions::functions::dates::clock::ClockReading;

const ZERO_DATE_BITS: u64 = 0;

pub(super) struct BoundRows {
    rows: Vec<Vec<Bound>>,
    needed: bool,
}

impl BoundRows {
    pub(super) fn needs_clock(&self) -> bool {
        self.needed
    }

    /// Width/carrier validation only; these rows are never staged or retained.
    pub(super) fn validation_rows(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<RowInsert>, EditError> {
        let mut rows = vector(self.rows.len(), budget, cancelled)?;
        for row in &self.rows {
            check(budget, cancelled)?;
            let mut values = vector(row.len(), budget, cancelled)?;
            for value in row {
                check(budget, cancelled)?;
                values.push(match value {
                    Bound::Literal(value) => copy_value(value, budget, cancelled)?,
                    Bound::Clock(_) => {
                        Value::DateTime(LegacyDateTime::from_bits(ZERO_DATE_BITS).unwrap())
                    }
                });
            }
            rows.push(RowInsert::new(values));
        }
        Ok(rows)
    }

    pub(super) fn resolve(
        self,
        reading: Option<ClockReading>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<RowInsert>, EditError> {
        if self.needed != reading.is_some() {
            return Err(EditError::Unsupported(WriteBlocker::Properties));
        }
        let mut rows = vector(self.rows.len(), budget, cancelled)?;
        for row in self.rows {
            check(budget, cancelled)?;
            let mut values = vector(row.len(), budget, cancelled)?;
            for value in row {
                check(budget, cancelled)?;
                values.push(match value {
                    Bound::Literal(value) => value,
                    Bound::Clock(function) => clock::project(
                        function,
                        reading.ok_or(EditError::Preservation)?,
                        budget,
                        cancelled,
                    )?,
                });
            }
            rows.push(RowInsert::new(values));
        }
        check(budget, cancelled)?;
        Ok(rows)
    }
}

pub(super) fn capture(
    image: &NativeImage,
    schema: &TableSchema,
    requests: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<BoundRows, EditError> {
    check(budget, cancelled)?;
    if requests.len() > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }
    let mut omitted = false;
    for request in requests {
        check(budget, cancelled)?;
        if request.values().len() != schema.columns().len() {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        for value in request.values() {
            check(budget, cancelled)?;
            omitted |= matches!(value, InsertFieldValue::Omitted);
        }
    }
    let properties = if omitted {
        let properties = inspect_table_properties(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            schema.name(),
            budget,
            cancelled,
        )?;
        if properties.object_id() != schema.object_id() || properties.name() != schema.name() {
            return Err(EditError::Preservation);
        }
        super::qualify_properties(schema, &properties, budget, cancelled)?;
        Some(properties)
    } else {
        None
    };
    let mut needed = false;
    let mut rows = vector(requests.len(), budget, cancelled)?;
    for request in requests {
        check(budget, cancelled)?;
        let mut row = vector(schema.columns().len(), budget, cancelled)?;
        for (column, original) in schema.columns().iter().zip(request.values()) {
            check(budget, cancelled)?;
            let bound = match original {
                InsertFieldValue::Supplied(value) => {
                    Bound::Literal(copy_value(value, budget, cancelled)?)
                }
                InsertFieldValue::Omitted => {
                    match declaration::capture(
                        column,
                        properties.as_ref().ok_or(EditError::Preservation)?,
                        budget,
                        cancelled,
                    )? {
                        Some(property) => {
                            literal::bind_clock(column, property.text()?, budget, cancelled)?
                        }
                        None => Bound::Literal(Value::Null),
                    }
                }
            };
            needed |= matches!(bound, Bound::Clock(_));
            row.push(bound);
        }
        rows.push(row);
    }
    check(budget, cancelled)?;
    Ok(BoundRows { rows, needed })
}

fn vector<T>(
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<T>, EditError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(result)
}
