//! Independent recursive INSERT routing, normalization and complete wire proof.
mod frames;
mod normalize;
mod source;
#[cfg(test)]
mod tests;
mod write;

use super::super::{ImageProof, copied, reserve};
use super::split::PromotionExpected;
use super::{WIRE_BYTES, page_range};
use crate::{
    EditError, NativeImage, NativeIndex,
    format::{page::PAGE_BYTES, reader::Reader},
    table::indexes::mutation::insertion::PrimaryInsertion,
};
use std::io::Cursor;

pub(super) struct Change {
    pages: Vec<u32>,
    indexes: Vec<u32>,
    begin: u32,
    physical: u32,
    usage: u32,
    promoted: Option<PromotionExpected>,
    rebased: Option<u32>,
}

pub(super) fn original(
    image: &NativeImage,
    index: &NativeIndex,
    definition: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<[u8; WIRE_BYTES]>, EditError> {
    Ok(source::Source::capture(image, index, definition, reader)?.entries)
}

pub(super) fn derive(
    image: &NativeImage,
    index: &NativeIndex,
    definition: u32,
    entries: &[[u8; WIRE_BYTES]],
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Change, EditError> {
    reader.tick()?;
    if !std::ptr::eq(proof.original, image.as_bytes())
        || proof.bytes.len() < proof.original.len()
        || !proof.bytes.len().is_multiple_of(PAGE_BYTES)
    {
        return Err(EditError::Preservation);
    }
    let source = source::Source::capture(image, index, definition, reader)?;
    source::membership(&source.entries, entries, reader)?;
    let normalized = normalize::prepare(&source, entries, reader)?;
    write::derive(image, index, &source, entries, &normalized, proof, reader)
}

impl Change {
    pub(super) fn index_pages(&self) -> &[u32] {
        &self.indexes
    }

    pub(super) fn promotion(&self) -> Option<&PromotionExpected> {
        self.promoted.as_ref()
    }

    pub(super) fn compare(
        &self,
        actual: &PrimaryInsertion,
        proof: &ImageProof<'_>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        write::compare(self, actual, proof, reader)
    }
}

fn raw(image: &NativeImage, number: u32) -> Result<&[u8], EditError> {
    image
        .as_bytes()
        .get(page_range(number)?)
        .ok_or(EditError::Preservation)
}

fn copied_numbers(
    values: &[u32],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u32>, EditError> {
    reader.work(values.len())?;
    let mut result = Vec::new();
    reserve(&mut result, values.len(), reader)?;
    result.extend_from_slice(values);
    Ok(result)
}
