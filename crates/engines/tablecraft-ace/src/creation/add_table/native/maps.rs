//! Actual column map membership, with native indirect storage when it is needed.
use super::super::AddTableError;
use crate::{
    InspectError, NativeImage,
    format::{
        allocation,
        bytes::u32_at,
        page::{PAGE_BYTES, USAGE_SIGNATURE},
        reader::Reader,
        row::{reference_page, reference_slot},
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const POINTER_BYTES: usize = size_of::<u32>();
const INLINE_HEADER: usize = 1 + POINTER_BYTES;
const INLINE_BYTES: usize = 8;
const BITMAP_HEADER: usize = 4;
const BITMAP_CAPACITY: usize = (PAGE_BYTES - BITMAP_HEADER) * u8::BITS as usize;

pub(super) struct Bitmap {
    pub number: u32,
    pub bytes: Vec<u8>,
}

pub(super) struct Map {
    pub reference: u32,
    pub bytes: Vec<u8>,
    pub bitmaps: Vec<Bitmap>,
}

pub(super) struct FreshMap {
    pub bytes: Vec<u8>,
    pub bitmaps: Vec<Bitmap>,
}

impl FreshMap {
    pub fn capture(
        members: &[u32],
        next_page: &mut usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AddTableError> {
        for pair in members.windows(2) {
            check(budget, cancelled)?;
            if pair[0] >= pair[1] {
                return Err(AddTableError::Preservation);
            }
        }
        let base = members.first().copied().unwrap_or(0) / (INLINE_BYTES * 8) as u32
            * (INLINE_BYTES * 8) as u32;
        if members
            .last()
            .is_none_or(|last| *last - base < (INLINE_BYTES * 8) as u32)
        {
            let mut bytes = zeros(INLINE_HEADER + INLINE_BYTES, budget)?;
            bytes[1..INLINE_HEADER].copy_from_slice(&base.to_le_bytes());
            for member in members {
                check(budget, cancelled)?;
                let bit = (*member - base) as usize;
                bytes[INLINE_HEADER + bit / 8] |= 1 << (bit % 8);
            }
            return Ok(Self {
                bytes,
                bitmaps: Vec::new(),
            });
        }
        let slots = members
            .last()
            .map_or(0, |number| *number as usize / BITMAP_CAPACITY + 1)
            .max(INLINE_BYTES / POINTER_BYTES + 1);
        let mut bytes = zeros(1 + slots * POINTER_BYTES, budget)?;
        bytes[0] = 1;
        let mut bitmaps = Vec::new();
        let mut cursor = 0;
        while cursor < members.len() {
            check(budget, cancelled)?;
            let block = members[cursor] as usize / BITMAP_CAPACITY;
            let number = u32::try_from(*next_page).map_err(|_| AddTableError::Capacity)?;
            *next_page = next_page.checked_add(1).ok_or(AddTableError::Capacity)?;
            let mut bitmap = zeros(PAGE_BYTES, budget)?;
            bitmap[..USAGE_SIGNATURE.len()].copy_from_slice(&USAGE_SIGNATURE);
            while cursor < members.len() && members[cursor] as usize / BITMAP_CAPACITY == block {
                check(budget, cancelled)?;
                let bit = members[cursor] as usize % BITMAP_CAPACITY;
                bitmap[BITMAP_HEADER + bit / 8] |= 1 << (bit % 8);
                cursor += 1;
            }
            bytes[1 + block * POINTER_BYTES..1 + (block + 1) * POINTER_BYTES]
                .copy_from_slice(&number.to_le_bytes());
            budget
                .charge_allocation(1, size_of::<Bitmap>() as u64)
                .map_err(InspectError::from)?;
            bitmaps
                .try_reserve_exact(1)
                .map_err(|_| InspectError::Allocation)?;
            bitmaps.push(Bitmap {
                number,
                bytes: bitmap,
            });
        }
        Ok(Self { bytes, bitmaps })
    }
}

fn zeros(length: usize, budget: &mut ResourceBudget) -> Result<Vec<u8>, AddTableError> {
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge(Resource::Steps, length as u64)
        .map_err(InspectError::from)?;
    bytes.resize(length, 0);
    Ok(bytes)
}

impl Map {
    pub fn capture(
        image: &NativeImage,
        reference: u32,
        additional: impl Iterator<Item = u32>,
        next_page: &mut usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AddTableError> {
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let before = reader.row(reference_page(reference), reference_slot(reference), None)?;
        let mut members = allocation::pages(&mut reader, reference)?;
        let mut changed = false;
        for page in additional {
            changed = true;
            reader.tick()?;
            if members.last().is_some_and(|last| *last >= page) {
                return Err(AddTableError::Source);
            }
            reader
                .budget
                .charge_allocation(1, POINTER_BYTES as u64)
                .map_err(InspectError::from)?;
            members
                .try_reserve_exact(1)
                .map_err(|_| InspectError::Allocation)?;
            members.push(page);
        }
        if !changed {
            return Ok(Self {
                reference,
                bytes: before,
                bitmaps: Vec::new(),
            });
        }
        let base = members.first().copied().unwrap_or(0) / (INLINE_BYTES * 8) as u32
            * (INLINE_BYTES * 8) as u32;
        let inline = before.first() == Some(&0)
            && members
                .last()
                .is_none_or(|last| u64::from(*last - base) < (INLINE_BYTES * 8) as u64);
        if inline {
            let mut bytes = zeros(
                before.len().max(INLINE_HEADER + INLINE_BYTES),
                reader.budget,
            )?;
            bytes[1..INLINE_HEADER].copy_from_slice(&base.to_le_bytes());
            for member in members {
                reader.tick()?;
                let bit = (member - base) as usize;
                bytes[INLINE_HEADER + bit / 8] |= 1 << (bit % 8);
            }
            return Ok(Self {
                reference,
                bytes,
                bitmaps: Vec::new(),
            });
        }
        let old_indirect = before.first() == Some(&1);
        if !old_indirect && before.first() != Some(&0) {
            return Err(AddTableError::Source);
        }
        let old_slots = if old_indirect {
            (before.len() - 1) / POINTER_BYTES
        } else {
            0
        };
        let needed = members
            .last()
            .map_or(0, |page| *page as usize / BITMAP_CAPACITY + 1);
        let slots = old_slots
            .max(needed)
            .max(before.len().saturating_sub(1).div_ceil(POINTER_BYTES));
        let mut bytes = zeros(1 + slots * POINTER_BYTES, reader.budget)?;
        bytes[0] = 1;
        let mut bitmaps = Vec::new();
        for block in 0..slots {
            reader.tick()?;
            let old_pointer = if block < old_slots {
                u32_at(&before, 1 + block * POINTER_BYTES)?
            } else {
                0
            };
            let mut has_members = false;
            for member in &members {
                reader.tick()?;
                if *member as usize / BITMAP_CAPACITY == block {
                    has_members = true;
                    break;
                }
            }
            if old_pointer == 0 && !has_members {
                continue;
            }
            let (number, mut bitmap) = if old_pointer == 0 {
                let number = u32::try_from(*next_page).map_err(|_| AddTableError::Capacity)?;
                *next_page = next_page.checked_add(1).ok_or(AddTableError::Capacity)?;
                let mut bitmap = zeros(PAGE_BYTES, reader.budget)?;
                bitmap[..USAGE_SIGNATURE.len()].copy_from_slice(&USAGE_SIGNATURE);
                (number, bitmap)
            } else {
                let page = reader.page(old_pointer)?;
                if page[..USAGE_SIGNATURE.len()] != USAGE_SIGNATURE {
                    return Err(AddTableError::Source);
                }
                let mut bitmap = zeros(PAGE_BYTES, reader.budget)?;
                bitmap.copy_from_slice(&page);
                (old_pointer, bitmap)
            };
            for member in &members {
                reader.tick()?;
                if *member as usize / BITMAP_CAPACITY == block {
                    let bit = *member as usize % BITMAP_CAPACITY;
                    bitmap[BITMAP_HEADER + bit / 8] |= 1 << (bit % 8);
                }
            }
            bytes[1 + block * POINTER_BYTES..1 + (block + 1) * POINTER_BYTES]
                .copy_from_slice(&number.to_le_bytes());
            reader
                .budget
                .charge_allocation(1, size_of::<Bitmap>() as u64)
                .map_err(InspectError::from)?;
            bitmaps
                .try_reserve_exact(1)
                .map_err(|_| InspectError::Allocation)?;
            bitmaps.push(Bitmap {
                number,
                bytes: bitmap,
            });
        }
        check(reader.budget, cancelled)?;
        Ok(Self {
            reference,
            bytes,
            bitmaps,
        })
    }
}
