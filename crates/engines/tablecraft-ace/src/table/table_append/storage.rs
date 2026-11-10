//! Complete physical role census, including declarations whose catalog root was retired.
mod pages;

use super::refused;
use crate::{
    EditError, InspectError, NativeImage, TableSchema,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u32_at,
        definition, layout,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES, TABLE_DEFINITION_SIGNATURE},
        reader::Reader,
        row::reference,
    },
    header::check,
    table::{indexes, long_value::maps},
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Header,
    Definition,
    Maps,
    Data,
    LongValue,
    Index,
}

struct Census {
    roles: Vec<Option<Role>>,
    references: Vec<u32>,
}

pub(super) struct Storage {
    pub catalog: Vec<u32>,
    pub maps: Vec<u32>,
}

pub(super) fn capture(
    image: &NativeImage,
    catalog: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Storage, EditError> {
    let result = physical(image, catalog, budget, cancelled)?;
    // Existing complete key/membership checking remains the authority for live
    // indexes. Retired roots below have their own empty-leaf storage contract.
    for (object, origin) in image
        .catalog()
        .catalog()
        .objects()
        .iter()
        .zip(image.catalog().origins())
    {
        check(budget, cancelled)?;
        if origin.table_storage() == Some(crate::NativeTableStorage::Local) {
            let checked = crate::check_table(
                &mut image.reader(),
                image.catalog().catalog().scope(),
                object.name().as_str(),
                budget,
                cancelled,
            )?;
            if checked
                .indexes()
                .iter()
                .any(|index| index.status() != crate::IndexCheckStatus::Checked)
            {
                return Err(refused());
            }
        }
    }
    Ok(result)
}

fn physical(
    image: &NativeImage,
    catalog: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Storage, EditError> {
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let count = reader.header.page_count() as usize;
    if image.as_bytes().len() != count.checked_mul(PAGE_BYTES).ok_or_else(refused)? {
        return Err(refused());
    }
    reader
        .budget
        .charge_allocation(count as u64, size_of::<Option<Role>>() as u64)
        .map_err(InspectError::from)?;
    let mut roles = Vec::new();
    roles
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    roles.resize(count, None);
    let mut census = Census {
        roles,
        references: Vec::new(),
    };
    census.claim(0, Role::Header)?;
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    global.require_allocated_storage(&mut reader, &global)?;
    census.map_storage(
        GLOBAL_FREE_REFERENCE,
        MapRole::GlobalFree,
        &global,
        &mut reader,
    )?;
    for number in 1..reader.header.page_count() {
        reader.tick()?;
        if census.roles[number as usize].is_some() {
            continue;
        }
        let page = reader.page(number)?;
        if page[..TABLE_DEFINITION_SIGNATURE.len()] == TABLE_DEFINITION_SIGNATURE {
            reader.work(image.catalog().origins().len())?;
            let live = image.catalog().origins().iter().any(|origin| {
                origin.type_code() == 1 && u32::try_from(origin.native_id()).ok() == Some(number)
            });
            census.definition(number, live, &global, &mut reader)?;
        }
    }
    let mut hosts = Vec::new();
    for number in 1..reader.header.page_count() {
        reader.tick()?;
        match census.roles[number as usize] {
            Some(Role::Maps) => {
                let page = reader.page(number)?;
                if page[..DATA_SIGNATURE.len()] == DATA_SIGNATURE {
                    let owner = u32_at(&page, OWNER_OFFSET)?;
                    let count = pages::framing(&page, owner, &mut reader)?;
                    if owner != 0 {
                        if number != crate::format::row::reference_page(GLOBAL_FREE_REFERENCE)
                            || owner != number
                        {
                            return Err(refused());
                        }
                        continue;
                    }
                    for slot in 0..count {
                        let slot = u8::try_from(slot).map_err(|_| refused())?;
                        let value = reference(number, slot);
                        let map = RepresentedMap::read(&mut reader, value, MapRole::Owned)?;
                        map.require_allocated_storage(&mut reader, &global)?;
                        for member in allocation::pages(&mut reader, value)? {
                            global.require_allocated(&mut reader, member)?;
                        }
                    }
                    push_page(&mut hosts, number, &mut reader)?;
                }
            }
            Some(_) => global.require_allocated(&mut reader, number)?,
            None => {
                let cell = global.cell(&mut reader, number)?;
                let page = reader.page(number)?;
                reader.work(page.len())?;
                if cell.before & cell.mask == 0 || page.iter().any(|byte| *byte != 0) {
                    return Err(refused());
                }
            }
        }
    }
    let catalog_pages = allocation::pages(&mut reader, catalog.layout.usage)?;
    for &number in &catalog_pages {
        reader.tick()?;
        if census.roles.get(number as usize) != Some(&Some(Role::Data)) {
            return Err(refused());
        }
        pages::framing(
            &reader.page(number)?,
            catalog.definition_page(),
            &mut reader,
        )?;
    }
    if hosts.is_empty() || catalog_pages.is_empty() {
        return Err(refused());
    }
    Ok(Storage {
        catalog: catalog_pages,
        maps: hosts,
    })
}

impl Census {
    fn claim(&mut self, number: u32, role: Role) -> Result<(), EditError> {
        let stored = self.roles.get_mut(number as usize).ok_or_else(refused)?;
        if stored.is_some() && !(*stored == Some(Role::Maps) && role == Role::Maps) {
            return Err(refused());
        }
        *stored = Some(role);
        Ok(())
    }

    fn map_storage(
        &mut self,
        value: u32,
        role: MapRole,
        global: &RepresentedMap,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Vec<u32>, EditError> {
        reader.work(self.references.len())?;
        if self.references.contains(&value) {
            return Err(refused());
        }
        let map = RepresentedMap::read(reader, value, role)?;
        map.require_allocated_storage(reader, global)?;
        let storage = RepresentedMap::read_storage(reader, value, role)?;
        for page in storage.pages {
            self.claim(page, Role::Maps)?;
        }
        reader
            .budget
            .charge_allocation(1, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        self.references
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        self.references.push(value);
        let members = if matches!(role, MapRole::GlobalFree) {
            Vec::new()
        } else {
            allocation::pages(reader, value)?
        };
        for &member in &members {
            global.require_allocated(reader, member)?;
        }
        Ok(members)
    }

    fn definition(
        &mut self,
        number: u32,
        live: bool,
        global: &RepresentedMap,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        definition::read_with_pages(reader, number, |page, reader| {
            reader.tick()?;
            self.claim(page, Role::Definition)
                .map_err(|error| match error {
                    EditError::Inspect(error) => error,
                    _ => InspectError::Unsupported,
                })
        })?;
        let layout = layout::read(reader, number)?;
        if !live && layout.records != 0 {
            return Err(refused());
        }
        let owned = self.map_storage(layout.usage, MapRole::Owned, global, reader)?;
        let metadata = definition::read(reader, number)?;
        let hint = u32_at(&metadata, layout::TABLE_FREE_MAP_OFFSET)?;
        let candidate = if hint == 0 {
            Vec::new()
        } else {
            self.map_storage(hint, MapRole::TableFreeSpace, global, reader)?
        };
        for &page in &candidate {
            reader.work(owned.len())?;
            if !owned.contains(&page) {
                return Err(refused());
            }
        }
        let mut rows = 0_u32;
        for &page in &owned {
            self.claim(page, Role::Data)?;
            rows = rows
                .checked_add(pages::ordinary(
                    page,
                    number,
                    layout.max_columns,
                    &owned,
                    reader,
                )?)
                .ok_or_else(refused)?;
        }
        if rows != layout.records {
            return Err(refused());
        }
        let storage = indexes::definition::physical_storage(reader, number, &layout)?;
        for (&map, &root) in storage.maps.iter().zip(&storage.roots) {
            let pages = self.map_storage(map, MapRole::Owned, global, reader)?;
            reader.work(pages.len())?;
            if !pages.contains(&root) {
                return Err(refused());
            }
            if !live && pages.as_slice() != [root] {
                return Err(refused());
            }
            for page in pages {
                self.claim(page, Role::Index)?;
                pages::index(page, number, reader)?;
                if !live {
                    pages::retired_index(page, reader)?;
                }
            }
        }
        let declared = maps::declarations_for_layout(reader, number, &layout, storage.tail)?;
        if !declared.complete {
            return Err(refused());
        }
        for column in declared.entries {
            let owned =
                self.map_storage(column.references.owned(), MapRole::Owned, global, reader)?;
            let candidate = self.map_storage(
                column.references.candidate(),
                MapRole::LongValueCandidate,
                global,
                reader,
            )?;
            for page in candidate {
                reader.work(owned.len())?;
                if !owned.contains(&page) {
                    return Err(refused());
                }
            }
            for page in owned {
                self.claim(page, Role::LongValue)?;
                let raw = reader.page(page)?;
                pages::framing(&raw, u32::from_le_bytes(*b"LVAL"), reader)?;
            }
        }
        Ok(())
    }
}

fn push_page(
    pages: &mut Vec<u32>,
    page: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader
        .budget
        .charge_allocation(1, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    pages.try_reserve(1).map_err(|_| InspectError::Allocation)?;
    pages.push(page);
    Ok(())
}
