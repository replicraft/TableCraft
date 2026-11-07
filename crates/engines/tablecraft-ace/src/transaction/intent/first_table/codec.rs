use super::super::fitted::carrier;
use super::{FirstTableIntentError, NativeFirstTableIntent, binding};
use crate::{
    CreateColumn, FirstTableDefinition, InspectError, format::page::PAGE_BYTES, header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::{DecimalConstraints, ScalarType};

const MAGIC: &[u8; 8] = b"TCNT0001";
const DEFAULT_MAGIC: &[u8; 8] = b"TCNT0002";
const POLICY_MAGIC: &[u8; 8] = b"TCNT0003";
const DEFAULT_PRESENT: u8 = 0x01;
const REQUIRED_PRESENT: u8 = 0x02;
const REQUIRED_VALUE: u8 = 0x04;
const ALLOW_EMPTY_PRESENT: u8 = 0x08;
const ALLOW_EMPTY_VALUE: u8 = 0x10;
const POLICY_FLAGS: u8 =
    DEFAULT_PRESENT | REQUIRED_PRESENT | REQUIRED_VALUE | ALLOW_EMPTY_PRESENT | ALLOW_EMPTY_VALUE;
// Pinned profile-2 ceiling: the existing constant literal expression allows
// at most 255 UTF16 units, so no original text can exceed this UTF8 bound.
const MAX_DEFAULT_EXPRESSION_UNITS: usize = 255;
const MAX_DEFAULT_BYTES: usize = MAX_DEFAULT_EXPRESSION_UNITS * char::MAX.len_utf8();
const DEFAULT_HEADER_BYTES: usize = carrier::HEADER_BYTES;
const PREFIX: &[u8; 4] = b"TCNT";
// This construction profile pins the canonical seven-page blank and current
// explicit empty Increment serializer. Altering those rules requires a new
// profile while retaining this reconstruction for existing pending journals.
const CONSTRUCTION_PROFILE: u16 = 1;
const DEFAULT_CONSTRUCTION_PROFILE: u16 = 2;
const POLICY_CONSTRUCTION_PROFILE: u16 = 3;
const CANONICAL_BEFORE_PAGES: u64 = 7;
const CANONICAL_BEFORE_BYTES: u64 = CANONICAL_BEFORE_PAGES * PAGE_BYTES as u64;
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_AT + size_of::<u64>();
const BINDING_BYTES: usize = 32;
// Full canonical V1 BEFORE binding under the existing TCNI0001:BEFORE domain.
// This rejects producer drift rather than silently reusing profile 1 for a new
// blank construction. The independent raw blank fixture pins these bytes too.
const CANONICAL_BEFORE_BINDING: [u8; BINDING_BYTES] = [
    0xb8, 0x41, 0xe3, 0x67, 0xe3, 0x88, 0x19, 0xa7, 0xd8, 0xc7, 0x41, 0xa2, 0x4c, 0x2f, 0xdc, 0x7f,
    0xe8, 0xdd, 0xf3, 0x7c, 0x20, 0x0a, 0x5d, 0xb9, 0xa3, 0xf3, 0x19, 0x35, 0xe6, 0x2a, 0x2e, 0x77,
];
const TABLE_LENGTH_AT: usize = BINDING_AT + BINDING_BYTES;
const PRIMARY_LENGTH_AT: usize = TABLE_LENGTH_AT + size_of::<u16>();
const COLUMNS_AT: usize = PRIMARY_LENGTH_AT + size_of::<u16>();
const RESERVED_AT: usize = COLUMNS_AT + size_of::<u16>();
const HEADER_BYTES: usize = RESERVED_AT + size_of::<u16>();
const MAX_COLUMNS: usize = 16;
const MAX_NAME_BYTES: usize = 64 * 4;
const FIELD_HEADER_BYTES: usize = 8;
const TEXT_FIELD_TAG: u8 = 11;
const V1_MAX_RECORD_BYTES: usize =
    HEADER_BYTES + 2 * MAX_NAME_BYTES + MAX_COLUMNS * (FIELD_HEADER_BYTES + MAX_NAME_BYTES);
pub(super) const MAX_RECORD_BYTES: usize =
    V1_MAX_RECORD_BYTES + MAX_COLUMNS * (DEFAULT_HEADER_BYTES + MAX_DEFAULT_BYTES);

fn word(raw: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(raw[at..at + size_of::<u16>()].try_into().unwrap())
}

fn field(column: CreateColumn<'_>) -> Result<(u8, u16, u8, u8), FirstTableIntentError> {
    if column.incrementing_autonumber() {
        return Ok((0, 0, 0, 0));
    }
    let tag = match column.scalar_type() {
        ScalarType::Boolean => 1,
        ScalarType::Byte => 2,
        ScalarType::Int16 => 3,
        ScalarType::Int32 => 4,
        ScalarType::Int64 => 5,
        ScalarType::Currency => 6,
        ScalarType::Single => 7,
        ScalarType::Double => 8,
        ScalarType::DateTime => 9,
        ScalarType::Guid => 10,
        ScalarType::DateTimeExtended => return Err(FirstTableIntentError::InvalidRecord),
        ScalarType::Text => {
            let units = column
                .max_text_utf16_units()
                .ok_or(FirstTableIntentError::InvalidRecord)?;
            return Ok((TEXT_FIELD_TAG, units as u16, 0, 0));
        }
        ScalarType::Binary => {
            let bytes = column
                .max_binary_bytes()
                .ok_or(FirstTableIntentError::InvalidRecord)?;
            return Ok((12, bytes as u16, 0, 0));
        }
        ScalarType::Decimal => {
            let constraints = column
                .decimal_constraints()
                .ok_or(FirstTableIntentError::InvalidRecord)?;
            return Ok((13, 0, constraints.precision(), constraints.scale()));
        }
    };
    Ok((tag, 0, 0, 0))
}

pub(super) fn encode(
    definition: &FirstTableDefinition,
    before: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, FirstTableIntentError> {
    check(budget, cancelled)?;
    if before.len() as u64 != CANONICAL_BEFORE_BYTES {
        return Err(FirstTableIntentError::BeforeMismatch);
    }
    let columns = definition.borrowed_columns(budget, cancelled)?;
    let has_defaults = columns
        .iter()
        .any(|column| column.default_literal().is_some());
    let has_policies = columns
        .iter()
        .any(|column| column.required().is_some() || column.allow_zero_length().is_some());
    let mut default_bytes = 0;
    for column in &columns {
        if let Some(value) = column.default_literal() {
            let encoded = carrier::row_view(value)?;
            if encoded.bytes().len() > MAX_DEFAULT_BYTES {
                return Err(FirstTableIntentError::InvalidRecord);
            }
            default_bytes += DEFAULT_HEADER_BYTES + encoded.bytes().len();
        }
    }
    let total = default_bytes
        + HEADER_BYTES
        + definition.table().len()
        + definition.primary_name().len()
        + columns
            .iter()
            .map(|column| FIELD_HEADER_BYTES + column.name().len())
            .sum::<usize>();
    if total > MAX_RECORD_BYTES {
        return Err(FirstTableIntentError::InvalidRecord);
    }
    let binding = binding::digest(before, budget, cancelled)?;
    budget
        .charge(Resource::Steps, BINDING_BYTES as u64)
        .map_err(InspectError::from)?;
    if binding != CANONICAL_BEFORE_BINDING {
        return Err(FirstTableIntentError::BeforeMismatch);
    }
    budget
        .charge_allocation(total as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, 2 * total as u64)
        .map_err(InspectError::from)?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(total)
        .map_err(|_| InspectError::Allocation)?;
    raw.resize(HEADER_BYTES, 0);
    let magic = if has_policies {
        POLICY_MAGIC
    } else if has_defaults {
        DEFAULT_MAGIC
    } else {
        MAGIC
    };
    let profile = if has_policies {
        POLICY_CONSTRUCTION_PROFILE
    } else if has_defaults {
        DEFAULT_CONSTRUCTION_PROFILE
    } else {
        CONSTRUCTION_PROFILE
    };
    raw[..MAGIC.len()].copy_from_slice(magic);
    raw[TOTAL_AT..PROFILE_AT].copy_from_slice(&(total as u32).to_le_bytes());
    raw[PROFILE_AT..FLAGS_AT].copy_from_slice(&profile.to_le_bytes());
    raw[BEFORE_AT..BINDING_AT].copy_from_slice(&(before.len() as u64).to_le_bytes());
    raw[BINDING_AT..TABLE_LENGTH_AT].copy_from_slice(&binding);
    raw[TABLE_LENGTH_AT..PRIMARY_LENGTH_AT]
        .copy_from_slice(&(definition.table().len() as u16).to_le_bytes());
    raw[PRIMARY_LENGTH_AT..COLUMNS_AT]
        .copy_from_slice(&(definition.primary_name().len() as u16).to_le_bytes());
    raw[COLUMNS_AT..RESERVED_AT].copy_from_slice(&(columns.len() as u16).to_le_bytes());
    raw.extend(definition.table().as_bytes());
    raw.extend(definition.primary_name().as_bytes());
    for column in columns {
        check(budget, cancelled)?;
        let (tag, capacity, precision, scale) = field(column)?;
        raw.extend((column.name().len() as u16).to_le_bytes());
        let mut flags = u8::from(column.default_literal().is_some());
        if let Some(required) = column.required() {
            flags |= REQUIRED_PRESENT | if required { REQUIRED_VALUE } else { 0 };
        }
        if let Some(allow) = column.allow_zero_length() {
            flags |= ALLOW_EMPTY_PRESENT | if allow { ALLOW_EMPTY_VALUE } else { 0 };
        }
        raw.extend([tag, flags]);
        raw.extend(capacity.to_le_bytes());
        raw.extend([precision, scale]);
        raw.extend(column.name().as_bytes());
        if let Some(value) = column.default_literal() {
            let encoded = carrier::row_view(value)?;
            raw.extend([encoded.tag, 0, 0, 0]);
            raw.extend((encoded.bytes().len() as u32).to_le_bytes());
            raw.extend(encoded.bytes());
        }
    }
    check(budget, cancelled)?;
    Ok(raw)
}

struct Cursor<'a> {
    raw: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], FirstTableIntentError> {
        let end = self
            .at
            .checked_add(length)
            .ok_or(FirstTableIntentError::InvalidRecord)?;
        let bytes = self
            .raw
            .get(self.at..end)
            .ok_or(FirstTableIntentError::InvalidRecord)?;
        self.at = end;
        Ok(bytes)
    }

    fn name(&mut self, length: usize) -> Result<&'a str, FirstTableIntentError> {
        if length == 0 || length > MAX_NAME_BYTES {
            return Err(FirstTableIntentError::InvalidRecord);
        }
        std::str::from_utf8(self.take(length)?).map_err(|_| FirstTableIntentError::InvalidRecord)
    }
}

