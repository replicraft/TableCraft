//! Independent original-partition pruning and all-level link reconstruction.
use super::super::super::{
    tree::{Node, Tree},
    vector, wire,
};
use super::{ASCENDING_PRESENT, PRESENT_I32_BYTES, ROW_BYTES, reference};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{bytes::u32_at, page::PAGE_BYTES},
    header::check,
    table::indexes::{
        header::{IndexHeader, LEVEL_OFFSET},
        page::LEAF,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const TAIL_AT: usize = std::mem::offset_of!(IndexHeader, tail);
const PREVIOUS_AT: usize = std::mem::offset_of!(IndexHeader, previous);
const NEXT_AT: usize = std::mem::offset_of!(IndexHeader, next);
const LEVELS: usize = 3;

struct Replay<'source, 'output, 'budget> {
    image: &'source NativeImage,
    root: u32,
    original: &'source [Vec<u8>],
    removed: &'source [u32],
    expected: &'output mut [u8],
    position: usize,
    removed_count: usize,
    retired: Vec<u32>,
    levels: [Vec<u32>; LEVELS],
    linked: [bool; LEVELS],
    budget: &'budget mut ResourceBudget,
    cancelled: &'budget dyn Fn() -> bool,
}

impl Replay<'_, '_, '_> {
    fn node(&mut self, node: &Node) -> Result<Option<usize>, EditError> {
        check(self.budget, self.cancelled)?;
        if node.height != 0 {
            let base = node.number as usize * PAGE_BYTES;
            self.budget
                .charge(Resource::Steps, (2 * ROW_BYTES) as u64)
                .map_err(InspectError::from)?;
            let linked = u32_at(self.image.as_bytes(), base + PREVIOUS_AT)? != 0
                || u32_at(self.image.as_bytes(), base + NEXT_AT)? != 0;
            *self
                .linked
                .get_mut(usize::from(node.height))
                .ok_or(EditError::Preservation)? |= linked;
        }
        let mut entries = vector(
            node.entries.len().max(node.children.len()),
            self.budget,
            self.cancelled,
        )?;
        let mut retained_tail = None;
        let maximum = if node.children.is_empty() {
            let end = self
                .position
                .checked_add(node.entries.len())
                .filter(|end| *end <= self.original.len())
                .ok_or(EditError::Preservation)?;
            let mut maximum = None;
            for position in self.position..end {
                check(self.budget, self.cancelled)?;
                let key = &self.original[position];
                self.budget
                    .charge(Resource::Steps, self.removed.len() as u64)
                    .map_err(InspectError::from)?;
                if self.removed.contains(&reference(key)?) {
                    self.removed_count = self
                        .removed_count
                        .checked_add(1)
                        .ok_or(InspectError::StructureLimit)?;
                    continue;
                }
                let mut entry = vector(key.len(), self.budget, self.cancelled)?;
                self.budget
                    .charge(Resource::Steps, key.len() as u64)
                    .map_err(InspectError::from)?;
                entry.extend_from_slice(key);
                entries.push(entry);
                maximum = Some(position);
            }
            self.position = end;
            maximum
        } else {
            let mut kept = vector(node.children.len(), self.budget, self.cancelled)?;
            for child in &node.children {
                if let Some(maximum) = self.node(child)? {
                    kept.push((child.number, maximum));
                }
            }
            let base = node.number as usize * PAGE_BYTES;
            let tail_form = u32_at(self.image.as_bytes(), base + TAIL_AT)? != 0;
            let tail = if tail_form && kept.len() > 1 {
                kept.last().ok_or(EditError::Preservation)?.0
            } else {
                0
            };
            for &(child, maximum) in &kept {
                check(self.budget, self.cancelled)?;
                if child == tail {
                    continue;
                }
                let key = self.original.get(maximum).ok_or(EditError::Preservation)?;
                let length = key
                    .len()
                    .checked_add(ROW_BYTES)
                    .ok_or(InspectError::StructureLimit)?;
                let mut entry = vector(length, self.budget, self.cancelled)?;
                self.budget
                    .charge(Resource::Steps, length as u64)
                    .map_err(InspectError::from)?;
                entry.extend_from_slice(key);
                entry.extend_from_slice(&child.to_be_bytes());
                entries.push(entry);
            }
            if !kept.is_empty() {
                retained_tail = Some(tail);
            }
            kept.last().map(|&(_, maximum)| maximum)
        };
        if maximum.is_none() {
            if node.number != self.root {
                self.retired.push(node.number);
                return Ok(None);
            }
            // Only a wholly empty root demotes. No nonempty child promotion.
            wire::write_page(
                self.image,
                node,
                &entries,
                self.expected,
                self.budget,
                self.cancelled,
            )?;
            let base = node.number as usize * PAGE_BYTES;
            self.budget
                .charge(Resource::Steps, (ROW_BYTES + 2 * size_of::<u8>()) as u64)
                .map_err(InspectError::from)?;
            self.expected[base + std::mem::offset_of!(IndexHeader, kind)] = LEAF;
            self.expected[base + LEVEL_OFFSET] = 0;
            self.expected[base + TAIL_AT..base + TAIL_AT + ROW_BYTES].fill(0);
            self.levels[0].push(node.number);
            return Ok(None);
        }

        wire::write_page(
            self.image,
            node,
            &entries,
            self.expected,
            self.budget,
            self.cancelled,
        )?;
        if let Some(tail) = retained_tail {
            let base = node.number as usize * PAGE_BYTES;
            self.budget
                .charge(Resource::Steps, ROW_BYTES as u64)
                .map_err(InspectError::from)?;
            self.expected[base + TAIL_AT..base + TAIL_AT + ROW_BYTES]
                .copy_from_slice(&tail.to_le_bytes());
        }
        self.levels
            .get_mut(usize::from(node.height))
            .ok_or(EditError::Preservation)?
            .push(node.number);
        Ok(maximum)
    }
}

