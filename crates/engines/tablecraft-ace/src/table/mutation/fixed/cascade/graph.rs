use super::super::super::{EditError, WriteBlocker};
use super::cells::{Cell, Cells, reserve, scalar};
use crate::{
    InspectError, NativeImage, NativeRelationship, TableSnapshot,
    format::layout::INDEX_KEY_COLUMNS, header::check, read_table,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Guid, Value},
};
use tablecraft_schema::{ReferentialAction, ScalarType};

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum Atom {
    Integral(i64),
    Decimal(i128),
    Guid([u8; Guid::BYTE_LEN]),
}
type Key = [Option<Atom>; INDEX_KEY_COLUMNS];
pub(super) struct Link {
    parent: usize,
    child: usize,
    parents: [usize; INDEX_KEY_COLUMNS],
    children: [usize; INDEX_KEY_COLUMNS],
    width: usize,
    cascade: bool,
    matches: Vec<(usize, usize)>,
}
pub(super) fn table(
    cells: &mut Cells,
    image: &NativeImage,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, cells.tables.len() as u64)
        .map_err(InspectError::from)?;
    if let Some(position) = cells
        .tables
        .iter()
        .position(|t| t.schema().name().eq_ignore_ascii_case(name))
    {
        return Ok(position);
    }
    if cells.tables.len() >= NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    let snapshot = read_table(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        name,
        budget,
        cancelled,
    )?;
    reserve(&mut cells.tables, budget)?;
    cells.tables.push(snapshot);
    Ok(cells.tables.len() - 1)
}
pub(super) fn link(
    cells: &mut Cells,
    image: &NativeImage,
    parent: usize,
    relationship: &NativeRelationship,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Link, EditError> {
    let declaration = relationship.declaration();
    let rules = declaration
        .rules()
        .ok_or(EditError::Unsupported(WriteBlocker::ConstraintCoverage))?;
    if !rules.enforced
        || declaration.columns().is_empty()
        || declaration.columns().len() > INDEX_KEY_COLUMNS
    {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let child = table(
        cells,
        image,
        declaration.child_table().as_str(),
        budget,
        cancelled,
    )?;
    let mut output = Link {
        parent,
        child,
        parents: [0; INDEX_KEY_COLUMNS],
        children: [0; INDEX_KEY_COLUMNS],
        width: declaration.columns().len(),
        cascade: match rules.on_update {
            ReferentialAction::Cascade => true,
            ReferentialAction::Restrict => false,
            _ => return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage)),
        },
        matches: Vec::new(),
    };
    for (i, pair) in declaration.columns().iter().enumerate() {
        check(budget, cancelled)?;
        let p = position(&cells.tables[parent], pair.parent().as_str())?;
        let c = position(&cells.tables[child], pair.child().as_str())?;
        if cells.tables[parent].columns()[p].data_type()
            != cells.tables[child].columns()[c].data_type()
        {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
        if cells.tables[parent].schema().columns()[p].decimal_parameters()
            != cells.tables[child].schema().columns()[c].decimal_parameters()
        {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
        output.parents[i] = p;
        output.children[i] = c;
    }
    let keys = keys(
        &cells.tables[parent],
        &output.parents[..output.width],
        budget,
        cancelled,
    )?;
    for row in 0..cells.tables[child].rows().len() {
        check(budget, cancelled)?;
        let key = key(
            &cells.tables[child].rows()[row],
            &output.children[..output.width],
        )?;
        if key.iter().all(Option::is_none) {
            continue;
        }
        budget
            .charge(
                Resource::Steps,
                keys.len().checked_ilog2().unwrap_or(0) as u64 + 1,
            )
            .map_err(InspectError::from)?;
        if let Ok(position) = keys.binary_search_by_key(&key, |entry| entry.0) {
            reserve(&mut output.matches, budget)?;
            output.matches.push((keys[position].1, row));
        } else {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
    }
    Ok(output)
}
fn position(table: &TableSnapshot, name: &str) -> Result<usize, EditError> {
    table
        .columns()
        .iter()
        .position(|c| {
            c.name().eq_ignore_ascii_case(name)
                && matches!(
                    c.data_type(),
                    ScalarType::Byte
                        | ScalarType::Int16
                        | ScalarType::Int32
                        | ScalarType::Int64
                        | ScalarType::Currency
                        | ScalarType::Decimal
                        | ScalarType::Guid
                )
        })
        .ok_or(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
}
fn key(row: &[Value], columns: &[usize]) -> Result<Key, EditError> {
    let mut key = [None; INDEX_KEY_COLUMNS];
    for (i, &column) in columns.iter().enumerate() {
        key[i] = match row.get(column).ok_or(InspectError::Malformed)? {
            Value::Null => None,
            Value::Byte(v) => Some(Atom::Integral(i64::from(*v))),
            Value::Int16(v) => Some(Atom::Integral(i64::from(*v))),
            Value::Int32(v) => Some(Atom::Integral(i64::from(*v))),
            Value::Int64(v) => Some(Atom::Integral(*v)),
            Value::Currency(v) => Some(Atom::Integral(v.scaled())),
            Value::Decimal(v) => Some(Atom::Decimal(v.coefficient())),
            Value::Guid(v) => Some(Atom::Guid(v.to_bytes())),
            _ => return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage)),
        };
    }
    Ok(key)
}
fn keys(
    table: &TableSnapshot,
    columns: &[usize],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<(Key, usize)>, EditError> {
    let mut output = Vec::new();
    for (row, values) in table.rows().iter().enumerate() {
        check(budget, cancelled)?;
        let key = key(values, columns)?;
        if key.iter().any(Option::is_some) {
            reserve(&mut output, budget)?;
            output.push((key, row));
        }
    }
    let levels = output.len().checked_ilog2().unwrap_or(0) as u64 + 1;
    budget
        .charge(Resource::Steps, output.len() as u64 * levels)
        .map_err(InspectError::from)?;
    output.sort_unstable();
    if output.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    Ok(output)
}
impl Link {
    fn cells(&self, parent_row: usize, child_row: usize, position: usize) -> (Cell, Cell) {
        (
            Cell {
                table: self.parent,
                row: parent_row,
                column: self.parents[position],
            },
            Cell {
                table: self.child,
                row: child_row,
                column: self.children[position],
            },
        )
    }
    fn changed(
        &self,
        cells: &Cells,
        parent: usize,
        budget: &mut ResourceBudget,
    ) -> Result<bool, EditError> {
        for &column in &self.parents[..self.width] {
            let cell = Cell {
                table: self.parent,
                row: parent,
                column,
            };
            if cells.value(cell, budget)? != cells.original(cell)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn propagate(
        &self,
        cells: &mut Cells,
        final_check: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        let mut changed = false;
        for &(parent, child) in &self.matches {
            check(budget, cancelled)?;
            if !self.changed(cells, parent, budget)? {
                continue;
            }
            if !self.cascade {
                return Err(EditError::ReferentialAction);
            }
            for position in 0..self.width {
                check(budget, cancelled)?;
                let (source, target) = self.cells(parent, child, position);
                let value = cells.value(source, budget)?;
                if final_check {
                    if cells.value(target, budget)? != value {
                        return Err(EditError::Conflict);
                    }
                } else if value != cells.original(source)? {
                    let value = scalar(value)?;
                    changed |= cells.assign(target, value, false, budget, cancelled)?;
                }
            }
        }
        Ok(changed)
    }
}
