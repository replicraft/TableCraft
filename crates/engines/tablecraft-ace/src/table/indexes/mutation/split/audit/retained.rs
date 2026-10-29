//! Generic topology authority, re-derived independently from retained final keys.
use super::super::super::{retained::RetainedProfile, tree::Tree};
use super::super::{Plan, leaf_entries};
use super::{framing, maps, nonroot};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex,
    format::{page::PAGE_BYTES, reader::Reader},
    table::indexes::{
        header::IndexHeader,
        page::{self, KeyRef},
    },
};
use std::io::Cursor;

pub(super) struct Operation<'a, P: RetainedProfile> {
    pub image: &'a NativeImage,
    pub index: &'a NativeIndex,
    pub owner: u32,
    pub column: usize,
    pub profile: &'a P,
    pub tree: &'a Tree,
    pub keys: &'a [KeyRef<P::Key>],
}

pub(super) struct Progress<'a> {
    pub next_eof: &'a mut u32,
    pub map_edits: &'a mut maps::Expected,
    pub pages: &'a mut Vec<u32>,
    pub split_position: &'a mut usize,
}

pub(super) struct Disposition {
    pub split_root: bool,
    pub recursive: bool,
}

pub(super) fn prove<P: RetainedProfile, const N: usize>(
    operation: &Operation<'_, P>,
    plan: &Plan,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut Progress<'_>,
) -> Result<Disposition, EditError> {
    prove_with::<P, N>(operation, plan, actual, progress, false)
}

pub(super) fn prove_population<P: RetainedProfile, const N: usize>(
    operation: &Operation<'_, P>,
    plan: &Plan,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut Progress<'_>,
) -> Result<Disposition, EditError> {
    prove_with::<P, N>(operation, plan, actual, progress, true)
}

