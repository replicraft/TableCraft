//! Source-bound capacity reservations precede all candidate page staging.
use super::{forward::FREE_OFFSET, slots, space::Space};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES, PAGE_NUMBER_MASK},
        reader::row_count,
        row::{MAX_SLOTS, SLOT_BYTES, reference_page, slot_offset},
    },
    header::check,
    table::mutation::{record_roots::Root, rows::RowAssignment},
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

struct Capacity {
    number: u32,
    eligible: bool,
    count: usize,
    frontier: usize,
    free: usize,
}

pub(in crate::table::mutation) struct Reservations {
    address: usize,
    length: usize,
    definition: u32,
    pages: Vec<Capacity>,
}

pub(in crate::table::mutation) struct Destination {
    pub page: u32,
    pub slot: u8,
    pub range: Range<usize>,
}

pub(in crate::table::mutation) struct Reservation {
    address: usize,
    length: usize,
    definition: u32,
    root: u32,
    terminal: usize,
    record_bytes: usize,
    destination: Option<Destination>,
}

impl Reservation {
    pub(in crate::table::mutation) fn require<U: RowAssignment>(
        &self,
        image: &NativeImage,
        definition: u32,
        root: &Root<'_, U>,
        record_bytes: usize,
    ) -> Result<(), EditError> {
        if self.address != image.as_bytes().as_ptr() as usize
            || self.length != image.as_bytes().len()
            || self.definition != definition
            || self.root != root.reference
            || self.terminal != root.final_record.offset
            || self.record_bytes != record_bytes
        {
            return Err(EditError::Preservation);
        }
        Ok(())
    }

    pub(in crate::table::mutation) fn destination(&self) -> Option<Destination> {
        self.destination.as_ref().map(|destination| Destination {
            page: destination.page,
            slot: destination.slot,
            // NO_CLONE: Return an owned destination observation without consuming the
            // retained reservation. Only range bounds are copied, not the
            // payload.
            range: destination.range.clone(),
        })
    }
}

impl Reservations {
    pub(in crate::table::mutation) fn capture(
        image: &NativeImage,
        space: &Space,
        owned: &[u32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        budget
            .charge_allocation(owned.len() as u64, size_of::<Capacity>() as u64)
            .map_err(InspectError::from)?;
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(owned.len())
            .map_err(|_| InspectError::Allocation)?;
        let mut previous = None;
        for &number in owned {
            check(budget, cancelled)?;
            if previous.is_some_and(|before| before >= number) {
                return Err(EditError::Preservation);
            }
            previous = Some(number);
            if number & !PAGE_NUMBER_MASK != 0 {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let eligible = space.append_eligible(image, number, budget, cancelled)?;
            let base = usize::try_from(number)
                .ok()
                .and_then(|number| number.checked_mul(PAGE_BYTES))
                .ok_or(EditError::Preservation)?;
            let raw = image
                .as_bytes()
                .get(base..base + PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            if raw[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
                || u32_at(raw, OWNER_OFFSET)? != space.definition()
            {
                return Err(InspectError::Malformed.into());
            }
            let count = row_count(raw)?;
            let frontier = slots::frontier(raw, budget, cancelled)?;
            let free = usize::from(u16_at(raw, FREE_OFFSET)?);
            if frontier.checked_sub(slot_offset(count)) != Some(free) {
                return Err(InspectError::Malformed.into());
            }
            pages.push(Capacity {
                number,
                eligible,
                count,
                frontier,
                free,
            });
        }
        check(budget, cancelled)?;
        Ok(Self {
            address: image.as_bytes().as_ptr() as usize,
            length: image.as_bytes().len(),
            definition: space.definition(),
            pages,
        })
    }

    pub(in crate::table::mutation) fn include_eof(
        &mut self,
        image: &NativeImage,
        future: &super::future::Future,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        future.require(image, self.definition)?;
        check(budget, cancelled)?;
        if self
            .pages
            .last()
            .is_some_and(|page| page.number >= future.number())
        {
            return Err(EditError::Preservation);
        }
        budget
            .charge_allocation(1, size_of::<Capacity>() as u64)
            .map_err(InspectError::from)?;
        budget
            .charge(
                Resource::Steps,
                self.pages
                    .len()
                    .checked_mul(size_of::<Capacity>())
                    .ok_or(InspectError::StructureLimit)? as u64,
            )
            .map_err(InspectError::from)?;
        self.pages
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        self.pages.push(Capacity {
            number: future.number(),
            eligible: true,
            count: 0,
            frontier: PAGE_BYTES,
            free: PAGE_BYTES - slot_offset(0),
        });
        Ok(())
    }

    pub(in crate::table::mutation) fn reserve<U: RowAssignment>(
        &mut self,
        image: &NativeImage,
        root: &Root<'_, U>,
        record_bytes: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Reservation, EditError> {
        self.try_reserve(image, root, record_bytes, budget, cancelled)?
            .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))
    }

    pub(in crate::table::mutation) fn try_reserve<U: RowAssignment>(
        &mut self,
        image: &NativeImage,
        root: &Root<'_, U>,
        record_bytes: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<Reservation>, EditError> {
        check(budget, cancelled)?;
        if self.address != image.as_bytes().as_ptr() as usize
            || self.length != image.as_bytes().len()
            || record_bytes < root.final_record.data.len()
        {
            return Err(EditError::Preservation);
        }
        let destination = if record_bytes == root.final_record.data.len() {
            None
        } else {
            let root_page = reference_page(root.reference);
            let mut root_position = None;
            for (position, page) in self.pages.iter().enumerate() {
                check(budget, cancelled)?;
                if page.number == root_page {
                    root_position = Some(position);
                }
            }
            let root_position = root_position.ok_or(EditError::Preservation)?;
            let mut selected = None;
            if fits(&self.pages[root_position], record_bytes)? {
                selected = Some(root_position);
            } else {
                for (position, page) in self.pages.iter().enumerate() {
                    check(budget, cancelled)?;
                    if position != root_position && fits(page, record_bytes)? {
                        selected = Some(position);
                        break;
                    }
                }
            }
            let Some(selected) = selected else {
                return Ok(None);
            };
            let page = self
                .pages
                .get_mut(selected)
                .ok_or(EditError::Preservation)?;
            let slot = u8::try_from(page.count).map_err(|_| EditError::Preservation)?;
            let base = usize::try_from(page.number)
                .ok()
                .and_then(|number| number.checked_mul(PAGE_BYTES))
                .ok_or(EditError::Preservation)?;
            let end = page.frontier;
            page.frontier = page
                .frontier
                .checked_sub(record_bytes)
                .ok_or(EditError::Preservation)?;
            page.free = page
                .free
                .checked_sub(record_bytes + SLOT_BYTES)
                .ok_or(EditError::Preservation)?;
            page.count += 1;
            Some(Destination {
                page: page.number,
                slot,
                range: base + page.frontier..base + end,
            })
        };
        Ok(Some(Reservation {
            address: self.address,
            length: self.length,
            definition: self.definition,
            root: root.reference,
            terminal: root.final_record.offset,
            record_bytes,
            destination,
        }))
    }
}

fn fits(page: &Capacity, bytes: usize) -> Result<bool, EditError> {
    let required = bytes
        .checked_add(SLOT_BYTES)
        .ok_or(EditError::Preservation)?;
    Ok(page.eligible
        && page.count < MAX_SLOTS
        && page.free >= required
        && page
            .frontier
            .checked_sub(bytes)
            .is_some_and(|start| start >= slot_offset(page.count + 1)))
}
