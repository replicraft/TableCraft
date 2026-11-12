//! Independent growing population replay, retaining the original page topology.
use super::{
    tree::{Node, Tree},
    vector, wire,
};
use crate::{EditError, InspectError, NativeImage, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const CHILD_BYTES: usize = size_of::<u32>();

struct Populations<'a> {
    original: &'a [Vec<u8>],
    final_keys: &'a [Vec<u8>],
}

struct Replay<'source, 'output, 'budget> {
    image: &'source NativeImage,
    final_keys: &'source [Vec<u8>],
    expected: &'output mut [u8],
    position: usize,
    leaves: usize,
    budget: &'budget mut ResourceBudget,
    cancelled: &'budget dyn Fn() -> bool,
}

impl Replay<'_, '_, '_> {
    fn node(&mut self, node: &Node) -> Result<usize, EditError> {
        check(self.budget, self.cancelled)?;
        let leaf = node.children.is_empty();
        let count = if leaf {
            let later = self.leaves.checked_sub(1).ok_or(EditError::Preservation)?;
            let available = self
                .final_keys
                .len()
                .checked_sub(self.position)
                .and_then(|remaining| remaining.checked_sub(later))
                .ok_or(EditError::Preservation)?;
            if later == 0 {
                available
            } else {
                node.entries.len().min(available)
            }
        } else {
            node.entries.len()
        };
        let mut entries = vector(count, self.budget, self.cancelled)?;
        let maximum = if leaf {
            // Earlier leaves retain their original count. The last leaf absorbs
            // growth, even when inserting an earlier key moves old membership.
            let end = self
                .position
                .checked_add(count)
                .filter(|end| *end <= self.final_keys.len())
                .ok_or(EditError::Preservation)?;
            if end == self.position {
                return Err(EditError::Preservation);
            }
            for key in &self.final_keys[self.position..end] {
                check(self.budget, self.cancelled)?;
                let mut entry = vector(key.len(), self.budget, self.cancelled)?;
                self.budget
                    .charge(Resource::Steps, key.len() as u64)
                    .map_err(InspectError::from)?;
                entry.extend_from_slice(key);
                entries.push(entry);
            }
            self.position = end;
            self.leaves -= 1;
            end - 1
        } else {
            let mut maximum = None;
            for (position, child) in node.children.iter().enumerate() {
                check(self.budget, self.cancelled)?;
                let child_maximum = self.node(child)?;
                if position < node.entries.len() {
                    let key = self
                        .final_keys
                        .get(child_maximum)
                        .ok_or(EditError::Preservation)?;
                    let length = key
                        .len()
                        .checked_add(CHILD_BYTES)
                        .ok_or(InspectError::StructureLimit)?;
                    let mut entry = vector(length, self.budget, self.cancelled)?;
                    self.budget
                        .charge(Resource::Steps, length as u64)
                        .map_err(InspectError::from)?;
                    entry.extend_from_slice(key);
                    entry.extend_from_slice(&child.number.to_be_bytes());
                    entries.push(entry);
                }
                maximum = Some(child_maximum);
            }
            if entries.len() != node.entries.len() {
                return Err(EditError::Preservation);
            }
            maximum.ok_or(EditError::Preservation)?
        };
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

fn leaves(
    node: &Node,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    check(budget, cancelled)?;
    if node.children.is_empty() {
        return Ok(1);
    }
    let mut total = 0_usize;
    for child in &node.children {
        total = total
            .checked_add(leaves(child, budget, cancelled)?)
            .ok_or(InspectError::StructureLimit)?;
    }
    Ok(total)
}

impl Tree {
    /// Both populations are independently derived from original raw rows and
    /// original typed INSERT requests. No producer keys or selections enter.
    pub(in crate::table) fn reconstruct_insertions(
        &self,
        image: &NativeImage,
        original: &[Vec<u8>],
        final_keys: &[Vec<u8>],
        expected: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.reconstruct_growing(
            image,
            Populations {
                original,
                final_keys,
            },
            expected,
            false,
            budget,
            cancelled,
        )
    }

    /// The retained manual INSERT language independently owns compressed leaf
    /// roots as well as existing NODE topology. No producer keys enter here.
    #[cfg(feature = "ace16-create")]
    pub(in crate::table) fn reconstruct_retained_manual_insertions(
        &self,
        image: &NativeImage,
        original: &[Vec<u8>],
        final_keys: &[Vec<u8>],
        expected: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.reconstruct_growing(
            image,
            Populations {
                original,
                final_keys,
            },
            expected,
            true,
            budget,
            cancelled,
        )
    }

    fn reconstruct_growing(
        &self,
        image: &NativeImage,
        populations: Populations<'_>,
        expected: &mut [u8],
        retained_leaf: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let Populations {
            original,
            final_keys,
        } = populations;
        if expected.len() != image.as_bytes().len()
            || (!retained_leaf && self.root.children.is_empty())
            || final_keys.len() < original.len()
        {
            return Err(EditError::Preservation);
        }
        self.original(original, budget, cancelled)?;
        if final_keys.len() == original.len() {
            for (before, after) in original.iter().zip(final_keys) {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, before.len() as u64)
                    .map_err(InspectError::from)?;
                if before != after {
                    return Err(EditError::Preservation);
                }
            }
            return Ok(());
        }
        let mut replay = Replay {
            image,
            final_keys,
            expected,
            position: 0,
            leaves: leaves(&self.root, budget, cancelled)?,
            budget,
            cancelled,
        };
        replay.node(&self.root)?;
        if replay.position != final_keys.len() || replay.leaves != 0 {
            return Err(EditError::Preservation);
        }
        check(replay.budget, cancelled)?;
        Ok(())
    }
}
