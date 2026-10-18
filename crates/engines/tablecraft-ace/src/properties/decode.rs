use super::{NativeProperty, PropertyGroup, PropertyGroupKind};
use crate::{
    InspectError,
    format::{data_type, text},
    header::check,
    limits::{MAX_COLUMNS, MAX_INDEXES, MAX_LONG_VALUE_BYTES},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::SchemaName;

const SIGNATURE: &[u8] = b"MR2\0";
const NAME_LIST: u16 = 0x80;
const TABLE_VALUES: u16 = 0;
const COLUMN_VALUES: u16 = 1;
const INDEX_VALUES: u16 = 2;
const BLOCK_HEADER_BYTES: usize = size_of::<u32>() + size_of::<u16>();
const VALUE_HEADER_BYTES: usize = size_of::<u16>() * 3 + size_of::<u8>() * 2;
const MAX_PROPERTY_NAMES: usize = 256;
const MAX_PROPERTY_GROUPS: usize = MAX_COLUMNS + MAX_INDEXES + 1;
const MAX_PROPERTY_ENTRIES: usize = 4096;

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], InspectError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(InspectError::Malformed)?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(InspectError::Truncated)?;
        self.position = end;
        Ok(bytes)
    }
    fn short(&mut self) -> Result<usize, InspectError> {
        Ok(u16::from_le_bytes(
            self.take(size_of::<u16>())?
                .try_into()
                .map_err(|_| InspectError::Malformed)?,
        ) as usize)
    }
    fn long(&mut self) -> Result<usize, InspectError> {
        Ok(u32::from_le_bytes(
            self.take(size_of::<u32>())?
                .try_into()
                .map_err(|_| InspectError::Malformed)?,
        ) as usize)
    }
    fn name(&mut self, budget: &mut ResourceBudget) -> Result<String, InspectError> {
        let length = self.short()?;
        text::decode(self.take(length)?, false, budget)
    }
}

pub(super) fn decode(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<PropertyGroup>, InspectError> {
    check(budget, cancelled)?;
    if bytes.len() > MAX_LONG_VALUE_BYTES {
        return Err(InspectError::StructureLimit);
    }
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let mut cursor = Cursor::new(bytes);
    if cursor.take(SIGNATURE.len())? != SIGNATURE {
        return Err(InspectError::Unsupported);
    }
    let mut names = None;
    let mut groups = Vec::new();
    let mut entries = 0;

    while cursor.remaining() != 0 {
        check(budget, cancelled)?;
        let length = cursor.long()?;
        let kind = cursor.short()? as u16;
        let body = length
            .checked_sub(BLOCK_HEADER_BYTES)
            .ok_or(InspectError::Malformed)?;
        let mut block = Cursor::new(cursor.take(body)?);
        if kind == NAME_LIST {
            if names.is_some() || !groups.is_empty() {
                return Err(InspectError::Malformed);
            }
            let mut parsed = Vec::new();
            while block.remaining() != 0 {
                check(budget, cancelled)?;
                if parsed.len() == MAX_PROPERTY_NAMES {
                    return Err(InspectError::StructureLimit);
                }
                let name = SchemaName::new(block.name(budget)?)?;
                if parsed
                    .iter()
                    .any(|other: &SchemaName| other.as_str().eq_ignore_ascii_case(name.as_str()))
                {
                    return Err(InspectError::Malformed);
                }
                reserve(&mut parsed, budget)?;
                parsed.push(name);
            }
            names = Some(parsed);
            continue;
        }

        let names = names.as_ref().ok_or(InspectError::Malformed)?;
        if groups.len() == MAX_PROPERTY_GROUPS {
            return Err(InspectError::StructureLimit);
        }
        let name_block_length = block.long()?;
        let name_block_bytes = name_block_length
            .checked_sub(size_of::<u32>())
            .ok_or(InspectError::Malformed)?;
        let mut name_block = Cursor::new(block.take(name_block_bytes)?);
        let name = name_block.name(budget)?;
        if name_block.remaining() != 0 {
            return Err(InspectError::Unsupported);
        }
        let kind = match kind {
            TABLE_VALUES => PropertyGroupKind::Table,
            COLUMN_VALUES => PropertyGroupKind::Column,
            INDEX_VALUES => PropertyGroupKind::Index,
            other => PropertyGroupKind::Unknown(other),
        };
        if kind == PropertyGroupKind::Table {
            if !name.is_empty() {
                return Err(InspectError::Malformed);
            }
        } else {
            SchemaName::new(copy(name.as_bytes(), budget).and_then(|bytes| {
                String::from_utf8(bytes).map_err(|_| InspectError::Malformed)
            })?)?;
        }
        if groups.iter().any(|group: &PropertyGroup| {
            group.kind == kind && group.name.eq_ignore_ascii_case(&name)
        }) {
            return Err(InspectError::Malformed);
        }
        let mut properties = Vec::new();
        while block.remaining() != 0 {
            check(budget, cancelled)?;
            if entries == MAX_PROPERTY_ENTRIES {
                return Err(InspectError::StructureLimit);
            }
            entries += 1;
            let length = block.short()?;
            let body = length
                .checked_sub(size_of::<u16>())
                .ok_or(InspectError::Malformed)?;
            if length < VALUE_HEADER_BYTES {
                return Err(InspectError::Malformed);
            }
            let mut value = Cursor::new(block.take(body)?);
            let flags = value.take(size_of::<u8>())?[0];
            let native_type = value.take(size_of::<u8>())?[0];
            let name_index = value.short()?;
            let data_size = value.short()?;
            let key = names
                .get(name_index)
                .ok_or(InspectError::Malformed)?
                .as_str();
            if properties
                .iter()
                .any(|property: &NativeProperty| property.name.eq_ignore_ascii_case(key))
            {
                return Err(InspectError::Malformed);
            }
            let raw = copy(value.take(data_size)?, budget)?;
            if native_type == data_type::BOOLEAN && raw.len() != size_of::<u8>() {
                return Err(InspectError::Malformed);
            }
            let decoded_text = match native_type {
                data_type::SHORT_TEXT | data_type::LONG_TEXT => {
                    Some(text::decode_long(&raw, budget)?)
                }
                _ => None,
            };
            let name = String::from_utf8(copy(key.as_bytes(), budget)?)
                .map_err(|_| InspectError::Malformed)?;
            reserve(&mut properties, budget)?;
            properties.push(NativeProperty {
                name,
                kind: native_type,
                flags,
                raw,
                text: decoded_text,
            });
        }
        reserve(&mut groups, budget)?;
        groups.push(PropertyGroup {
            kind,
            name,
            properties,
        });
    }
    check(budget, cancelled)?;
    Ok(groups)
}

fn reserve<T>(items: &mut Vec<T>, budget: &mut ResourceBudget) -> Result<(), InspectError> {
    budget.charge_allocation(1, size_of::<T>() as u64)?;
    items.try_reserve(1).map_err(|_| InspectError::Allocation)
}

fn copy(bytes: &[u8], budget: &mut ResourceBudget) -> Result<Vec<u8>, InspectError> {
    budget.charge_allocation(bytes.len() as u64, 1)?;
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(bytes.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.extend_from_slice(bytes);
    Ok(owned)
}

#[cfg(test)]
mod tests;
