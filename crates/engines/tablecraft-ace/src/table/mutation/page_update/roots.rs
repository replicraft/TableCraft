//! Checked logical-root ownership of physical final records.
use super::super::{
    EditError, WriteBlocker,
    rows::{Directory, RowAssignment},
};
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{
        allocation,
        bytes::u32_at,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{PhysicalRow, Reader, live_row_range, row_count},
        row::reference,
    },
    header::check,
    limits::MAX_LIVE_ROWS,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table::mutation) struct Root<'u, U> {
    pub reference: u32,
    pub word: u16,
    pub range: Range<usize>,
    pub final_record: PhysicalRow,
    pub update: &'u U,
}

#[derive(Clone, Copy)]
struct TargetOwner {
    offset: usize,
    root: u32,
}

pub(in crate::table::mutation) struct Roots<'u, U> {
    pub selected: Vec<Root<'u, U>>,
    pub owned: Vec<u32>,
}

pub(in crate::table::mutation) fn capture<'u, U: RowAssignment>(
    image: &NativeImage,
    schema: &TableSchema,
    updates: &'u [U],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Roots<'u, U>, EditError> {
    let mut directory = Directory::new(updates, budget, cancelled)?;
    directory.require_unique(budget, cancelled)?;
    let declared = usize::try_from(schema.declared_rows())
        .ok()
        .filter(|count| *count <= MAX_LIVE_ROWS)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(declared as u64, size_of::<TargetOwner>() as u64)
        .map_err(InspectError::from)?;
    budget
        .charge_allocation(updates.len() as u64, size_of::<Root<'_, U>>() as u64)
        .map_err(InspectError::from)?;
    let mut owners = Vec::new();
    owners
        .try_reserve_exact(declared)
        .map_err(|_| InspectError::Allocation)?;
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let owned = allocation::pages(&mut reader, schema.layout.usage)?;
    // Usage-map traversal is monotone. Establish that fact before using binary
    // lookup as a paid ownership proof for every followed final target.
    for pair in owned.windows(2) {
        reader.tick()?;
        if pair[0] >= pair[1] {
            return Err(InspectError::Malformed.into());
        }
    }
    let owned_levels = owned.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let mut ordinal = 0;
    for &number in &owned {
        let page = reader.page(number)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&page, OWNER_OFFSET)? != schema.definition
        {
            return Err(InspectError::Malformed.into());
        }
        for slot in 0..row_count(&page)? {
            reader.tick()?;
            let Some((word, start, end)) = live_row_range(&page, slot)? else {
                continue;
            };
            if ordinal >= declared {
                return Err(InspectError::Malformed.into());
            }
            reader
                .budget
                .charge(Resource::Rows, 1)
                .map_err(InspectError::from)?;
            let slot = u8::try_from(slot).map_err(|_| InspectError::Malformed)?;
            let root = reference(number, slot);
            // The existing reader owns pointer decoding, owner validation,
            // cycle detection and hop limits. Do not duplicate that grammar.
            let final_record = reader.row_location(number, slot, schema.definition, &page)?;
            let final_page = u32::try_from(final_record.offset / PAGE_BYTES)
                .map_err(|_| InspectError::Malformed)?;
            reader.work(owned_levels)?;
            if owned.binary_search(&final_page).is_err() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            owners.push(TargetOwner {
                offset: final_record.offset,
                root,
            });
            if let Some(request) = directory.at(ordinal, reader.budget, reader.cancelled)? {
                selected.push(Root {
                    reference: root,
                    word,
                    range: number as usize * PAGE_BYTES + start..number as usize * PAGE_BYTES + end,
                    final_record,
                    update: &updates[request],
                });
            }
            ordinal += 1;
        }
    }
    if ordinal != declared || selected.len() != updates.len() {
        return Err(EditError::Conflict);
    }
    let levels = owners.len().checked_ilog2().unwrap_or(0) as usize + 1;
    reader.work(owners.len() * levels * size_of::<TargetOwner>())?;
    owners.sort_unstable_by_key(|owner| (owner.offset, owner.root));
    for root in &selected {
        reader.work(2 * levels)?;
        let first = owners.partition_point(|owner| owner.offset < root.final_record.offset);
        let end = owners.partition_point(|owner| owner.offset <= root.final_record.offset);
        if end - first != 1 || owners[first].root != root.reference {
            // A stable physical slot is not exclusive logical ownership. This
            // includes forwarded roots which alias an ordinary visible target.
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }
    check(reader.budget, cancelled)?;
    Ok(Roots { selected, owned })
}
