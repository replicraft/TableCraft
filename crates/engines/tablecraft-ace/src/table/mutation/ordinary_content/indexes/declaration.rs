//! Shared strict raw retained-index declarations; no page-role authority.
use super::super::refused;
use crate::table::indexes::validate_manual_guid_tuple;
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableSchema,
    format::{
        bytes::u16_at,
        data_type::{
            BYTE, CURRENCY, GUID, INT16, INT32, INT64, MAX_SHORT_BINARY_BYTES,
            MAX_SHORT_TEXT_UNITS, SHORT_BINARY, SHORT_TEXT,
        },
        layout::{self, UPDATABLE_FLAG},
        page::PAGE_BYTES,
    },
    header::check,
    inspect_table_index_counter,
    table::indexes::definition::{
        ASCENDING, DATA_NUMBER_OFFSET, FLAGS_OFFSET, KEY_BYTES, KEY_START, LOGICAL_BYTES,
        NUMBER_OFFSET, PHYSICAL_BYTES, ROOT_OFFSET, TYPE_OFFSET, UNUSED_COLUMN, USAGE_OFFSET,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Guid,
};
use tablecraft_schema::ScalarType;

const PHYSICAL_MAGIC: u32 = 1923;
const LOGICAL_MAGIC: u32 = 1625;
const MAGIC_OFFSET: usize = 0;
const HISTORY_OFFSET: usize = size_of::<u32>();
const NO_REFERENCE_OFFSET: usize = 13;
const NO_REFERENCE: u32 = u32::MAX;
const NAME_LENGTH_BYTES: usize = size_of::<u16>();
const TUPLE_RECORD_PASSES: usize = 4;

#[cfg(test)]
mod tests;

pub(super) fn names<'index>(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: impl Iterator<Item = &'index NativeIndex>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    let layout = &schema.layout;
    let start = layout.index_start;
    let physical_end = layout
        .physical_indexes
        .checked_mul(PHYSICAL_BYTES)
        .and_then(|bytes| start.checked_add(bytes))
        .ok_or(InspectError::StructureLimit)?;
    let logical_end = layout
        .logical_indexes
        .checked_mul(LOGICAL_BYTES)
        .and_then(|bytes| physical_end.checked_add(bytes))
        .ok_or(InspectError::StructureLimit)?;
    let page = page(image, schema.definition_page())?;
    let mut position = logical_end;
    for (ordinal, entry) in indexes.enumerate() {
        check(budget, cancelled)?;
        let index = entry;
        let begin = physical_end + ordinal * LOGICAL_BYTES;
        let raw = page.get(begin..begin + LOGICAL_BYTES).ok_or_else(refused)?;
        logical(raw, index, budget)?;
        let bytes = usize::from(u16_at(page, position)?);
        position += NAME_LENGTH_BYTES;
        let encoded = page.get(position..position + bytes).ok_or_else(refused)?;
        budget
            .charge(Resource::Steps, bytes as u64)
            .map_err(InspectError::from)?;
        if !bytes.is_multiple_of(size_of::<u16>())
            || !encoded
                .as_chunks::<2>()
                .0
                .iter()
                .map(|word| u16::from_le_bytes(*word))
                .eq(index.name().encode_utf16())
        {
            return Err(refused());
        }
        position += bytes;
    }

    check(budget, cancelled)?;
    Ok(position)
}

pub(super) fn physical<'index>(
    image: &NativeImage,
    schema: &TableSchema,
    index: &NativeIndex,
    physical: usize,
    indexes: impl Iterator<Item = &'index NativeIndex>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let start = schema.layout.index_start;
    let definition = schema.definition_page();
    let page = page(image, definition)?;
    let begin = start + physical * PHYSICAL_BYTES;
    let raw = page
        .get(begin..begin + PHYSICAL_BYTES)
        .ok_or_else(refused)?;
    if index.columns().len() > 1 {
        tuple_physical_record(raw, index, budget, cancelled)?;
    } else {
        // Preserve the existing single-key reconstruction and exact charge.
        physical_record(raw, index, budget)?;
    }
    // The public reader performs a name search in its fresh metadata.
    // Prepay that search from the same immutable original index names.
    for candidate in indexes {
        check(budget, cancelled)?;
        let bytes = candidate.name().len().max(index.name().len());
        budget
            .charge(Resource::Steps, bytes as u64)
            .map_err(InspectError::from)?;
    }
    let history = inspect_table_index_counter(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        index.name(),
        budget,
        cancelled,
    )?;
    let count_start = layout::COLUMN_BLOCK_OFFSET + physical * layout::INDEX_ENTRY_BYTES;
    let raw_count = page
        .get(count_start..count_start + layout::INDEX_ENTRY_BYTES)
        .ok_or_else(refused)?;
    let mut expected_count = [0; layout::INDEX_ENTRY_BYTES];
    expected_count[HISTORY_OFFSET..HISTORY_OFFSET + size_of::<u32>()]
        .copy_from_slice(&history.value().to_le_bytes());
    budget
        .charge(Resource::Steps, layout::INDEX_ENTRY_BYTES as u64)
        .map_err(InspectError::from)?;
    if history.definition_page() != definition
        || history.physical_index() as usize != physical
        || history.byte_range().start
            != definition as usize * PAGE_BYTES + count_start + HISTORY_OFFSET
        || raw_count != expected_count
    {
        return Err(refused());
    }

    check(budget, cancelled)?;
    Ok(())
}

