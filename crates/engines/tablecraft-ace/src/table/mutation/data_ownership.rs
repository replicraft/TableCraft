//! Source-bound INDEX ownership excludes every selected ordinary DATA page.
use super::{EditError, WriteBlocker};
use crate::{
    NativeImage, NativeTableStorage, TableSchema,
    format::{allocation, layout, page::PAGE_NUMBER_MASK, reader::Reader, row::reference_page},
    header::check,
    table::indexes::definition::physical_owned_maps,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn require_index_disjointness(
    image: &NativeImage,
    selected: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;

    let data = {
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        // Derive membership from this immutable source, not staged/caller pages.
        let layout = layout::read(&mut reader, selected.definition_page())?;
        allocation::pages(&mut reader, layout.usage)?
    };
    if data.is_empty() {
        return Ok(());
    }

    // allocation::pages certifies ascending, unique membership, so each binary
    // search below uses the original DATA map's validated sorted order.
    let comparison_work = (data.len().checked_ilog2().unwrap_or(0) as usize + 1) * size_of::<u32>();

    for origin in image.catalog().origins() {
        check(budget, cancelled)?;
        if origin.table_storage() != Some(NativeTableStorage::Local) {
            continue;
        }

        let definition = origin.native_id() as u32 & PAGE_NUMBER_MASK;
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let layout = layout::read(&mut reader, definition)?;
        // No logical/field-key policy is inferred for another table. Even an
        // orphan physical descriptor has an authoritative owned-map reference.
        for reference in physical_owned_maps(&mut reader, definition, &layout)? {
            reader.work(comparison_work)?;
            if data.binary_search(&reference_page(reference)).is_ok() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            for page in allocation::pages(&mut reader, reference)? {
                reader.work(comparison_work)?;
                if data.binary_search(&page).is_ok() {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
            }
        }
    }

    check(budget, cancelled)?;
    Ok(())
}
