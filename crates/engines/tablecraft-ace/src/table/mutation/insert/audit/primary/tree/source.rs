//! Original raw root bounds, complete child streams and canonical ownership.
use super::super::{
    BITMAP_BITS, BITMAP_START, FREE_OFFSET, INDEX_VERSION, KEY_MARKER, PREFIX_OFFSET, WIRE_BYTES,
    decode, reference, value,
};
use super::{raw, reserve};
use crate::{
    EditError, NativeImage, NativeIndex,
    format::{allocation, bytes::u16_at, page::PAGE_BYTES, reader::Reader},
    limits::{MAX_INDEX_DEPTH, MAX_LIVE_ROWS},
    table::indexes::{
        header::IndexHeader,
        page::{ENTRY_START, LEAF, NODE},
    },
};
use std::{io::Cursor, ops::Range};

const BOUND_BYTES: usize = WIRE_BYTES + size_of::<u32>();

pub(super) struct Source {
    pub root: Branch,
    pub definition: u32,
    pub pages: Vec<u32>,
    pub entries: Vec<[u8; WIRE_BYTES]>,
}

pub(super) struct Branch {
    pub number: u32,
    pub prefix: usize,
    pub height: u8,
    pub tail: bool,
    pub range: Range<usize>,
    pub children: Vec<Branch>,
}

struct Capture {
    pages: Vec<u32>,
    entries: Vec<[u8; WIRE_BYTES]>,
    levels: Vec<Vec<u32>>,
}

impl Source {
    pub(super) fn capture(
        image: &NativeImage,
        index: &NativeIndex,
        definition: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        let mut capture = Capture {
            pages: Vec::new(),
            entries: Vec::new(),
            levels: Vec::new(),
        };
        let root = walk(
            image,
            index.root_page(),
            definition,
            0,
            &mut capture,
            reader,
        )?;
        for level in &capture.levels {
            for (position, number) in level.iter().enumerate() {
                reader.tick()?;
                let header = IndexHeader::read(raw(image, *number)?)?;
                let previous = position.checked_sub(1).map_or(0, |i| level[i]);
                let next = level.get(position + 1).copied().unwrap_or(0);
                if header.previous.get() != previous || header.next.get() != next {
                    return Err(EditError::Preservation);
                }
            }
        }

        let mut owned = super::copied_numbers(&capture.pages, reader)?;
        reader.work(owned.len().saturating_mul(owned.len()))?;
        owned.sort_unstable();
        if allocation::pages(reader, index.usage_map())? != owned {
            return Err(EditError::Preservation);
        }
        membership(&capture.entries, &capture.entries, reader)?;
        Ok(Self {
            root,
            definition,
            pages: capture.pages,
            entries: capture.entries,
        })
    }
}

