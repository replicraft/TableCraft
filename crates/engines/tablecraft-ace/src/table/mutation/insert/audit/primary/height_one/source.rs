//! Original raw root bounds, complete child streams and canonical ownership.
use super::super::{
    BITMAP_BITS, BITMAP_START, FREE_OFFSET, INDEX_VERSION, KEY_MARKER, PREFIX_OFFSET, WIRE_BYTES,
    decode, reference, value,
};
use super::{raw, reserve};
use crate::{
    EditError, NativeImage, NativeIndex,
    format::{allocation, bytes::u16_at, page::PAGE_BYTES, reader::Reader},
    limits::MAX_LIVE_ROWS,
    table::indexes::{
        header::IndexHeader,
        page::{ENTRY_START, LEAF, NODE},
    },
};
use std::{io::Cursor, ops::Range};

const BOUND_BYTES: usize = WIRE_BYTES + size_of::<u32>();
const ROOT_HEIGHT: u8 = 1;
const LEAF_HEIGHT: u8 = 0;

pub(super) struct Source {
    pub root: u32,
    pub definition: u32,
    pub prefix: usize,
    pub tail: bool,
    pub children: Vec<Child>,
    pub entries: Vec<[u8; WIRE_BYTES]>,
}

pub(super) struct Child {
    pub number: u32,
    pub prefix: usize,
    pub range: Range<usize>,
}

impl Source {
    pub(super) fn capture(
        image: &NativeImage,
        index: &NativeIndex,
        definition: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        let root = raw(image, index.root_page())?;
        let header = IndexHeader::read(root)?;
        if header.kind != NODE
            || header.version != INDEX_VERSION
            || header.owner.get() != definition
            || !header.qualified_for_update(ROOT_HEIGHT)
            || header.previous.get() != 0
            || header.next.get() != 0
        {
            return Err(EditError::Preservation);
        }
        let prefix = usize::from(header.prefix.get());
        let bounds = decode_root(root, reader)?;
        let tail = header.tail.get() != 0;
        let mut numbers = Vec::new();
        reserve(&mut numbers, bounds.len() + usize::from(tail), reader)?;
        numbers.extend(bounds.iter().map(|(_, number)| *number));
        if tail {
            numbers.push(header.tail.get());
        }
        if numbers.is_empty() || numbers.len() > MAX_LIVE_ROWS {
            return Err(EditError::Preservation);
        }
        for (position, number) in numbers.iter().enumerate() {
            reader.work(position + 1)?;
            if *number == 0 || *number == index.root_page() || numbers[..position].contains(number)
            {
                return Err(EditError::Preservation);
            }
        }

        let mut owned = Vec::new();
        reserve(&mut owned, numbers.len() + 1, reader)?;
        owned.extend_from_slice(&numbers);
        owned.push(index.root_page());
        reader.work(owned.len().saturating_mul(owned.len()))?;
        owned.sort_unstable();
        if allocation::pages(reader, index.usage_map())? != owned {
            return Err(EditError::Preservation);
        }

        let mut entries = Vec::new();
        let mut children = Vec::new();
        reserve(&mut children, numbers.len(), reader)?;
        for (position, number) in numbers.iter().enumerate() {
            let bytes = raw(image, *number)?;
            let header = IndexHeader::read(bytes)?;
            let previous = position.checked_sub(1).map_or(0, |offset| numbers[offset]);
            let next = numbers.get(position + 1).copied().unwrap_or(0);
            if header.kind != LEAF
                || header.version != INDEX_VERSION
                || header.owner.get() != definition
                || !header.qualified_for_update(LEAF_HEIGHT)
                || header.previous.get() != previous
                || header.next.get() != next
                || header.tail.get() != 0
            {
                return Err(EditError::Preservation);
            }
            let (keys, _) = decode(bytes, reader)?;
            if keys.is_empty() || entries.len().saturating_add(keys.len()) > MAX_LIVE_ROWS {
                return Err(EditError::Preservation);
            }
            if let Some((bound, child)) = bounds.get(position)
                && (*child != *number || keys.last() != Some(bound))
            {
                return Err(EditError::Preservation);
            }
            let start = entries.len();
            reserve(&mut entries, keys.len(), reader)?;
            entries.extend(keys);
            children.push(Child {
                number: *number,
                prefix: usize::from(header.prefix.get()),
                range: start..entries.len(),
            });
        }
        membership(&entries, &entries, reader)?;

        Ok(Self {
            root: index.root_page(),
            definition,
            prefix,
            tail,
            children,
            entries,
        })
    }
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
