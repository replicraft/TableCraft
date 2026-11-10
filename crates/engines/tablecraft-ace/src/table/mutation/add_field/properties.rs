//! Append one column map while retaining the original dictionary and groups.
use super::CurrentFieldAdditionError as Error;
use crate::{InspectError, TableProperties, header::check, properties::framing};
use tablecraft_core::limits::{Resource, ResourceBudget};

const SIGNATURE: &[u8; 4] = b"MR2\0";
const SIGNATURE_BYTES: usize = SIGNATURE.len();
const NAME_LIST: u16 = 128;
const BLOCK_HEADER_BYTES: usize = size_of::<u32>() + size_of::<u16>();
const GROUP_PREFIX_BYTES: usize = BLOCK_HEADER_BYTES + size_of::<u32>() + size_of::<u16>();
const PROPERTY_HEADER_BYTES: usize = 3 * size_of::<u16>() + 2 * size_of::<u8>();
const REQUIRED_BYTES: usize = PROPERTY_HEADER_BYTES + size_of::<u8>();
const DEFAULT_BYTES: usize = PROPERTY_HEADER_BYTES + size_of::<u16>();
const COLUMN_GROUP: u16 = 1;
const DDL_FLAG: u8 = 1;
const BOOLEAN: u8 = 1;
const LONG_TEXT: u8 = 12;
const KEYS: [&str; 2] = ["DefaultValue", "Required"];
const COPY_BYTES: usize = 256;

struct Plan {
    dictionary_end: usize,
    dictionary_length: usize,
    missing: [bool; KEYS.len()],
    indices: [u16; KEYS.len()],
    name_bytes: u16,
    group_bytes: u32,
    total_bytes: usize,
}

fn key_matches(raw: &[u8], key: &str) -> bool {
    raw.len() == key.len() * size_of::<u16>()
        && raw
            .as_chunks::<2>()
            .0
            .iter()
            .zip(key.bytes())
            .all(|(unit, byte)| {
                let unit = u16::from_le_bytes(*unit);
                unit <= 127 && (unit as u8).eq_ignore_ascii_case(&byte)
            })
}

fn plan(
    properties: &TableProperties,
    name_units: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Plan, Error> {
    check(budget, cancelled)?;
    let raw = properties.raw_bytes();
    let (names, dictionary_length, dictionary_end, original_bytes) = if raw.is_empty() {
        if !properties.groups().is_empty() {
            return Err(Error::Source);
        }
        (
            Vec::new(),
            BLOCK_HEADER_BYTES,
            0,
            SIGNATURE_BYTES + BLOCK_HEADER_BYTES,
        )
    } else {
        let names = framing::ordered_names(properties, budget, cancelled)?;
        let length = u32::from_le_bytes(
            raw.get(SIGNATURE_BYTES..SIGNATURE_BYTES + size_of::<u32>())
                .ok_or(Error::Source)?
                .try_into()
                .map_err(|_| Error::Source)?,
        ) as usize;
        let end = SIGNATURE_BYTES.checked_add(length).ok_or(Error::Capacity)?;
        if end > raw.len() {
            return Err(Error::Source);
        }
        (names, length, end, raw.len())
    };
    let mut indices = [None; KEYS.len()];
    for (index, name) in names.iter().enumerate() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, (name.len() * KEYS.len()) as u64)?;
        for (key, target) in KEYS.iter().enumerate() {
            if key_matches(name, target)
                && indices[key]
                    .replace(u16::try_from(index).map_err(|_| Error::Capacity)?)
                    .is_some()
            {
                return Err(Error::Source);
            }
        }
    }
    let missing = indices.map(|index| index.is_none());
    let mut next = names.len();
    let mut growth = 0_usize;
    let mut resolved = [0; KEYS.len()];
    for (key, name) in KEYS.iter().enumerate() {
        resolved[key] = match indices[key] {
            Some(index) => index,
            None => {
                let index = u16::try_from(next).map_err(|_| Error::Capacity)?;
                next = next.checked_add(1).ok_or(Error::Capacity)?;
                growth = growth
                    .checked_add(size_of::<u16>() + name.len() * size_of::<u16>())
                    .ok_or(Error::Capacity)?;
                index
            }
        };
    }
    let name_bytes = name_units
        .checked_mul(size_of::<u16>())
        .ok_or(Error::Capacity)?;
    let group_bytes = GROUP_PREFIX_BYTES
        .checked_add(name_bytes)
        .and_then(|length| length.checked_add(REQUIRED_BYTES + DEFAULT_BYTES))
        .ok_or(Error::Capacity)?;
    let total_bytes = original_bytes
        .checked_add(growth)
        .and_then(|length| length.checked_add(group_bytes))
        .ok_or(Error::Capacity)?;
    let dictionary_length = dictionary_length
        .checked_add(growth)
        .ok_or(Error::Capacity)?;
    u32::try_from(dictionary_length).map_err(|_| Error::Capacity)?;
    Ok(Plan {
        dictionary_end,
        dictionary_length,
        missing,
        indices: resolved,
        name_bytes: u16::try_from(name_bytes).map_err(|_| Error::Capacity)?,
        group_bytes: u32::try_from(group_bytes).map_err(|_| Error::Capacity)?,
        total_bytes,
    })
}

