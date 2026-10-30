//! Original-source authority for one genuinely new physical index.
use super::{MultipleOwners, OwnerContext};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, WriteBlocker,
    format::{
        allocation::{
            self, GLOBAL_FREE_REFERENCE, INLINE_BITMAP_OFFSET, INLINE_MAP, MapRole,
            QualifiedInlineRecord, RepresentedMap,
        },
        bytes::u32_at,
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    limits::MAX_IMAGE_BYTES,
    table::mutation::{
        index_create::{SecondaryIndexCreate, source},
        map_authority::{Census, Owner, RecordIdentity},
    },
};
use std::ops::Range;
use tablecraft_core::limits::ResourceBudget;

const INLINE_BASE_OFFSET: usize = size_of::<u8>();
const MAP_RECORD_BYTES: usize =
    INLINE_BITMAP_OFFSET + QualifiedInlineRecord::PAGE_CAPACITY / u8::BITS as usize;

pub(in crate::table) struct FreshIndexOwner {
    original: MultipleOwners,
    definition: u32,
    physical: u32,
    logical: u32,
    container: u32,
    root: u32,
    bodies: usize,
    global: Range<usize>,
}

impl FreshIndexOwner {
    pub(in crate::table) fn capture(
        image: &NativeImage,
        table: &str,
        request: &SecondaryIndexCreate<'_>,
        bodies: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let source = source::qualify(image, table, request, budget, cancelled)?;
        if !matches!(bodies, 1 | 3) {
            return Err(EditError::Preservation);
        }
        let container = u32::try_from(image.as_bytes().len() / PAGE_BYTES)
            .map_err(|_| InspectError::StructureLimit)?;
        let root = container
            .checked_add(1)
            .ok_or(InspectError::StructureLimit)?;
        let end = root
            .checked_add(u32::try_from(bodies).map_err(|_| InspectError::StructureLimit)?)
            .ok_or(InspectError::StructureLimit)?;
        let length = end as usize * PAGE_BYTES;
        if end as usize > QualifiedInlineRecord::PAGE_CAPACITY || length as u64 > MAX_IMAGE_BYTES {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }

        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let mut pages = Vec::new();
        for checked in source.checked.indexes() {
            for page in allocation::pages(&mut reader, checked.index().usage_map())? {
                reader.tick()?;
                reader.work(pages.len())?;
                if !pages.contains(&page) {
                    reader
                        .budget
                        .charge_allocation(1, size_of::<u32>() as u64)
                        .map_err(InspectError::from)?;
                    pages.try_reserve(1).map_err(|_| InspectError::Allocation)?;
                    pages.push(page);
                }
            }
        }
        let original =
            MultipleOwners::capture(image, &[&source.checked], &pages, reader.budget, cancelled)?;
        let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        let storage =
            RepresentedMap::read_storage(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        let raw = image
            .as_bytes()
            .get(storage.record.clone())
            .ok_or(InspectError::Truncated)?;
        if global.kind() != INLINE_MAP
            || raw.len() != MAP_RECORD_BYTES
            || u32_at(raw, INLINE_BASE_OFFSET)? != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let census = original.census(image)?.ok_or(EditError::Preservation)?;
        census.require_record_rewrite(
            &[Owner::GlobalFree],
            RecordIdentity {
                owner: Owner::GlobalFree,
                reference: GLOBAL_FREE_REFERENCE,
                range: storage.record.clone(),
            },
            reader.budget,
            cancelled,
        )?;
        for number in container..end {
            let cell = global.future_cell(&mut reader, number)?;
            if cell.before & cell.mask == 0 {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        check(reader.budget, cancelled)?;
        Ok(Self {
            original,
            definition: source.layout.definition,
            physical: source.layout.physical_count,
            logical: source.layout.logical_count,
            container,
            root,
            bodies,
            global: storage.record,
        })
    }

    pub(in crate::table) fn definition(&self) -> u32 {
        self.definition
    }

    pub(in crate::table) fn physical(&self) -> u32 {
        self.physical
    }

    pub(in crate::table) fn logical(&self) -> u32 {
        self.logical
    }

    pub(in crate::table) fn container(&self) -> u32 {
        self.container
    }

    pub(in crate::table) fn root(&self) -> u32 {
        self.root
    }

    pub(in crate::table) fn usage(&self) -> u32 {
        self.container << u8::BITS
    }

    pub(in crate::table) fn bodies(&self) -> usize {
        self.bodies
    }

    pub(in crate::table) fn global_record(&self) -> Range<usize> {
        self.global.clone()
    }

    pub(in crate::table) fn require_body(&self, number: u32) -> Result<(), EditError> {
        if number < self.root || number - self.root >= self.bodies as u32 {
            return Err(EditError::Preservation);
        }
        Ok(())
    }
}

impl OwnerContext for FreshIndexOwner {
    fn single(&self) -> Result<u32, EditError> {
        Err(EditError::Preservation)
    }

    fn index_definition(
        &self,
        _: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<u32, EditError> {
        check(budget, cancelled)?;
        Err(EditError::Preservation)
    }

    fn census(&self, image: &NativeImage) -> Result<Option<&Census>, EditError> {
        // Delegates the existing immutable byte-buffer identity/length binding.
        self.original.census(image)
    }
}
