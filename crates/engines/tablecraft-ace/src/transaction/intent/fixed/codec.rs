use super::{
    carrier,
    model::{self, NativeFixedEditIntent, NativeFixedIntentGroup},
};
use crate::transaction::intent::{NativeIntentError, binding, model::MAX_NAME_BYTES};
use crate::{FixedScalarUpdate, InspectError, NativeImage, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::transaction::intent) const MAGIC: &[u8; 8] = b"TCNI0003";
const PROFILE: u16 = 3;
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_LENGTH_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_LENGTH_AT + size_of::<u64>();
const BINDING_BYTES: usize = 32;
const TABLE_LENGTH_AT: usize = BINDING_AT + BINDING_BYTES;
const GROUP_COUNT_AT: usize = TABLE_LENGTH_AT + size_of::<u32>();
const ASSIGNMENT_COUNT_AT: usize = GROUP_COUNT_AT + size_of::<u32>();
const RESERVED_AT: usize = ASSIGNMENT_COUNT_AT + size_of::<u32>();
const HEADER_BYTES: usize = RESERVED_AT + size_of::<u32>();
const GROUP_NAME_LENGTH_AT: usize = 0;
const GROUP_ASSIGNMENTS_AT: usize = size_of::<u32>();
const GROUP_RESERVED_AT: usize = GROUP_ASSIGNMENTS_AT + size_of::<u32>();
const ROW_BYTES: usize = size_of::<u64>();
const REPLACEMENT_AT: usize = ROW_BYTES + carrier::WIDTH;
const GROUP_HEADER_BYTES: usize = GROUP_RESERVED_AT + size_of::<u32>();
const UPDATE_BYTES: usize = size_of::<u64>() + carrier::WIDTH * 2;
const COPY_BYTES: usize = 4096;
pub(super) const MAX_RECORD_BYTES: usize = HEADER_BYTES
    + MAX_NAME_BYTES
    + NativeImage::MAX_FIXED_BATCH_GROUPS * (GROUP_HEADER_BYTES + MAX_NAME_BYTES)
    + NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS * UPDATE_BYTES;

