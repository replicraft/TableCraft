//! Closed typed-row framing, completely scanned before decoded allocations.
use super::{NativeInsertIntent, NativeIntentError, binding, names};
use crate::{
    InspectError, RowInsert,
    header::check,
    limits::{MAX_COLUMNS, MAX_LIVE_ROWS},
    transaction::intent::fitted::carrier,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const MAGIC: &[u8; 8] = b"TCNA0001";
const PREFIX: &[u8; 4] = b"TCNA";
const PROFILE: u16 = 1;
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_LENGTH_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_LENGTH_AT + size_of::<u64>();
const BINDING_BYTES: usize = 32;
const NAME_LENGTH_AT: usize = BINDING_AT + BINDING_BYTES;
const COUNT_AT: usize = NAME_LENGTH_AT + size_of::<u32>();
const RESERVED_AT: usize = COUNT_AT + size_of::<u32>();
const HEADER_BYTES: usize = RESERVED_AT + size_of::<u32>();
const ROW_HEADER_BYTES: usize = 2 * size_of::<u32>();
pub(super) const MAX_RECORD_BYTES: usize = 64 * 1024 * 1024;

fn add(length: usize, extra: usize) -> Result<usize, NativeIntentError> {
    length
        .checked_add(extra)
        .filter(|length| *length <= MAX_RECORD_BYTES)
        .ok_or_else(|| InspectError::StructureLimit.into())
}

fn row_length(
    row: &RowInsert,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, NativeIntentError> {
    if row.values().is_empty() || row.values().len() > MAX_COLUMNS {
        return Err(NativeIntentError::InvalidRecord);
    }
    let mut length = ROW_HEADER_BYTES;
    for value in row.values() {
        check(budget, cancelled)?;
        let encoded = carrier::row_view(value)?;
        carrier::pay_scan(encoded.bytes(), budget, cancelled)?;
        length = add(add(length, carrier::HEADER_BYTES)?, encoded.bytes().len())?;
    }
    Ok(length)
}

pub(super) fn validate_requests(
    name_bytes: usize,
    rows: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, NativeIntentError> {
    check(budget, cancelled)?;
    if rows.len() > MAX_LIVE_ROWS || name_bytes > names::MAX_NAME_BYTES {
        return Err(InspectError::StructureLimit.into());
    }
    let mut total = add(HEADER_BYTES, name_bytes)?;
    let width = rows.first().map(|row| row.values().len());
    for row in rows {
        check(budget, cancelled)?;
        if Some(row.values().len()) != width {
            return Err(NativeIntentError::InvalidRecord);
        }
        total = add(total, row_length(row, budget, cancelled)?)?;
    }
    check(budget, cancelled)?;
    Ok(total)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], NativeIntentError> {
        let end = self
            .at
            .checked_add(length)
            .ok_or(NativeIntentError::InvalidRecord)?;
        let bytes = self
            .bytes
            .get(self.at..end)
            .ok_or(NativeIntentError::InvalidRecord)?;
        self.at = end;
        Ok(bytes)
    }

    fn u32(&mut self) -> Result<usize, NativeIntentError> {
        Ok(u32::from_le_bytes(self.take(size_of::<u32>())?.try_into().unwrap()) as usize)
    }

    fn row(&mut self) -> Result<(Cursor<'a>, usize), NativeIntentError> {
        let length = self.u32()?;
        let count = self.u32()?;
        if length < ROW_HEADER_BYTES || !(1..=MAX_COLUMNS).contains(&count) {
            return Err(NativeIntentError::InvalidRecord);
        }
        let bytes = self.take(length - ROW_HEADER_BYTES)?;
        Ok((Cursor { bytes, at: 0 }, count))
    }

    fn cell(
        &mut self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<carrier::Borrowed<'a>, NativeIntentError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, carrier::HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        let header = self.take(size_of::<u32>())?;
        if header[1..] != [0, 0, 0] {
            return Err(NativeIntentError::InvalidRecord);
        }
        let length = self.u32()?;
        let bytes = self.take(length)?;
        carrier::row_parse(header[0], bytes, budget, cancelled)
    }

    fn finished(&self) -> bool {
        self.at == self.bytes.len()
    }
}

struct Framing<'a> {
    table: &'a str,
    rows: &'a [u8],
    count: usize,
    before_length: u64,
    before_binding: [u8; BINDING_BYTES],
}

fn preflight<'a>(
    raw: &'a [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Framing<'a>, NativeIntentError> {
    check(budget, cancelled)?;
    if !(HEADER_BYTES..=MAX_RECORD_BYTES).contains(&raw.len()) {
        return Err(NativeIntentError::InvalidRecord);
    }
    if &raw[..MAGIC.len()] != MAGIC {
        return Err(if raw.starts_with(PREFIX) {
            NativeIntentError::UnsupportedVersion
        } else {
            NativeIntentError::InvalidRecord
        });
    }
    budget
        .charge(Resource::Steps, HEADER_BYTES as u64)
        .map_err(InspectError::from)?;
    let word = |at| u32::from_le_bytes(raw[at..at + size_of::<u32>()].try_into().unwrap()) as usize;
    if word(TOTAL_AT) != raw.len()
        || raw[FLAGS_AT..BEFORE_LENGTH_AT] != [0, 0]
        || word(RESERVED_AT) != 0
    {
        return Err(NativeIntentError::InvalidRecord);
    }
    if u16::from_le_bytes(raw[PROFILE_AT..FLAGS_AT].try_into().unwrap()) != PROFILE {
        return Err(NativeIntentError::UnsupportedProfile);
    }
    let before_length = u64::from_le_bytes(raw[BEFORE_LENGTH_AT..BINDING_AT].try_into().unwrap());
    binding::length(before_length)?;
    let count = word(COUNT_AT);
    let name_bytes = word(NAME_LENGTH_AT);
    if count > MAX_LIVE_ROWS || !(1..=names::MAX_NAME_BYTES).contains(&name_bytes) {
        return Err(NativeIntentError::InvalidRecord);
    }
    let mut cursor = Cursor {
        bytes: raw,
        at: HEADER_BYTES,
    };
    let name = cursor.take(name_bytes)?;
    carrier::pay_scan(name, budget, cancelled)?;
    let table = std::str::from_utf8(name).map_err(|_| NativeIntentError::InvalidRecord)?;
    names::validate_name(table, budget, cancelled)?;
    let rows = &raw[cursor.at..];
    let mut width = None;
    for _ in 0..count {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, ROW_HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        let (mut row, values) = cursor.row()?;
        if width.is_some_and(|width| width != values) {
            return Err(NativeIntentError::InvalidRecord);
        }
        width = Some(values);
        for _ in 0..values {
            row.cell(budget, cancelled)?;
        }
        if !row.finished() {
            return Err(NativeIntentError::InvalidRecord);
        }
    }
    if !cursor.finished() {
        return Err(NativeIntentError::InvalidRecord);
    }
    check(budget, cancelled)?;
    Ok(Framing {
        table,
        rows,
        count,
        before_length,
        before_binding: raw[BINDING_AT..NAME_LENGTH_AT].try_into().unwrap(),
    })
}

fn copy(
    output: &mut Vec<u8>,
    raw: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), NativeIntentError> {
    for chunk in raw.chunks(carrier::COPY_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        output.extend_from_slice(chunk);
    }
    Ok(())
}

impl NativeInsertIntent {
    /// Serialize exact supplied carriers; native encoding and admission are absent.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        let length = validate_requests(self.table().len(), self.rows(), budget, cancelled)?;
        budget
            .charge_allocation(length as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&(length as u32).to_le_bytes());
        output.extend_from_slice(&PROFILE.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&self.before_length.to_le_bytes());
        output.extend_from_slice(&self.before_binding);
        output.extend_from_slice(&(self.table().len() as u32).to_le_bytes());
        output.extend_from_slice(&(self.rows().len() as u32).to_le_bytes());
        output.extend_from_slice(&0_u32.to_le_bytes());
        copy(&mut output, self.table().as_bytes(), budget, cancelled)?;
        for row in self.rows() {
            check(budget, cancelled)?;
            let length = row_length(row, budget, cancelled)?;
            budget
                .charge(Resource::Steps, ROW_HEADER_BYTES as u64)
                .map_err(InspectError::from)?;
            output.extend_from_slice(&(length as u32).to_le_bytes());
            output.extend_from_slice(&(row.values().len() as u32).to_le_bytes());
            for value in row.values() {
                check(budget, cancelled)?;
                let encoded = carrier::row_view(value)?;
                budget
                    .charge(Resource::Steps, carrier::HEADER_BYTES as u64)
                    .map_err(InspectError::from)?;
                output.extend_from_slice(&[encoded.tag, 0, 0, 0]);
                output.extend_from_slice(&(encoded.bytes().len() as u32).to_le_bytes());
                copy(&mut output, encoded.bytes(), budget, cancelled)?;
            }
        }
        check(budget, cancelled)?;
        Ok(output)
    }

    /// Validate complete framing and every carrier before any decoded heap copy.
    pub fn decode(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let frame = preflight(raw, budget, cancelled)?;
        let table = names::copy_name(frame.table, budget)?;
        budget
            .charge_allocation(frame.count as u64, size_of::<RowInsert>() as u64)
            .map_err(InspectError::from)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(frame.count)
            .map_err(|_| InspectError::Allocation)?;
        let mut cursor = Cursor {
            bytes: frame.rows,
            at: 0,
        };
        for _ in 0..frame.count {
            check(budget, cancelled)?;
            let (mut row, count) = cursor.row()?;
            budget
                .charge(Resource::Steps, ROW_HEADER_BYTES as u64)
                .map_err(InspectError::from)?;
            budget
                .charge_allocation(count as u64, size_of::<Value>() as u64)
                .map_err(InspectError::from)?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(count)
                .map_err(|_| InspectError::Allocation)?;
            for _ in 0..count {
                values.push(carrier::owned(
                    row.cell(budget, cancelled)?,
                    budget,
                    cancelled,
                )?);
            }
            rows.push(RowInsert::new(values));
        }
        check(budget, cancelled)?;
        Ok(Self {
            table,
            rows,
            before_length: frame.before_length,
            before_binding: frame.before_binding,
        })
    }
}
