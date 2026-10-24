//! Raw String search retains scalar boundaries, including isolated UTF-16 units.
use super::{GeneralTextSearch, find_spans};
use crate::{
    InspectError,
    collation::{NativeSortOrder, comparison::full_key_utf16, key},
    header::check,
};
use std::ops::{Range, RangeInclusive};
use tablecraft_core::limits::ResourceBudget;

const HIGH_SURROGATES: RangeInclusive<u16> = 0xd800..=0xdbff;
const LOW_SURROGATES: RangeInclusive<u16> = 0xdc00..=0xdfff;

/// Find a provisional General substring in complete raw UTF-16 String units.
/// Isolated units retain their weights; a valid surrogate pair remains a single
/// boundary element, as in decoded General search. Results are unit ranges.
/// The existing shortest nonempty span, trailing-space and cumulative resource
/// policies are shared with find_general_text. Unknown orders never fall back.
/// This does not qualify native Access search, code pages or other locales.
pub fn find_general_utf16(
    order: NativeSortOrder,
    haystack: &[u16],
    needle: &[u16],
    search: GeneralTextSearch,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Range<usize>>, InspectError> {
    check(budget, cancelled)?;
    let family = order.general_family().ok_or(InspectError::Unsupported)?;
    let mut work = key::Work { budget, cancelled };
    let needle_key = full_key_utf16(family, needle, &mut work)?;
    let capacity = haystack
        .len()
        .checked_add(1)
        .ok_or(InspectError::StructureLimit)?;
    let mut boundaries = work.vector::<(usize, usize, usize)>(capacity)?;
    let mut offset = 0;
    let mut primary_bound = 0usize;
    while offset < haystack.len() {
        work.tick()?;
        boundaries.push((offset, offset, primary_bound));
        let width = if HIGH_SURROGATES.contains(&haystack[offset])
            && haystack
                .get(offset + 1)
                .is_some_and(|unit| LOW_SURROGATES.contains(unit))
        {
            2
        } else {
            1
        };
        for unit in &haystack[offset..offset + width] {
            work.tick()?;
            primary_bound = primary_bound
                .checked_add(key::primary_byte_bound(family, *unit)?)
                .ok_or(InspectError::StructureLimit)?;
        }
        offset += width;
    }
    boundaries.push((offset, offset, primary_bound));

    find_spans(
        &boundaries,
        &needle_key,
        needle.is_empty(),
        search,
        &mut work,
        |range, work| full_key_utf16(family, &haystack[range], work),
    )
}
