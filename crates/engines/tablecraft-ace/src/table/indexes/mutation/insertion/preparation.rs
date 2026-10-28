//! Fixed typed membership survives until physical continuation has completed.
use super::{ExpectedKey, IndexInsertion, InsertRow, membership};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableCheck, WriteBlocker,
    format::{
        page::PAGE_BYTES,
        reader::Reader,
        row::{MAX_SLOTS, reference_page, reference_slot},
    },
    header::check,
    limits::{MAX_IMAGE_BYTES, MAX_LIVE_ROWS},
    table::{
        indexes::{
            header::IndexHeader,
            key::KeyWork,
            mutation::{
                profile::WriteProfile,
                repack,
                row_key::RowKeyProfile,
                split::{self, insert::Operation},
                tree::Tree,
            },
            page::{KeyRef, LEAF, NODE},
        },
        mutation::append::State,
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Prepared<P: WriteProfile> {
    pub(super) result: IndexInsertion,
    pub(super) profile: P,
    pub(super) keys: Vec<KeyRef<P::Key>>,
    pub(super) requires_continuation: bool,
}

pub(super) fn prepare<P: WriteProfile + RowKeyProfile, const N: usize>(
    image: &NativeImage,
    checked: &TableCheck,
    index: &NativeIndex,
    profile: P,
    requests: &[InsertRow<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared<P>, EditError> {
    let start = index.root_page() as usize * PAGE_BYTES;
    let original = image
        .as_bytes()
        .get(start..start + PAGE_BYTES)
        .ok_or(InspectError::Truncated)?;
    let header = IndexHeader::read(original)?;
    let source_leaf =
        header.kind == LEAF && header.tail.get() == 0 && header.qualified_for_update(0);
    let source_node = header.kind == NODE;
    if (!source_leaf && !source_node) || header.previous.get() != 0 || header.next.get() != 0 {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut keys = Vec::new();
    let tree = Tree::read::<_, N>(
        image,
        &mut reader,
        checked.schema().definition_page(),
        index.root_page(),
        &profile,
        0,
        &mut keys,
    )?;
    membership::require_population(index, &keys, &profile, checked.rows(), &mut reader)?;
    if source_leaf != tree.is_leaf() {
        return Err(EditError::Preservation);
    }
    let original_count = keys.len();
    let original_distinct = membership::distinct(&keys, &profile, &mut reader)?;
    let mut source_pages = Vec::new();
    collect_pages(&tree, &mut source_pages, &mut reader)?;
    let capacity = original_count
        .checked_add(requests.len())
        .filter(|count| *count <= MAX_LIVE_ROWS)
        .ok_or(InspectError::StructureLimit)?;
    reader
        .budget
        .charge_allocation(requests.len() as u64, size_of::<KeyRef<P::Key>>() as u64)
        .map_err(InspectError::from)?;
    keys.try_reserve_exact(requests.len())
        .map_err(|_| InspectError::Allocation)?;
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    for (position, request) in requests.iter().enumerate() {
        reader.tick()?;
        let wire = profile.encoded_row(
            request,
            &mut KeyWork {
                budget: reader.budget,
                cancelled,
            },
        )?;
        let (key, consumed) = profile.decode(
            &wire,
            &mut KeyWork {
                budget: reader.budget,
                cancelled,
            },
        )?;
        if consumed != wire.len() {
            return Err(EditError::Preservation);
        }
        let page = reference_page(request.reference);
        if page == 0
            || u64::from(page) >= MAX_IMAGE_BYTES / PAGE_BYTES as u64
            || usize::from(reference_slot(request.reference)) >= MAX_SLOTS
        {
            return Err(EditError::Conflict);
        }
        reader.work(keys.len() + position)?;
        if keys.iter().any(|entry| entry.row == request.reference)
            || requests[..position]
                .iter()
                .any(|prior| prior.reference == request.reference)
        {
            return Err(EditError::Conflict);
        }
        if constraints.ignore_nulls() && profile.constraint_nulls(&key)?.0 {
            continue;
        }
        keys.push(KeyRef {
            key,
            row: request.reference,
        });
    }
    if keys.len() > capacity {
        return Err(EditError::Preservation);
    }
    let levels = keys.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let sort_work = keys
        .len()
        .checked_mul(levels)
        .and_then(|count| count.checked_mul(profile.max_key_bytes()))
        .ok_or(InspectError::StructureLimit)?;
    reader.work(sort_work)?;
    keys.sort_unstable_by(|left, right| left.compare_with(right, &profile));
    membership::require_constraints(index, &keys, &profile, &mut reader)?;
    let distinct_added = membership::distinct(&keys, &profile, &mut reader)?
        .checked_sub(original_distinct)
        .ok_or(EditError::Preservation)?;
    let changed = keys.len() != original_count;
    let mut patches = Vec::new();
    let mut requires_continuation = source_node && changed;
    if source_leaf && changed {
        let entries = split::leaf_entries(&keys, reader.budget, cancelled)?;
        if repack::borrowed_fits::<_, N>(original, &entries, &profile, reader.budget, cancelled)? {
            let after = repack::borrowed_page::<_, N>(
                original,
                &entries,
                &profile,
                reader.budget,
                cancelled,
            )?;
            repack::patches(
                original,
                &after,
                index.root_page(),
                &mut patches,
                reader.budget,
                cancelled,
            )?;
        } else {
            requires_continuation = true;
        }
    }
    check(reader.budget, cancelled)?;
    Ok(Prepared {
        result: IndexInsertion {
            patches,
            added: Vec::new(),
            promotions: Vec::new(),
            rebases: Vec::new(),
            physical: index.data_number(),
            root: index.root_page(),
            usage: index.usage_map(),
            original_count,
            source_pages,
            distinct_added,
            expected: Vec::new(),
        },
        profile,
        keys,
        requires_continuation,
    })
}

impl<P: WriteProfile> Prepared<P> {
    pub(super) fn finish<const N: usize>(
        mut self,
        image: &NativeImage,
        checked: &TableCheck,
        mut append: State,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(IndexInsertion, State), EditError> {
        check(budget, cancelled)?;
        if self.requires_continuation {
            let index = super::qualify(checked, self.result.physical, budget, cancelled)?;
            if index.root_page() != self.result.root || index.usage_map() != self.result.usage {
                return Err(EditError::Preservation);
            }
            let plan = split::insert::continue_index::<_, N>(
                Operation {
                    image,
                    checked,
                    index,
                    profile: &self.profile,
                    final_keys: &self.keys,
                },
                append,
                budget,
                cancelled,
            )?;
            self.result.patches = plan.patches;
            self.result.added = plan.added;
            self.result.promotions = plan.promotions;
            self.result.rebases = plan.rebases;
            append = plan.append;
        }
        // Typed keys remain available through continuation. Only completed
        // comparison facts enter the heterogeneous final plan collection.
        budget
            .charge_allocation(self.keys.len() as u64, size_of::<ExpectedKey>() as u64)
            .map_err(InspectError::from)?;
        self.result
            .expected
            .try_reserve_exact(self.keys.len())
            .map_err(|_| InspectError::Allocation)?;
        for entry in &self.keys {
            check(budget, cancelled)?;
            let mut encoded = [0; N];
            let length = self.profile.encode(entry.key, &mut encoded)?;
            budget
                .charge_allocation(length as u64, 1)
                .map_err(InspectError::from)?;
            budget
                .charge(Resource::Steps, length as u64)
                .map_err(InspectError::from)?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(length)
                .map_err(|_| InspectError::Allocation)?;
            bytes.extend_from_slice(&encoded[..length]);
            self.result.expected.push(ExpectedKey {
                bytes,
                reference: entry.row,
            });
        }
        check(budget, cancelled)?;
        Ok((self.result, append))
    }
}

fn collect_pages(
    tree: &Tree,
    pages: &mut Vec<u32>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    reader
        .budget
        .charge_allocation(1, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    pages
        .try_reserve_exact(1)
        .map_err(|_| InspectError::Allocation)?;
    pages.push(tree.number());
    for child in tree.children() {
        collect_pages(child, pages, reader)?;
    }
    Ok(())
}
