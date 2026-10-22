//! Bounded native long-value storage shared by MEMO and opaque OLE bytes.
mod definition;
#[cfg(test)]
mod tests;
use crate::{
    InspectError,
    format::{
        bytes::u32_at,
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        reader::{Reader, row_range},
        row::{DELETED_MASK, RELOCATED_MASK, reference_page, reference_slot},
    },
    limits::{MAX_LONG_VALUE_BYTES, MAX_LONG_VALUE_CHUNKS},
};
use definition::Definition;

const DEFINITION_BYTES: usize = size_of::<Definition>();
const LENGTH_TYPE_MASK: u32 = 0xc000_0000;
const INLINE: u32 = 0x8000_0000;
const SINGLE_ROW: u32 = 0x4000_0000;
const CHAINED: u32 = 0;
const CHAIN_REFERENCE_BYTES: usize = size_of::<u32>();
const LVAL_SIGNATURE: &[u8] = b"LVAL";

/// An empty definition denotes NULL; an inline zero-length payload is empty data.
pub(super) fn read<R: crate::NativeReadSource>(
    reader: &mut Reader<'_, R>,
    definition: &[u8],
) -> Result<Option<Vec<u8>>, InspectError> {
    reader.tick()?;
    if definition.is_empty() {
        return Ok(None);
    }
    let descriptor = Definition::read(definition)?;
    let tagged = descriptor.tagged_length.get();
    let length = (tagged & !LENGTH_TYPE_MASK) as usize;
    if length > MAX_LONG_VALUE_BYTES {
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
            let mut reference = descriptor.reference.get();
            let mut visited = [0_u32; MAX_LONG_VALUE_CHUNKS];
            let mut complete = false;
            for hop in 0..MAX_LONG_VALUE_CHUNKS {
                // Charge the bounded reference scan as chains grow; its
                // configured ceiling keeps the buffer and scan bounded.
                reader.work(hop + 1)?;
                if reference == 0 || visited[..hop].contains(&reference) {
                    return Err(InspectError::Malformed);
                }
                visited[hop] = reference;
                let page = reader.page(reference_page(reference))?;
                if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
                    || page.get(OWNER_OFFSET..OWNER_OFFSET + LVAL_SIGNATURE.len())
                        != Some(LVAL_SIGNATURE)
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
                bytes.extend_from_slice(payload);
                if bytes.len() == length {
                    if reference != 0 {
                        return Err(InspectError::Malformed);
                    }
                    complete = true;
                    break;
                }
                if kind == SINGLE_ROW {
                    return Err(InspectError::Truncated);
                }
            }
            if !complete {
                return Err(InspectError::StructureLimit);
            }
        }
        _ => return Err(InspectError::Unsupported),
    }
    reader.tick()?;
    Ok(Some(bytes))
}
