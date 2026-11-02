//! Independent original-partition pruning for the qualified height-one profile.
use super::super::{
    tree::{Node, Tree},
    vector, wire,
};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{bytes::u32_at, page::PAGE_BYTES},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const ROW_BYTES: usize = size_of::<u32>();
const PRESENT_I32_BYTES: usize = size_of::<u8>() + size_of::<i32>() + ROW_BYTES;
const TAIL_AT: usize = 20;
const PREVIOUS_AT: usize = 12;
const NEXT_AT: usize = 16;
const LEVEL_AT: usize = 26;
const LEAF_KIND: u8 = 4;
const ASCENDING_PRESENT: u8 = 127;

fn reference(key: &[u8]) -> Result<u32, EditError> {
    let at = key
        .len()
        .checked_sub(ROW_BYTES)
        .ok_or(EditError::Preservation)?;
    Ok(u32::from_be_bytes(
        key[at..].try_into().map_err(|_| EditError::Preservation)?,
    ))
}

fn empty_child(
    node: &Node,
    original: &[Vec<u8>],
    removed: &[u32],
    position: &mut usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    if node.children.is_empty() {
        let end = position
            .checked_add(node.entries.len())
            .filter(|end| *end <= original.len())
            .ok_or(EditError::Preservation)?;
        let mut empty = true;
        for key in &original[*position..end] {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, removed.len() as u64)
                .map_err(InspectError::from)?;
            empty &= removed.contains(&reference(key)?);
        }
        *position = end;
        return Ok(empty);
    }
    let mut needed = false;
    for child in &node.children {
        // All original partitions must be consumed, even after one is empty.
        needed |= empty_child(child, original, removed, position, budget, cancelled)?;
    }
    Ok(needed)
}

impl Tree {
    /// Select only from original membership; this grants no staging authority.
    pub(in crate::table) fn retirement_required(
        &self,
        original: &[Vec<u8>],
        removed: &[u32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        if removed.is_empty() {
            return Ok(false);
        }
        let mut position = 0;
        let needed = empty_child(
            &self.root,
            original,
            removed,
            &mut position,
            budget,
            cancelled,
        )?;
        if position != original.len() {
            return Err(EditError::Preservation);
        }
        Ok(needed)
    }

    pub(in crate::table) fn reconstruct_retirements(
        &self,
        image: &NativeImage,
        original: &[Vec<u8>],
        removed: &[u32],
        expected: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u32>, EditError> {
        check(budget, cancelled)?;
        self.original(original, budget, cancelled)?;
        let root = &self.root;
        let base = root.number as usize * PAGE_BYTES;
        let raw = image
            .as_bytes()
            .get(base..base + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        budget
            .charge(
                Resource::Steps,
                (original.len() + root.children.len()) as u64,
            )
            .map_err(InspectError::from)?;
        if expected.len() != image.as_bytes().len()
            || root.height != 1
            || root.children.len() != 2
            || root.entries.len() != 1
            || root.prefix != 0
            || root
                .children
                .iter()
                .any(|child| !child.children.is_empty() || child.entries.is_empty())
            || u32_at(raw, TAIL_AT)? != root.children[1].number
            || original
                .iter()
                .any(|key| key.len() != PRESENT_I32_BYTES || key[0] != ASCENDING_PRESENT)
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let mut retired = vector(root.children.len(), budget, cancelled)?;
        let mut kept = vector(root.children.len(), budget, cancelled)?;
        let mut maxima = vector(root.children.len(), budget, cancelled)?;
        let mut position = 0_usize;
        let mut removed_count = 0_usize;
        for child in &root.children {
            check(budget, cancelled)?;
            let end = position
                .checked_add(child.entries.len())
                .filter(|end| *end <= original.len())
                .ok_or(EditError::Preservation)?;
            let mut entries = vector(child.entries.len(), budget, cancelled)?;
            let mut maximum = None;
            for (ordinal, key) in original.iter().enumerate().take(end).skip(position) {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, removed.len() as u64)
                    .map_err(InspectError::from)?;
                if removed.contains(&reference(key)?) {
                    removed_count += 1;
                    continue;
                }
                let mut entry = vector(key.len(), budget, cancelled)?;
                budget
                    .charge(Resource::Steps, key.len() as u64)
                    .map_err(InspectError::from)?;
                entry.extend_from_slice(key);
                entries.push(entry);
                maximum = Some(ordinal);
            }
            position = end;
            if let Some(maximum) = maximum {
                wire::write_page(image, child, &entries, expected, budget, cancelled)?;
                kept.push(child.number);
                maxima.push(maximum);
            } else {
                retired.push(child.number);
            }
        }
        if position != original.len() || removed_count != removed.len() || retired.is_empty() {
            return Err(EditError::Preservation);
        }
        let mut bounds = vector(kept.len(), budget, cancelled)?;
        for (position, (&child, &maximum)) in kept.iter().zip(&maxima).enumerate() {
            check(budget, cancelled)?;
            if kept.len() > 1 && position + 1 == kept.len() {
                continue;
            }
            let key = &original[maximum];
            let width = key
                .len()
                .checked_add(ROW_BYTES)
                .ok_or(InspectError::StructureLimit)?;
            let mut bound = vector(width, budget, cancelled)?;
            budget
                .charge(Resource::Steps, width as u64)
                .map_err(InspectError::from)?;
            bound.extend_from_slice(key);
            bound.extend_from_slice(&child.to_be_bytes());
            bounds.push(bound);
        }
        wire::write_page(image, root, &bounds, expected, budget, cancelled)?;
        let tail = if kept.len() > 1 {
            *kept.last().ok_or(EditError::Preservation)?
        } else {
            0
        };
        budget
            .charge(Resource::Steps, (size_of::<u32>() + 2) as u64)
            .map_err(InspectError::from)?;
        expected[base + TAIL_AT..base + TAIL_AT + ROW_BYTES].copy_from_slice(&tail.to_le_bytes());
        if kept.is_empty() {
            expected[base] = LEAF_KIND;
            expected[base + LEVEL_AT] = 0;
        }
        for (position, &number) in kept.iter().enumerate() {
            check(budget, cancelled)?;
            let previous = position.checked_sub(1).map_or(0, |index| kept[index]);
            let next = kept.get(position + 1).copied().unwrap_or(0);
            let start = number as usize * PAGE_BYTES;
            budget
                .charge(Resource::Steps, (2 * ROW_BYTES) as u64)
                .map_err(InspectError::from)?;
            expected[start + PREVIOUS_AT..start + PREVIOUS_AT + ROW_BYTES]
                .copy_from_slice(&previous.to_le_bytes());
            expected[start + NEXT_AT..start + NEXT_AT + ROW_BYTES]
                .copy_from_slice(&next.to_le_bytes());
        }
        check(budget, cancelled)?;
        Ok(retired)
    }
}
