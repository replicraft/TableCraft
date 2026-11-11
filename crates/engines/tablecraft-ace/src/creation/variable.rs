//! Borrowed payload plans and exact row lengths, bound before image allocation.
use super::{
    CreateDatabaseError,
    column::Declaration,
    input::{Cell, Columns, Rows},
    profile, stored_text,
};
use crate::{
    InspectError,
    format::{data_type::NUMERIC_BYTES, page::PAGE_BYTES},
    header::check,
    table::decimal,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Decimal, Value},
};

const OFFSET_BYTES: usize = size_of::<u16>();
const DIRECTORY_BYTES: usize = size_of::<u16>();

#[derive(Clone, Copy)]
pub(super) enum Payload<'a> {
    Null,
    Text(stored_text::Plan<'a>),
    Binary(&'a [u8]),
    Decimal(Decimal),
}

impl Payload<'_> {
    pub fn byte_len(self) -> usize {
        match self {
            Self::Null => 0,
            Self::Text(plan) => plan.byte_len(),
            Self::Binary(bytes) => bytes.len(),
            Self::Decimal(_) => NUMERIC_BYTES,
        }
    }

    pub fn present(self) -> bool {
        !matches!(self, Self::Null)
    }

    pub fn write(
        self,
        destination: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CreateDatabaseError> {
        check(budget, cancelled)?;
        if destination.len() != self.byte_len() {
            return Err(CreateDatabaseError::Layout);
        }
        match self {
            Self::Null => Ok(()),
            Self::Text(plan) => plan.write(destination, budget, cancelled),
            Self::Binary(bytes) => {
                budget
                    .charge(Resource::Steps, bytes.len() as u64)
                    .map_err(InspectError::from)?;
                destination.copy_from_slice(bytes);
                Ok(())
            }
            Self::Decimal(value) => {
                budget
                    .charge(Resource::Steps, NUMERIC_BYTES as u64)
                    .map_err(InspectError::from)?;
                destination.copy_from_slice(&decimal::encode(value));
                Ok(())
            }
        }
    }
}

pub(super) struct Prepared<'a> {
    row_lengths: [usize; profile::MAX_ROWS],
    payloads: Vec<Payload<'a>>,
    variables: usize,
    pub free_bytes: usize,
    pub minimum_row_bytes: usize,
}

impl<'a> Prepared<'a> {
    pub fn row_length(&self, row: usize) -> usize {
        self.row_lengths[row]
    }

    pub fn row_payloads(&self, row: usize) -> Result<&[Payload<'a>], CreateDatabaseError> {
        let start = row
            .checked_mul(self.variables)
            .ok_or(CreateDatabaseError::Layout)?;
        let end = start
            .checked_add(self.variables)
            .ok_or(CreateDatabaseError::Layout)?;
        self.payloads
            .get(start..end)
            .ok_or(CreateDatabaseError::Layout)
    }
}

pub(super) fn prepare<'a>(
    columns: Columns<'a>,
    rows: Rows<'a>,
    fixed_end: usize,
    variables: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared<'a>, CreateDatabaseError> {
    let footer = variables
        .checked_add(2)
        .and_then(|count| count.checked_mul(OFFSET_BYTES))
        .ok_or(CreateDatabaseError::Layout)?;
    let minimum_row_bytes = fixed_end
        .checked_add(footer)
        .and_then(|bytes| bytes.checked_add(columns.count().div_ceil(u8::BITS as usize)))
        .ok_or(CreateDatabaseError::Layout)?;
    let mut row_lengths = [0; profile::MAX_ROWS];
    let mut used = super::record::DATA_HEADER_BYTES;

    // Complete value validation and physical fit precede even the borrowed-plan
    // allocation. The input is immutable for both passes; no payload is cloned.
    for (row, length) in row_lengths[..rows.count()].iter_mut().enumerate() {
        check(budget, cancelled)?;
        *length = minimum_row_bytes;
        for column in 0..columns.count() {
            if matches!(columns.declaration(column), Declaration::Fixed(_)) {
                continue;
            }
            let plan = capture(columns, rows, row, column, budget, cancelled)?;
            *length = length
                .checked_add(plan.byte_len())
                .ok_or(CreateDatabaseError::PageCapacity)?;
        }
        used = used
            .checked_add(*length)
            .and_then(|bytes| bytes.checked_add(DIRECTORY_BYTES))
            .ok_or(CreateDatabaseError::PageCapacity)?;
    }
    let free_bytes = PAGE_BYTES
        .checked_sub(used)
        .ok_or(CreateDatabaseError::PageCapacity)?;
    let count = rows
        .count()
        .checked_mul(variables)
        .ok_or(CreateDatabaseError::Layout)?;
    let allocation = count
        .checked_mul(size_of::<Payload<'_>>())
        .ok_or(CreateDatabaseError::Layout)?;
    let mut payloads = Vec::new();
    if count != 0 {
        budget
            .charge_allocation(allocation as u64, 1)
            .map_err(InspectError::from)?;
        payloads
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
    }
    for row in 0..rows.count() {
        for column in 0..columns.count() {
            if !matches!(columns.declaration(column), Declaration::Fixed(_)) {
                payloads.push(capture(columns, rows, row, column, budget, cancelled)?);
            }
        }
    }
    check(budget, cancelled)?;

    Ok(Prepared {
        row_lengths,
        payloads,
        variables,
        free_bytes,
        minimum_row_bytes,
    })
}

pub(in crate::creation) fn capture<'a>(
    columns: Columns<'a>,
    rows: Rows<'a>,
    row: usize,
    column: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Payload<'a>, CreateDatabaseError> {
    check(budget, cancelled)?;
    let Cell::Value(value) = rows.cell(row, column) else {
        return Err(CreateDatabaseError::Carrier);
    };
    if matches!(value, Value::Null) {
        return Ok(Payload::Null);
    }
    match (columns.declaration(column), value) {
        (Declaration::ShortText { units }, Value::Text(text)) => {
            let mut actual = 0_usize;
            for character in text.chars() {
                check(budget, cancelled)?;
                actual += character.len_utf16();
                if actual > units {
                    return Err(CreateDatabaseError::DeclaredValue);
                }
            }
            Ok(Payload::Text(stored_text::Plan::capture(
                text, budget, cancelled,
            )?))
        }
        (Declaration::ShortBinary { bytes }, Value::Binary(value)) => {
            if value.len() > bytes {
                return Err(CreateDatabaseError::DeclaredValue);
            }
            Ok(Payload::Binary(value))
        }
        (Declaration::Decimal(constraints), Value::Decimal(value)) => {
            constraints
                .validate(*value)
                .map_err(|_| CreateDatabaseError::DeclaredValue)?;
            Ok(Payload::Decimal(*value))
        }
        _ => Err(CreateDatabaseError::Carrier),
    }
}
