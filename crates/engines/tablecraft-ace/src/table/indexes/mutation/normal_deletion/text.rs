//! Retain original Text wires while removing exact existing root references.
use super::super::{IndexDataPatch, leaf_removal, repack, retained::Preserved};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableSchema, WriteBlocker,
    format::{data_type::SHORT_TEXT, page::PAGE_BYTES, reader::Reader},
    table::indexes::{
        header::IndexHeader,
        page,
        text::{self, TextProfile},
    },
};
use std::io::Cursor;

pub(super) fn patches(
    image: &NativeImage,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    index: &NativeIndex,
    schema: &TableSchema,
    column: usize,
    references: &[u32],
    result: &mut Vec<IndexDataPatch>,
) -> Result<(), EditError> {
    let [key] = index.columns() else {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    };
    let field = schema
        .columns()
        .get(column)
        .ok_or(InspectError::Malformed)?;
    if field.number() != key.number() {
        return Err(EditError::Preservation);
    }
    if field.native_type() != SHORT_TEXT || field.fixed() || field.extended_flags() != 0 {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let profile = Preserved(TextProfile::new(column, field, key.descending())?);
    let mut root = page::read_profile::<_, _, { text::MAX_ENTRY_BYTES }>(
        reader,
        index.root_page(),
        schema.definition_page(),
        &profile,
    )?;
    let start = (index.root_page() as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let original = image
        .as_bytes()
        .get(start..start + PAGE_BYTES)
        .ok_or(InspectError::Truncated)?;
    leaf_removal::require_root(&root, original)?;
    if IndexHeader::read(original)?.prefix.get() != 0 {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    leaf_removal::remove(
        &mut root.entries,
        references,
        reader.budget,
        reader.cancelled,
    )?;
    if references.is_empty() {
        return Ok(());
    }
    let after = repack::retained_page::<_, { text::MAX_ENTRY_BYTES }>(
        original,
        &root.entries,
        &profile,
        reader.budget,
        reader.cancelled,
    )?;
    repack::patches(
        original,
        &after,
        index.root_page(),
        result,
        reader.budget,
        reader.cancelled,
    )
}
