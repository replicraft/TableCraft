//! Parser-bound catalog descriptor publication, preserving record identities.
use crate::{
    InspectError, NativeImage,
    format::{
        bytes::u16_at,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{PhysicalRow, Reader, row_count, row_range},
        row::{
            OFFSET_MASK, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES, reference_page, slot_offset,
        },
    },
    header::check,
    table::{
        mutation::{
            catalog_property,
            current_schema::{Family, Table},
        },
        record_layout,
        table_append::Inventory,
    },
};
use std::ops::Range;
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use thiserror::Error;

const SINGLE_PROPERTY_TAG: u32 = 0x4000_0000;
const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();

#[derive(Debug, Error)]
pub(in crate::table::mutation) enum Error {
    #[error(transparent)]
    Inspect(#[from] InspectError),
    #[error(transparent)]
    Native(#[from] crate::EditError),
    #[error("native property descriptor source is unsupported")]
    Source,
    #[error("native property descriptor exceeds existing owned capacity")]
    Capacity,
    #[error("native property descriptor preservation check failed")]
    Preservation,
}

impl From<LimitError> for Error {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

impl From<Error> for crate::EditError {
    fn from(error: Error) -> Self {
        match error {
            Error::Inspect(error) => Self::Inspect(error),
            Error::Native(error) => error,
            Error::Source | Error::Capacity => Self::Unsupported(crate::WriteBlocker::RowLayout),
            Error::Preservation => Self::Preservation,
        }
    }
}

/// Exact expected page, derived from the original physical record and framing.
/// Admission of this page never permits arbitrary catalog replacement bytes.
pub(in crate::table::mutation) struct DescriptorPatch {
    page: Range<usize>,
    expected: Vec<u8>,
}

struct Geometry {
    page_base: usize,
    slot: usize,
    rows: usize,
    record: PhysicalRow,
    value_start: usize,
    variable_end_words: Range<usize>,
    mask: usize,
    bit: u8,
    footer: usize,
    growth: usize,
    free: usize,
    lowest: usize,
}

impl DescriptorPatch {
    /// Observe fixed descriptor capacity without a proposed name or ROW target.
    pub(in crate::table::mutation) fn check_capacity(
        image: &NativeImage,
        table: &Table,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, Error> {
        let geometry = Geometry::capture(image, table, budget, cancelled)?;
        Ok(geometry.growth <= geometry.free)
    }

    pub(in crate::table::mutation) fn capture(
        image: &NativeImage,
        table: &Table,
        reference: u32,
        body_bytes: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let geometry = Geometry::capture(image, table, budget, cancelled)?;
        if geometry.growth > geometry.free {
            return Err(Error::Capacity);
        }
        let length = u32::try_from(body_bytes).map_err(|_| Error::Capacity)?;
        if length == 0 || length >= SINGLE_PROPERTY_TAG {
            return Err(Error::Capacity);
        }

        let record_length = geometry
            .record
            .data
            .len()
            .checked_add(geometry.growth)
            .ok_or(Error::Capacity)?;
        budget.charge_allocation(record_length as u64, 1)?;
        let mut record = Vec::new();
        record
            .try_reserve_exact(record_length)
            .map_err(|_| InspectError::Allocation)?;
        record.extend_from_slice(&geometry.record.data[..geometry.footer]);
        record.resize(geometry.footer + geometry.growth, 0);
        record.extend_from_slice(&geometry.record.data[geometry.footer..]);

        let new_end = geometry.value_start + DESCRIPTOR_BYTES;
        let encoded_end = u16::try_from(new_end).map_err(|_| Error::Capacity)?;
        let value = &mut record[geometry.value_start..new_end];
        value[..size_of::<u32>()].copy_from_slice(&(SINGLE_PROPERTY_TAG | length).to_le_bytes());
        value[size_of::<u32>()..2 * size_of::<u32>()].copy_from_slice(&reference.to_le_bytes());
        value[2 * size_of::<u32>()..].fill(0);
        for word in (geometry.variable_end_words.start..geometry.variable_end_words.end)
            .step_by(VARIABLE_OFFSET_BYTES)
        {
            check(budget, cancelled)?;
            let at = word + geometry.growth;
            record[at..at + VARIABLE_OFFSET_BYTES].copy_from_slice(&encoded_end.to_le_bytes());
        }
        record[geometry.mask + geometry.growth] |= geometry.bit;

        budget.charge_allocation(PAGE_BYTES as u64, 1)?;
        budget.charge(
            Resource::Steps,
            (4 * PAGE_BYTES + 2 * record_length + 2 * DESCRIPTOR_BYTES) as u64,
        )?;
        let original = &image.as_bytes()[geometry.page_base..geometry.page_base + PAGE_BYTES];
        let mut expected = Vec::new();
        expected
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        expected.extend_from_slice(original);
        let start = geometry.record.offset - geometry.page_base;
        if geometry.growth != 0 {
            // ROW identities and forwarding flags remain stable. Only the packed
            // physical suffix moves to make room for the larger selected record.
            expected.copy_within(geometry.lowest..start, geometry.lowest - geometry.growth);
            for slot in geometry.slot..geometry.rows {
                check(budget, cancelled)?;
                let at = slot_offset(slot);
                let word = u16_at(original, at)?;
                let offset = usize::from(word & OFFSET_MASK)
                    .checked_sub(geometry.growth)
                    .ok_or(Error::Preservation)?;
                let offset = u16::try_from(offset).map_err(|_| Error::Preservation)?;
                let relocated = (word & !OFFSET_MASK) | offset;
                expected[at..at + size_of::<u16>()].copy_from_slice(&relocated.to_le_bytes());
            }
            let free =
                u16::try_from(geometry.free - geometry.growth).map_err(|_| Error::Preservation)?;
            expected[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
                .copy_from_slice(&free.to_le_bytes());
        }
        let new_start = start - geometry.growth;
        expected[new_start..new_start + record.len()].copy_from_slice(&record);
        check(budget, cancelled)?;

        Ok(Self {
            page: geometry.page_base..geometry.page_base + PAGE_BYTES,
            expected,
        })
    }

    pub(in crate::table::mutation) fn apply(&self, after: &mut [u8]) -> Result<(), Error> {
        let page = after
            .get_mut(self.page.start..self.page.end)
            .ok_or(Error::Preservation)?;
        page.copy_from_slice(&self.expected);
        Ok(())
    }

    pub(in crate::table::mutation) fn verify(&self, after: &[u8]) -> Result<(), Error> {
        if after.get(self.page.start..self.page.end) != Some(self.expected.as_slice()) {
            return Err(Error::Preservation);
        }
        Ok(())
    }

    pub(in crate::table::mutation) fn page_range(&self) -> Range<usize> {
        self.page.start..self.page.end
    }
}

impl Geometry {
    fn capture(
        image: &NativeImage,
        table: &Table,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        check(budget, cancelled)?;
        let catalog = &table.catalog;
        let object = table.schema.object_id();
        let mut selected_origin = None;
        for origin in image.catalog().origins() {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, 1)?;
            if origin.object_id() == object && selected_origin.replace(origin).is_some() {
                return Err(Error::Source);
            }
        }
        let origin = selected_origin.ok_or(Error::Source)?;
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let root = reader.page(origin.page())?;
        let record = match table.family {
            Family::Current => reader.row_location(
                origin.page(),
                origin.slot(),
                catalog.definition_page(),
                &root,
            )?,
            Family::Retained => {
                let inventory = Inventory::capture(image, reader.budget, cancelled)?;
                inventory.require_source(image)?;
                if inventory.catalog_definition() != catalog.definition_page()
                    || inventory.catalog_live_rows() != catalog.declared_rows()
                {
                    return Err(Error::Preservation);
                }
                let owned = inventory.catalog_pages();
                reader.read_location_observed(
                    origin.page(),
                    origin.slot(),
                    catalog.definition_page(),
                    &root,
                    &mut |reference, budget| {
                        let levels = owned.len().checked_ilog2().unwrap_or(0) as u64 + 1;
                        budget.charge(Resource::Steps, levels)?;
                        if owned.binary_search(&reference_page(reference)).is_err() {
                            return Err(InspectError::Malformed);
                        }
                        Ok(())
                    },
                )?
            }
        };
        let framed = record_layout::parse(&record.data, catalog, reader.budget)?;
        let index = catalog_property::column(catalog)?;
        let field = catalog.columns()[index].physical_field();
        if field.fixed
            || field.number >= framed.count
            || framed.count != catalog.layout.max_columns
            || framed.offsets.len() != catalog.layout.variable_slots + 1
            || field.variable + 1 >= framed.offsets.len()
        {
            return Err(Error::Source);
        }
        let value_start = framed.offsets[field.variable];
        let value_end = framed.offsets[field.variable + 1];
        let mask = framed.mask_start + field.number / u8::BITS as usize;
        let bit = 1_u8 << (field.number % u8::BITS as usize);
        let present = record.data[mask] & bit != 0;
        if present && value_end - value_start < DESCRIPTOR_BYTES {
            return Err(Error::Source);
        }
        for offset in &framed.offsets[field.variable + 1..] {
            check(reader.budget, cancelled)?;
            reader.work(size_of::<usize>())?;
            if *offset != value_end {
                return Err(Error::Source);
            }
        }
        for column in catalog.columns() {
            reader.tick()?;
            let following = column.physical_field();
            if following.number >= framed.count {
                return Err(Error::Source);
            }
            if !following.fixed && following.variable > field.variable {
                let mask = framed.mask_start + following.number / u8::BITS as usize;
                let bit = 1_u8 << (following.number % u8::BITS as usize);
                if record.data[mask] & bit != 0 {
                    return Err(Error::Source);
                }
            }
        }
        let available = framed
            .data_end
            .checked_sub(value_start)
            .ok_or(Error::Source)?;
        let growth = DESCRIPTOR_BYTES.saturating_sub(available);
        let count_word = framed
            .mask_start
            .checked_sub(VARIABLE_COUNT_BYTES)
            .ok_or(Error::Source)?;
        let first_end_word = count_word
            .checked_sub(framed.offsets.len() * VARIABLE_OFFSET_BYTES)
            .ok_or(Error::Source)?;
        let after_last_end_word = count_word
            .checked_sub((field.variable + 1) * VARIABLE_OFFSET_BYTES)
            .ok_or(Error::Source)?;
        let variable_end_words = first_end_word..after_last_end_word;

        let page_base = record.offset / PAGE_BYTES * PAGE_BYTES;
        let page = image
            .as_bytes()
            .get(page_base..page_base + PAGE_BYTES)
            .ok_or(Error::Source)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || page[OWNER_OFFSET..OWNER_OFFSET + size_of::<u32>()]
                != catalog.definition_page().to_le_bytes()
        {
            return Err(Error::Source);
        }
        let rows = row_count(page)?;
        let mut selected = None;
        let mut lowest = PAGE_BYTES;
        for slot in 0..rows {
            check(reader.budget, cancelled)?;
            reader.work(size_of::<u16>())?;
            let (_, start, end) = row_range(page, slot)?;
            if start >= end || end != lowest || start < slot_offset(rows) {
                return Err(Error::Source);
            }
            if page_base + start == record.offset
                && end - start == record.data.len()
                && selected.replace(slot).is_some()
            {
                return Err(Error::Source);
            }
            lowest = start;
        }
        let slot = selected.ok_or(Error::Source)?;
        let free = lowest.checked_sub(slot_offset(rows)).ok_or(Error::Source)?;
        if usize::from(u16_at(page, FREE_OFFSET)?) != free {
            return Err(Error::Source);
        }

        Ok(Self {
            page_base,
            slot,
            rows,
            record,
            value_start,
            variable_end_words,
            mask,
            bit,
            footer: framed.data_end,
            growth,
            free,
            lowest,
        })
    }
}
