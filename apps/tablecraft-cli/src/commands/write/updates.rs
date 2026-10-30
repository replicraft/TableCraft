//! Move exact typed values into groups, without cloning row contents.
pub(super) mod mixed;
use super::{
    error::{Error, input},
    input::{MAX_REQUESTS, reserve},
    tick, values,
};
use tablecraft_ace::InspectError;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) const SET_FIELDS: usize = 6;

pub(super) struct Group<'a, T> {
    pub table: &'a str,
    pub column: &'a str,
    pub rows: Vec<T>,
}

pub(super) fn parse<'a, T>(
    cells: &'a [String],
    max_value_bytes: usize,
    make: fn(usize, Value, Value) -> T,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Group<'a, T>>, Error> {
    parse_with_kind(
        cells,
        max_value_bytes,
        |_, ordinal, expected, value| make(ordinal, expected, value),
        budget,
        cancelled,
    )
}

pub(super) fn parse_with_kind<'a, T>(
    cells: &'a [String],
    max_value_bytes: usize,
    make: impl Fn(&str, usize, Value, Value) -> T,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Group<'a, T>>, Error> {
    let (assignments, remainder) = cells.as_chunks::<SET_FIELDS>();
    if assignments.is_empty() || !remainder.is_empty() || assignments.len() > MAX_REQUESTS {
        return Err(input("Expected 1..4096 complete --set assignments"));
    }
    let mut groups: Vec<Group<'_, T>> = Vec::new();
    for fields in assignments {
        tick(budget, cancelled)?;
        let text_bytes = fields
            .iter()
            .try_fold(0usize, |total, text| total.checked_add(text.len()))
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge_allocation(text_bytes as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Rows, 1)
            .map_err(InspectError::from)?;
        if fields[4].len() > max_value_bytes || fields[5].len() > max_value_bytes {
            return Err(input("An expected/replacement value exceeds --value-bytes"));
        }
        budget
            .charge(Resource::Steps, fields[2].len() as u64)
            .map_err(InspectError::from)?;
        let ordinal = fields[2]
            .parse()
            .map_err(|_| input("ROW must be a nonnegative original live-row ordinal"))?;
        let expected = values::update(&fields[3], &fields[4], budget)?;
        let value = values::update(&fields[3], &fields[5], budget)?;
        let mut found = None;
        for (position, group) in groups.iter().enumerate() {
            if same_name(group.table, &fields[0], budget, cancelled)?
                && same_name(group.column, &fields[1], budget, cancelled)?
            {
                found = Some(position);
                break;
            }
        }

        if let Some(position) = found {
            let group = &mut groups[position];
            reserve(&mut group.rows, 1, budget)?;
            group.rows.push(make(&fields[3], ordinal, expected, value));
        } else {
            let mut rows = Vec::new();
            reserve(&mut rows, 1, budget)?;
            rows.push(make(&fields[3], ordinal, expected, value));
            reserve(&mut groups, 1, budget)?;
            groups.push(Group {
                table: &fields[0],
                column: &fields[1],
                rows,
            });
        }
    }
    tick(budget, cancelled)?;
    Ok(groups)
}

pub(super) fn same_name(
    left: &str,
    right: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    tick(budget, cancelled)?;
    budget
        .charge(Resource::Steps, left.len().min(right.len()) as u64)
        .map_err(InspectError::from)?;
    Ok(left.eq_ignore_ascii_case(right))
}
