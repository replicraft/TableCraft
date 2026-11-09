//! Parser-bound fitting geometry; this does not substitute for settled source proof.
use super::super::{FieldRenameError as Error, source::Qualified};
use crate::{
    NativeImage,
    format::{
        bytes::{u16_at, u32_at},
        definition::LENGTH_OFFSET,
        page::{DATA_SIGNATURE, PAGE_BYTES},
        reader::{row_count, row_range},
        row::{OFFSET_MASK, reference_page, reference_slot, slot_offset},
    },
    header::check,
    properties::framing,
    table::mutation::saved_field::Observed,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const DEFINITION_FOOTER_BYTES: usize = 8;
const MAX_NAME_UNITS: usize = 64;
const GROUP_HEADER_BYTES: usize = size_of::<u32>() + size_of::<u16>();
const NAME_HEADER_BYTES: usize = size_of::<u32>() + size_of::<u16>();

pub(super) struct Layout {
    pub definition_base: usize,
    pub definition_length: usize,
    pub definition_free: usize,
    pub property_base: usize,
    pub property_slot: usize,
    pub property_start: usize,
    pub property_end: usize,
    pub database_start: usize,
    pub free: usize,
    pub group: Range<usize>,
    pub maximum_name_units: usize,
}

impl Layout {
    pub fn capture(
        image: &NativeImage,
        observed: &Observed,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        check(budget, cancelled)?;
        let bytes = image.as_bytes();
        let definition_base = observed.schema.definition_page() as usize * PAGE_BYTES;
        let definition = bytes
            .get(definition_base..definition_base + PAGE_BYTES)
            .ok_or(Error::Source)?;
        let definition_length = u32_at(definition, LENGTH_OFFSET)? as usize;
        let definition_free = PAGE_BYTES
            .checked_sub(definition_length + DEFINITION_FOOTER_BYTES)
            .ok_or(Error::Source)?;
        if usize::from(u16_at(definition, FREE_OFFSET)?) != definition_free
            || observed.definition_name.end > definition_base + definition_length
        {
            return Err(Error::Source);
        }

        let property_base = reference_page(observed.property_reference) as usize * PAGE_BYTES;
        let page = bytes
            .get(property_base..property_base + PAGE_BYTES)
            .ok_or(Error::Source)?;
        let property_slot = usize::from(reference_slot(observed.property_reference));
        let count = row_count(page)?;
        // The settled source proves the last row is the separate MSysDb OFF
        // owner. Only that final row may move after the selected live user row.
        if property_slot.checked_add(2) != Some(count) {
            return Err(Error::Source);
        }
        let (word, property_start, property_end) = row_range(page, property_slot)?;
        let (database_word, database_start, database_end) = row_range(page, property_slot + 1)?;
        let free = database_start
            .checked_sub(slot_offset(count))
            .ok_or(Error::Source)?;
        if word & !OFFSET_MASK != 0
            || database_word & !OFFSET_MASK != 0
            || database_end != property_start
            || property_base + property_start != observed.property_body.start
            || property_base + property_end != observed.property_body.end
            || usize::from(u16_at(page, FREE_OFFSET)?) != free
        {
            return Err(Error::Source);
        }
        budget.charge(Resource::Steps, PAGE_BYTES as u64)?;
        if page[slot_offset(count)..database_start]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::Source);
        }

        let mut group = None;
        framing::ordered_group_blocks(&observed.properties, budget, cancelled, |number, span| {
            if number == observed.group && group.replace(span).is_some() {
                return Err(crate::InspectError::Malformed);
            }
            Ok(())
        })?;
        let group = group.ok_or(Error::Source)?;
        let name = observed
            .property_name
            .start
            .checked_sub(observed.property_body.start)
            .ok_or(Error::Source)?;
        if group.start + GROUP_HEADER_BYTES + NAME_HEADER_BYTES != name
            || group.end > observed.property_body.len()
            || observed.property_descriptor.len() != 3 * size_of::<u32>()
        {
            return Err(Error::Source);
        }
        let maximum_name_units = (observed.identity.name_units
            + definition_free.min(free) / size_of::<u16>())
        .min(MAX_NAME_UNITS);
        Ok(Self {
            definition_base,
            definition_length,
            definition_free,
            property_base,
            property_slot,
            property_start,
            property_end,
            database_start,
            free,
            group,
            maximum_name_units,
        })
    }

    pub fn qualified(
        source: &Qualified,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        Self::capture(source.image(), &source.observed, budget, cancelled)
    }
}
