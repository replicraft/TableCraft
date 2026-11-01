//! Fitting original-partition deletion with no contraction or allocation state.
use super::super::{
    IndexDataPatch, encoding,
    profile::SingleProfile,
    tree::{Deletion, Tree},
};
use crate::{
    EditError, NativeImage, NativeIndex, WriteBlocker, format::reader::Reader, header::check,
};
use std::io::Cursor;

pub(super) fn patches(
    image: &NativeImage,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    index: &NativeIndex,
    owner: u32,
    references: &[u32],
    profile: &SingleProfile,
    patches: &mut Vec<IndexDataPatch>,
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
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
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
        patches,
        reader.budget,
        reader.cancelled,
    )?;
    if !remaining.is_empty() || deletion.removed != references.len() {
        return Err(EditError::Preservation);
    }
    check(reader.budget, reader.cancelled)?;
    Ok(())
}
