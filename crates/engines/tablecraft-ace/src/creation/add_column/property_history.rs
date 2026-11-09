//! Shared ordinary-row census over the actual qualified property-page ledger.
use crate::{
    EditError, InspectError, NativeImage,
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
    for number in pages {
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
        for slot in 0..page_rows {
            reader.tick()?;
            let (word, start, end) = row_range(&page, slot)?;
            if word & !OFFSET_MASK != 0 || start == end {
                return Err(PropertyHistoryError::Source);
            }
        }
    }

    Ok(rows)
}
