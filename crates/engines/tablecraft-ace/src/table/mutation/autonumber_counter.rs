//! Exact last-issued LONG facts and arithmetic, independent of mode/primary policy.
use crate::{
    EditError, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::{
        layout::{AUTO_NUMBER_FLAG, AUTO_NUMBER_GUID_FLAG, FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
    },
    header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

const LAST_ISSUED_OFFSET: usize = 20;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::table::mutation) struct Source {
    pub column: usize,
    pub offset: usize,
    pub before: [u8; size_of::<i32>()],
    pub last: i32,
}

pub(in crate::table::mutation) struct Plan {
    pub source: Source,
    pub after: [u8; size_of::<i32>()],
    pub values: Vec<i32>,
}

pub(in crate::table::mutation) fn capture(
    image: &NativeImage,
    schema: &TableSchema,
    column: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    for (position, field) in schema.columns().iter().enumerate() {
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
    let offset = (schema.definition_page() as usize)
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

pub(in crate::table::mutation) fn prepare(
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
