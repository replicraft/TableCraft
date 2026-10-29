//! Complete French weights with metered, primary-only candidate pruning.
use super::{qualify, units_key};
use crate::{
    GeneralSortFamily, InspectError, NativeSortOrder,
    collation::{GeneralTextSearch, key, search::find_spans},
    header::check,
};
use std::{ops::Range, ops::RangeInclusive};
use tablecraft_core::limits::ResourceBudget;

const HIGH_SURROGATES: RangeInclusive<u16> = 0xd800..=0xdbff;
const LOW_SURROGATES: RangeInclusive<u16> = 0xdc00..=0xdfff;

#[cfg(test)]
mod tests;

/// Zero-based UTF-16 bounds for provisional French legacy substring matching.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrenchTextSearch {
    /// Find the earliest start at or after this UTF-16 offset.
    ForwardFrom(usize),
    /// Find the latest start whose complete span ends at/before this boundary.
    ReverseThrough(usize),
}

/// Search decoded text under the exact French legacy 1036/0/0 weight model.
///
/// Returns original UTF-16 spans, choosing the shortest matching nonempty span
/// at the selected start. SQL trailing-space equivalence applies to complete
/// candidates, without physical index truncation. An actually empty needle
/// selects a boundary; a nonempty ignorable needle must consume input.
/// This engineering policy does not qualify native Access/NLS matching or LIKE.
/// All conversion, allocation, candidate encoding and comparison are metered.
pub fn find_french_legacy_text(
    order: NativeSortOrder,
    haystack: &str,
    needle: &str,
    search: FrenchTextSearch,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Range<usize>>, InspectError> {
    check(budget, cancelled)?;
    qualify(order)?;
    let mut work = key::Work { budget, cancelled };
    let haystack = text_units(haystack, &mut work)?;
    let needle = text_units(needle, &mut work)?;
    find_units(&haystack, &needle, search, &mut work)
}

/// Search complete UTF-16 Strings under the provisional French weight model.
///
/// Isolated surrogates retain their measured weights. A valid pair is one
/// boundary element; bounds inside it round toward the searched region.
/// Decoded and raw searches share consumed-span, padding and cumulative-budget
/// rules. Unsupported descriptors fail without a General or ordinal fallback.
pub fn find_french_legacy_utf16(
    order: NativeSortOrder,
    haystack: &[u16],
    needle: &[u16],
    search: FrenchTextSearch,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Range<usize>>, InspectError> {
    check(budget, cancelled)?;
    qualify(order)?;
    find_units(
        haystack,
        needle,
        search,
        &mut key::Work { budget, cancelled },
    )
}

fn text_units(text: &str, work: &mut key::Work<'_>) -> Result<Vec<u16>, InspectError> {
    // UTF-8 byte length bounds UTF-16 unit count without an unmetered pre-scan.
    let mut units = work.vector(text.len())?;
    for unit in text.encode_utf16() {
        work.tick()?;
        units.push(unit);
    }
    Ok(units)
}

fn find_units(
    haystack: &[u16],
    needle: &[u16],
    search: FrenchTextSearch,
    work: &mut key::Work<'_>,
) -> Result<Option<Range<usize>>, InspectError> {
    // Encode the whole needle eagerly, before any search-bound shortcut.
    let needle_key = units_key(needle, work)?;
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
                .checked_add(key::primary_byte_bound(GeneralSortFamily::Legacy, *unit)?)
                .ok_or(InspectError::StructureLimit)?;
        }
        offset += width;
    }
    boundaries.push((offset, offset, primary_bound));

    // encode_full_french changes ONLY the secondary stream after constructing
    // the same Legacy primary stream. Appending units cannot change earlier
    // primary bytes, including contextual repeats. Removing trailing spaces
    // can delay those bytes but cannot remove a non-space primary prefix.
    // Thus the shared scheduler's primary-prefix and primary-length proofs
    // apply unchanged. No secondary prefix or secondary ordering is used for
    // pruning: every possible match compares complete reversed French weights.
    let search = match search {
        FrenchTextSearch::ForwardFrom(start) => GeneralTextSearch::ForwardFrom(start),
        FrenchTextSearch::ReverseThrough(end) => GeneralTextSearch::ReverseThrough(end),
    };
    find_spans(
        &boundaries,
        &needle_key,
        needle.is_empty(),
        search,
        work,
        |range, work| units_key(&haystack[range], work),
    )
}
