//! Strict mutation evidence layered over permissive inert property inspection.
//! Semantic key/type qualification remains with preserved_source::properties.
use super::{PropertyGroupKind, TableProperties};
use crate::{InspectError, header::check};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

const SIGNATURE: &[u8; 4] = b"MR2\0";
const NAME_LIST: u16 = 0x80;
const TABLE_VALUES: u16 = 0;
const COLUMN_VALUES: u16 = 1;
const BLOCK_HEADER: usize = size_of::<u32>() + size_of::<u16>();
const VALUE_HEADER: usize = size_of::<u16>() * 3 + size_of::<u8>() * 2;
const COPY_CHUNK: usize = 256;

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
        Ok(usize::from(u16::from_le_bytes(
            self.take(size_of::<u16>())?.try_into().unwrap(),
        )))
    }

    fn long(&mut self) -> Result<usize, InspectError> {
        Ok(u32::from_le_bytes(self.take(size_of::<u32>())?.try_into().unwrap()) as usize)
    }

    fn name(&mut self) -> Result<&'a [u8], InspectError> {
        let length = self.short()?;
        if !length.is_multiple_of(size_of::<u16>()) {
            return Err(InspectError::Malformed);
        }
        self.take(length)
    }
}

fn name_equal(raw: &[u8], actual: &str, budget: &mut ResourceBudget) -> Result<bool, InspectError> {
    budget.charge(Resource::Steps, raw.len() as u64)?;
    Ok(raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|word| u16::from_le_bytes(*word))
        .eq(actual.encode_utf16()))
}

fn bytes_equal(
    left: &[u8],
    right: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, InspectError> {
    if left.len() != right.len() {
        return Ok(false);
    }
    budget.charge(Resource::Steps, left.len() as u64)?;
    for (left, right) in left.chunks(COPY_CHUNK).zip(right.chunks(COPY_CHUNK)) {
        check(budget, cancelled)?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Every original byte must belong to a known MR2 frame and a decoded entry.
/// Empty/signature-only/name-only bags and unreferenced dictionary names refuse.
/// This does not classify property semantics or issue a write permission.
pub(crate) fn validate(
    properties: &TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    ordered_names(properties, budget, cancelled).map(|_| ())
}

/// Validate the complete framing and retain its existing borrowed dictionary order.
/// Names remain unclassified data; mutation callers must separately qualify them.
pub(crate) fn ordered_names<'a>(
    properties: &'a TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<&'a [u8]>, InspectError> {
    ordered_names_impl(properties, budget, cancelled, None::<GroupVisitor>)
}

type GroupVisitor = fn(usize, Range<usize>) -> Result<(), InspectError>;

/// Retain raw group-name spans while performing the same complete framing proof.
/// The ranges are relative to these exact raw property bytes. Callers must wait
/// for successful completion before using a visited span as mutation evidence.
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(crate) fn ordered_names_with_groups<'a>(
    properties: &'a TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    visit: impl FnMut(usize, Range<usize>) -> Result<(), InspectError>,
) -> Result<Vec<&'a [u8]>, InspectError> {
    ordered_names_impl(properties, budget, cancelled, Some(visit))
}

fn ordered_names_impl<'a>(
    properties: &'a TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    mut visit: Option<impl FnMut(usize, Range<usize>) -> Result<(), InspectError>>,
) -> Result<Vec<&'a [u8]>, InspectError> {
    let _span = tracing::debug_span!("native.properties.complete_framing").entered();
    check(budget, cancelled)?;
    let mut cursor = Cursor::new(properties.raw_bytes());
    if cursor.take(SIGNATURE.len())? != SIGNATURE || properties.groups().is_empty() {
        return Err(InspectError::Unsupported);
    }
    let length = cursor.long()?;
    if cursor.short()? != usize::from(NAME_LIST) {
        return Err(InspectError::Unsupported);
    }
    let mut dictionary = Cursor::new(
        cursor.take(
            length
                .checked_sub(BLOCK_HEADER)
                .ok_or(InspectError::Malformed)?,
        )?,
    );
    let mut names = Vec::new();
    let mut used = Vec::new();
    while dictionary.remaining() != 0 {
        check(budget, cancelled)?;
        let name = dictionary.name()?;
        if name.is_empty() {
            return Err(InspectError::Unsupported);
        }
        budget.charge_allocation(1, size_of::<&[u8]>() as u64)?;
        names
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        budget.charge_allocation(1, size_of::<bool>() as u64)?;
        used.try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        names.push(name);
        used.push(false);
    }
    if names.is_empty() {
        return Err(InspectError::Unsupported);
    }
    let mut group_number = 0_usize;
    while cursor.remaining() != 0 {
        check(budget, cancelled)?;
        let length = cursor.long()?;
        let kind = cursor.short()? as u16;
        let group = properties
            .groups()
            .get(group_number)
            .ok_or(InspectError::Malformed)?;
        if !matches!(
            (kind, group.kind()),
            (TABLE_VALUES, PropertyGroupKind::Table) | (COLUMN_VALUES, PropertyGroupKind::Column)
        ) || group.properties().is_empty()
        {
            return Err(InspectError::Unsupported);
        }
        let block_start = cursor.position;
        let mut block = Cursor::new(
            cursor.take(
                length
                    .checked_sub(BLOCK_HEADER)
                    .ok_or(InspectError::Malformed)?,
            )?,
        );
        let name_length = block.long()?;
        let mut group_name = Cursor::new(
            block.take(
                name_length
                    .checked_sub(size_of::<u32>())
                    .ok_or(InspectError::Malformed)?,
            )?,
        );
        let raw_name = group_name.name()?;
        if !name_equal(raw_name, group.name(), budget)? || group_name.remaining() != 0 {
            return Err(InspectError::Unsupported);
        }
        if let Some(visit) = visit.as_mut() {
            budget.charge(Resource::Steps, 1)?;
            let name_start = block_start + size_of::<u32>() + size_of::<u16>();
            visit(group_number, name_start..name_start + raw_name.len())?;
        }
        let mut property_number = 0_usize;
        while block.remaining() != 0 {
            check(budget, cancelled)?;
            let length = block.short()?;
            if length < VALUE_HEADER {
                return Err(InspectError::Malformed);
            }
            let mut value = Cursor::new(block.take(length - size_of::<u16>())?);
            let flags = value.take(1)?[0];
            let native_type = value.take(1)?[0];
            let name_index = value.short()?;
            let size = value.short()?;
            let raw = value.take(size)?;
            let property = group
                .properties()
                .get(property_number)
                .ok_or(InspectError::Malformed)?;
            let name = names.get(name_index).ok_or(InspectError::Malformed)?;
            if value.remaining() != 0
                || flags != property.flags()
                || native_type != property.native_type()
                || !name_equal(name, property.name(), budget)?
                || !bytes_equal(raw, property.raw_value(), budget, cancelled)?
            {
                return Err(InspectError::Unsupported);
            }
            used[name_index] = true;
            property_number += 1;
        }
        if property_number != group.properties().len() {
            return Err(InspectError::Malformed);
        }
        group_number += 1;
    }
    if group_number != properties.groups().len() {
        return Err(InspectError::Malformed);
    }
    for assigned in used {
        check(budget, cancelled)?;
        if !assigned {
            return Err(InspectError::Unsupported);
        }
    }
    check(budget, cancelled)?;
    Ok(names)
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(all(test, feature = "ace16-create", feature = "journal"))]
mod name_spans;
