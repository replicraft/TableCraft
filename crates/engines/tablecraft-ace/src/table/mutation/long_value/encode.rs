//! Raw Memo/OLE payloads and exact qualified descriptor/chunk framing.
use super::super::{EditError, WriteBlocker};
use crate::{
    InspectError, NativeColumn,
    format::{
        data_type::{LONG_TEXT, OLE},
        layout::UPDATABLE_FLAG,
        text,
    },
    header::check,
    limits::{MAX_LONG_TEXT_BYTES, MAX_LONG_VALUE_BYTES, MAX_LONG_VALUE_CHUNKS},
};
use std::ops::Range;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const INLINE_BYTES: usize = 64;
const EXTERNAL_ROW_BYTES: usize = 4076;
const REFERENCE_BYTES: usize = size_of::<u32>();
const CHAIN_PAYLOAD_BYTES: usize = EXTERNAL_ROW_BYTES - REFERENCE_BYTES;
const DESCRIPTOR_WORDS: usize = 3;
const DESCRIPTOR_BYTES: usize = DESCRIPTOR_WORDS * size_of::<u32>();
const INLINE_TAG: u32 = 0x8000_0000;
const SINGLE_TAG: u32 = 0x4000_0000;
const ENCODING_CHECK_UNITS: usize = 256;

#[cfg(test)]
mod tests;

