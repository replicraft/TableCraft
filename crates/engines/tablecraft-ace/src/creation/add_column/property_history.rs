//! Shared ordinary-row census over the actual qualified property-page ledger.
use crate::{
    EditError, InspectError, NativeImage, TableSchema,
    format::{
        reader::{Reader, row_count, row_range},
        row::OFFSET_MASK,
    },
    table::mutation::catalog_property,
};
use tablecraft_core::limits::ResourceBudget;
use thiserror::Error;

#[derive(Debug, Error)]
pub(super) enum PropertyHistoryError {
    #[error(transparent)]
    Inspect(#[from] InspectError),
    #[error(transparent)]
    Placement(#[from] EditError),
    #[error("source property history is outside the bounded ordinary-row profile")]
    Source,
}

pub(super) fn count(
    image: &NativeImage,
    table: &TableSchema,
    maximum: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, PropertyHistoryError> {
    let pages = catalog_property::repeated_property_pages(image, budget, cancelled)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut rows = 0usize;
    let mut last = None;
    for &number in &pages {
        let page = reader.page(number)?;
        let page_rows = row_count(&page)?;
        if page_rows == 0 {
            return Err(PropertyHistoryError::Source);
        }
        rows = rows
            .checked_add(page_rows)
            .ok_or(PropertyHistoryError::Source)?;
        if rows >= maximum {
            return Err(PropertyHistoryError::Source);
        }
        last = Some((number, page_rows - 1));
        for slot in 0..page_rows {
            reader.tick()?;
            let (word, start, end) = row_range(&page, slot)?;
            if word & !OFFSET_MASK != 0 || start == end {
                return Err(PropertyHistoryError::Source);
            }
        }
    }

    // Keep every original row/count/slot refusal ahead of the new role policy.
    // Total column rows are not implicitly user history in a composed source.
    let (page, slot) = last.ok_or(PropertyHistoryError::Source)?;
    let slot = u8::try_from(slot).map_err(|_| PropertyHistoryError::Source)?;
    let ledger =
        catalog_property::ledger::capture(image.as_bytes(), image.catalog(), budget, cancelled)?;
    if !ledger.sole_user_history(table.object_id(), &pages, page, slot, budget, cancelled)? {
        return Err(PropertyHistoryError::Source);
    }
    Ok(rows)
}
