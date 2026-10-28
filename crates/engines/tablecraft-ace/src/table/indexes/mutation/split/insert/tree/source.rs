//! Original index topology and bounds. Final INSERT ranges never use old counts.
use super::{ENTRY_BUFFER, raw};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    limits::{MAX_INDEX_DEPTH, MAX_LIVE_ROWS},
    table::indexes::{
        header::IndexHeader,
        mutation::profile::SingleProfile,
        page::{self, KeyRef},
    },
};
use std::{io::Cursor, ops::Range};

pub(super) struct Node {
    pub number: u32,
    pub height: u8,
    pub bounds: Vec<KeyRef>,
    pub children: Vec<usize>,
    pub tail: bool,
    pub range: Range<usize>,
}

pub(super) struct Source {
    pub nodes: Vec<Node>,
    pub keys: Vec<KeyRef>,
    pub root: usize,
}

struct Capture<'a> {
    image: &'a NativeImage,
    owner: u32,
    profile: &'a SingleProfile,
    visited: Vec<bool>,
}

pub(super) fn capture(
    image: &NativeImage,
    index: &NativeIndex,
    owner: u32,
    profile: &SingleProfile,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Source, EditError> {
    let mut capture = Capture {
        image,
        owner,
        profile,
        visited: Vec::new(),
    };
    let pages = image.as_bytes().len() / PAGE_BYTES;
    reserve(&mut capture.visited, pages, reader)?;
    capture.visited.resize(pages, false);
    let mut source = Source {
        nodes: Vec::new(),
        keys: Vec::new(),
        root: 0,
    };
    source.root = visit(&mut capture, index.root_page(), 0, &mut source, reader)?;
    for pair in source.keys.windows(2) {
        reader.tick()?;
        if !pair[0].compare_with(&pair[1], profile).is_lt() {
            return Err(EditError::Preservation);
        }
    }
    for height in 0..=source.nodes[source.root].height {
        let mut previous = None;
        for node in source.nodes.iter().filter(|node| node.height == height) {
            reader.tick()?;
            let header = IndexHeader::read(raw(image, node.number)?)?;
            if header.previous.get() != previous.map(|prior: &Node| prior.number).unwrap_or(0) {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            if let Some(prior) = previous
                && IndexHeader::read(raw(image, prior.number)?)?.next.get() != node.number
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            previous = Some(node);
        }
        if let Some(last) = previous
            && IndexHeader::read(raw(image, last.number)?)?.next.get() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    Ok(source)
}

fn visit(
    capture: &mut Capture<'_>,
    number: u32,
    depth: usize,
    source: &mut Source,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    reader.tick()?;
    if depth >= MAX_INDEX_DEPTH {
        return Err(InspectError::StructureLimit.into());
    }
    let visited = capture
        .visited
        .get_mut(number as usize)
        .ok_or(EditError::Preservation)?;
    if *visited {
        return Err(EditError::Preservation);
    }
    *visited = true;
    let decoded =
        page::read_profile::<_, _, ENTRY_BUFFER>(reader, number, capture.owner, capture.profile)?;
    let start = source.keys.len();
    let index = source.nodes.len();
    reserve(&mut source.nodes, 1, reader)?;
    source.nodes.push(Node {
        number,
        height: 0,
        bounds: Vec::new(),
        children: Vec::new(),
        tail: decoded.tail != 0,
        range: start..start,
    });
    if decoded.leaf {
        if (decoded.entries.is_empty() && depth != 0) || decoded.tail != 0 {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let count = source
            .keys
            .len()
            .checked_add(decoded.entries.len())
            .ok_or(InspectError::StructureLimit)?;
        if count > MAX_LIVE_ROWS {
            return Err(InspectError::StructureLimit.into());
        }
        reserve(&mut source.keys, decoded.entries.len(), reader)?;
        for entry in decoded.entries {
            reader.tick()?;
            source.keys.push(entry.value);
        }
    } else {
        if decoded.entries.is_empty() && decoded.tail == 0 {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let mut children = Vec::new();
        let mut bounds = Vec::new();
        reserve(
            &mut children,
            decoded.entries.len() + usize::from(decoded.tail != 0),
            reader,
        )?;
        reserve(&mut bounds, decoded.entries.len(), reader)?;
        for entry in decoded.entries {
            let child = visit(capture, entry.child, depth + 1, source, reader)?;
            if source.keys.get(source.nodes[child].range.end - 1) != Some(&entry.value) {
                return Err(EditError::Preservation);
            }
            children.push(child);
            bounds.push(entry.value);
        }
        if decoded.tail != 0 {
            children.push(visit(capture, decoded.tail, depth + 1, source, reader)?);
        }
        let child_height = source.nodes[*children.first().ok_or(EditError::Preservation)?].height;
        for &child in &children {
            reader.tick()?;
            if source.nodes[child].height != child_height {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
        source.nodes[index].height = child_height
            .checked_add(1)
            .ok_or(InspectError::StructureLimit)?;
        source.nodes[index].children = children;
        source.nodes[index].bounds = bounds;
    }
    let node = &mut source.nodes[index];
    node.range.end = source.keys.len();
    if !IndexHeader::read(raw(capture.image, number)?)?.qualified_for_update(node.height) {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    Ok(index)
}

pub(super) fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader
        .budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    values
        .try_reserve(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(())
}
