//! Rebind original omissions from SOURCE; no producer default/cache inputs.
mod literal;

use super::super::{
    AutoNumberRowInsert, RowInsert,
    omitted::{InsertFieldValue, Original},
    primary_column,
    values::copy_value,
};
use crate::{
    EditError, InspectError, NativeImage, PropertyGroupKind, TableCheck, WriteBlocker,
    header::check, inspect_table_properties,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_expressions::functions::dates::clock::ClockReading;

pub(super) enum Rows {
    Ordinary(Vec<RowInsert>),
    Incrementing(Vec<AutoNumberRowInsert>),
}

impl Rows {
    pub(super) fn ordinary(&self) -> Result<&[RowInsert], EditError> {
        match self {
            Self::Ordinary(rows) => Ok(rows),
            Self::Incrementing(_) => Err(EditError::Preservation),
        }
    }

    pub(super) fn companions(&self) -> Result<&[AutoNumberRowInsert], EditError> {
        match self {
            Self::Incrementing(rows) => Ok(rows),
            Self::Ordinary(_) => Err(EditError::Preservation),
        }
    }
}

pub(super) fn derive(
    image: &NativeImage,
    checked: &TableCheck,
    original: Original<'_>,
    reading: Option<ClockReading>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Rows, EditError> {
    check(budget, cancelled)?;
    let schema = checked.schema();
    let generated = match original {
        Original::Ordinary(_) => None,
        Original::Incrementing { .. } => Some(primary_column::resolve(checked, budget, cancelled)?),
    };
    let width = schema.columns().len() - usize::from(generated.is_some());
    let mut needs_default = false;
    for request in original.requests() {
        check(budget, cancelled)?;
        if request.values().len() != width {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        for field in request.values() {
            check(budget, cancelled)?;
            needs_default |= matches!(field, InsertFieldValue::Omitted);
        }
    }
    let properties = if needs_default {
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
    let mut used_clock = false;
    let mut rows = Vec::new();
    reserve(&mut rows, original.requests().len(), budget)?;
    for request in original.requests() {
        check(budget, cancelled)?;
        let mut values = Vec::new();
        reserve(&mut values, width, budget)?;
        let mut field = 0;
        for (position, column) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            if generated == Some(position) {
                continue;
            }
            let original = request.values().get(field).ok_or(EditError::Preservation)?;
            field += 1;
            let value = match original {
                InsertFieldValue::Supplied(value) => copy_value(value, budget, cancelled)?,
                InsertFieldValue::Omitted => {
                    let properties = properties.as_ref().ok_or(EditError::Preservation)?;
                    let mut declaration = None;
                    for group in properties.groups() {
                        check(budget, cancelled)?;
                        budget
                            .charge(Resource::Steps, group.name().len() as u64)
                            .map_err(InspectError::from)?;
                        if group.kind() != PropertyGroupKind::Column
                            || !column.name().eq_ignore_ascii_case(group.name())
                        {
                            continue;
                        }
                        for property in group.properties() {
                            check(budget, cancelled)?;
                            budget
                                .charge(Resource::Steps, property.name().len() as u64)
                                .map_err(InspectError::from)?;
                            if property.name().eq_ignore_ascii_case("DefaultValue") {
                                if declaration.is_some() {
                                    return Err(EditError::Unsupported(WriteBlocker::Properties));
                                }
                                declaration = Some(property);
                            }
                        }
                    }
                    if let Some(declaration) = declaration {
                        literal::derive(
                            column,
                            declaration.text()?,
                            reading,
                            &mut used_clock,
                            budget,
                            cancelled,
                        )?
                    } else {
                        Value::Null
                    }
                }
            };
            values.push(value);
        }
        if field != width {
            return Err(EditError::Preservation);
        }
        rows.push(values);
    }
    check(budget, cancelled)?;
    if used_clock != reading.is_some() {
        return Err(EditError::Preservation);
    }
    match original {
        Original::Ordinary(_) => {
            let mut result = Vec::new();
            reserve(&mut result, rows.len(), budget)?;
            for row in rows {
                check(budget, cancelled)?;
                result.push(RowInsert::new(row));
            }
            Ok(Rows::Ordinary(result))
        }
        Original::Incrementing { .. } => {
            let mut result = Vec::new();
            reserve(&mut result, rows.len(), budget)?;
            for row in rows {
                check(budget, cancelled)?;
                result.push(AutoNumberRowInsert::new(row));
            }
            Ok(Rows::Incrementing(result))
        }
    }
}

fn reserve<T>(
    items: &mut Vec<T>,
    count: usize,
    budget: &mut ResourceBudget,
) -> Result<(), EditError> {
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    items
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(())
}
