//! Source-only union qualification, including every alias before owner dedup.
use crate::{
    EditError, IndexCheck, IndexCheckStatus, InspectError, NativeIndex, TableSchema, WriteBlocker,
    format::data_type::{LONG_TEXT, SHORT_TEXT},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table) fn qualify(
    schema: &TableSchema,
    indexes: &[IndexCheck],
    columns: &[usize],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for &column in columns {
        check(budget, cancelled)?;
        schema
            .columns()
            .get(column)
            .ok_or(InspectError::Malformed)?;
    }
    for number in 0..schema.layout.physical_indexes {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, indexes.len() as u64)
            .map_err(InspectError::from)?;
        if !indexes
            .iter()
            .any(|checked| checked.index().data_number() == number as u32)
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }

    for checked in indexes {
        check(budget, cancelled)?;
        let index = checked.index();
        if index.data_number() >= schema.layout.physical_indexes as u32 {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        work(budget, index.columns().len(), columns.len())?;
        if !affects(schema, index, columns)? {
            continue;
        }
        let constraints = index
            .constraints()
            .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        if checked.status() != IndexCheckStatus::Checked || constraints.ignore_nulls() {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }

        // All aliases must retain physical identity and effective physical
        // constraints. Logical primary/ordinary spelling may differ legitimately.
        for other in indexes {
            check(budget, cancelled)?;
            let other = other.index();
            if other.data_number() == index.data_number() {
                work(budget, index.columns().len(), 1)?;
                let other_constraints = other
                    .constraints()
                    .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
                if other.root_page() != index.root_page()
                    || other.usage_map() != index.usage_map()
                    || other.flags() != index.flags()
                    || other.columns() != index.columns()
                    || other_constraints.unique() != constraints.unique()
                    || other_constraints.required() != constraints.required()
                    || other_constraints.ignore_nulls() != constraints.ignore_nulls()
                {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
            } else if other.root_page() == index.root_page() {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }

        // Keep the existing slot profile's refusal before projecting Memo/digest
        // or unsupported collation siblings. Profile constructors run unchanged.
        work(budget, index.columns().len(), schema.columns().len())?;
        for key in index.columns() {
            let field = schema
                .columns()
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
        super::profiles::qualify(index, schema, budget, cancelled)?;
    }
    Ok(())
}

pub(in crate::table::indexes::mutation) fn affects(
    schema: &TableSchema,
    index: &NativeIndex,
    columns: &[usize],
) -> Result<bool, InspectError> {
    for &column in columns {
        let field = schema
            .columns()
            .get(column)
            .ok_or(InspectError::Malformed)?;
        if index
            .columns()
            .iter()
            .any(|key| key.number() == field.number())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn work(budget: &mut ResourceBudget, count: usize, width: usize) -> Result<(), EditError> {
    let work = count
        .checked_mul(width)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    Ok(())
}
