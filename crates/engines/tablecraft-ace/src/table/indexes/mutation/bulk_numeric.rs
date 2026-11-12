//! Paid bottom-up construction of fresh ordinary numeric index pages.
use super::{NumericKey, pages, profile::SingleProfile, repack};
use crate::{
    EditError, InspectError, WriteBlocker,
    header::check,
    table::{
        indexes::page::{Entry, KeyRef},
        mutation::index_creation::buffer,
    },
};
use std::{cmp::Ordering, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// Source-bound scalar facts; these are not index or mutation authority.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::table) struct NumericKeyRow {
    pub value: Option<i64>,
    pub reference: u32,
}

pub(in crate::table) struct CreatedPage {
    pub number: u32,
    pub bytes: Vec<u8>,
}

struct Node {
    items: Range<usize>,
    maximum: Option<usize>,
    height: u8,
    previous: Option<usize>,
    next: Option<usize>,
}

#[derive(Clone, Copy)]
enum Group {
    Keys,
    Children,
}

/// All original keys and complete topology; no source or budget borrow escapes.
pub(in crate::table) struct Population<const WIRE_BYTES: usize> {
    keys: Vec<NumericKeyRow>,
    nodes: Vec<Node>,
    profile: SingleProfile,
    base: u32,
    root: u32,
    distinct: u32,
    height: u8,
}

impl<const WIRE_BYTES: usize> Population<WIRE_BYTES> {
    pub(in crate::table) fn capture(
        mut keys: Vec<NumericKeyRow>,
        column: usize,
        kind: NumericKey,
        descending: bool,
        base: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        for key in &keys {
            check(budget, cancelled)?;
            if let Some(value) = key.value {
                kind.ordered(value)?;
            } else if !kind.nullable() {
                return Err(InspectError::Malformed.into());
            }
        }
        sort(&mut keys, descending, budget, cancelled)?;
        let mut distinct = 0_u32;
        let mut previous = None;
        for key in &keys {
            check(budget, cancelled)?;
            if previous != Some(key.value) {
                distinct = distinct
                    .checked_add(1)
                    .ok_or(InspectError::StructureLimit)?;
            }
            previous = Some(key.value);
        }
        let mut result = Self {
            keys,
            nodes: Vec::new(),
            profile: SingleProfile {
                column,
                kind,
                descending,
            },
            base,
            root: base,
            distinct,
            height: 0,
        };
        result.leaves(budget, cancelled)?;
        let mut level = 0..result.nodes.len();
        while level.len() > 1 {
            check(budget, cancelled)?;
            let next = result.parents(level.start..level.end, budget, cancelled)?;
            if next.len() >= level.len() {
                return Err(EditError::Preservation);
            }
            level = next;
        }
        let root = result.nodes.last().ok_or(EditError::Preservation)?;
        result.height = root.height;
        result.root = result.page(result.nodes.len() - 1)?;
        Ok(result)
    }

    /// Closed unique LONG population. NULL remains a stored key; only present
    /// duplicate values violate this secondary index's uniqueness contract.
    pub(in crate::table) fn capture_unique_long(
        keys: Vec<NumericKeyRow>,
        column: usize,
        descending: bool,
        base: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let population = Self::capture(
            keys,
            column,
            NumericKey::new(tablecraft_schema::ScalarType::Int32)?,
            descending,
            base,
            budget,
            cancelled,
        )?;
        let mut previous = None;
        for key in &population.keys {
            check(budget, cancelled)?;
            if let Some(value) = key.value {
                if previous == Some(value) {
                    return Err(
                        InspectError::from(crate::IndexConstraintViolation::DuplicateKey).into(),
                    );
                }
                previous = Some(value);
            }
        }
        Ok(population)
    }

    pub(in crate::table) fn page_count(&self) -> usize {
        self.nodes.len()
    }

    pub(in crate::table) fn root(&self) -> u32 {
        self.root
    }

