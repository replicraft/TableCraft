//! Original-wire primary membership and historical-counter certification.
mod split;
mod tree;
use super::super::Staged;
use super::{ImageProof, Row, copied, reserve};
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, NativeIndex, TableCheck,
    format::{
        bytes::{u16_at, u32_at},
        layout::RECORD_COUNT_OFFSET,
        page::PAGE_BYTES,
        reader::Reader,
        row::{MAX_SLOTS, reference_page, reference_slot},
    },
    inspect_table_index_counter,
    table::indexes::{
        header::IndexHeader,
        page::{ENTRY_START, LEAF, NODE},
    },
};
use std::{io::Cursor, mem::offset_of, ops::Range};
use tablecraft_schema::ScalarType;

const INDEX_VERSION: u8 = 1;
const PRIMARY_KIND: u8 = 1;
const PRIMARY_PHYSICAL: u32 = 0;
const PHYSICAL_INDEX_COUNT: usize = 1;
const KEY_MARKER: u8 = 0x7f;
const KEY_BYTES: usize = size_of::<i32>();
const REFERENCE_BYTES: usize = size_of::<u32>();
const WIRE_BYTES: usize = size_of::<u8>() + KEY_BYTES + REFERENCE_BYTES;
const BITMAP_START: usize = size_of::<IndexHeader>();
const BITMAP_BITS: usize = u8::BITS as usize;
const FREE_OFFSET: usize = offset_of!(IndexHeader, free);
const PREFIX_OFFSET: usize = offset_of!(IndexHeader, prefix);
const SIGN_BIT: u32 = 1 << (i32::BITS - 1);

