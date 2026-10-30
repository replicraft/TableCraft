use super::{
    binding,
    model::{self, MAX_NAME_BYTES, NativeInt32EditIntent, NativeIntentError},
};
use crate::{InspectError, Int32Update, header::check, limits::MAX_LIVE_ROWS};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: &[u8; 8] = b"TCNI0001";
const MAGIC_PREFIX: &[u8; 4] = b"TCNI";
const PROFILE: u16 = 1;
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_LENGTH_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_LENGTH_AT + size_of::<u64>();
const TABLE_LENGTH_AT: usize = BINDING_AT + 32;
const COLUMN_LENGTH_AT: usize = TABLE_LENGTH_AT + size_of::<u32>();
const COUNT_AT: usize = COLUMN_LENGTH_AT + size_of::<u32>();
const RESERVED_AT: usize = COUNT_AT + size_of::<u32>();
const HEADER_BYTES: usize = RESERVED_AT + size_of::<u32>();
const EXPECTED_AT: usize = size_of::<u64>();
const VALUE_AT: usize = EXPECTED_AT + size_of::<i32>();
const UPDATE_BYTES: usize = VALUE_AT + size_of::<i32>();
pub(super) const MAX_RECORD_BYTES: usize =
    HEADER_BYTES + 2 * MAX_NAME_BYTES + MAX_LIVE_ROWS * UPDATE_BYTES;
const COPY_BYTES: usize = 4096;

fn length(table: usize, column: usize, count: usize) -> Result<usize, NativeIntentError> {
    let result = count
        .checked_mul(UPDATE_BYTES)
        .and_then(|value| value.checked_add(table))
        .and_then(|value| value.checked_add(column))
        .and_then(|value| value.checked_add(HEADER_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    if table > MAX_NAME_BYTES
        || column > MAX_NAME_BYTES
        || count > MAX_LIVE_ROWS
        || result > MAX_RECORD_BYTES
    {
        return Err(InspectError::StructureLimit.into());
    }
    Ok(result)
}

fn copy(
    output: &mut Vec<u8>,
    value: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), NativeIntentError> {
    for chunk in value.chunks(COPY_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        output.extend_from_slice(chunk);
    }
    Ok(())
}

impl NativeInt32EditIntent {
    /// Encode the exact V1 request-data record under a paid bounded allocation.
    /// The result is not a native plan, journal or recovery permission.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        let _span = tracing::debug_span!("native.intent.encode").entered();
        check(budget, cancelled)?;
        let total = length(self.table().len(), self.column().len(), self.updates.len())?;
        budget
            .charge_allocation(total as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(total)
            .map_err(|_| InspectError::Allocation)?;
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&(total as u32).to_le_bytes());
        output.extend_from_slice(&PROFILE.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&self.before_length.to_le_bytes());
        output.extend_from_slice(&self.before_binding);
        output.extend_from_slice(&(self.table().len() as u32).to_le_bytes());
        output.extend_from_slice(&(self.column().len() as u32).to_le_bytes());
        output.extend_from_slice(&(self.updates.len() as u32).to_le_bytes());
        output.extend_from_slice(&0_u32.to_le_bytes());
        copy(&mut output, self.table().as_bytes(), budget, cancelled)?;
        copy(&mut output, self.column().as_bytes(), budget, cancelled)?;
        for update in &self.updates {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, UPDATE_BYTES as u64)
                .map_err(InspectError::from)?;
            output.extend_from_slice(&(update.row() as u64).to_le_bytes());
            output.extend_from_slice(&update.expected().to_le_bytes());
            output.extend_from_slice(&update.value().to_le_bytes());
        }
        check(budget, cancelled)?;
        Ok(output)
    }

    /// Decode one complete V1 record into bounded request data only. Unknown
    /// versions/profiles/flags, invalid names and trailing bytes never fall back.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let _span = tracing::debug_span!("native.intent.decode").entered();
        check(budget, cancelled)?;
        if bytes.len() < HEADER_BYTES || bytes.len() > MAX_RECORD_BYTES {
            return Err(NativeIntentError::InvalidRecord);
        }
        budget
            .charge(Resource::Steps, HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        if &bytes[..MAGIC.len()] != MAGIC {
            return Err(if &bytes[..MAGIC_PREFIX.len()] == MAGIC_PREFIX {
                NativeIntentError::UnsupportedVersion
            } else {
                NativeIntentError::InvalidRecord
            });
        }
        // The complete fixed header is present before these fixed-width reads.
        let u32_at = |at| u32::from_le_bytes(bytes[at..at + size_of::<u32>()].try_into().unwrap());
        let profile = u16::from_le_bytes(bytes[PROFILE_AT..FLAGS_AT].try_into().unwrap());
        if profile != PROFILE {
            return Err(NativeIntentError::UnsupportedProfile);
        }
        if bytes[FLAGS_AT..BEFORE_LENGTH_AT] != [0, 0] || u32_at(RESERVED_AT) != 0 {
            return Err(NativeIntentError::InvalidRecord);
        }
        let before_length =
            u64::from_le_bytes(bytes[BEFORE_LENGTH_AT..BINDING_AT].try_into().unwrap());
        binding::length(before_length)?;
        let table_length = u32_at(TABLE_LENGTH_AT) as usize;
        let column_length = u32_at(COLUMN_LENGTH_AT) as usize;
        let count = u32_at(COUNT_AT) as usize;
        let expected = length(table_length, column_length, count)?;
        if u32_at(TOTAL_AT) as usize != bytes.len() || expected != bytes.len() {
            return Err(NativeIntentError::InvalidRecord);
        }
        let table_end = HEADER_BYTES + table_length;
        let column_end = table_end + column_length;
        budget
            .charge(Resource::Steps, (table_length + column_length) as u64)
            .map_err(InspectError::from)?;
        let table = std::str::from_utf8(&bytes[HEADER_BYTES..table_end])
            .map_err(|_| NativeIntentError::InvalidRecord)?;
        let column = std::str::from_utf8(&bytes[table_end..column_end])
            .map_err(|_| NativeIntentError::InvalidRecord)?;
        model::validate_name(table, budget, cancelled)?;
        model::validate_name(column, budget, cancelled)?;
        budget
            .charge_allocation(count as u64, size_of::<Int32Update>() as u64)
            .map_err(InspectError::from)?;
        let mut updates = Vec::new();
        updates
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        for entry in bytes[column_end..].as_chunks::<UPDATE_BYTES>().0 {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, UPDATE_BYTES as u64)
                .map_err(InspectError::from)?;
            let row = usize::try_from(u64::from_le_bytes(entry[..EXPECTED_AT].try_into().unwrap()))
                .map_err(|_| InspectError::StructureLimit)?;
            let expected = i32::from_le_bytes(entry[EXPECTED_AT..VALUE_AT].try_into().unwrap());
            let value = i32::from_le_bytes(entry[VALUE_AT..UPDATE_BYTES].try_into().unwrap());
            updates.push(Int32Update::new(row, expected, value));
        }
        let before_binding = bytes[BINDING_AT..TABLE_LENGTH_AT].try_into().unwrap();
        Self::from_decoded(
            table,
            column,
            updates,
            before_length,
            before_binding,
            budget,
            cancelled,
        )
    }
}
