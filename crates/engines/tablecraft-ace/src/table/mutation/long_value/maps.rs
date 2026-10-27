//! One represented-byte ledger for column LVAL and ordinary descriptor placement.
use super::super::{EditError, WriteBlocker};
use crate::{
    InspectError, LongValueMapReferences, NativeImage, TableSchema,
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

const INLINE_MAP: u8 = 0;
const INDIRECT_MAP: u8 = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    Ordinary,
    LongValue,
}

pub(super) struct Edit {
    pub offset: usize,
    pub before: u8,
    pub after: u8,
}

pub(super) struct Scope {
    pub owned: RepresentedMap,
    pub hint: RepresentedMap,
    pub pages: Vec<u32>,
    pub hint_reference: u32,
}

pub(super) struct Maps {
    pub ordinary: Scope,
    pub long_value: Scope,
    pub global: RepresentedMap,
    pub original_eof: u32,
    pub edits: Vec<Edit>,
}

impl Scope {
    fn capture(
        reader: &mut Reader<'_, Cursor<&[u8]>>,
        owned_reference: u32,
        hint_reference: u32,
        role: MapRole,
        global: &RepresentedMap,
    ) -> Result<Self, EditError> {
        if owned_reference == 0 || hint_reference == 0 || owned_reference == hint_reference {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let owned = RepresentedMap::read(reader, owned_reference, MapRole::Owned)?;
        let hint = RepresentedMap::read(reader, hint_reference, role)?;
        owned.require_allocated_storage(reader, global)?;
        hint.require_allocated_storage(reader, global)?;
        let pages = allocation::pages(reader, owned_reference)?;
        let candidates = allocation::pages(reader, hint_reference)?;
        let levels = pages.len().checked_ilog2().unwrap_or(0) as usize + 1;
        for candidate in candidates {
            reader.tick()?;
            reader.work(levels)?;
            if pages.binary_search(&candidate).is_err() {
                return Err(InspectError::Malformed.into());
            }
        }
        for &number in &pages {
            global.require_allocated(reader, number)?;
        }

        Ok(Self {
            owned,
            hint,
            pages,
            hint_reference,
        })
    }
}

impl Maps {
    pub fn pages(&self, family: Family) -> &[u32] {
        &self.scope(family).pages
    }

    pub fn capture(
        image: &NativeImage,
        schema: &TableSchema,
        references: LongValueMapReferences,
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
        let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        global.require_allocated_storage(&mut reader, &global)?;
        global.require_allocated(&mut reader, schema.definition_page())?;
        let metadata = definition::read(&mut reader, schema.definition_page())?;
        let ordinary_hint = u32_at(&metadata, TABLE_FREE_MAP_OFFSET)?;
        let ordinary = Scope::capture(
            &mut reader,
            schema.layout.usage,
            ordinary_hint,
            MapRole::TableFreeSpace,
            &global,
        )?;
        let long_value = Scope::capture(
            &mut reader,
            references.owned(),
            references.candidate(),
            MapRole::LongValueCandidate,
            &global,
        )?;
        if !matches!(long_value.owned.kind(), INLINE_MAP | INDIRECT_MAP)
            || long_value.hint.kind() != INLINE_MAP
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        for &number in &long_value.pages {
            reader.tick()?;
            reader.work(ordinary.pages.len())?;
            if ordinary.pages.contains(&number) {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }

        Ok(Self {
            ordinary,
            long_value,
            global,
            original_eof: reader.header.page_count(),
            edits: Vec::new(),
        })
    }

    fn scope(&self, family: Family) -> &Scope {
        match family {
            Family::Ordinary => &self.ordinary,
            Family::LongValue => &self.long_value,
        }
    }

    fn scope_mut(&mut self, family: Family) -> &mut Scope {
        match family {
            Family::Ordinary => &mut self.ordinary,
            Family::LongValue => &mut self.long_value,
        }
    }

    fn cell(
        &self,
        map: &RepresentedMap,
        number: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<MapCell, EditError> {
        if number < self.original_eof {
            Ok(map.cell(reader, number)?)
        } else {
            Ok(map.future_cell(reader, number)?)
        }
    }

    fn current(
        &self,
        cell: &MapCell,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<bool, EditError> {
        reader.work(self.edits.len())?;
        let byte = self
            .edits
            .iter()
            .find(|edit| edit.offset == cell.offset)
            .map_or(cell.before, |edit| edit.after);
        Ok(byte & cell.mask != 0)
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
            if edit.before != cell.before {
                return Err(EditError::Preservation);
            }
            edit.after = if set {
                edit.after | cell.mask
            } else {
                edit.after & !cell.mask
            };
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
            });
        }

        Ok(())
    }

    pub fn candidate(
        &self,
        image: &NativeImage,
        family: Family,
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
        let scope = self.scope(family);
        for &number in scope.pages.iter().rev() {
            reader.tick()?;
            let cell = match self.cell(&scope.hint, number, &mut reader) {
                Ok(cell) => cell,
                Err(EditError::Inspect(InspectError::Unsupported)) => continue,
                Err(error) => return Err(error),
            };
            if self.current(&cell, &mut reader)? {
                return Ok(Some(number));
            }
        }

        Ok(None)
    }

    pub fn failed_fit(
        &mut self,
        image: &NativeImage,
        family: Family,
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
        let cell = self.cell(&self.scope(family).hint, number, &mut reader)?;
        if !self.current(&cell, &mut reader)? {
            return Err(EditError::Preservation);
        }
        self.change(cell, false, &mut reader)
    }

    pub fn allocate(
        &mut self,
        image: &NativeImage,
        family: Family,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                (self.ordinary.pages.len() + self.long_value.pages.len()) as u64,
            )
            .map_err(InspectError::from)?;
        let added = self
            .ordinary
            .pages
            .iter()
            .chain(&self.long_value.pages)
            .filter(|&&page| page >= self.original_eof)
            .count();
        let expected = self
            .original_eof
            .checked_add(u32::try_from(added).map_err(|_| InspectError::StructureLimit)?)
            .ok_or(InspectError::StructureLimit)?;
        if number != expected {
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
        let scope = self.scope(family);
        let owned = self.cell(&scope.owned, number, &mut reader)?;
        let hint = self.cell(&scope.hint, number, &mut reader)?;
        let global = self.cell(&self.global, number, &mut reader)?;
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
        let scope = self.scope_mut(family);
        scope
            .pages
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        scope.pages.push(number);

        Ok(())
    }
}
