//! Existing root-leaf removal shared by engineering and qualified normal actions.
use super::super::{
    header::IndexHeader,
    page::{Entry, Page},
};
use crate::{EditError, InspectError, WriteBlocker, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

#[cfg(test)]
mod tests;

pub(super) fn require_root<K>(root: &Page<K>, original: &[u8]) -> Result<(), EditError> {
    if !root.leaf
        || root.previous != 0
        || root.next != 0
        || root.tail != 0
        || !IndexHeader::read(original)?.qualified_for_update(0)
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    Ok(())
}

/// Stable removal preserves original retained key order and complete membership.
pub(super) fn remove<K>(
    entries: &mut Vec<Entry<K>>,
    references: &[u32],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut removed = 0;
    let mut retained = 0;
    for position in 0..entries.len() {
        check(budget, cancelled)?;
        // Preserve the existing Reader::tick followed by Reader::work polls.
        // The second check precedes the references charge, including work(0).
        if cancelled() {
            return Err(InspectError::Cancelled.into());
        }
        budget
            .charge(Resource::Steps, references.len() as u64)
            .map_err(InspectError::from)?;
        if references.contains(&entries[position].value.row) {
            removed += 1;
        } else {
            entries.swap(retained, position);
            retained += 1;
        }
    }
    if removed != references.len() {
        return Err(EditError::Preservation);
    }
    entries.truncate(retained);
    Ok(())
}
