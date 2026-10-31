//! Fresh complete metadata footprints deny ordinary DATA payload overlap.
use super::super::{EditError, WriteBlocker, map_authority};
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        definition,
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Protection {
    address: usize,
    length: usize,
    definition: u32,
    data: Vec<u32>,
}

fn protect(
    pages: &mut Vec<u32>,
    number: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), InspectError> {
    reader.tick()?;
    reader.work(pages.len())?;
    if !pages.contains(&number) {
        reader
            .budget
            .charge_allocation(1, size_of::<u32>() as u64)?;
        pages
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        pages.push(number);
    }
    Ok(())
}

fn members(
    pages: &mut Vec<u32>,
    reference: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), InspectError> {
    for number in allocation::pages(reader, reference)? {
        protect(pages, number, reader)?;
    }
    Ok(())
}

impl Protection {
    pub fn capture(
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        let selected = schema.definition_page();
        let mut protected = Vec::new();
        let census = map_authority::capture_catalog(
            image,
            budget,
            cancelled,
            |local, indexes, declarations, reader| {
                protect(&mut protected, 0, reader)?;
                definition::read_with_pages(reader, local.definition_page(), |number, reader| {
                    protect(&mut protected, number, reader)
                })?;
                if local.definition_page() != selected {
                    members(&mut protected, local.layout.usage, reader)?;
                }
                for index in indexes {
                    members(&mut protected, index.usage_map(), reader)?;
                }
                for declaration in &declarations.entries {
                    members(&mut protected, declaration.references.owned(), reader)?;
                }
                Ok(())
            },
        )?;
        census.require_exclusive(
            &[map_authority::Owner::OrdinaryOwned {
                definition: selected,
            }],
            budget,
            cancelled,
        )?;
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        census.protect_storage(&mut protected, &mut reader)?;
        // The census above grants storage exclusion only. Qualify the selected
        // owned-map role separately before using membership as DATA authority.
        RepresentedMap::read(&mut reader, schema.layout.usage, MapRole::Owned)?;
        let data = allocation::pages(&mut reader, schema.layout.usage)?;
        let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        global.require_allocated_storage(&mut reader, &global)?;
        for &page in &data {
            reader.tick()?;
            global.require_allocated(&mut reader, page)?;
        }
        let comparison = data.len().checked_ilog2().unwrap_or(0) as usize + 1;
        for page in protected {
            reader.tick()?;
            reader.work(comparison)?;
            if data.binary_search(&page).is_ok() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        check(reader.budget, cancelled)?;
        Ok(Self {
            address: image.as_bytes().as_ptr() as usize,
            length: image.as_bytes().len(),
            definition: selected,
            data,
        })
    }

    pub fn require(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        offset: usize,
        width: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        if image.as_bytes().as_ptr() as usize != self.address
            || image.as_bytes().len() != self.length
            || schema.definition_page() != self.definition
            || width == 0
        {
            return Err(EditError::Preservation);
        }
        let end = offset.checked_add(width).ok_or(EditError::Preservation)?;
        if end > self.length || offset / PAGE_BYTES != (end - 1) / PAGE_BYTES {
            return Err(EditError::Preservation);
        }
        let page = u32::try_from(offset / PAGE_BYTES).map_err(|_| EditError::Preservation)?;
        budget
            .charge(
                Resource::Steps,
                (self.data.len().checked_ilog2().unwrap_or(0) as u64 + 1) * size_of::<u32>() as u64,
            )
            .map_err(InspectError::from)?;
        if self.data.binary_search(&page).is_err() {
            return Err(EditError::Preservation);
        }
        Ok(())
    }
}
