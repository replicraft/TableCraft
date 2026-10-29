//! Explicit ACE16 preview access to exclusively owned resolved record payloads.
use super::{Row, RowAssignment};
use crate::{
    EditError, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::{
        bytes::u16_at,
        page::PAGE_BYTES,
        row::{MASK_BITS, reference_page, reference_slot, slot_offset},
    },
    header::check,
    table::mutation::page_update::roots,
};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table::mutation) fn visit<'u, U: RowAssignment>(
    image: &NativeImage,
    schema: &TableSchema,
    updates: &'u [U],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    mut visitor: impl FnMut(Row<'_>, &'u U) -> Result<(), EditError>,
) -> Result<(), EditError> {
    // Capture resolves every live root before any callback. Selected targets
    // must belong to this table and have exactly one logical owner.
    let captured = roots::capture(image, schema, updates, budget, cancelled)?;
    for root in captured.selected {
        check(budget, cancelled)?;
        let data = &root.final_record.data;
        if usize::from(u16_at(data, 0)?) != schema.layout.max_columns {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let mask = data
            .len()
            .checked_sub(schema.layout.max_columns.div_ceil(MASK_BITS))
            .ok_or(InspectError::Malformed)?;
        visitor(
            Row {
                data,
                reference: root.reference,
                offset: root.final_record.offset,
                mask,
                // These are the logical root's directory fields, never the
                // hidden target's. Fixed scalar callbacks do not patch them.
                slot_offset: reference_page(root.reference) as usize * PAGE_BYTES
                    + slot_offset(usize::from(reference_slot(root.reference))),
                slot_word: root.word,
            },
            root.update,
        )?;
    }
    check(budget, cancelled)?;
    Ok(())
}