pub(super) fn prove(
    image: &NativeImage,
    checked: &TableCheck,
    id: usize,
    rows: &[Row],
    staged: &Staged,
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let index = primary(checked, id, reader)?;
    let root = page_range(index.root_page())?;
    let original = image
        .as_bytes()
        .get(root.clone())
        .ok_or(EditError::Preservation)?;
    let header = IndexHeader::read(original)?;
    let source_leaf =
        header.kind == LEAF && header.tail.get() == 0 && header.qualified_for_update(0);
    let source_node = header.kind == NODE;
    if (!source_leaf && !source_node)
        || header.version != INDEX_VERSION
        || header.owner.get() != checked.schema().definition_page()
        || header.previous.get() != 0
        || header.next.get() != 0
    {
        return Err(EditError::Preservation);
    }
    let (mut entries, old_active) = if source_leaf {
        decode(original, reader)?
    } else {
        (
            tree::original(image, index, checked.schema().definition_page(), reader)?,
            0,
        )
    };
    let original_count = entries.len();
    if original_count != checked.rows() {
        return Err(EditError::Preservation);
    }
    reserve(&mut entries, rows.len(), reader)?;
    for row in rows {
        reader.tick()?;
        reader.work(entries.len() * REFERENCE_BYTES)?;
        if reference_page(row.reference) == 0
            || usize::try_from(reference_page(row.reference))
                .map_err(|_| InspectError::StructureLimit)?
                >= proof.bytes.len() / PAGE_BYTES
            || usize::from(reference_slot(row.reference)) >= MAX_SLOTS
            || entries
                .iter()
                .any(|entry| reference(entry) == row.reference)
        {
            return Err(EditError::Preservation);
        }
        entries.push(encode(row.value, row.reference));
    }
    let levels = entries.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = entries
        .len()
        .checked_mul(levels)
        .and_then(|work| work.checked_mul(WIRE_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    reader.work(work)?;
    entries.sort_unstable();
    for adjacent in entries.windows(2) {
        reader.tick()?;
        if value(&adjacent[0]) >= value(&adjacent[1]) {
            return Err(EditError::Preservation);
        }
    }
    if staged.indexes.physical() != index.data_number()
        || staged.indexes.root() != index.root_page()
        || staged.indexes.usage() != index.usage_map()
        || staged.indexes.original_count() != original_count
        || staged.indexes.expected().len() != entries.len()
    {
        return Err(EditError::Preservation);
    }
    for (declared, entry) in staged.indexes.expected().zip(&entries) {
        reader.tick()?;
        if declared != (Some(i64::from(value(entry))), reference(entry)) {
            return Err(EditError::Preservation);
        }
    }
    let rendered = if source_leaf {
        if rows.is_empty() {
            Some((copied(original, reader)?, old_active))
        } else {
            render(original, &entries, reader)?
        }
    } else {
        None
    };
    let recursive =
        source_node || (rendered.is_none() && !split::two_halves_fit(&entries, reader)?);
    if recursive {
        let change = tree::derive(
            image,
            index,
            checked.schema().definition_page(),
            &entries,
            proof,
            reader,
        )?;
        change.compare(&staged.indexes, proof, reader)?;
        let mut reservations = staged.data.append.indexes();
        for expected in change.index_pages() {
            let actual = reservations.next().ok_or(EditError::Preservation)?;
            if actual.number != *expected
                || actual.physical != index.data_number()
                || actual.usage != index.usage_map()
            {
                return Err(EditError::Preservation);
            }
        }
        if reservations.next().is_some() {
            return Err(EditError::Preservation);
        }
        if let Some(promotion) = change.promotion() {
            staged.data.append.require_index_bitmap(
                promotion.number,
                index.data_number(),
                index.usage_map(),
                reader.budget,
                reader.cancelled,
            )?;
        }
    } else {
        if let Some((after, active)) = rendered {
            if !staged.indexes.added.is_empty()
                || !staged.indexes.promotions.is_empty()
                || !staged.indexes.rebases.is_empty()
            {
                return Err(EditError::Preservation);
            }
            patches(
                original,
                &after,
                root.start,
                old_active.max(active),
                staged,
                rows.is_empty(),
                reader,
            )?;
            reader.work(PAGE_BYTES)?;
            proof
                .bytes
                .get_mut(root)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&after);
        } else {
            let growth = split::derive(
                image,
                index,
                checked.schema().definition_page(),
                &entries,
                proof,
                reader,
            )?;
            growth.compare(index, &staged.indexes, proof, reader)?;
            let mut reservations = staged.data.append.indexes();
            for expected in [growth.right, growth.left] {
                let actual = reservations.next().ok_or(EditError::Preservation)?;
                if actual.number != expected
                    || actual.physical != index.data_number()
                    || actual.usage != index.usage_map()
                {
                    return Err(EditError::Preservation);
                }
            }
            if reservations.next().is_some() {
                return Err(EditError::Preservation);
            }
            if let Some(promotion) = &growth.promoted {
                staged.data.append.require_index_bitmap(
                    promotion.number,
                    index.data_number(),
                    index.usage_map(),
                    reader.budget,
                    reader.cancelled,
                )?;
            }
        }
    }
    counters(image, checked, index, rows.len(), staged, proof, reader)
}

fn primary<'a>(
    checked: &'a TableCheck,
    id: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<&'a NativeIndex, EditError> {
    let schema = checked.schema();
    let field = schema.columns().get(id).ok_or(EditError::Preservation)?;
    if field.scalar_type() != Some(ScalarType::Int32)
        || schema.layout.physical_indexes != PHYSICAL_INDEX_COUNT
        || checked.indexes().is_empty()
    {
        return Err(EditError::Preservation);
    }
    let first = checked.indexes()[0].index();
    let mut primary = None;
    for checked_index in checked.indexes() {
        reader.tick()?;
        let index = checked_index.index();
        let constraints = index.constraints().ok_or(EditError::Preservation)?;
        if checked_index.status() != IndexCheckStatus::Checked
            || index.native_type() > PRIMARY_KIND
            || index.data_number() != PRIMARY_PHYSICAL
            || index.root_page() != first.root_page()
            || index.usage_map() != first.usage_map()
            || index.flags() != first.flags()
            || index.columns() != first.columns()
            || index.columns().len() != 1
            || index.columns()[0].number() != field.number()
            || index.columns()[0].descending()
            || !constraints.unique()
            || !constraints.required()
            || constraints.ignore_nulls()
        {
            return Err(EditError::Preservation);
        }
        if constraints.primary_key() && primary.replace(index).is_some() {
            return Err(EditError::Preservation);
        }
    }
    primary.ok_or(EditError::Preservation)
}

