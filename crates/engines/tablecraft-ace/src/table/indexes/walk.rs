use super::Int32IndexSelection;
use super::page::{self, KeyRef};
use super::profile::KeyWidth;
use crate::{
    InspectError,
    format::reader::Reader,
    limits::{MAX_INDEX_DEPTH, MAX_LIVE_ROWS},
};
use std::{
    cmp::Ordering,
    io::{Read, Seek},
};

pub(super) struct Walker<'a, 'b, R> {
    pub reader: &'a mut Reader<'b, R>,
    pub owner: u32,
    pub owned: &'a [u32],
    pub descending: bool,
    pub width: KeyWidth,
    pub visited: Vec<u32>,
    pub entries: Vec<KeyRef>,
    pub previous_leaf: u32,
    pub next_leaf: u32,
    pub selection: Int32IndexSelection,
    pub gap: bool,
}
impl<R: Read + Seek> Walker<'_, '_, R> {
    pub fn visit(
        &mut self,
        number: u32,
        depth: usize,
        lower: Option<KeyRef>,
        upper: Option<KeyRef>,
    ) -> Result<(), InspectError> {
        self.reader.tick()?;
        if depth >= MAX_INDEX_DEPTH {
            return Err(InspectError::StructureLimit);
        }
        if self.owned.binary_search(&number).is_err() || self.visited.contains(&number) {
            return Err(InspectError::Malformed);
        }
        if !self.selection.may_match(lower, upper, self.descending) {
            self.gap = true;
            return Ok(());
        }
        self.reader
            .budget
            .charge_allocation(1, size_of::<u32>() as u64)?;
        self.visited
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        self.visited.push(number);
        let page = page::read(self.reader, number, self.owner, self.descending, self.width)?;
        for entry in &page.entries {
            if lower.is_some_and(|lo| entry.value.compare(lo, self.descending) != Ordering::Greater)
                || upper
                    .is_some_and(|hi| entry.value.compare(hi, self.descending) == Ordering::Greater)
            {
                return Err(InspectError::Malformed);
            }
        }
        if page.leaf {
            if [page.previous, page.next].into_iter().any(|sibling| {
                sibling == number || sibling != 0 && self.owned.binary_search(&sibling).is_err()
            }) || (self.previous_leaf == 0 && ((page.previous != 0) != self.gap))
                || (self.previous_leaf != 0
                    && (self.gap
                        || page.previous != self.previous_leaf
                        || self.next_leaf != number))
            {
                return Err(InspectError::Malformed);
            }
            self.previous_leaf = number;
            self.next_leaf = page.next;
            self.gap = false;
            for entry in page.entries {
                self.reader.tick()?;
                if !self.selection.contains(entry.value.key) {
                    continue;
                }
                if self.entries.len() >= MAX_LIVE_ROWS {
                    return Err(InspectError::StructureLimit);
                }
                if self.entries.last().is_some_and(|last| {
                    last.compare(entry.value, self.descending) != Ordering::Less
                }) {
                    return Err(InspectError::Malformed);
                }
                self.reader
                    .budget
                    .charge_allocation(1, size_of::<KeyRef>() as u64)?;
                self.entries
                    .try_reserve(1)
                    .map_err(|_| InspectError::Allocation)?;
                self.entries.push(entry.value);
            }
        } else {
            let mut previous = lower;
            for entry in page.entries {
                self.visit(entry.child, depth + 1, previous, Some(entry.value))?;
                previous = Some(entry.value);
            }
            self.visit(page.tail, depth + 1, previous, upper)?;
        }
        Ok(())
    }
}
