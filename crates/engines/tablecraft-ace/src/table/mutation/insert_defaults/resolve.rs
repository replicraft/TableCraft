//! Re-read actual declarations for each fresh original presence projection.
use super::{literal, proof_literal, values::copy_value};
use crate::{
    EditError, InsertFieldValue, InspectError, NativeColumn, NativeImage, NativeProperty,
    OmittedRowInsert, PropertyGroupKind, RowInsert, TableProperties, TableSchema, WriteBlocker,
    format::{
        data_type::OLE,
        layout::{AUTO_NUMBER_FLAG, AUTO_NUMBER_GUID_FLAG},
    },
    header::check,
    inspect_table_properties,
    limits::{MAX_COLUMNS, MAX_LIVE_ROWS},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

#[derive(Clone, Copy)]
enum Projection {
    Preparation,
    IndependentProof,
}

pub(in crate::table::mutation) fn prepare(
    image: &NativeImage,
    schema: &TableSchema,
    requests: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RowInsert>, EditError> {
    resolve(
        image,
        schema,
        requests,
        Projection::Preparation,
        budget,
        cancelled,
    )
}

pub(in crate::table::mutation) fn derive(
    image: &NativeImage,
    schema: &TableSchema,
    requests: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RowInsert>, EditError> {
    resolve(
        image,
        schema,
        requests,
        Projection::IndependentProof,
        budget,
        cancelled,
    )
}

fn resolve(
    image: &NativeImage,
    schema: &TableSchema,
    requests: &[OmittedRowInsert],
    projection: Projection,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RowInsert>, EditError> {
    check(budget, cancelled)?;
    if requests.len() > MAX_LIVE_ROWS || !(1..=MAX_COLUMNS).contains(&schema.columns().len()) {
        return Err(InspectError::StructureLimit.into());
    }
    let mut needed = false;
    for request in requests {
        check(budget, cancelled)?;
        if request.values().len() != schema.columns().len() {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        for field in request.values() {
            check(budget, cancelled)?;
            needed |= matches!(field, InsertFieldValue::Omitted);
        }
    }
    let properties = if needed {
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
        Some(properties)
    } else {
        None
    };
    let mut resolved = vector(requests.len(), budget, cancelled)?;
    for request in requests {
        check(budget, cancelled)?;
        let mut values = vector(schema.columns().len(), budget, cancelled)?;
        for (column, original) in schema.columns().iter().zip(request.values()) {
            check(budget, cancelled)?;
            let value = match original {
                InsertFieldValue::Supplied(value) => copy_value(value, budget, cancelled)?,
                InsertFieldValue::Omitted => {
                    let properties = properties.as_ref().ok_or(EditError::Preservation)?;
                    match declaration(column, properties, budget, cancelled)? {
                        None => Value::Null,
                        Some(property) => match projection {
                            Projection::Preparation => {
                                literal::resolve(column, property.text()?, budget, cancelled)?
                            }
                            Projection::IndependentProof => {
                                let mut used_clock = false;
                                let value = proof_literal::derive(
                                    column,
                                    property.text()?,
                                    None,
                                    &mut used_clock,
                                    budget,
                                    cancelled,
                                )?;
                                if used_clock {
                                    return Err(EditError::Preservation);
                                }
                                value
                            }
                        },
                    }
                }
            };
            values.push(value);
        }
        resolved.push(RowInsert::new(values));
    }
    check(budget, cancelled)?;
    Ok(resolved)
}

fn declaration<'a>(
    column: &NativeColumn,
    properties: &'a TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<&'a NativeProperty>, EditError> {
    let mut selected = None;
    for group in properties.groups() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                (group.name().len() + column.name().len()) as u64,
            )
            .map_err(InspectError::from)?;
        if group.kind() != PropertyGroupKind::Column
            || !group.name().eq_ignore_ascii_case(column.name())
        {
            continue;
        }
        for property in group.properties() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, property.name().len() as u64)
                .map_err(InspectError::from)?;
            if property.name().eq_ignore_ascii_case("DefaultValue")
                && selected.replace(property).is_some()
            {
                return Err(EditError::Unsupported(WriteBlocker::Properties));
            }
        }
    }
    if selected.is_some()
        && (column.native_type() == OLE
            || column.flags() & (AUTO_NUMBER_FLAG | AUTO_NUMBER_GUID_FLAG) != 0)
    {
        return Err(EditError::Unsupported(WriteBlocker::Properties));
    }
    Ok(selected)
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
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(values)
}
