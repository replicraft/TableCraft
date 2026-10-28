//! Resolve the final tree into exact per-height links and existing Plan bytes.
use super::layout::{Contents, Layout, Origin};
use super::{Plan, leaf_entries, pages, patch, raw, source};
use crate::{
    EditError, InspectError, NativeImage,
    format::{page::PAGE_BYTES, reader::Reader},
    limits::MAX_INDEX_DEPTH,
    table::indexes::{
        header::{IndexHeader, LEVEL_OFFSET},
        mutation::{repack, retained::RetainedProfile},
        page::{Entry, KeyRef, NODE},
    },
};
use std::io::Cursor;

const KIND_OFFSET: usize = std::mem::offset_of!(IndexHeader, kind);
const PREVIOUS_OFFSET: usize = std::mem::offset_of!(IndexHeader, previous);
const NEXT_OFFSET: usize = std::mem::offset_of!(IndexHeader, next);
const TAIL_OFFSET: usize = std::mem::offset_of!(IndexHeader, tail);
const PREFIX_OFFSET: usize = std::mem::offset_of!(IndexHeader, prefix);

pub(super) struct Operation<'a, P: RetainedProfile> {
    pub image: &'a NativeImage,
    pub keys: &'a [KeyRef<P::Key>],
    pub profile: &'a P,
    pub layout: &'a Layout,
    pub numbers: &'a [u32],
}

pub(super) fn emit<P: RetainedProfile, const N: usize>(
    operation: Operation<'_, P>,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let layout = operation.layout;
    let mut levels: [Vec<usize>; MAX_INDEX_DEPTH] = std::array::from_fn(|_| Vec::new());
    let mut visited = Vec::new();
    source::reserve(&mut visited, layout.nodes.len(), reader)?;
    visited.resize(layout.nodes.len(), false);
    collect(layout, layout.root, &mut levels, &mut visited, reader)?;
    reader.work(visited.len())?;
    if visited.iter().any(|visited| !visited) {
        return Err(EditError::Preservation);
    }
    let mut links = Vec::new();
    source::reserve(&mut links, layout.nodes.len(), reader)?;
    links.resize(layout.nodes.len(), (0_u32, 0_u32));
    for level in &levels {
        for (position, &node) in level.iter().enumerate() {
            reader.tick()?;
            let previous = position
                .checked_sub(1)
                .map(|prior| operation.numbers[level[prior]])
                .unwrap_or(0);
            let next = level
                .get(position + 1)
                .map(|&next| operation.numbers[next])
                .unwrap_or(0);
            links[node] = (previous, next);
        }
    }
    // Source capture is root-first preorder, preserving the accepted H1
    // inventory while also covering every original internal page directly.
    for &node in &layout.original {
        reader.tick()?;
        let number = operation.numbers[node];
        let before = raw(operation.image, number)?;
        let after = render::<_, N>(&operation, node, links[node], plan.append.owner, reader)?;
        patch(number, before, after, plan, reader)?;
    }
    for &node in &layout.fresh {
        reader.tick()?;
        let bytes = render::<_, N>(&operation, node, links[node], plan.append.owner, reader)?;
        source::reserve(&mut plan.added, 1, reader)?;
        plan.added.push(super::super::super::AddedPage {
            number: operation.numbers[node],
            bytes,
        });
    }
    Ok(())
}

fn collect(
    layout: &Layout,
    index: usize,
    levels: &mut [Vec<usize>; MAX_INDEX_DEPTH],
    visited: &mut [bool],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    let seen = visited.get_mut(index).ok_or(EditError::Preservation)?;
    if *seen {
        return Err(EditError::Preservation);
    }
    *seen = true;
    let node = layout.nodes.get(index).ok_or(EditError::Preservation)?;
    let level = levels
        .get_mut(usize::from(node.height))
        .ok_or(InspectError::StructureLimit)?;
    source::reserve(level, 1, reader)?;
    level.push(index);
    if let Contents::Branch { children, .. } = &node.contents {
        for &child in children {
            collect(layout, child, levels, visited, reader)?;
        }
    }
    Ok(())
}

fn render<P: RetainedProfile, const N: usize>(
    operation: &Operation<'_, P>,
    index: usize,
    links: (u32, u32),
    owner: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u8>, EditError> {
    let node = &operation.layout.nodes[index];
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
            let count = children
                .len()
                .checked_sub(usize::from(*tail))
                .ok_or(EditError::Preservation)?;
            let mut entries = Vec::new();
            source::reserve(&mut entries, count, reader)?;
            for &child in &children[..count] {
                reader.tick()?;
                let key = operation
                    .keys
                    .get(operation.layout.nodes[child].maximum)
                    .ok_or(EditError::Preservation)?;
                entries.push(Entry {
                    value: KeyRef {
                        key: &key.key,
                        row: key.row,
                    },
                    child: operation.numbers[child],
                });
            }
            let tail = if *tail {
                operation.numbers[*children.last().ok_or(EditError::Preservation)?]
            } else {
                0
            };
            (entries, tail)
        }
    };
    let before = match node.origin {
        Origin::Fresh => {
            if node.height == 0 {
                return pages::leaf::<_, N>(
                    owner,
                    links.0,
                    links.1,
                    &entries,
                    operation.profile,
                    reader.budget,
                    reader.cancelled,
                );
            }
            return pages::node::<_, N>(
                &pages::NodeHeader {
                    owner,
                    height: node.height,
                    links,
                    tail,
                },
                &entries,
                operation.profile,
                reader.budget,
                reader.cancelled,
            );
        }
        Origin::Existing(number) | Origin::GrownRoot(number) => raw(operation.image, number)?,
    };
    let mut after = if matches!(node.origin, Origin::GrownRoot(_)) {
        let mut seed = Vec::new();
        source::reserve(&mut seed, PAGE_BYTES, reader)?;
        reader.work(PAGE_BYTES)?;
        seed.extend_from_slice(before);
        seed[PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()].fill(0);
        repack::borrowed_page::<_, N>(
            &seed,
            &entries,
            operation.profile,
            reader.budget,
            reader.cancelled,
        )?
    } else {
        repack::borrowed_page::<_, N>(
            before,
            &entries,
            operation.profile,
            reader.budget,
            reader.cancelled,
        )?
    };
    after[PREVIOUS_OFFSET..PREVIOUS_OFFSET + size_of::<u32>()]
        .copy_from_slice(&links.0.to_le_bytes());
    after[NEXT_OFFSET..NEXT_OFFSET + size_of::<u32>()].copy_from_slice(&links.1.to_le_bytes());
    after[TAIL_OFFSET..TAIL_OFFSET + size_of::<u32>()].copy_from_slice(&tail.to_le_bytes());
    after[LEVEL_OFFSET] = node.height;
    if matches!(node.origin, Origin::GrownRoot(_)) {
        after[KIND_OFFSET] = NODE;
    }
    Ok(after)
}
