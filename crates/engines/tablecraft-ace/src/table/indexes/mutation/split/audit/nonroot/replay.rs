//! Independent postorder capacity proof, followed by per-height link checks.
mod normalize;
use super::super::super::super::{encoding, tree::Tree};
use super::super::super::leaf_entries;
use super::{Operation, Progress, framing};
use crate::{
    EditError, InspectError, NativeImage,
    format::{page::PAGE_BYTES, reader::Reader},
    limits::MAX_INDEX_DEPTH,
    table::indexes::{
        header::IndexHeader,
        page::{self, Entry, KeyRef},
    },
};
use std::io::Cursor;

#[derive(Clone, Copy)]
struct Child {
    number: u32,
    maximum: KeyRef,
}

enum Payload<'a> {
    Leaf(&'a [KeyRef]),
    Branch { children: Vec<Child>, tail: bool },
}

struct Candidate<'a> {
    number: u32,
    height: u8,
    contents: Payload<'a>,
    source: framing::Source<'a>,
}

struct Replay<'a, 'b> {
    operation: &'a Operation<'b>,
    position: usize,
    added: Vec<u32>,
    levels: [Vec<u32>; MAX_INDEX_DEPTH],
}

struct Branch<'a> {
    number: u32,
    height: u8,
    children: &'a [Child],
    tail: bool,
    source: framing::Source<'a>,
}

struct PageSpec<'a> {
    number: u32,
    height: u8,
    entries: &'a [Entry],
    tail: u32,
    source: framing::Source<'a>,
}

pub(super) fn prove(
    operation: &Operation<'_>,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut Progress<'_>,
) -> Result<Vec<u32>, EditError> {
    let mut replay = Replay {
        operation,
        position: 0,
        added: Vec::new(),
        levels: std::array::from_fn(|_| Vec::new()),
    };
    let result = replay.walk(operation.tree, true, actual, progress)?;
    if result.len() != 1 || replay.position != operation.keys.len() {
        return Err(EditError::Preservation);
    }
    for numbers in &replay.levels {
        for (position, number) in numbers.iter().enumerate() {
            actual.tick()?;
            let bytes = actual.page(*number)?;
            let header = IndexHeader::read(&bytes)?;
            let previous = position.checked_sub(1).map_or(0, |prior| numbers[prior]);
            let next = numbers.get(position + 1).copied().unwrap_or(0);
            if (header.previous.get(), header.next.get()) != (previous, next) {
                return Err(EditError::Preservation);
            }
        }
    }
    Ok(replay.added)
}

impl Replay<'_, '_> {
    fn walk(
        &mut self,
        tree: &Tree,
        root: bool,
        actual: &mut Reader<'_, Cursor<&[u8]>>,
        progress: &mut Progress<'_>,
    ) -> Result<Vec<Child>, EditError> {
        actual.tick()?;
        let original = raw(self.operation.image, tree.number())?;
        let contents = if tree.is_leaf() {
            if tree.count() == 0 {
                return Err(EditError::Preservation);
            }
            let end = self
                .position
                .checked_add(tree.count())
                .filter(|end| *end <= self.operation.keys.len())
                .ok_or(EditError::Preservation)?;
            let keys = &self.operation.keys[self.position..end];
            self.position = end;
            Payload::Leaf(keys)
        } else {
            let mut children = Vec::new();
            for child in tree.children() {
                for result in self.walk(child, false, actual, progress)? {
                    push(&mut children, result, actual)?;
                }
            }
            let tail = IndexHeader::read(original)?.tail.get() != 0;
            Payload::Branch { children, tail }
        };
        normalize::finish(
            self,
            Candidate {
                number: tree.number(),
                height: tree.height(),
                contents,
                source: framing::Source::Existing(original),
            },
            root,
            actual,
            progress,
        )
    }

    fn leaf_page(
        &mut self,
        number: u32,
        keys: &[KeyRef],
        source: framing::Source<'_>,
        actual: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Child, EditError> {
        let entries = leaf_entries(keys, actual.budget, actual.cancelled)?;
        self.page(
            &PageSpec {
                number,
                height: 0,
                entries: &entries,
                tail: 0,
                source,
            },
            actual,
        )?;
        Ok(Child {
            number,
            maximum: *keys.last().ok_or(EditError::Preservation)?,
        })
    }

    fn branch(
        &mut self,
        branch: &Branch<'_>,
        actual: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Child, EditError> {
        let entries = bounds(branch.children, branch.tail, actual)?;
        let last = branch.children.last().ok_or(EditError::Preservation)?;
        self.page(
            &PageSpec {
                number: branch.number,
                height: branch.height,
                entries: &entries,
                tail: if branch.tail { last.number } else { 0 },
                source: branch.source,
            },
            actual,
        )?;
        Ok(Child {
            number: branch.number,
            maximum: last.maximum,
        })
    }

    fn page(
        &mut self,
        spec: &PageSpec<'_>,
        actual: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        let PageSpec {
            number,
            height,
            entries,
            tail,
            source,
        } = *spec;
        let page = page::read_profile::<_, _, { encoding::MAX_ENTRY_BYTES }>(
            actual,
            number,
            self.operation.owner,
            self.operation.profile,
        )?;
        if page.leaf != (height == 0) || page.tail != tail || page.entries.len() != entries.len() {
            return Err(EditError::Preservation);
        }
        for (entry, expected) in page.entries.iter().zip(entries) {
            actual.tick()?;
            if entry.value != expected.value || entry.child != expected.child {
                return Err(EditError::Preservation);
            }
        }
        let bytes = actual.page(number)?;
        framing::prove(
            &framing::Spec {
                source,
                after: &bytes,
                entries,
                profile: self.operation.profile,
                owner: self.operation.owner,
                height,
                tail,
            },
            actual,
        )?;
        let level = self
            .levels
            .get_mut(usize::from(height))
            .ok_or(EditError::Preservation)?;
        push(level, number, actual)
    }

    fn allocate(
        &mut self,
        actual: &mut Reader<'_, Cursor<&[u8]>>,
        progress: &mut Progress<'_>,
    ) -> Result<u32, EditError> {
        let number = *progress.next_eof;
        *progress.next_eof = number.checked_add(1).ok_or(EditError::Preservation)?;
        push(&mut self.added, number, actual)?;
        let mut source = self.operation.image.reader();
        super::super::maps::add(
            &mut Reader {
                source: &mut source,
                header: self.operation.image.catalog().header(),
                budget: actual.budget,
                cancelled: actual.cancelled,
                reads: 0,
            },
            self.operation.usage,
            [number],
            progress.map_edits,
        )?;
        Ok(number)
    }
}

fn bounds(
    children: &[Child],
    tail: bool,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<Entry>, EditError> {
    let count = children
        .len()
        .checked_sub(usize::from(tail))
        .ok_or(EditError::Preservation)?;
    let mut entries = Vec::new();
    for child in &children[..count] {
        push(
            &mut entries,
            Entry {
                value: child.maximum,
                child: child.number,
            },
            actual,
        )?;
    }
    Ok(entries)
}

fn push<T>(
    items: &mut Vec<T>,
    item: T,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    actual
        .budget
        .charge_allocation(1, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    items.try_reserve(1).map_err(|_| InspectError::Allocation)?;
    items.push(item);
    Ok(())
}

fn raw(image: &NativeImage, number: u32) -> Result<&[u8], EditError> {
    let offset = number as usize * PAGE_BYTES;
    image
        .as_bytes()
        .get(offset..offset + PAGE_BYTES)
        .ok_or(EditError::Preservation)
}