    pub(in crate::table) fn distinct(&self) -> u32 {
        self.distinct
    }

    pub(in crate::table) fn height(&self) -> u8 {
        self.height
    }

    pub(in crate::table) fn build(
        &self,
        owner: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<CreatedPage>, EditError> {
        let mut output = buffer::vector(self.nodes.len(), budget, cancelled)?;
        for (position, node) in self.nodes.iter().enumerate() {
            check(budget, cancelled)?;
            let links = (
                node.previous
                    .map(|index| self.page(index))
                    .transpose()?
                    .unwrap_or(0),
                node.next
                    .map(|index| self.page(index))
                    .transpose()?
                    .unwrap_or(0),
            );
            let bytes = if node.height == 0 {
                let entries = self.entries(
                    Group::Keys,
                    node.items.start..node.items.end,
                    budget,
                    cancelled,
                )?;
                pages::leaf::<_, WIRE_BYTES>(
                    owner,
                    links.0,
                    links.1,
                    &entries,
                    &self.profile,
                    budget,
                    cancelled,
                )?
            } else {
                let entries = self.entries(
                    Group::Children,
                    node.items.start..node.items.end,
                    budget,
                    cancelled,
                )?;
                let tail = self.page(
                    node.items
                        .end
                        .checked_sub(1)
                        .ok_or(EditError::Preservation)?,
                )?;
                pages::node::<_, WIRE_BYTES>(
                    &pages::NodeHeader {
                        owner,
                        height: node.height,
                        links,
                        tail,
                    },
                    &entries,
                    &self.profile,
                    budget,
                    cancelled,
                )?
            };
            output.push(CreatedPage {
                number: self.page(position)?,
                bytes,
            });
        }
        Ok(output)
    }

    fn leaves(
        &mut self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.keys.is_empty() {
            buffer::push(
                &mut self.nodes,
                Node {
                    items: 0..0,
                    maximum: None,
                    height: 0,
                    previous: None,
                    next: None,
                },
                budget,
                cancelled,
            )?;
            return Ok(());
        }
        let mut start = 0;
        while start < self.keys.len() {
            let end = self.end(Group::Keys, start, self.keys.len(), budget, cancelled)?;
            let previous = self.nodes.len().checked_sub(1);
            let position = self.nodes.len();
            buffer::push(
                &mut self.nodes,
                Node {
                    items: start..end,
                    maximum: Some(end - 1),
                    height: 0,
                    previous,
                    next: None,
                },
                budget,
                cancelled,
            )?;
            if let Some(previous) = previous {
                self.nodes[previous].next = Some(position);
            }
            start = end;
        }
        Ok(())
    }

    fn parents(
        &mut self,
        children: Range<usize>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Range<usize>, EditError> {
        let first = self.nodes.len();
        let height = self
            .nodes
            .get(children.start)
            .ok_or(EditError::Preservation)?
            .height
            .checked_add(1)
            .ok_or(InspectError::StructureLimit)?;
        let mut start = children.start;
        while start < children.end {
            check(budget, cancelled)?;
            let mut end = self.end(Group::Children, start, children.end, budget, cancelled)?;
            if children.end - end == 1 {
                end = end.checked_sub(1).ok_or(EditError::Preservation)?;
            }
            if end - start < 2 {
                return Err(EditError::Preservation);
            }
            let maximum = self.nodes[end - 1].maximum.ok_or(EditError::Preservation)?;
            let previous = if self.nodes.len() == first {
                None
            } else {
                Some(self.nodes.len() - 1)
            };
            let position = self.nodes.len();
            buffer::push(
                &mut self.nodes,
                Node {
                    items: start..end,
                    maximum: Some(maximum),
                    height,
                    previous,
                    next: None,
                },
                budget,
                cancelled,
            )?;
            if let Some(previous) = previous {
                self.nodes[previous].next = Some(position);
            }
            start = end;
        }
        Ok(first..self.nodes.len())
    }

    fn end(
        &self,
        kind: Group,
        start: usize,
        limit: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, EditError> {
        let minimum = match kind {
            Group::Keys => 1,
            Group::Children => 2,
        };
        let mut good = start
            .checked_add(minimum)
            .filter(|end| *end <= limit)
            .ok_or(EditError::Preservation)?;
        if !self.fits(kind, start..good, budget, cancelled)? {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let mut width = minimum;
        let bad = loop {
            check(budget, cancelled)?;
            if good == limit {
                return Ok(good);
            }
            let remaining = limit - start;
            width = if width > remaining / 2 {
                remaining
            } else {
                width * 2
            };
            let candidate = start + width;
            if self.fits(kind, start..candidate, budget, cancelled)? {
                good = candidate;
            } else {
                break candidate;
            }
        };
        let mut bad = bad;
        while good + 1 < bad {
            check(budget, cancelled)?;
            let middle = good + (bad - good) / 2;
            if self.fits(kind, start..middle, budget, cancelled)? {
                good = middle;
            } else {
                bad = middle;
            }
        }
        Ok(good)
    }

    fn fits(
        &self,
        kind: Group,
        range: Range<usize>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        let entries = self.entries(kind, range, budget, cancelled)?;
        repack::borrowed_fits_fresh::<_, WIRE_BYTES>(&entries, &self.profile, budget, cancelled)
    }

    fn entries(
        &self,
        kind: Group,
        range: Range<usize>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<Entry<&Option<i64>>>, EditError> {
        let count = range.len() - usize::from(matches!(kind, Group::Children));
        let mut entries = buffer::vector(count, budget, cancelled)?;
        for index in range.start..range.start + count {
            check(budget, cancelled)?;
            let (key, child) = match kind {
                Group::Keys => (self.keys.get(index).ok_or(EditError::Preservation)?, 0),
                Group::Children => {
                    let maximum = self
                        .nodes
                        .get(index)
                        .and_then(|node| node.maximum)
                        .ok_or(EditError::Preservation)?;
                    (
                        self.keys.get(maximum).ok_or(EditError::Preservation)?,
                        self.page(index)?,
                    )
                }
            };
            entries.push(Entry {
                value: KeyRef {
                    key: &key.value,
                    row: key.reference,
                },
                child,
            });
        }
        Ok(entries)
    }

    fn page(&self, index: usize) -> Result<u32, InspectError> {
        self.base
            .checked_add(u32::try_from(index).map_err(|_| InspectError::StructureLimit)?)
            .ok_or(InspectError::StructureLimit)
    }
}

fn compare(left: &NumericKeyRow, right: &NumericKeyRow, descending: bool) -> Ordering {
    let order = left.value.cmp(&right.value);
    (if descending { order.reverse() } else { order }).then(left.reference.cmp(&right.reference))
}

/// In-place heap ordering checks work/cancellation at every comparison and swap.
fn sort(
    keys: &mut [NumericKeyRow],
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    for start in (0..keys.len() / 2).rev() {
        sift(keys, start, keys.len(), descending, budget, cancelled)?;
    }
    for end in (1..keys.len()).rev() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, size_of::<NumericKeyRow>() as u64)?;
        keys.swap(0, end);
        sift(keys, 0, end, descending, budget, cancelled)?;
    }
    Ok(())
}

fn sift(
    keys: &mut [NumericKeyRow],
    mut root: usize,
    end: usize,
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    while root < end / 2 {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, (2 * size_of::<NumericKeyRow>()) as u64)?;
        let mut child = root
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or(InspectError::StructureLimit)?;
        if child + 1 < end && compare(&keys[child], &keys[child + 1], descending).is_lt() {
            child += 1;
        }
        if !compare(&keys[root], &keys[child], descending).is_lt() {
            break;
        }
        keys.swap(root, child);
        root = child;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
