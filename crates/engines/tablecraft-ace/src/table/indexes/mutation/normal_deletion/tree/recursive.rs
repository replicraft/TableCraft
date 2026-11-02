//! Source-qualified height-two pruning using the real retained tree producer.
use super::super::super::{
    encoding,
    profile::SingleProfile,
    retirement,
    tree::{Contraction, Tree},
};
use super::super::Deletions;
use crate::{
    EditError, NativeImage, NativeIndex, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    table::indexes::{header::IndexHeader, page::KeyRef, profile::KeyType},
};
use std::io::Cursor;

const PRESENT_ENTRY_BYTES: usize = size_of::<u8>() + size_of::<i32>() + size_of::<u32>();

pub(super) struct Operation<'a> {
    pub tree: Tree,
    pub keys: Vec<KeyRef<Option<i64>>>,
    pub index: &'a NativeIndex,
    pub references: &'a [u32],
    pub profile: &'a SingleProfile,
}

fn header<'a>(image: &'a NativeImage, tree: &Tree) -> Result<&'a IndexHeader, EditError> {
    let base = tree.number() as usize * PAGE_BYTES;
    let raw = image
        .as_bytes()
        .get(base..base + PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    Ok(IndexHeader::read(raw)?)
}

pub(super) fn patches(
    image: &NativeImage,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    operation: Operation<'_>,
    output: &mut Deletions,
) -> Result<(), EditError> {
    let Operation {
        tree,
        keys,
        index,
        references,
        profile,
    } = operation;
    let root = header(image, &tree)?;
    reader.work(keys.len() + tree.children().len())?;
    if tree.height() != 2
        || tree.children().len() != 2
        || root.tail.get() != 0
        || root.prefix.get() != 0
        || profile.descending
        || !matches!(profile.kind.key_type(), KeyType::Int32)
        || keys.iter().any(|key| key.key.is_none())
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }

    let mut survivors = 0_usize;
    for key in &keys {
        reader.tick()?;
        reader.work(references.len())?;
        if !references.contains(&key.row) {
            survivors += 1;
        }
    }
    // This tranche qualifies retained, nonempty height-two roots only.
    if survivors == 0 {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    for parent in tree.children() {
        reader.tick()?;
        reader.work(parent.children().len())?;
        let raw = header(image, parent)?;
        if parent.height() != 1
            || parent.children().len() < 2
            || usize::from(raw.prefix.get()) > PRESENT_ENTRY_BYTES
            || raw.tail.get()
                != parent
                    .children()
                    .last()
                    .ok_or(EditError::Preservation)?
                    .number()
            || parent
                .children()
                .iter()
                .any(|child| !child.is_leaf() || child.count() == 0)
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }

    let mut siblings = tree.sibling_chains(image, index, reader.budget, reader.cancelled)?;
    let mut remaining = keys.as_slice();
    let mut state = Contraction {
        bounded_single_child: true,
        profile,
        references,
        index,
        retired: &mut output.retired,
        leaves: Vec::new(),
        removed: 0,
    };
    let maximum = tree.contract::<_, { encoding::MAX_ENTRY_BYTES }>(
        image,
        &mut remaining,
        &mut state,
        &mut output.patches,
        reader.budget,
        reader.cancelled,
    )?;
    if maximum.is_none() || !remaining.is_empty() || state.removed != references.len() {
        return Err(EditError::Preservation);
    }
    retirement::leaf_links(
        image,
        &state.leaves,
        &mut output.patches,
        reader.budget,
        reader.cancelled,
    )?;
    siblings.repair(
        image,
        false,
        &output.retired,
        &mut output.patches,
        reader.budget,
        reader.cancelled,
    )?;
    check(reader.budget, reader.cancelled)?;
    Ok(())
}
