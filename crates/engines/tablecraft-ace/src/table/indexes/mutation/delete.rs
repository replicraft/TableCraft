//! Cardinality changes retain every original nonempty leaf partition and link.
use super::super::{NativeIndex, header::IndexHeader, page};
use super::{
    PatchSet,
    profile::WriteProfile,
    repack,
    tree::{Contraction, Deletion, Tree},
};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
};

pub(super) fn edit<P: WriteProfile, const N: usize>(
    image: &NativeImage,
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
    index: &NativeIndex,
    owner: u32,
    references: &[u32],
    profile: &P,
    patches: &mut PatchSet,
) -> Result<(), EditError> {
    let mut root = page::read_profile::<_, _, N>(reader, index.root_page(), owner, profile)?;
    if !root.leaf {
        // Keep the established root-leaf path and its allocation behavior. A
        // node tree needs the separate original-partition deletion traversal.
        drop(root);
        return edit_tree::<P, N>(image, reader, index, owner, references, profile, patches);
    }
    let offset = (index.root_page() as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let end = offset
        .checked_add(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let original = image
        .bytes
        .get(offset..end)
        .ok_or(InspectError::Truncated)?;
    if !root.leaf
        || root.previous != 0
        || root.next != 0
        || root.tail != 0
        || !IndexHeader::read(original)?.qualified_for_update(0)
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    // Every ordinary root has one physical entry in this admitted non-omitting
    // index. Complete before checks already established row/key agreement.
    let mut removed = 0;
    let mut retained = 0;
    for position in 0..root.entries.len() {
        reader.tick()?;
        reader.work(references.len())?;
        if references.contains(&root.entries[position].value.row) {
            removed += 1;
        } else {
            root.entries.swap(retained, position);
            retained += 1;
        }
    }
    if removed != references.len() {
        return Err(EditError::Preservation);
    }
    root.entries.truncate(retained);
    if references.is_empty() {
        return Ok(());
    }
    let after = repack::page::<P, N>(
        original,
        &root.entries,
        profile,
        reader.budget,
        reader.cancelled,
    )?;
    repack::patches(
        original,
        &after,
        index.root_page(),
        &mut patches.indexes,
        reader.budget,
        reader.cancelled,
    )
}

fn edit_tree<P: WriteProfile, const N: usize>(
    image: &NativeImage,
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
    index: &NativeIndex,
    owner: u32,
    references: &[u32],
    profile: &P,
    patches: &mut PatchSet,
) -> Result<(), EditError> {
    let mut keys = Vec::new();
    let tree = Tree::read::<P, N>(
        image,
        reader,
        owner,
        index.root_page(),
        profile,
        0,
        &mut keys,
    )?;
    if references.is_empty() {
        // A no-op still qualifies every original tree page, but must not
        // canonicalize otherwise unchanged valid page encodings.
        reader.tick()?;
        return Ok(());
    }
    let mut original = keys.as_slice();
    let contraction =
        tree.needs_contraction(&mut original, references, reader.budget, reader.cancelled)?;
    if !original.is_empty() {
        return Err(EditError::Preservation);
    }
    if contraction {
        let mut remaining = keys.as_slice();
        let mut state = Contraction {
            profile,
            references,
            index,
            retired: &mut patches.retired,
            leaves: Vec::new(),
            removed: 0,
        };
        tree.contract::<P, N>(
            image,
            &mut remaining,
            &mut state,
            &mut patches.indexes,
            reader.budget,
            reader.cancelled,
        )?;
        if !remaining.is_empty() || state.removed != references.len() {
            return Err(EditError::Preservation);
        }
        super::retirement::leaf_links(
            image,
            &state.leaves,
            &mut patches.indexes,
            reader.budget,
            reader.cancelled,
        )?;
        return Ok(());
    }
    let mut remaining = keys.as_slice();
    let mut state = Deletion {
        profile,
        references,
        removed: 0,
    };
    tree.delete::<P, N>(
        image,
        &mut remaining,
        &mut state,
        &mut patches.indexes,
        reader.budget,
        reader.cancelled,
    )?;
    if !remaining.is_empty() || state.removed != references.len() {
        return Err(EditError::Preservation);
    }
    reader.tick()?;
    Ok(())
}
