//! Full original facts and catalog/LVAL ownership precede coordinated planning.
use crate::{
    EditError, InspectError, NativeImage, RowInsert, WriteBlocker,
    header::check,
    table::{
        indexes::mutation::{insertion, normal_index_audit::Ownership},
        mutation::{
            fixed_records::indexed::CheckedIndexes, lval_ownership::Inventory,
            preserved_source::Source,
        },
    },
};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn qualify<'image>(
    image: &'image NativeImage,
    source: &Source,
    table: &str,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CheckedIndexes<'image>, EditError> {
    if !super::selection::selected(image, source, requests, budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    // This source-qualified policy retains the primary's prefix0 framing.
    // The shared fitting repacker may otherwise choose new compression. That
    // distinct storage policy needs independent source/proof qualification.
    check(budget, cancelled)?;
    let count = source
        .records
        .len()
        .checked_add(requests.len())
        .ok_or(InspectError::StructureLimit)?;
    let primary_wire = size_of::<u8>() + size_of::<i32>() + size_of::<u32>();
    let active = count
        .checked_mul(primary_wire)
        .ok_or(InspectError::StructureLimit)?;
    if active > crate::format::page::PAGE_BYTES - crate::table::indexes::page::ENTRY_START {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let facts = CheckedIndexes::capture_actions(image, table, budget, cancelled)?;
    facts.require_source(image, source.table.schema(), budget, cancelled)?;
    if facts.checked().rows() != source.records.len()
        || source.table.rows().len() != source.records.len()
    {
        return Err(EditError::Preservation);
    }
    let ownership = Ownership::capture_integral_updates(image, facts.checked(), budget, cancelled)?;
    let inventory = Inventory::capture(image, budget, cancelled)?;
    for physical in 0..3 {
        check(budget, cancelled)?;
        let index = insertion::qualify_actions(facts.checked(), physical, budget, cancelled)?;
        let tree = ownership.require(image, source.table.schema(), index, budget, cancelled)?;
        if tree.pages() != [index.root_page()] {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        inventory.require_no_long_value_data_overlap(tree.pages(), budget, cancelled)?;
    }
    check(budget, cancelled)?;
    Ok(facts)
}