pub(in crate::table::mutation::ordinary_content) fn validate_selected<'index>(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: impl Iterator<Item = &'index NativeIndex> + Clone,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    validate_for(
        image,
        schema,
        indexes,
        KeyFamily::SelectedIntegral,
        budget,
        cancelled,
    )
}

pub(in crate::table::mutation::ordinary_content) fn validate_actions<'index>(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: impl Iterator<Item = &'index NativeIndex> + Clone,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    validate_for(
        image,
        schema,
        indexes,
        KeyFamily::SparseIntegral,
        budget,
        cancelled,
    )
}

/// INSERT owns its declaration proof while sharing actual native-number key lookup.
pub(in crate::table::mutation::ordinary_content) fn validate_generated<'index>(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: impl Iterator<Item = &'index NativeIndex> + Clone,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    validate_for(
        image,
        schema,
        indexes,
        KeyFamily::SparseIntegral,
        budget,
        cancelled,
    )
}

pub(in crate::table::mutation::ordinary_content) fn validate_retained_updates<'index>(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: impl Iterator<Item = &'index NativeIndex> + Clone,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    validate_for(
        image,
        schema,
        indexes,
        KeyFamily::SelectedScalar,
        budget,
        cancelled,
    )
}

fn validate_for<'index>(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: impl Iterator<Item = &'index NativeIndex> + Clone,
    family: KeyFamily,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    let mut logical_count = 0_usize;
    for index in indexes.clone() {
        check(budget, cancelled)?;
        match family {
            KeyFamily::SelectedIntegral => qualify_selected(schema, index, budget, cancelled)?,
            _ => qualify_for(schema, index, family, budget, cancelled)?,
        }
        logical_count += 1;
    }
    if logical_count != schema.layout.logical_indexes {
        return Err(refused());
    }
    let end = names(image, schema, indexes.clone(), budget, cancelled)?;
    for physical_number in 0..schema.layout.physical_indexes {
        check(budget, cancelled)?;
        let mut selected = None;
        for index in indexes.clone() {
            check(budget, cancelled)?;
            if index.data_number() as usize != physical_number {
                continue;
            }
            if let Some(prior) = selected {
                aliases(prior, index, budget, cancelled)?;
            } else {
                selected = Some(index);
            }
        }
        let index = selected.ok_or_else(refused)?;
        physical(
            image,
            schema,
            index,
            physical_number,
            indexes.clone(),
            budget,
            cancelled,
        )?;
    }
    check(budget, cancelled)?;
    Ok(end)
}

