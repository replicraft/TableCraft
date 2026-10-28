//! Resolve independently normalized symbols, replay maps and frame exact pages.

use super::super::{Wire, page_range, split::maps::Allocation};
use super::{
    Change, ImageProof, frames,
    normalize::{Normalized, Origin, Payload},
    raw, reserve,
    source::Source,
};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex,
    format::{page::PAGE_BYTES, reader::Reader},
    table::indexes::mutation::insertion::IndexInsertion,
};
use std::io::Cursor;

pub(super) fn derive(
    image: &NativeImage,
    index: &NativeIndex,
    source: &Source,
    entries: &[Wire],
    normalized: &Normalized,
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Change, EditError> {
    let pages = super::copied_numbers(&source.pages, reader)?;
    for number in &pages {
        let range = page_range(*number)?;
        reader.work(PAGE_BYTES)?;
        if proof.bytes.get(range.clone()) != image.as_bytes().get(range) {
            return Err(EditError::Preservation);
        }
    }
    let begin =
        u32::try_from(proof.bytes.len() / PAGE_BYTES).map_err(|_| InspectError::StructureLimit)?;
    let mut ids = Vec::new();
    reserve(&mut ids, normalized.nodes.len(), reader)?;
    ids.extend(normalized.nodes.iter().map(|node| match node.origin {
        Origin::Existing { number, .. } | Origin::GrownRoot { number } => Some(number),
        Origin::Fresh => None,
    }));
    let mut indexes = Vec::new();
    reserve(&mut indexes, normalized.fresh.len(), reader)?;
    let (promoted, rebased) = if normalized.fresh.is_empty() {
        (None, None)
    } else {
        let mut allocation = Allocation::capture(image, index, source.definition, proof, reader)?;
        for position in &normalized.fresh {
            reader.tick()?;
            let slot = ids.get_mut(*position).ok_or(EditError::Preservation)?;
            if slot.is_some() {
                return Err(EditError::Preservation);
            }
            let number = allocation.reserve(proof, reader)?;
            *slot = Some(number);
            indexes.push(number);
        }
        allocation.finish(proof, reader)?
    };

    let mut levels = Vec::new();
    let mut visited = Vec::new();
    collect(
        normalized.root,
        normalized,
        &mut levels,
        &mut visited,
        reader,
    )?;
    if visited.len() != normalized.nodes.len()
        || visited.len() != source.pages.len() + normalized.fresh.len()
    {
        return Err(EditError::Preservation);
    }
    for level in levels {
        for (position, symbol) in level.iter().enumerate() {
            reader.tick()?;
            let node = &normalized.nodes[*symbol];
            let number = ids[*symbol].ok_or(EditError::Preservation)?;
            let previous = position
                .checked_sub(1)
                .map(|i| ids[level[i]])
                .unwrap_or(Some(0))
                .ok_or(EditError::Preservation)?;
            let next = level
                .get(position + 1)
                .map(|i| ids[*i])
                .unwrap_or(Some(0))
                .ok_or(EditError::Preservation)?;
            let (before, reset) = match node.origin {
                Origin::Existing { number, .. } => (Some(raw(image, number)?), false),
                Origin::GrownRoot { number } => (Some(raw(image, number)?), true),
                Origin::Fresh => (None, true),
            };
            let seed = frames::seed(
                before,
                source.definition,
                node.height,
                (previous, next),
                reset,
                reader,
            )?;
            let after = match &node.payload {
                Payload::Leaf(range) => frames::leaf(
                    Some(&seed),
                    source.definition,
                    (previous, next),
                    &entries[range.clone()],
                    reader,
                )?,
                Payload::Branch { children, tail } => {
                    let count = children
                        .len()
                        .checked_sub(usize::from(*tail))
                        .ok_or(EditError::Preservation)?;
                    let mut maxima = Vec::new();
                    let mut numbers = Vec::new();
                    reserve(&mut maxima, count, reader)?;
                    reserve(&mut numbers, count, reader)?;
                    for child in &children[..count] {
                        reader.tick()?;
                        let maximum = normalized.nodes[*child]
                            .maximum
                            .ok_or(EditError::Preservation)?;
                        maxima.push(
                            entries
                                .get(maximum)
                                .ok_or(EditError::Preservation)?
                                .copy_paid(reader)?,
                        );
                        numbers.push(ids[*child].ok_or(EditError::Preservation)?);
                    }
                    let tail_number = if *tail {
                        ids[*children.last().ok_or(EditError::Preservation)?]
                            .ok_or(EditError::Preservation)?
                    } else {
                        0
                    };
                    frames::branch(&seed, &maxima, &numbers, tail_number, reader)?
                }
            };
            reader.work(PAGE_BYTES)?;
            proof.bytes[page_range(number)?].copy_from_slice(&after);
        }
    }

    Ok(Change {
        pages,
        indexes,
        begin,
        physical: index.data_number(),
        usage: index.usage_map(),
        promoted,
        rebased,
    })
}

fn collect(
    index: usize,
    normalized: &Normalized,
    levels: &mut Vec<Vec<usize>>,
    visited: &mut Vec<usize>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(visited.len() + 1)?;
    if visited.contains(&index) {
        return Err(EditError::Preservation);
    }
    reserve(visited, 1, reader)?;
    visited.push(index);
    let node = normalized.nodes.get(index).ok_or(EditError::Preservation)?;
    while levels.len() <= usize::from(node.height) {
        reserve(levels, 1, reader)?;
        levels.push(Vec::new());
    }
    let level = &mut levels[usize::from(node.height)];
    reserve(level, 1, reader)?;
    level.push(index);
    if let Payload::Branch { children, .. } = &node.payload {
        for child in children {
            if normalized
                .nodes
                .get(*child)
                .is_none_or(|child| child.height + 1 != node.height)
            {
                return Err(EditError::Preservation);
            }
            collect(*child, normalized, levels, visited, reader)?;
        }
    }
    Ok(())
}

pub(super) fn compare(
    change: &Change,
    actual: &IndexInsertion,
    proof: &ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let extra = usize::from(change.promoted.is_some());
    let added = change
        .indexes
        .len()
        .checked_add(extra)
        .ok_or(EditError::Preservation)?;
    let end = change
        .begin
        .checked_add(u32::try_from(added).map_err(|_| InspectError::StructureLimit)?)
        .ok_or(EditError::Preservation)?;
    if proof.bytes.len() / PAGE_BYTES != end as usize
        || actual.added.len() != added
        || actual.promotions.len() != extra
    {
        return Err(EditError::Preservation);
    }
    let mut next = change.begin;
    for number in &change.indexes {
        reader.tick()?;
        if change
            .promoted
            .as_ref()
            .is_some_and(|promotion| promotion.number == next)
        {
            next = next.checked_add(1).ok_or(EditError::Preservation)?;
        }
        if *number != next {
            return Err(EditError::Preservation);
        }
        next = next.checked_add(1).ok_or(EditError::Preservation)?;
    }
    if let Some(promotion) = &change.promoted
        && promotion.number == next
    {
        next = next.checked_add(1).ok_or(EditError::Preservation)?;
    }
    if next != end {
        return Err(EditError::Preservation);
    }

    let mut position = 0;
    for number in &change.pages {
        let range = page_range(*number)?;
        let before = proof
            .original
            .get(range.clone())
            .ok_or(EditError::Preservation)?;
        let after = proof
            .bytes
            .get(range.clone())
            .ok_or(EditError::Preservation)?;
        reader.work(PAGE_BYTES)?;
        if before == after {
            continue;
        }
        let patch = actual
            .patches
            .get(position)
            .ok_or(EditError::Preservation)?;
        if patch.offset != range.start || patch.before != before || patch.after != after {
            return Err(EditError::Preservation);
        }
        position += 1;
    }
    if position != actual.patches.len() {
        return Err(EditError::Preservation);
    }
    for (offset, page) in actual.added.iter().enumerate() {
        let number = change
            .begin
            .checked_add(u32::try_from(offset).map_err(|_| InspectError::StructureLimit)?)
            .ok_or(EditError::Preservation)?;
        reader.work(PAGE_BYTES)?;
        if page.number != number || page.bytes != proof.bytes[page_range(number)?] {
            return Err(EditError::Preservation);
        }
    }
    if let Some(promotion) = &change.promoted {
        let actual = &actual.promotions[0];
        reader.work(PAGE_BYTES + 2 * promotion.before.len())?;
        if actual.usage != change.usage
            || actual.number != promotion.number
            || actual.offset != promotion.offset
            || actual.before != promotion.before
            || actual.after != promotion.after
            || actual.bitmap != promotion.bitmap
        {
            return Err(EditError::Preservation);
        }
    }
    match change.rebased {
        Some(base) => {
            if actual.rebases.len() != 1 {
                return Err(EditError::Preservation);
            }
            let actual = &actual.rebases[0];
            if actual.usage != change.usage
                || actual.physical != change.physical
                || actual.base != base
            {
                return Err(EditError::Preservation);
            }
        }
        None if !actual.rebases.is_empty() => return Err(EditError::Preservation),
        None => {}
    }

    Ok(())
}
