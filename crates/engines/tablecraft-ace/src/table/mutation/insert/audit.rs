//! Request-bound INSERT proof, separate from construction and composition.
mod data;
mod long_values;
mod primary;
mod records;
#[cfg(test)]
mod tests;

use super::{RowInsert, Staged, policies, primary_column};
use crate::{
    EditError, InspectError, NativeImage,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    limits::{MAX_IMAGE_BYTES, MAX_LIVE_ROWS},
    table::mutation::{
        gates, preview, variable_split::metadata::Metadata,
        variable_update::map_audit::ExpectedByte,
    },
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::ResourceBudget;

const COMPARISON_CHUNK: usize = 256;

struct Row {
    bytes: Vec<u8>,
    reference: u32,
    range: Range<usize>,
}

struct ImageProof<'a> {
    original: &'a [u8],
    bytes: Vec<u8>,
    map_edits: Vec<ExpectedByte>,
    qualified_zero_columns: Vec<usize>,
}

impl ImageProof<'_> {
    fn append(
        &mut self,
        number: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        if number as usize != self.bytes.len() / PAGE_BYTES {
            return Err(EditError::Preservation);
        }

        let length = self
            .bytes
            .len()
            .checked_add(PAGE_BYTES)
            .filter(|length| *length as u64 <= MAX_IMAGE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        reader
            .budget
            .charge_allocation(PAGE_BYTES as u64, 1)
            .map_err(InspectError::from)?;
        reader.work(PAGE_BYTES)?;
        self.bytes
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        self.bytes.resize(length, 0);

        Ok(())
    }
}

pub(super) fn prove(
    image: &NativeImage,
    table: &str,
    requests: &[RowInsert],
    staged: &Staged,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    preview::qualify(image)?;
    let checked = gates::before_insert(image, table, budget, cancelled)?;
    if checked
        .rows()
        .checked_add(requests.len())
        .is_none_or(|count| count > MAX_LIVE_ROWS)
    {
        return Err(InspectError::StructureLimit.into());
    }
    let schema = checked.schema();
    records::qualify(schema, budget, cancelled)?;
    let id = primary_column::resolve(&checked, budget, cancelled)?;
    policies::require_text_declarations(image, schema, requests, budget, cancelled)?;

    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let bytes = copied(image.as_bytes(), &mut reader)?;
    let mut proof = ImageProof {
        original: image.as_bytes(),
        bytes,
        map_edits: Vec::new(),
        qualified_zero_columns: Vec::new(),
    };

    let resolved = long_values::derive(image, &checked, requests, staged, &mut proof, &mut reader)?;
    let metadata = Metadata::capture_for_insert_initialization(
        image,
        schema,
        checked.indexes(),
        &proof.qualified_zero_columns,
        reader.budget,
        reader.cancelled,
    )?;
    let mut rows = records::derive(schema, id, requests, resolved.as_deref(), &mut reader)?;

    data::prove(
        &mut proof,
        schema,
        &metadata,
        &mut rows,
        &staged.data,
        &mut reader,
    )?;
    primary::prove(
        primary::Operation {
            image,
            checked: &checked,
            id,
            requests,
            rows: &rows,
            staged,
        },
        &mut proof,
        &mut reader,
    )?;

    if staged.data.append.length != proof.bytes.len()
        || staged.data.append.reservations_len()
            != (proof.bytes.len() - proof.original.len()) / PAGE_BYTES
        || staged.data.append.map_edits.len() != proof.map_edits.len()
    {
        return Err(EditError::Preservation);
    }
    for expected in &proof.map_edits {
        reader.work(staged.data.append.map_edits.len())?;
        let actual = staged
            .data
            .append
            .map_edits
            .iter()
            .find(|actual| actual.offset == expected.offset)
            .ok_or(EditError::Preservation)?;
        if actual.before != expected.before || actual.after != expected.after {
            return Err(EditError::Preservation);
        }
    }

    if after.len() != proof.bytes.len() {
        return Err(EditError::Preservation);
    }
    for (expected, actual) in proof
        .bytes
        .chunks(COMPARISON_CHUNK)
        .zip(after.chunks(COMPARISON_CHUNK))
    {
        reader.work(expected.len())?;
        if expected != actual {
            return Err(EditError::Preservation);
        }
    }

    reader.tick()?;
    Ok(())
}

fn copied(bytes: &[u8], reader: &mut Reader<'_, Cursor<&[u8]>>) -> Result<Vec<u8>, EditError> {
    reader
        .budget
        .charge_allocation(bytes.len() as u64, 1)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(bytes.len())
        .map_err(|_| InspectError::Allocation)?;
    for chunk in bytes.chunks(COMPARISON_CHUNK) {
        reader.work(chunk.len())?;
        result.extend_from_slice(chunk);
    }

    Ok(result)
}

fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    reader
        .budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;

    Ok(())
}
