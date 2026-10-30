//! Borrowed typed cells, resolved against one immutable original definition.
use super::super::{EditError, FixedScalarUpdate, SlotScalarUpdate, WriteBlocker, field, rows};
use crate::{InspectError, NativeColumn, NativeImage, TableSchema};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

/// One selected fitted-record column, with explicit original-value expectations.
pub enum FittedColumnUpdates<'a> {
    /// Existing fixed scalar payload/presence changes.
    Fixed {
        /// Exact native column name.
        column: &'a str,
        /// Complete original-row fixed replacements.
        updates: &'a [FixedScalarUpdate],
    },
    /// One ordinary Short Text/Binary field using original record capacity.
    Variable {
        /// Exact native column name.
        column: &'a str,
        /// Complete original-row variable replacements.
        updates: &'a [SlotScalarUpdate],
    },
}

pub(super) struct Cell<'a> {
    pub row: usize,
    pub expected: &'a Value,
    pub value: &'a Value,
}

impl FittedColumnUpdates<'_> {
    pub(super) fn name(&self) -> &str {
        match self {
            Self::Fixed { column, .. } | Self::Variable { column, .. } => column,
        }
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Fixed { updates, .. } => updates.len(),
            Self::Variable { updates, .. } => updates.len(),
        }
    }

    pub(super) fn cell(&self, position: usize) -> Option<Cell<'_>> {
        match self {
            Self::Fixed { updates, .. } => updates.get(position).map(|cell| Cell {
                row: cell.row(),
                expected: cell.expected(),
                value: cell.value(),
            }),
            Self::Variable { updates, .. } => updates.get(position).map(|cell| Cell {
                row: cell.row(),
                expected: cell.expected(),
                value: cell.value(),
            }),
        }
    }
}

pub(super) struct Ordinal(pub usize);

impl rows::RowAssignment for Ordinal {
    fn row(&self) -> usize {
        self.0
    }
}

pub(super) struct Bound {
    pub columns: Vec<usize>,
    pub rows: Vec<Ordinal>,
    pub variable_group: usize,
}