fn page(image: &NativeImage, definition: u32) -> Result<&[u8], EditError> {
    let start = usize::try_from(definition)
        .ok()
        .and_then(|number| number.checked_mul(PAGE_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    let end = start
        .checked_add(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    image.as_bytes().get(start..end).ok_or_else(refused)
}
pub(super) fn qualify(
    schema: &TableSchema,
    index: &NativeIndex,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    qualify_for(schema, index, KeyFamily::GlobalInt32, budget, cancelled)
}

fn qualify_selected(
    schema: &TableSchema,
    index: &NativeIndex,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    qualify_for(
        schema,
        index,
        KeyFamily::SelectedIntegral,
        budget,
        cancelled,
    )
}

// Global content closure still uses the Int32 population and frame certifier.
// Selected-table closure has a separate complete readonly scalar-index proof.
#[derive(Clone, Copy)]
enum KeyFamily {
    GlobalInt32,
    SelectedIntegral,
    SelectedScalar,
    SparseIntegral,
}

fn qualify_for(
    schema: &TableSchema,
    index: &NativeIndex,
    family: KeyFamily,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if matches!(family, KeyFamily::SelectedScalar) && index.columns().len() > 1 {
        return validate_manual_guid_tuple(schema, index, budget, cancelled).map_err(|error| {
            match error {
                InspectError::Unsupported => refused(),
                other => other.into(),
            }
        });
    }
    let [key] = index.columns() else {
        return Err(refused());
    };
    let constraints = index.constraints().ok_or_else(refused)?;
    let field = if matches!(family, KeyFamily::SparseIntegral) {
        let mut selected = None;
        for field in schema.columns() {
            check(budget, cancelled)?;
            if field.number() == key.number() {
                if selected.is_some() {
                    return Err(refused());
                }
                selected = Some(field);
            }
        }
        selected.ok_or_else(refused)?
    } else {
        schema.columns().get(key.number()).ok_or_else(refused)?
    };
    if matches!(family, KeyFamily::GlobalInt32)
        && (field.native_type() != INT32 || field.scalar_type() != Some(ScalarType::Int32))
    {
        return Err(refused());
    }
    let binary = matches!(
        family,
        KeyFamily::SparseIntegral | KeyFamily::SelectedScalar
    ) && field.native_type() == SHORT_BINARY
        && field.scalar_type() == Some(ScalarType::Binary);
    let text = matches!(
        family,
        KeyFamily::SparseIntegral | KeyFamily::SelectedScalar
    ) && field.native_type() == SHORT_TEXT
        && field.scalar_type() == Some(ScalarType::Text);
    let guid = matches!(family, KeyFamily::SelectedScalar)
        && field.native_type() == GUID
        && field.scalar_type() == Some(ScalarType::Guid);
    let variable = binary || text;
    let width = match (field.native_type(), field.scalar_type()) {
        (BYTE, Some(ScalarType::Byte)) => size_of::<u8>(),
        (INT16, Some(ScalarType::Int16)) => size_of::<i16>(),
        (INT32, Some(ScalarType::Int32)) => size_of::<i32>(),
        (INT64, Some(ScalarType::Int64)) => size_of::<i64>(),
        (CURRENCY, Some(ScalarType::Currency)) => size_of::<i64>(),
        (GUID, Some(ScalarType::Guid)) if guid => Guid::BYTE_LEN,
        (SHORT_BINARY, Some(ScalarType::Binary)) if binary => field.storage_length(),
        (SHORT_TEXT, Some(ScalarType::Text)) if text => field.storage_length(),
        _ => return Err(refused()),
    };
    // This selected dense declaration belongs to the normal UPDATE absence
    // proof. It qualifies no physical mutation: full original membership and
    // each actual target are checked by their later owning services.
    let selected_omission = matches!(family, KeyFamily::SelectedIntegral)
        && !constraints.primary_key()
        && field.native_type() == INT32
        && field.scalar_type() == Some(ScalarType::Int32)
        && field.flags() == (layout::FIXED_FLAG | UPDATABLE_FLAG)
        && field.complex_type_id().is_none();
    if index.native_type() > 1
        || index.foreign_reference().is_some()
        || constraints.ignore_nulls() && !selected_omission
        || field.number() != key.number()
        || field.storage_length() != width
        || guid
            && (field.flags() != (layout::FIXED_FLAG | UPDATABLE_FLAG)
                || field.complex_type_id().is_some())
        || variable && field.fixed()
        || !variable && !field.fixed()
        || binary && !(1..=MAX_SHORT_BINARY_BYTES).contains(&width)
        || text
            && (field.flags() != UPDATABLE_FLAG
                || width == 0
                || !width.is_multiple_of(size_of::<u16>())
                || width / size_of::<u16>() > MAX_SHORT_TEXT_UNITS
                || field
                    .text_sort_order()
                    .and_then(|order| order.general_family())
                    .is_none())
        || field.extended_flags() != 0
    {
        return Err(refused());
    }
    Ok(())
}

pub(super) fn aliases(
    before: &NativeIndex,
    after: &NativeIndex,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, before.columns().len() as u64)
        .map_err(InspectError::from)?;
    if before.root_page() != after.root_page()
        || before.usage_map() != after.usage_map()
        || before.flags() != after.flags()
        || before.columns() != after.columns()
    {
        return Err(refused());
    }
    Ok(())
}

pub(super) fn physical_record(
    raw: &[u8],
    index: &NativeIndex,
    budget: &mut ResourceBudget,
) -> Result<(), EditError> {
    let mut expected = [0; PHYSICAL_BYTES];
    expected[MAGIC_OFFSET..MAGIC_OFFSET + size_of::<u32>()]
        .copy_from_slice(&PHYSICAL_MAGIC.to_le_bytes());
    for number in 0..layout::INDEX_KEY_COLUMNS {
        let at = KEY_START + number * KEY_BYTES;
        expected[at..at + size_of::<u16>()].copy_from_slice(&UNUSED_COLUMN.to_le_bytes());
    }
    let key = index.columns()[0];
    expected[KEY_START..KEY_START + size_of::<u16>()]
        .copy_from_slice(&(key.number() as u16).to_le_bytes());
    expected[KEY_START + size_of::<u16>()] = if key.descending() { 0 } else { ASCENDING };
    expected[USAGE_OFFSET..USAGE_OFFSET + size_of::<u32>()]
        .copy_from_slice(&index.usage_map().to_le_bytes());
    expected[ROOT_OFFSET..ROOT_OFFSET + size_of::<u32>()]
        .copy_from_slice(&index.root_page().to_le_bytes());
    expected[FLAGS_OFFSET] = index.flags();
    budget
        .charge(Resource::Steps, PHYSICAL_BYTES as u64)
        .map_err(InspectError::from)?;
    if raw != expected {
        return Err(refused());
    }
    Ok(())
}

/// Complete raw tuple slots after the caller has qualified every alias under
/// its actual declaration family. This proves raw closure, not Source permission.
fn tuple_physical_record(
    raw: &[u8],
    index: &NativeIndex,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if !(2..=layout::INDEX_KEY_COLUMNS).contains(&index.columns().len()) {
        return Err(refused());
    }
    // Prepay bounded zero-fill, header/slot writes and complete comparison
    // before reconstructing any expected bytes. Per-slot checks also cancel.
    budget
        .charge(
            Resource::Steps,
            (TUPLE_RECORD_PASSES * PHYSICAL_BYTES) as u64,
        )
        .map_err(InspectError::from)?;
    let mut expected = [0; PHYSICAL_BYTES];
    expected[MAGIC_OFFSET..MAGIC_OFFSET + size_of::<u32>()]
        .copy_from_slice(&PHYSICAL_MAGIC.to_le_bytes());
    for number in 0..layout::INDEX_KEY_COLUMNS {
        check(budget, cancelled)?;
        let at = KEY_START + number * KEY_BYTES;
        expected[at..at + size_of::<u16>()].copy_from_slice(&UNUSED_COLUMN.to_le_bytes());
    }
    for (position, key) in index.columns().iter().enumerate() {
        check(budget, cancelled)?;
        let at = KEY_START + position * KEY_BYTES;
        let number = u16::try_from(key.number()).map_err(|_| refused())?;
        expected[at..at + size_of::<u16>()].copy_from_slice(&number.to_le_bytes());
        expected[at + size_of::<u16>()] = if key.descending() { 0 } else { ASCENDING };
    }
    expected[USAGE_OFFSET..USAGE_OFFSET + size_of::<u32>()]
        .copy_from_slice(&index.usage_map().to_le_bytes());
    expected[ROOT_OFFSET..ROOT_OFFSET + size_of::<u32>()]
        .copy_from_slice(&index.root_page().to_le_bytes());
    expected[FLAGS_OFFSET] = index.flags();
    if raw != expected {
        return Err(refused());
    }
    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn logical(
    raw: &[u8],
    index: &NativeIndex,
    budget: &mut ResourceBudget,
) -> Result<(), EditError> {
    let mut expected = [0; LOGICAL_BYTES];
    expected[MAGIC_OFFSET..MAGIC_OFFSET + size_of::<u32>()]
        .copy_from_slice(&LOGICAL_MAGIC.to_le_bytes());
    expected[NUMBER_OFFSET..NUMBER_OFFSET + size_of::<u32>()]
        .copy_from_slice(&index.number().to_le_bytes());
    expected[DATA_NUMBER_OFFSET..DATA_NUMBER_OFFSET + size_of::<u32>()]
        .copy_from_slice(&index.data_number().to_le_bytes());
    expected[NO_REFERENCE_OFFSET..NO_REFERENCE_OFFSET + size_of::<u32>()]
        .copy_from_slice(&NO_REFERENCE.to_le_bytes());
    expected[TYPE_OFFSET] = index.native_type();
    budget
        .charge(Resource::Steps, LOGICAL_BYTES as u64)
        .map_err(InspectError::from)?;
    if raw != expected {
        return Err(refused());
    }
    Ok(())
}
