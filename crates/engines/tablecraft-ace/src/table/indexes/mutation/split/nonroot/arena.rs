//! Postorder decisions use original leaf counts and complete final child maxima.
mod normalize;
use super::super::super::{encoding, repack, tree::Tree};
use super::super::{Plan, leaf_entries};
use super::Operation;
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    limits::MAX_INDEX_DEPTH,
    table::indexes::{
        header::IndexHeader,
        page::{Entry, KeyRef},
    },
};
use std::{io::Cursor, ops::Range};

pub(super) enum Contents {
    Leaf(Range<usize>),
    Branch { children: Vec<usize>, tail: bool },
}

pub(super) struct Node {
    pub number: u32,
    pub height: u8,
    pub maximum: KeyRef,
    pub contents: Contents,
    pub grown_root: bool,
    pub previous: u32,
    pub next: u32,
}

pub(super) struct Arena {
    pub nodes: Vec<Node>,
    pub root: usize,
    pub added: Vec<u32>,
    position: usize,
}

pub(super) fn prepare(
    operation: &Operation<'_>,
    tree: &Tree,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Arena, EditError> {
    let mut arena = Arena {
        nodes: Vec::new(),
        root: 0,
        added: Vec::new(),
        position: 0,
    };
    let result = build(operation, tree, true, &mut arena, plan, reader)?;
    if result.len() != 1 || arena.position != operation.keys.len() {
        return Err(EditError::Preservation);
    }
    arena.root = result[0];
    Ok(arena)
}

fn build(
    operation: &Operation<'_>,
    tree: &Tree,
    root: bool,
    arena: &mut Arena,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<usize>, EditError> {
    reader.tick()?;
    let original = raw(operation.image, tree.number())?;
    let (contents, maximum) = if tree.is_leaf() {
        let start = arena.position;
        let end = start
            .checked_add(tree.count())
            .filter(|end| start < *end && *end <= operation.keys.len())
            .ok_or(EditError::Preservation)?;
        arena.position = end;
        (Contents::Leaf(start..end), operation.keys[end - 1])
    } else {
        let mut children = Vec::new();
        for child in tree.children() {
            for result in build(operation, child, false, arena, plan, reader)? {
                push_child(&mut children, result, reader)?;
            }
        }
        let last = *children.last().ok_or(EditError::Preservation)?;
        let maximum = arena.nodes[last].maximum;
        let tail = IndexHeader::read(original)?.tail.get() != 0;
        (Contents::Branch { children, tail }, maximum)
    };
    let index = push(
        arena,
        Node {
            number: tree.number(),
            height: tree.height(),
            maximum,
            contents,
            grown_root: false,
            previous: 0,
            next: 0,
        },
        reader,
    )?;
    if normalize::fits(operation, arena, index, reader)? {
        let mut result = Vec::new();
        push_child(&mut result, index, reader)?;
        return Ok(result);
    }
    if !root {
        let left = normalize::divide(operation, arena, plan, index, reader)?;
        return normalize::run(operation, arena, plan, (index, left), reader);
    }

    let height = tree
        .height()
        .checked_add(1)
        .filter(|height| usize::from(*height) < MAX_INDEX_DEPTH)
        .ok_or(InspectError::StructureLimit)?;
    // The original root is replaced only after its lower siblings normalize.
    // Its nested right page is allocated before the first new left sibling.
    arena.nodes[index].number = allocate(operation, arena, plan, reader)?;
    if let Contents::Branch { tail, .. } = &mut arena.nodes[index].contents {
        *tail = true;
    }
    let left = normalize::divide(operation, arena, plan, index, reader)?;
    let children = normalize::run(operation, arena, plan, (index, left), reader)?;
    let root = push_branch(
        arena,
        tree.number(),
        height,
        children,
        tree.is_leaf(),
        true,
        reader,
    )?;
    if !normalize::fits(operation, arena, root, reader)? {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let mut result = Vec::new();
    push_child(&mut result, root, reader)?;
    Ok(result)
}

fn allocate(
    operation: &Operation<'_>,
    arena: &mut Arena,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<u32, EditError> {
    let number = super::super::allocation::reserve(operation.image, operation.index, plan, reader)?;
    reader
        .budget
        .charge_allocation(1, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    arena
        .added
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    arena.added.push(number);
    Ok(number)
}

fn push_leaf(
    arena: &mut Arena,
    number: u32,
    range: Range<usize>,
    operation: &Operation<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    if range.start >= range.end {
        return Err(EditError::Preservation);
    }
    let maximum = *operation
        .keys
        .get(range.end.checked_sub(1).ok_or(EditError::Preservation)?)
        .ok_or(EditError::Preservation)?;
    push(
        arena,
        Node {
            number,
            height: 0,
            maximum,
            contents: Contents::Leaf(range),
            grown_root: false,
            previous: 0,
            next: 0,
        },
        reader,
    )
}

fn push_branch(
    arena: &mut Arena,
    number: u32,
    height: u8,
    children: Vec<usize>,
    tail: bool,
    grown_root: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    let last = *children.last().ok_or(EditError::Preservation)?;
    let maximum = arena
        .nodes
        .get(last)
        .ok_or(EditError::Preservation)?
        .maximum;
    push(
        arena,
        Node {
            number,
            height,
            maximum,
            contents: Contents::Branch { children, tail },
            grown_root,
            previous: 0,
            next: 0,
        },
        reader,
    )
}

fn push(
    arena: &mut Arena,
    node: Node,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    reader
        .budget
        .charge_allocation(1, size_of::<Node>() as u64)
        .map_err(InspectError::from)?;
    arena
        .nodes
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    arena.nodes.push(node);
    Ok(arena.nodes.len() - 1)
}

fn push_child(
    children: &mut Vec<usize>,
    child: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader
        .budget
        .charge_allocation(1, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    children
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    children.push(child);
    Ok(())
}

fn copy_children(
    children: &[usize],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<usize>, EditError> {
    reader.work(children.len())?;
    reader
        .budget
        .charge_allocation(children.len() as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    let mut copy = Vec::new();
    copy.try_reserve_exact(children.len())
        .map_err(|_| InspectError::Allocation)?;
    copy.extend_from_slice(children);
    Ok(copy)
}

pub(super) fn bounds(
    nodes: &[Node],
    children: &[usize],
    tail: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<Entry>, EditError> {
    let count = children
        .len()
        .checked_sub(usize::from(tail))
        .ok_or(EditError::Preservation)?;
    reader
        .budget
        .charge_allocation(count as u64, size_of::<Entry>() as u64)
        .map_err(InspectError::from)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for &child in &children[..count] {
        reader.tick()?;
        let node = nodes.get(child).ok_or(EditError::Preservation)?;
        entries.push(Entry {
            value: node.maximum,
            child: node.number,
        });
    }
    Ok(entries)
}

pub(super) fn raw(image: &NativeImage, number: u32) -> Result<&[u8], EditError> {
    let start = number as usize * PAGE_BYTES;
    image
        .as_bytes()
        .get(start..start + PAGE_BYTES)
        .ok_or_else(|| InspectError::Malformed.into())
}
