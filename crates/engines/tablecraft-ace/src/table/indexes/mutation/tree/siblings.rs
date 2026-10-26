//! Contraction qualifies complete original internal chains, then repairs survivors.
mod audit;
pub(in crate::table) use audit::audit_siblings;

use super::{Tree, raw_page};
use crate::{
    EditError, InspectError, NativeImage,
    header::check,
    table::indexes::{
        NativeIndex,
        header::IndexHeader,
        mutation::{
            IndexDataPatch,
            retirement::{RetiredPage, header_patch},
        },
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

struct Node {
    number: u32,
    previous: u32,
    next: u32,
}

struct Level {
    nodes: Vec<Node>,
    linked: bool,
}

pub(in crate::table) struct SiblingAudit {
    root: u32,
    physical: u32,
    levels: Vec<Level>,
    pages: Vec<u32>,
    empty_root: bool,
}

impl Tree {
    pub(in crate::table::indexes::mutation) fn sibling_chains(
        &self,
        image: &NativeImage,
        index: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<SiblingAudit, EditError> {
        let count = usize::from(self.height);
        budget
            .charge_allocation(count as u64, size_of::<Level>() as u64)
            .map_err(InspectError::from)?;
        let mut levels = Vec::new();
        levels
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        for _ in 0..count {
            levels.push(Level {
                nodes: Vec::new(),
                linked: false,
            });
        }
        let mut result = SiblingAudit {
            root: self.number,
            physical: index.data_number(),
            levels,
            pages: Vec::new(),
            empty_root: false,
        };
        self.collect_siblings(image, &mut result, budget, cancelled)?;
        for level in &mut result.levels {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, level.nodes.len() as u64)
                .map_err(InspectError::from)?;
            level.linked = level
                .nodes
                .iter()
                .any(|node| node.previous != 0 || node.next != 0);
            if !level.linked {
                continue;
            }
            for (position, node) in level.nodes.iter().enumerate() {
                check(budget, cancelled)?;
                let previous = position
                    .checked_sub(1)
                    .map_or(0, |index| level.nodes[index].number);
                let next = level.nodes.get(position + 1).map_or(0, |node| node.number);
                if node.previous != previous || node.next != next {
                    return Err(InspectError::Malformed.into());
                }
            }
        }
        Ok(result)
    }

    fn collect_siblings(
        &self,
        image: &NativeImage,
        result: &mut SiblingAudit,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        budget
            .charge_allocation(1, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        result
            .pages
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        result.pages.push(self.number);
        if self.children.is_empty() {
            return Ok(());
        }
        budget
            .charge(Resource::Steps, size_of::<IndexHeader>() as u64)
            .map_err(InspectError::from)?;
        let header = IndexHeader::read(raw_page(image, self.number)?)?;
        let position = usize::from(self.height)
            .checked_sub(1)
            .ok_or(EditError::Preservation)?;
        let level = result
            .levels
            .get_mut(position)
            .ok_or(EditError::Preservation)?;
        budget
            .charge_allocation(1, size_of::<Node>() as u64)
            .map_err(InspectError::from)?;
        level
            .nodes
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        level.nodes.push(Node {
            number: self.number,
            previous: header.previous.get(),
            next: header.next.get(),
        });
        for child in &self.children {
            child.collect_siblings(image, result, budget, cancelled)?;
        }
        Ok(())
    }
}

impl SiblingAudit {
    pub(in crate::table::indexes::mutation) fn repair(
        &mut self,
        image: &NativeImage,
        empty_root: bool,
        retired: &[RetiredPage],
        patches: &mut Vec<IndexDataPatch>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.empty_root = empty_root;
        for level in &self.levels {
            check(budget, cancelled)?;
            if !level.linked {
                continue;
            }
            budget
                .charge_allocation(level.nodes.len() as u64, size_of::<u32>() as u64)
                .map_err(InspectError::from)?;
            let mut surviving = Vec::new();
            surviving
                .try_reserve_exact(level.nodes.len())
                .map_err(|_| InspectError::Allocation)?;
            for node in &level.nodes {
                if !self.retired(node.number, retired, budget, cancelled)? {
                    surviving.push(node.number);
                }
            }
            for (position, &number) in surviving.iter().enumerate() {
                let previous = position.checked_sub(1).map_or(0, |index| surviving[index]);
                let next = surviving.get(position + 1).copied().unwrap_or(0);
                header_patch(
                    image,
                    number,
                    std::mem::offset_of!(IndexHeader, previous),
                    &previous.to_le_bytes(),
                    patches,
                    budget,
                    cancelled,
                )?;
                header_patch(
                    image,
                    number,
                    std::mem::offset_of!(IndexHeader, next),
                    &next.to_le_bytes(),
                    patches,
                    budget,
                    cancelled,
                )?;
            }
        }
        Ok(())
    }

    fn retired(
        &self,
        number: u32,
        retired: &[RetiredPage],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, retired.len() as u64)
            .map_err(InspectError::from)?;
        Ok(retired.iter().any(|page| {
            page.physical == self.physical && page.root == self.root && page.number == number
        }))
    }
}
