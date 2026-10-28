//! Preflight complete variable payloads before reserving any long-value page.
use super::{field, payload};
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        page::PAGE_BYTES,
        row::{MASK_BITS, SLOT_BYTES, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES, slot_offset},
    },
    header::check,
    table::{
        long_value::profile as long_profile,
        mutation::long_value::encode::{self, Encoded, StorageChoice},
    },
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Guid, Value},
};

const DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();
const MAX_RECORD_BYTES: usize = PAGE_BYTES - slot_offset(1);

pub(in crate::table::mutation::insert) enum Payload {
    Bytes(Vec<u8>),
    Long {
        encoded: Encoded,
        choice: StorageChoice,
    },
}

impl Payload {
    fn length(&self) -> usize {
        match self {
            Self::Bytes(bytes) => bytes.len(),
            Self::Long {
                encoded: Encoded::Null,
                ..
            } => 0,
            Self::Long {
                encoded: Encoded::Present(bytes),
                choice,
            } => {
                DESCRIPTOR_BYTES
                    + if *choice == StorageChoice::Inline {
                        bytes.len()
                    } else {
                        0
                    }
            }
        }
    }
}

pub(in crate::table::mutation::insert) struct Input {
    pub payloads: Vec<Payload>,
}

pub(in crate::table::mutation::insert) fn prepare(
    schema: &TableSchema,
    variables: &[usize],
    values: &[Value],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Input, EditError> {
    check(budget, cancelled)?;
    if values.len() != schema.columns().len() {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    // Constructor and carrier refusals precede every external reservation,
    // including fields outside the selected index and ordinary fixed NULLs.
    for (position, column) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if column.fixed() {
            let width = field::width(column)?;
            let mut temporary = [0; Guid::BYTE_LEN];
            field::write(column, &values[position], &mut temporary[..width])?;
        }
    }
    budget
        .charge_allocation(variables.len() as u64, size_of::<Payload>() as u64)
        .map_err(InspectError::from)?;
    let mut payloads = Vec::new();
    payloads
        .try_reserve_exact(variables.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut length = schema.layout.fixed_end;
    for &position in variables {
        check(budget, cancelled)?;
        let field = &schema.columns()[position];
        let next = if long_profile::ordinary(field) {
            let encoded = encode::encode(field, &values[position], budget, cancelled)?;
            let choice = encoded.preferred();
            Payload::Long { encoded, choice }
        } else {
            Payload::Bytes(payload::encode(
                field,
                &values[position],
                budget,
                cancelled,
            )?)
        };
        length = length
            .checked_add(next.length())
            .ok_or(InspectError::StructureLimit)?;
        payloads.push(next);
    }
    let footer = if variables.is_empty() {
        0
    } else {
        VARIABLE_OFFSET_BYTES
            .checked_mul(variables.len() + 1)
            .and_then(|bytes| bytes.checked_add(VARIABLE_COUNT_BYTES))
            .ok_or(InspectError::StructureLimit)?
    };
    length = length
        .checked_add(footer)
        .and_then(|bytes| bytes.checked_add(schema.layout.max_columns.div_ceil(MASK_BITS)))
        .ok_or(InspectError::StructureLimit)?;
    // Actual variable ordinal order is the declared physical payload order.
    // Externalizing an empty or NULL value cannot save bytes and is forbidden.
    for payload in &mut payloads {
        check(budget, cancelled)?;
        if length <= MAX_RECORD_BYTES {
            break;
        }
        if let Payload::Long {
            encoded: Encoded::Present(bytes),
            choice,
        } = payload
            && *choice == StorageChoice::Inline
            && !bytes.is_empty()
        {
            *choice = StorageChoice::Single;
            length = length
                .checked_sub(bytes.len())
                .ok_or(EditError::Preservation)?;
        }
    }
    if length
        .checked_add(SLOT_BYTES)
        .is_none_or(|length| length > PAGE_BYTES - slot_offset(0))
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    Ok(Input { payloads })
}
