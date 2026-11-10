//! Shared original-group framing; public fitted encoding retains TCNI0005.
use super::{
    carrier,
    grammar::Grammar,
    model::{self, Family, NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates},
    profile::WireProfile,
    read,
};
use crate::{InspectError, header::check, transaction::intent::NativeIntentError};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(in crate::transaction::intent) const MAGIC: &[u8; 8] = b"TCNI0005";
pub(super) const PROFILE: u16 = 5;
pub(super) const TOTAL_AT: usize = MAGIC.len();
pub(super) const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
pub(super) const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
pub(super) const BEFORE_LENGTH_AT: usize = FLAGS_AT + size_of::<u16>();
pub(super) const BINDING_AT: usize = BEFORE_LENGTH_AT + size_of::<u64>();
pub(super) const BINDING_BYTES: usize = 32;
pub(super) const TABLE_LENGTH_AT: usize = BINDING_AT + BINDING_BYTES;
pub(super) const GROUP_COUNT_AT: usize = TABLE_LENGTH_AT + size_of::<u32>();
pub(super) const ASSIGNMENT_COUNT_AT: usize = GROUP_COUNT_AT + size_of::<u32>();
pub(super) const RESERVED_AT: usize = ASSIGNMENT_COUNT_AT + size_of::<u32>();
pub(super) const HEADER_BYTES: usize = RESERVED_AT + size_of::<u32>();
pub(super) const GROUP_TOTAL_AT: usize = 0;
pub(super) const GROUP_NAME_AT: usize = GROUP_TOTAL_AT + size_of::<u32>();
pub(super) const GROUP_COUNT_AT_IN_GROUP: usize = GROUP_NAME_AT + size_of::<u32>();
pub(super) const GROUP_FAMILY_AT: usize = GROUP_COUNT_AT_IN_GROUP + size_of::<u32>();
pub(super) const GROUP_PADDING_AT: usize = GROUP_FAMILY_AT + size_of::<u8>();
pub(super) const GROUP_RESERVED_AT: usize = 16;
pub(super) const GROUP_HEADER_BYTES: usize = GROUP_RESERVED_AT + size_of::<u32>();
pub(super) const UPDATE_HEADER_BYTES: usize = size_of::<u32>() + size_of::<u64>();
pub(super) const MIN_UPDATE_BYTES: usize = UPDATE_HEADER_BYTES + 2 * carrier::HEADER_BYTES;
pub(super) const CARRIER_TAG_AT: usize = 0;
pub(super) const CARRIER_PADDING_AT: usize = 1;
pub(super) const CARRIER_LENGTH_AT: usize = 4;
// A consumer ceiling, independent of native field fit or file eligibility.
pub(super) const MAX_RECORD_BYTES: usize = crate::limits::MAX_IMAGE_BYTES as usize;

fn add(total: usize, bytes: usize, grammar: Grammar) -> Result<usize, NativeIntentError> {
    total
        .checked_add(bytes)
        .filter(|total| *total <= grammar.record_bytes())
        .ok_or_else(|| InspectError::StructureLimit.into())
}

fn update_length(
    expected: &Value,
    value: &Value,
    family: Family,
    grammar: Grammar,
) -> Result<usize, NativeIntentError> {
    let expected = grammar.view(expected, family)?;
    let value = grammar.view(value, family)?;
    add(
        add(MIN_UPDATE_BYTES, expected.bytes().len(), grammar)?,
        value.bytes().len(),
        grammar,
    )
}

