//! Resolve original insertion intent after full source qualification.
mod constant;
pub(super) mod preview;
mod request;
pub(super) mod stage;

use super::{IncrementingAutoNumber, autonumber, values::copy_value};
use crate::{
    EditError, InspectError, NativeImage, PropertyGroupKind, TableCheck, WriteBlocker,
    header::check, inspect_table_properties, limits::MAX_LIVE_ROWS,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub use request::{InsertFieldValue, OmittedRowInsert};

pub(super) struct ResolvedRow {
    pub values: Vec<Value>,
}

pub(super) struct Prepared<'a> {
    pub rows: Vec<ResolvedRow>,
    pub original: Original<'a>,
}

#[derive(Clone, Copy)]
pub(super) enum Original<'a> {
    Ordinary(&'a [OmittedRowInsert]),
    Incrementing {
        requests: &'a [OmittedRowInsert],
        policy: IncrementingAutoNumber,
    },
}

impl<'a> Original<'a> {
    pub(super) fn requests(self) -> &'a [OmittedRowInsert] {
        match self {
            Self::Ordinary(requests) | Self::Incrementing { requests, .. } => requests,
        }
    }
}

/// Resolve the generated position through its actual owner. Only original
/// requests and an explicit policy survive into the independent audit input.
pub(super) fn prepare<'a>(
    image: &NativeImage,
    checked: &TableCheck,
    original: Original<'a>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared<'a>, EditError> {
    check(budget, cancelled)?;
    let schema = checked.schema();
    let requests = original.requests();
    if requests.len() > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }
    let excluded = match original {
        Original::Ordinary(_) => None,
        Original::Incrementing { policy, .. } => {
            Some(autonumber::counter::bind(image, checked, policy, budget, cancelled)?.column)
        }
    };
    let width = schema.columns().len() - usize::from(excluded.is_some());
    let mut needed = false;
    for request in requests {
        check(budget, cancelled)?;
        if request.values().len() != width {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        for value in request.values() {
            check(budget, cancelled)?;
            needed |= matches!(value, InsertFieldValue::Omitted);
        }
    }

    let properties = if needed {
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

    let mut rows = Vec::new();
    reserve(&mut rows, requests.len(), budget)?;
    for request in requests {
        check(budget, cancelled)?;
        let mut values = Vec::new();
        reserve(&mut values, width, budget)?;
        let columns = schema
            .columns()
            .iter()
            .enumerate()
            .filter(|(position, _)| Some(*position) != excluded);
        for ((_, column), intent) in columns.zip(request.values()) {
            check(budget, cancelled)?;
            let value = match intent {
                InsertFieldValue::Supplied(value) => copy_value(value, budget, cancelled)?,
                InsertFieldValue::Omitted => {
                    let mut selected = None;
                    for group in properties.as_ref().ok_or(EditError::Preservation)?.groups() {
                        check(budget, cancelled)?;
                        budget
                            .charge(
                                Resource::Steps,
                                group.name().len().min(column.name().len()) as u64,
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
                    match selected {
                        Some(property) => {
                            constant::resolve(column, property.text()?, budget, cancelled)?
                        }
                        None => Value::Null,
                    }
                }
            };
            values.push(value);
        }
        rows.push(ResolvedRow { values });
    }
    check(budget, cancelled)?;
    Ok(Prepared { rows, original })
}

fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    budget: &mut ResourceBudget,
) -> Result<(), EditError> {
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(())
}