fn column<'a>(name: &'a str, header: &[u8]) -> Result<CreateColumn<'a>, FirstTableIntentError> {
    let tag = header[2];
    let capacity = usize::from(word(header, 4));
    let precision = header[6];
    let scale = header[7];
    if tag != TEXT_FIELD_TAG && tag != 12 && capacity != 0
        || tag != 13 && (precision != 0 || scale != 0)
    {
        return Err(FirstTableIntentError::InvalidRecord);
    }
    let scalar = match tag {
        0 | 4 => ScalarType::Int32,
        1 => ScalarType::Boolean,
        2 => ScalarType::Byte,
        3 => ScalarType::Int16,
        5 => ScalarType::Int64,
        6 => ScalarType::Currency,
        7 => ScalarType::Single,
        8 => ScalarType::Double,
        9 => ScalarType::DateTime,
        10 => ScalarType::Guid,
        TEXT_FIELD_TAG => return Ok(CreateColumn::short_text(name, capacity)?),
        12 => return Ok(CreateColumn::short_binary(name, capacity)?),
        13 => {
            let constraints = DecimalConstraints::new(precision, scale)
                .map_err(|_| FirstTableIntentError::InvalidRecord)?;
            return Ok(CreateColumn::decimal(name, constraints));
        }
        _ => return Err(FirstTableIntentError::InvalidRecord),
    };
    Ok(CreateColumn::new(name, scalar))
}

