//! INSERT-only routed ranges and symbolic leaf normalization, before allocation.
use super::{ENTRY_BUFFER, leaf_entries, raw};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    table::indexes::{
        header::IndexHeader,
        mutation::{encoding, profile::SingleProfile, repack, tree::Tree},
        page::{ENTRY_START, Entry, KeyRef},
    },
};
use std::{io::Cursor, ops::Range};

const KEY_ROW_BYTES: usize = size_of::<u8>() + size_of::<i32>() + size_of::<u32>();
const BOUND_BYTES: usize = KEY_ROW_BYTES + size_of::<u32>();
const CAPACITY: usize = PAGE_BYTES - ENTRY_START;

#[derive(Clone, Copy)]
pub(super) enum Origin {
    Existing(u32),
    Fresh,
}

pub(super) struct Leaf {
    pub origin: Origin,
    pub range: Range<usize>,
}

pub(super) struct Layout {
    pub leaves: Vec<Leaf>,
    pub order: Vec<usize>,
    pub original: Vec<usize>,
    pub fresh: Vec<usize>,
    pub tail: bool,
}

struct Context<'a> {
    image: &'a NativeImage,
    keys: &'a [KeyRef],
    profile: &'a SingleProfile,
}

pub(super) fn prepare(
    image: &NativeImage,
    tree: &Tree,
    ranges: &[Range<usize>],
    keys: &[KeyRef],
    profile: &SingleProfile,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Layout, EditError> {
    if ranges.len() != tree.children().len() {
        return Err(EditError::Preservation);
    }
    let tail = IndexHeader::read(raw(image, tree.number())?)?.tail.get() != 0;
    let context = Context {
        image,
        keys,
        profile,
    };
    let mut layout = Layout {
        leaves: Vec::new(),
        order: Vec::new(),
        original: Vec::new(),
        fresh: Vec::new(),
        tail,
    };
    for (child, range) in tree.children().iter().zip(ranges) {
        reader.tick()?;
        if range.is_empty() || range.end > keys.len() {
            return Err(EditError::Preservation);
        }
        let existing = push(
            &mut layout.leaves,
            Leaf {
                origin: Origin::Existing(child.number()),
                range: range.clone(),
            },
            reader,
        )?;
        push(&mut layout.original, existing, reader)?;
        if fits(&context, &layout.leaves[existing], reader)? {
            push(&mut layout.order, existing, reader)?;
            continue;
        }

        // Match the established schedule: the first division happens before
        // the initial RIGHT/LEFT worklist is visited, not during that visit.
        let left = divide(&mut layout, existing, reader)?;
        let mut work = Vec::new();
        push(&mut work, existing, reader)?;
        push(&mut work, left, reader)?;
        let mut ordered = Vec::new();
        push(&mut ordered, left, reader)?;
        push(&mut ordered, existing, reader)?;
        loop {
            reader.tick()?;
            let mut changed = false;
            let mut position = 0;
            while position < work.len() {
                let current = work[position];
                position += 1;
                if fits(&context, &layout.leaves[current], reader)? {
                    continue;
                }
                let left = divide(&mut layout, current, reader)?;
                push(&mut work, left, reader)?;
                reader.work(ordered.len())?;
                let before = ordered
                    .iter()
                    .position(|&entry| entry == current)
                    .ok_or(EditError::Preservation)?;
                reserve(&mut ordered, 1, reader)?;
                ordered.insert(before, left);
                changed = true;
            }
            if !changed {
                break;
            }
        }
        reserve(&mut layout.order, ordered.len(), reader)?;
        reader.work(ordered.len())?;
        layout.order.extend(ordered);
    }
    if !root_fits(image, tree.number(), keys, &layout, profile, reader)? {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    Ok(layout)
}

fn fits(
    context: &Context<'_>,
    leaf: &Leaf,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<bool, EditError> {
    let keys = context
        .keys
        .get(leaf.range.clone())
        .ok_or(EditError::Preservation)?;
    let entries = leaf_entries(keys, reader.budget, reader.cancelled)?;
    match leaf.origin {
        Origin::Existing(number) => repack::borrowed_fits::<_, ENTRY_BUFFER>(
            raw(context.image, number)?,
            &entries,
            context.profile,
            reader.budget,
            reader.cancelled,
        ),
        Origin::Fresh => repack::borrowed_fits_fresh::<_, ENTRY_BUFFER>(
            &entries,
            context.profile,
            reader.budget,
            reader.cancelled,
        ),
    }
}

fn divide(
    layout: &mut Layout,
    current: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    reader.tick()?;
    let range = layout
        .leaves
        .get(current)
        .ok_or(EditError::Preservation)?
        .range
        .clone();
    if range.len() < 2 {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let middle = range.start + range.len() / 2;
    let left = push(
        &mut layout.leaves,
        Leaf {
            origin: Origin::Fresh,
            range: range.start..middle,
        },
        reader,
    )?;
    layout.leaves[current].range.start = middle;
    push(&mut layout.fresh, left, reader)?;
    Ok(left)
}

fn root_fits(
    image: &NativeImage,
    root: u32,
    keys: &[KeyRef],
    layout: &Layout,
    profile: &SingleProfile,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<bool, EditError> {
    let count = layout
        .order
        .len()
        .checked_sub(usize::from(layout.tail))
        .ok_or(EditError::Preservation)?;
    if count < 2 {
        return Ok(count * BOUND_BYTES <= CAPACITY);
    }
    let mut first = [0; KEY_ROW_BYTES];
    let mut common = KEY_ROW_BYTES;
    for (position, &child) in layout.order[..count].iter().enumerate() {
        reader.work(KEY_ROW_BYTES)?;
        let last = layout.leaves[child]
            .range
            .end
            .checked_sub(1)
            .ok_or(EditError::Preservation)?;
        let value = *keys.get(last).ok_or(EditError::Preservation)?;
        let encoded =
            encoding::retained_entry::<_, ENTRY_BUFFER>(&Entry { value, child: 0 }, profile)?;
        if encoded.length != KEY_ROW_BYTES {
            return Err(EditError::Preservation);
        }
        if position == 0 {
            first.copy_from_slice(&encoded.bytes[..KEY_ROW_BYTES]);
        } else {
            common = first[..common]
                .iter()
                .zip(&encoded.bytes[..common])
                .take_while(|(left, right)| left == right)
                .count();
        }
    }
    let seed = usize::from(IndexHeader::read(raw(image, root)?)?.prefix.get());
    let saved = count - 1;
    let total = count
        .checked_mul(BOUND_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let seeded = total
        .checked_sub(seed.min(common) * saved)
        .ok_or(EditError::Preservation)?;
    Ok(seeded <= CAPACITY || total - common * saved <= CAPACITY)
}

pub(super) fn bounds<'a>(
    layout: &Layout,
    keys: &'a [KeyRef],
    numbers: &[u32],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<Entry<&'a Option<i64>>>, EditError> {
    let count = layout
        .order
        .len()
        .checked_sub(usize::from(layout.tail))
        .ok_or(EditError::Preservation)?;
    let mut entries = Vec::new();
    reserve(&mut entries, count, reader)?;
    for &child in &layout.order[..count] {
        reader.tick()?;
        let last = layout.leaves[child]
            .range
            .end
            .checked_sub(1)
            .ok_or(EditError::Preservation)?;
        let key = keys.get(last).ok_or(EditError::Preservation)?;
        let number = *numbers
            .get(child)
            .filter(|number| **number != 0)
            .ok_or(EditError::Preservation)?;
        entries.push(Entry {
            value: KeyRef {
                key: &key.key,
                row: key.row,
            },
            child: number,
        });
    }
    Ok(entries)
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

fn push<T>(
    values: &mut Vec<T>,
    value: T,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    reserve(values, 1, reader)?;
    let index = values.len();
    values.push(value);
    Ok(index)
}
