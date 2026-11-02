//! Actual fixed-population replay shared by independent numeric UPDATE proofs.
use super::{
    tree::{Node, Tree},
    vector,
};
use crate::{EditError, InspectError, NativeImage, format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const REFERENCE_BYTES: usize = size_of::<u32>();

struct Replay<'a, 'budget> {
    image: &'a NativeImage,
    keys: &'a [Vec<u8>],
    expected: &'a mut [u8],
    position: usize,
    budget: &'budget mut ResourceBudget,
    cancelled: &'budget dyn Fn() -> bool,
}

impl Replay<'_, '_> {
    fn node(&mut self, node: &Node) -> Result<Option<usize>, EditError> {
        check(self.budget, self.cancelled)?;
        let mut entries = vector(node.entries.len(), self.budget, self.cancelled)?;
        let maximum = if node.children.is_empty() {
            let start = self.position;
            let end = start
                .checked_add(node.entries.len())
                .filter(|end| *end <= self.keys.len())
                .ok_or(EditError::Preservation)?;
            for key in &self.keys[start..end] {
                let mut entry = vector(key.len(), self.budget, self.cancelled)?;
                self.budget
                    .charge(Resource::Steps, key.len() as u64)
                    .map_err(InspectError::from)?;
                entry.extend_from_slice(key);
                entries.push(entry);
            }
            self.position = end;
            (start != end).then(|| end - 1)
        } else {
            let mut maximum = None;
            for (position, child) in node.children.iter().enumerate() {
                check(self.budget, self.cancelled)?;
                let last = self.node(child)?.ok_or(EditError::Preservation)?;
                if position < node.entries.len() {
                    let key = self.keys.get(last).ok_or(EditError::Preservation)?;
                    let width = key
                        .len()
                        .checked_add(REFERENCE_BYTES)
                        .ok_or(InspectError::StructureLimit)?;
                    let mut entry = vector(width, self.budget, self.cancelled)?;
                    self.budget
                        .charge(Resource::Steps, width as u64)
                        .map_err(InspectError::from)?;
                    entry.extend_from_slice(key);
                    entry.extend_from_slice(&child.number.to_be_bytes());
                    entries.push(entry);
                }
                maximum = Some(last);
            }
            maximum
        };
        if entries.len() != node.entries.len() {
            return Err(EditError::Preservation);
        }

        super::wire::write_page(
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

/// Caller has independently validated original membership and final constraints.
/// This consumes raw original/final populations, never producer patches or keys.
pub(in crate::table) fn reconstruct(
    image: &NativeImage,
    tree: &Tree,
    original: &[Vec<u8>],
    final_keys: &[Vec<u8>],
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut work = original
        .len()
        .checked_mul(size_of::<Vec<u8>>())
        .ok_or(InspectError::StructureLimit)?;
    for key in original {
        work = work
            .checked_add(key.len())
            .ok_or(InspectError::StructureLimit)?;
    }
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    if original == final_keys {
        // Preserve all retained original upper bounds when the whole physical
        // index is unchanged. Affected indexes alone select fitting repacking.
        for page in &tree.pages {
            check(budget, cancelled)?;
            let start = (*page as usize)
                .checked_mul(PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            let end = start
                .checked_add(PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            budget
                .charge(Resource::Steps, PAGE_BYTES as u64)
                .map_err(InspectError::from)?;
            if expected.get(start..end) != image.as_bytes().get(start..end) {
                return Err(EditError::Preservation);
            }
        }
        check(budget, cancelled)?;
        return Ok(());
    }
    let mut replay = Replay {
        image,
        keys: final_keys,
        expected,
        position: 0,
        budget,
        cancelled,
    };
    replay.node(&tree.root)?;
    if replay.position != final_keys.len() {
        return Err(EditError::Preservation);
    }
    check(replay.budget, cancelled)?;
    Ok(())
}
