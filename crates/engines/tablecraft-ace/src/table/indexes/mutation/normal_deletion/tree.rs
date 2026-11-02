//! Original-partition deletion and qualified child retirement.
mod recursive;
use super::super::{
    encoding,
    profile::SingleProfile,
    retirement,
    tree::{Contraction, Deletion, Tree},
};
use super::Deletions;
use crate::{
    EditError, NativeImage, NativeIndex, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    table::indexes::{header::IndexHeader, profile::KeyType},
};
use std::io::Cursor;

pub(super) fn patches(
    image: &NativeImage,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    index: &NativeIndex,
    owner: u32,
    references: &[u32],
    profile: &SingleProfile,
    output: &mut Deletions,
) -> Result<(), EditError> {
    let mut keys = Vec::new();
    let tree = Tree::read::<_, { encoding::MAX_ENTRY_BYTES }>(
        image,
        reader,
        owner,
        index.root_page(),
        profile,
        0,
        &mut keys,
    )?;
    if references.is_empty() {
        // Validate every original page, but preserve even loose valid bounds.
        check(reader.budget, reader.cancelled)?;
        return Ok(());
    }

    let mut original = keys.as_slice();
    let contraction =
        tree.needs_contraction(&mut original, references, reader.budget, reader.cancelled)?;
    if !original.is_empty() {
        return Err(EditError::Preservation);
    }
    if contraction {
        if tree.height() == 2 {
            return recursive::patches(
                image,
                reader,
                recursive::Operation {
                    tree,
                    keys,
                    index,
                    references,
                    profile,
                },
                output,
            );
        }
        let start = index.root_page() as usize * PAGE_BYTES;
        let raw = image
            .as_bytes()
            .get(start..start + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        let header = IndexHeader::read(raw)?;
        reader.work(keys.len() + tree.children().len())?;
        if tree.height() != 1
            || tree.children().len() != 2
            || tree.children().iter().any(|child| child.count() == 0)
            || header.prefix.get() != 0
            || header.tail.get() != tree.children()[1].number()
            || profile.descending
            || !matches!(profile.kind.key_type(), KeyType::Int32)
            || keys.iter().any(|key| key.key.is_none())
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
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
        tree.contract::<_, { encoding::MAX_ENTRY_BYTES }>(
            image,
            &mut remaining,
            &mut state,
            &mut output.patches,
            reader.budget,
            reader.cancelled,
        )?;
        if !remaining.is_empty() || state.removed != references.len() {
            return Err(EditError::Preservation);
        }
        retirement::leaf_links(
            image,
            &state.leaves,
            &mut output.patches,
            reader.budget,
            reader.cancelled,
        )?;
        check(reader.budget, reader.cancelled)?;
        return Ok(());
    }

    let mut remaining = keys.as_slice();
    let mut deletion = Deletion {
        profile,
        references,
        removed: 0,
    };
    tree.delete::<_, { encoding::MAX_ENTRY_BYTES }>(
        image,
        &mut remaining,
        &mut deletion,
        &mut output.patches,
        reader.budget,
        reader.cancelled,
    )?;
    if !remaining.is_empty() || deletion.removed != references.len() {
        return Err(EditError::Preservation);
    }
    check(reader.budget, reader.cancelled)?;
    Ok(())
}
