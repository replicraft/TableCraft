//! Cardinality-changing root-leaf edits reuse the existing physical key codecs.
use super::super::{NativeIndex, header::IndexHeader, page};
use super::{IndexDataPatch, profile::WriteProfile, repack};
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
    patches: &mut Vec<IndexDataPatch>,
) -> Result<(), EditError> {
    let mut root = page::read_profile::<_, _, N>(reader, index.root_page(), owner, profile)?;
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
        patches,
        reader.budget,
        reader.cancelled,
    )
}
