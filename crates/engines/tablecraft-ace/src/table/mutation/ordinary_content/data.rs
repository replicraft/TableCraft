//! Complete ordinary user-row framing and data-role coverage, not program scans.
use super::{
    definition::Table,
    ledger::{Ledger, Role},
    records, refused,
};
use crate::{
    EditError, InspectError,
    format::{
        bytes::u16_at,
        data_type::{BOOLEAN, SHORT_BINARY},
        layout::Layout,
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const COLUMN_COUNT_OFFSET: usize = 0;
const COLUMN_COUNT_BYTES: usize = size_of::<u16>();
const VARIABLE_WORD_BYTES: usize = size_of::<u16>();
const MASK_BITS: usize = u8::BITS as usize;

pub(super) fn validate(
    ledger: &mut Ledger<'_>,
    definition: u32,
    table: &Table<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut total = 0_u32;
    for number in ledger.members(table.owned, budget, cancelled)? {
        ledger.claim(number, Role::Data, budget, cancelled)?;
        let page = ledger.page(number)?;
        let count = records::count(page, definition, budget, cancelled)?;
        for ordinal in 0..count {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Rows, 1)
                .map_err(InspectError::from)?;
            row(
                records::row(page, ordinal)?,
                &table.layout,
                budget,
                cancelled,
            )?;
        }
        total = total
            .checked_add(count as u32)
            .ok_or(InspectError::StructureLimit)?;
    }
    if total != table.layout.records {
        return Err(refused());
    }
    Ok(())
}

fn row(
    bytes: &[u8],
    layout: &Layout,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    if usize::from(u16_at(bytes, COLUMN_COUNT_OFFSET)?) != layout.max_columns {
        return Err(refused());
    }
    let mask_bytes = layout.max_columns.div_ceil(MASK_BITS);
    let mask = bytes.len().checked_sub(mask_bytes).ok_or_else(refused)?;
    if !layout.max_columns.is_multiple_of(MASK_BITS) {
        let allowed = (1_u8 << (layout.max_columns % MASK_BITS)) - 1;
        if bytes[bytes.len() - 1] & !allowed != 0 {
            return Err(refused());
        }
    }
    let mut offsets = Vec::new();
    let data_end = if layout.variable_slots == 0 {
        if mask != layout.fixed_end {
            return Err(refused());
        }
        mask
    } else {
        let count_at = mask.checked_sub(VARIABLE_WORD_BYTES).ok_or_else(refused)?;
        if usize::from(u16_at(bytes, count_at)?) != layout.variable_slots {
            return Err(refused());
        }
        let end = count_at
            .checked_sub(VARIABLE_WORD_BYTES * (layout.variable_slots + 1))
            .ok_or_else(refused)?;
        budget
            .charge_allocation(
                (layout.variable_slots + 1) as u64,
                size_of::<usize>() as u64,
            )
            .map_err(InspectError::from)?;
        offsets
            .try_reserve_exact(layout.variable_slots + 1)
            .map_err(|_| InspectError::Allocation)?;
        let mut previous = layout.fixed_end;
        for index in 0..=layout.variable_slots {
            check(budget, cancelled)?;
            let offset = usize::from(u16_at(bytes, count_at - VARIABLE_WORD_BYTES * (index + 1))?);
            if offset < previous || offset > end {
                return Err(refused());
            }
            offsets.push(offset);
            previous = offset;
        }
        if offsets[0] != layout.fixed_end {
            return Err(refused());
        }
        // The final offset ends active variable payloads. The independently
        // bounded gap before the footer is inactive record capacity, preserved
        // as data rather than interpreted as another field or payload reference.
        // Each offset was checked against the physical footer boundary above.
        end
    };
    if layout.fixed_end > data_end {
        return Err(refused());
    }
    budget
        .charge_allocation(layout.fixed_end as u64, size_of::<bool>() as u64)
        .map_err(InspectError::from)?;
    let mut fixed = Vec::new();
    fixed
        .try_reserve_exact(layout.fixed_end)
        .map_err(|_| InspectError::Allocation)?;
    fixed.resize(layout.fixed_end, false);
    fixed[..COLUMN_COUNT_BYTES].fill(true);
    for field in &layout.fields {
        check(budget, cancelled)?;
        let present =
            bytes[mask + field.number / MASK_BITS] & (1 << (field.number % MASK_BITS)) != 0;
        if field.fixed {
            if field.kind == BOOLEAN {
                continue;
            }
            let start = COLUMN_COUNT_BYTES
                .checked_add(field.offset)
                .ok_or_else(refused)?;
            let end = start.checked_add(field.length).ok_or_else(refused)?;
            let span = fixed.get_mut(start..end).ok_or_else(refused)?;
            budget
                .charge(Resource::Steps, span.len() as u64)
                .map_err(InspectError::from)?;
            if span.iter().any(|claimed| *claimed) {
                return Err(refused());
            }
            span.fill(true);
            // Both active and inactive fixed bytes belong to a declared scalar
            // storage role. A NULL mask never authorizes rewriting them.
        } else {
            let start = *offsets.get(field.variable).ok_or_else(refused)?;
            let end = *offsets.get(field.variable + 1).ok_or_else(refused)?;
            if !present && start != end {
                return Err(refused());
            }
            if present && field.kind == SHORT_BINARY && end - start > field.length {
                return Err(refused());
            }
            // Actual full-table scalar decoding separately validates Text/
            // Decimal payloads, including escaped Text bytes exceeding UTF16
            // declaration width. They remain data, not XML/program candidates.
        }
    }
    budget
        .charge(Resource::Steps, fixed.len() as u64)
        .map_err(InspectError::from)?;
    for (at, claimed) in fixed.into_iter().enumerate() {
        if !claimed && bytes[at] != 0 {
            return Err(refused());
        }
    }
    Ok(())
}
