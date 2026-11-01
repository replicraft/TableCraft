//! Independent decreasing-cardinality Binary proof; no staged removal facts.
use super::vector;
use crate::{
    EditError, InspectError, NativeImage, NativeIndex,
    format::{bytes::u16_at, page::PAGE_BYTES},
    header::check,
    table::{
        indexes::mutation::normal_index_audit::binary::{Key, original_value, sort, wire},
        mutation::preserved_source::Source,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_AT: usize = 2;
const MASK_AT: usize = 27;
const ACTIVE_AT: usize = 480;
const MASK_BYTES: usize = ACTIVE_AT - MASK_AT;
const BIT_WIDTH: usize = u8::BITS as usize;

pub(super) fn reconstruct(
    image: &NativeImage,
    source: &Source,
    index: &NativeIndex,
    removed: &[u32],
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let [column] = index.columns() else {
        return Err(EditError::Preservation);
    };
    let schema = source.table.schema();
    budget
        .charge(Resource::Steps, schema.columns().len() as u64)
        .map_err(InspectError::from)?;
    let position = schema
        .columns()
        .iter()
        .position(|field| field.number() == column.number())
        .ok_or(EditError::Preservation)?;
    let field = &schema.columns()[position];
    if source.records.len() != source.table.rows().len() {
        return Err(EditError::Preservation);
    }
    let mut original = vector(source.records.len(), budget, cancelled)?;
    for (record, row) in source.records.iter().zip(source.table.rows()) {
        check(budget, cancelled)?;
        let value = original_value(
            &record.data,
            schema,
            field,
            row.get(position).ok_or(EditError::Preservation)?,
            budget,
            cancelled,
        )?;
        original.push((value, record.reference));
    }
    sort(&mut original, column.descending(), budget, cancelled)?;
    let base = (index.root_page() as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let before = image
        .as_bytes()
        .get(base..base + PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    require_original(before, &original, column.descending(), budget, cancelled)?;

    let mut survivors = vector(original.len(), budget, cancelled)?;
    let mut count = 0_usize;
    for entry in original {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, removed.len() as u64)
            .map_err(InspectError::from)?;
        if removed.contains(&entry.1) {
            count += 1;
        } else {
            survivors.push(entry);
        }
    }
    if count != removed.len() {
        return Err(EditError::Preservation);
    }
    if removed.is_empty() {
        return Ok(());
    }
    let mut mask = [0; MASK_BYTES];
    let mut active = 0_usize;
    for entry in survivors {
        check(budget, cancelled)?;
        let (bytes, width) = wire(entry, column.descending())?;
        let next = active
            .checked_add(width)
            .filter(|next| *next <= PAGE_BYTES - ACTIVE_AT)
            .ok_or(EditError::Preservation)?;
        budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        expected
            .get_mut(base + ACTIVE_AT + active..base + ACTIVE_AT + next)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&bytes[..width]);
        mask[next / BIT_WIDTH] |= 1 << (next % BIT_WIDTH);
        active = next;
    }
    budget
        .charge(Resource::Steps, (MASK_BYTES + size_of::<u16>()) as u64)
        .map_err(InspectError::from)?;
    expected
        .get_mut(base + MASK_AT..base + ACTIVE_AT)
        .ok_or(EditError::Preservation)?
        .copy_from_slice(&mask);
    let free =
        u16::try_from(PAGE_BYTES - ACTIVE_AT - active).map_err(|_| EditError::Preservation)?;
    expected
        .get_mut(base + FREE_AT..base + FREE_AT + size_of::<u16>())
        .ok_or(EditError::Preservation)?
        .copy_from_slice(&free.to_le_bytes());
    // No history, descriptor, prefix or tail write occurs. The complete proof
    // retains the original image outside the independently derived active area.
    check(budget, cancelled)?;
    Ok(())
}

fn require_original(
    before: &[u8],
    keys: &[Key<'_>],
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut mask = [0; MASK_BYTES];
    let mut active = 0_usize;
    for entry in keys {
        check(budget, cancelled)?;
        let (bytes, width) = wire(*entry, descending)?;
        let next = active
            .checked_add(width)
            .filter(|next| *next <= PAGE_BYTES - ACTIVE_AT)
            .ok_or(EditError::Preservation)?;
        budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        if before.get(ACTIVE_AT + active..ACTIVE_AT + next) != Some(&bytes[..width]) {
            return Err(EditError::Preservation);
        }
        mask[next / BIT_WIDTH] |= 1 << (next % BIT_WIDTH);
        active = next;
    }
    budget
        .charge(Resource::Steps, MASK_BYTES as u64)
        .map_err(InspectError::from)?;
    if before[MASK_AT..ACTIVE_AT] != mask
        || usize::from(u16_at(before, FREE_AT)?) != PAGE_BYTES - ACTIVE_AT - active
    {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
