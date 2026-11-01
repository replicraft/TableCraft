//! Contiguous represented EOF DATA claims against one immutable original image.
use crate::{
    EditError, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u32_at,
        definition,
        layout::TABLE_FREE_MAP_OFFSET,
        page::{PAGE_BYTES, PAGE_NUMBER_MASK},
        reader::Reader,
    },
    header::check,
    limits::MAX_IMAGE_BYTES,
    table::mutation::{
        lval_ownership::Inventory,
        map_authority::{self, Owner, RecordIdentity},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table::mutation) struct FuturePages {
    address: usize,
    length: usize,
    definition: u32,
    number: u32,
    count: usize,
    cells: Vec<Claim>,
}

struct Claim {
    offset: usize,
    before: u8,
    mask: u8,
    role: usize,
}

impl FuturePages {
    pub(in crate::table::mutation) fn capture(
        image: &NativeImage,
        schema: &TableSchema,
        count: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        let length = image.as_bytes().len();
        let additional = count
            .checked_mul(PAGE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        if count == 0 {
            return Err(EditError::Preservation);
        }
        let after = length
            .checked_add(additional)
            .ok_or(InspectError::StructureLimit)?;
        if !length.is_multiple_of(PAGE_BYTES)
            || u64::try_from(after).map_err(|_| InspectError::StructureLimit)? > MAX_IMAGE_BYTES
        {
            return Err(InspectError::StructureLimit.into());
        }
        let number =
            u32::try_from(length / PAGE_BYTES).map_err(|_| InspectError::StructureLimit)?;
        if number != image.catalog().header().page_count() || number & !PAGE_NUMBER_MASK != 0 {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let last = number
            .checked_add(u32::try_from(count - 1).map_err(|_| InspectError::StructureLimit)?)
            .ok_or(InspectError::StructureLimit)?;
        if last & !PAGE_NUMBER_MASK != 0 {
            return Err(InspectError::StructureLimit.into());
        }
        let definition = schema.definition_page();
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let raw = definition::read(&mut reader, definition)?;
        let hint = u32_at(&raw, TABLE_FREE_MAP_OFFSET)?;
        if hint == 0 || hint == schema.layout.usage {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let references = [GLOBAL_FREE_REFERENCE, schema.layout.usage, hint];
        let roles = [MapRole::GlobalFree, MapRole::Owned, MapRole::TableFreeSpace];
        let owners = [
            Owner::GlobalFree,
            Owner::OrdinaryOwned { definition },
            Owner::OrdinaryCandidate { definition },
        ];
        let mut storage = Vec::new();
        let mut records = Vec::new();
        reader
            .budget
            .charge_allocation(3, size_of::<RecordIdentity>() as u64)
            .map_err(InspectError::from)?;
        records
            .try_reserve_exact(3)
            .map_err(|_| InspectError::Allocation)?;
        for index in 0..3 {
            reader.tick()?;
            let map = RepresentedMap::read_storage(&mut reader, references[index], roles[index])?;
            for &page in &map.pages {
                reader.tick()?;
                reader.work(storage.len())?;
                if !storage.contains(&page) {
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
                    storage.push(page);
                }
            }
            records.push(RecordIdentity {
                owner: owners[index],
                reference: references[index],
                range: map.record,
            });
        }
        let global = RepresentedMap::read(&mut reader, references[0], roles[0])?;
        let owned = RepresentedMap::read(&mut reader, references[1], roles[1])?;
        let candidate = RepresentedMap::read(&mut reader, references[2], roles[2])?;
        for map in [&global, &owned, &candidate] {
            map.require_allocated_storage(&mut reader, &global)?;
        }
        let mut cells: Vec<Claim> = Vec::new();
        for number in number..=last {
            reader.tick()?;
            for (index, map) in [&global, &owned, &candidate].into_iter().enumerate() {
                reader.tick()?;
                let cell = map.future_cell(&mut reader, number)?;
                if cell.mask.count_ones() != 1
                    || image.as_bytes().get(cell.offset) != Some(&cell.before)
                    || (cell.before & cell.mask != 0) != (index == 0)
                {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
                let role = index;
                reader.work(cells.len())?;
                if let Some(prior) = cells.iter_mut().find(|prior| prior.offset == cell.offset) {
                    if prior.before != cell.before
                        || prior.role != role
                        || prior.mask & cell.mask != 0
                    {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                    prior.mask |= cell.mask;
                } else {
                    reader
                        .budget
                        .charge_allocation(1, size_of::<Claim>() as u64)
                        .map_err(InspectError::from)?;
                    reader.work(
                        cells
                            .len()
                            .checked_mul(size_of::<Claim>())
                            .ok_or(InspectError::StructureLimit)?,
                    )?;
                    cells
                        .try_reserve_exact(1)
                        .map_err(|_| InspectError::Allocation)?;
                    cells.push(Claim {
                        offset: cell.offset,
                        before: cell.before,
                        mask: cell.mask,
                        role,
                    });
                }
            }
        }
        let census = map_authority::capture_catalog(
            image,
            budget,
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
                check_role(
                    local.layout.usage,
                    MapRole::Owned,
                    number,
                    last,
                    &storage,
                    reader,
                )?;
                let raw = definition::read(reader, local.definition_page())?;
                let hint = u32_at(&raw, TABLE_FREE_MAP_OFFSET)?;
                if hint != 0 {
                    check_role(hint, MapRole::TableFreeSpace, number, last, &[], reader)?;
                }
                for index in indexes {
                    check_role(
                        index.usage_map(),
                        MapRole::Owned,
                        number,
                        last,
                        &storage,
                        reader,
                    )?;
                }
                for declaration in &declarations.entries {
                    check_role(
                        declaration.references.owned(),
                        MapRole::Owned,
                        number,
                        last,
                        &storage,
                        reader,
                    )?;
                    check_role(
                        declaration.references.candidate(),
                        MapRole::LongValueCandidate,
                        number,
                        last,
                        &[],
                        reader,
                    )?;
                }
                Ok(())
            },
        )?;
        for record in records {
            census.require_record_rewrite(&owners, record, budget, cancelled)?;
        }
        let live = Inventory::capture(image, budget, cancelled)?;
        live.require_no_long_value_data_overlap(&storage, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(Self {
            address: image.as_bytes().as_ptr() as usize,
            length,
            definition,
            number,
            count,
            cells,
        })
    }

    pub(in crate::table::mutation) fn require(
        &self,
        image: &NativeImage,
        definition: u32,
    ) -> Result<(), EditError> {
        if self.address != image.as_bytes().as_ptr() as usize
            || self.length != image.as_bytes().len()
            || self.definition != definition
            || self.number as usize * PAGE_BYTES != self.length
        {
            return Err(EditError::Preservation);
        }
        Ok(())
    }

    pub(in crate::table::mutation) fn count(&self) -> usize {
        self.count
    }

    pub(in crate::table::mutation) fn contains(&self, number: u32) -> bool {
        number >= self.number && ((number - self.number) as usize) < self.count
    }

    pub(in crate::table::mutation) fn apply(
        &self,
        image: &NativeImage,
        after: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.require(image, self.definition)?;
        if after.len() != self.length + self.count * PAGE_BYTES {
            return Err(EditError::Preservation);
        }
        for cell in &self.cells {
            check(budget, cancelled)?;
            let byte = after.get_mut(cell.offset).ok_or(EditError::Preservation)?;
            if *byte != cell.before {
                return Err(EditError::Preservation);
            }
            *byte = if cell.role == 0 {
                *byte & !cell.mask
            } else {
                *byte | cell.mask
            };
        }
        Ok(())
    }
}

fn check_role(
    reference: u32,
    role: MapRole,
    first: u32,
    last: u32,
    protected: &[u32],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    let map = RepresentedMap::read(reader, reference, role)?;
    for number in first..=last {
        reader.tick()?;
        match map.future_cell(reader, number) {
            Ok(cell) if cell.before & cell.mask != 0 => {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            Ok(_) | Err(InspectError::Unsupported) => {}
            Err(error) => return Err(error.into()),
        }
    }
    for member in allocation::pages(reader, reference)? {
        reader.tick()?;
        reader.work(protected.len())?;
        if protected.contains(&member) {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }
    Ok(())
}
