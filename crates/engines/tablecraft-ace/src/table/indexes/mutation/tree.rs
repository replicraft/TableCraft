//! Retain the existing page topology, entry counts, prefix and entry-end bitmap.
//! The completed source check establishes ownership, ordering and row coverage.
use super::super::{
    header::IndexHeader,
    page::{self, ENTRY_START, Entry, KeyRef},
    profile::KeyType,
};
use super::{IndexDataPatch, encoding};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    limits::{MAX_INDEX_DEPTH, MAX_LIVE_ROWS},
};
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Tree {
    number: u32,
    prefix: usize,
    length: usize,
    count: usize,
    height: u8,
    children: Vec<Tree>,
}

impl Tree {
    pub(super) fn read(
        image: &NativeImage,
        reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
        owner: u32,
        number: u32,
        descending: bool,
        depth: usize,
        keys: &mut Vec<KeyRef>,
    ) -> Result<Self, EditError> {
        reader.tick()?;
        if depth >= MAX_INDEX_DEPTH {
            return Err(InspectError::StructureLimit.into());
        }
        // The immutable image was fully checked before this bounded traversal;
        // repeat the decoder's owner/type checks rather than trusting offsets.
        let page = page::read(reader, number, owner, descending, KeyType::Int32)?;
        let raw = raw_page(image, number)?;
        let header = IndexHeader::read(raw)?;
        let end = PAGE_BYTES
            .checked_sub(usize::from(header.free.get()))
            .filter(|end| *end >= ENTRY_START)
            .ok_or(InspectError::Malformed)?;
        reader
            .budget
            .charge_allocation(1, size_of::<Self>() as u64)
            .map_err(InspectError::from)?;
        let mut children = Vec::new();
        let count = page.entries.len();
        if page.leaf {
            if count == 0 && depth != 0 {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            if keys.len() + count > MAX_LIVE_ROWS {
                return Err(InspectError::StructureLimit.into());
            }
            reader
                .budget
                .charge_allocation(count as u64, size_of::<KeyRef>() as u64)
                .map_err(InspectError::from)?;
            keys.try_reserve_exact(count)
                .map_err(|_| InspectError::Allocation)?;
            keys.extend(page.entries.into_iter().map(|entry| entry.value));
        } else {
            reader
                .budget
                .charge_allocation((count + 1) as u64, size_of::<Self>() as u64)
                .map_err(InspectError::from)?;
            children
                .try_reserve_exact(count + 1)
                .map_err(|_| InspectError::Allocation)?;
            for child in page
                .entries
                .iter()
                .map(|entry| entry.child)
                .chain([page.tail])
            {
                children.push(Self::read(
                    image,
                    reader,
                    owner,
                    child,
                    descending,
                    depth + 1,
                    keys,
                )?);
            }
        }
        let height = if page.leaf {
            0
        } else {
            let height = children.first().ok_or(InspectError::Malformed)?.height;
            if children.iter().any(|child| child.height != height) {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            height.checked_add(1).ok_or(InspectError::StructureLimit)?
        };
        if !header.qualified_for_update(height) {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        Ok(Self {
            number,
            prefix: usize::from(header.prefix.get()),
            length: end - ENTRY_START,
            count,
            height,
            children,
        })
    }

    pub(super) fn rewrite(
        &self,
        image: &NativeImage,
        keys: &mut &[KeyRef],
        descending: bool,
        patches: &mut Vec<IndexDataPatch>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<KeyRef>, EditError> {
        check(budget, cancelled)?;
        budget
            .charge_allocation(self.count as u64, size_of::<Entry>() as u64)
            .map_err(InspectError::from)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(self.count)
            .map_err(|_| InspectError::Allocation)?;
        let last = if self.children.is_empty() {
            let (slice, rest) = keys
                .split_at_checked(self.count)
                .ok_or(EditError::Preservation)?;
            entries.extend(slice.iter().map(|value| Entry {
                value: *value,
                child: 0,
            }));
            *keys = rest;
            slice.last().copied()
        } else {
            let mut last = None;
            for (position, child) in self.children.iter().enumerate() {
                check(budget, cancelled)?;
                last = child.rewrite(image, keys, descending, patches, budget, cancelled)?;
                let value = last.ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
                if position < self.count {
                    // Node separators name the complete last key/row pair in
                    // each non-tail subtree, including ties between NULL keys.
                    entries.push(Entry {
                        value,
                        child: child.number,
                    });
                }
            }
            last
        };
        let after = encoding::encode(
            &entries,
            self.prefix,
            self.length,
            descending,
            budget,
            cancelled,
        )?;
        let raw = raw_page(image, self.number)?;
        let original = &raw[ENTRY_START..ENTRY_START + self.length];
        if after != original {
            budget
                .charge_allocation(self.length as u64, 1)
                .map_err(InspectError::from)?;
            let mut before = Vec::new();
            before
                .try_reserve_exact(self.length)
                .map_err(|_| InspectError::Allocation)?;
            before.extend_from_slice(original);
            budget
                .charge_allocation(1, size_of::<IndexDataPatch>() as u64)
                .map_err(InspectError::from)?;
            patches
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            patches.push(IndexDataPatch {
                offset: self.number as usize * PAGE_BYTES + ENTRY_START,
                before,
                after,
            });
        }
        Ok(last)
    }
}

fn raw_page(image: &NativeImage, number: u32) -> Result<&[u8], InspectError> {
    let offset = (number as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::Malformed)?;
    image
        .bytes
        .get(offset..offset + PAGE_BYTES)
        .ok_or(InspectError::Malformed)
}