fn prove_with<P: RetainedProfile, const N: usize>(
    operation: &Operation<'_, P>,
    plan: &Plan,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut Progress<'_>,
    population: bool,
) -> Result<Disposition, EditError> {
    let image = operation.image;
    let index = operation.index;
    let profile = operation.profile;
    let cancelled = actual.cancelled;
    let mut source = image.reader();
    let mut split = false;
    let mut verified_nonroot = false;
    let original_root = image
        .as_bytes()
        .get(index.root_page() as usize * PAGE_BYTES..(index.root_page() as usize + 1) * PAGE_BYTES)
        .ok_or(InspectError::Malformed)?;
    let recursive = if operation.tree.is_leaf() {
        let entries = leaf_entries(operation.keys, actual.budget, cancelled)?;
        let overflowing = !nonroot::fits::<_, N>(
            &entries,
            usize::from(IndexHeader::read(original_root)?.prefix.get()),
            profile,
            actual,
        )?;
        if overflowing {
            if entries.len() < 2 {
                return Err(EditError::Preservation);
            }
            let middle = entries.len() / 2;
            let left_fits = nonroot::fits::<_, N>(&entries[..middle], 0, profile, actual)?;
            let right_fits = nonroot::fits::<_, N>(&entries[middle..], 0, profile, actual)?;
            split = left_fits && right_fits;
            !split
        } else {
            false
        }
    } else {
        true
    };
    if !recursive && !split {
        let entries = leaf_entries(operation.keys, actual.budget, cancelled)?;
        let decoded =
            page::read_profile::<_, _, N>(actual, index.root_page(), operation.owner, profile)?;
        if !decoded.leaf || decoded.tail != 0 || decoded.entries.len() != operation.keys.len() {
            return Err(EditError::Preservation);
        }
        for (entry, expected) in decoded.entries.iter().zip(operation.keys) {
            actual.tick()?;
            if entry.child != 0 || entry.value != *expected {
                return Err(EditError::Preservation);
            }
        }

        let current = actual.page(index.root_page())?;
        if population {
            nonroot::existing::<P, N>(
                original_root,
                &current,
                &entries,
                profile,
                operation.owner,
                actual,
            )?;
        }
        nonroot::layout::<_, N>(
            &entries,
            usize::from(IndexHeader::read(original_root)?.prefix.get()),
            &current,
            profile,
            actual,
        )?;
    }

    if recursive {
        let operation = nonroot::Operation {
            image,
            tree: operation.tree,
            keys: operation.keys,
            profile,
            owner: operation.owner,
            usage: index.usage_map(),
        };
        let mut replay = nonroot::Progress {
            next_eof: progress.next_eof,
            map_edits: progress.map_edits,
        };
        let added = if population {
            nonroot::prove_population::<_, N>(&operation, actual, &mut replay)?
        } else {
            nonroot::prove::<_, N>(&operation, actual, &mut replay)?
        };
        actual
            .budget
            .charge_allocation(added.len() as u64, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        progress
            .pages
            .try_reserve(added.len())
            .map_err(|_| InspectError::Allocation)?;
        progress.pages.extend(added);
        verified_nonroot = true;
    }

    if split {
        let (right, left) = {
            let mut reader = Reader {
                source: &mut source,
                header: image.catalog().header(),
                budget: actual.budget,
                cancelled,
                reads: 0,
            };
            let right = maps::reserve(
                &mut reader,
                index.usage_map(),
                progress.next_eof,
                progress.map_edits,
            )?;
            let left = maps::reserve(
                &mut reader,
                index.usage_map(),
                progress.next_eof,
                progress.map_edits,
            )?;
            (right, left)
        };
        let fact = plan
            .certificates
            .get(*progress.split_position)
            .ok_or(EditError::Preservation)?;
        if fact.physical != index.data_number()
            || fact.root != index.root_page()
            || fact.usage != index.usage_map()
            || fact.column != operation.column
            || fact.descending != index.columns()[0].descending()
            || fact.right != right
            || fact.left != left
            || fact.count != operation.keys.len()
            || fact.count < 2
            || progress.pages.as_slice() != [fact.root]
        {
            return Err(EditError::Preservation);
        }
        let middle = operation.keys.len() / 2;
        let root = page::read_profile::<_, _, { N }>(actual, fact.root, operation.owner, profile)?;
        let l = page::read_profile::<_, _, { N }>(actual, left, operation.owner, profile)?;
        let r = page::read_profile::<_, _, { N }>(actual, right, operation.owner, profile)?;
        if root.leaf
            || root.entries.len() != 1
            || root.entries[0].child != left
            || root.entries[0].value != operation.keys[middle - 1]
            || root.tail != right
            || !l.leaf
            || !r.leaf
            || l.entries.len() != middle
            || r.entries.len() != operation.keys.len() - middle
        {
            return Err(EditError::Preservation);
        }
        for (entry, expected) in l.entries.iter().chain(&r.entries).zip(operation.keys) {
            actual.tick()?;
            if entry.child != 0 || entry.value != *expected {
                return Err(EditError::Preservation);
            }
        }
        let root_raw = actual.page(fact.root)?;
        framing::root::<_, N>(
            original_root,
            &root_raw,
            fact,
            &operation.keys[middle - 1],
            profile,
            actual.budget,
            cancelled,
        )?;
        let left_raw = actual.page(left)?;
        framing::leaf::<_, N>(
            &left_raw,
            operation.owner,
            (0, right),
            &operation.keys[..middle],
            profile,
            actual,
        )?;
        let right_raw = actual.page(right)?;
        framing::leaf::<_, N>(
            &right_raw,
            operation.owner,
            (left, 0),
            &operation.keys[middle..],
            profile,
            actual,
        )?;
        actual
            .budget
            .charge_allocation(2, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        progress
            .pages
            .try_reserve(2)
            .map_err(|_| InspectError::Allocation)?;
        progress.pages.extend([right, left]);
        *progress.split_position += 1;
    }

    Ok(Disposition {
        split_root: split,
        recursive: verified_nonroot,
    })
}
