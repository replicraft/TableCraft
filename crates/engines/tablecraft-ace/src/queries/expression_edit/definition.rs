//! Selected local SELECT grammar and exact unaffected fragment preservation.
use super::QueryFragmentExpressionEdit;
use crate::{
    QueryAttribute as A, QueryExpressionEditsError as Error, SavedQueryDefinition, header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const ORDINARY_QUERY_FLAGS: u32 = 0;
const SELECT_QUERY_TYPE: i16 = 1;
const SELECT_OPTION_FLAGS: i16 = 0;
const NATIVE_ORDER_BYTES: usize = size_of::<u32>();

pub(super) fn qualify(
    definition: &SavedQueryDefinition,
    edits: &[QueryFragmentExpressionEdit<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    if definition.origin().flags() != ORDINARY_QUERY_FLAGS || edits.len() > definition.rows().len()
    {
        return Err(Error::Definition);
    }
    let mut starts = 0;
    let mut types = 0;
    let mut flags = 0;
    let mut ends = 0;
    let mut columns = 0;
    let mut predicates = 0;
    for (position, row) in definition.rows().iter().enumerate() {
        check(budget, cancelled)?;
        if row.native_object_id() != definition.origin().native_id()
            || row.extra().is_some()
            || row
                .order()
                .is_none_or(|order| order.len() != NATIVE_ORDER_BYTES)
        {
            return Err(Error::Definition);
        }
        budget.charge(Resource::Steps, position as u64)?;
        if definition.rows()[..position].iter().any(|previous| {
            previous.attribute() == row.attribute() && previous.order() == row.order()
        }) {
            return Err(Error::Definition);
        }
        match row.attribute() {
            Some(A::Start) | Some(A::End) => {
                if row.attribute() == Some(A::Start) {
                    starts += 1;
                } else {
                    ends += 1;
                }
                if row.expression().is_some()
                    || row.flag().is_some()
                    || row.name1().is_some()
                    || row.name2().is_some()
                {
                    return Err(Error::Definition);
                }
            }
            Some(A::Type) | Some(A::Flags) => {
                let expected = if row.attribute() == Some(A::Type) {
                    types += 1;
                    SELECT_QUERY_TYPE
                } else {
                    flags += 1;
                    SELECT_OPTION_FLAGS
                };
                if row.flag() != Some(expected)
                    || row.expression().is_some()
                    || row.name1().is_some()
                    || row.name2().is_some()
                {
                    return Err(Error::Definition);
                }
            }
            Some(A::Table) => {
                if row.expression().is_some() || row.flag().is_some() || row.name1().is_none() {
                    return Err(Error::Definition);
                }
            }
            Some(A::Column) => {
                columns += 1;
                if row.expression().is_none() || row.flag().is_some() || row.name2().is_some() {
                    return Err(Error::Definition);
                }
            }
            Some(A::Where) => {
                predicates += 1;
                if row.expression().is_none()
                    || row.flag().is_some()
                    || row.name1().is_some()
                    || row.name2().is_some()
                {
                    return Err(Error::Definition);
                }
            }
            _ => return Err(Error::Definition),
        }
    }
    if (starts, types, flags, ends) != (1, 1, 1, 1) || columns == 0 || predicates > 1 {
        return Err(Error::Definition);
    }
    for edit in edits {
        check(budget, cancelled)?;
        if !matches!(edit.attribute, A::Column | A::Where)
            || edit.expected.is_empty()
            || edit.replacement.is_empty()
        {
            return Err(Error::Definition);
        }
        budget.charge(Resource::Steps, definition.rows().len() as u64)?;
        let mut selected = definition.rows().iter().filter(|row| {
            row.attribute() == Some(edit.attribute) && row.order() == Some(edit.order.as_slice())
        });
        let row = selected.next().ok_or(Error::Definition)?;
        if selected.next().is_some() || row.expression() != Some(edit.expected) {
            return Err(Error::Definition);
        }
    }
    Ok(())
}

pub(super) fn verify(
    before: &SavedQueryDefinition,
    after: &SavedQueryDefinition,
    edits: &[QueryFragmentExpressionEdit<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    if before.origin().native_id() != after.origin().native_id()
        || before.origin().parent_id() != after.origin().parent_id()
        || before.origin().page() != after.origin().page()
        || before.origin().slot() != after.origin().slot()
        || before.origin().flags() != after.origin().flags()
        || before.object().name() != after.object().name()
        || before.rows().len() != after.rows().len()
    {
        return Err(Error::Preservation);
    }
    for (old, fresh) in before.rows().iter().zip(after.rows()) {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, edits.len() as u64)?;
        let replacement = edits.iter().find(|edit| {
            old.attribute() == Some(edit.attribute) && old.order() == Some(edit.order.as_slice())
        });
        if old.native_object_id() != fresh.native_object_id()
            || old.attribute() != fresh.attribute()
            || old.flag() != fresh.flag()
            || old.extra() != fresh.extra()
            || old.name1() != fresh.name1()
            || old.name2() != fresh.name2()
            || old.order() != fresh.order()
            || fresh.expression()
                != replacement
                    .map(|edit| edit.replacement)
                    .or(old.expression())
        {
            return Err(Error::Preservation);
        }
    }
    Ok(())
}
