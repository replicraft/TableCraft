//! Derive final level links, preserve old residue and initialize fresh pages.
use super::super::super::{IndexDataPatch, encoding, repack};
use super::super::{AddedPage, Plan, leaf_entries, pages};
use super::Operation;
use super::arena::{self, Arena, Contents, Node};
use crate::{
    EditError, InspectError,
    format::{page::PAGE_BYTES, reader::Reader},
    limits::MAX_INDEX_DEPTH,
    table::indexes::header::{IndexHeader, LEVEL_OFFSET},
};
use std::io::Cursor;

const PREVIOUS_OFFSET: usize = std::mem::offset_of!(IndexHeader, previous);
const NEXT_OFFSET: usize = std::mem::offset_of!(IndexHeader, next);
const TAIL_OFFSET: usize = std::mem::offset_of!(IndexHeader, tail);
const PREFIX_OFFSET: usize = std::mem::offset_of!(IndexHeader, prefix);

pub(super) fn emit(
    operation: &Operation<'_>,
    arena: &mut Arena,
    plan: &mut Plan,
    patches: &mut Vec<IndexDataPatch>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let mut levels: [Vec<usize>; MAX_INDEX_DEPTH] = std::array::from_fn(|_| Vec::new());
    collect(arena.root, arena, &mut levels, reader)?;
    for level in &levels {
        for (position, &index) in level.iter().enumerate() {
            reader.tick()?;
            let previous = position
                .checked_sub(1)
                .map(|prior| arena.nodes[level[prior]].number)
                .unwrap_or(0);
            let next = level
                .get(position + 1)
                .map(|&next| arena.nodes[next].number)
                .unwrap_or(0);
            arena.nodes[index].previous = previous;
            arena.nodes[index].next = next;
        }
    }
    let original_eof = operation.image.catalog().header().page_count();
    for node in arena.nodes.iter().filter(|node| node.number < original_eof) {
        let before = arena::raw(operation.image, node.number)?;
        let after = serialize(operation, arena, node, plan.owner, reader)?;
        reader.work(PAGE_BYTES)?;
        if before == after {
            continue;
        }
        reader
            .budget
            .charge_allocation(PAGE_BYTES as u64, 1)
            .map_err(InspectError::from)?;
        let mut original = Vec::new();
        original
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        original.extend_from_slice(before);
        reader
            .budget
            .charge_allocation(1, size_of::<IndexDataPatch>() as u64)
            .map_err(InspectError::from)?;
        patches
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        patches.push(IndexDataPatch {
            offset: node.number as usize * PAGE_BYTES,
            before: original,
            after,
        });
    }
    for &number in &arena.added {
        reader.tick()?;
        reader.work(arena.nodes.len())?;
        let node = arena
            .nodes
            .iter()
            .find(|node| node.number == number)
            .ok_or(EditError::Preservation)?;
        let bytes = serialize(operation, arena, node, plan.owner, reader)?;
        reader
            .budget
            .charge_allocation(1, size_of::<AddedPage>() as u64)
            .map_err(InspectError::from)?;
        plan.added
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        plan.added.push(AddedPage { number, bytes });
    }
    Ok(())
}

fn collect(
    index: usize,
    arena: &Arena,
    levels: &mut [Vec<usize>; MAX_INDEX_DEPTH],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    let node = arena.nodes.get(index).ok_or(EditError::Preservation)?;
    let level = levels
        .get_mut(usize::from(node.height))
        .ok_or(InspectError::StructureLimit)?;
    reader
        .budget
        .charge_allocation(1, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    level.try_reserve(1).map_err(|_| InspectError::Allocation)?;
    level.push(index);
    if let Contents::Branch { children, .. } = &node.contents {
        for &child in children {
            collect(child, arena, levels, reader)?;
        }
    }
    Ok(())
}

fn serialize(
    operation: &Operation<'_>,
    arena: &Arena,
    node: &Node,
    owner: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u8>, EditError> {
    reader.tick()?;
    let fresh = node.number >= operation.image.catalog().header().page_count();
    let (entries, tail) = match &node.contents {
        Contents::Leaf(range) => (
            leaf_entries(
                &operation.keys[range.clone()],
                reader.budget,
                reader.cancelled,
            )?,
            0,
        ),
        Contents::Branch { children, tail } => {
            let entries = arena::bounds(&arena.nodes, children, *tail, reader)?;
            let tail = if *tail {
                let &last = children.last().ok_or(EditError::Preservation)?;
                arena.nodes.get(last).ok_or(EditError::Preservation)?.number
            } else {
                0
            };
            (entries, tail)
        }
    };
    if fresh {
        if node.height == 0 {
            return pages::leaf(
                owner,
                node.previous,
                node.next,
                &entries,
                operation.profile,
                reader.budget,
                reader.cancelled,
            );
        }
        return pages::node(
            &pages::NodeHeader {
                owner,
                height: node.height,
                links: (node.previous, node.next),
                tail,
            },
            &entries,
            operation.profile,
            reader.budget,
            reader.cancelled,
        );
    }
    let original = arena::raw(operation.image, node.number)?;
    let mut after = if node.grown_root {
        reader
            .budget
            .charge_allocation(PAGE_BYTES as u64, 1)
            .map_err(InspectError::from)?;
        reader.work(PAGE_BYTES)?;
        let mut seed = Vec::new();
        seed.try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        seed.extend_from_slice(original);
        seed[PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()].fill(0);
        repack::page::<_, { encoding::MAX_ENTRY_BYTES }>(
            &seed,
            &entries,
            operation.profile,
            reader.budget,
            reader.cancelled,
        )?
    } else {
        repack::page::<_, { encoding::MAX_ENTRY_BYTES }>(
            original,
            &entries,
            operation.profile,
            reader.budget,
            reader.cancelled,
        )?
    };
    after[PREVIOUS_OFFSET..PREVIOUS_OFFSET + size_of::<u32>()]
        .copy_from_slice(&node.previous.to_le_bytes());
    after[NEXT_OFFSET..NEXT_OFFSET + size_of::<u32>()].copy_from_slice(&node.next.to_le_bytes());
    after[TAIL_OFFSET..TAIL_OFFSET + size_of::<u32>()].copy_from_slice(&tail.to_le_bytes());
    after[LEVEL_OFFSET] = node.height;
    Ok(after)
}
