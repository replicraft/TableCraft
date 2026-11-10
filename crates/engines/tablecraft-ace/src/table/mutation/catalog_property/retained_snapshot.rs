//! Retained metadata snapshots use an actual fitted LVAL carrier and bound parent.
use super::{append_retained_fitted_body, single_descriptor::DescriptorPatch};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{
        bytes::u16_at,
        page::{DATA_SIGNATURE, PAGE_BYTES},
        reader::{row_count, row_range},
        row::{ROW_COUNT_OFFSET, reference_page, reference_slot, slot_offset},
    },
    header::check,
    table::mutation::{
        current_schema::{Family, Table},
        owned_lval::property::PropertyAppend,
    },
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = DATA_SIGNATURE.len();

/// A borrowed original prevents moving its source while its exact placement is
/// being composed or checked. This supplies no native schema mutation authority.
pub(in crate::table::mutation) struct Snapshot<'image> {
    image: &'image NativeImage,
    descriptor: DescriptorPatch,
    append: PropertyAppend,
    body: Vec<u8>,
}

pub(in crate::table::mutation) fn compose<'image>(
    image: &'image NativeImage,
    table: &Table,
    body: Vec<u8>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Snapshot<'image>, EditError> {
    check(budget, cancelled)?;
    if table.family != Family::Retained {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    image.require_retained_capture()?;
    let append = append_retained_fitted_body(image, &body, budget, cancelled)?;
    if append.length != image.as_bytes().len() || !append.maps.is_empty() {
        return Err(EditError::Preservation);
    }
    let descriptor = DescriptorPatch::capture(
        image,
        table,
        append.reference,
        body.len(),
        budget,
        cancelled,
    )?;
    let page = append.page as usize * PAGE_BYTES;
    if descriptor.page_range().contains(&page) {
        return Err(EditError::Preservation);
    }
    Ok(Snapshot {
        image,
        descriptor,
        append,
        body,
    })
}

impl Snapshot<'_> {
    pub(in crate::table::mutation) fn body(&self) -> &[u8] {
        &self.body
    }

    pub(in crate::table::mutation) fn property_reference(&self) -> u32 {
        self.append.reference
    }

    pub(in crate::table::mutation) fn catalog_page_range(&self) -> Range<usize> {
        self.descriptor.page_range()
    }

    pub(in crate::table::mutation) fn property_page_range(&self) -> Range<usize> {
        let start = self.append.page as usize * PAGE_BYTES;
        start..start + PAGE_BYTES
    }

    pub(in crate::table::mutation) fn apply(
        &self,
        after: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        if after.len() != self.image.as_bytes().len() {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, (2 * PAGE_BYTES) as u64)
            .map_err(InspectError::from)?;
        let page = self.property_page_range();
        after
            .get_mut(page)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&self.append.bytes);
        self.descriptor.apply(after)?;
        self.verify_placement(after, budget, cancelled)
    }

    /// Rebuild the selected LVAL record's exact permitted spans from original
    /// physical framing, independently of the generic append publisher's patch.
    pub(in crate::table::mutation) fn verify_placement(
        &self,
        after: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        if after.len() != self.image.as_bytes().len()
            || reference_page(self.append.reference) != self.append.page
        {
            return Err(EditError::Preservation);
        }
        self.descriptor.verify(after)?;
        let page = self.property_page_range();
        let before = self
            .image
            .as_bytes()
            .get(page.start..page.end)
            .ok_or(EditError::Preservation)?;
        let observed = after
            .get(page.start..page.end)
            .ok_or(EditError::Preservation)?;
        let count = row_count(before)?;
        let expected_end = if count == 0 {
            PAGE_BYTES
        } else {
            row_range(before, count - 1)?.1
        };
        if row_count(observed)? != count + 1
            || usize::from(reference_slot(self.append.reference)) != count
        {
            return Err(EditError::Preservation);
        }
        let (word, start, end) = row_range(observed, count)?;
        let directory = slot_offset(count);
        let free = start
            .checked_sub(slot_offset(count + 1))
            .ok_or(EditError::Preservation)?;
        budget
            .charge(Resource::Steps, self.body.len() as u64)
            .map_err(InspectError::from)?;
        if usize::from(word) != start
            || end != expected_end
            || end.checked_sub(start) != Some(self.body.len())
            || observed[start..end] != self.body
            || usize::from(u16_at(observed, FREE_OFFSET)?) != free
        {
            return Err(EditError::Preservation);
        }
        let permitted = [
            FREE_OFFSET..FREE_OFFSET + size_of::<u16>(),
            ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>(),
            directory..directory + size_of::<u16>(),
            start..end,
        ];
        budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        for (offset, (old, new)) in before.iter().zip(observed).enumerate() {
            if old != new && !permitted.iter().any(|range| range.contains(&offset)) {
                return Err(EditError::Preservation);
            }
        }
        check(budget, cancelled)?;
        Ok(())
    }
}
