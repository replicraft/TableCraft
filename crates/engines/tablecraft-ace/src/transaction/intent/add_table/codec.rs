//! Distinct source-bound actions with closed legacy or described declaration templates.
use super::super::first_table::{
    LEGACY_MAX_DECLARATION_TEMPLATE_BYTES, MAX_DECLARATION_TEMPLATE_BYTES,
    MAX_NATIVE_DECLARATION_TEMPLATE_BYTES, decode_declaration_template,
    decode_native_declaration_template, encode_declaration_template,
};
use super::{AddTableIntentError, BeforeBinding, NativeAddTableIntent};
use crate::{FirstTableDefinition, InspectError, format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: &[u8; 8] = b"TCAT0001";
const REPEATED_MAGIC: &[u8; 8] = b"TCAT0002";
const DESCRIBED_MAGIC: &[u8; 8] = b"TCAT0003";
const REPEATED_DESCRIBED_MAGIC: &[u8; 8] = b"TCAT0004";
const MIXED_MAGIC: &[u8; 8] = b"TCAT0005";
const RETAINED_MAGIC: &[u8; 8] = b"TCAT0006";
const NATIVE_MAGIC: &[u8; 8] = b"TCAT0007";
const PREFIX: &[u8; 4] = b"TCAT";
const PROFILE: u16 = 1;
const REPEATED_PROFILE: u16 = 2;
const DESCRIBED_PROFILE: u16 = 3;
const REPEATED_DESCRIBED_PROFILE: u16 = 4;
const MIXED_PROFILE: u16 = 5;
const RETAINED_PROFILE: u16 = 6;
const NATIVE_PROFILE: u16 = 7;
const NATIVE_RETAINED_FLAG: u16 = 1;
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_AT + size_of::<u64>();
const BINDING_BYTES: usize = size_of::<BeforeBinding>();
const NESTED_LENGTH_AT: usize = BINDING_AT + BINDING_BYTES;
const RESERVED_AT: usize = NESTED_LENGTH_AT + size_of::<u32>();
const HEADER_BYTES: usize = RESERVED_AT + size_of::<u32>();
const NESTED_MAGIC: &[u8; 8] = b"TCNT0001";
const DESCRIBED_NESTED_MAGIC: &[u8; 8] = b"TCNT0005";
const NESTED_TOTAL_AT: usize = NESTED_MAGIC.len();
const NESTED_PROFILE_AT: usize = NESTED_TOTAL_AT + size_of::<u32>();
const NESTED_FLAGS_AT: usize = NESTED_PROFILE_AT + size_of::<u16>();
const NESTED_BEFORE_AT: usize = NESTED_FLAGS_AT + size_of::<u16>();
const NESTED_BINDING_AT: usize = NESTED_BEFORE_AT + size_of::<u64>();
const NESTED_TABLE_LENGTH_AT: usize = NESTED_BINDING_AT + BINDING_BYTES;
const NESTED_PRIMARY_LENGTH_AT: usize = NESTED_TABLE_LENGTH_AT + size_of::<u16>();
const NESTED_COLUMNS_AT: usize = NESTED_PRIMARY_LENGTH_AT + size_of::<u16>();
const NESTED_RESERVED_AT: usize = NESTED_COLUMNS_AT + size_of::<u16>();
const NESTED_HEADER_BYTES: usize = NESTED_RESERVED_AT + size_of::<u16>();
const MANUAL_NESTED_MAGIC: &[u8; 8] = b"TCNT0006";
const NO_PRIMARY_POSITION: u16 = u16::MAX;
const COPY_BYTES: usize = 4096;

// Profile 1 admits only the initial ten-page AddTable source. This length is
// framing, not source authority: the fresh native operation must qualify roles.
const PROFILE_SOURCE_PAGES: u64 = 10;
const BEFORE_BYTES: u64 = PROFILE_SOURCE_PAGES * PAGE_BYTES as u64;
// These are protocol-2 admission bounds, deliberately independent of the native
// producer's source ledger. Existing TCAT1 stays a disjoint ten-page language.
const REPEATED_MIN_PAGES: u64 = 13;
const REPEATED_MAX_PAGES: u64 = 61;
const REPEATED_PAGE_STEP: u64 = 3;
const LEGACY_MAX_RECORD_BYTES: usize = HEADER_BYTES + LEGACY_MAX_DECLARATION_TEMPLATE_BYTES;
pub(super) const MAX_RECORD_BYTES: usize = HEADER_BYTES + MAX_DECLARATION_TEMPLATE_BYTES;
pub(super) const NATIVE_MAX_RECORD_BYTES: usize =
    HEADER_BYTES + MAX_NATIVE_DECLARATION_TEMPLATE_BYTES;

// Mixed source framing is independent of native ledger parsing. A keyless
// target needs two pages; every target with a primary key needs three.
const MIXED_MIN_PAGES: u64 = 9;
const MIXED_MAPPED_PAGES: u64 = 64;
const KEYLESS_TARGET_PAGES: u64 = 2;
const KEYED_TARGET_PAGES: u64 = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Profile {
    Legacy,
    Described,
    Repeated,
    RepeatedDescribed,
    Mixed,
    Retained,
    NativeMixed,
    NativeRetained,
}

impl Profile {
    fn wire(self) -> (&'static [u8; 8], u16) {
        match self {
            Self::Legacy => (MAGIC, PROFILE),
            Self::Repeated => (REPEATED_MAGIC, REPEATED_PROFILE),
            Self::Described => (DESCRIBED_MAGIC, DESCRIBED_PROFILE),
            Self::RepeatedDescribed => (REPEATED_DESCRIBED_MAGIC, REPEATED_DESCRIBED_PROFILE),
            Self::Mixed => (MIXED_MAGIC, MIXED_PROFILE),
            Self::Retained => (RETAINED_MAGIC, RETAINED_PROFILE),
            Self::NativeMixed | Self::NativeRetained => (NATIVE_MAGIC, NATIVE_PROFILE),
        }
    }

    pub(super) fn digest(
        self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<BeforeBinding, super::NativeIntentError> {
        match self {
            Self::Retained | Self::NativeMixed | Self::NativeRetained => {
                super::binding::digest_retained(before, budget, cancelled)
            }
            _ => super::binding::digest(before, budget, cancelled),
        }
    }

    fn described(self) -> bool {
        matches!(self, Self::Described | Self::RepeatedDescribed)
    }

    pub(super) fn is_native(self) -> bool {
        matches!(self, Self::NativeMixed | Self::NativeRetained)
    }

    fn maximum(self) -> usize {
        if self.is_native() {
            NATIVE_MAX_RECORD_BYTES
        } else if self.described() || matches!(self, Self::Mixed | Self::Retained) {
            MAX_RECORD_BYTES
        } else {
            LEGACY_MAX_RECORD_BYTES
        }
    }
}

pub(super) fn legacy_profile(length: u64) -> Option<Profile> {
    match version_for_length(length)?.1 {
        PROFILE => Some(Profile::Legacy),
        REPEATED_PROFILE => Some(Profile::Repeated),
        _ => None,
    }
}

pub(super) fn mixed_before_length(length: u64, keyless: bool) -> bool {
    let added_pages = if keyless {
        KEYLESS_TARGET_PAGES
    } else {
        KEYED_TARGET_PAGES
    };
    length.is_multiple_of(PAGE_BYTES as u64)
        && (MIXED_MIN_PAGES..=MIXED_MAPPED_PAGES - added_pages)
            .contains(&(length / PAGE_BYTES as u64))
}

pub(super) fn retained_before_length(length: u64) -> bool {
    super::binding::retained_length(length).is_ok()
}

fn repeated_length(length: u64) -> bool {
    length.is_multiple_of(PAGE_BYTES as u64)
        && (REPEATED_MIN_PAGES..=REPEATED_MAX_PAGES).contains(&(length / PAGE_BYTES as u64))
        && (length / PAGE_BYTES as u64 - REPEATED_MIN_PAGES).is_multiple_of(REPEATED_PAGE_STEP)
}

fn version_for_length(length: u64) -> Option<(&'static [u8; 8], u16)> {
    if length == BEFORE_BYTES {
        Some((MAGIC, PROFILE))
    } else if repeated_length(length) {
        Some((REPEATED_MAGIC, REPEATED_PROFILE))
    } else {
        None
    }
}

fn pin_template(raw: &[u8], outer: Profile) -> Result<(), AddTableIntentError> {
    let described = outer.described();
    let mixed = matches!(outer, Profile::Mixed | Profile::Retained);
    let maximum = if outer.is_native() {
        MAX_NATIVE_DECLARATION_TEMPLATE_BYTES
    } else if described || mixed {
        MAX_DECLARATION_TEMPLATE_BYTES
    } else {
        LEGACY_MAX_DECLARATION_TEMPLATE_BYTES
    };
    if !(NESTED_HEADER_BYTES..=maximum).contains(&raw.len()) {
        return Err(AddTableIntentError::InvalidRecord);
    }
    if outer.is_native() {
        if !raw.starts_with(b"TCNT0007") {
            return Err(AddTableIntentError::UnsupportedVersion);
        }
        let profile =
            u16::from_le_bytes(raw[NESTED_PROFILE_AT..NESTED_FLAGS_AT].try_into().unwrap());
        if profile != NATIVE_PROFILE {
            return Err(AddTableIntentError::UnsupportedProfile);
        }
        // Full native declaration validation follows before any native planning.
        return Ok(());
    }
    let expected = match (&raw[..NESTED_MAGIC.len()], described, mixed) {
        (b"TCNT0001", false, _) => 1_u16,
        (b"TCNT0002", false, _) => 2,
        (b"TCNT0003", false, _) => 3,
        (b"TCNT0004", false, _) => 4,
        (b"TCNT0005", true, false) => 5,
        (b"TCNT0005", false, true) => 5,
        (b"TCNT0006", false, true) => 6,
        _ => return Err(AddTableIntentError::UnsupportedVersion),
    };
    let profile = u16::from_le_bytes(raw[NESTED_PROFILE_AT..NESTED_FLAGS_AT].try_into().unwrap());
    if profile != expected {
        return Err(AddTableIntentError::UnsupportedProfile);
    }
    if raw[NESTED_FLAGS_AT..NESTED_BEFORE_AT] != [0; size_of::<u16>()]
        || (expected != 6 && raw[NESTED_RESERVED_AT..NESTED_HEADER_BYTES] != [0; size_of::<u16>()])
    {
        return Err(AddTableIntentError::InvalidRecord);
    }
    Ok(())
}

pub(super) fn encode(
    definition: &FirstTableDefinition,
    outer: Profile,
    before_length: u64,
    before_binding: BeforeBinding,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, AddTableIntentError> {
    check(budget, cancelled)?;
    if definition.is_native() != (outer.is_native()) {
        return Err(AddTableIntentError::UnsupportedProfile);
    }
    let nested = encode_declaration_template(definition, budget, cancelled)?;
    let outer = match (outer, nested.starts_with(DESCRIBED_NESTED_MAGIC)) {
        (Profile::Legacy, true) => Profile::Described,
        (Profile::Repeated, true) => Profile::RepeatedDescribed,
        _ => outer,
    };
    pin_template(&nested, outer)?;
    if !profile_accepts_length(outer, before_length, &nested) {
        return Err(AddTableIntentError::BeforeMismatch);
    }
    let (magic, profile) = outer.wire();
    let total = HEADER_BYTES
        .checked_add(nested.len())
        .filter(|length| *length <= outer.maximum())
        .ok_or(AddTableIntentError::InvalidRecord)?;
    budget
        .charge_allocation(total as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, HEADER_BYTES as u64)
        .map_err(InspectError::from)?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(total)
        .map_err(|_| InspectError::Allocation)?;
    raw.resize(HEADER_BYTES, 0);
    raw[..MAGIC.len()].copy_from_slice(magic);
    raw[TOTAL_AT..PROFILE_AT].copy_from_slice(&(total as u32).to_le_bytes());
    raw[PROFILE_AT..FLAGS_AT].copy_from_slice(&profile.to_le_bytes());
    if outer == Profile::NativeRetained {
        raw[FLAGS_AT..BEFORE_AT].copy_from_slice(&NATIVE_RETAINED_FLAG.to_le_bytes());
    }
    raw[BEFORE_AT..BINDING_AT].copy_from_slice(&before_length.to_le_bytes());
    raw[BINDING_AT..NESTED_LENGTH_AT].copy_from_slice(&before_binding);
    raw[NESTED_LENGTH_AT..RESERVED_AT].copy_from_slice(&(nested.len() as u32).to_le_bytes());
    for chunk in nested.chunks(COPY_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        raw.extend_from_slice(chunk);
    }
    check(budget, cancelled)?;
    Ok(raw)
}

pub(super) fn decode(
    raw: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeAddTableIntent, AddTableIntentError> {
    decode_family(raw, DecodeFamily::Original, budget, cancelled)
}

pub(super) fn decode_retained(
    raw: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeAddTableIntent, AddTableIntentError> {
    decode_family(raw, DecodeFamily::Retained, budget, cancelled)
}

pub(super) fn decode_native(
    raw: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeAddTableIntent, AddTableIntentError> {
    decode_family(raw, DecodeFamily::Native, budget, cancelled)
}

#[derive(Clone, Copy)]
enum DecodeFamily {
    Original,
    Retained,
    Native,
}

fn decode_family(
    raw: &[u8],
    family: DecodeFamily,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeAddTableIntent, AddTableIntentError> {
    check(budget, cancelled)?;
    let retained = matches!(family, DecodeFamily::Retained);
    let native = matches!(family, DecodeFamily::Native);
    let described = raw.starts_with(DESCRIBED_MAGIC) || raw.starts_with(REPEATED_DESCRIBED_MAGIC);
    let mixed = raw.starts_with(MIXED_MAGIC);
    let maximum = if native {
        NATIVE_MAX_RECORD_BYTES
    } else if described || mixed || retained {
        MAX_RECORD_BYTES
    } else {
        LEGACY_MAX_RECORD_BYTES
    };
    if !(HEADER_BYTES..=maximum).contains(&raw.len()) {
        return Err(AddTableIntentError::InvalidRecord);
    }
    budget
        .charge(Resource::Steps, HEADER_BYTES as u64)
        .map_err(InspectError::from)?;
    let expected_profile = if native {
        if &raw[..MAGIC.len()] != NATIVE_MAGIC {
            return Err(AddTableIntentError::UnsupportedVersion);
        }
        match u16::from_le_bytes(raw[FLAGS_AT..BEFORE_AT].try_into().unwrap()) {
            0 => Profile::NativeMixed,
            NATIVE_RETAINED_FLAG => Profile::NativeRetained,
            _ => return Err(AddTableIntentError::InvalidRecord),
        }
    } else if retained {
        if &raw[..MAGIC.len()] != RETAINED_MAGIC {
            return Err(AddTableIntentError::UnsupportedVersion);
        }
        Profile::Retained
    } else {
        match &raw[..MAGIC.len()] {
            magic if magic == MAGIC => Profile::Legacy,
            magic if magic == REPEATED_MAGIC => Profile::Repeated,
            magic if magic == DESCRIBED_MAGIC => Profile::Described,
            magic if magic == REPEATED_DESCRIBED_MAGIC => Profile::RepeatedDescribed,
            magic if magic == MIXED_MAGIC => Profile::Mixed,
            _ => {
                return Err(if raw.starts_with(PREFIX) {
                    AddTableIntentError::UnsupportedVersion
                } else {
                    AddTableIntentError::InvalidRecord
                });
            }
        }
    };
    if u16::from_le_bytes(raw[PROFILE_AT..FLAGS_AT].try_into().unwrap())
        != expected_profile.wire().1
    {
        return Err(AddTableIntentError::UnsupportedProfile);
    }
    let total = u32::from_le_bytes(raw[TOTAL_AT..PROFILE_AT].try_into().unwrap()) as usize;
    let before_length = u64::from_le_bytes(raw[BEFORE_AT..BINDING_AT].try_into().unwrap());
    let nested_length =
        u32::from_le_bytes(raw[NESTED_LENGTH_AT..RESERVED_AT].try_into().unwrap()) as usize;
    if total != raw.len()
        || !native && raw[FLAGS_AT..BEFORE_AT] != [0; size_of::<u16>()]
        || raw[RESERVED_AT..HEADER_BYTES] != [0; size_of::<u32>()]
        || (!matches!(
            expected_profile,
            Profile::Mixed | Profile::Retained | Profile::NativeMixed | Profile::NativeRetained
        ) && !profile_accepts_length(expected_profile, before_length, &[]))
        || nested_length != raw.len() - HEADER_BYTES
    {
        return Err(AddTableIntentError::InvalidRecord);
    }
    let nested = &raw[HEADER_BYTES..];
    budget
        .charge(Resource::Steps, NESTED_HEADER_BYTES as u64)
        .map_err(InspectError::from)?;
    pin_template(nested, expected_profile)?;
    if !profile_accepts_length(expected_profile, before_length, nested) {
        return Err(AddTableIntentError::InvalidRecord);
    }
    let definition = if native {
        decode_native_declaration_template(nested, budget, cancelled)?
    } else {
        decode_declaration_template(nested, budget, cancelled)?
    };
    check(budget, cancelled)?;
    Ok(NativeAddTableIntent {
        definition,
        profile: expected_profile,
        before_length,
        before_binding: raw[BINDING_AT..NESTED_LENGTH_AT].try_into().unwrap(),
    })
}

fn profile_accepts_length(profile: Profile, length: u64, nested: &[u8]) -> bool {
    match profile {
        Profile::Legacy | Profile::Described => length == BEFORE_BYTES,
        Profile::Repeated | Profile::RepeatedDescribed => repeated_length(length),
        Profile::Retained | Profile::NativeMixed | Profile::NativeRetained => {
            retained_before_length(length)
        }
        Profile::Mixed => {
            let keyless = nested.starts_with(MANUAL_NESTED_MAGIC)
                && u16::from_le_bytes(
                    nested[NESTED_PRIMARY_LENGTH_AT..NESTED_COLUMNS_AT]
                        .try_into()
                        .unwrap(),
                ) == 0
                && u16::from_le_bytes(
                    nested[NESTED_RESERVED_AT..NESTED_HEADER_BYTES]
                        .try_into()
                        .unwrap(),
                ) == NO_PRIMARY_POSITION;
            mixed_before_length(length, keyless)
        }
    }
}
