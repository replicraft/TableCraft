//! Closed TCQE0002 framing; every entry is validated before owned allocation.
use super::{
    Identity, MAX_EDITS, MAX_EXPRESSION_BYTES, MAX_NAME_BYTES, NativeQueryExpressionEditsIntent,
    OwnedQueryFragmentExpressionEdit, QueryExpressionEditsIntentError as Error, binding, copy_text,
    validate_edit, validate_name,
};
use crate::{
    InspectError, QueryAttribute, QueryFragmentExpressionEdit, format::page::PAGE_BYTES,
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: &[u8; 8] = b"TCQE0002";
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
const EDIT_COUNT_AT: usize = QUERY_LENGTH_AT + size_of::<u16>();
const ENTRIES_LENGTH_AT: usize = EDIT_COUNT_AT + size_of::<u16>();
const HEADER_BYTES: usize = ENTRIES_LENGTH_AT + size_of::<u32>();
const ENTRY_FLAGS_AT: usize = size_of::<u8>();
const EXPECTED_LENGTH_AT: usize = ENTRY_FLAGS_AT + size_of::<u8>();
const REPLACEMENT_LENGTH_AT: usize = EXPECTED_LENGTH_AT + size_of::<u16>();
const ENTRY_RESERVED_AT: usize = REPLACEMENT_LENGTH_AT + size_of::<u16>();
const ORDER_AT: usize = ENTRY_RESERVED_AT + size_of::<u16>();
const ORDER_BYTES: usize = size_of::<u32>();
const ENTRY_HEADER_BYTES: usize = ORDER_AT + ORDER_BYTES;
pub(super) const MAX_RECORD_BYTES: usize =
    HEADER_BYTES + MAX_NAME_BYTES + MAX_EDITS * (ENTRY_HEADER_BYTES + 2 * MAX_EXPRESSION_BYTES);

fn bytes<const N: usize>(input: &[u8], at: usize) -> Result<[u8; N], Error> {
    let end = at.checked_add(N).ok_or(Error::InvalidRecord)?;
    input
        .get(at..end)
        .and_then(|raw| raw.try_into().ok())
        .ok_or(Error::InvalidRecord)
}

struct Entry<'a> {
    attribute: QueryAttribute,
    order: [u8; ORDER_BYTES],
    expected: &'a str,
    replacement: &'a str,
}

fn entry<'a>(
    input: &'a [u8],
    at: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Entry<'a>, usize), Error> {
    check(budget, cancelled)?;
    let header_end = at
        .checked_add(ENTRY_HEADER_BYTES)
        .ok_or(Error::InvalidRecord)?;
    let header = input.get(at..header_end).ok_or(Error::InvalidRecord)?;
    if header[ENTRY_FLAGS_AT] != 0 || u16::from_le_bytes(bytes(header, ENTRY_RESERVED_AT)?) != 0 {
        return Err(Error::InvalidRecord);
    }
    let expected_length = usize::from(u16::from_le_bytes(bytes(header, EXPECTED_LENGTH_AT)?));
    let replacement_length = usize::from(u16::from_le_bytes(bytes(header, REPLACEMENT_LENGTH_AT)?));
    if expected_length > MAX_EXPRESSION_BYTES || replacement_length > MAX_EXPRESSION_BYTES {
        return Err(Error::InvalidRecord);
    }
    let expected_start = header_end;
    let expected_end = expected_start
        .checked_add(expected_length)
        .ok_or(Error::InvalidRecord)?;
    let end = expected_end
        .checked_add(replacement_length)
        .ok_or(Error::InvalidRecord)?;
    let expected = std::str::from_utf8(
        input
            .get(expected_start..expected_end)
            .ok_or(Error::InvalidRecord)?,
    )
    .map_err(|_| Error::InvalidRecord)?;
    let replacement =
        std::str::from_utf8(input.get(expected_end..end).ok_or(Error::InvalidRecord)?)
            .map_err(|_| Error::InvalidRecord)?;
    let attribute = QueryAttribute::from_code(header[0]);
    validate_edit(attribute, expected, replacement)?;
    budget.charge(Resource::Steps, (end - at) as u64)?;
    Ok((
        Entry {
            attribute,
            order: bytes(header, ORDER_AT)?,
            expected,
            replacement,
        },
        end,
    ))
}

