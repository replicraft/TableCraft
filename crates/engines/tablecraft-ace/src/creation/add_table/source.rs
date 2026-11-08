//! Native source geometry and ownership, without a remembered creator flag.
use super::{
    AddTableError,
    append::Page,
    geometry::{Geometry, Role},
};
use crate::{
    IndexCheckStatus, InspectError, NativeImage, PayloadPresence, check_table,
    create_blank_ace16_database,
    format::{data_type, layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES, row::reference},
    header::check,
    inspect_table_schema,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) const MAP_PAGE: usize = 3;
pub(super) const CATALOG_PAGE: usize = 4;
const BASE_MAP_ROWS: usize = 23;
const MAP_RECORD_BYTES: usize = 13;
const MAP_HEADER_BYTES: usize = 5;
const TABLES_ID: i32 = 0x0f00_0001;

pub(super) struct Source {
    pub catalog: Page,
    pub maps: Page,
    pub geometry: Geometry,
}

pub(super) fn page(bytes: &[u8], number: usize) -> &[u8] {
    &bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES]
}

fn equal(
    actual: &[u8],
    expected: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, actual.len().max(expected.len()) as u64)
        .map_err(crate::InspectError::from)?;
    if actual != expected {
        return Err(AddTableError::Source);
    }
    Ok(())
}

pub(super) fn capture(
    image: &NativeImage,
    proposed: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, AddTableError> {
    let source = qualify(image, budget, cancelled)?;
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, proposed.len() as u64)
        .map_err(InspectError::from)?;
    if !proposed.is_ascii() {
        return Err(AddTableError::Source);
    }
    let objects = image.catalog().catalog();
    for object in objects.objects() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                object.name().as_str().len().max(proposed.len()) as u64,
            )
            .map_err(InspectError::from)?;
    }
    if objects
        .lookup(proposed, None, None)
        .map_err(InspectError::from)?
        .is_some()
    {
        return Err(AddTableError::DuplicateName);
    }
    Ok(source)
}

pub(super) fn qualify(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, AddTableError> {
    check(budget, cancelled)?;
    image.require_standard_capture()?;
    let bytes = image.as_bytes();
    let geometry = Geometry::capture(bytes.len()).ok_or(AddTableError::Source)?;
    let canonical = create_blank_ace16_database(budget, cancelled)?;
    // These system-only roles must have their exact qualified native structure.
    // Existing user definitions, rows, property records and free tails are never rebuilt.
    for number in [0, 1, 5, 6] {
        equal(
            page(bytes, number),
            page(canonical.as_bytes(), number),
            budget,
            cancelled,
        )?;
    }
    let catalog_definition = page(bytes, 2);
    let expected_definition = page(canonical.as_bytes(), 2);
    equal(
        &catalog_definition[..RECORD_COUNT_OFFSET],
        &expected_definition[..RECORD_COUNT_OFFSET],
        budget,
        cancelled,
    )?;
    equal(
        &catalog_definition[RECORD_COUNT_OFFSET + size_of::<u32>()..],
        &expected_definition[RECORD_COUNT_OFFSET + size_of::<u32>()..],
        budget,
        cancelled,
    )?;
    if catalog_definition[RECORD_COUNT_OFFSET..RECORD_COUNT_OFFSET + size_of::<u32>()]
        != (geometry.catalog_rows as u32).to_le_bytes()
    {
        return Err(AddTableError::Source);
    }

    let catalog = Page::capture(page(bytes, CATALOG_PAGE), 2, budget, cancelled)?;
    let canonical_catalog = Page::capture(
        page(canonical.as_bytes(), CATALOG_PAGE),
        2,
        budget,
        cancelled,
    )?;
    if catalog.count != geometry.catalog_rows {
        return Err(AddTableError::Source);
    }
    for (actual, expected) in catalog.records.iter().zip(&canonical_catalog.records) {
        equal(
            &page(bytes, CATALOG_PAGE)[actual.clone()],
            &page(canonical.as_bytes(), CATALOG_PAGE)[expected.clone()],
            budget,
            cancelled,
        )?;
    }
    let objects = image.catalog().catalog();
    if objects.objects().len() != geometry.catalog_rows {
        return Err(AddTableError::Source);
    }
    for object in objects.objects() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, object.name().as_str().len() as u64)
            .map_err(InspectError::from)?;
        if !object.name().as_str().is_ascii() {
            return Err(AddTableError::Source);
        }
    }
    let maps = Page::capture(page(bytes, MAP_PAGE), 0, budget, cancelled)?;
    if maps.count != geometry.map_rows {
        return Err(AddTableError::Source);
    }
    for (slot, range) in maps.records.iter().enumerate() {
        check(budget, cancelled)?;
        let raw = &page(bytes, MAP_PAGE)[range.clone()];
        let mut expected = [0; MAP_RECORD_BYTES];
        let (member, candidate) = match slot {
            0 | 4 => (Some(CATALOG_PAGE), false),
            3 | 7 => (Some(8), slot == 7),
            22 => (Some(9), false),
            BASE_MAP_ROWS.. => {
                let table = 1 + (slot - BASE_MAP_ROWS) / 3;
                let role = geometry.table(table).ok_or(AddTableError::Source)?;
                let position = (slot - BASE_MAP_ROWS) % 3;
                (
                    Some(if position == 2 { role.root } else { role.data }),
                    position == 1,
                )
            }
            _ => (None, false),
        };
        if let Some(member) = member {
            expected[MAP_HEADER_BYTES + member / 8] = 1 << (member % 8);
        }
        budget
            .charge(Resource::Steps, raw.len() as u64)
            .map_err(InspectError::from)?;
        // A candidate is only a hint; an absent hint never grants ownership.
        if candidate && raw == [0; MAP_RECORD_BYTES] {
            continue;
        }
        equal(raw, &expected, budget, cancelled)?;
    }
    for table in 0..geometry.tables {
        check(budget, cancelled)?;
        qualify_table(
            image,
            geometry.table(table).ok_or(AddTableError::Source)?,
            geometry.tables > 1,
            budget,
            cancelled,
        )?;
    }
    check(budget, cancelled)?;
    Ok(Source {
        catalog,
        maps,
        geometry,
    })
}

