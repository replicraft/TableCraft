//! Independent cardinality-decreasing replay over the original checked graph.
use super::{
    tree::{Node, Tree},
    vector, wire,
};
use crate::{EditError, InspectError, NativeImage, WriteBlocker, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const REFERENCE_BYTES: usize = size_of::<u32>();

struct Replay<'source, 'output, 'budget> {
    image: &'source NativeImage,
    original: &'source [Vec<u8>],
    removed: &'source [u32],
    expected: &'output mut [u8],
    position: usize,
    removed_count: usize,
    budget: &'budget mut ResourceBudget,
    cancelled: &'budget dyn Fn() -> bool,
}

impl Replay<'_, '_, '_> {
    fn node(&mut self, node: &Node) -> Result<usize, EditError> {
        check(self.budget, self.cancelled)?;
        let mut entries = vector(node.entries.len(), self.budget, self.cancelled)?;
        let maximum = if node.children.is_empty() {
            // Consume the complete ORIGINAL partition. Surviving global keys
            // cannot be redistributed into another child's old positions.
            let end = self
                .position
                .checked_add(node.entries.len())
                .filter(|end| *end <= self.original.len())
                .ok_or(EditError::Preservation)?;
            let mut maximum = None;
            for position in self.position..end {
                check(self.budget, self.cancelled)?;
                let key = &self.original[position];
                let row_at = key
                    .len()
                    .checked_sub(REFERENCE_BYTES)
                    .ok_or(EditError::Preservation)?;
                let reference = u32::from_be_bytes(
                    key[row_at..]
                        .try_into()
                        .map_err(|_| EditError::Preservation)?,
                );
                self.budget
                    .charge(Resource::Steps, self.removed.len() as u64)
                    .map_err(InspectError::from)?;
                if self.removed.contains(&reference) {
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
            maximum.ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?
        } else {
            let mut maximum = None;
            for (position, child) in node.children.iter().enumerate() {
                check(self.budget, self.cancelled)?;
                let retained_maximum = self.node(child)?;
                if position < node.entries.len() {
                    let key = self
                        .original
                        .get(retained_maximum)
                        .ok_or(EditError::Preservation)?;
                    let length = key
                        .len()
                        .checked_add(REFERENCE_BYTES)
                        .ok_or(InspectError::StructureLimit)?;
                    let mut entry = vector(length, self.budget, self.cancelled)?;
                    self.budget
                        .charge(Resource::Steps, length as u64)
                        .map_err(InspectError::from)?;
                    entry.extend_from_slice(key);
                    entry.extend_from_slice(&child.number.to_be_bytes());
                    entries.push(entry);
                }
                maximum = Some(retained_maximum);
            }
            if entries.len() != node.entries.len() {
                return Err(EditError::Preservation);
            }
            maximum.ok_or(EditError::Preservation)?
        };
        // This manual encoder belongs to the independent proof owner. It is
        // also the existing UPDATE proof's real consumer, not producer repack.
        wire::write_page(
            self.image,
            node,
            &entries,
            self.expected,
            self.budget,
            self.cancelled,
        )?;
        Ok(maximum)
    }
}

impl Tree {
    /// Original keys must be freshly derived from raw records by the proof.
    /// Verify exact original membership and bounds before applying requests.
    pub(in crate::table) fn reconstruct_deletions(
        &self,
        image: &NativeImage,
        original: &[Vec<u8>],
        removed: &[u32],
        expected: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if expected.len() != image.as_bytes().len() || self.root.children.is_empty() {
            return Err(EditError::Preservation);
        }
        self.original(original, budget, cancelled)?;
        if removed.is_empty() {
            // No canonicalization of untouched valid retained upper bounds.
            check(budget, cancelled)?;
            return Ok(());
        }
        let mut replay = Replay {
            image,
            original,
            removed,
            expected,
            position: 0,
            removed_count: 0,
            budget,
            cancelled,
        };
        replay.node(&self.root)?;
        if replay.position != original.len() || replay.removed_count != removed.len() {
            return Err(EditError::Preservation);
        }
        check(replay.budget, cancelled)?;
        Ok(())
    }
}
