//! Separate TCND0001 and TCND0002 framing, checked before copying decoded witnesses.
use super::super::row_ordinals::RowOrdinals;
use super::{NativeDeleteIntent, NativeIntentError, SourceFamily, names};
use crate::{
    EditError, InspectError, RowDelete, format::page::PAGE_BYTES, header::check,
    limits::MAX_LIVE_ROWS,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: &[u8; 8] = b"TCND0001";
const PREFIX: &[u8; 4] = b"TCND";
const PROFILE: u16 = 1;
const CURRENT_MAGIC: &[u8; 8] = b"TCND0002";
const CURRENT_PROFILE: u16 = 2;
const RETAINED_MAGIC: &[u8; 8] = b"TCND0003";
const RETAINED_PROFILE: u16 = 3;
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
const ROW_HEADER_BYTES: usize = size_of::<u64>() + size_of::<u32>();
const MIN_ROW_BYTES: usize = size_of::<u16>() + size_of::<u8>();
const WORD_BITS: usize = u64::BITS as usize;
const ROW_WORDS: usize = MAX_LIVE_ROWS.div_ceil(WORD_BITS);
const COPY_BYTES: usize = 4096;
pub(super) const MAX_RECORD_BYTES: usize = 64 * 1024 * 1024;

impl SourceFamily {
    const fn maximum_bytes(self) -> usize {
        match self {
            Self::Ordinary => MAX_RECORD_BYTES,
            Self::Current => NativeDeleteIntent::MAX_CURRENT_ENCODED_BYTES,
            Self::Retained => NativeDeleteIntent::MAX_RETAINED_ENCODED_BYTES,
        }
    }

    const fn maximum_rows(self) -> usize {
        match self {
            Self::Ordinary => MAX_LIVE_ROWS,
            Self::Current | Self::Retained => u32::MAX as usize,
        }
    }

    const fn magic(self) -> &'static [u8; 8] {
        match self {
            Self::Ordinary => MAGIC,
            Self::Current => CURRENT_MAGIC,
            Self::Retained => RETAINED_MAGIC,
        }
    }

    const fn profile(self) -> u16 {
        match self {
            Self::Ordinary => PROFILE,
            Self::Current => CURRENT_PROFILE,
            Self::Retained => RETAINED_PROFILE,
        }
    }
}

enum Seen<'a> {
    Ordinary(&'a mut [u64; ROW_WORDS]),
    Current(RowOrdinals),
}

impl Seen<'_> {
    fn insert(
        &mut self,
        row: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeIntentError> {
        match self {
            Self::Ordinary(bitmap) => mark(bitmap, row),
            Self::Current(rows) => rows.insert(row, budget, cancelled),
        }
    }
}

fn scan(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), NativeIntentError> {
    for chunk in bytes.chunks(COPY_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
    }
    Ok(())
}

fn mark(seen: &mut [u64; ROW_WORDS], row: usize) -> Result<(), NativeIntentError> {
    if row >= MAX_LIVE_ROWS {
        return Err(NativeIntentError::InvalidRecord);
    }
    let word = &mut seen[row / WORD_BITS];
    let mask = 1_u64 << (row % WORD_BITS);
    if *word & mask != 0 {
        return Err(EditError::Conflict.into());
    }
    *word |= mask;
    Ok(())
}

