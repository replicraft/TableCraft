use super::{EditError, Int32Update, WriteBlocker};
use crate::{
    InspectError,
    format::{
        allocation,
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::{COLUMN_COUNT_BYTES, DELETED_MASK, MASK_BITS, OFFSET_MASK, reference},
    },
    table::schema::TableSchema,
};
use std::io::{Read, Seek};
use tablecraft_core::limits::Resource;

pub(super) trait FixedAssignment<const WIDTH: usize> {
    fn row(&self) -> usize;
    fn expected(&self) -> [u8; WIDTH];
    fn value(&self) -> [u8; WIDTH];
}

impl FixedAssignment<{ size_of::<i32>() }> for Int32Update {
    fn row(&self) -> usize {
        self.row()
    }
    fn expected(&self) -> [u8; size_of::<i32>()] {
        self.expected().to_le_bytes()
    }
    fn value(&self) -> [u8; size_of::<i32>()] {
        self.value().to_le_bytes()
    }
}

pub(super) struct Patch<const WIDTH: usize> {
    pub reference: u32,
    pub offset: usize,
    pub before: [u8; WIDTH],
    pub after: [u8; WIDTH],
}

pub(super) fn patches<R: Read + Seek, U: FixedAssignment<WIDTH>, const WIDTH: usize>(
    reader: &mut Reader<'_, R>,
    schema: &TableSchema,
    column: usize,
    updates: &[U],
) -> Result<Vec<Patch<WIDTH>>, EditError> {
    let field = &schema.columns[column].field;
    let pages = allocation::pages(reader, schema.layout.usage)?;
    reader
        .budget
        .charge_allocation(updates.len() as u64, size_of::<Patch<WIDTH>>() as u64)
        .map_err(InspectError::from)?;
    let mut patches = Vec::new();
    patches
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
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
            let (flags, start, end) = row_range(&page, slot)?;
            if flags & DELETED_MASK != 0 {
                continue;
            }
            reader
                .budget
                .charge(Resource::Rows, 1)
                .map_err(InspectError::from)?;
            reader.work(updates.len())?;
            if let Some(update) = updates.iter().find(|update| update.row() == ordinal) {
                // No pointer following or row resize in this physical profile.
                if flags & !OFFSET_MASK != 0
                    || usize::from(u16_at(&page[start..end], 0)?) != schema.layout.max_columns
                {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
                let mask_bytes = schema.layout.max_columns.div_ceil(MASK_BITS);
                let mask = end.checked_sub(mask_bytes).ok_or(InspectError::Malformed)?;
                let offset = start + COLUMN_COUNT_BYTES + field.offset;
                if offset + WIDTH > mask
                    || page[mask + field.number / MASK_BITS] & (1 << (field.number % MASK_BITS))
                        == 0
                {
                    return Err(EditError::Conflict);
                }
                let before: [u8; WIDTH] = page[offset..offset + WIDTH]
                    .try_into()
                    .map_err(|_| InspectError::Malformed)?;
                if before != update.expected() {
                    return Err(EditError::Conflict);
                }
                matched += 1;
                if update.expected() != update.value() {
                    patches.push(Patch {
                        reference: reference(number, slot as u8),
                        offset: number as usize * PAGE_BYTES + offset,
                        before,
                        after: update.value(),
                    });
                }
            }
            ordinal += 1;
        }
    }
    if matched != updates.len() || ordinal != schema.declared_rows() as usize {
        return Err(EditError::Conflict);
    }
    reader.work(
        patches
            .len()
            .saturating_mul(patches.len().checked_ilog2().unwrap_or(0) as usize + 1),
    )?;
    patches.sort_unstable_by_key(|patch| patch.offset);
    if patches
        .windows(2)
        .any(|pair| pair[0].offset + WIDTH > pair[1].offset)
    {
        return Err(EditError::Preservation);
    }
    Ok(patches)
}
