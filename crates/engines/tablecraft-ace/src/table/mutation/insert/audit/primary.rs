//! Independently certified physical numeric index membership and framing.
mod counters;
mod eligible;
mod profile;
mod selection;
mod split;
mod tree;
mod wire;
use super::super::{RowInsert, Staged};
use super::{ImageProof, Row, copied, reserve};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableCheck,
    format::{
        bytes::u16_at,
        page::PAGE_BYTES,
        reader::Reader,
        row::{MAX_SLOTS, reference_page, reference_slot},
    },
    table::indexes::{
        header::IndexHeader,
        mutation::insertion::IndexInsertion,
        page::{ENTRY_START, LEAF, NODE},
    },
};
use profile::Profile;
use std::{io::Cursor, mem::offset_of, ops::Range};
use wire::{MAX_WIRE_BYTES, Wire};

const INDEX_VERSION: u8 = 1;
const REFERENCE_BYTES: usize = size_of::<u32>();
const BITMAP_START: usize = size_of::<IndexHeader>();
const BITMAP_BITS: usize = u8::BITS as usize;
const FREE_OFFSET: usize = offset_of!(IndexHeader, free);
const PREFIX_OFFSET: usize = offset_of!(IndexHeader, prefix);

pub(super) struct Operation<'a> {
    pub image: &'a NativeImage,
    pub checked: &'a TableCheck,
    pub id: usize,
    pub requests: &'a [RowInsert],
    pub rows: &'a [Row],
    pub staged: &'a Staged,
}

pub(super) fn prove(
    operation: Operation<'_>,
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let Operation {
        image,
        checked,
        id,
        requests,
        rows,
        staged,
    } = operation;
    let selections = selection::capture(checked, id, reader)?;
    if selections.len() != staged.indexes.len() || requests.len() != rows.len() {
        return Err(EditError::Preservation);
    }
    let mut sources: Vec<(Vec<Wire>, Vec<u32>)> = Vec::new();
    reserve(&mut sources, selections.len(), reader)?;
    for selection in &selections {
        let source = tree::inventory(
            image,
            selection.index,
            checked.schema().definition_page(),
            reader,
        )?;
        for (_, previous) in &sources {
            for number in &source.1 {
                reader.work(previous.len() + 1)?;
                if previous.contains(number) {
                    return Err(EditError::Preservation);
                }
            }
        }
        sources.push(source);
    }
    let primary_references = eligible::primary_references(checked, &selections, &sources, reader)?;

    let mut deltas = Vec::new();
    reserve(&mut deltas, selections.len(), reader)?;
    for ((selection, actual), (mut entries, pages)) in
        selections.iter().zip(&staged.indexes).zip(sources)
    {
        let index = selection.index;
        let profile = &selection.profile;
        let original_count = entries.len();
        if actual.source_pages() != pages {
            return Err(EditError::Preservation);
        }
        let mut facts = if profile.ignore_nulls || profile.variable() {
            eligible::original(
                checked,
                selection,
                primary_references
                    .as_deref()
                    .ok_or(EditError::Preservation)?,
                &entries,
                reader,
            )?
        } else {
            if original_count != checked.rows() {
                return Err(EditError::Preservation);
            }
            Vec::new()
        };
        for entry in &entries {
            reader.work(entry.len())?;
            eligible::nulls(profile, entry, &facts, reader)?;
            if profile.unique && !profile.exact_unique(entry) {
                return Err(EditError::Preservation);
            }
        }
        let mut distinct = 0;
        reserve(&mut entries, rows.len(), reader)?;
        for (request, row) in requests.iter().zip(rows) {
            reader.tick()?;
            let wire = profile.request(request.values(), row.reference, reader)?;
            let nulls = profile.row_nulls(request.values(), reader)?;
            reader.work(entries.len() * REFERENCE_BYTES)?;
            if reference_page(row.reference) == 0
                || reference_page(row.reference) as usize >= proof.bytes.len() / PAGE_BYTES
                || usize::from(reference_slot(row.reference)) >= MAX_SLOTS
                || entries
                    .iter()
                    .any(|entry| reference(entry) == row.reference)
            {
                return Err(EditError::Preservation);
            }
            if profile.ignore_nulls && nulls.all {
                continue;
            }
            if profile.unique && !profile.exact_unique(&wire) {
                return Err(EditError::Preservation);
            }
            if profile.variable() {
                reserve(&mut facts, 1, reader)?;
                facts.push(eligible::RowFacts {
                    reference: row.reference,
                    nulls,
                });
            }
            let mut found = false;
            for entry in &entries {
                reader.work(entry.len())?;
                found |=
                    entry[..entry.len() - REFERENCE_BYTES] == wire[..wire.len() - REFERENCE_BYTES];
            }
            if !found {
                distinct += 1;
            }
            reader.work(size_of::<Wire>())?;
            entries.push(wire);
        }
        if profile.variable() {
            eligible::sort_facts(&mut facts, reader)?;
        }
        wire::sort_paid(&mut entries, reader)?;
        for pair in entries.windows(2) {
            reader.work(pair[0].len() + pair[1].len())?;
            if pair[0] >= pair[1]
                || (profile.unique
                    && !eligible::nulls(profile, &pair[0], &facts, reader)?.all
                    && pair[0][..pair[0].len() - REFERENCE_BYTES]
                        == pair[1][..pair[1].len() - REFERENCE_BYTES])
            {
                return Err(EditError::Preservation);
            }
        }
        if actual.physical() != index.data_number()
            || actual.root() != index.root_page()
            || actual.usage() != index.usage_map()
            || actual.original_count() != original_count
            || actual.distinct_added() != distinct
            || actual.expected().len() != entries.len()
        {
            return Err(EditError::Preservation);
        }
        for (expected, entry) in actual.expected().zip(&entries) {
            reader.work(entry.len())?;
            if expected != (&entry[..entry.len() - REFERENCE_BYTES], reference(entry)) {
                return Err(EditError::Preservation);
            }
        }
        prove_index(
            index,
            &entries,
            entries.len() == original_count,
            actual,
            &operation,
            proof,
            reader,
        )?;
        deltas.push((index, distinct));
    }
    counters::prove(image, checked, rows.len(), &deltas, staged, proof, reader)
}

