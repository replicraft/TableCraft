//! Native source geometry and ownership, without a remembered creator flag.
use super::{AddTableError, append::Page};
use crate::{
    IndexCheckStatus, InspectError, NativeImage, PayloadPresence, check_table,
    create_blank_ace16_database,
    format::{layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES, row::reference},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) const OLD_PAGES: usize = 10;
pub(super) const NEW_DEFINITION: usize = 10;
pub(super) const NEW_DATA: usize = 11;
pub(super) const NEW_ROOT: usize = 12;
pub(super) const MAP_PAGE: usize = 3;
pub(super) const CATALOG_PAGE: usize = 4;
pub(super) const OWNED_SLOT: usize = 23;
pub(super) const CANDIDATE_SLOT: usize = 24;
pub(super) const INDEX_SLOT: usize = 25;
const OLD_CATALOG_ROWS: usize = 8;
const OLD_MAP_ROWS: usize = 23;
const FIRST_USER_DEFINITION: u32 = 7;
const FIRST_USER_DATA: u8 = 8;
const FIRST_USER_ROOT: u32 = 9;
const TABLES_ID: i32 = 0x0f00_0001;

pub(super) struct Source {
    pub catalog: Page,
    pub maps: Page,
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
    if bytes.len() != OLD_PAGES * PAGE_BYTES {
        return Err(AddTableError::Source);
    }
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
        != (OLD_CATALOG_ROWS as u32).to_le_bytes()
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
    if catalog.count != OLD_CATALOG_ROWS {
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
    if objects.objects().len() != OLD_CATALOG_ROWS {
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
    budget
        .charge(Resource::Steps, image.catalog().origins().len() as u64)
        .map_err(InspectError::from)?;
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.native_id() == FIRST_USER_DEFINITION as i32)
        .ok_or(AddTableError::Source)?;
    if origin.is_system()
        || origin.type_code() != 1
        || origin.parent_id() != TABLES_ID
        || origin.page() != CATALOG_PAGE as u32
        || origin.slot() != 7
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

    let maps = Page::capture(page(bytes, MAP_PAGE), 0, budget, cancelled)?;
    if maps.count != OLD_MAP_ROWS {
        return Err(AddTableError::Source);
    }
    for (slot, range) in maps.records.iter().enumerate() {
        check(budget, cancelled)?;
        let raw = &page(bytes, MAP_PAGE)[range.clone()];
        let mut expected = [0; 13];
        let member = match slot {
            0 | 4 => Some(CATALOG_PAGE as u8),
            3 | 7 => Some(FIRST_USER_DATA),
            22 => Some(FIRST_USER_ROOT as u8),
            _ => None,
        };
        if let Some(member) = member {
            expected[5 + usize::from(member) / 8] = 1 << (member % 8);
        }
        budget
            .charge(Resource::Steps, raw.len() as u64)
            .map_err(InspectError::from)?;
        // A candidate is only a hint; an absent hint never grants ownership.
        if slot == 7 && raw == [0; 13] {
            continue;
        }
        equal(raw, &expected, budget, cancelled)?;
    }
    if page(bytes, FIRST_USER_DEFINITION as usize)[4..8] != [0; 4]
        || page(bytes, FIRST_USER_DEFINITION as usize)[59..63]
            != reference(MAP_PAGE as u32, 7).to_le_bytes()
    {
        return Err(AddTableError::Source);
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
    if schema.definition_page() != FIRST_USER_DEFINITION
        || schema.physical_layout().usage != reference(MAP_PAGE as u32, 3)
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
        || index.index().root_page() != FIRST_USER_ROOT
        || index.index().usage_map() != reference(MAP_PAGE as u32, 22)
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
        page(bytes, FIRST_USER_DATA as usize),
        FIRST_USER_DEFINITION,
        budget,
        cancelled,
    )?;
    budget
        .charge(Resource::Steps, 27)
        .map_err(InspectError::from)?;
    let root = page(bytes, FIRST_USER_ROOT as usize);
    if root[..2] != [4, 1]
        || root[4..8] != FIRST_USER_DEFINITION.to_le_bytes()
        || root[8..27].iter().any(|byte| *byte != 0)
    {
        return Err(AddTableError::Source);
    }
    check(budget, cancelled)?;
    Ok(Source { catalog, maps })
}
