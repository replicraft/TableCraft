//! Derived single-unit equivalence dictionary for bounded General LIKE classes.
use super::{GeneralSortFamily, NativeSortOrder, key};
use crate::{InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// Whether a complete raw UTF-16 span equals some single unit's General weights.
///
/// This is a provisional collation-element model for LIKE class/range matching,
/// not a native index lookup or a promise of native Access pattern behavior.
/// Full raw units, including isolated surrogates, retain their weight meaning.
/// Trailing spaces are retained for pattern literals. No index prefix/digest,
/// Unicode normalization or ordinal fallback supplies equivalence. Empty or
/// ignorable spans can equal a single ignorable unit; wildcard consumption rules
/// belong to the matcher and are not chosen by this dictionary.
///
/// The compiled derived dictionary is immutable and needs no file, OS locale,
/// runtime construction or input-dependent cache. Encoding, lookup traversal and
/// final publication use the caller's cumulative resource/cancellation controls.
pub fn is_general_like_unit(
    order: NativeSortOrder,
    units: &[u16],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, InspectError> {
    check(budget, cancelled)?;
    let family = order.general_family().ok_or(InspectError::Unsupported)?;
    let mut work = key::Work { budget, cancelled };
    let key = key::encode_full(family, units, &mut work)?;
    // contains traverses at most one edge per key byte; charge that complete
    // upper bound before entering the immutable FST reader.
    work.budget
        .charge(Resource::Steps, key.bytes.len() as u64)?;
    work.tick()?;
    let dictionary = dictionary(family)?;
    let result = dictionary.contains(&key.bytes);
    work.tick()?;
    Ok(result)
}

fn dictionary(family: GeneralSortFamily) -> Result<fst::Set<&'static [u8]>, InspectError> {
    let bytes: &[u8] = match family {
        GeneralSortFamily::Legacy => include_bytes!("../../data/general/like-legacy.fst"),
        GeneralSortFamily::General2010 => include_bytes!("../../data/general/like-2010.fst"),
    };
    fst::Set::new(bytes).map_err(|_| InspectError::Malformed)
}

#[cfg(test)]
mod tests;
