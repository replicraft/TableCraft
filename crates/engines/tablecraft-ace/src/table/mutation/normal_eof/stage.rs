//! Serialize and reserve every original row before allocating candidate bytes.
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeImage, WriteBlocker,
    format::{
        bytes::u16_at,
        page::PAGE_BYTES,
        reader::row_count,
        row::{
            DELETED_MASK, MAX_SLOTS, OFFSET_MASK, RELOCATED_MASK, ROW_COUNT_OFFSET, SLOT_BYTES,
            reference, reference_page, reference_slot, slot_offset,
        },
    },
    header::check,
    table::mutation::{
        normal_fitted::{
            binding::{self, Selected},
            stage as records,
        },
        preserved_source::Source,
        record_placement::{
            empty,
            future::Future,
            owned::{Reservation, Reservations},
            slots,
            space::Space,
        },
        record_roots::{self, Root},
        record_serialization::policy::RecordPolicy,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = 2;
const POINTER_BYTES: usize = size_of::<u32>();

pub(super) fn owned_fits(
    image: &NativeImage,
    space: &Space,
    owned: &[u32],
    roots: &[Root<'_, Selected>],
    records: &[records::Patch],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let mut reservations = Reservations::capture(image, space, owned, budget, cancelled)?;
    for root in roots {
        for record in records {
            check(budget, cancelled)?;
            if root.final_record.offset == record.offset
                && reservations
                    .try_reserve(image, root, record.after.len(), budget, cancelled)?
                    .is_none()
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub(super) fn build(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, usize), EditError> {
    let bound = binding::all(source, groups, budget, cancelled)?;
    let schema = source.table.schema();
    let captured = record_roots::capture(image, schema, &bound.rows, budget, cancelled)?;
    let space = Space::capture_normal(image, schema, &captured.owned, budget, cancelled)?;
    let (records, changed_rows) = records::prepare_with(
        image,
        source,
        groups,
        RecordPolicy::GrowingPreferred,
        budget,
        cancelled,
    )?;
    if owned_fits(
        image,
        &space,
        &captured.owned,
        &captured.selected,
        &records,
        budget,
        cancelled,
    )? {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let future = Future::capture(image, schema, budget, cancelled)?;
    let mut reservations =
        Reservations::capture(image, &space, &captured.owned, budget, cancelled)?;
    reservations.include_eof(image, &future, budget, cancelled)?;
    let mut operations = binding::vector(records.len(), budget)?;
    let mut used = false;
    for root in &captured.selected {
        for record in &records {
            check(budget, cancelled)?;
            if root.final_record.offset != record.offset {
                continue;
            }
            if !binding::equal_bytes(&root.final_record.data, &record.before, budget, cancelled)? {
                return Err(EditError::Preservation);
            }
            let reservation =
                reservations.reserve(image, root, record.after.len(), budget, cancelled)?;
            used |= reservation
                .destination()
                .is_some_and(|target| target.page == future.number());
            operations.push((root, record, reservation));
        }
    }
    if !used || operations.len() != changed_rows {
        return Err(EditError::Preservation);
    }
    let length = image
        .as_bytes()
        .len()
        .checked_add(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, length as u64)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    after.extend_from_slice(image.as_bytes());
    let initial = empty::data(schema.definition_page(), budget)?;
    after.extend_from_slice(&initial);
    {
        let mut writer = Writer {
            image,
            space: &space,
            future: &future,
            after: &mut after,
        };
        for (root, record, reservation) in operations {
            writer.apply(root, &record.after, &reservation, budget, cancelled)?;
        }
    }
    // Original metadata is still untouched at this phase. Only the qualified
    // three cells are then changed, separately from all DATA row placement.
    space.audit(
        image.as_bytes(),
        &after[..image.as_bytes().len()],
        budget,
        cancelled,
    )?;
    future.apply(image, &mut after, budget, cancelled)?;
    Ok((after, changed_rows))
}

struct Writer<'a> {
    image: &'a NativeImage,
    space: &'a Space,
    future: &'a Future,
    after: &'a mut [u8],
}

impl Writer<'_> {
    fn apply(
        &mut self,
        root: &Root<'_, Selected>,
        row: &[u8],
        reservation: &Reservation,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        reservation.require(self.image, self.space.definition(), root, row.len())?;
        let root_page = reference_page(root.reference);
        let terminal = u32::try_from(root.final_record.offset / PAGE_BYTES)
            .map_err(|_| EditError::Preservation)?;
        self.space
            .require_page(self.image, root_page, false, budget, cancelled)?;
        self.space
            .require_page(self.image, terminal, false, budget, cancelled)?;
        let Some(target) = reservation.destination() else {
            let end = root
                .final_record
                .offset
                .checked_add(row.len())
                .ok_or(EditError::Preservation)?;
            let bytes = self
                .after
                .get_mut(root.final_record.offset..end)
                .ok_or(EditError::Preservation)?;
            if !binding::equal_bytes(bytes, &root.final_record.data, budget, cancelled)? {
                return Err(EditError::Preservation);
            }
            bytes.copy_from_slice(row);
            return Ok(());
        };
        if target.page == self.future.number() {
            self.future.require(self.image, self.space.definition())?;
        } else {
            self.space
                .require_page(self.image, target.page, true, budget, cancelled)?;
        }
        if row.len() <= root.final_record.data.len()
            || root.range.len() < POINTER_BYTES
            || root.word & !(OFFSET_MASK | RELOCATED_MASK) != 0
        {
            return Err(EditError::Preservation);
        }
        {
            let base = target.page as usize * PAGE_BYTES;
            let page = self
                .after
                .get_mut(base..base + PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            let count = row_count(page)?;
            let end = slots::frontier(page, budget, cancelled)?;
            let start = target
                .range
                .start
                .checked_sub(base)
                .ok_or(EditError::Preservation)?;
            let stop = target
                .range
                .end
                .checked_sub(base)
                .ok_or(EditError::Preservation)?;
            let free = usize::from(u16_at(page, FREE_OFFSET)?);
            if count != usize::from(target.slot)
                || count >= MAX_SLOTS
                || stop != end
                || stop.checked_sub(start) != Some(row.len())
                || start < slot_offset(count + 1)
                || end.checked_sub(slot_offset(count)) != Some(free)
                || free < row.len() + SLOT_BYTES
            {
                return Err(EditError::Preservation);
            }
            budget
                .charge(Resource::Steps, row.len() as u64)
                .map_err(InspectError::from)?;
            page[start..stop].copy_from_slice(row);
            let word = u16::try_from(start).map_err(|_| EditError::Preservation)? | DELETED_MASK;
            page[slot_offset(count)..slot_offset(count + 1)].copy_from_slice(&word.to_le_bytes());
            let next_count = u16::try_from(count + 1).map_err(|_| EditError::Preservation)?;
            let next_free = u16::try_from(free - row.len() - SLOT_BYTES)
                .map_err(|_| EditError::Preservation)?;
            page[ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>()]
                .copy_from_slice(&next_count.to_le_bytes());
            page[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
                .copy_from_slice(&next_free.to_le_bytes());
        }
        {
            let base = root_page as usize * PAGE_BYTES;
            let page = self
                .after
                .get_mut(base..base + PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            let start = root
                .range
                .start
                .checked_sub(base)
                .ok_or(EditError::Preservation)?;
            let end = root
                .range
                .end
                .checked_sub(base)
                .ok_or(EditError::Preservation)?;
            let at = slot_offset(usize::from(reference_slot(root.reference)));
            if u16_at(page, at)? != root.word
                || !binding::equal_bytes(
                    &page[start..end],
                    &self.image.as_bytes()[root.range.clone()],
                    budget,
                    cancelled,
                )?
            {
                return Err(EditError::Preservation);
            }
            if root.word & RELOCATED_MASK == 0 {
                page[start..end].fill(0);
            }
            page[start..start + POINTER_BYTES]
                .copy_from_slice(&reference(target.page, target.slot).to_le_bytes());
            page[at..at + SLOT_BYTES].copy_from_slice(&(root.word | RELOCATED_MASK).to_le_bytes());
        }
        check(budget, cancelled)?;
        Ok(())
    }
}