fn decode(
    page: &[u8],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(Vec<[u8; WIRE_BYTES]>, usize), EditError> {
    let prefix = usize::from(u16_at(page, PREFIX_OFFSET)?);
    if prefix > WIRE_BYTES {
        return Err(EditError::Preservation);
    }
    let mut entries = Vec::new();
    let mut previous_end = 0;
    let mut first = [0; WIRE_BYTES];
    reader.work((ENTRY_START - BITMAP_START) * BITMAP_BITS)?;
    for endpoint in 0..(ENTRY_START - BITMAP_START) * BITMAP_BITS {
        if page[BITMAP_START + endpoint / BITMAP_BITS] & (1 << (endpoint % BITMAP_BITS)) == 0 {
            continue;
        }
        reader.tick()?;
        let shared = if entries.is_empty() { 0 } else { prefix };
        if endpoint > PAGE_BYTES - ENTRY_START
            || endpoint.checked_sub(previous_end) != Some(WIRE_BYTES - shared)
        {
            return Err(EditError::Preservation);
        }
        let mut wire = first;
        wire[shared..].copy_from_slice(&page[ENTRY_START + previous_end..ENTRY_START + endpoint]);
        if wire[0] != KEY_MARKER {
            return Err(EditError::Preservation);
        }
        if entries.is_empty() {
            first = wire;
        } else if entries
            .last()
            .is_some_and(|prior| value(prior) >= value(&wire))
        {
            return Err(EditError::Preservation);
        }
        reserve(&mut entries, 1, reader)?;
        entries.push(wire);
        previous_end = endpoint;
    }
    if usize::from(u16_at(page, FREE_OFFSET)?) != PAGE_BYTES - ENTRY_START - previous_end {
        return Err(EditError::Preservation);
    }
    Ok((entries, previous_end))
}

fn render(
    original: &[u8],
    entries: &[[u8; WIRE_BYTES]],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Option<(Vec<u8>, usize)>, EditError> {
    let mut common = entries.first().map_or(0, |_| WIRE_BYTES);
    if let Some(first) = entries.first() {
        for entry in &entries[1..] {
            reader.tick()?;
            reader.work(common)?;
            common = first[..common]
                .iter()
                .zip(entry)
                .take_while(|(left, right)| left == right)
                .count();
        }
    }
    let saved = entries.len().saturating_sub(1);
    let total = entries
        .len()
        .checked_mul(WIRE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let mut prefix = usize::from(u16_at(original, PREFIX_OFFSET)?).min(common);
    let capacity = PAGE_BYTES - ENTRY_START;
    if total - prefix * saved > capacity {
        prefix = common;
    }
    let active = total - prefix * saved;
    if active > capacity {
        return Ok(None);
    }
    let mut after = copied(original, reader)?;
    reader.work(ENTRY_START - BITMAP_START + active)?;
    after[BITMAP_START..ENTRY_START].fill(0);
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        reader.tick()?;
        let shared = if position == 0 { 0 } else { prefix };
        let next = end + WIRE_BYTES - shared;
        after[ENTRY_START + end..ENTRY_START + next].copy_from_slice(&entry[shared..]);
        after[BITMAP_START + next / BITMAP_BITS] |= 1 << (next % BITMAP_BITS);
        end = next;
    }
    after[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
        .copy_from_slice(&((capacity - active) as u16).to_le_bytes());
    after[PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()]
        .copy_from_slice(&(prefix as u16).to_le_bytes());
    Ok(Some((after, active)))
}

fn patches(
    original: &[u8],
    after: &[u8],
    root: usize,
    active: usize,
    staged: &Staged,
    empty: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let mut count = 0;
    if !empty {
        for range in [
            FREE_OFFSET..FREE_OFFSET + size_of::<u16>(),
            PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>(),
            BITMAP_START..ENTRY_START + active,
        ] {
            reader.tick()?;
            reader.work(range.len())?;
            if original[range.clone()] == after[range.clone()] {
                continue;
            }
            let patch = staged
                .indexes
                .patches
                .get(count)
                .ok_or(EditError::Preservation)?;
            if patch.offset != root + range.start
                || patch.before != original[range.clone()]
                || patch.after != after[range]
            {
                return Err(EditError::Preservation);
            }
            count += 1;
        }
    }
    if count != staged.indexes.patches.len() {
        return Err(EditError::Preservation);
    }
    Ok(())
}

fn counters(
    image: &NativeImage,
    checked: &TableCheck,
    index: &NativeIndex,
    inserted: usize,
    staged: &Staged,
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let schema = checked.schema();
    let mut source = image.reader();
    let historical = inspect_table_index_counter(
        &mut source,
        image.catalog().catalog().scope(),
        schema.name(),
        index.name(),
        reader.budget,
        reader.cancelled,
    )?;
    let range = historical.byte_range();
    let table = page_range(schema.definition_page())?.start + RECORD_COUNT_OFFSET;
    let count = u32_at(image.as_bytes(), table)?;
    if historical.definition_page() != schema.definition_page()
        || historical.physical_index() != index.data_number()
        || range.len() != size_of::<u32>()
        || count != schema.declared_rows()
        || count as usize != checked.rows()
    {
        return Err(EditError::Preservation);
    }
    if inserted == 0 {
        if !staged.counters.is_empty() {
            return Err(EditError::Preservation);
        }
        return Ok(());
    }
    let expected = [(table, count), (range.start, historical.value())];
    if staged.counters.len() != expected.len() {
        return Err(EditError::Preservation);
    }
    let delta = u32::try_from(inserted).map_err(|_| InspectError::StructureLimit)?;
    for (patch, (offset, value)) in staged.counters.iter().zip(expected) {
        reader.tick()?;
        let before = value.to_le_bytes();
        let after = value
            .checked_add(delta)
            .ok_or(InspectError::StructureLimit)?
            .to_le_bytes();
        reader.work(size_of::<u32>())?;
        if patch.offset != offset
            || patch.before != before
            || patch.after != after
            || image.as_bytes().get(offset..offset + size_of::<u32>()) != Some(before.as_slice())
        {
            return Err(EditError::Preservation);
        }
        proof
            .bytes
            .get_mut(offset..offset + size_of::<u32>())
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&after);
    }
    Ok(())
}

fn encode(value: i32, reference: u32) -> [u8; WIRE_BYTES] {
    let mut wire = [0; WIRE_BYTES];
    wire[0] = KEY_MARKER;
    wire[1..1 + KEY_BYTES].copy_from_slice(&((value as u32) ^ SIGN_BIT).to_be_bytes());
    wire[1 + KEY_BYTES..].copy_from_slice(&reference.to_be_bytes());
    wire
}

fn value(wire: &[u8; WIRE_BYTES]) -> i32 {
    (u32::from_be_bytes(wire[1..1 + KEY_BYTES].try_into().unwrap()) ^ SIGN_BIT) as i32
}

fn reference(wire: &[u8; WIRE_BYTES]) -> u32 {
    u32::from_be_bytes(wire[1 + KEY_BYTES..].try_into().unwrap())
}

fn page_range(page: u32) -> Result<Range<usize>, EditError> {
    let start = usize::try_from(page)
        .map_err(|_| InspectError::StructureLimit)?
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let end = start
        .checked_add(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    Ok(start..end)
}
