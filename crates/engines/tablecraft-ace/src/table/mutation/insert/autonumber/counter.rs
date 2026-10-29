//! Source-bound last-issued counter; generation never scans or repairs live IDs.
use super::IncrementingAutoNumber;
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::{
        layout::{AUTO_NUMBER_FLAG, AUTO_NUMBER_GUID_FLAG, FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
    },
    header::check,
    table::mutation::insert::{counters, primary_column},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

// Pinned TableImpl stores the LAST issued LONG here, despite the upstream
// OFFSET_NEXT_AUTO_NUMBER name. It is independent of live and historical rows.
const LAST_ISSUED_OFFSET: usize = 20;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::table::mutation::insert) struct Source {
    pub column: usize,
    pub offset: usize,
    pub before: [u8; size_of::<i32>()],
    pub last: i32,
}

pub(in crate::table::mutation::insert) struct Plan {
    pub source: Source,
    pub after: [u8; size_of::<i32>()],
    pub values: Vec<i32>,
}

impl Plan {
    pub(in crate::table::mutation::insert) fn patch(&self) -> Option<counters::Patch> {
        (self.source.before != self.after).then_some(counters::Patch {
            offset: self.source.offset,
            before: self.source.before,
            after: self.after,
        })
    }
}

pub(in crate::table::mutation::insert) fn bind(
    image: &NativeImage,
    checked: &TableCheck,
    policy: IncrementingAutoNumber,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    let source = source(image, checked, budget, cancelled)?;
    if source.last != policy.expected_last() {
        return Err(EditError::Conflict);
    }
    Ok(source)
}

pub(in crate::table::mutation::insert) fn source(
    image: &NativeImage,
    checked: &TableCheck,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    let column = primary_column::resolve(checked, budget, cancelled)?;
    for (position, field) in checked.schema().columns().iter().enumerate() {
        check(budget, cancelled)?;
        if position == column {
            if field.scalar_type() != Some(ScalarType::Int32)
                || !field.fixed()
                || field.storage_length() != size_of::<i32>()
                || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG | AUTO_NUMBER_FLAG)
                || field.extended_flags() != 0
                || field.complex_type_id().is_some()
            {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        } else if field.flags() & (AUTO_NUMBER_FLAG | AUTO_NUMBER_GUID_FLAG) != 0 {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }
    let offset = (checked.schema().definition_page() as usize)
        .checked_mul(PAGE_BYTES)
        .and_then(|base| base.checked_add(LAST_ISSUED_OFFSET))
        .ok_or(InspectError::StructureLimit)?;
    let before: [u8; size_of::<i32>()] = image
        .as_bytes()
        .get(offset..offset + size_of::<i32>())
        .ok_or(InspectError::Truncated)?
        .try_into()
        .map_err(|_| InspectError::Truncated)?;
    let last = i32::from_le_bytes(before);
    Ok(Source {
        column,
        offset,
        before,
        last,
    })
}

pub(in crate::table::mutation::insert) fn prepare(
    source: Source,
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Plan, EditError> {
    check(budget, cancelled)?;
    let delta = i32::try_from(count).map_err(|_| EditError::AutoNumberExhausted)?;
    let last = source
        .last
        .checked_add(delta)
        .ok_or(EditError::AutoNumberExhausted)?;
    budget
        .charge_allocation(count as u64, size_of::<i32>() as u64)
        .map_err(InspectError::from)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for ordinal in 0..count {
        check(budget, cancelled)?;
        let step = i32::try_from(ordinal + 1).map_err(|_| EditError::AutoNumberExhausted)?;
        values.push(
            source
                .last
                .checked_add(step)
                .ok_or(EditError::AutoNumberExhausted)?,
        );
    }
    Ok(Plan {
        source,
        after: last.to_le_bytes(),
        values,
    })
}
