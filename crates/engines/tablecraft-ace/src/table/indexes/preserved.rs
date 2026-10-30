//! Read-only single Int32-shaped membership for preserved ordinary/complex keys.
//! The caller separately certifies native field/complex-reference declarations.
use super::{
    Int64IndexSelection, NativeIndex, key::SingleKey, page::MAX_ENTRY_BYTES, profile::KeyType,
    walk::Walker,
};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{allocation, reader::Reader},
    header::check,
};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table) fn certify(
    image: &NativeImage,
    definition: u32,
    index: &NativeIndex,
    expected: &[(u32, Option<i32>)],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u32>, EditError> {
    check(budget, cancelled)?;
    let [column] = index.columns() else {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    };
    let constraints = index
        .constraints()
        .filter(|constraints| !constraints.ignore_nulls())
        .ok_or(EditError::Unsupported(WriteBlocker::ConstraintCoverage))?;
    let profile = SingleKey {
        column: column.number(),
        key_type: KeyType::Int32,
        descending: column.descending(),
        selection: Int64IndexSelection::All,
    };
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let owned = allocation::pages(&mut reader, index.usage_map())?;
    for pair in owned.windows(2) {
        reader.tick()?;
        if pair[0] >= pair[1] {
            return Err(InspectError::Malformed.into());
        }
    }
    // Walker checks its growing visited list. Prepay the complete worst-case
    // membership comparisons before traversal instead of hiding that byte work.
    let membership = owned
        .len()
        .checked_mul(owned.len())
        .ok_or(InspectError::StructureLimit)?;
    reader.work(membership)?;
    let mut walker = Walker::<_, _, MAX_ENTRY_BYTES> {
        reader: &mut reader,
        owner: definition,
        owned: &owned,
        profile: &profile,
        visited: Vec::new(),
        entries: Vec::new(),
        maximum_rows: expected.len(),
        previous_leaf: 0,
        next_leaf: 0,
        gap: false,
    };
    walker.visit(index.root_page(), 0, None, None)?;
    if walker.next_leaf != 0 || walker.gap || walker.entries.len() != expected.len() {
        return Err(InspectError::Malformed.into());
    }
    let entries = walker.entries;
    let mut visited = walker.visited;
    let levels = visited.len().checked_ilog2().unwrap_or(0) as usize + 1;
    reader.work(
        visited
            .len()
            .checked_mul(levels)
            .ok_or(InspectError::StructureLimit)?,
    )?;
    visited.sort_unstable();
    if visited != owned {
        return Err(EditError::Preservation);
    }

    reader
        .budget
        .charge_allocation(expected.len() as u64, size_of::<bool>() as u64)
        .map_err(InspectError::from)?;
    let mut seen = Vec::new();
    seen.try_reserve_exact(expected.len())
        .map_err(|_| InspectError::Allocation)?;
    seen.resize(expected.len(), false);
    let mut previous = None;
    for entry in entries {
        reader.tick()?;
        reader.work(expected.len())?;
        let position = expected
            .iter()
            .position(|(reference, _)| *reference == entry.row)
            .ok_or(InspectError::Malformed)?;
        if seen[position] || entry.key != expected[position].1.map(i64::from) {
            return Err(InspectError::Malformed.into());
        }
        constraints
            .check_tuple(&mut previous, entry.key, entry.key.is_none())
            .map_err(InspectError::from)?;
        seen[position] = true;
    }
    reader.tick()?;
    Ok(owned)
}