pub(super) enum Encoded {
    Null,
    Present(Vec<u8>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StorageChoice {
    Inline,
    Single,
    Chained,
}

pub(super) struct Chunk {
    pub payload: Range<usize>,
    pub linked: bool,
}

pub(super) fn qualify(field: &NativeColumn) -> Result<(), EditError> {
    if field.fixed()
        || !matches!(field.native_type(), LONG_TEXT | OLE)
        || field.storage_length() != 0
        || field.field.flags != UPDATABLE_FLAG
        || field.field.extended_flags != 0
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }

    Ok(())
}

pub(super) fn encode(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Encoded, EditError> {
    check(budget, cancelled)?;
    qualify(field)?;

    let length = match value {
        Value::Null => return Ok(Encoded::Null),
        Value::Binary(bytes) if field.native_type() == OLE => bytes.len(),
        Value::Text(value) if field.native_type() == LONG_TEXT => {
            if value.len() > MAX_LONG_TEXT_BYTES {
                return Err(InspectError::StructureLimit.into());
            }
            budget
                .charge(Resource::Steps, value.len() as u64)
                .map_err(InspectError::from)?;
            let mut units = 0_usize;
            for _ in value.encode_utf16() {
                if units.is_multiple_of(ENCODING_CHECK_UNITS) {
                    check(budget, cancelled)?;
                }
                units += 1;
            }
            units
                .checked_mul(size_of::<u16>())
                .ok_or(InspectError::StructureLimit)?
        }
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    if length > MAX_LONG_VALUE_BYTES {
        return Err(InspectError::StructureLimit.into());
    }

    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, length as u64)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;

    match value {
        Value::Binary(value) => {
            for chunk in value.chunks(ENCODING_CHECK_UNITS) {
                check(budget, cancelled)?;
                bytes.extend_from_slice(chunk);
            }
        }
        Value::Text(value) => {
            for (position, unit) in value.encode_utf16().enumerate() {
                if position.is_multiple_of(ENCODING_CHECK_UNITS) {
                    check(budget, cancelled)?;
                }
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
            let decoded = text::decode_long_with_limits(
                &bytes,
                MAX_LONG_VALUE_BYTES,
                MAX_LONG_TEXT_BYTES,
                budget,
                cancelled,
            )?;
            budget
                .charge(Resource::Steps, value.len() as u64)
                .map_err(InspectError::from)?;
            if decoded != *value {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    }

    check(budget, cancelled)?;
    Ok(Encoded::Present(bytes))
}

impl Encoded {
    pub fn preferred(&self) -> StorageChoice {
        match self {
            Self::Null => StorageChoice::Inline,
            Self::Present(bytes) if bytes.len() <= INLINE_BYTES => StorageChoice::Inline,
            Self::Present(bytes) if bytes.len() <= EXTERNAL_ROW_BYTES => StorageChoice::Single,
            Self::Present(_) => StorageChoice::Chained,
        }
    }

    fn validate_choice(&self, choice: StorageChoice) -> Result<(), EditError> {
        let valid = match self {
            Self::Null => choice == StorageChoice::Inline,
            Self::Present(bytes) => match choice {
                StorageChoice::Inline => bytes.len() <= INLINE_BYTES,
                StorageChoice::Single => !bytes.is_empty() && bytes.len() <= EXTERNAL_ROW_BYTES,
                StorageChoice::Chained => bytes.len() > EXTERNAL_ROW_BYTES,
            },
        };
        if !valid {
            return Err(EditError::Preservation);
        }

        Ok(())
    }

    pub fn chunks(
        &self,
        choice: StorageChoice,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<Chunk>, EditError> {
        check(budget, cancelled)?;
        self.validate_choice(choice)?;
        let Self::Present(bytes) = self else {
            return Ok(Vec::new());
        };
        if choice == StorageChoice::Inline {
            return Ok(Vec::new());
        }

        let linked = choice == StorageChoice::Chained;
        let capacity = if linked {
            CHAIN_PAYLOAD_BYTES
        } else {
            EXTERNAL_ROW_BYTES
        };
        let count = bytes.len().div_ceil(capacity);
        if count > MAX_LONG_VALUE_CHUNKS {
            return Err(InspectError::StructureLimit.into());
        }
        budget
            .charge_allocation(count as u64, size_of::<Chunk>() as u64)
            .map_err(InspectError::from)?;
        let mut chunks = Vec::new();
        chunks
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        let mut start = 0;
        while start < bytes.len() {
            check(budget, cancelled)?;
            let end = start.saturating_add(capacity).min(bytes.len());
            chunks.push(Chunk {
                payload: start..end,
                linked,
            });
            start = end;
        }

        Ok(chunks)
    }

    pub fn descriptor(
        &self,
        choice: StorageChoice,
        first: Option<u32>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EditError> {
        check(budget, cancelled)?;
        self.validate_choice(choice)?;
        let Self::Present(bytes) = self else {
            if first.is_some() {
                return Err(EditError::Preservation);
            }
            return Ok(Vec::new());
        };
        let inline = choice == StorageChoice::Inline;
        if inline != first.is_none() || first == Some(0) {
            return Err(EditError::Preservation);
        }
        let length = DESCRIPTOR_BYTES + if inline { bytes.len() } else { 0 };
        budget
            .charge_allocation(length as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, length as u64)
            .map_err(InspectError::from)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        let tag = match choice {
            StorageChoice::Inline => INLINE_TAG,
            StorageChoice::Single => SINGLE_TAG,
            StorageChoice::Chained => 0,
        };
        let length = u32::try_from(bytes.len()).map_err(|_| InspectError::StructureLimit)?;
        result.extend_from_slice(&(tag | length).to_le_bytes());
        result.extend_from_slice(&first.unwrap_or(0).to_le_bytes());
        result.extend_from_slice(&0_u32.to_le_bytes());
        if inline {
            result.extend_from_slice(bytes);
        }

        Ok(result)
    }

    pub fn body(
        &self,
        chunk: &Chunk,
        next: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EditError> {
        check(budget, cancelled)?;
        let Self::Present(bytes) = self else {
            return Err(EditError::Preservation);
        };
        if !chunk.linked && next != 0 {
            return Err(EditError::Preservation);
        }
        let payload = bytes
            .get(chunk.payload.clone())
            .filter(|payload| !payload.is_empty())
            .ok_or(EditError::Preservation)?;
        let length = payload.len() + if chunk.linked { REFERENCE_BYTES } else { 0 };
        if length > EXTERNAL_ROW_BYTES {
            return Err(EditError::Preservation);
        }
        budget
            .charge_allocation(length as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, length as u64)
            .map_err(InspectError::from)?;
        let mut body = Vec::new();
        body.try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        if chunk.linked {
            body.extend_from_slice(&next.to_le_bytes());
        }
        body.extend_from_slice(payload);

        Ok(body)
    }
}
