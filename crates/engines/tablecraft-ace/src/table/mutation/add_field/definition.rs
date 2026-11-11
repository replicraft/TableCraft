//! Append a descriptor/name while retaining every original native byte frame.
mod descriptor;
use super::{CurrentFieldAdditionError as Error, Target};
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{
        bytes::{u16_at, u32_at},
        definition::{LENGTH_OFFSET, PAGE_HEADER_BYTES},
        layout::{self, COLUMN_COUNT_OFFSET, COLUMN_ENTRY_BYTES, MAX_COLUMN_COUNT_OFFSET},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    identifier::MAX_NAME_UNITS,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = 2;
const NAME_LENGTH_BYTES: usize = size_of::<u16>();
const COPY_BYTES: usize = 16 * 1024;

pub(super) struct Plan {
    pub base: usize,
    pub length: usize,
    pub descriptor_start: usize,
    pub descriptor_end: usize,
    pub index_start: usize,
    pub number: usize,
    pub active_count: usize,
    pub column_id: u16,
    pub fixed_offset: usize,
    pub variable: usize,
    pub variable_high_water: usize,
    pub maximum_name_units: usize,
}

impl Plan {
    pub(super) fn capture(
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        Self::capture_with_width(image, schema, size_of::<i32>(), budget, cancelled)
    }

    pub(super) fn for_target(
        image: &NativeImage,
        schema: &TableSchema,
        target: Target,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let width = if let Target::Typed(target) = target {
            let field = target.field(0).map_err(super::creation_error)?;
            if field.offset.is_some() {
                field.width
            } else {
                0
            }
        } else {
            size_of::<i32>()
        };
        Self::capture_with_width(image, schema, width, budget, cancelled)
    }

    fn capture_with_width(
        image: &NativeImage,
        schema: &TableSchema,
        fixed_width: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        check(budget, cancelled)?;
        let layout = schema.physical_layout();
        let count = schema.columns().len();
        if count == 0 || layout.max_columns < count {
            return Err(Error::Source);
        }

        let base = schema.definition_page() as usize * PAGE_BYTES;
        let page = image
            .as_bytes()
            .get(base..base + PAGE_BYTES)
            .ok_or(Error::Source)?;
        let length = u32_at(page, LENGTH_OFFSET)? as usize;
        let free = PAGE_BYTES
            .checked_sub(length + PAGE_HEADER_BYTES)
            .ok_or(Error::Source)?;
        if usize::from(u16_at(page, FREE_OFFSET)?) != free {
            return Err(Error::Source);
        }

        let descriptor_start =
            layout::COLUMN_BLOCK_OFFSET + layout.physical_indexes * layout::INDEX_ENTRY_BYTES;
        let descriptor_end = descriptor_start + count * COLUMN_ENTRY_BYTES;
        if descriptor_end > layout.index_start || layout.index_start > length {
            return Err(Error::Source);
        }

        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let mut observed = 0;
        let mut next_id = 0;
        let parsed = layout::read_with_field_names(
            &mut reader,
            schema.definition_page(),
            |number, id, entry, _| {
                if schema.columns().get(observed).map(|field| field.number()) != Some(number)
                    || entry
                        != (descriptor_start + observed * COLUMN_ENTRY_BYTES
                            ..descriptor_start + (observed + 1) * COLUMN_ENTRY_BYTES)
                {
                    return Err(InspectError::Unsupported);
                }
                next_id = next_id.max(usize::from(id) + 1);
                observed += 1;
                Ok(())
            },
        );
        match parsed {
            Ok(_) => {}
            Err(InspectError::Unsupported) => return Err(Error::Source),
            Err(error) => return Err(Error::Inspect(error)),
        }
        if observed != count {
            return Err(Error::Source);
        }

        let mut fixed_offset = 0;
        let mut variable = 0;
        for field in schema.columns() {
            check(budget, cancelled)?;
            if !field.fixed() {
                variable = variable.max(
                    field
                        .physical_field()
                        .variable
                        .checked_add(1)
                        .ok_or(Error::Capacity)?,
                );
            }
            // A Boolean lives in the record mask; its unused offset grants no
            // payload space and must not move the appended fixed field.
            if field.fixed() && field.storage_length() != 0 {
                fixed_offset = fixed_offset.max(
                    field
                        .physical_field()
                        .offset
                        .checked_add(field.storage_length())
                        .ok_or(Error::Capacity)?,
                );
            }
        }
        if fixed_offset
            .checked_add(fixed_width)
            .is_none_or(|end| end > u16::MAX as usize)
        {
            return Err(Error::Capacity);
        }

        let maximum_name_units = free
            .checked_sub(COLUMN_ENTRY_BYTES + NAME_LENGTH_BYTES)
            .map_or(0, |bytes| bytes / size_of::<u16>())
            .min(MAX_NAME_UNITS);

        Ok(Self {
            base,
            length,
            descriptor_start,
            descriptor_end,
            index_start: layout.index_start,
            number: layout.max_columns,
            active_count: count,
            column_id: u16::try_from(next_id).map_err(|_| Error::Capacity)?,
            fixed_offset,
            variable,
            variable_high_water: layout.variable_slots,
            maximum_name_units,
        })
    }

    pub(super) fn build(
        &self,
        image: &NativeImage,
        name: &str,
        target: Target,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, Error> {
        let frame = descriptor::target(self, target)?;
        self.build_frame(image, name, frame, budget, cancelled)
    }

    fn build_frame(
        &self,
        image: &NativeImage,
        name: &str,
        frame: descriptor::Frame,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, Error> {
        let before = image.as_bytes();
        budget.charge_allocation(before.len() as u64, 1)?;
        let mut after = Vec::new();
        after
            .try_reserve_exact(before.len())
            .map_err(|_| InspectError::Allocation)?;
        for chunk in before.chunks(COPY_BYTES) {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, chunk.len() as u64)?;
            after.extend_from_slice(chunk);
        }

        let name_bytes = NAME_LENGTH_BYTES + name.len() * size_of::<u16>();
        let delta = COLUMN_ENTRY_BYTES + name_bytes;
        let end = self
            .length
            .checked_add(delta)
            .filter(|end| *end <= PAGE_BYTES - PAGE_HEADER_BYTES)
            .ok_or(Error::Capacity)?;
        let root = &mut after[self.base..self.base + PAGE_BYTES];
        budget.charge(Resource::Steps, (2 * PAGE_BYTES) as u64)?;
        root.copy_within(
            self.descriptor_end..self.length,
            self.descriptor_end + COLUMN_ENTRY_BYTES,
        );
        root.copy_within(
            self.index_start + COLUMN_ENTRY_BYTES..self.length + COLUMN_ENTRY_BYTES,
            self.index_start + delta,
        );

        root[self.descriptor_end..self.descriptor_end + COLUMN_ENTRY_BYTES]
            .copy_from_slice(&frame.bytes);
        if frame.variable {
            put16(
                root,
                layout::VARIABLE_SLOT_COUNT_OFFSET,
                self.variable_high_water + 1,
            )?;
        }

        let at = self.index_start + COLUMN_ENTRY_BYTES;
        put16(root, at, name.len() * size_of::<u16>())?;
        for (index, unit) in name.encode_utf16().enumerate() {
            let at = at + NAME_LENGTH_BYTES + index * size_of::<u16>();
            root[at..at + size_of::<u16>()].copy_from_slice(&unit.to_le_bytes());
        }

        put16(root, FREE_OFFSET, PAGE_BYTES - end - PAGE_HEADER_BYTES)?;
        root[LENGTH_OFFSET..LENGTH_OFFSET + size_of::<u32>()]
            .copy_from_slice(&(end as u32).to_le_bytes());
        put16(root, MAX_COLUMN_COUNT_OFFSET, self.number + 1)?;
        put16(root, COLUMN_COUNT_OFFSET, self.active_count + 1)?;
        Ok(after)
    }
}

fn put16(bytes: &mut [u8], at: usize, value: usize) -> Result<(), Error> {
    let value = u16::try_from(value).map_err(|_| Error::Capacity)?;
    bytes
        .get_mut(at..at + size_of::<u16>())
        .ok_or(Error::Preservation)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}
