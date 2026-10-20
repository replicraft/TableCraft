use super::{
    EditError, Int32Update,
    patch::BytePatch,
    rows::{self, RowAssignment},
};
use crate::{
    InspectError,
    format::{
        reader::Reader,
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    table::schema::TableSchema,
};

pub(super) trait FixedAssignment<const WIDTH: usize>: RowAssignment {
    fn expected(&self) -> [u8; WIDTH];
    fn value(&self) -> [u8; WIDTH];
}

impl RowAssignment for Int32Update {
    fn row(&self) -> usize {
        self.row()
    }
}

impl FixedAssignment<{ size_of::<i32>() }> for Int32Update {
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

impl<const WIDTH: usize> BytePatch for Patch<WIDTH> {
    fn offset(&self) -> usize {
        self.offset
    }
    fn before(&self) -> &[u8] {
        &self.before
    }
    fn after(&self) -> &[u8] {
        &self.after
    }
}

pub(super) fn patches<R: crate::NativeReadSource, U: FixedAssignment<WIDTH>, const WIDTH: usize>(
    reader: &mut Reader<'_, R>,
    schema: &TableSchema,
    column: usize,
    updates: &[U],
) -> Result<Vec<Patch<WIDTH>>, EditError> {
    let field = &schema.columns[column].field;
    reader
        .budget
        .charge_allocation(updates.len() as u64, size_of::<Patch<WIDTH>>() as u64)
        .map_err(InspectError::from)?;
    let mut patches = Vec::new();
    patches
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    rows::visit(reader, schema, updates, |row, update| {
        let offset = COLUMN_COUNT_BYTES + field.offset;
        if offset + WIDTH > row.mask
            || row.data[row.mask + field.number / MASK_BITS] & (1 << (field.number % MASK_BITS))
                == 0
        {
            return Err(EditError::Conflict);
        }
        let before: [u8; WIDTH] = row.data[offset..offset + WIDTH]
            .try_into()
            .map_err(|_| InspectError::Malformed)?;
        if before != update.expected() {
            return Err(EditError::Conflict);
        }
        if update.expected() != update.value() {
            patches.push(Patch {
                reference: row.reference,
                offset: row.offset + offset,
                before,
                after: update.value(),
            });
        }
        Ok(())
    })?;
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
