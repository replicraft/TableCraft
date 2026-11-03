//! Fitting GUID replacement through the actual shared typed tree kernel.
use super::{IndexDataPatch, IndexKeyUpdate, normal_fitting, normal_index_audit::Ownership};
use crate::{
    EditError, InspectError, NativeImage, TableCheck,
    format::{data_type::GUID, reader::Reader},
    header::check,
    table::indexes::guid::{self, GuidIndexSelection, GuidProfile},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Guid,
};

const UPDATE_KEY_PASSES: usize = 2;
const CONSTRAINT_KEY_PASSES: usize = 2;

pub(in crate::table) fn patches(
    image: &NativeImage,
    checked: &TableCheck,
    ownership: &Ownership,
    updates: &[IndexKeyUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexDataPatch>, EditError> {
    let schema = checked.schema();
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut patches = Vec::new();
    for (position, selected) in checked.indexes().iter().enumerate() {
        reader.tick()?;
        reader.work(position)?;
        let index = selected.index();
        if checked.indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == index.data_number())
        {
            continue;
        }
        ownership.require(image, schema, index, reader.budget, cancelled)?;
        let [key] = index.columns() else {
            return Err(EditError::Preservation);
        };
        let column = key.number();
        let field = schema
            .columns()
            .get(column)
            .ok_or(EditError::Preservation)?;
        if field.native_type() != GUID {
            continue;
        }
        reader.work(updates.len())?;
        if !updates.iter().any(|update| update.column == column) {
            continue;
        }
        let mut affected = 0_usize;
        for update in updates {
            reader.tick()?;
            if update.column == column {
                affected = affected
                    .checked_add(1)
                    .ok_or(InspectError::StructureLimit)?;
            }
        }
        let work = affected
            .checked_mul(Guid::BYTE_LEN * UPDATE_KEY_PASSES)
            .ok_or(InspectError::StructureLimit)?;
        // The shared generic key-update pass performs an additional canonical
        // expected-key comparison and 16-byte copy before fitting serialization.
        reader
            .budget
            .charge(Resource::Steps, work as u64)
            .map_err(InspectError::from)?;
        let constraint_work = checked
            .rows()
            .checked_mul(Guid::BYTE_LEN * CONSTRAINT_KEY_PASSES)
            .ok_or(InspectError::StructureLimit)?;
        // The separate final constraint pass copies each present key and may
        // compare it with its predecessor. This is additional to sort work.
        reader
            .budget
            .charge(Resource::Steps, constraint_work as u64)
            .map_err(InspectError::from)?;
        let profile = GuidProfile {
            column,
            descending: key.descending(),
            selection: GuidIndexSelection::All,
        };
        normal_fitting::rewrite::<_, { guid::MAX_ENTRY_BYTES }>(
            image,
            &mut reader,
            index,
            schema,
            updates,
            &profile,
            &mut patches,
        )?;
    }
    check(reader.budget, cancelled)?;
    Ok(patches)
}
