//! Complete original map rewrite ownership and independent retirement bit replay.
use super::vector;
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u32_at,
        definition,
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    table::{
        indexes::mutation::retirement::RetiredPage,
        mutation::{
            lval_ownership::Inventory,
            map_authority::{self, Owner, RecordIdentity},
        },
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn require_rewrites(
    image: &NativeImage,
    checked: &TableCheck,
    retired: &[RetiredPage],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if retired.is_empty() {
        return Ok(());
    }
    let schema = checked.schema();
    let count = retired
        .len()
        .checked_add(1)
        .ok_or(InspectError::StructureLimit)?;
    let mut owners = vector(count, budget, cancelled)?;
    let mut references = vector(count, budget, cancelled)?;
    owners.push(Owner::GlobalFree);
    references.push(GLOBAL_FREE_REFERENCE);
    for page in retired {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, checked.indexes().len() as u64)
            .map_err(InspectError::from)?;
        let index = checked
            .indexes()
            .iter()
            .find(|entry| entry.index().data_number() == page.physical)
            .ok_or(EditError::Preservation)?
            .index();
        if page.root != index.root_page()
            || page.usage != index.usage_map()
            || page.number == page.root
        {
            return Err(EditError::Preservation);
        }
        let owner = Owner::IndexOwned {
            definition: schema.definition_page(),
            physical: page.physical,
        };
        budget
            .charge(Resource::Steps, owners.len() as u64)
            .map_err(InspectError::from)?;
        if !owners.contains(&owner) {
            owners.push(owner);
            references.push(index.usage_map());
        }
    }
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    let mut storage = Vec::new();
    let mut records = vector(owners.len(), reader.budget, cancelled)?;
    for (position, (owner, reference)) in owners.iter().zip(&references).enumerate() {
        reader.tick()?;
        let role = if position == 0 {
            MapRole::GlobalFree
        } else {
            MapRole::Owned
        };
        let map = RepresentedMap::read_storage(&mut reader, *reference, role)?;
        let represented = RepresentedMap::read(&mut reader, *reference, role)?;
        represented.require_allocated_storage(&mut reader, &global)?;
        for number in &map.pages {
            reader.tick()?;
            reader.work(storage.len())?;
            if !storage.contains(number) {
                reader
                    .budget
                    .charge_allocation(1, size_of::<u32>() as u64)
                    .map_err(InspectError::from)?;
                reader.work(
                    storage
                        .len()
                        .checked_mul(size_of::<u32>())
                        .ok_or(InspectError::StructureLimit)?,
                )?;
                storage
                    .try_reserve_exact(1)
                    .map_err(|_| InspectError::Allocation)?;
                storage.push(*number);
            }
        }
        records.push(RecordIdentity {
            owner: *owner,
            reference: *reference,
            range: map.record,
        });
    }
    let census = map_authority::capture_catalog(
        image,
        reader.budget,
        cancelled,
        |local, indexes, declarations, reader| {
            definition::read_with_pages(reader, local.definition_page(), |page, reader| {
                reader.tick()?;
                reader.work(storage.len())?;
                if storage.contains(&page) {
                    return Err(InspectError::Unsupported);
                }
                Ok(())
            })?;
            disjoint(local.layout.usage, &storage, reader)?;
            for index in indexes {
                disjoint(index.usage_map(), &storage, reader)?;
            }
            for declaration in &declarations.entries {
                disjoint(declaration.references.owned(), &storage, reader)?;
            }
            Ok(())
        },
    )?;
    for record in records {
        census.require_record_rewrite(&owners, record, reader.budget, cancelled)?;
    }
    let live = Inventory::capture(image, reader.budget, cancelled)?;
    live.require_no_long_value_data_overlap(&storage, reader.budget, cancelled)?;
    check(reader.budget, cancelled)?;
    Ok(())
}

fn disjoint(
    reference: u32,
    storage: &[u32],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    for page in allocation::pages(reader, reference)? {
        reader.tick()?;
        reader.work(storage.len())?;
        if storage.contains(&page) {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }
    Ok(())
}

struct Toggle {
    offset: usize,
    before: u8,
    mask: u8,
}

pub(super) fn reconstruct(
    image: &NativeImage,
    checked: &TableCheck,
    retired: &[RetiredPage],
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    require_rewrites(image, checked, retired, budget, cancelled)?;
    let count = retired
        .len()
        .checked_mul(2)
        .ok_or(InspectError::StructureLimit)?;
    let mut changes: Vec<Toggle> = vector(count, budget, cancelled)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    for page in retired {
        reader.tick()?;
        let owned = RepresentedMap::read(&mut reader, page.usage, MapRole::Owned)?;
        for (position, map) in [&owned, &global].into_iter().enumerate() {
            let cell = map.cell(&mut reader, page.number)?;
            if (cell.before & cell.mask != 0) != (position == 0) || cell.mask.count_ones() != 1 {
                return Err(EditError::Preservation);
            }
            reader.work(changes.len())?;
            if let Some(change) = changes
                .iter_mut()
                .find(|change| change.offset == cell.offset)
            {
                if change.before != cell.before || change.mask & cell.mask != 0 {
                    return Err(EditError::Preservation);
                }
                change.mask |= cell.mask;
            } else {
                changes.push(Toggle {
                    offset: cell.offset,
                    before: cell.before,
                    mask: cell.mask,
                });
            }
        }
        // The independent graph proof selected this exact child page. No
        // producer patch or staged retired-page list supplies these bytes.
        let start = page.number as usize * PAGE_BYTES;
        reader.work(size_of::<u32>())?;
        expected
            .get_mut(start..start + size_of::<u32>())
            .ok_or(EditError::Preservation)?
            .fill(0);
        if u32_at(image.as_bytes(), start + size_of::<u32>())? != checked.schema().definition_page()
        {
            return Err(EditError::Preservation);
        }
    }
    for change in changes {
        reader.tick()?;
        let byte = expected
            .get_mut(change.offset)
            .ok_or(EditError::Preservation)?;
        if *byte != change.before {
            return Err(EditError::Preservation);
        }
        *byte ^= change.mask;
    }
    check(reader.budget, cancelled)?;
    Ok(())
}
