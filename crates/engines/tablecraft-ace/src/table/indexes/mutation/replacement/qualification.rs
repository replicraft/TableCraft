//! Close affected index profiles before reading replacement operands.
use super::super::super::{IndexCheck, IndexCheckStatus, encoded_tuple, profile::Coverage};
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::data_type::{LONG_TEXT, SHORT_TEXT},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table) fn qualify(
    schema: &TableSchema,
    indexes: &[IndexCheck],
    column: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for number in 0..schema.layout.physical_indexes {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, indexes.len() as u64)
            .map_err(InspectError::from)?;
        if !indexes
            .iter()
            .any(|index| index.index().data_number() == number as u32)
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    for checked in indexes {
        check(budget, cancelled)?;
        let index = checked.index();
        budget
            .charge(
                Resource::Steps,
                (indexes.len() + index.columns().len() * schema.columns.len()) as u64,
            )
            .map_err(InspectError::from)?;
        if !index
            .columns()
            .iter()
            .any(|key| key.number() == schema.columns[column].number())
        {
            continue;
        }
        if checked.status() != IndexCheckStatus::Checked
            || !index
                .constraints()
                .is_some_and(|constraint| !constraint.ignore_nulls())
            || indexes.iter().any(|other| {
                other.index().data_number() != index.data_number()
                    && other.index().root_page() == index.root_page()
            })
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }

        // Complete read membership never grants new Memo/digest/collation
        // mutation authority. Reject before projecting any sibling LVAL.
        for key in index.columns() {
            let field = schema
                .columns
                .iter()
                .find(|field| field.number() == key.number())
                .ok_or(InspectError::Malformed)?;
            if field.native_type() == LONG_TEXT
                || (field.native_type() == SHORT_TEXT
                    && field
                        .text_sort_order()
                        .and_then(|order| order.general_family())
                        .is_none())
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
        if index.columns().len() > 1 {
            encoded_tuple::EncodedTupleProfile::new(index, schema, Coverage::CompleteTable)?;
        }
    }
    Ok(())
}
