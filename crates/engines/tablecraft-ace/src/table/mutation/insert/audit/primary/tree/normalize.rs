//! Request-routed subtrees and audit-owned symbolic postorder normalization.
use super::super::Wire;
use super::{
    frames, reserve,
    source::{Branch, Source},
};
use crate::{EditError, format::reader::Reader, limits::MAX_INDEX_DEPTH};
use std::{io::Cursor, ops::Range};

#[derive(Clone, Copy)]
pub(super) enum Origin {
    Existing { number: u32, prefix: usize },
    Fresh,
    GrownRoot { number: u32 },
}

pub(super) enum Payload {
    Leaf(Range<usize>),
    Branch { children: Vec<usize>, tail: bool },
}

pub(super) struct Node {
    pub origin: Origin,
    pub height: u8,
    pub maximum: Option<usize>,
    pub payload: Payload,
}

pub(super) struct Normalized {
    pub nodes: Vec<Node>,
    pub fresh: Vec<usize>,
    pub root: usize,
}

pub(super) fn prepare(
    source: &Source,
    entries: &[Wire],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Normalized, EditError> {
    let mut result = Normalized {
        nodes: Vec::new(),
        fresh: Vec::new(),
        root: 0,
    };
    let forest = routed(
        &source.root,
        0..entries.len(),
        source,
        entries,
        &mut result,
        true,
        reader,
    )?;
    if forest.len() != 1 {
        return Err(EditError::Preservation);
    }
    result.root = forest[0];
    Ok(result)
}

fn routed(
    original: &Branch,
    range: Range<usize>,
    source: &Source,
    entries: &[Wire],
    result: &mut Normalized,
    root: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<usize>, EditError> {
    reader.tick()?;
    let payload = if original.height == 0 {
        Payload::Leaf(range.clone())
    } else {
        let mut children = Vec::new();
        let mut start = range.start;
        for (position, child) in original.children.iter().enumerate() {
            let mut end = range.end;
            if position + 1 < original.children.len() {
                let bound = source
                    .entries
                    .get(child.range.end - 1)
                    .ok_or(EditError::Preservation)?;
                end = start;
                while end < range.end && &entries[end] <= bound {
                    reader.tick()?;
                    end += 1;
                }
            }
            if start >= end {
                return Err(EditError::Preservation);
            }
            let normalized = routed(child, start..end, source, entries, result, false, reader)?;
            reserve(&mut children, normalized.len(), reader)?;
            children.extend(normalized);
            start = end;
        }
        if start != range.end {
            return Err(EditError::Preservation);
        }
        Payload::Branch {
            children,
            tail: original.tail,
        }
    };
    let index = result.nodes.len();
    reserve(&mut result.nodes, 1, reader)?;
    result.nodes.push(Node {
        origin: Origin::Existing {
            number: original.number,
            prefix: original.prefix,
        },
        height: original.height,
        maximum: range.end.checked_sub(1).filter(|_| !range.is_empty()),
        payload,
    });
    if fits(&result.nodes[index], result, entries, reader)? {
        return singleton(index, reader);
    }
    if !root {
        return normalize(index, result, entries, reader);
    }

    let height = original
        .height
        .checked_add(1)
        .ok_or(EditError::Preservation)?;
    if usize::from(height) >= MAX_INDEX_DEPTH {
        return Err(EditError::Preservation);
    }
    result.nodes[index].origin = Origin::Fresh;
    if let Payload::Branch { tail, .. } = &mut result.nodes[index].payload {
        *tail = true;
    }
    push(&mut result.fresh, index, reader)?;
    let children = normalize(index, result, entries, reader)?;
    let grown = result.nodes.len();
    reserve(&mut result.nodes, 1, reader)?;
    result.nodes.push(Node {
        origin: Origin::GrownRoot {
            number: original.number,
        },
        height,
        maximum: range.end.checked_sub(1),
        payload: Payload::Branch {
            children,
            tail: original.height == 0,
        },
    });
    if !fits(&result.nodes[grown], result, entries, reader)? {
        return Err(EditError::Preservation);
    }
    singleton(grown, reader)
}

fn normalize(
    index: usize,
    result: &mut Normalized,
    entries: &[Wire],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<usize>, EditError> {
    let left = divide(result, index, reader)?;
    let mut work = singleton(index, reader)?;
    push(&mut work, left, reader)?;
    let mut logical = singleton(left, reader)?;
    push(&mut logical, index, reader)?;
    loop {
        reader.tick()?;
        let mut changed = false;
        let mut visit = 0;
        while visit < work.len() {
            let index = work[visit];
            visit += 1;
            if fits(&result.nodes[index], result, entries, reader)? {
                continue;
            }
            let left = divide(result, index, reader)?;
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
    Ok(logical)
}

fn divide(
    result: &mut Normalized,
    index: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    reader.tick()?;
    let (payload, maximum) = match &mut result.nodes[index].payload {
        Payload::Leaf(range) => {
            let count = range
                .end
                .checked_sub(range.start)
                .filter(|count| *count >= 2)
                .ok_or(EditError::Preservation)?;
            let middle = range.start + count / 2;
            let left = range.start..middle;
            range.start = middle;
            (Payload::Leaf(left), Some(middle - 1))
        }
        Payload::Branch { children, .. } => {
            if children.len() < 2 {
                return Err(EditError::Preservation);
            }
            let middle = children.len() / 2;
            let mut left = Vec::new();
            reserve(&mut left, middle, reader)?;
            reader.work(children.len())?;
            left.extend(children.drain(..middle));
            (
                Payload::Branch {
                    children: left,
                    tail: true,
                },
                None,
            )
        }
    };
    let maximum = match &payload {
        Payload::Leaf(_) => maximum,
        Payload::Branch { children, .. } => {
            result.nodes[*children.last().ok_or(EditError::Preservation)?].maximum
        }
    };
    let left = result.nodes.len();
    let height = result.nodes[index].height;
    reserve(&mut result.nodes, 1, reader)?;
    result.nodes.push(Node {
        origin: Origin::Fresh,
        height,
        maximum,
        payload,
    });
    push(&mut result.fresh, left, reader)?;
    Ok(left)
}

fn fits(
    node: &Node,
    result: &Normalized,
    entries: &[Wire],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<bool, EditError> {
    let seed = match node.origin {
        Origin::Existing { prefix, .. } => prefix,
        Origin::Fresh | Origin::GrownRoot { .. } => 0,
    };
    match &node.payload {
        Payload::Leaf(range) => Ok(frames::measure(&entries[range.clone()], seed, reader)?.fits()),
        Payload::Branch { children, tail } => {
            let count = children
                .len()
                .checked_sub(usize::from(*tail))
                .ok_or(EditError::Preservation)?;
            let mut maxima = Vec::new();
            reserve(&mut maxima, count, reader)?;
            for child in &children[..count] {
                reader.tick()?;
                let maximum = result
                    .nodes
                    .get(*child)
                    .and_then(|node| node.maximum)
                    .ok_or(EditError::Preservation)?;
                maxima.push(*entries.get(maximum).ok_or(EditError::Preservation)?);
            }
            Ok(frames::parent(&maxima, seed, reader)?.fits())
        }
    }
}

fn singleton(
    value: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<usize>, EditError> {
    let mut result = Vec::new();
    push(&mut result, value, reader)?;
    Ok(result)
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
