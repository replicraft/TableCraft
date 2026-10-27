//! Explicit candidate-hint authority, separate from owned/global map polarity.
use super::super::{EditError, WriteBlocker};
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, PageCursor, RepresentedMap},
        bytes::u32_at,
        definition,
        layout::TABLE_FREE_MAP_OFFSET,
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Space {
    definition: u32,
    hint: RepresentedMap,
    global: RepresentedMap,
    protected: Vec<u32>,
}

impl Space {
    pub(super) fn capture(
        image: &NativeImage,
        schema: &TableSchema,
        owned: &[u32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let metadata = definition::read(&mut reader, schema.definition)?;
        let reference = u32_at(&metadata, TABLE_FREE_MAP_OFFSET)?;
        if reference == 0 || reference == schema.layout.usage {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        global.require_allocated_storage(&mut reader, &global)?;
        global.require_allocated(&mut reader, schema.definition)?;
        let own_map = RepresentedMap::read(&mut reader, schema.layout.usage, MapRole::Owned)?;
        own_map.require_allocated_storage(&mut reader, &global)?;
        let hint = RepresentedMap::read(&mut reader, reference, MapRole::TableFreeSpace)?;
        hint.require_allocated_storage(&mut reader, &global)?;
        let mut cursor = PageCursor::new(&mut reader, reference)?;
        let levels = owned.len().checked_ilog2().unwrap_or(0) as usize + 1;
        while let Some(number) = cursor.next(&mut reader)? {
            reader.work(levels)?;
            if owned.binary_search(&number).is_err() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        let mut protected = Vec::new();
        global.protect_storage(&mut reader, &mut protected)?;
        own_map.protect_storage(&mut reader, &mut protected)?;
        hint.protect_storage(&mut reader, &mut protected)?;
        reader
            .budget
            .charge_allocation(1, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        protected
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        protected.push(schema.definition);
        Ok(Self {
            definition: schema.definition,
            hint,
            global,
            protected,
        })
    }

    pub(super) fn definition(&self) -> u32 {
        self.definition
    }

    pub(super) fn audit(
        &self,
        before: &[u8],
        after: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        for number in &self.protected {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, PAGE_BYTES as u64)
                .map_err(InspectError::from)?;
            let start = *number as usize * PAGE_BYTES;
            if before
                .get(start..start + PAGE_BYTES)
                .ok_or(EditError::Preservation)?
                != after
                    .get(start..start + PAGE_BYTES)
                    .ok_or(EditError::Preservation)?
            {
                return Err(EditError::Preservation);
            }
        }
        Ok(())
    }

    pub(super) fn require_page(
        &self,
        image: &NativeImage,
        number: u32,
        appending: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        self.global.require_allocated(&mut reader, number)?;
        if appending {
            // Capture already proved this immutable hint's grammar. An absent
            // represented cell denies this append profile; it is not a reason
            // to invent a hint or select a different page. Other failures keep
            // their original typed identity.
            let candidate = match self.hint.cell(&mut reader, number) {
                Ok(cell) => cell,
                Err(InspectError::Unsupported) => {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
                Err(error) => return Err(error.into()),
            };
            if candidate.before & candidate.mask == 0 {
                // False does not mean physically full. This bounded profile
                // will not invent membership or choose another candidate page.
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        Ok(())
    }
}
