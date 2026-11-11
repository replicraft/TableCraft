//! Closed typed-row framing, completely scanned before decoded allocations.
use super::super::row_carrier::{self as carrier, Borrowed, Encoded, Profile};
use super::{NativeIntentError, NativeOmittedInsertIntent, SourceFamily, binding, names};
use crate::{
    InsertFieldValue, InspectError, OmittedRowInsert,
    header::check,
    limits::{MAX_COLUMNS, MAX_LIVE_ROWS},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const MAGIC: &[u8; 8] = b"TCNA0002";
const PREFIX: &[u8; 4] = b"TCNA";
const PROFILE: u16 = 2;
const WIDER_MAGIC: &[u8; 8] = b"TCNA0006";
const WIDER_PROFILE: u16 = 6;
const CURRENT_MAGIC: &[u8; 8] = b"TCNA0007";
const CURRENT_PROFILE: u16 = 7;
const RETAINED_MAGIC: &[u8; 8] = b"TCNA0008";
const RETAINED_PROFILE: u16 = 8;
const RETAINED_OPAQUE_MAGIC: &[u8; 8] = b"TCNA0009";
const RETAINED_OPAQUE_PROFILE: u16 = 9;
const RETAINED_KEYLESS_MAGIC: &[u8; 8] = b"TCNA0010";
const RETAINED_KEYLESS_PROFILE: u16 = 10;

impl Profile {
    const fn omitted_tag(self) -> u8 {
        match self {
            Self::Legacy => 8,
            Self::Wider => 14,
        }
    }
}

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

impl SourceFamily {
    fn view(self, value: &Value, profile: Profile) -> Result<Encoded<'_>, NativeIntentError> {
        match self {
            Self::Ordinary => carrier::row_view(value, profile),
            Self::Current | Self::Retained | Self::RetainedOpaque | Self::RetainedKeyless => {
                carrier::length_prefixed_row_view(value, profile)
            }
        }
    }

    fn parse<'a>(
        self,
        tag: u8,
        bytes: &'a [u8],
        profile: Profile,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Borrowed<'a>, NativeIntentError> {
        match self {
            Self::Ordinary => carrier::row_parse(tag, bytes, profile, budget, cancelled),
            Self::Current | Self::Retained | Self::RetainedOpaque | Self::RetainedKeyless => {
                carrier::length_prefixed_row_parse(tag, bytes, profile, budget, cancelled)
            }
        }
    }

    const fn maximum_bytes(self) -> usize {
        match self {
            Self::Ordinary => MAX_RECORD_BYTES,
            Self::Current => NativeOmittedInsertIntent::MAX_CURRENT_ENCODED_BYTES,
            Self::Retained | Self::RetainedOpaque | Self::RetainedKeyless => {
                NativeOmittedInsertIntent::MAX_RETAINED_ENCODED_BYTES
            }
        }
    }

    const fn maximum_rows(self) -> usize {
        match self {
            Self::Ordinary => MAX_LIVE_ROWS,
            Self::Current | Self::Retained | Self::RetainedOpaque | Self::RetainedKeyless => {
                u32::MAX as usize
            }
        }
    }

    const fn maximum_columns(self) -> usize {
        match self {
            Self::Ordinary => MAX_COLUMNS,
            Self::Current | Self::Retained | Self::RetainedOpaque | Self::RetainedKeyless => {
                u32::MAX as usize
            }
        }
    }
}

fn add(length: usize, extra: usize, family: SourceFamily) -> Result<usize, NativeIntentError> {
    length
        .checked_add(extra)
        .filter(|length| *length <= family.maximum_bytes())
        .ok_or_else(|| InspectError::StructureLimit.into())
}

