//! Original-bound INSERT routing and postorder symbolic normalization.
use super::{
    raw,
    source::{self, Source},
};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    limits::MAX_INDEX_DEPTH,
    table::indexes::{
        header::IndexHeader,
        mutation::{encoding, repack, retained::RetainedProfile},
        page::{ENTRY_START, Entry, KeyRef},
    },
};
use std::{io::Cursor, ops::Range};

const CAPACITY: usize = PAGE_BYTES - ENTRY_START;

#[derive(Clone, Copy)]
pub(super) enum Origin {
    Existing(u32),
    Fresh,
    GrownRoot(u32),
}

pub(super) enum Contents {
    Leaf(Range<usize>),
    Branch { children: Vec<usize>, tail: bool },
}

pub(super) struct Node {
    pub origin: Origin,
    pub height: u8,
    pub contents: Contents,
    pub maximum: usize,
}

pub(super) struct Layout {
    pub nodes: Vec<Node>,
    pub original: Vec<usize>,
    pub fresh: Vec<usize>,
    pub root: usize,
}

struct Context<'a, P: RetainedProfile, const N: usize> {
    image: &'a NativeImage,
    source: &'a Source<P::Key>,
    keys: &'a [KeyRef<P::Key>],
    profile: &'a P,
}

pub(super) fn prepare<P: RetainedProfile, const N: usize>(
    image: &NativeImage,
    source: &Source<P::Key>,
    keys: &[KeyRef<P::Key>],
    profile: &P,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Layout, EditError> {
    let context = Context::<_, N> {
        image,
        source,
        keys,
        profile,
    };
    let mut layout = Layout {
        nodes: Vec::new(),
        original: Vec::new(),
        fresh: Vec::new(),
        root: 0,
    };
    source::reserve(&mut layout.original, source.nodes.len(), reader)?;
    layout.original.resize(source.nodes.len(), usize::MAX);
    let roots = build(
        &context,
        source.root,
        0..keys.len(),
        true,
        &mut layout,
        reader,
    )?;
    if roots.len() != 1 {
        return Err(EditError::Preservation);
    }
    layout.root = roots[0];
    reader.work(layout.original.len())?;
    if layout.original.contains(&usize::MAX) {
        return Err(EditError::Preservation);
    }
    Ok(layout)
}

fn build<P: RetainedProfile, const N: usize>(
    context: &Context<'_, P, N>,
    original: usize,
    range: Range<usize>,
    root: bool,
    layout: &mut Layout,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<usize>, EditError> {
    reader.tick()?;
    if range.is_empty() || range.end > context.keys.len() {
        return Err(EditError::Preservation);
    }
    let source = context
        .source
        .nodes
        .get(original)
        .ok_or(EditError::Preservation)?;
    let contents = if source.children.is_empty() {
        Contents::Leaf(range.clone())
    } else {
        let mut children = Vec::new();
        let mut start = range.start;
        for (position, &child) in source.children.iter().enumerate() {
            let mut end = start;
            if position + 1 == source.children.len() {
                end = range.end;
            } else {
                let bound = source.bounds.get(position).ok_or(EditError::Preservation)?;
                while end < range.end {
                    reader.tick()?;
                    if context.keys[end]
                        .compare_with(bound, context.profile)
                        .is_gt()
                    {
                        break;
                    }
                    end += 1;
                }
            }
            let normalized = build(context, child, start..end, false, layout, reader)?;
            source::reserve(&mut children, normalized.len(), reader)?;
            reader.work(normalized.len())?;
            children.extend(normalized);
            start = end;
        }
        Contents::Branch {
            children,
            tail: source.tail,
        }
    };
    let node = push_node(
        layout,
        Node {
            origin: Origin::Existing(source.number),
            height: source.height,
            contents,
            maximum: range.end - 1,
        },
        reader,
    )?;
    layout.original[original] = node;
    if fits(context, layout, node, reader)? {
        return singleton(node, reader);
    }
    if !root {
        let left = divide(layout, node, reader)?;
        return normalize(context, layout, node, left, reader);
    }

    let height = source
        .height
        .checked_add(1)
        .filter(|height| usize::from(*height) < MAX_INDEX_DEPTH)
        .ok_or(InspectError::StructureLimit)?;
    let leaf_root = matches!(layout.nodes[node].contents, Contents::Leaf(_));
    let contents = match &mut layout.nodes[node].contents {
        Contents::Branch { children, .. } => Contents::Branch {
            children: std::mem::take(children),
            tail: true,
        },
        Contents::Leaf(range) => Contents::Leaf(range.clone()),
    };
    // An overflowing internal root moves its completed contents into a Fresh
    // RIGHT before the initial LEFT is created. Neither nested page inherits
    // the original root's seed or opaque bytes.
    let right = push_node(
        layout,
        Node {
            origin: Origin::Fresh,
            height: source.height,
            contents,
            maximum: range.end - 1,
        },
        reader,
    )?;
    push(&mut layout.fresh, right, reader)?;
    let left = divide(layout, right, reader)?;
    let children = normalize(context, layout, right, left, reader)?;
    layout.nodes[node].origin = Origin::GrownRoot(source.number);
    layout.nodes[node].height = height;
    layout.nodes[node].contents = Contents::Branch {
        children,
        tail: leaf_root,
    };
    if !fits(context, layout, node, reader)? {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    singleton(node, reader)
}

fn normalize<P: RetainedProfile, const N: usize>(
    context: &Context<'_, P, N>,
    layout: &mut Layout,
    right: usize,
    left: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<usize>, EditError> {
    let mut work = Vec::new();
    push(&mut work, right, reader)?;
    push(&mut work, left, reader)?;
    let mut ordered = Vec::new();
    push(&mut ordered, left, reader)?;
    push(&mut ordered, right, reader)?;
    loop {
        reader.tick()?;
        let mut changed = false;
        let mut position = 0;
        while position < work.len() {
            let current = work[position];
            position += 1;
            if fits(context, layout, current, reader)? {
                continue;
            }
            let left = divide(layout, current, reader)?;
            push(&mut work, left, reader)?;
            reader.work(ordered.len())?;
            let before = ordered
                .iter()
                .position(|&node| node == current)
                .ok_or(EditError::Preservation)?;
            source::reserve(&mut ordered, 1, reader)?;
            ordered.insert(before, left);
            changed = true;
        }
        if !changed {
            return Ok(ordered);
        }
    }
}

fn divide(
    layout: &mut Layout,
    right: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    reader.tick()?;
    let height = layout.nodes[right].height;
    let (contents, maximum) = match &layout.nodes[right].contents {
        Contents::Leaf(range) => {
            if range.len() < 2 {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            let middle = range.start + range.len() / 2;
            (Contents::Leaf(range.start..middle), middle - 1)
        }
        Contents::Branch { children, .. } => {
            if children.len() < 2 {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            let middle = children.len() / 2;
            let mut left = Vec::new();
            source::reserve(&mut left, middle, reader)?;
            reader.work(middle)?;
            left.extend_from_slice(&children[..middle]);
            let maximum = layout.nodes[*left.last().ok_or(EditError::Preservation)?].maximum;
            (
                Contents::Branch {
                    children: left,
                    tail: true,
                },
                maximum,
            )
        }
    };
    match &mut layout.nodes[right].contents {
        Contents::Leaf(range) => {
            let middle = range.start + range.len() / 2;
            range.start = middle;
        }
        Contents::Branch { children, .. } => {
            let middle = children.len() / 2;
            reader.work(children.len())?;
            drop(children.drain(..middle));
        }
    }
    let left = push_node(
        layout,
        Node {
            origin: Origin::Fresh,
            height,
            contents,
            maximum,
        },
        reader,
    )?;
    push(&mut layout.fresh, left, reader)?;
    Ok(left)
}

fn fits<P: RetainedProfile, const N: usize>(
    context: &Context<'_, P, N>,
    layout: &Layout,
    index: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<bool, EditError> {
    let node = &layout.nodes[index];
    match &node.contents {
        Contents::Leaf(range) => {
            let entries = super::leaf_entries(
                &context.keys[range.clone()],
                reader.budget,
                reader.cancelled,
            )?;
            match node.origin {
                Origin::Existing(number) => repack::borrowed_fits::<_, N>(
                    raw(context.image, number)?,
                    &entries,
                    context.profile,
                    reader.budget,
                    reader.cancelled,
                ),
                Origin::Fresh => repack::borrowed_fits_fresh::<_, N>(
                    &entries,
                    context.profile,
                    reader.budget,
                    reader.cancelled,
                ),
                Origin::GrownRoot(_) => Err(EditError::Preservation),
            }
        }
        Contents::Branch { children, tail } => {
            let count = children
                .len()
                .checked_sub(usize::from(*tail))
                .ok_or(EditError::Preservation)?;
            let mut first = [0; N];
            let mut common = 0;
            let mut total = 0usize;
            for (position, &child) in children[..count].iter().enumerate() {
                reader.work(N)?;
                let key = &context.keys[layout.nodes[child].maximum];
                let encoded = encoding::borrowed_entry::<_, N>(
                    &Entry {
                        value: KeyRef {
                            key: &key.key,
                            row: key.row,
                        },
                        child: 0,
                    },
                    context.profile,
                )?;
                let key_row_bytes = encoded.length;
                total = total
                    .checked_add(key_row_bytes + size_of::<u32>())
                    .ok_or(InspectError::StructureLimit)?;
                if position == 0 {
                    first[..key_row_bytes].copy_from_slice(&encoded.bytes[..key_row_bytes]);
                    common = key_row_bytes;
                } else {
                    common = first[..common.min(key_row_bytes)]
                        .iter()
                        .zip(&encoded.bytes[..common.min(key_row_bytes)])
                        .take_while(|(left, right)| left == right)
                        .count();
                }
            }
            // Child references cannot extend the common prefix: every bound
            // has a distinct complete key/ROW before its child pointer.
            if count < 2 {
                return Ok(total <= CAPACITY);
            }
            let seed = match node.origin {
                Origin::Existing(number) => {
                    usize::from(IndexHeader::read(raw(context.image, number)?)?.prefix.get())
                }
                Origin::Fresh | Origin::GrownRoot(_) => 0,
            };
            let saved = count - 1;
            Ok(total - seed.min(common) * saved <= CAPACITY || total - common * saved <= CAPACITY)
        }
    }
}

fn push_node(
    layout: &mut Layout,
    node: Node,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    source::reserve(&mut layout.nodes, 1, reader)?;
    let index = layout.nodes.len();
    layout.nodes.push(node);
    Ok(index)
}

fn push(
    values: &mut Vec<usize>,
    value: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    source::reserve(values, 1, reader)?;
    values.push(value);
    Ok(())
}

fn singleton(
    value: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<usize>, EditError> {
    let mut result = Vec::new();
    push(&mut result, value, reader)?;
    Ok(result)
}
