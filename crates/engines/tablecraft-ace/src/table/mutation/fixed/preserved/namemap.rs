//! Conservative current-name binding for native Name AutoCorrect records.
//! Historical, foreign or unknown records never establish dependency absence.
use crate::{
    EditError, InspectError, NativeProperty, PropertyGroupKind, TableProperties, TableSchema,
    WriteBlocker, format::data_type, header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: u32 = 0x550e_cc0a;
const GUID_BYTES: usize = 16;
const FIXED_BYTES: usize = GUID_BYTES + size_of::<u32>() + GUID_BYTES;
const TABLE: u32 = 0;
const REFERENCE: u32 = 6;
const COLUMN: u32 = 7;
const END: u32 = 12;

pub(super) fn qualify(
    schema: &TableSchema,
    properties: &TableProperties,
    property: &NativeProperty,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let bytes = property.raw_value();
    if property.native_type() != data_type::OLE
        || property.flags() != 0
        || bytes.len() < size_of::<u32>()
        || long(bytes, 0)? != MAGIC
    {
        return Err(refused());
    }
    if bytes.len() == size_of::<u32>() {
        return Ok(());
    }
    if bytes.len() < size_of::<u32>() * 2 || long(bytes, size_of::<u32>())? != 0 {
        return Err(refused());
    }
    let table_guid = guid(properties, "", budget, cancelled)?.ok_or_else(refused)?;
    if table_guid.iter().all(|byte| *byte == 0) {
        return Err(refused());
    }
    budget
        .charge_allocation(schema.columns().len() as u64, size_of::<bool>() as u64)
        .map_err(InspectError::from)?;
    let mut seen = Vec::new();
    seen.try_reserve_exact(schema.columns().len())
        .map_err(|_| InspectError::Allocation)?;
    seen.resize(schema.columns().len(), false);
    let capacity = schema
        .columns()
        .len()
        .checked_add(1)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(capacity as u64, size_of::<[u8; GUID_BYTES]>() as u64)
        .map_err(InspectError::from)?;
    let mut guids: Vec<[u8; GUID_BYTES]> = Vec::new();
    guids
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    let mut table_seen = false;
    let mut ended = false;
    let mut cursor = size_of::<u32>() * 2;
    let mut records = 0_usize;

    while cursor < bytes.len() {
        check(budget, cancelled)?;
        records += 1;
        if records
            > schema
                .columns()
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(2))
                .ok_or(InspectError::StructureLimit)?
        {
            return Err(refused());
        }
        let fixed = bytes
            .get(
                cursor
                    ..cursor
                        .checked_add(FIXED_BYTES)
                        .ok_or(InspectError::StructureLimit)?,
            )
            .ok_or_else(refused)?;
        let own = &fixed[..GUID_BYTES];
        let kind = long(fixed, GUID_BYTES)?;
        let payload = &fixed[GUID_BYTES + size_of::<u32>()..];
        cursor += FIXED_BYTES;
        let start = cursor;
        loop {
            check(budget, cancelled)?;
            let word = bytes
                .get(cursor..cursor + size_of::<u16>())
                .ok_or_else(refused)?;
            cursor += size_of::<u16>();
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            if word == [0, 0] {
                break;
            }
        }
        let name = &bytes[start..cursor - size_of::<u16>()];
        if ended {
            return Err(refused());
        }
        match kind {
            TABLE
                if !table_seen && own == table_guid && same_name(name, schema.name(), budget)? =>
            {
                table_seen = true;
                guids.push(own.try_into().map_err(|_| refused())?);
            }
            COLUMN if table_seen && payload == table_guid => {
                budget
                    .charge(Resource::Steps, (guids.len() * GUID_BYTES) as u64)
                    .map_err(InspectError::from)?;
                if own == table_guid
                    || own.iter().all(|byte| *byte == 0)
                    || guids.iter().any(|guid| guid.as_slice() == own)
                {
                    return Err(refused());
                }

                let mut selected = None;
                for (position, field) in schema.columns().iter().enumerate() {
                    check(budget, cancelled)?;
                    let actual = guid(properties, field.name(), budget, cancelled)?;
                    let matched = if let Some(actual) = actual {
                        budget
                            .charge(Resource::Steps, GUID_BYTES as u64)
                            .map_err(InspectError::from)?;
                        own == actual
                    } else {
                        field.complex_type_id().is_some() && same_name(name, field.name(), budget)?
                    };
                    if matched
                        && (selected.replace(position).is_some()
                            || !same_name(name, field.name(), budget)?)
                    {
                        return Err(refused());
                    }
                }
                let selected = selected.ok_or_else(refused)?;
                if seen[selected] {
                    return Err(refused());
                }
                seen[selected] = true;
                guids.push(own.try_into().map_err(|_| refused())?);
            }
            REFERENCE
                if table_seen && own.iter().all(|byte| *byte == 0) && payload == table_guid =>
            {
                let mut matches = 0;
                for field in schema.columns() {
                    check(budget, cancelled)?;
                    if same_name(name, field.name(), budget)? {
                        matches += 1;
                    }
                }
                if matches != 1 {
                    return Err(refused());
                }
            }
            END if table_seen
                && own.iter().all(|byte| *byte == 0)
                && name.is_empty()
                && cursor == bytes.len() =>
            {
                ended = true;
            }
            _ => return Err(refused()),
        }
        if cursor < bytes.len() {
            if long(bytes, cursor)? != 0 {
                return Err(refused());
            }
            cursor += size_of::<u32>();
            if cursor == bytes.len() {
                return Err(refused());
            }
        }
    }
    if !table_seen || !ended || seen.iter().any(|seen| !seen) {
        return Err(refused());
    }
    check(budget, cancelled)?;
    Ok(())
}

fn guid<'a>(
    properties: &'a TableProperties,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<&'a [u8]>, EditError> {
    for group in properties.groups() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, group.name().len().max(name.len()) as u64)
            .map_err(InspectError::from)?;
        if group.kind()
            == if name.is_empty() {
                PropertyGroupKind::Table
            } else {
                PropertyGroupKind::Column
            }
            && group.name().eq_ignore_ascii_case(name)
        {
            for property in group.properties() {
                check(budget, cancelled)?;
                budget
                    .charge(
                        Resource::Steps,
                        property.name().len().max("GUID".len()) as u64,
                    )
                    .map_err(InspectError::from)?;
                if property.name().eq_ignore_ascii_case("GUID") {
                    if property.native_type() != data_type::SHORT_BINARY
                        || property.raw_value().len() != GUID_BYTES
                        || property.flags() != 1
                    {
                        return Err(refused());
                    }
                    return Ok(Some(property.raw_value()));
                }
            }
            return Ok(None);
        }
    }
    Ok(None)
}

fn same_name(bytes: &[u8], name: &str, budget: &mut ResourceBudget) -> Result<bool, EditError> {
    budget
        .charge(Resource::Steps, bytes.len().max(name.len()) as u64)
        .map_err(InspectError::from)?;
    Ok(bytes.as_chunks::<2>().1.is_empty()
        && bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .eq(name.encode_utf16()))
}

fn long(bytes: &[u8], offset: usize) -> Result<u32, EditError> {
    let end = offset
        .checked_add(size_of::<u32>())
        .ok_or(InspectError::StructureLimit)?;
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or_else(refused)?
            .try_into()
            .map_err(|_| refused())?,
    ))
}

fn refused() -> EditError {
    EditError::Unsupported(WriteBlocker::Properties)
}
