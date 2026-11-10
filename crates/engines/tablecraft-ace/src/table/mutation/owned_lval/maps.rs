//! One represented-byte ledger for column LVAL and ordinary descriptor placement.
use super::super::{EditError, storage_scope::Scope};
use crate::{
    InspectError, LongValueMapReferences, NativeImage, TableSchema,
    format::{
        allocation::{MapCell, RepresentedMap},
        reader::Reader,
    },
    header::check,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::table::mutation) enum Family {
    Ordinary,
    LongValue,
}

pub(in crate::table::mutation) struct Edit {
    pub offset: usize,
    pub before: u8,
    pub after: u8,
}

/// Required state of an already represented immediate-EOF cell. This grants
/// no physical page authority; callers separately prove exact source/EOF scope.
#[derive(Clone, Copy)]
pub(in crate::table::mutation) enum EofAllocation {
    #[cfg(feature = "ace16-write-preview")]
    RequireFree,
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    RequireAllocated,
}

impl EofAllocation {
    fn expected_free(self) -> bool {
        match self {
            #[cfg(feature = "ace16-write-preview")]
            Self::RequireFree => true,
            #[cfg(all(feature = "journal", feature = "ace16-create"))]
            Self::RequireAllocated => false,
        }
    }
}

pub(in crate::table::mutation) struct Maps {
    pub ordinary: Scope,
    pub long_value: Scope,
    pub global: RepresentedMap,
    pub original_eof: u32,
    pub edits: Vec<Edit>,
    eof_allocation: EofAllocation,
}

impl Maps {
    pub fn pages(&self, family: Family) -> &[u32] {
        &self.scope(family).pages
    }

    pub fn capture_with_eof_policy(
        image: &NativeImage,
        schema: &TableSchema,
        references: LongValueMapReferences,
        eof_allocation: EofAllocation,
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
        Self::capture_from_reader(&mut reader, schema, references, eof_allocation)
    }

    /// Read the same actual map scopes from an immutable byte-view reader.
    /// Caller-specific page and source admission remains outside this observation.
    pub fn capture_from_reader(
        reader: &mut Reader<'_, Cursor<&[u8]>>,
        schema: &TableSchema,
        references: LongValueMapReferences,
        eof_allocation: EofAllocation,
    ) -> Result<Self, EditError> {
        let observed = super::observation::ObservedScopes::capture(reader, schema, references)?;
        Ok(Self {
            ordinary: observed.ordinary,
            long_value: observed.long_value,
            global: observed.global,
            original_eof: reader.header.page_count(),
            edits: Vec::new(),
            eof_allocation,
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
            || self.current(&global, &mut reader)? != self.eof_allocation.expected_free()
        {
            return Err(InspectError::Malformed.into());
        }
        self.change(owned, true, &mut reader)?;
        self.change(hint, true, &mut reader)?;
        // Preserve preview's original free->allocated transition and ordering.
        // Qualified canonical EOF cells already zero remain entirely unchanged.
        if self.eof_allocation.expected_free() {
            self.change(global, false, &mut reader)?;
        }
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
