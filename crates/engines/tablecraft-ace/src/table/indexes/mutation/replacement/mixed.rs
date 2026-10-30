//! One fitted original-to-joint-final transform per affected physical owner.
mod profiles;
mod qualification;
pub(in crate::table) use qualification::qualify;

use super::{RowKeyUpdate, rows};
use crate::format::reader::Reader;
use crate::table::indexes::mutation::IndexDataPatch;
use crate::{EditError, IndexCheck, InspectError, NativeImage, TableSchema};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table) fn patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    columns: &[usize],
    updates: &[RowKeyUpdate<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexDataPatch>, EditError> {
    qualify(schema, indexes, columns, budget, cancelled)?;

    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    rows::validate(schema, updates, &mut reader)?;
    let mut patches = Vec::new();

    // Physical ordering is independent of logical names and alias order. Every
    // alias was qualified before any owner is read or changed; no per-cell plan
    // can observe an intermediate Age/Name tuple.
    for number in 0..schema.layout.physical_indexes {
        reader.tick()?;
        reader.work(indexes.len())?;
        let checked = indexes
            .iter()
            .find(|checked| checked.index().data_number() == number as u32)
            .ok_or(InspectError::Malformed)?;
        let index = checked.index();
        reader.work(index.columns().len() * columns.len())?;
        if !qualification::affects(schema, index, columns)? {
            continue;
        }

        profiles::replace(image, schema, index, updates, &mut reader, &mut patches)?;
    }
    reader.tick()?;
    Ok(patches)
}
