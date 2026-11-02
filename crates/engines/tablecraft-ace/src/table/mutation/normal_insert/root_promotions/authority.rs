//! One original-bound authority for all four future INDEX pages.
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableSchema, WriteBlocker,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapCell, MapRole, RepresentedMap},
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

struct Claim {
    cell: MapCell,
    set: bool,
}

pub(super) struct Authority {
    address: usize,
    length: usize,
    definition: u32,
    claims: Vec<Claim>,
}

impl Authority {
    pub(super) fn capture(
        image: &NativeImage,
        schema: &TableSchema,
        indexes: [&NativeIndex; 2],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        let length = image.as_bytes().len();
        let final_length = length
            .checked_add(4 * PAGE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        if !length.is_multiple_of(PAGE_BYTES) || final_length as u64 > MAX_IMAGE_BYTES {
            return Err(InspectError::StructureLimit.into());
        }
        let first = u32::try_from(length / PAGE_BYTES).map_err(|_| InspectError::StructureLimit)?;
        let last = first.checked_add(3).ok_or(InspectError::StructureLimit)?;
        if first != image.catalog().header().page_count() || last & !PAGE_NUMBER_MASK != 0 {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let definition = schema.definition_page();
        let owners = [
            Owner::GlobalFree,
            Owner::IndexOwned {
                definition,
                physical: indexes[0].data_number(),
            },
            Owner::IndexOwned {
                definition,
                physical: indexes[1].data_number(),
            },
        ];
        let references = [
            GLOBAL_FREE_REFERENCE,
            indexes[0].usage_map(),
            indexes[1].usage_map(),
        ];
        let roles = [MapRole::GlobalFree, MapRole::Owned, MapRole::Owned];
        let mut storage = super::vector(0, budget, cancelled)?;
        let mut records = super::vector(3, budget, cancelled)?;
        let mut claims: Vec<Claim> = super::vector(8, budget, cancelled)?;
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let global = RepresentedMap::read(&mut reader, references[0], roles[0])?;
        global.require_allocated_storage(&mut reader, &global)?;
        for ordinal in 0..3 {
            reader.tick()?;
            let map =
                RepresentedMap::read_storage(&mut reader, references[ordinal], roles[ordinal])?;
            for &page in &map.pages {
                reader.tick()?;
                reader.work(storage.len())?;
                if !storage.contains(&page) {
                    reader
                        .budget
                        .charge_allocation((storage.len() + 1) as u64, size_of::<u32>() as u64)
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
                owner: owners[ordinal],
                reference: references[ordinal],
                range: map.record,
            });
            if ordinal == 0 {
                continue;
            }
            let owned = RepresentedMap::read(&mut reader, references[ordinal], roles[ordinal])?;
            owned.require_allocated_storage(&mut reader, &global)?;
            let right = first + ((ordinal - 1) * 2) as u32;
            for number in [right, right + 1] {
                for (map, set) in [(&global, false), (&owned, true)] {
                    reader.tick()?;
                    let cell = map.future_cell(&mut reader, number)?;
                    if cell.mask.count_ones() != 1
                        || image.as_bytes().get(cell.offset) != Some(&cell.before)
                        || (cell.before & cell.mask != 0) == set
                    {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                    reader.work(claims.len())?;
                    for old in &claims {
                        if old.cell.offset == cell.offset
                            && (old.cell.before != cell.before || old.cell.mask & cell.mask != 0)
                        {
                            return Err(EditError::Preservation);
                        }
                    }
                    claims.push(Claim { cell, set });
                }
            }
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
                for number in first..=last {
                    check_role(local.layout.usage, MapRole::Owned, number, &storage, reader)?;
                    let raw = definition::read(reader, local.definition_page())?;
                    let hint = u32_at(&raw, TABLE_FREE_MAP_OFFSET)?;
                    if hint != 0 {
                        check_role(hint, MapRole::TableFreeSpace, number, &[], reader)?;
                    }
                    for selected in indexes {
                        check_role(
                            selected.usage_map(),
                            MapRole::Owned,
                            number,
                            &storage,
                            reader,
                        )?;
                    }
                    for declaration in &declarations.entries {
                        check_role(
                            declaration.references.owned(),
                            MapRole::Owned,
                            number,
                            &storage,
                            reader,
                        )?;
                        check_role(
                            declaration.references.candidate(),
                            MapRole::LongValueCandidate,
                            number,
                            &[],
                            reader,
                        )?;
                    }
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
        Ok(Self {
            address: image.as_bytes().as_ptr() as usize,
            length,
            definition,
            claims,
        })
    }

    pub(super) fn apply(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        after: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.address != image.as_bytes().as_ptr() as usize
            || self.length != image.as_bytes().len()
            || self.definition != schema.definition_page()
            || after.len() != self.length + 4 * PAGE_BYTES
        {
            return Err(EditError::Preservation);
        }
        // Validate the complete original-byte set before touching any shared byte.
        for claim in &self.claims {
            check(budget, cancelled)?;
            if after.get(claim.cell.offset) != Some(&claim.cell.before) {
                return Err(EditError::Preservation);
            }
        }
        for claim in &self.claims {
            check(budget, cancelled)?;
            let target = &mut after[claim.cell.offset];
            *target = if claim.set {
                *target | claim.cell.mask
            } else {
                *target & !claim.cell.mask
            };
        }
        Ok(())
    }
}

fn check_role(
    reference: u32,
    role: MapRole,
    number: u32,
    protected: &[u32],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    let map = RepresentedMap::read(reader, reference, role)?;
    match map.future_cell(reader, number) {
        Ok(cell) if cell.before & cell.mask != 0 => {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        Ok(_) | Err(InspectError::Unsupported) => {}
        Err(error) => return Err(error.into()),
    }
    for page in allocation::pages(reader, reference)? {
        reader.tick()?;
        reader.work(protected.len())?;
        if protected.contains(&page) {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }
    Ok(())
}
