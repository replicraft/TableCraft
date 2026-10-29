use super::{TableExtraCoverage, TableExtraGroup, TableExtraProperty};
use crate::{InspectError, header::check, limits::MAX_LONG_VALUE_BYTES};
use tablecraft_core::limits::{Resource, ResourceBudget};

const SIGNATURE: &[u8] = b"MR2\0";
const NAME_LIST: u16 = 0x80;
const OBSERVED_GROUPS: [u16; 2] = [3, 4];
const OBSERVED_FLAGS: u8 = 1;
const OBSERVED_TEXT_TYPE: u8 = 12;
const BLOCK_HEADER_BYTES: usize = size_of::<u32>() + size_of::<u16>();
const VALUE_HEADER_BYTES: usize = size_of::<u32>() * 2 + size_of::<u16>() + size_of::<u8>() * 2;
const NAME_METADATA_BYTES: usize = size_of::<u32>() * 2 + size_of::<u16>();
const MAX_NAMES: usize = 256;
const MAX_GROUPS: usize = 1024;
const MAX_ENTRIES: usize = 4096;

pub(super) struct Decoded {
    pub(super) coverage: TableExtraCoverage,
    pub(super) names: Vec<String>,
    pub(super) groups: Vec<TableExtraGroup>,
}

struct Cursor<'a> {
    raw: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(raw: &'a [u8]) -> Self {
        Self { raw, position: 0 }
    }

    fn remaining(&self) -> usize {
        self.raw.len() - self.position
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], InspectError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(InspectError::Malformed)?;
        let bytes = self
            .raw
            .get(self.position..end)
            .ok_or(InspectError::Truncated)?;
        self.position = end;
        Ok(bytes)
    }

    fn short(&mut self) -> Result<u16, InspectError> {
        Ok(u16::from_le_bytes(
            self.take(size_of::<u16>())?
                .try_into()
                .map_err(|_| InspectError::Malformed)?,
        ))
    }

    fn long(&mut self) -> Result<usize, InspectError> {
        usize::try_from(u32::from_le_bytes(
            self.take(size_of::<u32>())?
                .try_into()
                .map_err(|_| InspectError::Malformed)?,
        ))
        .map_err(|_| InspectError::StructureLimit)
    }
}

