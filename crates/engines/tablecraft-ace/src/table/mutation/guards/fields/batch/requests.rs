//! Complete original user intent admission before any event frame is evaluated.
use super::super::super::{BeforeEventGuardError, buffer, error};
use super::{bound::SourceFacts, projection::Int32Projection};
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, InspectError, NativeImage, WriteBlocker,
    format::data_type::{BOOLEAN, CURRENCY, DATE_TIME, INT32, INT64},
    header::check,
    table::mutation::{field, rows},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

pub(super) struct Group<'request> {
    pub native: usize,
    pub snapshot: usize,
    pub frame: Option<usize>,
    pub updates: &'request [FixedScalarUpdate],
}

pub(super) struct Cell<'request> {
    pub row: usize,
    pub group: usize,
    pub update: &'request FixedScalarUpdate,
}

pub(super) struct Batch<'request> {
    pub groups: Vec<Group<'request>>,
    pub cells: Vec<Cell<'request>>,
    /// Original live ordinal order, independent of caller group/row order.
    pub ordinals: Vec<usize>,
}

pub(super) fn bind<'request>(
    source: SourceFacts<'_>,
    projection: &Int32Projection<'_>,
    groups: &[FixedColumnUpdates<'request>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Batch<'request>, BeforeEventGuardError> {
    projection.require_table(source.table)?;
    if groups.is_empty() || groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(unsupported_column());
    }
    let capacity = groups.iter().try_fold(0_usize, |count, group| {
        count
            .checked_add(group.updates().len())
            .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
            .ok_or(InspectError::StructureLimit)
    })?;
    let mut bound: Vec<Group<'request>> = buffer::vector(groups.len(), budget, cancelled)?;
    let mut cells = buffer::vector(capacity, budget, cancelled)?;

    for (group_index, group) in groups.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                (group.table().len() + source.table.schema().name().len()) as u64,
            )
            .map_err(InspectError::from)?;
        if !group
            .table()
            .eq_ignore_ascii_case(source.table.schema().name())
        {
            return Err(unsupported_column());
        }
        let mut selected = None;
        for (native, field) in source.table.schema().columns().iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    (field.name().len() + group.column().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if field.name().eq_ignore_ascii_case(group.column())
                && selected.replace(native).is_some()
            {
                return Err(unsupported_column());
            }
        }
        let native = selected.ok_or_else(unsupported_column)?;
        for previous in &bound {
            check(budget, cancelled)?;
            if previous.native == native {
                return Err(unsupported_column());
            }
        }
        let field = &source.table.schema().columns()[native];
        let (kind, width) = match field.native_type() {
            BOOLEAN => (ScalarType::Boolean, 0),
            INT32 => (ScalarType::Int32, size_of::<i32>()),
            INT64 => (ScalarType::Int64, size_of::<i64>()),
            CURRENCY => (ScalarType::Currency, size_of::<i64>()),
            DATE_TIME => (ScalarType::DateTime, size_of::<f64>()),
            _ => return Err(unsupported_column()),
        };
        field::fixed_column_at(source.table.schema(), native, kind, width)
            .map_err(error::intent)?;
        for dependency in source.dependencies {
            check(budget, cancelled)?;
            if *dependency == native {
                return Err(indexed());
            }
        }
        for index in source.indexes.indexes() {
            for key in index.columns() {
                check(budget, cancelled)?;
                if key.number() == native {
                    return Err(indexed());
                }
            }
        }
        let mut snapshot = None;
        for (position, column) in source.table.columns().iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    (column.name().len() + field.name().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if column.name().eq_ignore_ascii_case(field.name())
                && (snapshot.replace(position).is_some() || column.data_type() != kind)
            {
                return Err(unsupported_column());
            }
        }
        let snapshot = snapshot.ok_or_else(unsupported_column)?;
        rows::unique(group.updates(), budget, cancelled).map_err(error::intent)?;
        for update in group.updates() {
            check(budget, cancelled)?;
            if !carrier(kind, update.expected()) || !carrier(kind, update.value()) {
                return Err(unsupported_column());
            }
            let original = source
                .table
                .rows()
                .get(update.row())
                .and_then(|row| row.get(snapshot))
                .ok_or_else(conflict)?;
            if original != update.expected() {
                return Err(conflict());
            }
            cells.push(Cell {
                row: update.row(),
                group: group_index,
                update,
            });
        }
        let frame = if kind == ScalarType::Int32 {
            Some(
                projection
                    .frame_position(native, budget, cancelled)?
                    .ok_or(InspectError::Malformed)?,
            )
        } else {
            None
        };
        bound.push(Group {
            native,
            snapshot,
            frame,
            updates: group.updates(),
        });
    }

    let levels = capacity.checked_ilog2().unwrap_or(0) as usize + 1;
    let work = capacity
        .checked_mul(levels)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    check(budget, cancelled)?;
    cells.sort_unstable_by_key(|cell| (cell.row, cell.group));
    check(budget, cancelled)?;
    let mut ordinals = buffer::vector(capacity, budget, cancelled)?;
    for cell in &cells {
        check(budget, cancelled)?;
        if ordinals.last().copied() != Some(cell.row) {
            ordinals.push(cell.row);
        }
    }
    check(budget, cancelled)?;
    Ok(Batch {
        groups: bound,
        cells,
        ordinals,
    })
}

fn carrier(kind: ScalarType, value: &Value) -> bool {
    matches!(
        (kind, value),
        (ScalarType::Boolean, Value::Boolean(_))
            | (ScalarType::Int32, Value::Null | Value::Int32(_))
            | (ScalarType::Int64, Value::Null | Value::Int64(_))
            | (ScalarType::Currency, Value::Null | Value::Currency(_))
            | (ScalarType::DateTime, Value::Null | Value::DateTime(_))
    )
}

fn unsupported_column() -> BeforeEventGuardError {
    BeforeEventGuardError::Intent(EditError::Unsupported(WriteBlocker::Column))
}

fn indexed() -> BeforeEventGuardError {
    BeforeEventGuardError::Intent(EditError::Unsupported(WriteBlocker::IndexedColumn))
}

fn conflict() -> BeforeEventGuardError {
    BeforeEventGuardError::Intent(EditError::Conflict)
}
