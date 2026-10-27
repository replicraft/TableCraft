//! Exact original required-primary associations and iterative directed-cycle proof.
use super::{EditError, reserve, state::Table};
use crate::{
    InspectError, NativeRelationship, WriteBlocker, format::layout::INDEX_KEY_COLUMNS,
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Guid, Value},
};
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum Atom {
    Integral(i64),
    Decimal(i128),
    Guid([u8; Guid::BYTE_LEN]),
}

type Key = [Option<Atom>; INDEX_KEY_COLUMNS];

pub(super) struct Link {
    pub parent: usize,
    pub child: usize,
    pub matches: Vec<(usize, usize)>,
}

pub(super) fn link(
    tables: &[Table],
    parent: usize,
    child: usize,
    relationship: &NativeRelationship,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Link, EditError> {
    let columns = relationship.declaration().columns();
    if columns.is_empty() || columns.len() > INDEX_KEY_COLUMNS {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let mut parents = [0; INDEX_KEY_COLUMNS];
    let mut children = [0; INDEX_KEY_COLUMNS];
    for (position, pair) in columns.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                (tables[parent].snapshot.columns().len() + tables[child].snapshot.columns().len())
                    as u64,
            )
            .map_err(InspectError::from)?;
        let p = column(&tables[parent], pair.parent().as_str())?;
        let c = column(&tables[child], pair.child().as_str())?;
        if tables[parent].snapshot.columns()[p].data_type()
            != tables[child].snapshot.columns()[c].data_type()
            || tables[parent].snapshot.schema().columns()[p].decimal_parameters()
                != tables[child].snapshot.schema().columns()[c].decimal_parameters()
        {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
        parents[position] = p;
        children[position] = c;
    }
    let parents = &parents[..columns.len()];
    let children = &children[..columns.len()];
    let mut primary = false;
    for checked in tables[parent].before.indexes() {
        check(budget, cancelled)?;
        let index = checked.index();
        if index
            .constraints()
            .is_some_and(|rules| rules.primary_key() && rules.required() && rules.unique())
            && index.columns().len() == parents.len()
            && index
                .columns()
                .iter()
                .zip(parents)
                .all(|(indexed, &position)| {
                    indexed.number() == tables[parent].before.schema().columns()[position].number()
                })
        {
            primary = true;
        }
    }
    if !primary {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let mut keys = Vec::new();
    for (row, values) in tables[parent].snapshot.rows().iter().enumerate() {
        check(budget, cancelled)?;
        let key = key(values, parents)?;
        if key[..parents.len()].iter().any(Option::is_none) {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
        reserve(&mut keys, budget)?;
        keys.push((key, row));
    }
    let levels = keys.len().checked_ilog2().unwrap_or(0) as u64 + 1;
    budget
        .charge(Resource::Steps, keys.len() as u64 * levels)
        .map_err(InspectError::from)?;
    keys.sort_unstable();
    if keys.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let mut matches = Vec::new();
    for (child_row, values) in tables[child].snapshot.rows().iter().enumerate() {
        check(budget, cancelled)?;
        let key = key(values, children)?;
        if key.iter().all(Option::is_none) {
            continue;
        }
        budget
            .charge(Resource::Steps, levels)
            .map_err(InspectError::from)?;
        let position = keys
            .binary_search_by_key(&key, |entry| entry.0)
            .map_err(|_| EditError::Unsupported(WriteBlocker::ConstraintCoverage))?;
        reserve(&mut matches, budget)?;
        matches.push((keys[position].1, child_row));
    }
    Ok(Link {
        parent,
        child,
        matches,
    })
}

fn column(table: &Table, name: &str) -> Result<usize, EditError> {
    table
        .snapshot
        .columns()
        .iter()
        .position(|column| {
            column.name().eq_ignore_ascii_case(name)
                && matches!(
                    column.data_type(),
                    ScalarType::Byte
                        | ScalarType::Int16
                        | ScalarType::Int32
                        | ScalarType::Int64
                        | ScalarType::Currency
                        | ScalarType::Guid
                        | ScalarType::Decimal
                )
        })
        .ok_or(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
}

fn key(values: &[Value], columns: &[usize]) -> Result<Key, EditError> {
    let mut key = [None; INDEX_KEY_COLUMNS];
    for (position, &column) in columns.iter().enumerate() {
        key[position] = match values.get(column).ok_or(InspectError::Malformed)? {
            Value::Null => None,
            Value::Byte(value) => Some(Atom::Integral(i64::from(*value))),
            Value::Int16(value) => Some(Atom::Integral(i64::from(*value))),
            Value::Int32(value) => Some(Atom::Integral(i64::from(*value))),
            Value::Int64(value) => Some(Atom::Integral(*value)),
            Value::Currency(value) => Some(Atom::Integral(value.scaled())),
            Value::Decimal(value) => Some(Atom::Decimal(value.coefficient())),
            Value::Guid(value) => Some(Atom::Guid(value.to_bytes())),
            _ => return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage)),
        };
    }
    Ok(key)
}

pub(super) fn acyclic(
    tables: usize,
    links: &[Link],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    budget
        .charge_allocation(tables as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    let mut incoming = Vec::new();
    incoming
        .try_reserve_exact(tables)
        .map_err(|_| InspectError::Allocation)?;
    incoming.resize(tables, 0_usize);
    for link in links {
        check(budget, cancelled)?;
        incoming[link.child] = incoming[link.child]
            .checked_add(1)
            .ok_or(InspectError::StructureLimit)?;
    }
    let mut ready = Vec::new();
    for (table, &count) in incoming.iter().enumerate() {
        check(budget, cancelled)?;
        if count == 0 {
            reserve(&mut ready, budget)?;
            ready.push(table);
        }
    }
    let mut next = 0;
    while next < ready.len() {
        for link in links {
            check(budget, cancelled)?;
            if link.parent == ready[next] {
                incoming[link.child] -= 1;
                if incoming[link.child] == 0 {
                    reserve(&mut ready, budget)?;
                    ready.push(link.child);
                }
            }
        }
        next += 1;
    }
    if ready.len() != tables {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