fn prove_index(
    index: &NativeIndex,
    entries: &[Wire],
    empty: bool,
    actual: &IndexInsertion,
    operation: &Operation<'_>,
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let image = operation.image;
    let checked = operation.checked;
    let staged = operation.staged;
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
    if empty {
        if !actual.patches.is_empty()
            || !actual.added.is_empty()
            || !actual.promotions.is_empty()
            || !actual.rebases.is_empty()
        {
            return Err(EditError::Preservation);
        }
        for reservation in staged.data.append.indexes() {
            reader.tick()?;
            if reservation.physical == index.data_number() {
                return Err(EditError::Preservation);
            }
        }
        return Ok(());
    }
    let old_active = if source_leaf {
        decode(original, reader)?.1
    } else {
        0
    };
    let rendered = if source_leaf {
        render(original, entries, reader)?
    } else {
        None
    };
    let recursive = source_node || (rendered.is_none() && !split::two_halves_fit(entries, reader)?);
    if recursive {
        let change = tree::derive(
            image,
            index,
            checked.schema().definition_page(),
            entries,
            proof,
            reader,
        )?;
        change.compare(actual, proof, reader)?;
        let mut reservations = staged
            .data
            .append
            .indexes()
            .filter(|reservation| reservation.physical == index.data_number());
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
            if !actual.added.is_empty()
                || !actual.promotions.is_empty()
                || !actual.rebases.is_empty()
            {
                return Err(EditError::Preservation);
            }
            patches(
                original,
                &after,
                root.start,
                old_active.max(active),
                actual,
                empty,
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
                entries,
                proof,
                reader,
            )?;
            growth.compare(index, actual, proof, reader)?;
            let mut reservations = staged
                .data
                .append
                .indexes()
                .filter(|reservation| reservation.physical == index.data_number());
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
    Ok(())
}

fn decode(
    page: &[u8],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(Vec<Wire>, usize), EditError> {
    let (entries, active) = decode_page(page, false, reader)?;
    for pair in entries.windows(2) {
        reader.tick()?;
        if pair[0] >= pair[1] {
            return Err(EditError::Preservation);
        }
    }
    Ok((entries, active))
}

fn decode_page(
    page: &[u8],
    children: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(Vec<Wire>, usize), EditError> {
    let prefix = usize::from(u16_at(page, PREFIX_OFFSET)?);
    let maximum = MAX_WIRE_BYTES - usize::from(!children) * size_of::<u32>();
    let minimum = 1 + REFERENCE_BYTES + usize::from(children) * size_of::<u32>();
    if prefix > maximum {
        return Err(EditError::Preservation);
    }
    let mut entries: Vec<Wire> = Vec::new();
    let mut previous_end = 0;
    reader.work((ENTRY_START - BITMAP_START) * BITMAP_BITS)?;
    for endpoint in 0..(ENTRY_START - BITMAP_START) * BITMAP_BITS {
        if page[BITMAP_START + endpoint / BITMAP_BITS] & (1 << (endpoint % BITMAP_BITS)) == 0 {
            continue;
        }
        reader.tick()?;
        let shared = if entries.is_empty() { 0 } else { prefix };
        let suffix = endpoint
            .checked_sub(previous_end)
            .ok_or(EditError::Preservation)?;
        let length = shared.checked_add(suffix).ok_or(EditError::Preservation)?;
        if endpoint > PAGE_BYTES - ENTRY_START || !(minimum..=maximum).contains(&length) {
            return Err(EditError::Preservation);
        }
        reader.work(length)?;
        let mut bytes = [0; MAX_WIRE_BYTES];
        if let Some(first) = entries.first() {
            let common = first.get(..shared).ok_or(EditError::Preservation)?;
            bytes[..shared].copy_from_slice(common);
        }
        bytes[shared..length]
            .copy_from_slice(&page[ENTRY_START + previous_end..ENTRY_START + endpoint]);
        reserve(&mut entries, 1, reader)?;
        let wire = if children {
            Wire::read_branch_paid(&bytes[..length], reader)?
        } else {
            Wire::read_paid(&bytes[..length], reader)?
        };
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
    entries: &[Wire],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Option<(Vec<u8>, usize)>, EditError> {
    let prefix = usize::from(u16_at(original, PREFIX_OFFSET)?);
    let (prefix, active) = layout(entries, prefix, reader)?;
    if active > PAGE_BYTES - ENTRY_START {
        return Ok(None);
    }
    let mut after = copied(original, reader)?;
    reader.work(ENTRY_START - BITMAP_START + active)?;
    after[BITMAP_START..ENTRY_START].fill(0);
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        reader.tick()?;
        let suffix = &entry[if position == 0 { 0 } else { prefix }..];
        after[ENTRY_START + end..ENTRY_START + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        after[BITMAP_START + end / BITMAP_BITS] |= 1 << (end % BITMAP_BITS);
    }
    after[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
        .copy_from_slice(&((PAGE_BYTES - ENTRY_START - active) as u16).to_le_bytes());
    after[PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()]
        .copy_from_slice(&(prefix as u16).to_le_bytes());
    Ok(Some((after, active)))
}

fn layout(
    entries: &[Wire],
    seed: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(usize, usize), EditError> {
    let mut common = entries.first().map_or(0, |entry| entry.len());
    let mut total = 0_usize;
    for entry in entries {
        reader.work(entry.len() + 1)?;
        total = total
            .checked_add(entry.len())
            .ok_or(InspectError::StructureLimit)?;
        if let Some(first) = entries.first() {
            common = first[..common]
                .iter()
                .zip(entry)
                .take_while(|(left, right)| left == right)
                .count();
        }
    }
    let saved = entries.len().saturating_sub(1);
    let mut prefix = seed.min(common);
    if total - saved * prefix > PAGE_BYTES - ENTRY_START {
        prefix = common;
    }
    Ok((prefix, total - saved * prefix))
}

fn patches(
    original: &[u8],
    after: &[u8],
    root: usize,
    active: usize,
    actual: &IndexInsertion,
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
            let patch = actual.patches.get(count).ok_or(EditError::Preservation)?;
            if patch.offset != root + range.start
                || patch.before != original[range.clone()]
                || patch.after != after[range]
            {
                return Err(EditError::Preservation);
            }
            count += 1;
        }
    }
    if count != actual.patches.len() {
        return Err(EditError::Preservation);
    }
    Ok(())
}

fn reference(wire: &Wire) -> u32 {
    u32::from_be_bytes(
        wire[wire.len() - REFERENCE_BYTES..]
            .try_into()
            .expect("qualified ROW width"),
    )
}

fn page_range(number: u32) -> Result<Range<usize>, EditError> {
    let start = (number as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let end = start
        .checked_add(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    Ok(start..end)
}