fn group_length(
    group: &NativeFittedIntentGroup,
    grammar: Grammar,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, NativeIntentError> {
    group_parts_length(group.column(), group.updates(), grammar, budget, cancelled)
}

pub(super) fn group_parts_length(
    column: &str,
    updates: &NativeFittedUpdates,
    grammar: Grammar,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, NativeIntentError> {
    let mut length = add(GROUP_HEADER_BYTES, column.len(), grammar)?;
    for cell in updates.cells() {
        check(budget, cancelled)?;
        length = add(
            length,
            update_length(cell.expected, cell.value, updates.family(), grammar)?,
            grammar,
        )?;
    }
    Ok(length)
}

pub(super) fn encoded_length(
    intent: &NativeFittedEditIntent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, NativeIntentError> {
    parts_length(
        intent.table(),
        intent.groups(),
        intent.grammar,
        budget,
        cancelled,
    )
}

pub(super) fn parts_length(
    table: &str,
    groups: &[NativeFittedIntentGroup],
    grammar: Grammar,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, NativeIntentError> {
    model::validate_groups(groups, grammar, budget, cancelled)?;
    let mut total = add(HEADER_BYTES, table.len(), grammar)?;
    for group in groups {
        check(budget, cancelled)?;
        total = add(
            total,
            group_length(group, grammar, budget, cancelled)?,
            grammar,
        )?;
    }
    check(budget, cancelled)?;
    Ok(total)
}

fn copy(
    output: &mut Vec<u8>,
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), NativeIntentError> {
    for chunk in bytes.chunks(carrier::COPY_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        output.extend_from_slice(chunk);
    }
    check(budget, cancelled)?;
    Ok(())
}

fn value(
    output: &mut Vec<u8>,
    value: &Value,
    family: Family,
    grammar: Grammar,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), NativeIntentError> {
    check(budget, cancelled)?;
    let encoded = grammar.view(value, family)?;
    budget
        .charge(Resource::Steps, carrier::HEADER_BYTES as u64)
        .map_err(InspectError::from)?;
    output.push(encoded.tag);
    output.extend_from_slice(&[0; CARRIER_LENGTH_AT - CARRIER_PADDING_AT]);
    output.extend_from_slice(&(encoded.bytes().len() as u32).to_le_bytes());
    copy(output, encoded.bytes(), budget, cancelled)
}

impl NativeFittedEditIntent {
    /// Encode exact original groups; no final records or permission is retained.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        self.encode_profile(WireProfile::Fitted, budget, cancelled)
    }

    pub(in crate::transaction::intent) fn encode_profile(
        &self,
        profile: WireProfile,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        let _span = tracing::debug_span!("native.fitted_intent.encode").entered();
        let grammar = profile.grammar();
        if self.grammar != grammar {
            return Err(NativeIntentError::UnsupportedProfile);
        }
        let total = encoded_length(self, budget, cancelled)?;
        let count = model::validate_groups(self.groups(), grammar, budget, cancelled)?;
        budget
            .charge_allocation(total as u64, 1)
            .map_err(InspectError::from)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(total)
            .map_err(|_| InspectError::Allocation)?;
        budget
            .charge(Resource::Steps, HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        output.extend_from_slice(profile.magic());
        output.extend_from_slice(&(total as u32).to_le_bytes());
        output.extend_from_slice(&profile.number().to_le_bytes());
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
            let total = group_length(group, grammar, budget, cancelled)?;
            budget
                .charge(Resource::Steps, GROUP_HEADER_BYTES as u64)
                .map_err(InspectError::from)?;
            output.extend_from_slice(&(total as u32).to_le_bytes());
            output.extend_from_slice(&(group.column().len() as u32).to_le_bytes());
            output.extend_from_slice(&(group.updates().len() as u32).to_le_bytes());
            output.push(match group.updates().family() {
                Family::Fixed => 0,
                Family::Variable => 1,
            });
            output.extend_from_slice(&[0; GROUP_RESERVED_AT - GROUP_PADDING_AT]);
            output.extend_from_slice(&0_u32.to_le_bytes());
            copy(&mut output, group.column().as_bytes(), budget, cancelled)?;
            for cell in group.updates().cells() {
                check(budget, cancelled)?;
                let family = group.updates().family();
                let length = update_length(cell.expected, cell.value, family, grammar)?;
                budget
                    .charge(Resource::Steps, UPDATE_HEADER_BYTES as u64)
                    .map_err(InspectError::from)?;
                output.extend_from_slice(&(length as u32).to_le_bytes());
                output.extend_from_slice(&(cell.row as u64).to_le_bytes());
                value(
                    &mut output,
                    cell.expected,
                    family,
                    grammar,
                    budget,
                    cancelled,
                )?;
                value(&mut output, cell.value, family, grammar, budget, cancelled)?;
            }
        }
        if output.len() != total {
            return Err(NativeIntentError::InvalidRecord);
        }
        check(budget, cancelled)?;
        Ok(output)
    }

    /// Encode only retained TCNI17 data; older fitted grammars refuse.
    pub fn encode_retained(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        self.encode_profile(WireProfile::RetainedRows, budget, cancelled)
    }

    /// Decode only TCNI17, retaining its actual wire-width grammar.
    pub fn decode_retained(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        read::decode(bytes, WireProfile::RetainedRows, budget, cancelled)
    }

    /// Encode only TCNI18 retained opaque-row request data, never source authority.
    pub fn encode_retained_opaque(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        self.encode_profile(WireProfile::RetainedOpaqueRows, budget, cancelled)
    }

    /// Decode only TCNI0018/profile18; the original TCNI17 decoder stays closed.
    pub fn decode_retained_opaque(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        read::decode(bytes, WireProfile::RetainedOpaqueRows, budget, cancelled)
    }

    /// Decode profile5 only, with complete nonallocating preflight first.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        read::decode(bytes, WireProfile::Fitted, budget, cancelled)
    }

    pub(in crate::transaction::intent) fn decode_profile(
        bytes: &[u8],
        profile: WireProfile,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        read::decode(bytes, profile, budget, cancelled)
    }
}
