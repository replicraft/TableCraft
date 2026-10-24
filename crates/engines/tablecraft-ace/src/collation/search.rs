//! Metered substring search under the explicit full-string General weight model.
mod raw;

use super::{
    NativeSortOrder,
    comparison::{compare_keys, full_key},
    key,
};
use crate::{InspectError, header::check};
pub use raw::find_general_utf16;
use std::{cmp::Ordering, ops::Range};
use tablecraft_core::limits::ResourceBudget;

/// Explicit zero-based UTF-16 bounds for provisional General substring search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralTextSearch {
    /// Find the earliest valid string boundary at or after this offset.
    ForwardFrom(usize),
    /// Find the latest start whose complete match ends at/before this boundary.
    ReverseThrough(usize),
}

/// Find a full-weight-equivalent substring, returning its consumed UTF-16 span.
///
/// This provisional model accepts unequal-length case/accent/expansion matches.
/// Candidate spans use valid Unicode string boundaries and are never prefix-
/// truncated. At a chosen start, the shortest matching nonempty span wins. An
/// actually empty needle returns the selected boundary, while a nonempty
/// weight-ignorable needle must still consume text. Bounds inside surrogate
/// pairs round toward the searched region. Native Access search qualification
/// remains separate; no OS locale or module setting is inferred.
///
/// Allocation/scans/comparisons charge one cumulative caller budget. Primary
/// weights are append-only within this model: a candidate that is not a prefix
/// of the needle's primary weights cannot become a match by extending its end.
/// This pruning never uses truncated physical index keys or skips required
/// validation of a supplied sort descriptor/needle.
pub fn find_general_text(
    order: NativeSortOrder,
    haystack: &str,
    needle: &str,
    search: GeneralTextSearch,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Range<usize>>, InspectError> {
    check(budget, cancelled)?;
    let family = order.general_family().ok_or(InspectError::Unsupported)?;
    let mut work = key::Work { budget, cancelled };
    let needle_key = full_key(family, needle, &mut work)?;
    let capacity = haystack
        .len()
        .checked_add(1)
        .ok_or(InspectError::StructureLimit)?;
    let mut boundaries = work.vector::<(usize, usize, usize)>(capacity)?;
    let mut units = 0;
    let mut primary_bound = 0usize;
    let mut unit_iter = haystack.encode_utf16();
    for (byte, character) in haystack.char_indices() {
        work.tick()?;
        boundaries.push((byte, units, primary_bound));
        for _ in 0..character.len_utf16() {
            work.tick()?;
            let unit = unit_iter.next().ok_or(InspectError::Malformed)?;
            primary_bound = primary_bound
                .checked_add(key::primary_byte_bound(family, unit)?)
                .ok_or(InspectError::StructureLimit)?;
        }
        units += character.len_utf16();
    }
    boundaries.push((haystack.len(), units, primary_bound));
    find_spans(
        &boundaries,
        &needle_key,
        needle.is_empty(),
        search,
        &mut work,
        |range, work| full_key(family, &haystack[range], work),
    )
}

fn find_spans(
    boundaries: &[(usize, usize, usize)],
    needle_key: &key::ComparisonKey,
    empty_needle: bool,
    search: GeneralTextSearch,
    work: &mut key::Work<'_>,
    mut encode: impl FnMut(Range<usize>, &mut key::Work<'_>) -> Result<key::ComparisonKey, InspectError>,
) -> Result<Option<Range<usize>>, InspectError> {
    let units = boundaries.last().ok_or(InspectError::Malformed)?.1;
    let (first, last, reverse) = match search {
        GeneralTextSearch::ForwardFrom(start) => (start, units, false),
        GeneralTextSearch::ReverseThrough(end) => (0, end.min(units), true),
    };
    if empty_needle {
        for position in 0..boundaries.len() {
            work.tick()?;
            let index = if reverse {
                boundaries.len() - 1 - position
            } else {
                position
            };
            let (_, units, _) = boundaries[index];
            if (reverse && units <= last) || (!reverse && units >= first) {
                work.tick()?;
                return Ok(Some(units..units));
            }
        }
        work.tick()?;
        return Ok(None);
    }
    let mut final_end = boundaries.len() - 1;
    while boundaries[final_end].1 > last {
        work.tick()?;
        final_end -= 1;
    }
    for position in 0..final_end {
        work.tick()?;
        let start = if reverse {
            final_end - 1 - position
        } else {
            position
        };
        let (start_offset, start_unit, prefix_bound) = boundaries[start];
        if start_unit < first || start_unit >= last {
            continue;
        }
        // Static inline lengths bound the remaining primary stream; repeats
        // use the maximum contextual inline width. This skips provably too-short
        // suffixes without assuming equal source/needle character lengths.
        if boundaries[final_end].2 - prefix_bound < needle_key.primary_length {
            continue;
        }
        let Some(first_end) =
            first_complete(boundaries, start, final_end, needle_key, work, &mut encode)?
        else {
            continue;
        };
        for &(end_offset, end_unit, _) in &boundaries[first_end..=final_end] {
            work.tick()?;
            if end_unit > last {
                break;
            }
            let candidate = encode(start_offset..end_offset, work)?;
            if !can_extend(&candidate, needle_key, work)? {
                break;
            }
            if compare_keys(&candidate, needle_key, work)? == Ordering::Equal {
                work.tick()?;
                return Ok(Some(start_unit..end_unit));
            }
        }
    }
    work.tick()?;
    Ok(None)
}

fn first_complete(
    boundaries: &[(usize, usize, usize)],
    start: usize,
    final_end: usize,
    needle: &key::ComparisonKey,
    work: &mut key::Work<'_>,
    encode: &mut impl FnMut(
        Range<usize>,
        &mut key::Work<'_>,
    ) -> Result<key::ComparisonKey, InspectError>,
) -> Result<Option<usize>, InspectError> {
    let begin = boundaries[start].0;
    let mut low = start + 1;
    let mut high = low;
    // Primary lengths only increase. Exponential probing plus a lower-bound
    // search avoids encoding every shorter prefix of a long matching needle.
    loop {
        work.tick()?;
        let candidate = encode(begin..boundaries[high].0, work)?;
        if candidate.primary_length >= needle.primary_length {
            break;
        }
        if !can_extend(&candidate, needle, work)? || high == final_end {
            return Ok(None);
        }
        low = high + 1;
        high = start + (high - start).saturating_mul(2).min(final_end - start);
    }
    while low < high {
        work.tick()?;
        let middle = low + (high - low) / 2;
        let candidate = encode(begin..boundaries[middle].0, work)?;
        if candidate.primary_length >= needle.primary_length {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    Ok(Some(low))
}

fn can_extend(
    candidate: &key::ComparisonKey,
    needle: &key::ComparisonKey,
    work: &mut key::Work<'_>,
) -> Result<bool, InspectError> {
    if candidate.primary_length > needle.primary_length {
        return Ok(false);
    }
    for (left, right) in candidate.bytes[..candidate.primary_length]
        .iter()
        .zip(&needle.bytes)
    {
        work.tick()?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}
