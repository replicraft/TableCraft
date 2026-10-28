//! Independent symbolic passes, separate from logical sibling order and IDs.
use super::super::WIRE_BYTES;
use super::{frames, reserve, source::Source};
use crate::{EditError, format::reader::Reader};
use std::{io::Cursor, ops::Range};

#[derive(Clone, Copy)]
pub(super) enum Origin {
    Existing { number: u32, prefix: usize },
    Fresh,
}

pub(super) struct Node {
    pub origin: Origin,
    pub range: Range<usize>,
}

pub(super) struct Normalized {
    pub nodes: Vec<Node>,
    pub logical: Vec<usize>,
    pub fresh: Vec<usize>,
}

pub(super) fn prepare(
    source: &Source,
    entries: &[[u8; WIRE_BYTES]],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Normalized, EditError> {
    let mut result = Normalized {
        nodes: Vec::new(),
        logical: Vec::new(),
        fresh: Vec::new(),
    };
    let mut start = 0;
    for (position, child) in source.children.iter().enumerate() {
        reader.tick()?;
        let mut end = entries.len();
        if position + 1 < source.children.len() {
            let maximum = source
                .entries
                .get(child.range.end - 1)
                .ok_or(EditError::Preservation)?;
            end = start;
            while end < entries.len() && &entries[end] <= maximum {
                reader.tick()?;
                end += 1;
            }
        }
        if start >= end {
            return Err(EditError::Preservation);
        }
        let index = result.nodes.len();
        reserve(&mut result.nodes, 1, reader)?;
        result.nodes.push(Node {
            origin: Origin::Existing {
                number: child.number,
                prefix: child.prefix,
            },
            range: start..end,
        });
        start = end;

        if fits(&result.nodes[index], entries, reader)? {
            push(&mut result.logical, index, reader)?;
            continue;
        }
        // Match divide-before-normalize: revisit original RIGHT before LEFT.
        let left = divide(&mut result, index, reader)?;
        let mut work = Vec::new();
        push(&mut work, index, reader)?;
        push(&mut work, left, reader)?;
        let mut logical = Vec::new();
        push(&mut logical, left, reader)?;
        push(&mut logical, index, reader)?;
        loop {
            reader.tick()?;
            let mut changed = false;
            let mut visit = 0;
            while visit < work.len() {
                let index = work[visit];
                visit += 1;
                if fits(&result.nodes[index], entries, reader)? {
                    continue;
                }
                let left = divide(&mut result, index, reader)?;
                push(&mut work, left, reader)?;
                reader.work(logical.len())?;
                let position = logical
                    .iter()
                    .position(|node| *node == index)
                    .ok_or(EditError::Preservation)?;
                reserve(&mut logical, 1, reader)?;
                logical.insert(position, left);
                changed = true;
            }
            if !changed {
                break;
            }
        }
        reserve(&mut result.logical, logical.len(), reader)?;
        result.logical.extend(logical);
    }
    if start != entries.len()
        || result.nodes.len() != source.children.len() + result.fresh.len()
        || result.logical.len() != result.nodes.len()
    {
        return Err(EditError::Preservation);
    }
    let mut next = 0;
    for index in &result.logical {
        reader.tick()?;
        let node = result.nodes.get(*index).ok_or(EditError::Preservation)?;
        if node.range.start != next || node.range.start >= node.range.end {
            return Err(EditError::Preservation);
        }
        next = node.range.end;
    }
    if next != entries.len() {
        return Err(EditError::Preservation);
    }

    Ok(result)
}

fn fits(
    node: &Node,
    entries: &[[u8; WIRE_BYTES]],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<bool, EditError> {
    reader.tick()?;
    let prefix = match node.origin {
        Origin::Existing { prefix, .. } => prefix,
        Origin::Fresh => 0,
    };
    let entries = entries
        .get(node.range.clone())
        .ok_or(EditError::Preservation)?;
    Ok(frames::measure(entries, prefix, reader)?.fits())
}

fn divide(
    normalized: &mut Normalized,
    index: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    reader.tick()?;
    let range = normalized
        .nodes
        .get(index)
        .ok_or(EditError::Preservation)?
        .range
        .clone();
    let count = range
        .end
        .checked_sub(range.start)
        .filter(|count| *count >= 2)
        .ok_or(EditError::Preservation)?;
    let middle = range.start + count / 2;
    let left = normalized.nodes.len();
    reserve(&mut normalized.nodes, 1, reader)?;
    normalized.nodes.push(Node {
        origin: Origin::Fresh,
        range: range.start..middle,
    });
    normalized.nodes[index].range = middle..range.end;
    push(&mut normalized.fresh, left, reader)?;
    Ok(left)
}

fn push(
    values: &mut Vec<usize>,
    value: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reserve(values, 1, reader)?;
    values.push(value);
    Ok(())
}