fn qualify_table(
    image: &NativeImage,
    role: Role,
    repeated: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
    let bytes = image.as_bytes();
    let objects = image.catalog().catalog();
    budget
        .charge(Resource::Steps, image.catalog().origins().len() as u64)
        .map_err(InspectError::from)?;
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.native_id() == role.definition as i32)
        .ok_or(AddTableError::Source)?;
    if origin.is_system()
        || origin.type_code() != 1
        || origin.parent_id() != TABLES_ID
        || origin.page() != CATALOG_PAGE as u32
        || origin.slot() as usize != role.catalog
    {
        return Err(AddTableError::Source);
    }
    budget
        .charge(Resource::Steps, objects.objects().len() as u64)
        .map_err(InspectError::from)?;
    let object = objects
        .get(origin.object_id())
        .ok_or(AddTableError::Source)?;
    let payloads = image.inspect_object_payloads(origin.object_id(), budget, cancelled)?;
    if [payloads.body(), payloads.module(), payloads.extra()]
        .iter()
        .any(|presence| *presence != PayloadPresence::Absent)
        || !image
            .inspect_object_children(origin.object_id(), budget, cancelled)?
            .is_empty()
    {
        return Err(AddTableError::Source);
    }

    if page(bytes, role.definition)[4..8] != [0; 4]
        || page(bytes, role.definition)[59..63]
            != reference(MAP_PAGE as u32, role.candidate as u8).to_le_bytes()
    {
        return Err(AddTableError::Source);
    }
    // The repeated profile admits only ordinary in-row creator carriers.
    // LONG_TEXT/OLE and complex references can expose storage or program roles
    // outside this closed TD/DATA/root lineage. Preserve the original one-table
    // profile unchanged for existing TCAT1 records.
    if repeated {
        let schema = inspect_table_schema(
            &mut image.reader(),
            objects.scope(),
            object.name().as_str(),
            budget,
            cancelled,
        )?;
        budget
            .charge(Resource::Steps, schema.columns().len() as u64)
            .map_err(InspectError::from)?;
        if schema.columns().iter().any(|column| {
            column.extended_flags() != 0
                || !matches!(
                    column.native_type(),
                    data_type::BOOLEAN
                        | data_type::BYTE
                        | data_type::INT16
                        | data_type::INT32
                        | data_type::INT64
                        | data_type::CURRENCY
                        | data_type::SINGLE
                        | data_type::DOUBLE
                        | data_type::DATE_TIME
                        | data_type::SHORT_BINARY
                        | data_type::SHORT_TEXT
                        | data_type::GUID
                        | data_type::NUMERIC
                )
        }) {
            return Err(AddTableError::Source);
        }
    }
    let checked = check_table(
        &mut image.reader(),
        objects.scope(),
        object.name().as_str(),
        budget,
        cancelled,
    )?;
    let schema = checked.schema();
    budget
        .charge(Resource::Steps, schema.columns().len() as u64)
        .map_err(InspectError::from)?;
    if schema.definition_page() != role.definition as u32
        || schema.physical_layout().usage != reference(MAP_PAGE as u32, role.owned as u8)
        || schema.physical_layout().physical_indexes != 1
        || schema.physical_layout().logical_indexes != 1
        || schema
            .columns()
            .iter()
            .any(|column| column.scalar_type().is_none())
        || schema.columns().first().is_none_or(|column| {
            column.number() != 0 || column.native_type() != 4 || column.flags() != 7
        })
    {
        return Err(AddTableError::Source);
    }
    let [index] = checked.indexes() else {
        return Err(AddTableError::Source);
    };
    if index.status() != IndexCheckStatus::Checked
        || index.index().root_page() != role.root as u32
        || index.index().usage_map() != reference(MAP_PAGE as u32, role.index as u8)
        || index.index().native_type() != 1
        || index.index().flags() != 0x89
        || index.index().columns().len() != 1
        || index.index().columns()[0].number() != 0
    {
        return Err(AddTableError::Source);
    }
    // The single owned DATA page and root occupy distinct fixed roles; no
    // indirect maps, external descendants or append destinations are admitted.
    Page::capture(
        page(bytes, role.data),
        role.definition as u32,
        budget,
        cancelled,
    )?;
    budget
        .charge(Resource::Steps, 27)
        .map_err(InspectError::from)?;
    let root = page(bytes, role.root);
    if root[..2] != [4, 1]
        || root[4..8] != (role.definition as u32).to_le_bytes()
        || root[8..27].iter().any(|byte| *byte != 0)
    {
        return Err(AddTableError::Source);
    }
    check(budget, cancelled)?;
    Ok(())
}
