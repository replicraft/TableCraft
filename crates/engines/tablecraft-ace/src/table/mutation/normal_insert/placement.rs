//! Reserve visible slots against original owned capacity before staging bytes.
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{
        allocation,
        bytes::{u16_at, u32_at},
        layout::RECORD_COUNT_OFFSET,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::Reader,
        row::{MAX_SLOTS, ROW_COUNT_OFFSET, SLOT_BYTES, slot_offset},
    },
    header::check,
    table::mutation::{
        preserved_source::Source,
        record_placement::{slots, space::Space},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const MAX_REFERENCE_PAGE: u32 = 0x00ff_ffff;

struct Capacity {
    number: u32,
    count: usize,
    frontier: usize,
    free: usize,
    eligible: bool,
}

struct Reservation {
    number: u32,
    count: usize,
    free: usize,
    start: usize,
    end: usize,
}

pub(super) struct Staged {
    space: Space,
    rows: Vec<Reservation>,
    count_offset: usize,
    original_count: u32,
    final_count: u32,
}

pub(super) fn reserve(
    image: &NativeImage,
    source: &Source,
    records: &[Vec<u8>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    let schema = source.table.schema();

    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let owned = allocation::pages(&mut reader, schema.layout.usage)?;
    for pair in owned.windows(2) {
        check(budget, cancelled)?;
        if pair[0] >= pair[1] {
            return Err(EditError::Preservation);
        }
    }

    let space = Space::capture_normal(image, schema, &owned, budget, cancelled)?;

    budget
        .charge_allocation(owned.len() as u64, size_of::<Capacity>() as u64)
        .map_err(InspectError::from)?;

    let mut capacities = Vec::new();
    capacities
        .try_reserve_exact(owned.len())
        .map_err(|_| InspectError::Allocation)?;

    for number in owned {
        check(budget, cancelled)?;
        let eligible = space.append_eligible(image, number, budget, cancelled)?;
        let start = number as usize * PAGE_BYTES;
        let page = image
            .as_bytes()
            .get(start..start + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        if number > MAX_REFERENCE_PAGE
            || page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(page, OWNER_OFFSET)? != schema.definition_page()
        {
            return Err(EditError::Preservation);
        }

        let count = usize::from(u16_at(page, ROW_COUNT_OFFSET)?);
        if count > MAX_SLOTS {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let frontier = slots::frontier(page, budget, cancelled)?;
        let free = usize::from(u16_at(page, FREE_OFFSET)?);
        if frontier.checked_sub(slot_offset(count)) != Some(free) {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }

        capacities.push(Capacity {
            number,
            count,
            frontier,
            free,
            eligible,
        });
    }

    budget
        .charge_allocation(records.len() as u64, size_of::<Reservation>() as u64)
        .map_err(InspectError::from)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(records.len())
        .map_err(|_| InspectError::Allocation)?;

    for record in records {
        check(budget, cancelled)?;
        let needed = record
            .len()
            .checked_add(SLOT_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        let mut selected = None;
        for capacity in &mut capacities {
            check(budget, cancelled)?;
            if !capacity.eligible || capacity.count == MAX_SLOTS || needed > capacity.free {
                continue;
            }
            let end = capacity.frontier;
            let start = end
                .checked_sub(record.len())
                .ok_or(EditError::Preservation)?;

            rows.push(Reservation {
                number: capacity.number,
                count: capacity.count,
                free: capacity.free,
                start,
                end,
            });

            capacity.count += 1;
            capacity.frontier = start;
            capacity.free = start
                .checked_sub(slot_offset(capacity.count))
                .ok_or(EditError::Preservation)?;
            selected = Some(());
            break;
        }
        if selected.is_none() {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }

    let count_offset = schema.definition_page() as usize * PAGE_BYTES + RECORD_COUNT_OFFSET;
    let original_count = u32_at(image.as_bytes(), count_offset)?;
    if original_count != schema.declared_rows() || original_count as usize != source.records.len() {
        return Err(EditError::Preservation);
    }
    let added = u32::try_from(records.len()).map_err(|_| InspectError::StructureLimit)?;
    let final_count = original_count
        .checked_add(added)
        .ok_or(InspectError::StructureLimit)?;
    check(budget, cancelled)?;
    Ok(Staged {
        space,
        rows,
        count_offset,
        original_count,
        final_count,
    })
}

impl Staged {
    pub(super) fn references(&self) -> impl ExactSizeIterator<Item = u32> + '_ {
        self.rows
            .iter()
            .map(|reservation| (reservation.number << 8) | reservation.count as u32)
    }

    pub(super) fn apply(
        self,
        image: &NativeImage,
        records: &[Vec<u8>],
        after: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if records.len() != self.rows.len() || after.len() != image.as_bytes().len() {
            return Err(EditError::Preservation);
        }

        for (reservation, record) in self.rows.iter().zip(records) {
            self.space
                .require_page(image, reservation.number, true, budget, cancelled)?;
            let base = reservation.number as usize * PAGE_BYTES;
            let page = after
                .get_mut(base..base + PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            if usize::from(u16_at(page, ROW_COUNT_OFFSET)?) != reservation.count
                || usize::from(u16_at(page, FREE_OFFSET)?) != reservation.free
                || reservation.end - reservation.start != record.len()
            {
                return Err(EditError::Preservation);
            }

            budget
                .charge(Resource::Steps, (record.len() + 3 * SLOT_BYTES) as u64)
                .map_err(InspectError::from)?;
            check(budget, cancelled)?;
            page[reservation.start..reservation.end].copy_from_slice(record);
            let word =
                u16::try_from(reservation.start).map_err(|_| InspectError::StructureLimit)?;
            page[slot_offset(reservation.count)..slot_offset(reservation.count + 1)]
                .copy_from_slice(&word.to_le_bytes());
            let count =
                u16::try_from(reservation.count + 1).map_err(|_| InspectError::StructureLimit)?;
            page[ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>()]
                .copy_from_slice(&count.to_le_bytes());
            let free = u16::try_from(reservation.start - slot_offset(reservation.count + 1))
                .map_err(|_| InspectError::StructureLimit)?;
            page[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()].copy_from_slice(&free.to_le_bytes());
        }

        // The only allowed metadata edit follows protection audit, just as
        // qualified allocation applies explicit map cells after its DATA audit.
        self.space
            .audit(image.as_bytes(), after, budget, cancelled)?;
        if u32_at(after, self.count_offset)? != self.original_count {
            return Err(EditError::Preservation);
        }
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        after[self.count_offset..self.count_offset + size_of::<u32>()]
            .copy_from_slice(&self.final_count.to_le_bytes());
        check(budget, cancelled)?;
        Ok(())
    }
}
