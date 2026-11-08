//! Closed TCQE0001 framing; full validation precedes string allocation.
use super::{
    Identity, MAX_LITERAL_BYTES, MAX_NAME_BYTES, NativeQueryExpressionIntent,
    QueryExpressionIntentError, binding, copy_text, validate_request,
};
use crate::{format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: &[u8; 8] = b"TCQE0001";
const PREFIX: &[u8; 4] = b"TCQE";
const PROFILE: u16 = 1;
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_LENGTH_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_LENGTH_AT + size_of::<u64>();
const BINDING_BYTES: usize = 32;
const NATIVE_ID_AT: usize = BINDING_AT + BINDING_BYTES;
const PARENT_ID_AT: usize = NATIVE_ID_AT + size_of::<i32>();
const CATALOG_PAGE_AT: usize = PARENT_ID_AT + size_of::<i32>();
const CATALOG_SLOT_AT: usize = CATALOG_PAGE_AT + size_of::<u32>();
const SLOT_RESERVED_AT: usize = CATALOG_SLOT_AT + size_of::<u8>();
const SLOT_RESERVED_BYTES: usize = 3;
const QUERY_LENGTH_AT: usize = SLOT_RESERVED_AT + SLOT_RESERVED_BYTES;
const EXPECTED_LENGTH_AT: usize = QUERY_LENGTH_AT + size_of::<u16>();
const REPLACEMENT_LENGTH_AT: usize = EXPECTED_LENGTH_AT + size_of::<u16>();
const RESERVED_AT: usize = REPLACEMENT_LENGTH_AT + size_of::<u16>();
const HEADER_BYTES: usize = RESERVED_AT + size_of::<u16>();
pub(super) const MAX_RECORD_BYTES: usize = HEADER_BYTES + MAX_NAME_BYTES + 2 * MAX_LITERAL_BYTES;

fn bytes<const N: usize>(input: &[u8], at: usize) -> Result<[u8; N], QueryExpressionIntentError> {
    input
        .get(at..at + N)
        .and_then(|raw| raw.try_into().ok())
        .ok_or(QueryExpressionIntentError::InvalidRecord)
}

pub(super) fn encode(
    intent: &NativeQueryExpressionIntent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, QueryExpressionIntentError> {
    check(budget, cancelled)?;
    validate_request(
        intent.query(),
        intent.expected_expression(),
        intent.replacement_expression(),
    )?;
    binding::length(intent.before_length)?;
    validate_identity(intent.identity, intent.before_length)?;
    let length =
        HEADER_BYTES + intent.query.len() + intent.expected.len() + intent.replacement.len();
    budget.charge_allocation(length as u64, 1)?;
    budget.charge(Resource::Steps, length as u64)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| crate::InspectError::Allocation)?;
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&(length as u32).to_le_bytes());
    output.extend_from_slice(&PROFILE.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&intent.before_length.to_le_bytes());
    output.extend_from_slice(&intent.before_binding);
    output.extend_from_slice(&intent.identity.native_id.to_le_bytes());
    output.extend_from_slice(&intent.identity.parent_id.to_le_bytes());
    output.extend_from_slice(&intent.identity.catalog_page.to_le_bytes());
    output.push(intent.identity.catalog_slot);
    output.extend_from_slice(&[0; SLOT_RESERVED_BYTES]);
    output.extend_from_slice(&(intent.query.len() as u16).to_le_bytes());
    output.extend_from_slice(&(intent.expected.len() as u16).to_le_bytes());
    output.extend_from_slice(&(intent.replacement.len() as u16).to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(intent.query.as_bytes());
    output.extend_from_slice(intent.expected.as_bytes());
    output.extend_from_slice(intent.replacement.as_bytes());
    check(budget, cancelled)?;
    Ok(output)
}

pub(super) fn decode(
    input: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeQueryExpressionIntent, QueryExpressionIntentError> {
    check(budget, cancelled)?;
    if !(HEADER_BYTES..=MAX_RECORD_BYTES).contains(&input.len())
        || input.get(..PREFIX.len()) != Some(PREFIX)
    {
        return Err(QueryExpressionIntentError::InvalidRecord);
    }
    if input.get(..MAGIC.len()) != Some(MAGIC) {
        return Err(QueryExpressionIntentError::UnsupportedVersion);
    }
    budget.charge(Resource::Steps, input.len() as u64)?;
    if u32::from_le_bytes(bytes(input, TOTAL_AT)?) as usize != input.len()
        || u16::from_le_bytes(bytes(input, FLAGS_AT)?) != 0
        || input[SLOT_RESERVED_AT..QUERY_LENGTH_AT] != [0; SLOT_RESERVED_BYTES]
        || u16::from_le_bytes(bytes(input, RESERVED_AT)?) != 0
    {
        return Err(QueryExpressionIntentError::InvalidRecord);
    }
    if u16::from_le_bytes(bytes(input, PROFILE_AT)?) != PROFILE {
        return Err(QueryExpressionIntentError::UnsupportedProfile);
    }
    let before_length = u64::from_le_bytes(bytes(input, BEFORE_LENGTH_AT)?);
    binding::length(before_length)?;
    let identity = Identity {
        native_id: i32::from_le_bytes(bytes(input, NATIVE_ID_AT)?),
        parent_id: i32::from_le_bytes(bytes(input, PARENT_ID_AT)?),
        catalog_page: u32::from_le_bytes(bytes(input, CATALOG_PAGE_AT)?),
        catalog_slot: input[CATALOG_SLOT_AT],
    };
    validate_identity(identity, before_length)?;
    let query_length = usize::from(u16::from_le_bytes(bytes(input, QUERY_LENGTH_AT)?));
    let expected_length = usize::from(u16::from_le_bytes(bytes(input, EXPECTED_LENGTH_AT)?));
    let replacement_length = usize::from(u16::from_le_bytes(bytes(input, REPLACEMENT_LENGTH_AT)?));
    if query_length > MAX_NAME_BYTES
        || expected_length > MAX_LITERAL_BYTES
        || replacement_length > MAX_LITERAL_BYTES
        || HEADER_BYTES + query_length + expected_length + replacement_length != input.len()
    {
        return Err(QueryExpressionIntentError::InvalidRecord);
    }
    let query_end = HEADER_BYTES + query_length;
    let expected_end = query_end + expected_length;
    let query = std::str::from_utf8(&input[HEADER_BYTES..query_end])
        .map_err(|_| QueryExpressionIntentError::InvalidRecord)?;
    let expected = std::str::from_utf8(&input[query_end..expected_end])
        .map_err(|_| QueryExpressionIntentError::InvalidRecord)?;
    let replacement = std::str::from_utf8(&input[expected_end..])
        .map_err(|_| QueryExpressionIntentError::InvalidRecord)?;
    validate_request(query, expected, replacement)?;
    let result = NativeQueryExpressionIntent {
        identity,
        query: copy_text(query, budget)?,
        expected: copy_text(expected, budget)?,
        replacement: copy_text(replacement, budget)?,
        before_length,
        before_binding: bytes(input, BINDING_AT)?,
    };
    check(budget, cancelled)?;
    Ok(result)
}

fn validate_identity(
    identity: Identity,
    before_length: u64,
) -> Result<(), QueryExpressionIntentError> {
    if !before_length.is_multiple_of(PAGE_BYTES as u64)
        || identity.catalog_page == 0
        || u64::from(identity.catalog_page) >= before_length / PAGE_BYTES as u64
        || identity.catalog_slot == u8::MAX
    {
        return Err(QueryExpressionIntentError::InvalidRecord);
    }
    Ok(())
}
