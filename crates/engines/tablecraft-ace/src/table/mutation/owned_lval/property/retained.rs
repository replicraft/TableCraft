//! Fitted property append on actual allocated retained catalog LVAL storage.
use super::{PropertyAppend, capacity};
use crate::{
    EditError, InspectError, LongValueMapReferences, NativeImage, TableSchema, WriteBlocker,
    format::{
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::{MAX_SLOTS, reference, slot_offset},
    },
    header::check,
    table::mutation::owned_lval::{
        maps::{EofAllocation, Family, Maps},
        placement::{Plan, geometry},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const LVAL_OWNER: &[u8; 4] = b"LVAL";

fn capture(
    image: &NativeImage,
    schema: &TableSchema,
    references: LongValueMapReferences,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Maps, EditError> {
    image.require_retained_capture()?;
    let maps = Maps::capture_with_eof_policy(
        image,
        schema,
        references,
        EofAllocation::RequireAllocated,
        budget,
        cancelled,
    )?;
    for &page in maps.pages(Family::LongValue) {
        check(budget, cancelled)?;
        let start = page as usize * PAGE_BYTES;
        let raw = image
            .as_bytes()
            .get(start..start + PAGE_BYTES)
            .ok_or(InspectError::Truncated)?;
        budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        if raw[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || &raw[OWNER_OFFSET..OWNER_OFFSET + LVAL_OWNER.len()] != LVAL_OWNER
        {
            return Err(InspectError::Malformed.into());
        }
        let count = row_count(raw)?;
        if count > MAX_SLOTS {
            return Err(InspectError::Malformed.into());
        }
        let mut end = PAGE_BYTES;
        for slot in 0..count {
            check(budget, cancelled)?;
            let (word, start, limit) = row_range(raw, slot)?;
            if usize::from(word) != start
                || limit != end
                || start >= limit
                || start < slot_offset(count)
            {
                return Err(InspectError::Malformed.into());
            }
            end = start;
        }
        geometry(raw)?;
    }
    Ok(maps)
}

struct Selected {
    page: u32,
    count: usize,
    end: usize,
    available: usize,
}

fn selected(
    image: &NativeImage,
    maps: &Maps,
    minimum: Option<usize>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Selected>, EditError> {
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut selected: Option<Selected> = None;
    for &page in maps.pages(Family::LongValue).iter().rev() {
        reader.tick()?;
        let hint = maps.long_value.hint.cell(&mut reader, page)?;
        if hint.before & hint.mask == 0 {
            continue;
        }
        let start = page as usize * PAGE_BYTES;
        let raw = image
            .as_bytes()
            .get(start..start + PAGE_BYTES)
            .ok_or(InspectError::Truncated)?;
        let (count, end) = geometry(raw)?;
        let Some(available) = capacity(count, end) else {
            continue;
        };
        let current = Selected {
            page,
            count,
            end,
            available,
        };
        if let Some(minimum) = minimum {
            if available >= minimum {
                return Ok(Some(current));
            }
        } else if selected
            .as_ref()
            .is_none_or(|prior| available > prior.available)
        {
            selected = Some(current);
        }
    }
    Ok(selected)
}

/// Capacity of the real selected candidate; this observer cannot allocate EOF.
pub(in crate::table::mutation) fn fitted_capacity(
    image: &NativeImage,
    schema: &TableSchema,
    references: LongValueMapReferences,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<usize>, EditError> {
    let maps = capture(image, schema, references, budget, cancelled)?;
    Ok(selected(image, &maps, None, budget, cancelled)?.map(|selected| selected.available))
}

/// Append only when the shared allocator's selected owned candidate fits.
/// Every map, old body and existing physical slot remains unchanged.
pub(in crate::table::mutation) fn append_fitted(
    image: &NativeImage,
    schema: &TableSchema,
    references: LongValueMapReferences,
    body: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PropertyAppend, EditError> {
    let maps = capture(image, schema, references, budget, cancelled)?;
    let Selected {
        page, count, end, ..
    } = selected(image, &maps, Some(body.len()), budget, cancelled)?
        .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
    if body.is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let start = page as usize * PAGE_BYTES;
    let original = &image.as_bytes()[start..start + PAGE_BYTES];
    let mut plan = Plan::new(image, schema.definition_page(), maps);
    let index = plan.append_existing_long_value(image, page, body, budget, cancelled)?;
    let appended = &plan.appended[index];
    if plan.original_length != image.as_bytes().len()
        || plan.length != image.as_bytes().len()
        || !plan.maps.edits.is_empty()
        || plan.pages.len() != 1
        || plan.attempts.len() != 1
        || plan.appended.len() != 1
        || plan.attempts[0].page != page
        || plan.attempts[0].family != Family::LongValue
        || plan.attempts[0].bytes != body.len()
        || !plan.attempts[0].fitted
        || appended.reference
            != reference(
                page,
                u8::try_from(count).map_err(|_| InspectError::Malformed)?,
            )
        || appended.family != Family::LongValue
        || appended.range != (start + end - body.len()..start + end)
    {
        return Err(EditError::Preservation);
    }
    let reference = appended.reference;
    let changed = plan.pages.pop().ok_or(EditError::Preservation)?;
    budget
        .charge(Resource::Steps, (original.len() + body.len()) as u64)
        .map_err(InspectError::from)?;
    if changed.number != page
        || changed.family != Family::LongValue
        || changed.before != original
        || changed.after.get(end - body.len()..end) != Some(body)
    {
        return Err(EditError::Preservation);
    }
    Ok(PropertyAppend {
        page,
        bytes: changed.after,
        reference,
        length: plan.length,
        maps: plan.maps.edits,
    })
}