fn row_length(
    row: &OmittedRowInsert,
    profile: Profile,
    family: SourceFamily,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, NativeIntentError> {
    if row.values().is_empty() || row.values().len() > family.maximum_columns() {
        return Err(NativeIntentError::InvalidRecord);
    }
    let mut length = ROW_HEADER_BYTES;
    for field in row.values() {
        check(budget, cancelled)?;
        length = add(length, carrier::HEADER_BYTES, family)?;
        if let InsertFieldValue::Supplied(value) = field {
            let encoded = family.view(value, profile)?;
            carrier::pay_scan(encoded.bytes(), budget, cancelled)?;
            length = add(length, encoded.bytes().len(), family)?;
        }
    }
    Ok(length)
}

pub(super) fn validate_requests(
    name_bytes: usize,
    rows: &[OmittedRowInsert],
    profile: Profile,
    family: SourceFamily,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, NativeIntentError> {
    check(budget, cancelled)?;
    if rows.len() > family.maximum_rows() || name_bytes > names::MAX_NAME_BYTES {
        return Err(InspectError::StructureLimit.into());
    }
    let mut total = add(HEADER_BYTES, name_bytes, family)?;
    let width = rows.first().map(|row| row.values().len());
    for row in rows {
        check(budget, cancelled)?;
        if Some(row.values().len()) != width {
            return Err(NativeIntentError::InvalidRecord);
        }
        total = add(
            total,
            row_length(row, profile, family, budget, cancelled)?,
            family,
        )?;
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

    fn row(&mut self, family: SourceFamily) -> Result<(Cursor<'a>, usize), NativeIntentError> {
        let length = self.u32()?;
        let count = self.u32()?;
        if length < ROW_HEADER_BYTES
            || !(1..=family.maximum_columns()).contains(&count)
            || (family != SourceFamily::Ordinary
                && count > (length - ROW_HEADER_BYTES) / carrier::HEADER_BYTES)
        {
            return Err(NativeIntentError::InvalidRecord);
        }
        let bytes = self.take(length - ROW_HEADER_BYTES)?;
        Ok((Cursor { bytes, at: 0 }, count))
    }

    fn cell(
        &mut self,
        profile: Profile,
        family: SourceFamily,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<Borrowed<'a>>, NativeIntentError> {
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
        if header[0] == profile.omitted_tag() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err(NativeIntentError::InvalidRecord)
            };
        }
        family
            .parse(header[0], bytes, profile, budget, cancelled)
            .map(Some)
    }

    fn finished(&self) -> bool {
        self.at == self.bytes.len()
    }
}

struct Framing<'a> {
    table: &'a str,
    profile: Profile,
    rows: &'a [u8],
    count: usize,
    before_length: u64,
    before_binding: [u8; BINDING_BYTES],
}