fn qualify(
    tree: &Tree,
    image: &NativeImage,
    original: &[Vec<u8>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let root = &tree.root;
    budget
        .charge(
            Resource::Steps,
            (original.len() + root.children.len()) as u64,
        )
        .map_err(InspectError::from)?;
    if root.height != 2
        || !(1..=2).contains(&root.children.len())
        || root.entries.len() != root.children.len()
        || root.prefix != 0
        || u32_at(
            image.as_bytes(),
            root.number as usize * PAGE_BYTES + TAIL_AT,
        )? != 0
        || original
            .iter()
            .any(|key| key.len() != PRESENT_I32_BYTES || key[0] != ASCENDING_PRESENT)
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    for parent in &root.children {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, parent.children.len() as u64)
            .map_err(InspectError::from)?;
        if parent.height != 1
            || parent.prefix > PRESENT_I32_BYTES
            || parent.children.is_empty()
            || parent.entries.len() + usize::from(parent.children.len() > 1)
                != parent.children.len()
            || u32_at(
                image.as_bytes(),
                parent.number as usize * PAGE_BYTES + TAIL_AT,
            )? != if parent.children.len() == 1 {
                0
            } else {
                parent
                    .children
                    .last()
                    .ok_or(EditError::Preservation)?
                    .number
            }
            || parent.children.iter().any(|child| {
                child.height != 0 || !child.children.is_empty() || child.entries.is_empty()
            })
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    Ok(())
}

pub(super) fn reconstruct(
    tree: &Tree,
    image: &NativeImage,
    original: &[Vec<u8>],
    removed: &[u32],
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u32>, EditError> {
    check(budget, cancelled)?;
    if expected.len() != image.as_bytes().len() {
        return Err(EditError::Preservation);
    }
    tree.original(original, budget, cancelled)?;
    qualify(tree, image, original, budget, cancelled)?;
    if removed.len() > original.len()
        || removed.len() == original.len() && tree.root.children.len() != 1
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }

    let retired = vector(tree.pages.len(), budget, cancelled)?;
    let levels = [
        vector(tree.pages.len(), budget, cancelled)?,
        vector(tree.pages.len(), budget, cancelled)?,
        vector(tree.pages.len(), budget, cancelled)?,
    ];
    let mut replay = Replay {
        image,
        root: tree.root.number,
        original,
        removed,
        expected,
        position: 0,
        removed_count: 0,
        retired,
        levels,
        linked: [true, false, false],
        budget,
        cancelled,
    };
    let maximum = replay.node(&tree.root)?;
    replay
        .budget
        .charge(Resource::Steps, replay.retired.len() as u64)
        .map_err(InspectError::from)?;
    if maximum.is_none() != (removed.len() == original.len())
        || replay.position != original.len()
        || replay.removed_count != removed.len()
        || replay.retired.is_empty()
        || replay.retired.contains(&tree.root.number)
    {
        return Err(EditError::Preservation);
    }
    for (height, level) in replay.levels.iter().enumerate() {
        check(replay.budget, cancelled)?;
        if !replay.linked[height] {
            continue;
        }
        for (position, &number) in level.iter().enumerate() {
            check(replay.budget, cancelled)?;
            let previous = position.checked_sub(1).map_or(0, |at| level[at]);
            let next = level.get(position + 1).copied().unwrap_or(0);
            let base = number as usize * PAGE_BYTES;
            replay
                .budget
                .charge(Resource::Steps, (2 * ROW_BYTES) as u64)
                .map_err(InspectError::from)?;
            replay.expected[base + PREVIOUS_AT..base + PREVIOUS_AT + ROW_BYTES]
                .copy_from_slice(&previous.to_le_bytes());
            replay.expected[base + NEXT_AT..base + NEXT_AT + ROW_BYTES]
                .copy_from_slice(&next.to_le_bytes());
        }
    }
    check(replay.budget, cancelled)?;
    Ok(replay.retired)
}