pub(super) fn body_bytes(
    properties: &TableProperties,
    name_units: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, Error> {
    Ok(plan(properties, name_units, budget, cancelled)?.total_bytes)
}

fn copy(
    output: &mut Vec<u8>,
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    budget.charge(Resource::Steps, bytes.len() as u64)?;
    for chunk in bytes.chunks(COPY_BYTES) {
        check(budget, cancelled)?;
        output.extend_from_slice(chunk);
    }
    Ok(())
}

fn append_name(
    output: &mut Vec<u8>,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let bytes = u16::try_from(name.len() * size_of::<u16>()).map_err(|_| Error::Capacity)?;
    copy(output, &bytes.to_le_bytes(), budget, cancelled)?;
    for byte in name.bytes() {
        copy(output, &u16::from(byte).to_le_bytes(), budget, cancelled)?;
    }
    Ok(())
}

pub(super) fn append_number(
    properties: &TableProperties,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    check(budget, cancelled)?;
    if name.is_empty() || !name.is_ascii() {
        return Err(Error::InvalidName);
    }
    let plan = plan(properties, name.len(), budget, cancelled)?;
    budget.charge_allocation(plan.total_bytes as u64, size_of::<u8>() as u64)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(plan.total_bytes)
        .map_err(|_| InspectError::Allocation)?;
    let raw = properties.raw_bytes();
    if raw.is_empty() {
        copy(&mut output, SIGNATURE, budget, cancelled)?;
        copy(
            &mut output,
            &(plan.dictionary_length as u32).to_le_bytes(),
            budget,
            cancelled,
        )?;
        copy(&mut output, &NAME_LIST.to_le_bytes(), budget, cancelled)?;
    } else {
        copy(&mut output, &raw[..plan.dictionary_end], budget, cancelled)?;
        output[SIGNATURE_BYTES..SIGNATURE_BYTES + size_of::<u32>()]
            .copy_from_slice(&(plan.dictionary_length as u32).to_le_bytes());
    }
    for (key, missing) in KEYS.iter().zip(plan.missing) {
        if missing {
            append_name(&mut output, key, budget, cancelled)?;
        }
    }
    copy(&mut output, &raw[plan.dictionary_end..], budget, cancelled)?;

    copy(
        &mut output,
        &plan.group_bytes.to_le_bytes(),
        budget,
        cancelled,
    )?;
    copy(&mut output, &COLUMN_GROUP.to_le_bytes(), budget, cancelled)?;
    let name_frame = u32::from(plan.name_bytes) + size_of::<u32>() as u32 + size_of::<u16>() as u32;
    copy(&mut output, &name_frame.to_le_bytes(), budget, cancelled)?;
    append_name(&mut output, name, budget, cancelled)?;
    let order = if plan.indices[0] < plan.indices[1] {
        [0, 1]
    } else {
        [1, 0]
    };
    for key in order {
        let (kind, value) = if key == 0 {
            (LONG_TEXT, &[b'0', 0][..])
        } else {
            (BOOLEAN, &[0][..])
        };
        copy(
            &mut output,
            &((PROPERTY_HEADER_BYTES + value.len()) as u16).to_le_bytes(),
            budget,
            cancelled,
        )?;
        copy(&mut output, &[DDL_FLAG, kind], budget, cancelled)?;
        copy(
            &mut output,
            &plan.indices[key].to_le_bytes(),
            budget,
            cancelled,
        )?;
        copy(
            &mut output,
            &(value.len() as u16).to_le_bytes(),
            budget,
            cancelled,
        )?;
        copy(&mut output, value, budget, cancelled)?;
    }
    if output.len() != plan.total_bytes {
        return Err(Error::Preservation);
    }
    check(budget, cancelled)?;
    Ok(output)
}
