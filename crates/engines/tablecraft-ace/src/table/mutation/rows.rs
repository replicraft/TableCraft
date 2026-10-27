use super::{EditError, WriteBlocker};
use crate::{
    InspectError, TableSchema,
    format::{
        allocation,
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, live_row_range, row_count},
        row::{MASK_BITS, OFFSET_MASK, reference},
    },
    header::check,
    limits::MAX_LIVE_ROWS,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

mod directory;

pub(super) use directory::Directory;

pub(super) trait RowAssignment {
    fn row(&self) -> usize;
}

pub(super) fn unique<U: RowAssignment>(
    updates: &[U],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if updates.len() > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }
    match updates.len() {
        0 => Ok(()),
        1 => check(budget, cancelled).map_err(EditError::from),
        _ => Directory::new(updates, budget, cancelled)?.require_unique(budget, cancelled),
    }
}

pub(super) struct Row<'a> {
    pub data: &'a [u8],
    pub reference: u32,
    pub offset: usize,
    pub mask: usize,
    #[cfg(feature = "ace16-write-preview")]
    pub slot_offset: usize,
    #[cfg(feature = "ace16-write-preview")]
    pub slot_word: u16,
}

pub(super) fn visit<'u, R: crate::NativeReadSource, U: RowAssignment>(
    reader: &mut Reader<'_, R>,
    schema: &TableSchema,
    updates: &'u [U],
    mut visitor: impl FnMut(Row<'_>, &'u U) -> Result<(), EditError>,
) -> Result<(), EditError> {
    visit_with_budget(reader, schema, updates, |_, row, update| {
        visitor(row, update)
    })
}

pub(super) fn visit_with_budget<'u, R: crate::NativeReadSource, U: RowAssignment>(
    reader: &mut Reader<'_, R>,
    schema: &TableSchema,
    updates: &'u [U],
    mut visitor: impl FnMut(&mut ResourceBudget, Row<'_>, &'u U) -> Result<(), EditError>,
) -> Result<(), EditError> {
    let mut directory = if updates.len() > 1 {
        Some(Directory::new(updates, reader.budget, reader.cancelled)?)
    } else {
        None
    };
    let pages = allocation::pages(reader, schema.layout.usage)?;
    let mut ordinal = 0;
    let mut matched = 0;
    for number in pages {
        let page = reader.page(number)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&page, OWNER_OFFSET)? != schema.definition
        {
            return Err(InspectError::Malformed.into());
        }
        for slot in 0..row_count(&page)? {
            reader.tick()?;
            let Some((flags, start, end)) = live_row_range(&page, slot)? else {
                continue;
            };
            reader
                .budget
                .charge(Resource::Rows, 1)
                .map_err(InspectError::from)?;
            let request = if let Some(directory) = &mut directory {
                directory.at(ordinal, reader.budget, reader.cancelled)?
            } else {
                reader.work(updates.len())?;
                updates
                    .first()
                    .filter(|update| update.row() == ordinal)
                    .map(|_| 0)
            };
            if let Some(request) = request {
                let update = &updates[request];
                if flags & !OFFSET_MASK != 0
                    || usize::from(u16_at(&page[start..end], 0)?) != schema.layout.max_columns
                {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
                let data = &page[start..end];
                let mask = data
                    .len()
                    .checked_sub(schema.layout.max_columns.div_ceil(MASK_BITS))
                    .ok_or(InspectError::Malformed)?;
                visitor(
                    reader.budget,
                    Row {
                        data,
                        reference: reference(number, slot as u8),
                        offset: number as usize * PAGE_BYTES + start,
                        mask,
                        #[cfg(feature = "ace16-write-preview")]
                        slot_offset: number as usize * PAGE_BYTES
                            + crate::format::row::slot_offset(slot),
                        #[cfg(feature = "ace16-write-preview")]
                        slot_word: flags,
                    },
                    update,
                )?;
                matched += 1;
            }
            ordinal += 1;
        }
    }
    if matched != updates.len() || ordinal != schema.declared_rows() as usize {
        return Err(EditError::Conflict);
    }
    Ok(())
}