pub(super) fn decode(
    raw: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Decoded, InspectError> {
    check(budget, cancelled)?;
    if raw.len() > MAX_LONG_VALUE_BYTES {
        return Err(InspectError::StructureLimit);
    }
    if raw.is_empty() || !raw.starts_with(SIGNATURE) {
        return Ok(Decoded {
            coverage: if raw.is_empty() {
                TableExtraCoverage::Empty
            } else {
                TableExtraCoverage::Opaque
            },
            names: Vec::new(),
            groups: Vec::new(),
        });
    }

    let mut cursor = Cursor::new(&raw[SIGNATURE.len()..]);
    let mut names = None;
    let mut groups: Vec<TableExtraGroup> = Vec::new();
    let mut entries = 0;
    let mut coverage = TableExtraCoverage::WideMr2;
    while cursor.remaining() != 0 {
        check(budget, cancelled)?;
        let length = cursor.long()?;
        let kind = cursor.short()?;
        let body_length = length
            .checked_sub(BLOCK_HEADER_BYTES)
            .ok_or(InspectError::Malformed)?;
        let body = cursor.take(body_length)?;
        if kind == NAME_LIST {
            if names.is_some() || !groups.is_empty() {
                return Err(InspectError::Malformed);
            }
            names = Some(read_names(body, budget, cancelled)?);
            continue;
        }

        let names = names.as_ref().ok_or(InspectError::Malformed)?;
        if groups.len() == MAX_GROUPS {
            return Err(InspectError::StructureLimit);
        }
        let properties = if OBSERVED_GROUPS.contains(&kind) {
            read_properties(body, names, &mut entries, &mut coverage, budget, cancelled)?
        } else {
            coverage = TableExtraCoverage::Opaque;
            None
        };
        if properties.is_some() {
            for group in &groups {
                check(budget, cancelled)?;
                if group.kind == kind && group.properties.is_some() {
                    return Err(InspectError::Malformed);
                }
            }
        }
        let raw = copy(body, budget, cancelled)?;
        reserve(&mut groups, budget)?;
        groups.push(TableExtraGroup {
            kind,
            raw,
            properties,
        });
    }
    let names = names.ok_or(InspectError::Malformed)?;
    check(budget, cancelled)?;
    Ok(Decoded {
        coverage,
        names,
        groups,
    })
}

fn read_names(
    raw: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<String>, InspectError> {
    let mut cursor = Cursor::new(raw);
    let mut names: Vec<String> = Vec::new();
    while cursor.remaining() != 0 {
        check(budget, cancelled)?;
        if names.len() == MAX_NAMES {
            return Err(InspectError::StructureLimit);
        }
        let length = usize::from(cursor.short()?);
        let name = utf16(cursor.take(length)?, budget, cancelled)?;
        if name.is_empty() {
            return Err(InspectError::Malformed);
        }
        for other in &names {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, name.len().max(other.len()) as u64)?;
            if name.eq_ignore_ascii_case(other) {
                return Err(InspectError::Malformed);
            }
        }
        reserve(&mut names, budget)?;
        names.push(name);
    }
    Ok(names)
}

fn read_properties(
    raw: &[u8],
    names: &[String],
    entries: &mut usize,
    coverage: &mut TableExtraCoverage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Vec<TableExtraProperty>>, InspectError> {
    let mut cursor = Cursor::new(raw);
    let metadata_length = cursor.long()?;
    let metadata_body = metadata_length
        .checked_sub(size_of::<u32>())
        .ok_or(InspectError::Malformed)?;
    let metadata = cursor.take(metadata_body)?;
    budget.charge(Resource::Steps, metadata.len() as u64)?;
    check(budget, cancelled)?;
    if metadata_length != NAME_METADATA_BYTES || metadata.iter().any(|byte| *byte != 0) {
        *coverage = TableExtraCoverage::Opaque;
        return Ok(None);
    }

    let mut properties: Vec<TableExtraProperty> = Vec::new();
    while cursor.remaining() != 0 {
        check(budget, cancelled)?;
        if *entries == MAX_ENTRIES {
            return Err(InspectError::StructureLimit);
        }
        *entries += 1;
        let length = cursor.long()?;
        if length < VALUE_HEADER_BYTES {
            return Err(InspectError::Malformed);
        }
        let mut entry = Cursor::new(cursor.take(length - size_of::<u32>())?);
        let flags = entry.take(size_of::<u8>())?[0];
        let native_type = entry.take(size_of::<u8>())?[0];
        let name_index = usize::from(entry.short()?);
        let size = entry.long()?;
        let name = names.get(name_index).ok_or(InspectError::Malformed)?;
        for other in &properties {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, name.len().max(other.name.len()) as u64)?;
            if name.eq_ignore_ascii_case(&other.name) {
                return Err(InspectError::Malformed);
            }
        }
        let value = entry.take(size)?;
        if entry.remaining() != 0 {
            return Err(InspectError::Malformed);
        }
        let text = if flags == OBSERVED_FLAGS && native_type == OBSERVED_TEXT_TYPE {
            Some(utf16(value, budget, cancelled)?)
        } else {
            *coverage = TableExtraCoverage::Opaque;
            None
        };
        let name = String::from_utf8(copy(name.as_bytes(), budget, cancelled)?)
            .map_err(|_| InspectError::Malformed)?;
        let raw = copy(value, budget, cancelled)?;
        reserve(&mut properties, budget)?;
        properties.push(TableExtraProperty {
            name,
            native_type,
            flags,
            raw,
            text,
        });
    }
    Ok(Some(properties))
}

fn utf16(
    raw: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, InspectError> {
    if !raw.len().is_multiple_of(size_of::<u16>()) {
        return Err(InspectError::Malformed);
    }
    let units = || {
        raw.as_chunks::<{ size_of::<u16>() }>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
    };
    let mut length = 0usize;
    for scalar in char::decode_utf16(units()) {
        check(budget, cancelled)?;
        let scalar = scalar.map_err(|_| InspectError::Malformed)?;
        length = length
            .checked_add(scalar.len_utf8())
            .ok_or(InspectError::StructureLimit)?;
        if length > MAX_LONG_VALUE_BYTES {
            return Err(InspectError::StructureLimit);
        }
    }
    budget.charge_allocation(length as u64, size_of::<u8>() as u64)?;
    let mut text = String::new();
    text.try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    for scalar in char::decode_utf16(units()) {
        check(budget, cancelled)?;
        text.push(scalar.map_err(|_| InspectError::Malformed)?);
    }
    Ok(text)
}

fn copy(
    raw: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, InspectError> {
    budget.charge_allocation(raw.len() as u64, size_of::<u8>() as u64)?;
    budget.charge(Resource::Steps, raw.len() as u64)?;
    check(budget, cancelled)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(raw.len())
        .map_err(|_| InspectError::Allocation)?;
    bytes.extend_from_slice(raw);
    Ok(bytes)
}

fn reserve<T>(values: &mut Vec<T>, budget: &mut ResourceBudget) -> Result<(), InspectError> {
    budget.charge_allocation(1, size_of::<T>() as u64)?;
    values.try_reserve(1).map_err(|_| InspectError::Allocation)
}
