//! Shared ACE16 access to exclusively owned resolved record payloads.
use super::{Row, RowAssignment};
#[cfg(feature = "ace16-write-preview")]
use crate::format::{
    page::PAGE_BYTES,
    row::{reference_page, reference_slot, slot_offset},
};
use crate::{
    EditError, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::{bytes::u16_at, row::MASK_BITS},
    header::check,
    table::mutation::record_roots as roots,
};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table::mutation) enum LayoutPolicy {
    Complete,
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    HistoricalPresent,
}

#[cfg(feature = "ace16-write-preview")]
pub(in crate::table::mutation) fn visit<'u, U: RowAssignment>(
    image: &NativeImage,
    schema: &TableSchema,
    updates: &'u [U],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    visitor: impl FnMut(Row<'_>, &'u U) -> Result<(), EditError>,
) -> Result<(), EditError> {
    visit_with_policy(
        image,
        schema,
        updates,
        LayoutPolicy::Complete,
        budget,
        cancelled,
        visitor,
    )
}

pub(in crate::table::mutation) fn visit_with_policy<'u, U: RowAssignment>(
    image: &NativeImage,
    schema: &TableSchema,
    updates: &'u [U],
    policy: LayoutPolicy,
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
        let count = usize::from(u16_at(data, 0)?);
        if matches!(policy, LayoutPolicy::Complete) && count != schema.layout.max_columns {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let mask = match &policy {
            LayoutPolicy::Complete => data
                .len()
                .checked_sub(schema.layout.max_columns.div_ceil(MASK_BITS))
                .ok_or(InspectError::Malformed)?,
            #[cfg(all(feature = "journal", feature = "ace16-create"))]
            LayoutPolicy::HistoricalPresent => {
                crate::table::record_layout::parse(data, schema, budget)?.mask_start
            }
        };
        visitor(
            Row {
                data,
                reference: root.reference,
                offset: root.final_record.offset,
                mask,
                // These are the logical root's directory fields, never the
                // hidden target's. Fixed scalar callbacks do not patch them.
                #[cfg(feature = "ace16-write-preview")]
                slot_offset: reference_page(root.reference) as usize * PAGE_BYTES
                    + slot_offset(usize::from(reference_slot(root.reference))),
                #[cfg(feature = "ace16-write-preview")]
                slot_word: root.word,
            },
            root.update,
        )?;
    }
    check(budget, cancelled)?;
    Ok(())
}
