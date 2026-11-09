//! One source-qualified property-page rollover, using the real append owner.
use super::{
    maps::{EofAllocation, Family, Maps},
    placement::{Plan, geometry},
};
use crate::{
    EditError, InspectError, LongValueMapReferences, NativeImage, TableSchema, WriteBlocker,
    format::{
        allocation,
        page::PAGE_BYTES,
        reader::Reader,
        row::{MAX_SLOTS, SLOT_BYTES, reference, slot_offset},
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FIRST_PAGE: u32 = 10;
const LAST_PAGE: u32 = FIRST_PAGE + 1;
const INLINE_MAP: u8 = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Growth {
    OneEof,
    FittedOnly,
}

pub(in crate::table::mutation) struct PropertyAppend {
    pub page: u32,
    pub bytes: Vec<u8>,
    pub reference: u32,
    pub length: usize,
    pub maps: Vec<super::maps::Edit>,
}

pub(in crate::table::mutation) fn capture_maps(
    image: &NativeImage,
    schema: &TableSchema,
    references: LongValueMapReferences,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Maps, EditError> {
    let maps = Maps::capture_with_eof_policy(
        image,
        schema,
        references,
        EofAllocation::RequireAllocated,
        budget,
        cancelled,
    )?;
    let pages = maps.pages(Family::LongValue);
    if maps.global.kind() != INLINE_MAP
        || maps.long_value.owned.kind() != INLINE_MAP
        || maps.long_value.hint.kind() != INLINE_MAP
        || !matches!(pages, [FIRST_PAGE] | [FIRST_PAGE, LAST_PAGE])
        || image.as_bytes().len() != (FIRST_PAGE as usize + pages.len()) * PAGE_BYTES
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let candidates = allocation::pages(&mut reader, references.candidate())?;
    if candidates.as_slice() != &pages[pages.len() - 1..] {
        return Err(EditError::Preservation);
    }
    if pages.len() == 1 {
        for map in [
            &maps.ordinary.owned,
            &maps.ordinary.hint,
            &maps.long_value.owned,
            &maps.long_value.hint,
            &maps.global,
        ] {
            let cell = map.future_cell(&mut reader, LAST_PAGE)?;
            if cell.before & cell.mask != 0 {
                return Err(EditError::Preservation);
            }
        }
    }
    // Scope::capture proves represented metadata allocation and ordinary/LVAL
    // separation. Full canonical history is still the caller's final authority.
    Ok(maps)
}

pub(in crate::table::mutation) fn append(
    image: &NativeImage,
    schema: &TableSchema,
    references: LongValueMapReferences,
    body: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PropertyAppend, EditError> {
    append_with_growth(
        image,
        schema,
        references,
        body,
        Growth::OneEof,
        budget,
        cancelled,
    )
}

pub(in crate::table::mutation) fn append_fitted(
    image: &NativeImage,
    schema: &TableSchema,
    references: LongValueMapReferences,
    body: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PropertyAppend, EditError> {
    append_with_growth(
        image,
        schema,
        references,
        body,
        Growth::FittedOnly,
        budget,
        cancelled,
    )
}

fn append_with_growth(
    image: &NativeImage,
    schema: &TableSchema,
    references: LongValueMapReferences,
    body: &[u8],
    growth: Growth,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PropertyAppend, EditError> {
    check(budget, cancelled)?;
    let maps = capture_maps(image, schema, references, budget, cancelled)?;
    let page = *maps
        .pages(Family::LongValue)
        .last()
        .ok_or(EditError::Preservation)?;
    let start = page as usize * PAGE_BYTES;
    let original = &image.as_bytes()[start..start + PAGE_BYTES];
    let (count, end) = geometry(original)?;
    let fitted = count < MAX_SLOTS
        && body
            .len()
            .checked_add(SLOT_BYTES)
            .is_some_and(|needed| needed <= end - slot_offset(count));
    if !fitted && (page == LAST_PAGE || growth == Growth::FittedOnly) {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    // Derive the exact two represented-byte edits from the real map owner.
    // The global cell is already allocated and is never an edit target.
    let expected_edits = if fitted {
        None
    } else {
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let old_hint = maps.long_value.hint.cell(&mut reader, FIRST_PAGE)?;
        let new_hint = maps.long_value.hint.future_cell(&mut reader, LAST_PAGE)?;
        let new_owned = maps.long_value.owned.future_cell(&mut reader, LAST_PAGE)?;
        let global = maps.global.future_cell(&mut reader, LAST_PAGE)?;
        if old_hint.offset != new_hint.offset
            || old_hint.before != new_hint.before
            || new_owned.offset == new_hint.offset
            || global.offset == new_owned.offset
            || global.offset == new_hint.offset
        {
            return Err(EditError::Preservation);
        }
        Some([
            (
                new_hint.offset,
                new_hint.before,
                (new_hint.before & !old_hint.mask) | new_hint.mask,
            ),
            (
                new_owned.offset,
                new_owned.before,
                new_owned.before | new_owned.mask,
            ),
        ])
    };
    let mut plan = Plan::new(image, schema.definition_page(), maps);
    let index = plan.append(image, Family::LongValue, body, budget, cancelled)?;
    let target = if fitted { page } else { LAST_PAGE };
    let expected_length = image.as_bytes().len() + usize::from(!fitted) * PAGE_BYTES;
    if plan.original_length != image.as_bytes().len()
        || plan.length != expected_length
        || plan.appended.len() != 1
        || plan.attempts.len() != 1
        || plan.attempts[0].family != Family::LongValue
        || plan.attempts[0].page != page
        || plan.attempts[0].bytes != body.len()
        || plan.attempts[0].fitted != fitted
        || plan.maps.pages(Family::LongValue)
            != if target == FIRST_PAGE {
                &[FIRST_PAGE][..]
            } else {
                &[FIRST_PAGE, LAST_PAGE][..]
            }
        || fitted && !plan.maps.edits.is_empty()
    {
        return Err(EditError::Preservation);
    }
    let appended = &plan.appended[index];
    let slot = if fitted { count } else { 0 };
    let target_end = if fitted { end } else { PAGE_BYTES };
    let expected_start = target as usize * PAGE_BYTES + target_end - body.len();
    if appended.family != Family::LongValue
        || appended.range != (expected_start..target as usize * PAGE_BYTES + target_end)
        || appended.reference
            != reference(
                target,
                u8::try_from(slot).map_err(|_| InspectError::Malformed)?,
            )
    {
        return Err(EditError::Preservation);
    }
    let reference = appended.reference;
    // Plan retains the failed candidate page. It must be byte-identical before
    // discarding that proof carrier; only the new EOF page is published.
    if !fitted {
        if page != FIRST_PAGE || plan.pages.len() != 2 || plan.maps.edits.len() != 2 {
            return Err(EditError::Preservation);
        }
        let expected = expected_edits.ok_or(EditError::Preservation)?;
        for (offset, before, after) in expected {
            if !plan
                .maps
                .edits
                .iter()
                .any(|edit| edit.offset == offset && edit.before == before && edit.after == after)
            {
                return Err(EditError::Preservation);
            }
        }
        let failed = plan.pages.remove(0);
        budget
            .charge(Resource::Steps, (2 * original.len()) as u64)
            .map_err(InspectError::from)?;
        if failed.number != FIRST_PAGE || failed.before != original || failed.after != original {
            return Err(EditError::Preservation);
        }
    }
    let changed = plan.pages.pop().ok_or(EditError::Preservation)?;
    let comparison_bytes = body.len() + if fitted { original.len() } else { 0 };
    budget
        .charge(Resource::Steps, comparison_bytes as u64)
        .map_err(InspectError::from)?;
    if !plan.pages.is_empty()
        || changed.number != target
        || changed.family != Family::LongValue
        || fitted && changed.before != original
        || !fitted && !changed.before.is_empty()
        || changed.after.get(target_end - body.len()..target_end) != Some(body)
    {
        return Err(EditError::Preservation);
    }
    Ok(PropertyAppend {
        page: target,
        bytes: changed.after,
        reference,
        length: plan.length,
        maps: plan.maps.edits,
    })
}
