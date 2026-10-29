//! Stable allocation-order passes; logical sibling order is tracked separately.
use super::super::super::super::retained::RetainedProfile;
use super::{
    Arena, Contents, Operation, Plan, allocate, bounds, copy_children, leaf_entries, push_branch,
    push_child, push_leaf, raw, repack,
};
use crate::table::mutation::append::owners::OwnerContext;
use crate::{EditError, InspectError, WriteBlocker, format::reader::Reader};
use std::io::Cursor;

pub(super) fn run<P: RetainedProfile, const N: usize>(
    operation: &Operation<'_, P, N>,
    arena: &mut Arena,
    plan: &mut Plan<impl OwnerContext>,
    pair: (usize, usize),
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<usize>, EditError> {
    let (right, left) = pair;
    let mut work = Vec::new();
    push_child(&mut work, right, reader)?;
    push_child(&mut work, left, reader)?;
    let mut ordered = Vec::new();
    push_child(&mut ordered, left, reader)?;
    push_child(&mut ordered, right, reader)?;
    loop {
        reader.tick()?;
        let mut changed = false;
        let mut position = 0;
        // A page is visited once per pass. New left pages join the end and
        // may be visited in this pass; a retained right waits for the next pass.
        while position < work.len() {
            let index = work[position];
            position += 1;
            if fits(operation, arena, index, reader)? {
                continue;
            }
            let left = divide(operation, arena, plan, index, reader)?;
            push_child(&mut work, left, reader)?;
            reader.work(ordered.len())?;
            let before = ordered
                .iter()
                .position(|&child| child == index)
                .ok_or(EditError::Preservation)?;
            reader
                .budget
                .charge_allocation(1, size_of::<usize>() as u64)
                .map_err(InspectError::from)?;
            ordered
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            ordered.insert(before, left);
            changed = true;
        }
        if !changed {
            return Ok(ordered);
        }
    }
}

pub(super) fn fits<P: RetainedProfile, const N: usize>(
    operation: &Operation<'_, P, N>,
    arena: &Arena,
    index: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<bool, EditError> {
    reader.tick()?;
    let node = arena.nodes.get(index).ok_or(EditError::Preservation)?;
    let entries = match &node.contents {
        Contents::Leaf(range) => leaf_entries(
            operation
                .keys
                .get(range.clone())
                .ok_or(EditError::Preservation)?,
            reader.budget,
            reader.cancelled,
        )?,
        Contents::Branch { children, tail } => {
            bounds(operation.keys, &arena.nodes, children, *tail, reader)?
        }
    };
    if node.number >= operation.image.catalog().header().page_count() || node.grown_root {
        repack::borrowed_fits_fresh::<P, N>(
            &entries,
            operation.profile,
            reader.budget,
            reader.cancelled,
        )
    } else {
        repack::borrowed_fits::<P, N>(
            raw(operation.image, node.number)?,
            &entries,
            operation.profile,
            reader.budget,
            reader.cancelled,
        )
    }
}

pub(super) fn divide<P: RetainedProfile, const N: usize>(
    operation: &Operation<'_, P, N>,
    arena: &mut Arena,
    plan: &mut Plan<impl OwnerContext>,
    index: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    reader.tick()?;
    let node = arena.nodes.get(index).ok_or(EditError::Preservation)?;
    match &node.contents {
        Contents::Leaf(range) => {
            let count = range
                .end
                .checked_sub(range.start)
                .ok_or(EditError::Preservation)?;
            if count < 2 {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            let middle = range.start + count / 2;
            let left = range.start..middle;
            let right = middle..range.end;
            let number = allocate(operation, arena, plan, reader)?;
            let left = push_leaf(arena, number, left, operation, reader)?;
            arena.nodes[index].contents = Contents::Leaf(right);
            Ok(left)
        }
        Contents::Branch { children, tail } => {
            if children.len() < 2 {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            let middle = children.len() / 2;
            let tail = *tail;
            let height = node.height;
            let left = copy_children(&children[..middle], reader)?;
            let right = copy_children(&children[middle..], reader)?;
            let number = allocate(operation, arena, plan, reader)?;
            let left = push_branch(arena, number, height, left, true, false, reader)?;
            arena.nodes[index].contents = Contents::Branch {
                children: right,
                tail,
            };
            Ok(left)
        }
    }
}
