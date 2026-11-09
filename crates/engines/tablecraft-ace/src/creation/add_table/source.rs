//! Native source geometry and ownership, without a remembered creator flag.
use super::{
    AddTableError,
    geometry::{Geometry, MAX_TABLES, Role, SYSTEM_OBJECTS, SYSTEM_PAGES},
};
use crate::creation::catalog_page::Page;
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
const MAP_RECORD_BYTES: usize = 13;
const MAP_HEADER_BYTES: usize = 5;
const TABLES_ID: i32 = 0x0f00_0001;

pub(in crate::creation) struct Source {
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
    require_name(image, proposed, budget, cancelled)?;
    Ok(source)
}

pub(in crate::creation) fn require_name(
    image: &NativeImage,
    proposed: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
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
    Ok(())
}

pub(super) fn qualify(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, AddTableError> {
    check(budget, cancelled)?;
    let geometry = Geometry::capture(image.as_bytes().len()).ok_or(AddTableError::Source)?;
    qualify_geometry(image, geometry, true, budget, cancelled)
}

pub(in crate::creation) fn qualify_geometry(
    image: &NativeImage,
    geometry: Geometry,
    legacy: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, AddTableError> {
    check(budget, cancelled)?;
    image.require_standard_capture()?;
    let bytes = image.as_bytes();
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
            // Read independently captured source/canonical catalog or map record using borrowed
            // inventory bounds; inventories remain reusable. Only range bounds are copied, not
            // the payload.
            &page(bytes, CATALOG_PAGE)[actual.clone()],
            // Read independently captured source/canonical catalog or map record using borrowed
            // inventory bounds; inventories remain reusable. Only range bounds are copied, not
            // the payload.
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
        // Read independently captured source/canonical catalog or map record using borrowed
        // inventory bounds; inventories remain reusable. Only range bounds are copied, not the
        // payload.
        let raw = &page(bytes, MAP_PAGE)[range.clone()];
        let mut expected = [0; MAP_RECORD_BYTES];
        let mut member = if matches!(slot, 0 | 4) {
            Some(CATALOG_PAGE)
        } else {
            None
        };
        let mut candidate = false;
        for table in 0..geometry.tables {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, 4)
                .map_err(InspectError::from)?;
            let role = geometry.table(table).ok_or(AddTableError::Source)?;
            if slot == role.owned || slot == role.candidate {
                member = Some(role.data);
                candidate = slot == role.candidate;
            } else if role.primary.is_some_and(|primary| primary.map == slot) {
                member = role.primary.map(|primary| primary.root);
            }
        }
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
            legacy,
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
    legacy: bool,
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
    if repeated || !legacy {
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
        || schema.physical_layout().physical_indexes != usize::from(role.primary.is_some())
        || schema.physical_layout().logical_indexes != usize::from(role.primary.is_some())
        || schema
            .columns()
            .iter()
            .any(|column| column.scalar_type().is_none())
    {
        return Err(AddTableError::Source);
    }
    if legacy
        && schema.columns().first().is_none_or(|column| {
            column.number() != 0 || column.native_type() != 4 || column.flags() != 7
        })
    {
        return Err(AddTableError::Source);
    }
    if !legacy {
        for (position, column) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            let generated = column.flags() == 7;
            let flags = if column.fixed() { 3 } else { 2 };
            if column.number() != position
                || column.extended_flags() != 0
                || (column.flags() != flags && !generated)
                || (generated
                    && (position != 0 || column.native_type() != 4 || role.primary.is_none()))
            {
                return Err(AddTableError::Source);
            }
        }
    }
    if let Some(primary) = role.primary {
        let [index] = checked.indexes() else {
            return Err(AddTableError::Source);
        };
        let [key] = index.index().columns() else {
            return Err(AddTableError::Source);
        };
        let key_column = schema
            .columns()
            .get(key.number())
            .ok_or(AddTableError::Source)?;
        if index.status() != IndexCheckStatus::Checked
            || index.index().root_page() != primary.root as u32
            || index.index().usage_map() != reference(MAP_PAGE as u32, primary.map as u8)
            || index.index().native_type() != 1
            || index.index().flags() != 0x89
            || key.descending()
            || (legacy && key.number() != 0)
            || !matches!(
                key_column.native_type(),
                data_type::INT32 | data_type::INT64 | data_type::CURRENCY
            )
            || (schema
                .columns()
                .first()
                .is_some_and(|column| column.flags() == 7)
                && key.number() != 0)
        {
            return Err(AddTableError::Source);
        }
    } else if !checked.indexes().is_empty() {
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
    if let Some(primary) = role.primary {
        let root = page(bytes, primary.root);
        if root[..2] != [4, 1]
            || root[4..8] != (role.definition as u32).to_le_bytes()
            || root[8..27].iter().any(|byte| *byte != 0)
        {
            return Err(AddTableError::Source);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

/// Derive roles from ordered native catalog/schema facts before selecting a
/// construction profile. This is not a fallback from failed legacy admission.
pub(in crate::creation) fn derive_geometry(
    image: &NativeImage,
    keyed: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Geometry, bool), AddTableError> {
    check(budget, cancelled)?;
    let bytes = image.as_bytes();
    if !bytes.len().is_multiple_of(PAGE_BYTES) || !(9..=62).contains(&(bytes.len() / PAGE_BYTES)) {
        return Err(AddTableError::Source);
    }
    let objects = image.catalog().catalog();
    let tables = objects
        .objects()
        .len()
        .checked_sub(SYSTEM_OBJECTS)
        .ok_or(AddTableError::Source)?;
    if tables == 0 || tables > MAX_TABLES {
        return Err(AddTableError::Source);
    }
    budget
        .charge(
            Resource::Steps,
            size_of::<[Option<Role>; MAX_TABLES]>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut roles = [None; MAX_TABLES];
    let mut next = SYSTEM_PAGES;
    let mut legacy = true;
    for (table, slot) in roles.iter_mut().take(tables).enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, image.catalog().origins().len() as u64)
            .map_err(InspectError::from)?;
        let origin = image
            .catalog()
            .origins()
            .iter()
            .find(|origin| {
                origin.page() == CATALOG_PAGE as u32
                    && origin.slot() as usize == SYSTEM_OBJECTS + table
            })
            .ok_or(AddTableError::Source)?;
        if origin.native_id() != next as i32 || origin.is_system() || origin.type_code() != 1 {
            return Err(AddTableError::Source);
        }
        let object = objects
            .get(origin.object_id())
            .ok_or(AddTableError::Source)?;
        let schema = inspect_table_schema(
            &mut image.reader(),
            objects.scope(),
            object.name().as_str(),
            budget,
            cancelled,
        )?;
        let physical = schema.physical_layout();
        if physical.physical_indexes > 1 || physical.logical_indexes != physical.physical_indexes {
            return Err(AddTableError::Source);
        }
        let has_key = physical.physical_indexes == 1;
        let role = Role::at(table, next, has_key).ok_or(AddTableError::Source)?;
        legacy &= has_key
            && schema.columns().first().is_some_and(|column| {
                column.number() == 0 && column.native_type() == 4 && column.flags() == 7
            });
        next += role.pages();
        *slot = Some(role);
    }
    if next != bytes.len() / PAGE_BYTES {
        return Err(AddTableError::Source);
    }
    let geometry =
        Geometry::from_roles(roles, tables, next, keyed).ok_or(AddTableError::Capacity)?;
    Ok((geometry, legacy))
}
