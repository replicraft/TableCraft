//! Root-inclusive exclusive retirement; existing row-retirement languages stay closed.
use crate::{
    EditError, InspectError, NativeImage,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    table::mutation::{
        index_creation::buffer,
        map_changes::{self, ByteEdit},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn cells(
    before: &NativeImage,
    pages: &[u32],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<ByteEdit>, EditError> {
    let mut input = before.reader();
    let mut reader = Reader {
        source: &mut input,
        header: before.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    global.require_allocated_storage(&mut reader, &global)?;
    let mut edits = buffer::vector(pages.len(), reader.budget, reader.cancelled)?;
    for &page in pages {
        reader.tick()?;
        if page < 2 {
            return Err(EditError::Preservation);
        }
        map_changes::merge(
            &mut edits,
            global.cell(&mut reader, page)?,
            true,
            &mut reader,
        )?;
    }
    Ok(edits)
}

pub(super) fn apply(
    before: &NativeImage,
    pages: &[u32],
    after: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let edits = cells(before, pages, budget, cancelled)?;
    for &page in pages {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        let start = page as usize * PAGE_BYTES;
        after
            .get_mut(start..start + PAGE_BYTES)
            .ok_or(EditError::Preservation)?
            .fill(0);
    }
    for edit in edits {
        check(budget, cancelled)?;
        let actual = after.get_mut(edit.offset).ok_or(EditError::Preservation)?;
        if *actual != edit.before || edit.clear != 0 {
            return Err(EditError::Preservation);
        }
        *actual |= edit.set;
    }
    Ok(())
}

/// In-place page ordering pays and checks cancellation at every heap boundary.
pub(super) fn sort_pages(
    pages: &mut [u32],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    for start in (0..pages.len() / 2).rev() {
        sift(pages, start, pages.len(), budget, cancelled)?;
    }
    for end in (1..pages.len()).rev() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, 1)?;
        pages.swap(0, end);
        sift(pages, 0, end, budget, cancelled)?;
    }
    Ok(())
}
fn sift(
    pages: &mut [u32],
    mut root: usize,
    end: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    while root < end / 2 {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, 3)?;
        let mut child = root
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or(InspectError::StructureLimit)?;
        if child + 1 < end && pages[child] < pages[child + 1] {
            child += 1;
        }
        if pages[root] >= pages[child] {
            break;
        }
        pages.swap(root, child);
        root = child;
    }
    Ok(())
}