pub(super) fn encode(
    intent: &NativeQueryExpressionEditsIntent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    check(budget, cancelled)?;
    intent.validate(budget, cancelled)?;
    binding::length(intent.before_length)?;
    validate_identity(intent.identity, intent.before_length)?;
    let entries_length = intent
        .edits
        .iter()
        .map(|edit| {
            let edit = edit.as_request();
            ENTRY_HEADER_BYTES + edit.expected.len() + edit.replacement.len()
        })
        .sum::<usize>();
    let length = HEADER_BYTES + intent.query.len() + entries_length;
    if length > MAX_RECORD_BYTES {
        return Err(Error::InvalidRecord);
    }

    budget.charge_allocation(length as u64, 1)?;
    budget.charge(Resource::Steps, length as u64)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
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
    output.extend_from_slice(&(intent.edits.len() as u16).to_le_bytes());
    output.extend_from_slice(&(entries_length as u32).to_le_bytes());
    output.extend_from_slice(intent.query.as_bytes());
    for edit in &intent.edits {
        check(budget, cancelled)?;
        let edit = edit.as_request();
        output.push(edit.attribute.code());
        output.push(0);
        output.extend_from_slice(&(edit.expected.len() as u16).to_le_bytes());
        output.extend_from_slice(&(edit.replacement.len() as u16).to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&edit.order);
        output.extend_from_slice(edit.expected.as_bytes());
        output.extend_from_slice(edit.replacement.as_bytes());
    }
    check(budget, cancelled)?;
    Ok(output)
}

pub(super) fn decode(
    input: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeQueryExpressionEditsIntent, Error> {
    check(budget, cancelled)?;
    if !(HEADER_BYTES..=MAX_RECORD_BYTES).contains(&input.len())
        || input.get(..PREFIX.len()) != Some(PREFIX)
    {
        return Err(Error::InvalidRecord);
    }
    if input.get(..MAGIC.len()) != Some(MAGIC) {
        return Err(Error::UnsupportedVersion);
    }
    budget.charge(Resource::Steps, input.len() as u64)?;
    if u32::from_le_bytes(bytes(input, TOTAL_AT)?) as usize != input.len()
        || u16::from_le_bytes(bytes(input, FLAGS_AT)?) != 0
        || input[SLOT_RESERVED_AT..QUERY_LENGTH_AT] != [0; SLOT_RESERVED_BYTES]
    {
        return Err(Error::InvalidRecord);
    }
    if u16::from_le_bytes(bytes(input, PROFILE_AT)?) != PROFILE {
        return Err(Error::UnsupportedProfile);
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
    let count = usize::from(u16::from_le_bytes(bytes(input, EDIT_COUNT_AT)?));
    let entries_length = u32::from_le_bytes(bytes(input, ENTRIES_LENGTH_AT)?) as usize;
    let total = HEADER_BYTES
        .checked_add(query_length)
        .and_then(|length| length.checked_add(entries_length));
    if query_length > MAX_NAME_BYTES || count > MAX_EDITS || total != Some(input.len()) {
        return Err(Error::InvalidRecord);
    }
    let entries_start = HEADER_BYTES + query_length;
    let query = std::str::from_utf8(
        input
            .get(HEADER_BYTES..entries_start)
            .ok_or(Error::InvalidRecord)?,
    )
    .map_err(|_| Error::InvalidRecord)?;
    validate_name(query)?;

    // Structural validation and duplicate-slot refusal allocate no owned list or
    // string. Repeated borrowed scans spend actual framing/text work and check
    // cancellation, so adversarial large lists cannot bypass cumulative limits.
    let mut cursor = entries_start;
    for _ in 0..count {
        let current_at = cursor;
        let (current, end) = entry(input, cursor, budget, cancelled)?;
        let mut prior_at = entries_start;
        while prior_at < current_at {
            let (prior, next) = entry(input, prior_at, budget, cancelled)?;
            if prior.attribute == current.attribute && prior.order == current.order {
                return Err(Error::InvalidRecord);
            }
            prior_at = next;
        }
        cursor = end;
    }
    if cursor != input.len() {
        return Err(Error::InvalidRecord);
    }

    budget.charge_allocation(
        count as u64,
        size_of::<OwnedQueryFragmentExpressionEdit>() as u64,
    )?;
    let mut edits = Vec::new();
    edits
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    cursor = entries_start;
    for _ in 0..count {
        let (current, end) = entry(input, cursor, budget, cancelled)?;
        let request = QueryFragmentExpressionEdit {
            attribute: current.attribute,
            order: current.order,
            expected: current.expected,
            replacement: current.replacement,
        };
        edits.push(OwnedQueryFragmentExpressionEdit::copy_from_request(
            &request, budget, cancelled,
        )?);
        cursor = end;
    }
    let intent = NativeQueryExpressionEditsIntent {
        identity,
        query: copy_text(query, budget, cancelled)?,
        edits,
        before_length,
        before_binding: bytes(input, BINDING_AT)?,
    };
    check(budget, cancelled)?;
    Ok(intent)
}

fn validate_identity(identity: Identity, before_length: u64) -> Result<(), Error> {
    if !before_length.is_multiple_of(PAGE_BYTES as u64)
        || identity.catalog_page == 0
        || u64::from(identity.catalog_page) >= before_length / PAGE_BYTES as u64
        || identity.catalog_slot == u8::MAX
    {
        return Err(Error::InvalidRecord);
    }
    Ok(())
}
