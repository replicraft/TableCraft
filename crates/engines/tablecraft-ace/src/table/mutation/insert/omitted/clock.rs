//! Bind all needed defaults before authorizing one operation clock callback.
mod preview;

use super::{
    InsertFieldValue, Original, Prepared, ResolvedRow, constant::Bound, declaration, reserve,
};
use crate::table::mutation::insert::{
    autonumber, default_clock, primary_column, record, values::copy_value,
};
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker, header::check,
    inspect_table_properties, limits::MAX_LIVE_ROWS,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::functions::dates::clock::ClockReading;

pub(in crate::table::mutation::insert) struct BoundRows<'a> {
    rows: Vec<Vec<Bound>>,
    original: Original<'a>,
    needed: bool,
}

impl<'a> BoundRows<'a> {
    pub(in crate::table::mutation::insert) fn needs_clock(&self) -> bool {
        self.needed
    }

    pub(in crate::table::mutation::insert) fn resolve(
        self,
        reading: Option<ClockReading>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Prepared<'a>, EditError> {
        if self.needed != reading.is_some() {
            return Err(EditError::Preservation);
        }
        let mut rows = Vec::new();
        reserve(&mut rows, self.rows.len(), budget)?;
        for bound in self.rows {
            check(budget, cancelled)?;
            let mut values = Vec::new();
            reserve(&mut values, bound.len(), budget)?;
            for value in bound {
                check(budget, cancelled)?;
                values.push(match value {
                    Bound::Literal(value) => value,
                    Bound::Clock(function) => default_clock::project(
                        function,
                        reading.ok_or(EditError::Preservation)?,
                        budget,
                        cancelled,
                    )?,
                });
            }
            rows.push(ResolvedRow { values });
        }
        check(budget, cancelled)?;
        Ok(Prepared {
            rows,
            original: self.original,
        })
    }
}

pub(in crate::table::mutation::insert) fn bind<'a>(
    image: &NativeImage,
    checked: &TableCheck,
    original: Original<'a>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<BoundRows<'a>, EditError> {
    check(budget, cancelled)?;
    if original.requests().len() > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }
    let schema = checked.schema();
    let generated = match original {
        Original::Ordinary(_) => None,
        Original::Incrementing { policy, .. } => Some(autonumber::counter::bind(
            image, checked, policy, budget, cancelled,
        )?),
    };
    let excluded = generated.map(|source| source.column);
    let width = schema.columns().len() - usize::from(excluded.is_some());
    let mut omitted = false;
    for request in original.requests() {
        check(budget, cancelled)?;
        if request.values().len() != width {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        for value in request.values() {
            check(budget, cancelled)?;
            omitted |= matches!(value, InsertFieldValue::Omitted);
        }
    }

    // Reuse the exact generation range owner before any host read. Its output
    // is only a preflight here; final staging and audit bind the original again.
    if let Some(source) = generated {
        autonumber::counter::prepare(source, original.requests().len(), budget, cancelled)?;
        record::Profile::bind_incrementing(schema, source.column, budget, cancelled)?;
    } else {
        primary_column::resolve(checked, budget, cancelled)?;
        record::Profile::bind(schema, budget, cancelled)?;
    }
    let properties = if omitted {
        Some(inspect_table_properties(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            schema.name(),
            budget,
            cancelled,
        )?)
    } else {
        None
    };

    let mut needed = false;
    let mut rows = Vec::new();
    reserve(&mut rows, original.requests().len(), budget)?;
    for request in original.requests() {
        check(budget, cancelled)?;
        let mut row = Vec::new();
        reserve(&mut row, width, budget)?;
        let columns = schema
            .columns()
            .iter()
            .enumerate()
            .filter(|(position, _)| Some(*position) != excluded);
        for ((_, column), intent) in columns.zip(request.values()) {
            check(budget, cancelled)?;
            let value = match intent {
                InsertFieldValue::Supplied(value) => {
                    Bound::Literal(copy_value(value, budget, cancelled)?)
                }
                InsertFieldValue::Omitted => {
                    let declaration = declaration(
                        column,
                        properties.as_ref().ok_or(EditError::Preservation)?,
                        budget,
                        cancelled,
                    )?;
                    match declaration {
                        Some(property) => super::constant::bind_clock(
                            column,
                            property.text()?,
                            budget,
                            cancelled,
                        )?,
                        None => Bound::Literal(Value::Null),
                    }
                }
            };
            needed |= matches!(value, Bound::Clock(_));
            row.push(value);
        }
        rows.push(row);
    }
    check(budget, cancelled)?;
    Ok(BoundRows {
        rows,
        original,
        needed,
    })
}