pub(super) fn decode(
    raw: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeFirstTableIntent, FirstTableIntentError> {
    check(budget, cancelled)?;
    if raw.len() < HEADER_BYTES || raw.len() > MAX_RECORD_BYTES || !raw.starts_with(PREFIX) {
        return Err(FirstTableIntentError::InvalidRecord);
    }
    budget
        .charge(Resource::Steps, raw.len() as u64)
        .map_err(InspectError::from)?;
    let (defaults_version, policies_version) = match &raw[..MAGIC.len()] {
        version if version == MAGIC => (false, false),
        version if version == DEFAULT_MAGIC => (true, false),
        version if version == POLICY_MAGIC => (true, true),
        _ => return Err(FirstTableIntentError::UnsupportedVersion),
    };
    let expected_profile = if policies_version {
        POLICY_CONSTRUCTION_PROFILE
    } else if defaults_version {
        DEFAULT_CONSTRUCTION_PROFILE
    } else {
        CONSTRUCTION_PROFILE
    };
    if word(raw, PROFILE_AT) != expected_profile {
        return Err(FirstTableIntentError::UnsupportedProfile);
    }
    if !defaults_version && raw.len() > V1_MAX_RECORD_BYTES {
        return Err(FirstTableIntentError::InvalidRecord);
    }
    let before_length = u64::from_le_bytes(raw[BEFORE_AT..BINDING_AT].try_into().unwrap());
    let total = u32::from_le_bytes(raw[TOTAL_AT..PROFILE_AT].try_into().unwrap()) as usize;
    let count = usize::from(word(raw, COLUMNS_AT));
    if total != raw.len()
        || word(raw, FLAGS_AT) != 0
        || word(raw, RESERVED_AT) != 0
        || before_length != CANONICAL_BEFORE_BYTES
        || !(1..=MAX_COLUMNS).contains(&count)
    {
        return Err(FirstTableIntentError::InvalidRecord);
    }
    let before_binding = raw[BINDING_AT..TABLE_LENGTH_AT].try_into().unwrap();
    budget
        .charge(Resource::Steps, BINDING_BYTES as u64)
        .map_err(InspectError::from)?;
    if before_binding != CANONICAL_BEFORE_BINDING {
        return Err(FirstTableIntentError::BeforeMismatch);
    }
    let mut cursor = Cursor {
        raw,
        at: HEADER_BYTES,
    };
    let table = cursor.name(usize::from(word(raw, TABLE_LENGTH_AT)))?;
    let primary = cursor.name(usize::from(word(raw, PRIMARY_LENGTH_AT)))?;
    let mut columns = [CreateColumn::new("", ScalarType::Int32); MAX_COLUMNS];
    let mut defaults: [Option<carrier::Borrowed<'_>>; MAX_COLUMNS] = std::array::from_fn(|_| None);
    let mut found_policy = false;
    for (position, destination) in columns[..count].iter_mut().enumerate() {
        check(budget, cancelled)?;
        let header = cursor.take(FIELD_HEADER_BYTES)?;
        let name = cursor.name(usize::from(word(header, 0)))?;
        if (header[2] == 0) != (position == 0) {
            return Err(FirstTableIntentError::InvalidRecord);
        }
        let flags = header[3];
        let allowed = if policies_version {
            POLICY_FLAGS
        } else {
            u8::from(defaults_version)
        };
        if flags & !allowed != 0
            || position == 0 && flags != 0
            || flags & REQUIRED_VALUE != 0 && flags & REQUIRED_PRESENT == 0
            || flags & ALLOW_EMPTY_VALUE != 0 && flags & ALLOW_EMPTY_PRESENT == 0
            || flags & ALLOW_EMPTY_PRESENT != 0 && header[2] != TEXT_FIELD_TAG
        {
            return Err(FirstTableIntentError::InvalidRecord);
        }
        found_policy |= flags & (REQUIRED_PRESENT | ALLOW_EMPTY_PRESENT) != 0;
        if flags & DEFAULT_PRESENT != 0 {
            let default_header = cursor.take(DEFAULT_HEADER_BYTES)?;
            if default_header[1..4] != [0, 0, 0] {
                return Err(FirstTableIntentError::InvalidRecord);
            }
            let length = u32::from_le_bytes(default_header[4..8].try_into().unwrap()) as usize;
            if length > MAX_DEFAULT_BYTES {
                return Err(FirstTableIntentError::InvalidRecord);
            }
            if default_header[0] == 7 {
                return Err(FirstTableIntentError::InvalidRecord);
            }
            let payload = cursor.take(length)?;
            defaults[position] = Some(carrier::row_parse(
                default_header[0],
                payload,
                budget,
                cancelled,
            )?);
        }
        let mut declaration = column(name, header)?;
        if flags & REQUIRED_PRESENT != 0 {
            declaration = declaration.with_required(flags & REQUIRED_VALUE != 0);
        }
        if flags & ALLOW_EMPTY_PRESENT != 0 {
            declaration = declaration.with_allow_zero_length(flags & ALLOW_EMPTY_VALUE != 0)?;
        }
        *destination = if position == 0 {
            declaration
                .with_primary_key(primary)?
                .with_incrementing_autonumber()?
        } else {
            declaration
        };
    }
    if cursor.at != raw.len() {
        return Err(FirstTableIntentError::InvalidRecord);
    }
    if policies_version && !found_policy
        || defaults_version && !policies_version && defaults.iter().all(Option::is_none)
    {
        return Err(FirstTableIntentError::InvalidRecord);
    }
    let mut owned_defaults: [Option<Value>; MAX_COLUMNS] = std::array::from_fn(|_| None);
    for (destination, original) in owned_defaults.iter_mut().zip(defaults) {
        if let Some(original) = original {
            *destination = Some(carrier::owned(original, budget, cancelled)?);
        }
    }
    for (column, value) in columns[..count].iter_mut().zip(&owned_defaults) {
        if let Some(value) = value {
            *column = column.with_default_literal(value)?;
        }
    }
    // Full framing is checked before bounded default ownership. Capture then
    // validates all native declarations before retaining names and declarations.
    let definition = FirstTableDefinition::capture(table, &columns[..count], budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(NativeFirstTableIntent {
        definition,
        before_length,
        before_binding,
    })
}
