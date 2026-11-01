//! Original target metadata and paid request directory, never stage authority.
use crate::{
    EditError, FittedColumnUpdates, FixedColumnUpdates, InspectError, NativeImage, WriteBlocker,
    format::data_type::SHORT_BINARY,
    header::check,
    table::mutation::{
        fixed_records, preserved_source::Source, record_serialization::storage, rows,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
pub(in crate::table::mutation) enum Target {
    Fixed {
        column: usize,
        kind: ScalarType,
        width: usize,
    },
    Variable {
        column: usize,
    },
}

impl Target {
    pub(in crate::table::mutation) fn column(self) -> usize {
        match self {
            Self::Fixed { column, .. } | Self::Variable { column } => column,
        }
    }
}

pub(in crate::table::mutation) struct Assignment {
    pub group: usize,
    pub index: usize,
    ordinal: usize,
}

pub(in crate::table::mutation) struct Selected {
    pub ordinal: usize,
    pub start: usize,
    pub end: usize,
}

pub(in crate::table::mutation) struct Bound {
    pub targets: Vec<Target>,
    pub columns: Vec<usize>,
    pub assignments: Vec<Assignment>,
    pub rows: Vec<Selected>,
}

pub(in crate::table::mutation) fn vector<T>(
    count: usize,
    budget: &mut ResourceBudget,
) -> Result<Vec<T>, EditError> {
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(result)
}

pub(in crate::table::mutation) fn all(
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Bound, EditError> {
    all_with(source, groups, false, budget, cancelled)
}

pub(super) fn all_indexed_binary(
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Bound, EditError> {
    all_with(source, groups, true, budget, cancelled)
}

fn all_with(
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    indexed_binary: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Bound, EditError> {
    check(budget, cancelled)?;
    if groups.is_empty() || groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    let schema = source.table.schema();
    let mut columns = vector(groups.len(), budget)?;
    let mut fixed = vector(groups.len(), budget)?;
    let mut count = 0_usize;
    let mut variable_count = 0_usize;
    for group in groups {
        check(budget, cancelled)?;
        count = count
            .checked_add(group.len())
            .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
            .ok_or(InspectError::StructureLimit)?;
        let mut selected = None;
        for (position, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            fixed_records::name_work(field.name(), group.name(), budget)?;
            if field.name().eq_ignore_ascii_case(group.name())
                && selected.replace(position).is_some()
            {
                return Err(EditError::Preservation);
            }
        }
        let selected = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        budget
            .charge(Resource::Steps, columns.len() as u64)
            .map_err(InspectError::from)?;
        if columns.contains(&selected) {
            return Err(EditError::Conflict);
        }
        columns.push(selected);
        match group {
            FittedColumnUpdates::Fixed { column, updates } => {
                fixed.push(FixedColumnUpdates::new(schema.name(), column, updates));
            }
            FittedColumnUpdates::Variable { .. } => variable_count += 1,
        }
    }
    if variable_count == 0 {
        // Fixed-only requests keep their existing ordinary TCNI0004 route.
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let fixed_bound = if fixed.is_empty() {
        Vec::new()
    } else {
        fixed_records::binding::all_normal(source, &fixed, budget, cancelled)?.0
    };
    let mut fixed_bound = fixed_bound.into_iter();
    let mut targets = vector(groups.len(), budget)?;
    for (group, column) in groups.iter().zip(columns.iter().copied()) {
        check(budget, cancelled)?;
        match group {
            FittedColumnUpdates::Fixed { .. } => {
                let bound = fixed_bound.next().ok_or(EditError::Preservation)?;
                if bound.column != column {
                    return Err(EditError::Preservation);
                }
                targets.push(Target::Fixed {
                    column,
                    kind: bound.kind,
                    width: bound.width,
                });
            }
            FittedColumnUpdates::Variable { updates, .. } => {
                let field = &schema.columns()[column];
                storage::qualify(field)?;
                for dependency in &source.dependencies {
                    check(budget, cancelled)?;
                    if *dependency == column {
                        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                    }
                }
                for index in source.indexes.indexes() {
                    for key in index.columns() {
                        check(budget, cancelled)?;
                        if key.number() == field.number()
                            && !(indexed_binary && field.native_type() == SHORT_BINARY)
                        {
                            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                        }
                    }
                }
                rows::unique(updates, budget, cancelled)?;
                for update in *updates {
                    check(budget, cancelled)?;
                    storage::validate(field, update.expected())?;
                    storage::validate(field, update.value())?;
                    if update.row() >= source.records.len() {
                        return Err(EditError::Conflict);
                    }
                }
                targets.push(Target::Variable { column });
            }
        }
    }
    let mut assignments = vector(count, budget)?;
    for (group, request) in groups.iter().enumerate() {
        for index in 0..request.len() {
            check(budget, cancelled)?;
            let cell = request.cell(index).ok_or(EditError::Preservation)?;
            assignments.push(Assignment {
                group,
                index,
                ordinal: cell.row,
            });
        }
    }
    let levels = count.checked_ilog2().unwrap_or(0) as usize + 1;
    budget
        .charge(Resource::Steps, (count * levels) as u64)
        .map_err(InspectError::from)?;
    assignments.sort_unstable_by_key(|cell| (cell.ordinal, cell.group));
    let mut selected: Vec<Selected> = vector(count, budget)?;
    for (index, assignment) in assignments.iter().enumerate() {
        check(budget, cancelled)?;
        if let Some(row) = selected.last_mut()
            && row.ordinal == assignment.ordinal
        {
            row.end = index + 1;
        } else {
            selected.push(Selected {
                ordinal: assignment.ordinal,
                start: index,
                end: index + 1,
            });
        }
    }
    let levels = columns.len().checked_ilog2().unwrap_or(0) as usize + 1;
    budget
        .charge(Resource::Steps, (columns.len() * levels) as u64)
        .map_err(InspectError::from)?;
    // decode_selected emits schema order regardless of request group order.
    columns.sort_unstable();
    check(budget, cancelled)?;
    Ok(Bound {
        targets,
        columns,
        assignments,
        rows: selected,
    })
}

pub(in crate::table::mutation) fn equal(
    left: &Value,
    right: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    let bytes = |value: &Value| match value {
        Value::Text(text) => text.len(),
        Value::Binary(bytes) => bytes.len(),
        _ => size_of::<Value>(),
    };
    let work = bytes(left)
        .checked_add(bytes(right))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    Ok(left == right)
}

pub(in crate::table::mutation) fn equal_bytes(
    left: &[u8],
    right: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    let work = left
        .len()
        .checked_add(right.len())
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    Ok(left == right)
}

/// The native selected decoder emits ascending schema positions. Resolve each
/// original request group through that explicitly sorted projection.
pub(in crate::table::mutation) fn value<'a>(
    bound: &Bound,
    values: &'a [Value],
    group: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a Value, EditError> {
    check(budget, cancelled)?;
    let column = bound
        .targets
        .get(group)
        .ok_or(EditError::Preservation)?
        .column();
    let levels = bound.columns.len().checked_ilog2().unwrap_or(0) as u64 + 1;
    budget
        .charge(Resource::Steps, levels)
        .map_err(InspectError::from)?;
    let position = bound
        .columns
        .binary_search(&column)
        .map_err(|_| EditError::Preservation)?;
    values.get(position).ok_or(EditError::Preservation)
}
