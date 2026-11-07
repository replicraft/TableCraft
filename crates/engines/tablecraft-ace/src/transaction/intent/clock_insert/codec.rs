//! Closed captured-clock envelope over the unchanged validated TCNA2 row grammar.
use super::{NativeClockInsertIntent, NativeIntentError, NativeOmittedInsertIntent};
use crate::{InspectError, header::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::ExtendedDateTime,
};
use tablecraft_expressions::functions::dates::clock::ClockReading;

const MAGIC: &[u8; 8] = b"TCNA0005";
const PREFIX: &[u8; 4] = b"TCNA";
const PROFILE: u16 = 5;
const ORIGINAL_MAGIC: &[u8; 8] = b"TCNA0002";
const ORIGINAL_PROFILE: u16 = 2;
const ORIGINAL_PROFILE_AT: usize = ORIGINAL_MAGIC.len() + size_of::<u32>();
const ORIGINAL_PROFILE_END: usize = ORIGINAL_PROFILE_AT + size_of::<u16>();
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const KIND_AT: usize = FLAGS_AT + size_of::<u16>();
const FIRST_RESERVED_AT: usize = KIND_AT + size_of::<u8>();
const YEAR_AT: usize = FIRST_RESERVED_AT + size_of::<u8>();
const MONTH_AT: usize = YEAR_AT + size_of::<u16>();
const DAY_AT: usize = MONTH_AT + size_of::<u8>();
const HOUR_AT: usize = DAY_AT + size_of::<u8>();
const MINUTE_AT: usize = HOUR_AT + size_of::<u8>();
const SECOND_AT: usize = MINUTE_AT + size_of::<u8>();
const RESERVED_AT: usize = SECOND_AT + size_of::<u8>();
const RESERVED_BYTES: usize = 7;
const HEADER_BYTES: usize = RESERVED_AT + RESERVED_BYTES;
const ABSENT: u8 = 0;
const CIVIL: u8 = 1;
const COPY_BYTES: usize = 4096;
pub(super) const MAX_RECORD_BYTES: usize = 64 * 1024 * 1024;

pub(super) fn length(nested: usize) -> Result<usize, NativeIntentError> {
    HEADER_BYTES
        .checked_add(nested)
        .filter(|length| *length <= MAX_RECORD_BYTES)
        .ok_or_else(|| InspectError::StructureLimit.into())
}

impl NativeClockInsertIntent {
    /// Encode only original intent and a retained clock; no provider is called.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        check(budget, cancelled)?;
        let total = length(self.original.encoded_length(budget, cancelled)?)?;
        let original = self.original.encode(budget, cancelled)?;
        if length(original.len())? != total {
            return Err(NativeIntentError::InvalidRecord);
        }
        budget
            .charge_allocation(total as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        let mut header = [0_u8; HEADER_BYTES];
        header[..MAGIC.len()].copy_from_slice(MAGIC);
        header[TOTAL_AT..PROFILE_AT].copy_from_slice(&(total as u32).to_le_bytes());
        header[PROFILE_AT..FLAGS_AT].copy_from_slice(&PROFILE.to_le_bytes());
        if let Some(reading) = self.reading {
            let civil = reading.date_time();
            let (year, month, day) = civil.date();
            let (hour, minute, second, fraction) = civil.time();
            if fraction != 0 {
                return Err(NativeIntentError::InvalidRecord);
            }
            header[KIND_AT] = CIVIL;
            header[YEAR_AT..MONTH_AT].copy_from_slice(&year.to_le_bytes());
            header[MONTH_AT] = month;
            header[DAY_AT] = day;
            header[HOUR_AT] = hour;
            header[MINUTE_AT] = minute;
            header[SECOND_AT] = second;
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(total)
            .map_err(|_| InspectError::Allocation)?;
        output.extend_from_slice(&header);
        for chunk in original.chunks(COPY_BYTES) {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, chunk.len() as u64)
                .map_err(InspectError::from)?;
            output.extend_from_slice(chunk);
        }
        check(budget, cancelled)?;
        Ok(output)
    }

    /// Validate outer framing before the existing nested preflight/allocation.
    /// Civil execution data is reconstructed without timezone or host reads.
    pub fn decode(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        check(budget, cancelled)?;
        if raw.len() < HEADER_BYTES || raw.len() > MAX_RECORD_BYTES {
            return Err(NativeIntentError::InvalidRecord);
        }
        budget
            .charge(Resource::Steps, HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        if raw.get(..MAGIC.len()) != Some(MAGIC.as_slice()) {
            return Err(if raw.get(..PREFIX.len()) == Some(PREFIX.as_slice()) {
                NativeIntentError::UnsupportedVersion
            } else {
                NativeIntentError::InvalidRecord
            });
        }
        if u32::from_le_bytes(raw[TOTAL_AT..PROFILE_AT].try_into().unwrap()) as usize != raw.len() {
            return Err(NativeIntentError::InvalidRecord);
        }
        if u16::from_le_bytes(raw[PROFILE_AT..FLAGS_AT].try_into().unwrap()) != PROFILE {
            return Err(NativeIntentError::UnsupportedProfile);
        }
        if raw[FLAGS_AT..KIND_AT].iter().any(|byte| *byte != 0)
            || raw[FIRST_RESERVED_AT] != 0
            || raw[RESERVED_AT..HEADER_BYTES].iter().any(|byte| *byte != 0)
        {
            return Err(NativeIntentError::InvalidRecord);
        }
        let reading = match raw[KIND_AT] {
            ABSENT => {
                if raw[YEAR_AT..RESERVED_AT].iter().any(|byte| *byte != 0) {
                    return Err(NativeIntentError::InvalidRecord);
                }
                None
            }
            CIVIL => {
                let year = u16::from_le_bytes(raw[YEAR_AT..MONTH_AT].try_into().unwrap());
                let civil = ExtendedDateTime::new(
                    year,
                    raw[MONTH_AT],
                    raw[DAY_AT],
                    raw[HOUR_AT],
                    raw[MINUTE_AT],
                    raw[SECOND_AT],
                    0,
                )
                .map_err(|_| NativeIntentError::InvalidRecord)?;
                Some(ClockReading::new(civil).map_err(|_| NativeIntentError::InvalidRecord)?)
            }
            _ => return Err(NativeIntentError::InvalidRecord),
        };
        // TCNA5 pins TCNA2's original row grammar. A newer omission-bearing
        // record must not widen an existing execution envelope by delegation.
        let nested = &raw[HEADER_BYTES..];
        budget
            .charge(Resource::Steps, ORIGINAL_PROFILE_END as u64)
            .map_err(InspectError::from)?;
        if nested.get(..ORIGINAL_MAGIC.len()) != Some(ORIGINAL_MAGIC.as_slice()) {
            return Err(if nested.starts_with(PREFIX) {
                NativeIntentError::UnsupportedVersion
            } else {
                NativeIntentError::InvalidRecord
            });
        }
        let profile = nested
            .get(ORIGINAL_PROFILE_AT..ORIGINAL_PROFILE_END)
            .ok_or(NativeIntentError::InvalidRecord)?;
        if u16::from_le_bytes(profile.try_into().unwrap()) != ORIGINAL_PROFILE {
            return Err(NativeIntentError::UnsupportedProfile);
        }
        let original = NativeOmittedInsertIntent::decode(nested, budget, cancelled)?;
        Self::from_original(original, reading, budget, cancelled)
    }
}
