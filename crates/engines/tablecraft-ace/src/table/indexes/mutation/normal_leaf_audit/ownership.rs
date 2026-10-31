//! Fresh catalog-wide writable leaf-page qualification; no staged page authority.
use super::vector;
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, NativeIndex, TableCheck, TableSchema,
    WriteBlocker,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        definition,
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    table::{
        indexes::{
            header::IndexHeader,
            page::{LEAF, PAGE_VERSION},
        },
        mutation::map_authority::{self, Owner},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table) struct Ownership {
    address: usize,
    length: usize,
    definition: u32,
    leaves: Vec<(u32, u32)>,
}

fn protect(
    protected: &mut Vec<u32>,
    page: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), InspectError> {
    reader.tick()?;
    reader.work(protected.len())?;
    if !protected.contains(&page) {
        reader
            .budget
            .charge_allocation(1, size_of::<u32>() as u64)?;
        protected
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        protected.push(page);
    }
    Ok(())
}

fn members(
    protected: &mut Vec<u32>,
    reference: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), InspectError> {
    for page in allocation::pages(reader, reference)? {
        protect(protected, page, reader)?;
    }
    Ok(())
}

impl Ownership {
    pub(in crate::table) fn capture(
        image: &NativeImage,
        checked: &TableCheck,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        let schema = checked.schema();
        let selected = schema.definition_page();
        // No orphan physical descriptor or incompatible logical alias may hide
        // another mutable owner, including when the operation changes no keys.
        for number in 0..schema.layout.physical_indexes {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, checked.indexes().len() as u64)
                .map_err(InspectError::from)?;
            if !checked
                .indexes()
                .iter()
                .any(|check| check.index().data_number() as usize == number)
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
        let mut owners = vector(checked.indexes().len(), budget, cancelled)?;
        let mut leaves = vector(checked.indexes().len(), budget, cancelled)?;
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
        for (position, check) in checked.indexes().iter().enumerate() {
            reader.tick()?;
            let index = check.index();
            let constraints = index
                .constraints()
                .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
            if check.status() != IndexCheckStatus::Checked
                || index.native_type() > 1
                || constraints.ignore_nulls()
                || index.columns().is_empty()
                || index.data_number() as usize >= schema.layout.physical_indexes
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            reader.work(position)?;
            if let Some(prior) = checked.indexes()[..position]
                .iter()
                .map(|check| check.index())
                .find(|prior| prior.data_number() == index.data_number())
            {
                if prior.root_page() != index.root_page()
                    || prior.usage_map() != index.usage_map()
                    || prior.columns() != index.columns()
                    || prior.flags() != index.flags()
                {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
                continue;
            }
            let map = RepresentedMap::read(&mut reader, index.usage_map(), MapRole::Owned)?;
            map.require_allocated_storage(&mut reader, &global)?;
            let pages = allocation::pages(&mut reader, index.usage_map())?;
            if pages.as_slice() != [index.root_page()] {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            global.require_allocated(&mut reader, index.root_page())?;
            let raw = reader.page(index.root_page())?;
            let header = IndexHeader::read(&raw)?;
            if header.kind != LEAF
                || header.version != PAGE_VERSION
                || header.owner.get() != selected
                || !header.qualified_for_update(0)
                || header.prefix.get() != 0
                || header.previous.get() != 0
                || header.next.get() != 0
                || header.tail.get() != 0
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            reader.work(leaves.len())?;
            if leaves.iter().any(|&(_, page)| page == index.root_page()) {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            owners.push(Owner::IndexOwned {
                definition: selected,
                physical: index.data_number(),
            });
            leaves.push((index.data_number(), index.root_page()));
        }
        let mut protected = Vec::new();
        let census = map_authority::capture_catalog(
            image,
            reader.budget,
            cancelled,
            |local, indexes, declarations, reader| {
                protect(&mut protected, 0, reader)?;
                definition::read_with_pages(reader, local.definition_page(), |page, reader| {
                    protect(&mut protected, page, reader)
                })?;
                members(&mut protected, local.layout.usage, reader)?;
                if local.definition_page() != selected {
                    for index in indexes {
                        members(&mut protected, index.usage_map(), reader)?;
                    }
                }
                for declaration in &declarations.entries {
                    members(&mut protected, declaration.references.owned(), reader)?;
                }
                Ok(())
            },
        )?;
        census.require_exclusive(&owners, reader.budget, cancelled)?;
        census.protect_storage(&mut protected, &mut reader)?;
        for (_, page) in &leaves {
            reader.tick()?;
            reader.work(protected.len())?;
            if protected.contains(page) {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        check(reader.budget, cancelled)?;
        Ok(Self {
            address: image.as_bytes().as_ptr() as usize,
            length: image.as_bytes().len(),
            definition: selected,
            leaves,
        })
    }

    pub(in crate::table) fn require(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        index: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, self.leaves.len() as u64)
            .map_err(InspectError::from)?;
        if self.address != image.as_bytes().as_ptr() as usize
            || self.length != image.as_bytes().len()
            || self.definition != schema.definition_page()
            || !self
                .leaves
                .contains(&(index.data_number(), index.root_page()))
            || (index.root_page() as usize)
                .checked_mul(PAGE_BYTES)
                .and_then(|start| start.checked_add(PAGE_BYTES))
                .is_none_or(|end| end > self.length)
        {
            return Err(EditError::Preservation);
        }
        Ok(())
    }
}
