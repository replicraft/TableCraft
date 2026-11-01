//! Retire one exhausted original DATA hint after adding its EOF successor.
use crate::{
    EditError, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::{
        allocation::{self, MapCell, MapRole, RepresentedMap},
        bytes::{u16_at, u32_at},
        definition,
        layout::TABLE_FREE_MAP_OFFSET,
        page::PAGE_BYTES,
        reader::Reader,
        row::{MAX_SLOTS, ROW_COUNT_OFFSET, SLOT_BYTES},
    },
    header::check,
};
use tablecraft_core::limits::ResourceBudget;

const FREE_OFFSET: usize = 2;

pub(super) struct Hint {
    address: usize,
    length: usize,
    old: MapCell,
    new: MapCell,
}

impl Hint {
    pub(super) fn capture(
        image: &NativeImage,
        schema: &TableSchema,
        record_bytes: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let definition = definition::read(&mut reader, schema.definition_page())?;
        let reference = u32_at(&definition, TABLE_FREE_MAP_OFFSET)?;
        let members = allocation::pages(&mut reader, reference)?;
        let [number] = members.as_slice() else {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        };
        let owned = allocation::pages(&mut reader, schema.layout.usage)?;
        reader.work(owned.len())?;
        if !owned.contains(number) {
            return Err(EditError::Preservation);
        }
        let page = reader.page(*number)?;
        let needed = record_bytes
            .checked_add(SLOT_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        if usize::from(u16_at(&page, ROW_COUNT_OFFSET)?) < MAX_SLOTS
            && needed <= usize::from(u16_at(&page, FREE_OFFSET)?)
        {
            return Err(EditError::Preservation);
        }
        let map = RepresentedMap::read(&mut reader, reference, MapRole::TableFreeSpace)?;
        let old = map.cell(&mut reader, *number)?;
        let new = map.future_cell(&mut reader, image.catalog().header().page_count())?;
        if old.before & old.mask == 0 || new.before & new.mask != 0 {
            return Err(EditError::Preservation);
        }
        Ok(Self {
            address: image.as_bytes().as_ptr() as usize,
            length: image.as_bytes().len(),
            old,
            new,
        })
    }

    pub(super) fn apply_after_data(
        self,
        image: &NativeImage,
        after: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        if self.address != image.as_bytes().as_ptr() as usize
            || self.length != image.as_bytes().len()
            || after.len() != self.length + 3 * PAGE_BYTES
        {
            return Err(EditError::Preservation);
        }
        let expected = if self.old.offset == self.new.offset {
            self.old.before | self.new.mask
        } else {
            self.old.before
        };
        if after.get(self.old.offset) != Some(&expected) {
            return Err(EditError::Preservation);
        }
        after[self.old.offset] &= !self.old.mask;
        Ok(())
    }
}
