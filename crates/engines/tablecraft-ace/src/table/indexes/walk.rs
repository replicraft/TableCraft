use super::key::KeyProfile;
use super::page::{self, KeyRef};
use crate::{InspectError, format::reader::Reader};
use std::cmp::Ordering;

pub(super) struct Walker<'a, 'b, R, P: KeyProfile, const N: usize> {
    pub reader: &'a mut Reader<'b, R>,
    pub owner: u32,
    pub owned: &'a [u32],
    pub profile: &'a P,
    pub visited: Vec<u32>,
    pub entries: Vec<KeyRef<P::Key>>,
    pub maximum_rows: usize,
    pub previous_leaf: u32,
    pub next_leaf: u32,
    pub gap: bool,
}
impl<R: crate::NativeReadSource, P: KeyProfile, const N: usize> Walker<'_, '_, R, P, N> {
    pub fn visit(&mut self, root: u32) -> Result<(), InspectError> {
        // Frames own parent separators until every child has finished. Fences
        // refer to ancestors by position, never by a cloned key or self-borrow.
        let mut frames: Vec<Frame<P::Key>> = Vec::new();
        let mut pending = Some(Child {
            number: root,
            lower: None,
            upper: None,
        });

        loop {
            let child = if let Some(child) = pending.take() {
                child
            } else {
                let Some(frame_index) = frames.len().checked_sub(1) else {
                    return Ok(());
                };
                self.reader.tick()?;
                let frame = &mut frames[frame_index];
                let Some(child) = frame.next_child(frame_index) else {
                    frames.pop();
                    continue;
                };
                child
            };

            self.reader.tick()?;
            self.reader.work(self.visited.len())?;
            if self.owned.binary_search(&child.number).is_err()
                || self.visited.contains(&child.number)
            {
                return Err(InspectError::Malformed);
            }
            let lower = fence(&frames, child.lower);
            let upper = fence(&frames, child.upper);
            if !self.profile.may_match(lower, upper) {
                self.gap = true;
                continue;
            }
            self.reader
                .budget
                .charge_allocation(1, size_of::<u32>() as u64)?;
            self.visited
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            self.visited.push(child.number);

            let page =
                page::read_profile::<_, _, N>(self.reader, child.number, self.owner, self.profile)?;
            for entry in &page.entries {
                self.reader.work(self.profile.components())?;
                if lower.is_some_and(|lo| {
                    entry.value.compare_with(lo, self.profile) != Ordering::Greater
                }) || upper.is_some_and(|hi| {
                    entry.value.compare_with(hi, self.profile) == Ordering::Greater
                }) {
                    return Err(InspectError::Malformed);
                }
            }

            if page.leaf {
                self.leaf(child.number, page)?;
            } else {
                // Every retained frame consumes a distinct owned native page;
                // source geometry, visited checks and the paid stack bound it.
                if frames.len() >= self.owned.len() {
                    return Err(InspectError::Malformed);
                }
                self.reader
                    .budget
                    .charge_allocation(1, size_of::<Frame<P::Key>>() as u64)?;
                frames
                    .try_reserve(1)
                    .map_err(|_| InspectError::Allocation)?;
                frames.push(Frame {
                    page,
                    next: 0,
                    lower: child.lower,
                    upper: child.upper,
                });
            }
        }
    }

    fn leaf(&mut self, number: u32, page: page::Page<P::Key>) -> Result<(), InspectError> {
        if [page.previous, page.next].into_iter().any(|sibling| {
            sibling == number || sibling != 0 && self.owned.binary_search(&sibling).is_err()
        }) || (self.previous_leaf == 0 && ((page.previous != 0) != self.gap))
            || (self.previous_leaf != 0
                && (self.gap || page.previous != self.previous_leaf || self.next_leaf != number))
        {
            return Err(InspectError::Malformed);
        }
        self.previous_leaf = number;
        self.next_leaf = page.next;
        self.gap = false;
        for entry in page.entries {
            self.reader.work(self.profile.components())?;
            if !self.profile.contains(&entry.value.key) {
                continue;
            }
            if self.entries.len() >= self.maximum_rows {
                return Err(InspectError::StructureLimit);
            }
            if self
                .entries
                .last()
                .is_some_and(|last| last.compare_with(&entry.value, self.profile) != Ordering::Less)
            {
                return Err(InspectError::Malformed);
            }
            self.reader
                .budget
                .charge_allocation(1, size_of::<KeyRef<P::Key>>() as u64)?;
            self.entries
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            self.entries.push(entry.value);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Fence {
    frame: usize,
    entry: usize,
}

struct Child {
    number: u32,
    lower: Option<Fence>,
    upper: Option<Fence>,
}

struct Frame<K> {
    page: page::Page<K>,
    next: usize,
    lower: Option<Fence>,
    upper: Option<Fence>,
}

impl<K> Frame<K> {
    fn next_child(&mut self, frame: usize) -> Option<Child> {
        let position = self.next;
        if position > self.page.entries.len() {
            return None;
        }
        self.next += 1;
        let lower = position
            .checked_sub(1)
            .map(|entry| Fence { frame, entry })
            .or(self.lower);
        if let Some(entry) = self.page.entries.get(position) {
            Some(Child {
                number: entry.child,
                lower,
                upper: Some(Fence {
                    frame,
                    entry: position,
                }),
            })
        } else if self.page.tail != 0 {
            Some(Child {
                number: self.page.tail,
                lower,
                upper: self.upper,
            })
        } else {
            None
        }
    }
}

fn fence<K>(frames: &[Frame<K>], fence: Option<Fence>) -> Option<&KeyRef<K>> {
    fence.map(|fence| &frames[fence.frame].page.entries[fence.entry].value)
}

#[cfg(test)]
mod tests;
