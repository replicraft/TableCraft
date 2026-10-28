//! Resolve independently normalized symbols, replay maps and frame exact pages.
#[cfg(test)]
mod tests;

use super::super::{WIRE_BYTES, page_range, split::maps::Allocation};
use super::{
    Change, ImageProof, frames,
    normalize::{Normalized, Origin},
    raw, reserve,
    source::Source,
};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex,
    format::{page::PAGE_BYTES, reader::Reader},
    table::indexes::mutation::insertion::PrimaryInsertion,
};
use std::io::Cursor;

pub(super) fn derive(
    image: &NativeImage,
    index: &NativeIndex,
    source: &Source,
    entries: &[[u8; WIRE_BYTES]],
    normalized: &Normalized,
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Change, EditError> {
    let mut maxima = Vec::new();
    reserve(&mut maxima, normalized.logical.len(), reader)?;
    for position in &normalized.logical {
        reader.tick()?;
        let node = normalized
            .nodes
            .get(*position)
            .ok_or(EditError::Preservation)?;
        if node.range.start >= node.range.end {
            return Err(EditError::Preservation);
        }
        maxima.push(
            *entries
                .get(node.range.end - 1)
                .ok_or(EditError::Preservation)?,
        );
    }
    let bounds = maxima
        .len()
        .checked_sub(usize::from(source.tail))
        .ok_or(EditError::Preservation)?;
    if !frames::parent(&maxima[..bounds], source.prefix, reader)?.fits() {
        return Err(EditError::Preservation);
    }

    let mut pages = Vec::new();
    reserve(&mut pages, source.children.len() + 1, reader)?;
    pages.push(source.root);
    pages.extend(source.children.iter().map(|child| child.number));
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
        Origin::Existing { number, .. } => Some(number),
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

    let mut logical_ids = Vec::new();
    reserve(&mut logical_ids, normalized.logical.len(), reader)?;
    for index in &normalized.logical {
        reader.tick()?;
        logical_ids.push(
            ids.get(*index)
                .and_then(|number| *number)
                .ok_or(EditError::Preservation)?,
        );
    }
    let tail = if source.tail {
        let actual = logical_ids.last().copied().ok_or(EditError::Preservation)?;
        let original = source
            .children
            .last()
            .ok_or(EditError::Preservation)?
            .number;
        if actual != original {
            return Err(EditError::Preservation);
        }
        actual
    } else {
        0
    };
    let root = frames::root(
        raw(image, source.root)?,
        &maxima[..bounds],
        &logical_ids[..bounds],
        tail,
        reader,
    )?;
    reader.work(PAGE_BYTES)?;
    proof.bytes[page_range(source.root)?].copy_from_slice(&root);
    for (position, index) in normalized.logical.iter().enumerate() {
        let node = normalized
            .nodes
            .get(*index)
            .ok_or(EditError::Preservation)?;
        let number = logical_ids[position];
        let previous = position
            .checked_sub(1)
            .map_or(0, |offset| logical_ids[offset]);
        let next = logical_ids.get(position + 1).copied().unwrap_or(0);
        let before = match node.origin {
            Origin::Existing { number, .. } => Some(raw(image, number)?),
            Origin::Fresh => None,
        };
        let after = frames::leaf(
            before,
            source.definition,
            (previous, next),
            &entries[node.range.clone()],
            reader,
        )?;
        reader.work(PAGE_BYTES)?;
        proof.bytes[page_range(number)?].copy_from_slice(&after);
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

pub(super) fn compare(
    change: &Change,
    actual: &PrimaryInsertion,
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
