//! Two explicit represented INDEX EOF claims, sealed to the original image.
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

pub(super) struct Authority {
    address: usize,
    length: usize,
    definition: u32,
    physical: u32,
    cells: [[MapCell; 2]; 2],
}

impl Authority {
    pub(super) fn capture(
        image: &NativeImage,
        schema: &TableSchema,
        index: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let _span = tracing::debug_span!("native.ordinary.insert.root_split.authority").entered();
        check(budget, cancelled)?;
        let length = image.as_bytes().len();
        let final_length = length
            .checked_add(2 * PAGE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        if !length.is_multiple_of(PAGE_BYTES) || final_length as u64 > MAX_IMAGE_BYTES {
            return Err(InspectError::StructureLimit.into());
        }
        let right = u32::try_from(length / PAGE_BYTES).map_err(|_| InspectError::StructureLimit)?;
        let left = right.checked_add(1).ok_or(InspectError::StructureLimit)?;
        if right != image.catalog().header().page_count() || left & !PAGE_NUMBER_MASK != 0 {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let definition = schema.definition_page();
        let owners = [
            Owner::GlobalFree,
            Owner::IndexOwned {
                definition,
                physical: index.data_number(),
            },
        ];
        let references = [GLOBAL_FREE_REFERENCE, index.usage_map()];
        let roles = [MapRole::GlobalFree, MapRole::Owned];
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let mut storage = Vec::new();
        let mut records = Vec::new();
        reader
            .budget
            .charge_allocation(2, size_of::<RecordIdentity>() as u64)
            .map_err(InspectError::from)?;
        records
            .try_reserve_exact(2)
            .map_err(|_| InspectError::Allocation)?;
        for ordinal in 0..2 {
            reader.tick()?;
            let map =
                RepresentedMap::read_storage(&mut reader, references[ordinal], roles[ordinal])?;
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
                owner: owners[ordinal],
                reference: references[ordinal],
                range: map.record,
            });
        }
        let global = RepresentedMap::read(&mut reader, references[0], roles[0])?;
        let owned = RepresentedMap::read(&mut reader, references[1], roles[1])?;
        global.require_allocated_storage(&mut reader, &global)?;
        owned.require_allocated_storage(&mut reader, &global)?;
        let cells = [
            [
                global.future_cell(&mut reader, right)?,
                owned.future_cell(&mut reader, right)?,
            ],
            [
                global.future_cell(&mut reader, left)?,
                owned.future_cell(&mut reader, left)?,
            ],
        ];
        for pair in &cells {
            for (role, cell) in pair.iter().enumerate() {
                reader.tick()?;
                if cell.mask.count_ones() != 1
                    || image.as_bytes().get(cell.offset) != Some(&cell.before)
                    || (cell.before & cell.mask != 0) != (role == 0)
                {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
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
                for number in [right, left] {
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
            physical: index.data_number(),
            cells,
        })
    }

    pub(super) fn require(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        index: &NativeIndex,
    ) -> Result<(), EditError> {
        if self.address != image.as_bytes().as_ptr() as usize
            || self.length != image.as_bytes().len()
            || self.definition != schema.definition_page()
            || self.physical != index.data_number()
        {
            return Err(EditError::Preservation);
        }
        Ok(())
    }

    pub(super) fn apply(
        &self,
        image: &NativeImage,
        after: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.address != image.as_bytes().as_ptr() as usize
            || self.length != image.as_bytes().len()
            || after.len() != self.length + 2 * PAGE_BYTES
        {
            return Err(EditError::Preservation);
        }
        // Check all original cells first: two different page bits may share a byte.
        for pair in &self.cells {
            for cell in pair {
                check(budget, cancelled)?;
                if after.get(cell.offset) != Some(&cell.before) {
                    return Err(EditError::Preservation);
                }
            }
        }
        for pair in &self.cells {
            for (role, cell) in pair.iter().enumerate() {
                check(budget, cancelled)?;
                if role == 0 {
                    after[cell.offset] &= !cell.mask;
                } else {
                    after[cell.offset] |= cell.mask;
                }
            }
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
