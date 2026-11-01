//! Independent nullable/directional Binary population and raw leaf reconstruction.
use super::{FREE_AT, KEY_AT, MASK_AT, MASK_BYTES, RequestedRows};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex,
    format::{bytes::u16_at, page::PAGE_BYTES},
    header::check,
    table::{
        indexes::mutation::normal_index_audit::binary::{
            Key, distinct, original_value, requested, sort, wire,
        },
        mutation::preserved_source::Source,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn reconstruct(
    image: &NativeImage,
    source: &Source,
    index: &NativeIndex,
    rows: RequestedRows<'_>,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let RequestedRows {
        requests,
        references,
    } = rows;
    if requests.len() != references.len() || source.table.rows().len() != source.records.len() {
        return Err(EditError::Preservation);
    }
    let [key] = index.columns() else {
        return Err(EditError::Preservation);
    };
    let schema = source.table.schema();
    let mut column = None;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() == key.number() {
            column = Some(position);
        }
    }
    let column = column.ok_or(EditError::Preservation)?;
    let field = &schema.columns()[column];
    let mut original = super::super::vector(source.records.len(), budget, cancelled)?;
    for (record, observed) in source.records.iter().zip(source.table.rows()) {
        check(budget, cancelled)?;
        let value = original_value(
            &record.data,
            schema,
            field,
            observed.get(column).ok_or(EditError::Preservation)?,
            budget,
            cancelled,
        )?;
        original.push((value, record.reference));
    }
    sort(&mut original, key.descending(), budget, cancelled)?;

    let base = (index.root_page() as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let page = image
        .as_bytes()
        .get(base..base + PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    let mut original_mask = [0; MASK_BYTES];
    let mut original_bytes = 0_usize;
    for entry in &original {
        check(budget, cancelled)?;
        let (bytes, width) = wire(*entry, key.descending())?;
        let next = original_bytes
            .checked_add(width)
            .ok_or(InspectError::StructureLimit)?;
        if next > PAGE_BYTES - KEY_AT {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        if page.get(KEY_AT + original_bytes..KEY_AT + next) != Some(&bytes[..width]) {
            return Err(EditError::Preservation);
        }
        original_mask[next / 8] |= 1 << (next % 8);
        original_bytes = next;
    }
    budget
        .charge(Resource::Steps, MASK_BYTES as u64)
        .map_err(InspectError::from)?;
    if page[MASK_AT..KEY_AT] != original_mask
        || usize::from(u16_at(page, FREE_AT)?) != PAGE_BYTES - KEY_AT - original_bytes
    {
        return Err(EditError::Preservation);
    }

    let capacity = original
        .len()
        .checked_add(requests.len())
        .ok_or(InspectError::StructureLimit)?;
    let mut final_keys = super::super::vector(capacity, budget, cancelled)?;
    budget
        .charge(
            Resource::Steps,
            (original.len() * size_of::<Key<'_>>()) as u64,
        )
        .map_err(InspectError::from)?;
    final_keys.extend_from_slice(&original);
    for (request, reference) in requests.iter().zip(references) {
        check(budget, cancelled)?;
        let value = requested(
            field,
            request
                .values()
                .get(column)
                .ok_or(EditError::Preservation)?,
        )?;
        final_keys.push((value, *reference));
    }
    sort(&mut final_keys, key.descending(), budget, cancelled)?;
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    let before_distinct = distinct(&original, budget, cancelled)?;
    let after_distinct = distinct(&final_keys, budget, cancelled)?;
    let mut previous: Option<Option<&[u8]>> = None;
    let mut mask = [0; MASK_BYTES];
    let mut active_bytes = 0_usize;
    for entry in &final_keys {
        check(budget, cancelled)?;
        if constraints.unique() && entry.0.is_some() {
            let bytes = entry
                .0
                .map_or(0, <[u8]>::len)
                .max(previous.flatten().map_or(0, <[u8]>::len));
            budget
                .charge(Resource::Steps, bytes as u64)
                .map_err(InspectError::from)?;
        }
        if constraints.required() && entry.0.is_none()
            || constraints.unique() && entry.0.is_some() && previous == Some(entry.0)
        {
            return Err(EditError::Preservation);
        }
        previous = Some(entry.0);
        let (bytes, width) = wire(*entry, key.descending())?;
        let next = active_bytes
            .checked_add(width)
            .ok_or(InspectError::StructureLimit)?;
        if next > PAGE_BYTES - KEY_AT {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        expected
            .get_mut(base + KEY_AT + active_bytes..base + KEY_AT + next)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&bytes[..width]);
        mask[next / 8] |= 1 << (next % 8);
        active_bytes = next;
    }
    budget
        .charge(Resource::Steps, (MASK_BYTES + size_of::<u16>()) as u64)
        .map_err(InspectError::from)?;
    expected
        .get_mut(base + MASK_AT..base + KEY_AT)
        .ok_or(EditError::Preservation)?
        .copy_from_slice(&mask);
    let free = u16::try_from(PAGE_BYTES - KEY_AT - active_bytes)
        .map_err(|_| InspectError::StructureLimit)?;
    expected
        .get_mut(base + FREE_AT..base + FREE_AT + size_of::<u16>())
        .ok_or(EditError::Preservation)?
        .copy_from_slice(&free.to_le_bytes());

    // The original clone retains every inactive tail byte, declaration, map,
    // property and unrelated object. Only each physical active prefix changes.
    super::reconstruct_history(
        image,
        source,
        index,
        (before_distinct, after_distinct),
        expected,
        budget,
        cancelled,
    )
}
