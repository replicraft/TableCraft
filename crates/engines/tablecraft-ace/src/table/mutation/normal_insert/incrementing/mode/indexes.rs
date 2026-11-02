//! Additional physical owners retain their actual manual present-I32 keys.
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{
        data_type::INT32,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
    },
    header::check,
    table::{indexes::page::LEAF, mutation::preserved_source::Source},
};
use tablecraft_core::{limits::ResourceBudget, values::Value};

pub(super) fn capture(
    image: &NativeImage,
    source: &Source,
    generated: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let schema = source.table.schema();
    let mut generated_owners = 0;
    let mut primaries = 0;

    for physical in 0..schema.layout.physical_indexes {
        check(budget, cancelled)?;
        let physical = u32::try_from(physical).map_err(|_| InspectError::StructureLimit)?;
        let mut first: Option<&crate::NativeIndex> = None;

        for index in source.indexes.indexes() {
            check(budget, cancelled)?;
            if index.data_number() != physical {
                continue;
            }
            let [key] = index.columns() else {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            };
            let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
            if index.native_type() > super::LOCAL_INDEX_KIND_MAX
                || key.descending()
                || constraints.ignore_nulls()
                || image
                    .as_bytes()
                    .get(index.root_page() as usize * PAGE_BYTES)
                    != Some(&LEAF)
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }

            if let Some(owner) = first {
                let [owner_key] = owner.columns() else {
                    return Err(EditError::Preservation);
                };
                if index.root_page() != owner.root_page()
                    || index.usage_map() != owner.usage_map()
                    || index.flags() != owner.flags()
                    || key.number() != owner_key.number()
                    || key.descending() != owner_key.descending()
                {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
            } else {
                first = Some(index);
            }

            if key.number() == schema.columns()[generated].number() {
                if !constraints.required() || !constraints.unique() {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
                primaries += usize::from(constraints.primary_key());
            } else if constraints.primary_key() {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }

        let owner = first.ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        let [key] = owner.columns() else {
            return Err(EditError::Preservation);
        };
        let field = schema
            .columns()
            .get(key.number())
            .ok_or(EditError::Preservation)?;
        if key.number() == schema.columns()[generated].number() {
            generated_owners += 1;
        } else if field.native_type() != INT32
            || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
            || field.extended_flags() != 0
            || field.storage_length() != size_of::<i32>()
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }

        for row in source.table.rows() {
            check(budget, cancelled)?;
            if !matches!(row.get(key.number()), Some(Value::Int32(_))) {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
    }

    if generated_owners != 1 || primaries != 1 {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }

    check(budget, cancelled)?;
    Ok(())
}
