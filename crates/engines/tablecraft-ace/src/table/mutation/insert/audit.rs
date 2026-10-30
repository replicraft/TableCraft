//! Request-bound INSERT proof, separate from construction and composition.
mod data;
mod defaults;
mod global;
mod long_values;
mod primary;
mod records;
#[cfg(test)]
mod tests;

use super::{
    AuditRequest, AutoNumberRowInsert, IncrementingAutoNumber, RowInsert, Staged, autonumber,
    omitted::Original, policies, primary_column,
};
use crate::{
    EditError, InspectError, NativeImage,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    limits::MAX_LIVE_ROWS,
    table::mutation::{
        gates, preview, variable_split::metadata::Metadata,
        variable_update::map_audit::ExpectedByte,
    },
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::functions::dates::clock::ClockReading;

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
    global: global::Replay<'a>,
}

impl ImageProof<'_> {
    fn append(
        &mut self,
        number: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        self.global.append(
            &mut self.bytes,
            &mut self.map_edits,
            &self.qualified_zero_columns,
            number,
            reader,
        )
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
    prove_with(
        image,
        table,
        AuditRequest::Explicit(requests),
        staged,
        after,
        budget,
        cancelled,
    )
}

pub(super) fn prove_incrementing(
    image: &NativeImage,
    table: &str,
    requests: (&[AutoNumberRowInsert], IncrementingAutoNumber),
    staged: &Staged,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    prove_with(
        image,
        table,
        AuditRequest::Incrementing {
            requests: requests.0,
            policy: requests.1,
        },
        staged,
        after,
        budget,
        cancelled,
    )
}

pub(super) fn prove_with(
    image: &NativeImage,
    table: &str,
    input: AuditRequest<'_>,
    staged: &Staged,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    prove_with_clock(image, table, input, staged, after, None, budget, cancelled)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Original operation clock is separate from staged producer facts"
)]
pub(super) fn prove_with_clock(
    image: &NativeImage,
    table: &str,
    input: AuditRequest<'_>,
    staged: &Staged,
    after: &[u8],
    reading: Option<ClockReading>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if reading.is_some() && !matches!(input, AuditRequest::Omitted(_)) {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    preview::qualify(image)?;
    let (checked, count) = match input {
        AuditRequest::Explicit(requests) => {
            if staged.autonumber.is_some() {
                return Err(EditError::Preservation);
            }
            (
                gates::before_insert(image, table, budget, cancelled)?,
                requests.len(),
            )
        }
        AuditRequest::Incrementing { requests, .. } => (
            gates::before_incrementing_insert(image, table, budget, cancelled)?,
            requests.len(),
        ),
        AuditRequest::Omitted(Original::Ordinary(requests)) => {
            if staged.autonumber.is_some() {
                return Err(EditError::Preservation);
            }
            (
                gates::before_insert(image, table, budget, cancelled)?,
                requests.len(),
            )
        }
        AuditRequest::Omitted(Original::Incrementing { requests, .. }) => (
            gates::before_incrementing_insert(image, table, budget, cancelled)?,
            requests.len(),
        ),
    };
    if checked
        .rows()
        .checked_add(count)
        .is_none_or(|count| count > MAX_LIVE_ROWS)
    {
        return Err(InspectError::StructureLimit.into());
    }
    let schema = checked.schema();
    if matches!(
        input,
        AuditRequest::Explicit(_) | AuditRequest::Omitted(Original::Ordinary(_))
    ) {
        records::qualify(schema, budget, cancelled)?;
    }
    let id = primary_column::resolve(&checked, budget, cancelled)?;
    let defaults = match input {
        AuditRequest::Omitted(original) => Some(defaults::derive(
            image, &checked, original, reading, budget, cancelled,
        )?),
        _ => None,
    };
    let generation_input = match input {
        AuditRequest::Incrementing { requests, policy } => Some((requests, policy)),
        AuditRequest::Omitted(Original::Incrementing { policy, .. }) => Some((
            defaults
                .as_ref()
                .ok_or(EditError::Preservation)?
                .companions()?,
            policy,
        )),
        _ => None,
    };
    let generation = if let Some((requests, policy)) = generation_input {
        records::layout::capture_incrementing(schema, id, budget, cancelled)?;
        let plan = staged.autonumber.as_ref().ok_or(EditError::Preservation)?;
        Some(autonumber::audit::Operation {
            image,
            checked: &checked,
            requests,
            policy,
            plan,
            returned: &plan.values,
        })
    } else {
        None
    };
    let expanded = generation
        .as_ref()
        .map(|operation| operation.rows(budget, cancelled))
        .transpose()?;
    let requests = match input {
        AuditRequest::Explicit(requests) => requests,
        AuditRequest::Incrementing { .. }
        | AuditRequest::Omitted(Original::Incrementing { .. }) => {
            expanded.as_deref().ok_or(EditError::Preservation)?
        }
        AuditRequest::Omitted(Original::Ordinary(_)) => defaults
            .as_ref()
            .ok_or(EditError::Preservation)?
            .ordinary()?,
    };
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
        global: global::Replay::new(image, schema.definition_page()),
    };

    let generated_column = generation.as_ref().map(|_| id);
    let resolved = long_values::derive(
        image,
        &checked,
        requests,
        generated_column,
        staged,
        &mut proof,
        &mut reader,
    )?;
    let metadata = Metadata::capture_for_insert_initialization(
        image,
        schema,
        checked.indexes(),
        &proof.qualified_zero_columns,
        reader.budget,
        reader.cancelled,
    )?;
    let mut rows = if generation.is_some() {
        records::derive_incrementing(schema, id, requests, resolved.as_deref(), &mut reader)?
    } else {
        records::derive(schema, id, requests, resolved.as_deref(), &mut reader)?
    };

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

    proof.global.compare(staged, &proof.bytes, &mut reader)?;
    if let Some(operation) = &generation {
        operation.apply(&mut proof.bytes, reader.budget, reader.cancelled)?;
    }

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
