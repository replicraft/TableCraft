//! Bounded native long-value storage shared by MEMO and opaque OLE bytes.
mod check;
mod definition;
pub(in crate::table) mod maps;
mod model;
pub(in crate::table) mod profile;
pub(in crate::table) mod trace;
pub use check::check_table_long_values;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use check::check_table_long_values_for_initialization;
pub use model::{
    LongValueCheck, LongValueCheckStatus, LongValueCounts, LongValueMapReferences,
    TableLongValueCheck,
};
#[cfg(test)]
mod tests;
mod visited;
use crate::{
    InspectError, LongValueReadLimits, NativeReadSource,
    format::{
        bytes::u32_at,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_range},
        row::{DELETED_MASK, RELOCATED_MASK, reference_page, reference_slot},
    },
};
use definition::Definition;
use visited::Visited;

const DEFINITION_BYTES: usize = size_of::<Definition>();
const LENGTH_TYPE_MASK: u32 = 0xc000_0000;
const INLINE: u32 = 0x8000_0000;
const SINGLE_ROW: u32 = 0x4000_0000;
const CHAINED: u32 = 0;
const CHAIN_REFERENCE_BYTES: usize = size_of::<u32>();
const LVAL_SIGNATURE: &[u8] = b"LVAL";
// Explicit larger reads use finite batches under the same cumulative budget.
const EXTENDED_BATCH_CHUNKS: usize = 256;

/// An empty definition denotes NULL; an inline zero-length payload is empty data.
pub(super) fn read<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    definition: &[u8],
) -> Result<Option<Vec<u8>>, InspectError> {
    read_with_trace(reader, definition, None)
}

fn read_with_trace<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    definition: &[u8],
    trace: Option<&mut Vec<trace::Chunk>>,
) -> Result<Option<Vec<u8>>, InspectError> {
    reader.tick()?;
    if definition.is_empty() {
        return Ok(None);
    }
    let descriptor = Definition::read(definition)?;
    let tagged = descriptor.tagged_length.get();
    let length = (tagged & !LENGTH_TYPE_MASK) as usize;
    let limits = reader.source.long_value_limits();
    if length > limits.maximum_bytes() as usize {
        return Err(InspectError::StructureLimit);
    }
    reader.budget.charge_allocation(length as u64, 1)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    match tagged & LENGTH_TYPE_MASK {
        INLINE => {
            if definition.len() != DEFINITION_BYTES + length {
                return Err(InspectError::Malformed);
            }
            bytes.extend_from_slice(&definition[DEFINITION_BYTES..]);
        }
        kind @ (SINGLE_ROW | CHAINED) => {
            if definition.len() != DEFINITION_BYTES || length == 0 {
                return Err(InspectError::Malformed);
            }
            if limits.maximum_bytes() > LongValueReadLimits::DEFAULT_BYTES {
                let mut value_reader = Reader {
                    source: &mut *reader.source,
                    header: reader.header,
                    budget: &mut *reader.budget,
                    cancelled: reader.cancelled,
                    reads: 0,
                };
                read_external(
                    &mut value_reader,
                    descriptor.reference.get(),
                    kind,
                    length,
                    &mut bytes,
                    trace,
                )?;
            } else {
                read_external(
                    reader,
                    descriptor.reference.get(),
                    kind,
                    length,
                    &mut bytes,
                    trace,
                )?;
            }
        }
        _ => return Err(InspectError::Unsupported),
    }
    reader.tick()?;
    Ok(Some(bytes))
}

fn read_external<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    mut reference: u32,
    kind: u32,
    length: usize,
    bytes: &mut Vec<u8>,
    mut trace: Option<&mut Vec<trace::Chunk>>,
) -> Result<(), InspectError> {
    let limits = reader.source.long_value_limits();
    let extended = limits.maximum_bytes() > LongValueReadLimits::DEFAULT_BYTES;
    let mut visited = Visited::new(extended);

    for hop in 0..limits.maximum_chunks() {
        visited.insert(reference, hop, reader)?;
        if extended && hop % EXTENDED_BATCH_CHUNKS == 0 {
            reader.reads = 0;
        }

        let current = reference;
        let page = reader.page(reference_page(reference))?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || page.get(OWNER_OFFSET..OWNER_OFFSET + LVAL_SIGNATURE.len()) != Some(LVAL_SIGNATURE)
        {
            return Err(InspectError::Malformed);
        }
        let (flags, start, end) = row_range(&page, usize::from(reference_slot(reference)))?;
        if flags & (DELETED_MASK | RELOCATED_MASK) != 0 {
            return Err(InspectError::Malformed);
        }

        let row = &page[start..end];
        let payload = if kind == CHAINED {
            reference = u32_at(row, 0)?;
            row.get(CHAIN_REFERENCE_BYTES..)
                .ok_or(InspectError::Truncated)?
        } else {
            reference = 0;
            row
        };
        let remaining = length - bytes.len();
        if payload.is_empty() || payload.len() > remaining {
            return Err(InspectError::Malformed);
        }
        if let Some(chunks) = trace.as_deref_mut() {
            reader
                .budget
                .charge_allocation(1, size_of::<trace::Chunk>() as u64)?;
            chunks
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            chunks.push(trace::Chunk {
                reference: current,
                offset: reference_page(current) as usize * PAGE_BYTES + start,
                bytes: row.len(),
            });
        }
        bytes.extend_from_slice(payload);
        if bytes.len() == length {
            if reference != 0 {
                return Err(InspectError::Malformed);
            }
            return Ok(());
        }
        if kind == SINGLE_ROW {
            return Err(InspectError::Truncated);
        }
    }

    Err(InspectError::StructureLimit)
}