fn preflight<'a>(
    raw: &'a [u8],
    family: SourceFamily,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Framing<'a>, NativeIntentError> {
    check(budget, cancelled)?;
    if !(HEADER_BYTES..=family.maximum_bytes()).contains(&raw.len()) {
        return Err(NativeIntentError::InvalidRecord);
    }
    let (profile, expected_number) = match &raw[..MAGIC.len()] {
        version if family == SourceFamily::Ordinary && version == MAGIC => {
            (Profile::Legacy, PROFILE)
        }
        version if family == SourceFamily::Ordinary && version == WIDER_MAGIC => {
            (Profile::Wider, WIDER_PROFILE)
        }
        version if family == SourceFamily::Current && version == CURRENT_MAGIC => {
            (Profile::Wider, CURRENT_PROFILE)
        }
        version if family == SourceFamily::Retained && version == RETAINED_MAGIC => {
            (Profile::Wider, RETAINED_PROFILE)
        }
        version if family == SourceFamily::RetainedOpaque && version == RETAINED_OPAQUE_MAGIC => {
            (Profile::Wider, RETAINED_OPAQUE_PROFILE)
        }
        version if family == SourceFamily::RetainedKeyless && version == RETAINED_KEYLESS_MAGIC => {
            (Profile::Wider, RETAINED_KEYLESS_PROFILE)
        }
        _ => {
            return Err(if raw.starts_with(PREFIX) {
                NativeIntentError::UnsupportedVersion
            } else {
                NativeIntentError::InvalidRecord
            });
        }
    };
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
    if u16::from_le_bytes(raw[PROFILE_AT..FLAGS_AT].try_into().unwrap()) != expected_number {
        return Err(NativeIntentError::UnsupportedProfile);
    }
    let before_length = u64::from_le_bytes(raw[BEFORE_LENGTH_AT..BINDING_AT].try_into().unwrap());
    match family {
        SourceFamily::Retained | SourceFamily::RetainedOpaque | SourceFamily::RetainedKeyless => {
            binding::retained_length(before_length)?
        }
        _ => binding::length(before_length)?,
    }
    let count = word(COUNT_AT);
    let name_bytes = word(NAME_LENGTH_AT);
    if count > family.maximum_rows() || !(1..=names::MAX_NAME_BYTES).contains(&name_bytes) {
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
        let (mut row, values) = cursor.row(family)?;
        if width.is_some_and(|width| width != values) {
            return Err(NativeIntentError::InvalidRecord);
        }
        width = Some(values);
        for _ in 0..values {
            row.cell(profile, family, budget, cancelled)?;
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
        profile,
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

impl NativeOmittedInsertIntent {
    /// Serialize exact presence/carriers; default expansion and native admission are absent.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        let profile = self.profile;
        let length = validate_requests(
            self.table().len(),
            self.rows(),
            profile,
            self.family,
            budget,
            cancelled,
        )?;
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
        let (magic, number) = if self.family == SourceFamily::RetainedKeyless {
            (RETAINED_KEYLESS_MAGIC, RETAINED_KEYLESS_PROFILE)
        } else if self.family == SourceFamily::RetainedOpaque {
            (RETAINED_OPAQUE_MAGIC, RETAINED_OPAQUE_PROFILE)
        } else if self.family == SourceFamily::Retained {
            (RETAINED_MAGIC, RETAINED_PROFILE)
        } else if self.family == SourceFamily::Current {
            (CURRENT_MAGIC, CURRENT_PROFILE)
        } else if profile == Profile::Wider {
            (WIDER_MAGIC, WIDER_PROFILE)
        } else {
            (MAGIC, PROFILE)
        };
        output.extend_from_slice(magic);
        output.extend_from_slice(&(length as u32).to_le_bytes());
        output.extend_from_slice(&number.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&self.before_length.to_le_bytes());
        output.extend_from_slice(&self.before_binding);
        output.extend_from_slice(&(self.table().len() as u32).to_le_bytes());
        output.extend_from_slice(&(self.rows().len() as u32).to_le_bytes());
        output.extend_from_slice(&0_u32.to_le_bytes());
        copy(&mut output, self.table().as_bytes(), budget, cancelled)?;
        for row in self.rows() {
            check(budget, cancelled)?;
            let length = row_length(row, profile, self.family, budget, cancelled)?;
            budget
                .charge(Resource::Steps, ROW_HEADER_BYTES as u64)
                .map_err(InspectError::from)?;
            output.extend_from_slice(&(length as u32).to_le_bytes());
            output.extend_from_slice(&(row.values().len() as u32).to_le_bytes());
            for field in row.values() {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, carrier::HEADER_BYTES as u64)
                    .map_err(InspectError::from)?;
                match field {
                    InsertFieldValue::Omitted => {
                        output.extend_from_slice(&[profile.omitted_tag(), 0, 0, 0]);
                        output.extend_from_slice(&0_u32.to_le_bytes());
                    }
                    InsertFieldValue::Supplied(value) => {
                        let encoded = self.family.view(value, profile)?;
                        output.extend_from_slice(&[encoded.tag(), 0, 0, 0]);
                        output.extend_from_slice(&(encoded.bytes().len() as u32).to_le_bytes());
                        copy(&mut output, encoded.bytes(), budget, cancelled)?;
                    }
                }
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
        Self::decode_family(raw, SourceFamily::Ordinary, budget, cancelled)
    }

    /// Decode only TCNA0007 original presence; older families are never retried.
    pub fn decode_current(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::decode_family(raw, SourceFamily::Current, budget, cancelled)
    }

    /// Decode only TCNA0008 retained original presence; never retry old families.
    pub fn decode_retained(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::decode_family(raw, SourceFamily::Retained, budget, cancelled)
    }

    /// Decode only TCNA0009 opaque-history original presence.
    pub fn decode_retained_opaque(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::decode_family(raw, SourceFamily::RetainedOpaque, budget, cancelled)
    }

    /// Decode only TCNA0010/profile10 original presence. Older generated and
    /// ordinary request languages are rejected without retry.
    ///
    /// # Errors
    /// Refuses malformed framing, unsupported carriers or source lengths,
    /// cancellation, allocation failure and explicit resource exhaustion.
    pub fn decode_retained_keyless(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::decode_family(raw, SourceFamily::RetainedKeyless, budget, cancelled)
    }

    fn decode_family(
        raw: &[u8],
        family: SourceFamily,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let frame = preflight(raw, family, budget, cancelled)?;
        let profile = frame.profile;
        let table = names::copy_name(frame.table, budget)?;
        budget
            .charge_allocation(frame.count as u64, size_of::<OmittedRowInsert>() as u64)
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
            let (mut row, count) = cursor.row(family)?;
            budget
                .charge(Resource::Steps, ROW_HEADER_BYTES as u64)
                .map_err(InspectError::from)?;
            budget
                .charge_allocation(count as u64, size_of::<InsertFieldValue>() as u64)
                .map_err(InspectError::from)?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(count)
                .map_err(|_| InspectError::Allocation)?;
            for _ in 0..count {
                let field = match row.cell(profile, family, budget, cancelled)? {
                    None => InsertFieldValue::Omitted,
                    Some(value) => {
                        InsertFieldValue::Supplied(carrier::owned(value, budget, cancelled)?)
                    }
                };
                values.push(field);
            }
            rows.push(OmittedRowInsert::new(values));
        }
        check(budget, cancelled)?;
        Ok(Self {
            table,
            rows,
            before_length: frame.before_length,
            before_binding: frame.before_binding,
            profile,
            family,
        })
    }
}
