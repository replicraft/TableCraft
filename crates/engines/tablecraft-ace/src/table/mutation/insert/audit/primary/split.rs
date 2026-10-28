//! Independent two-leaf INSERT growth; source/request wires supply authority.
mod framing;
pub(in crate::table::mutation::insert::audit::primary) mod maps;
#[cfg(test)]
mod tests;

use super::super::{ImageProof, copied, reserve};
use super::{INDEX_VERSION, WIRE_BYTES, decode, reference, value};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex,
    format::{page::PAGE_BYTES, reader::Reader},
    table::indexes::{header::IndexHeader, mutation::insertion::PrimaryInsertion, page::LEAF},
};
use std::io::Cursor;

pub(super) struct PromotionExpected {
    pub number: u32,
    pub offset: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub bitmap: Vec<u8>,
}

pub(super) struct Growth {
    pub begin: u32,
    pub right: u32,
    pub left: u32,
    pub promoted: Option<PromotionExpected>,
    pub rebased: Option<u32>,
}

pub(super) fn derive(
    image: &NativeImage,
    index: &NativeIndex,
    owner: u32,
    entries: &[[u8; WIRE_BYTES]],
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Growth, EditError> {
    reader.tick()?;
    if !std::ptr::eq(proof.original, image.as_bytes())
        || proof.bytes.len() < proof.original.len()
        || !proof.bytes.len().is_multiple_of(PAGE_BYTES)
    {
        return Err(EditError::Preservation);
    }
    let range = super::page_range(index.root_page())?;
    let original = image
        .as_bytes()
        .get(range.clone())
        .ok_or(EditError::Preservation)?;
    let header = IndexHeader::read(original)?;
    if header.kind != LEAF
        || header.version != INDEX_VERSION
        || header.owner.get() != owner
        || header.previous.get() != 0
        || header.next.get() != 0
        || header.tail.get() != 0
        || !header.qualified_for_update(0)
    {
        return Err(EditError::Preservation);
    }
    let (before, _) = decode(original, reader)?;
    framing::membership(&before, entries, reader)?;
    let middle = entries.len() / 2;
    if middle == 0 || middle == entries.len() {
        return Err(EditError::Preservation);
    }
    let retained = framing::measure(entries, usize::from(header.prefix.get()), reader)?;
    let left_layout = framing::measure(&entries[..middle], 0, reader)?;
    let right_layout = framing::measure(&entries[middle..], 0, reader)?;
    if retained.fits() || !left_layout.fits() || !right_layout.fits() {
        return Err(EditError::Preservation);
    }

    // The incoming expected image and ledger are independently certified DATA,
    // not a cloned builder cursor or selected append events.
    let begin =
        u32::try_from(proof.bytes.len() / PAGE_BYTES).map_err(|_| InspectError::StructureLimit)?;
    let mut allocation = maps::Allocation::capture(image, index, owner, proof, reader)?;
    let right = allocation.reserve(proof, reader)?;
    let left = allocation.reserve(proof, reader)?;
    let right_page = framing::leaf(owner, left, 0, &entries[middle..], reader)?;
    let left_page = framing::leaf(owner, 0, right, &entries[..middle], reader)?;
    let root = framing::root(original, left, right, &entries[middle - 1], reader)?;
    reader.work(3 * PAGE_BYTES)?;
    proof.bytes[super::page_range(right)?].copy_from_slice(&right_page);
    proof.bytes[super::page_range(left)?].copy_from_slice(&left_page);
    proof.bytes[range].copy_from_slice(&root);
    let (promoted, rebased) = allocation.finish(proof, reader)?;

    Ok(Growth {
        begin,
        right,
        left,
        promoted,
        rebased,
    })
}

impl Growth {
    pub(super) fn compare(
        &self,
        index: &NativeIndex,
        actual: &PrimaryInsertion,
        proof: &ImageProof<'_>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        let extra = u32::from(self.promoted.is_some());
        let following = self.begin.checked_add(1).ok_or(EditError::Preservation)?;
        let end = self
            .begin
            .checked_add(2 + extra)
            .ok_or(EditError::Preservation)?;
        let left = match &self.promoted {
            None => following,
            Some(promotion) if promotion.number == following => {
                following.checked_add(1).ok_or(EditError::Preservation)?
            }
            Some(promotion)
                if promotion.number
                    == following.checked_add(1).ok_or(EditError::Preservation)? =>
            {
                following
            }
            Some(_) => return Err(EditError::Preservation),
        };
        if self.right != self.begin
            || self.left != left
            || proof.bytes.len() / PAGE_BYTES != end as usize
        {
            return Err(EditError::Preservation);
        }

        let root = super::page_range(index.root_page())?;
        if actual.patches.len() != 1 {
            return Err(EditError::Preservation);
        }
        let patch = &actual.patches[0];
        reader.work(PAGE_BYTES)?;
        if patch.offset != root.start
            || patch.before != proof.original[root.clone()]
            || patch.after != proof.bytes[root]
        {
            return Err(EditError::Preservation);
        }
        let extra = usize::from(self.promoted.is_some());
        if actual.added.len() != 2 + extra || actual.promotions.len() != extra {
            return Err(EditError::Preservation);
        }
        let mut expected = Vec::new();
        reserve(&mut expected, 2 + extra, reader)?;
        expected.extend([self.right, self.left]);
        if let Some(promotion) = &self.promoted {
            expected.push(promotion.number);
        }
        expected.sort_unstable();
        for (page, number) in actual.added.iter().zip(expected) {
            reader.work(PAGE_BYTES)?;
            if page.number != number || page.bytes != proof.bytes[super::page_range(number)?] {
                return Err(EditError::Preservation);
            }
        }
        if let Some(promotion) = &self.promoted {
            let actual = &actual.promotions[0];
            reader.work(PAGE_BYTES + 2 * promotion.before.len())?;
            if actual.usage != index.usage_map()
                || actual.number != promotion.number
                || actual.offset != promotion.offset
                || actual.before != promotion.before
                || actual.after != promotion.after
                || actual.bitmap != promotion.bitmap
            {
                return Err(EditError::Preservation);
            }
        }
        match self.rebased {
            Some(base) => {
                if actual.rebases.len() != 1 {
                    return Err(EditError::Preservation);
                }
                let actual = &actual.rebases[0];
                if actual.usage != index.usage_map()
                    || actual.physical != index.data_number()
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
}
