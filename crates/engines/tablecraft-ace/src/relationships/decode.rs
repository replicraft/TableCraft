use super::{NativeRelationship, flags};
use crate::{InspectError, header::check, limits::MAX_COLUMNS};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::{Column, NameComparison, Relationship, RelationshipColumn, SchemaName};

pub(super) const COLUMNS: [&str; 8] = [
    "szRelationship",
    "grbit",
    "ccolumn",
    "icolumn",
    "szReferencedObject",
    "szObject",
    "szReferencedColumn",
    "szColumn",
];
const MAX_RELATIONSHIPS: usize = 256;

struct Pending {
    name: SchemaName,
    parent: SchemaName,
    child: SchemaName,
    flags: u32,
    columns: Vec<Option<RelationshipColumn>>,
}

pub(super) fn decode(
    columns: &[Column],
    rows: &[Vec<Value>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<NativeRelationship>, InspectError> {
    let mut pending: Vec<Pending> = Vec::new();
    for row in rows {
        check(budget, cancelled)?;
        let name = text(cell(columns, row, "szRelationship")?)?;
        let parent = text(cell(columns, row, "szReferencedObject")?)?;
        let child = text(cell(columns, row, "szObject")?)?;
        let flags = integer(cell(columns, row, "grbit")?)? as u32;
        let count = usize::try_from(integer(cell(columns, row, "ccolumn")?)?)
            .map_err(|_| InspectError::Malformed)?;
        let ordinal = usize::try_from(integer(cell(columns, row, "icolumn")?)?)
            .map_err(|_| InspectError::Malformed)?;
        if count == 0 || count > MAX_COLUMNS || ordinal >= count {
            return Err(InspectError::Malformed);
        }
        budget.charge(Resource::Steps, pending.len() as u64)?;
        let position = if let Some(position) = pending
            .iter()
            .position(|group| group.name.as_str().eq_ignore_ascii_case(name))
        {
            position
        } else {
            if pending.len() == MAX_RELATIONSHIPS {
                return Err(InspectError::StructureLimit);
            }
            budget.charge_allocation(1, size_of::<Pending>() as u64)?;
            pending
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            budget
                .charge_allocation(count as u64, size_of::<Option<RelationshipColumn>>() as u64)?;
            let mut columns = Vec::new();
            columns
                .try_reserve_exact(count)
                .map_err(|_| InspectError::Allocation)?;
            columns.resize_with(count, || None);
            pending.push(Pending {
                name: copy(name, budget)?,
                parent: copy(parent, budget)?,
                child: copy(child, budget)?,
                flags,
                columns,
            });
            pending.len() - 1
        };
        let group = &mut pending[position];
        if group.flags != flags
            || group.columns.len() != count
            || !group.parent.as_str().eq_ignore_ascii_case(parent)
            || !group.child.as_str().eq_ignore_ascii_case(child)
            || group.columns[ordinal].is_some()
        {
            return Err(InspectError::Malformed);
        }
        let parent = copy(text(cell(columns, row, "szReferencedColumn")?)?, budget)?;
        let child = copy(text(cell(columns, row, "szColumn")?)?, budget)?;
        group.columns[ordinal] = Some(RelationshipColumn::new(parent, child));
    }
    let mut relationships = Vec::new();
    budget.charge_allocation(pending.len() as u64, size_of::<NativeRelationship>() as u64)?;
    relationships
        .try_reserve_exact(pending.len())
        .map_err(|_| InspectError::Allocation)?;
    for group in pending {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            (group.columns.len() * group.columns.len()) as u64,
        )?;
        budget.charge_allocation(
            group.columns.len() as u64,
            size_of::<RelationshipColumn>() as u64,
        )?;
        let mut columns = Vec::new();
        columns
            .try_reserve_exact(group.columns.len())
            .map_err(|_| InspectError::Allocation)?;
        for column in group.columns {
            columns.push(column.ok_or(InspectError::Malformed)?);
        }
        let declaration = Relationship::new(
            group.name,
            group.parent,
            group.child,
            columns,
            flags::rules(group.flags),
            NameComparison::AsciiInsensitive,
        )?;
        relationships.push(NativeRelationship {
            declaration,
            flags: group.flags,
        });
    }
    check(budget, cancelled)?;
    Ok(relationships)
}

fn cell<'a>(columns: &[Column], row: &'a [Value], name: &str) -> Result<&'a Value, InspectError> {
    let position = columns
        .iter()
        .position(|column| column.name().eq_ignore_ascii_case(name))
        .ok_or(InspectError::Malformed)?;
    row.get(position).ok_or(InspectError::Malformed)
}
fn text(value: &Value) -> Result<&str, InspectError> {
    match value {
        Value::Text(text) => Ok(text),
        _ => Err(InspectError::Malformed),
    }
}
fn integer(value: &Value) -> Result<i32, InspectError> {
    match value {
        Value::Int32(integer) => Ok(*integer),
        _ => Err(InspectError::Malformed),
    }
}
fn copy(name: &str, budget: &mut ResourceBudget) -> Result<SchemaName, InspectError> {
    budget.charge_allocation(name.len() as u64, 1)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(name);
    Ok(SchemaName::new(owned)?)
}

#[cfg(test)]
mod tests;
