//! Complete physical groups and logical aliases from fresh immutable metadata.
use super::{Profile, reserve};
use crate::{EditError, IndexCheckStatus, NativeIndex, TableCheck, format::reader::Reader};
use std::io::Cursor;
use tablecraft_schema::ScalarType;

pub(super) struct Selection<'a> {
    pub index: &'a NativeIndex,
    pub column: usize,
    pub profile: Profile,
}

pub(super) fn capture<'a>(
    checked: &'a TableCheck,
    primary: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<Selection<'a>>, EditError> {
    let mut result = Vec::new();
    reserve(
        &mut result,
        checked.schema().layout.physical_indexes,
        reader,
    )?;
    let mut primary_count = 0;
    for physical in 0..checked.schema().layout.physical_indexes {
        let mut selected: Option<&NativeIndex> = None;
        for entry in checked.indexes() {
            reader.tick()?;
            let index = entry.index();
            if entry.status() != IndexCheckStatus::Checked
                || index.native_type() > 1
                || index.data_number() as usize >= checked.schema().layout.physical_indexes
            {
                return Err(EditError::Preservation);
            }
            if index.data_number() as usize != physical {
                continue;
            }
            if let Some(prior) = selected {
                reader.work(index.columns().len())?;
                if index.root_page() != prior.root_page()
                    || index.usage_map() != prior.usage_map()
                    || index.flags() != prior.flags()
                    || index.columns() != prior.columns()
                {
                    return Err(EditError::Preservation);
                }
            } else {
                selected = Some(index);
            }
            let constraints = index.constraints().ok_or(EditError::Preservation)?;
            if constraints.primary_key() {
                primary_count += 1;
                let [key] = index.columns() else {
                    return Err(EditError::Preservation);
                };
                let field = checked
                    .schema()
                    .columns()
                    .get(primary)
                    .ok_or(EditError::Preservation)?;
                if key.number() != field.number()
                    || key.descending()
                    || field.scalar_type() != Some(ScalarType::Int32)
                    || !constraints.required()
                    || !constraints.unique()
                {
                    return Err(EditError::Preservation);
                }
            }
        }
        let index = selected.ok_or(EditError::Preservation)?;
        let [key] = index.columns() else {
            return Err(EditError::Preservation);
        };
        let mut column = None;
        for (position, field) in checked.schema().columns().iter().enumerate() {
            reader.tick()?;
            if field.number() == key.number() && column.replace(position).is_some() {
                return Err(EditError::Preservation);
            }
        }
        let column = column.ok_or(EditError::Preservation)?;
        result.push(Selection {
            index,
            column,
            profile: Profile::bind(index, &checked.schema().columns()[column])?,
        });
    }
    if primary_count != 1 {
        return Err(EditError::Preservation);
    }
    Ok(result)
}