fn walk(
    image: &NativeImage,
    number: u32,
    definition: u32,
    depth: usize,
    capture: &mut Capture,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Branch, EditError> {
    reader.work(capture.pages.len() + 1)?;
    if depth >= MAX_INDEX_DEPTH || number == 0 || capture.pages.contains(&number) {
        return Err(EditError::Preservation);
    }
    reserve(&mut capture.pages, 1, reader)?;
    capture.pages.push(number);
    let page = raw(image, number)?;
    let header = IndexHeader::read(page)?;
    if header.version != INDEX_VERSION || header.owner.get() != definition {
        return Err(EditError::Preservation);
    }
    let start = capture.entries.len();
    let mut children = Vec::new();
    let height = if header.kind == LEAF {
        let (entries, _) = decode(page, reader)?;
        if header.tail.get() != 0
            || (entries.is_empty() && header.prefix.get() != 0)
            || (depth != 0 && entries.is_empty())
            || capture.entries.len().saturating_add(entries.len()) > MAX_LIVE_ROWS
        {
            return Err(EditError::Preservation);
        }
        reserve(&mut capture.entries, entries.len(), reader)?;
        reader.work(entries.len().saturating_mul(WIRE_BYTES))?;
        capture.entries.extend(entries);
        0
    } else if header.kind == NODE {
        let bounds = decode_root(page, reader)?;
        let count = bounds.len() + usize::from(header.tail.get() != 0);
        if count == 0 || count > MAX_LIVE_ROWS {
            return Err(EditError::Preservation);
        }
        reserve(&mut children, count, reader)?;
        for (bound, child) in &bounds {
            let source = walk(image, *child, definition, depth + 1, capture, reader)?;
            if source.range.is_empty() || capture.entries.get(source.range.end - 1) != Some(bound) {
                return Err(EditError::Preservation);
            }
            children.push(source);
        }
        if header.tail.get() != 0 {
            children.push(walk(
                image,
                header.tail.get(),
                definition,
                depth + 1,
                capture,
                reader,
            )?);
        }
        let first = children.first().ok_or(EditError::Preservation)?.height;
        if children.iter().any(|child| child.height != first) {
            return Err(EditError::Preservation);
        }
        first.checked_add(1).ok_or(EditError::Preservation)?
    } else {
        return Err(EditError::Preservation);
    };
    if usize::from(height) >= MAX_INDEX_DEPTH || !header.qualified_for_update(height) {
        return Err(EditError::Preservation);
    }
    while capture.levels.len() <= usize::from(height) {
        reserve(&mut capture.levels, 1, reader)?;
        capture.levels.push(Vec::new());
    }
    let level = &mut capture.levels[usize::from(height)];
    reserve(level, 1, reader)?;
    level.push(number);

    Ok(Branch {
        number,
        prefix: usize::from(header.prefix.get()),
        height,
        tail: header.tail.get() != 0,
        range: start..capture.entries.len(),
        children,
    })
}

fn decode_root(
    page: &[u8],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<([u8; WIRE_BYTES], u32)>, EditError> {
    let prefix = usize::from(u16_at(page, PREFIX_OFFSET)?);
    if prefix > BOUND_BYTES {
        return Err(EditError::Preservation);
    }
    let mut bounds = Vec::new();
    let mut first = [0; BOUND_BYTES];
    let mut previous_end = 0;
    reader.work((ENTRY_START - BITMAP_START) * BITMAP_BITS)?;
    for endpoint in 0..(ENTRY_START - BITMAP_START) * BITMAP_BITS {
        if page[BITMAP_START + endpoint / BITMAP_BITS] & (1 << (endpoint % BITMAP_BITS)) == 0 {
            continue;
        }
        reader.tick()?;
        let shared = if bounds.is_empty() { 0 } else { prefix };
        if endpoint > PAGE_BYTES - ENTRY_START
            || endpoint.checked_sub(previous_end) != Some(BOUND_BYTES - shared)
        {
            return Err(EditError::Preservation);
        }
        let mut wire = first;
        wire[shared..].copy_from_slice(&page[ENTRY_START + previous_end..ENTRY_START + endpoint]);
        if bounds.is_empty() {
            first = wire;
        }
        let mut key = [0; WIRE_BYTES];
        key.copy_from_slice(&wire[..WIRE_BYTES]);
        if key[0] != KEY_MARKER
            || bounds
                .last()
                .is_some_and(|(prior, _)| value(prior) >= value(&key))
        {
            return Err(EditError::Preservation);
        }
        let child = u32::from_be_bytes(
            wire[WIRE_BYTES..]
                .try_into()
                .map_err(|_| EditError::Preservation)?,
        );
        reserve(&mut bounds, 1, reader)?;
        bounds.push((key, child));
        previous_end = endpoint;
    }
    if (bounds.is_empty() && prefix != 0)
        || usize::from(u16_at(page, FREE_OFFSET)?) != PAGE_BYTES - ENTRY_START - previous_end
    {
        return Err(EditError::Preservation);
    }

    Ok(bounds)
}

pub(super) fn membership(
    original: &[[u8; WIRE_BYTES]],
    entries: &[[u8; WIRE_BYTES]],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if entries.len() < original.len() || entries.len() > MAX_LIVE_ROWS {
        return Err(EditError::Preservation);
    }
    for (position, entry) in entries.iter().enumerate() {
        reader.work(position + 1)?;
        if entry[0] != KEY_MARKER
            || (position > 0 && value(&entries[position - 1]) >= value(entry))
            || entries[..position]
                .iter()
                .any(|prior| reference(prior) == reference(entry))
        {
            return Err(EditError::Preservation);
        }
    }
    for entry in original {
        reader.work(entries.len())?;
        if entries.binary_search(entry).is_err() {
            return Err(EditError::Preservation);
        }
    }

    Ok(())
}
