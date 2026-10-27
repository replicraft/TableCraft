//! Represented owned/candidate/global cells composed against one frozen image.
use super::super::{EditError, WriteBlocker};
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapCell, MapRole, RepresentedMap},
        bytes::u32_at,
        definition,
        layout::TABLE_FREE_MAP_OFFSET,
        reader::Reader,
    },
    header::check,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Edit {
    pub offset: usize,
    pub before: u8,
    pub after: u8,
    touched: u8,
}

pub(super) struct Maps {
    pub owned_pages: Vec<u32>,
    pub edits: Vec<Edit>,
    owned: RepresentedMap,
    hint: RepresentedMap,
    global: RepresentedMap,
    eof: u32,
}

impl Maps {
    pub(super) fn capture(
        image: &NativeImage,
        schema: &TableSchema,
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
        let hint_reference = u32_at(&metadata, TABLE_FREE_MAP_OFFSET)?;
        if hint_reference == 0 || hint_reference == schema.layout.usage {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        global.require_allocated_storage(&mut reader, &global)?;
        global.require_allocated(&mut reader, schema.definition)?;
        let owned = RepresentedMap::read(&mut reader, schema.layout.usage, MapRole::Owned)?;
        owned.require_allocated_storage(&mut reader, &global)?;
        let hint = RepresentedMap::read(&mut reader, hint_reference, MapRole::TableFreeSpace)?;
        hint.require_allocated_storage(&mut reader, &global)?;
        let owned_pages = allocation::pages(&mut reader, schema.layout.usage)?;
        for number in &owned_pages {
            global.require_allocated(&mut reader, *number)?;
        }
        Ok(Self {
            owned_pages,
            edits: Vec::new(),
            owned,
            hint,
            global,
            eof: reader.header.page_count(),
        })
    }

    fn current(
        &self,
        cell: &MapCell,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<bool, EditError> {
        reader.work(self.edits.len())?;
        let value = self
            .edits
            .iter()
            .find(|edit| edit.offset == cell.offset)
            .map_or(cell.before, |edit| edit.after);
        Ok(value & cell.mask != 0)
    }

    fn change(
        &mut self,
        cell: MapCell,
        set: bool,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.work(self.edits.len())?;
        if let Some(edit) = self
            .edits
            .iter_mut()
            .find(|edit| edit.offset == cell.offset)
        {
            if edit.before != cell.before || edit.touched & cell.mask != 0 {
                return Err(EditError::Preservation);
            }
            edit.after = if set {
                edit.after | cell.mask
            } else {
                edit.after & !cell.mask
            };
            edit.touched |= cell.mask;
        } else {
            reader
                .budget
                .charge_allocation(1, size_of::<Edit>() as u64)
                .map_err(InspectError::from)?;
            self.edits
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            self.edits.push(Edit {
                offset: cell.offset,
                before: cell.before,
                after: if set {
                    cell.before | cell.mask
                } else {
                    cell.before & !cell.mask
                },
                touched: cell.mask,
            });
        }
        Ok(())
    }

    /// Highest hinted owned page, including already staged EOF pages.
    pub(super) fn candidate(
        &self,
        image: &NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<u32>, EditError> {
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        for &number in self.owned_pages.iter().rev() {
            reader.tick()?;
            let cell = if number < self.eof {
                match self.hint.cell(&mut reader, number) {
                    Ok(cell) => cell,
                    Err(InspectError::Unsupported) => continue,
                    Err(error) => return Err(error.into()),
                }
            } else {
                self.hint.future_cell(&mut reader, number)?
            };
            if self.current(&cell, &mut reader)? {
                return Ok(Some(number));
            }
        }
        Ok(None)
    }

    pub(super) fn failed_fit(
        &mut self,
        image: &NativeImage,
        number: u32,
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
        let cell = if number < self.eof {
            self.hint.cell(&mut reader, number)?
        } else {
            self.hint.future_cell(&mut reader, number)?
        };
        if !self.current(&cell, &mut reader)? {
            return Err(EditError::Preservation);
        }
        // A page added in this same operation may later fail to fit. Its
        // provisional set and subsequent clear are one coherent staged intent.
        if let Some(edit) = self
            .edits
            .iter_mut()
            .find(|edit| edit.offset == cell.offset && edit.touched & cell.mask != 0)
        {
            edit.after &= !cell.mask;
            return Ok(());
        }
        self.change(cell, false, &mut reader)
    }

    pub(super) fn allocate(
        &mut self,
        image: &NativeImage,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        let expected = self
            .owned_pages
            .iter()
            .filter(|&&page| page >= self.eof)
            .count();
        let relative = number
            .checked_sub(self.eof)
            .ok_or(EditError::Preservation)?;
        if usize::try_from(relative).map_err(|_| InspectError::Malformed)? != expected {
            return Err(EditError::Preservation);
        }
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let owned = self.owned.future_cell(&mut reader, number)?;
        let hint = self.hint.future_cell(&mut reader, number)?;
        let global = self.global.future_cell(&mut reader, number)?;
        if self.current(&owned, &mut reader)?
            || self.current(&hint, &mut reader)?
            || !self.current(&global, &mut reader)?
        {
            return Err(InspectError::Malformed.into());
        }
        self.change(owned, true, &mut reader)?;
        self.change(hint, true, &mut reader)?;
        self.change(global, false, &mut reader)?;
        reader
            .budget
            .charge_allocation(1, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        self.owned_pages
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        self.owned_pages.push(number);
        Ok(())
    }

    pub(super) fn audit_bytes(
        &self,
        before: &[u8],
        after: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        for edit in &self.edits {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            if before.get(edit.offset) != Some(&edit.before)
                || after.get(edit.offset) != Some(&edit.after)
                || (edit.before ^ edit.after) & !edit.touched != 0
            {
                return Err(EditError::Preservation);
            }
        }
        Ok(())
    }

    pub(super) fn audit_membership(
        &self,
        after: &[u8],
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let mut source = Cursor::new(after);
        let header = crate::inspect_header(&mut source, budget, cancelled)?;
        let mut reader = Reader {
            source: &mut source,
            header,
            budget,
            cancelled,
            reads: 0,
        };
        let actual = allocation::pages(&mut reader, schema.layout.usage)?;
        reader.work(actual.len() + self.owned_pages.len())?;
        if actual != self.owned_pages {
            return Err(EditError::Preservation);
        }
        let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        global.require_allocated_storage(&mut reader, &global)?;
        let owned = RepresentedMap::read(&mut reader, schema.layout.usage, MapRole::Owned)?;
        owned.require_allocated_storage(&mut reader, &global)?;
        let metadata = definition::read(&mut reader, schema.definition)?;
        let hint_reference = u32_at(&metadata, TABLE_FREE_MAP_OFFSET)?;
        let hint = RepresentedMap::read(&mut reader, hint_reference, MapRole::TableFreeSpace)?;
        hint.require_allocated_storage(&mut reader, &global)?;
        for number in &actual {
            global.require_allocated(&mut reader, *number)?;
        }
        let mut candidates = allocation::PageCursor::new(&mut reader, hint_reference)?;
        while let Some(number) = candidates.next(&mut reader)? {
            reader.work(actual.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
            if actual.binary_search(&number).is_err() {
                return Err(EditError::Preservation);
            }
        }
        Ok(())
    }
}