impl NativeFixedEditIntent {
    /// Encode the exact ordered original request data, with canonical carrier padding.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        let _span = tracing::debug_span!("native.fixed_intent.encode").entered();
        check(budget, cancelled)?;
        let count = model::assignment_count(self.groups(), budget, cancelled)?;
        let mut total = HEADER_BYTES + self.table().len();
        for group in self.groups() {
            check(budget, cancelled)?;
            total = total
                .checked_add(GROUP_HEADER_BYTES)
                .and_then(|n| n.checked_add(group.column().len()))
                .and_then(|n| n.checked_add(group.updates().len().checked_mul(UPDATE_BYTES)?))
                .filter(|n| *n <= MAX_RECORD_BYTES && *n <= u32::MAX as usize)
                .ok_or(InspectError::StructureLimit)?;
        }
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
        output.extend_from_slice(&(self.groups().len() as u32).to_le_bytes());
        output.extend_from_slice(&(count as u32).to_le_bytes());
        output.extend_from_slice(&0_u32.to_le_bytes());
        copy(&mut output, self.table().as_bytes(), budget, cancelled)?;
        for group in self.groups() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, GROUP_HEADER_BYTES as u64)
                .map_err(InspectError::from)?;
            output.extend_from_slice(&(group.column().len() as u32).to_le_bytes());
            output.extend_from_slice(&(group.updates().len() as u32).to_le_bytes());
            output.extend_from_slice(&0_u32.to_le_bytes());
            copy(&mut output, group.column().as_bytes(), budget, cancelled)?;
            for update in group.updates() {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, UPDATE_BYTES as u64)
                    .map_err(InspectError::from)?;
                output.extend_from_slice(&(update.row() as u64).to_le_bytes());
                output.extend_from_slice(&carrier::encode(update.expected())?);
                output.extend_from_slice(&carrier::encode(update.value())?);
            }
        }
        check(budget, cancelled)?;
        Ok(output)
    }

    /// Decode only TCNI0003/profile3; framing and bounds are checked before allocation.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let _span = tracing::debug_span!("native.fixed_intent.decode").entered();
        check(budget, cancelled)?;
        if bytes.len() < HEADER_BYTES || bytes.len() > MAX_RECORD_BYTES {
            return Err(NativeIntentError::InvalidRecord);
        }
        budget
            .charge(Resource::Steps, HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        if &bytes[..MAGIC.len()] != MAGIC {
            return Err(if bytes.starts_with(b"TCNI") {
                NativeIntentError::UnsupportedVersion
            } else {
                NativeIntentError::InvalidRecord
            });
        }
        if u16::from_le_bytes(bytes[PROFILE_AT..FLAGS_AT].try_into().unwrap()) != PROFILE {
            return Err(NativeIntentError::UnsupportedProfile);
        }
        if u32_at(bytes, TOTAL_AT) as usize != bytes.len()
            || bytes[FLAGS_AT..BEFORE_LENGTH_AT] != [0, 0]
            || u32_at(bytes, RESERVED_AT) != 0
        {
            return Err(NativeIntentError::InvalidRecord);
        }
        let before_length =
            u64::from_le_bytes(bytes[BEFORE_LENGTH_AT..BINDING_AT].try_into().unwrap());
        binding::length(before_length)?;
        let table_len = u32_at(bytes, TABLE_LENGTH_AT) as usize;
        let group_count = u32_at(bytes, GROUP_COUNT_AT) as usize;
        let count = u32_at(bytes, ASSIGNMENT_COUNT_AT) as usize;
        if group_count == 0
            || group_count > NativeImage::MAX_FIXED_BATCH_GROUPS
            || count > NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS
        {
            return Err(InspectError::StructureLimit.into());
        }
        // Check all variable framing, carrier tags and aggregate counts before reserving.
        let mut scan = Cursor {
            bytes,
            at: HEADER_BYTES,
        };
        scan.name(table_len, budget, cancelled)?;
        let mut actual = 0_usize;
        for _ in 0..group_count {
            let (_, entries) = scan.group(budget, cancelled)?;
            actual = actual
                .checked_add(entries)
                .filter(|n| *n <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
                .ok_or(InspectError::StructureLimit)?;
            for _ in 0..entries {
                scan.update(budget, cancelled)?;
            }
        }
        if actual != count || scan.at != bytes.len() {
            return Err(NativeIntentError::InvalidRecord);
        }
        let mut cursor = Cursor {
            bytes,
            at: HEADER_BYTES,
        };
        let table = cursor.name(table_len, budget, cancelled)?;
        budget
            .charge_allocation(
                group_count as u64,
                size_of::<NativeFixedIntentGroup>() as u64,
            )
            .map_err(InspectError::from)?;
        let mut groups = Vec::new();
        groups
            .try_reserve_exact(group_count)
            .map_err(|_| InspectError::Allocation)?;
        for _ in 0..group_count {
            let (column, count) = cursor.group(budget, cancelled)?;
            budget
                .charge_allocation(count as u64, size_of::<FixedScalarUpdate>() as u64)
                .map_err(InspectError::from)?;
            let mut updates = Vec::new();
            updates
                .try_reserve_exact(count)
                .map_err(|_| InspectError::Allocation)?;
            for _ in 0..count {
                updates.push(cursor.update(budget, cancelled)?);
            }
            groups.push(NativeFixedIntentGroup::new(
                column, updates, budget, cancelled,
            )?);
        }
        Self::from_decoded(
            table,
            groups,
            before_length,
            bytes[BINDING_AT..TABLE_LENGTH_AT].try_into().unwrap(),
            budget,
            cancelled,
        )
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], NativeIntentError> {
        let end = self
            .at
            .checked_add(count)
            .ok_or(NativeIntentError::InvalidRecord)?;
        let value = self
            .bytes
            .get(self.at..end)
            .ok_or(NativeIntentError::InvalidRecord)?;
        self.at = end;
        Ok(value)
    }

    fn name(
        &mut self,
        count: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<&'a str, NativeIntentError> {
        check(budget, cancelled)?;
        if count == 0 || count > MAX_NAME_BYTES {
            return Err(NativeIntentError::InvalidRecord);
        }
        budget
            .charge(Resource::Steps, count as u64)
            .map_err(InspectError::from)?;
        let value =
            std::str::from_utf8(self.take(count)?).map_err(|_| NativeIntentError::InvalidRecord)?;
        crate::transaction::intent::model::validate_name(value, budget, cancelled)?;
        Ok(value)
    }

    fn group(
        &mut self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(&'a str, usize), NativeIntentError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, GROUP_HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        let header = self.take(GROUP_HEADER_BYTES)?;
        if u32_at(header, GROUP_RESERVED_AT) != 0 {
            return Err(NativeIntentError::InvalidRecord);
        }
        let count = u32_at(header, GROUP_ASSIGNMENTS_AT) as usize;
        if count > NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS {
            return Err(InspectError::StructureLimit.into());
        }
        Ok((
            self.name(
                u32_at(header, GROUP_NAME_LENGTH_AT) as usize,
                budget,
                cancelled,
            )?,
            count,
        ))
    }

    fn update(
        &mut self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FixedScalarUpdate, NativeIntentError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, UPDATE_BYTES as u64)
            .map_err(InspectError::from)?;
        let raw = self.take(UPDATE_BYTES)?;
        let row = usize::try_from(u64::from_le_bytes(raw[..ROW_BYTES].try_into().unwrap()))
            .map_err(|_| InspectError::StructureLimit)?;
        if row >= crate::limits::MAX_LIVE_ROWS {
            return Err(InspectError::StructureLimit.into());
        }
        let expected = carrier::decode(raw[ROW_BYTES..REPLACEMENT_AT].try_into().unwrap())?;
        let replacement = carrier::decode(raw[REPLACEMENT_AT..UPDATE_BYTES].try_into().unwrap())?;
        Ok(FixedScalarUpdate::new(row, expected, replacement))
    }
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + size_of::<u32>()].try_into().unwrap())
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