pub(super) fn bind(
    schema: &TableSchema,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Bound, EditError> {
    if groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    budget
        .charge_allocation(groups.len() as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(groups.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut variable_group = None;
    let mut fixed_groups = 0;
    let mut count = 0_usize;

    for (group_number, group) in groups.iter().enumerate() {
        check(budget, cancelled)?;
        count = count
            .checked_add(group.len())
            .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge(
                Resource::Steps,
                (schema.columns().len() + columns.len()) as u64,
            )
            .map_err(InspectError::from)?;
        pay_name_scan(schema, group.name(), budget, cancelled)?;
        let position = schema
            .columns()
            .iter()
            .position(|field| field.name().eq_ignore_ascii_case(group.name()))
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        if columns.contains(&position) {
            return Err(EditError::Conflict);
        }
        let selected = &schema.columns()[position];
        match group {
            FittedColumnUpdates::Fixed { updates, .. } => {
                let (kind, width) = fixed_shape(selected)?;
                pay_name_scan(schema, selected.name(), budget, cancelled)?;
                budget
                    .charge(Resource::Steps, schema.columns().len() as u64)
                    .map_err(InspectError::from)?;
                field::fixed_column(schema, selected.name(), kind, width)?;
                rows::unique(updates, budget, cancelled)?;
                fixed_groups += 1;
            }
            FittedColumnUpdates::Variable { updates, .. } => {
                if variable_group.replace(group_number).is_some() {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
                super::super::slot::storage::qualify(selected)?;
                rows::unique(updates, budget, cancelled)?;
            }
        }
        for index in 0..group.len() {
            check(budget, cancelled)?;
            let cell = group.cell(index).ok_or(EditError::Preservation)?;
            budget
                .charge(Resource::Steps, (2 * selected.storage_length()) as u64)
                .map_err(InspectError::from)?;
            match group {
                FittedColumnUpdates::Fixed { .. } => {
                    fixed_bytes(selected, cell.expected)?;
                    fixed_bytes(selected, cell.value)?;
                }
                FittedColumnUpdates::Variable { .. } => {
                    super::super::slot::storage::validate(selected, cell.expected)?;
                    super::super::slot::storage::validate(selected, cell.value)?;
                }
            }
        }
        columns.push(position);
    }
    if fixed_groups == 0 {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let variable_group = variable_group.ok_or(EditError::Unsupported(WriteBlocker::Column))?;

    budget
        .charge_allocation(count as u64, size_of::<Ordinal>() as u64)
        .map_err(InspectError::from)?;
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for group in groups {
        for index in 0..group.len() {
            check(budget, cancelled)?;
            selected.push(Ordinal(
                group.cell(index).ok_or(EditError::Preservation)?.row,
            ));
        }
    }
    let levels = selected.len().checked_ilog2().unwrap_or(0) as usize + 1;
    budget
        .charge(Resource::Steps, (selected.len() * levels) as u64)
        .map_err(InspectError::from)?;
    selected.sort_unstable_by_key(|row| row.0);
    selected.dedup_by_key(|row| row.0);

    Ok(Bound {
        columns,
        rows: selected,
        variable_group,
    })
}

pub(super) fn fixed_shape(column: &NativeColumn) -> Result<(ScalarType, usize), EditError> {
    let kind = column
        .scalar_type()
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    let width = match kind {
        ScalarType::Boolean => 0,
        ScalarType::Byte => 1,
        ScalarType::Int16 => 2,
        ScalarType::Int32 | ScalarType::Single => 4,
        ScalarType::Int64 | ScalarType::Currency | ScalarType::Double | ScalarType::DateTime => 8,
        ScalarType::Guid => 16,
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    Ok((kind, width))
}

// Stack-only exact scalar encoding; the native primitive owns representation.
pub(super) fn fixed_bytes(
    column: &NativeColumn,
    value: &Value,
) -> Result<([u8; 16], usize, bool), EditError> {
    let (kind, width) = fixed_shape(column)?;
    let mut bytes = [0; 16];
    if kind == ScalarType::Boolean {
        let Value::Boolean(value) = value else {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        };
        return Ok((bytes, width, *value));
    }
    if matches!(value, Value::Null) {
        return Ok((bytes, width, false));
    }
    match width {
        1 => bytes[..1].copy_from_slice(&super::super::fixed::encode_fixed::<1>(kind, value)?),
        2 => bytes[..2].copy_from_slice(&super::super::fixed::encode_fixed::<2>(kind, value)?),
        4 => bytes[..4].copy_from_slice(&super::super::fixed::encode_fixed::<4>(kind, value)?),
        8 => bytes[..8].copy_from_slice(&super::super::fixed::encode_fixed::<8>(kind, value)?),
        16 => bytes.copy_from_slice(&super::super::fixed::encode_fixed::<16>(kind, value)?),
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    }
    Ok((bytes, width, true))
}

pub(super) fn check(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    crate::header::check(budget, cancelled)?;
    Ok(())
}

// Work helpers preserve representational equality; they add no SQL coercion.
pub(super) fn pay_names(
    left: &str,
    right: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let work = (left.len() as u64)
        .checked_add(right.len() as u64)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work)
        .map_err(InspectError::from)?;
    Ok(())
}

pub(super) fn pay_name_scan(
    schema: &TableSchema,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for field in schema.columns() {
        pay_names(field.name(), name, budget, cancelled)?;
    }
    Ok(())
}

pub(super) fn values_equal(
    left: &Value,
    right: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    let cost = |value: &Value| match value {
        Value::Text(text) => text.len() as u64,
        Value::Binary(bytes) => bytes.len() as u64,
        _ => size_of::<Value>() as u64,
    };
    let work = cost(left)
        .checked_add(cost(right))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work)
        .map_err(InspectError::from)?;
    Ok(left == right)
}

pub(super) fn rows_equal(
    left: &[Value],
    right: &[Value],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.iter().zip(right) {
        if !values_equal(left, right, budget, cancelled)? {
            return Ok(false);
        }
    }
    Ok(true)
}