pub(super) fn validate_requests(
    name_bytes: usize,
    deletes: &[RowDelete],
    family: SourceFamily,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, NativeIntentError> {
    check(budget, cancelled)?;
    let maximum_name_bytes = if family == SourceFamily::Retained {
        u32::MAX as usize
    } else {
        names::MAX_NAME_BYTES
    };
    if deletes.len() > family.maximum_rows() || name_bytes > maximum_name_bytes {
        return Err(InspectError::StructureLimit.into());
    }
    let mut bitmap;
    let mut seen = match family {
        SourceFamily::Ordinary => {
            budget
                .charge(Resource::Steps, ROW_WORDS as u64)
                .map_err(InspectError::from)?;
            bitmap = [0_u64; ROW_WORDS];
            Seen::Ordinary(&mut bitmap)
        }
        SourceFamily::Current | SourceFamily::Retained => Seen::Current(RowOrdinals::default()),
    };
    let mut length = HEADER_BYTES
        .checked_add(name_bytes)
        .filter(|length| *length <= family.maximum_bytes())
        .ok_or(InspectError::StructureLimit)?;
    for request in deletes {
        check(budget, cancelled)?;
        seen.insert(request.row(), budget, cancelled)?;
        if !(MIN_ROW_BYTES..=PAGE_BYTES).contains(&request.expected_bytes().len()) {
            return Err(NativeIntentError::InvalidRecord);
        }
        length = length
            .checked_add(ROW_HEADER_BYTES)
            .and_then(|value| value.checked_add(request.expected_bytes().len()))
            .filter(|value| *value <= family.maximum_bytes())
            .ok_or(InspectError::StructureLimit)?;
        scan(request.expected_bytes(), budget, cancelled)?;
    }
    check(budget, cancelled)?;
    Ok(length)
}

fn copy(
    output: &mut Vec<u8>,
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), NativeIntentError> {
    for chunk in bytes.chunks(COPY_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        output.extend_from_slice(chunk);
    }
    Ok(())
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

    fn row(&mut self) -> Result<(usize, &'a [u8]), NativeIntentError> {
        let ordinal = u64::from_le_bytes(self.take(size_of::<u64>())?.try_into().unwrap());
        let length = u32::from_le_bytes(self.take(size_of::<u32>())?.try_into().unwrap()) as usize;
        if !(MIN_ROW_BYTES..=PAGE_BYTES).contains(&length) {
            return Err(NativeIntentError::InvalidRecord);
        }
        let ordinal = usize::try_from(ordinal).map_err(|_| NativeIntentError::InvalidRecord)?;
        Ok((ordinal, self.take(length)?))
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
    family: SourceFamily,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Framing<'a>, NativeIntentError> {
    check(budget, cancelled)?;
    if raw.len() < HEADER_BYTES || raw.len() > family.maximum_bytes() {
        return Err(NativeIntentError::InvalidRecord);
    }
    let u32_at = |at| u32::from_le_bytes(raw[at..at + size_of::<u32>()].try_into().unwrap());
    if &raw[..MAGIC.len()] != family.magic() {
        return Err(if raw.starts_with(PREFIX) {
            NativeIntentError::UnsupportedVersion
        } else {
            NativeIntentError::InvalidRecord
        });
    }
    if u32_at(TOTAL_AT) as usize != raw.len()
        || raw[FLAGS_AT..BEFORE_LENGTH_AT] != [0, 0]
        || u32_at(RESERVED_AT) != 0
    {
        return Err(NativeIntentError::InvalidRecord);
    }
    if u16::from_le_bytes(raw[PROFILE_AT..FLAGS_AT].try_into().unwrap()) != family.profile() {
        return Err(NativeIntentError::UnsupportedProfile);
    }
    let before_length = u64::from_le_bytes(raw[BEFORE_LENGTH_AT..BINDING_AT].try_into().unwrap());
    family.length(before_length)?;
    let before_binding = raw[BINDING_AT..NAME_LENGTH_AT].try_into().unwrap();
    let count = u32_at(COUNT_AT) as usize;
    let name_bytes = u32_at(NAME_LENGTH_AT) as usize;
    let maximum_name_bytes = if family == SourceFamily::Retained {
        u32::MAX as usize
    } else {
        names::MAX_NAME_BYTES
    };
    if count > family.maximum_rows() || name_bytes == 0 || name_bytes > maximum_name_bytes {
        return Err(NativeIntentError::InvalidRecord);
    }
    let mut cursor = Cursor {
        bytes: raw,
        at: HEADER_BYTES,
    };
    let name = cursor.take(name_bytes)?;
    scan(name, budget, cancelled)?;
    let table = std::str::from_utf8(name).map_err(|_| NativeIntentError::InvalidRecord)?;
    family.validate_name(table, budget, cancelled)?;
    let rows = &raw[cursor.at..];
    let mut bitmap;
    let mut seen = match family {
        SourceFamily::Ordinary => {
            budget
                .charge(Resource::Steps, ROW_WORDS as u64)
                .map_err(InspectError::from)?;
            bitmap = [0_u64; ROW_WORDS];
            Seen::Ordinary(&mut bitmap)
        }
        SourceFamily::Current | SourceFamily::Retained => Seen::Current(RowOrdinals::default()),
    };
    for _ in 0..count {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, ROW_HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        let (row, expected) = cursor.row()?;
        seen.insert(row, budget, cancelled)?;
        scan(expected, budget, cancelled)?;
    }
    if cursor.at != raw.len() {
        return Err(NativeIntentError::InvalidRecord);
    }
    check(budget, cancelled)?;
    Ok(Framing {
        table,
        rows,
        count,
        before_length,
        before_binding,
    })
}

impl NativeDeleteIntent {
    /// Encode the retained request family under a paid, fallible allocation.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        let _span = tracing::debug_span!("native.delete_intent.encode").entered();
        let length = validate_requests(
            self.table().len(),
            self.deletes(),
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
        output.extend_from_slice(self.family.magic());
        output.extend_from_slice(&(length as u32).to_le_bytes());
        output.extend_from_slice(&self.family.profile().to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&self.before_length.to_le_bytes());
        output.extend_from_slice(&self.before_binding);
        output.extend_from_slice(&(self.table().len() as u32).to_le_bytes());
        output.extend_from_slice(&(self.deletes().len() as u32).to_le_bytes());
        output.extend_from_slice(&0_u32.to_le_bytes());
        copy(&mut output, self.table().as_bytes(), budget, cancelled)?;
        for request in self.deletes() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, ROW_HEADER_BYTES as u64)
                .map_err(InspectError::from)?;
            output.extend_from_slice(&(request.row() as u64).to_le_bytes());
            output.extend_from_slice(&(request.expected_bytes().len() as u32).to_le_bytes());
            copy(&mut output, request.expected_bytes(), budget, cancelled)?;
        }
        check(budget, cancelled)?;
        Ok(output)
    }

    /// Validate TCND0001 framing and duplicates before copying decoded witnesses.
    /// Source/record semantics remain mandatory fresh normal-planner checks.
    pub fn decode(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::decode_family(raw, SourceFamily::Ordinary, budget, cancelled)
    }

    /// Decode only TCND0002; older or unknown families are not retried.
    pub fn decode_current(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::decode_family(raw, SourceFamily::Current, budget, cancelled)
    }

    /// Decode only TCND0003; older versions never enter retained source proof.
    pub fn decode_retained(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::decode_family(raw, SourceFamily::Retained, budget, cancelled)
    }

    fn decode_family(
        raw: &[u8],
        family: SourceFamily,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let _span = tracing::debug_span!("native.delete_intent.decode").entered();
        let frame = preflight(raw, family, budget, cancelled)?;
        let table = names::copy_name(frame.table, budget)?;
        budget
            .charge_allocation(frame.count as u64, size_of::<RowDelete>() as u64)
            .map_err(InspectError::from)?;
        let mut deletes = Vec::new();
        deletes
            .try_reserve_exact(frame.count)
            .map_err(|_| InspectError::Allocation)?;
        let mut cursor = Cursor {
            bytes: frame.rows,
            at: 0,
        };
        for _ in 0..frame.count {
            check(budget, cancelled)?;
            let (row, expected) = cursor.row()?;
            budget
                .charge_allocation(expected.len() as u64, 1)
                .map_err(InspectError::from)?;
            let mut owned = Vec::new();
            owned
                .try_reserve_exact(expected.len())
                .map_err(|_| InspectError::Allocation)?;
            copy(&mut owned, expected, budget, cancelled)?;
            deletes.push(RowDelete::new(row, owned));
        }
        check(budget, cancelled)?;
        Ok(Self {
            table,
            deletes,
            before_length: frame.before_length,
            before_binding: frame.before_binding,
            family,
        })
    }
}
