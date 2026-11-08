//! Closed native role/dependency and physical-ownership admission.
use super::{NativeTableRenameIdentity, TableRenameError, copy_text, record};
use crate::{
    EditError, FileFormat, InspectError, NativeColumn, NativeImage, NativeObjectOrigin,
    NativeTableStorage, PropertyGroupKind, TableSchema, compare_general_text,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u32_at,
        data_type, definition,
        layout::{
            CALCULATED_EXT_MASK, TABLE_FREE_MAP_OFFSET, UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG,
        },
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES, PAGE_NUMBER_MASK},
        reader::{Reader, row_count, row_range},
        row::OFFSET_MASK,
    },
    header::check,
    inspect_catalog, inspect_database_properties, inspect_table_schema,
};
use std::{cmp::Ordering, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::ObjectKind;

const COMPARISON_CHUNK_BYTES: usize = 64 * 1024;
const SYSTEM_FLAGS: u32 = 0x8000_0002;
const CONTAINER_TYPE: i16 = 3;
const DATABASE_TYPE: i16 = 2;
const DDL_FLAG: u8 = 1;
const TRACK_NAMES: &str = "Track Name AutoCorrect Info";
const PERFORM_NAMES: &str = "Perform Name AutoCorrect";
const SYSTEM_COLUMN_FLAG: u8 = 0x10;
const CATALOG_VARIABLE_FLAGS: u8 = SYSTEM_COLUMN_FLAG | UPDATABLE_FLAG;
const COLUMN_MAP_BYTES: usize = size_of::<u16>() + 2 * size_of::<u32>();
const DECLARATION_TERMINATOR_BYTES: usize = size_of::<u16>();
const SYSTEM_ROLES: [&str; 8] = [
    "DatabaseRoot",
    "Tables",
    "Databases",
    "Relationships",
    "MSysObjects",
    "MSysRelationships",
    "MSysACEs",
    "MSysDb",
];

pub(super) struct Qualified {
    pub catalog_schema: TableSchema,
    pub identity: NativeTableRenameIdentity,
    pub table: String,
    pub proposed: String,
    name_column: usize,
}

impl Qualified {
    pub fn name_column(&self) -> &NativeColumn {
        &self.catalog_schema.columns()[self.name_column]
    }
}

pub(super) fn serialization_error(error: EditError) -> TableRenameError {
    match error {
        EditError::Inspect(error) => TableRenameError::Inspect(error),
        EditError::Preservation => TableRenameError::Preservation,
        _ => TableRenameError::Source,
    }
}

pub(super) fn qualify(
    image: &NativeImage,
    requested: &str,
    proposed: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Qualified, TableRenameError> {
    check(budget, cancelled)?;
    let header = image.catalog().header();
    if !matches!(header.format(), FileFormat::Ace2016 | FileFormat::Ace2019)
        || header.subversion() != 0
        || header.encoded_pages()
    {
        return Err(TableRenameError::Source);
    }
    let inventory = image.catalog();
    let objects = inventory.catalog().objects();
    let origins = inventory.origins();
    if objects.len() != SYSTEM_ROLES.len() + 1 {
        return Err(TableRenameError::Dependency);
    }
    let root = find_role(image, "DatabaseRoot", budget, cancelled)?;
    let tables = find_role(image, "Tables", budget, cancelled)?;
    let databases = find_role(image, "Databases", budget, cancelled)?;
    let relationships = find_role(image, "Relationships", budget, cancelled)?;
    for container in [root, tables, databases, relationships] {
        if container.type_code() != CONTAINER_TYPE
            || !container.is_system()
            || container.flags() & !SYSTEM_FLAGS != 0
            || container.parent_id()
                != if container.object_id() == root.object_id() {
                    0
                } else {
                    root.native_id()
                }
        {
            return Err(TableRenameError::Source);
        }
    }
    let catalog = find_role(image, "MSysObjects", budget, cancelled)?;
    let system_relationships = find_role(image, "MSysRelationships", budget, cancelled)?;
    let aces = find_role(image, "MSysACEs", budget, cancelled)?;
    for origin in [catalog, system_relationships, aces] {
        if origin.table_storage() != Some(NativeTableStorage::Local)
            || !origin.is_system()
            || origin.flags() & !SYSTEM_FLAGS != 0
            || origin.parent_id() != tables.native_id()
        {
            return Err(TableRenameError::Source);
        }
    }
    let db = find_role(image, "MSysDb", budget, cancelled)?;
    if db.type_code() != DATABASE_TYPE
        || !db.is_system()
        || db.flags() & !SYSTEM_FLAGS != 0
        || db.parent_id() != databases.native_id()
    {
        return Err(TableRenameError::Dependency);
    }

    let mut user = None;
    for (object, origin) in objects.iter().zip(origins) {
        check(budget, cancelled)?;
        if !origin.is_system() {
            if user.replace((object, origin)).is_some()
                || object.kind() != ObjectKind::Table
                || origin.table_storage() != Some(NativeTableStorage::Local)
                || origin.flags() != 0
                || origin.parent_id() != tables.native_id()
            {
                return Err(TableRenameError::Dependency);
            }
        } else if ![
            root,
            tables,
            databases,
            relationships,
            catalog,
            system_relationships,
            aces,
            db,
        ]
        .iter()
        .any(|known| known.object_id() == origin.object_id())
        {
            return Err(TableRenameError::Dependency);
        }
    }
    let (object, origin) = user.ok_or(TableRenameError::Source)?;
    budget
        .charge(
            Resource::Steps,
            (requested.len() + object.name().as_str().len()) as u64,
        )
        .map_err(InspectError::from)?;
    if !object.name().as_str().eq_ignore_ascii_case(requested) {
        return Err(TableRenameError::Source);
    }
    let definition_page =
        u32::try_from(origin.native_id()).map_err(|_| TableRenameError::Source)?;
    if definition_page == 0 || definition_page & !PAGE_NUMBER_MASK != 0 {
        return Err(TableRenameError::Source);
    }
    let identity = NativeTableRenameIdentity {
        object_id: origin.object_id(),
        native_id: origin.native_id(),
        parent_id: origin.parent_id(),
        definition_page,
        catalog_page: origin.page(),
        catalog_slot: origin.slot(),
    };

    let scope = inventory.catalog().scope();
    let properties = inspect_database_properties(&mut image.reader(), scope, budget, cancelled)?
        .ok_or(TableRenameError::Dependency)?;
    if properties.object_id() != db.object_id() || properties.groups().len() != 1 {
        return Err(TableRenameError::Dependency);
    }
    let group = &properties.groups()[0];
    if group.kind() != PropertyGroupKind::Table
        || !group.name().is_empty()
        || group.properties().len() != 2
    {
        return Err(TableRenameError::Dependency);
    }
    for name in [TRACK_NAMES, PERFORM_NAMES] {
        budget
            .charge(
                Resource::Steps,
                group
                    .properties()
                    .iter()
                    .map(|property| property.name().len().max(name.len()) as u64)
                    .sum(),
            )
            .map_err(InspectError::from)?;
        let mut matching = group
            .properties()
            .iter()
            .filter(|property| property.name().eq_ignore_ascii_case(name));
        let property = matching.next().ok_or(TableRenameError::Dependency)?;
        if matching.next().is_some()
            || property.native_type() != data_type::INT32
            || property.flags() != DDL_FLAG
            || property.raw_value() != 0_i32.to_le_bytes()
        {
            return Err(TableRenameError::Dependency);
        }
    }

    let catalog_schema =
        inspect_table_schema(&mut image.reader(), scope, "MSysObjects", budget, cancelled)?;
    closed_catalog_geometry(&catalog_schema, budget, cancelled)?;
    let name_column = catalog_schema
        .columns()
        .iter()
        .position(|column| column.name().eq_ignore_ascii_case("Name"))
        .ok_or(TableRenameError::Source)?;
    let name = &catalog_schema.columns()[name_column];
    if name.fixed()
        || name.native_type() != data_type::SHORT_TEXT
        || name.flags() != CATALOG_VARIABLE_FLAGS
        || name.extended_flags() & !UNICODE_COMPRESSION_FLAG != 0
        || name.complex_type_id().is_some()
        || name.storage_length() > data_type::MAX_SHORT_TEXT_UNITS * size_of::<u16>()
        || !name.storage_length().is_multiple_of(size_of::<u16>())
    {
        return Err(TableRenameError::Source);
    }
    let order = name.text_sort_order().ok_or(TableRenameError::Source)?;
    // One ordinary user table is deliberate: wider identifier equality and
    // dependency repair require separate admission. Compare against every other
    // native object conservatively under the stored, reviewed General policy.
    for other in objects {
        check(budget, cancelled)?;
        if other.id() != object.id()
            && compare_general_text(order, proposed, other.name().as_str(), budget, cancelled)?
                == Ordering::Equal
        {
            return Err(TableRenameError::DuplicateName);
        }
    }
    for origin in origins {
        record::qualify_payloads(
            image,
            &catalog_schema,
            origin,
            origin.object_id() == db.object_id(),
            properties.raw_bytes(),
            budget,
            cancelled,
        )?;
    }
    qualify_storage(image, identity, budget, cancelled)?;
    Ok(Qualified {
        table: copy_text(object.name().as_str(), budget, cancelled)?,
        proposed: copy_text(proposed, budget, cancelled)?,
        catalog_schema,
        identity,
        name_column,
    })
}

fn closed_catalog_geometry(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableRenameError> {
    if schema.columns().len() != schema.layout.max_columns {
        return Err(TableRenameError::Source);
    }
    // Schema construction sorts actual columns by their distinct native column
    // numbers. Exhausting that entire range excludes retired/uninspected mask
    // bits, whose payloads cannot be assumed absent from catalog dependencies.
    let mut seen_variables = [false; crate::limits::MAX_COLUMNS];
    let mut variable_count = 0;
    for (position, column) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if column.number() != position {
            return Err(TableRenameError::Source);
        }
        if column.fixed() {
            continue;
        }
        let variable = column.field.variable;
        if variable >= schema.layout.variable_slots
            || variable >= seen_variables.len()
            || seen_variables[variable]
        {
            return Err(TableRenameError::Source);
        }
        seen_variables[variable] = true;
        variable_count += 1;
    }
    // Unique indices inside the declared range, with equal cardinality, exhaust
    // every native variable slot. Equal counts without uniqueness are inadequate.
    if variable_count != schema.layout.variable_slots {
        return Err(TableRenameError::Source);
    }
    Ok(())
}

fn find_role<'a>(
    image: &'a NativeImage,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a NativeObjectOrigin, TableRenameError> {
    let mut matched = None;
    for (object, origin) in image
        .catalog()
        .catalog()
        .objects()
        .iter()
        .zip(image.catalog().origins())
    {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                object.name().as_str().len().max(name.len()) as u64,
            )
            .map_err(InspectError::from)?;
        if object.name().as_str().eq_ignore_ascii_case(name) && matched.replace(origin).is_some() {
            return Err(TableRenameError::Source);
        }
    }
    matched.ok_or(TableRenameError::Source)
}

fn qualify_storage(
    image: &NativeImage,
    identity: NativeTableRenameIdentity,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableRenameError> {
    let mut cursor = image.reader();
    let mut reader = Reader {
        source: &mut cursor,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    let protected =
        RepresentedMap::read_storage(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    if protected.pages.contains(&identity.catalog_page()) {
        return Err(TableRenameError::Source);
    }
    global.require_allocated_storage(&mut reader, &global)?;

    for (object, origin) in image
        .catalog()
        .catalog()
        .objects()
        .iter()
        .zip(image.catalog().origins())
    {
        if origin.table_storage() != Some(NativeTableStorage::Local) {
            continue;
        }
        // The catalog guarantees unique native IDs, not unique masked TDEF
        // pages. Unknown high bits could alias another ordinary local table.
        let native_definition =
            u32::try_from(origin.native_id()).map_err(|_| TableRenameError::Source)?;
        if native_definition == 0 || native_definition & !PAGE_NUMBER_MASK != 0 {
            return Err(TableRenameError::Source);
        }
        let schema = inspect_table_schema(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            object.name().as_str(),
            reader.budget,
            cancelled,
        )?;
        if schema.definition_page() != native_definition {
            return Err(TableRenameError::Source);
        }
        if origin.object_id() == identity.object_id()
            && schema.definition_page() != identity.definition_page()
        {
            return Err(TableRenameError::Source);
        }
        if schema.layout.physical_indexes != 0 || schema.layout.logical_indexes != 0 {
            return Err(TableRenameError::Source);
        }
        if !origin.is_system()
            && schema.columns().iter().any(|column| {
                column.scalar_type().is_none()
                    || matches!(column.native_type(), data_type::LONG_TEXT | data_type::OLE)
                    || column.complex_type_id().is_some()
                    || column.extended_flags() & CALCULATED_EXT_MASK != 0
            })
        {
            return Err(TableRenameError::Dependency);
        }
        let raw_definition =
            definition::read_with_pages(&mut reader, schema.definition_page(), |page, reader| {
                if page == identity.catalog_page() {
                    return Err(InspectError::Malformed);
                }
                global.require_allocated(reader, page)
            })?;
        let free_map = u32_at(&raw_definition, TABLE_FREE_MAP_OFFSET)?;
        for (reference, role) in [
            (schema.layout.usage, MapRole::Owned),
            (free_map, MapRole::TableFreeSpace),
        ] {
            let storage = RepresentedMap::read_storage(&mut reader, reference, role)?;
            if storage.pages.contains(&identity.catalog_page()) {
                return Err(TableRenameError::Source);
            }
            for page in storage.pages {
                global.require_allocated(&mut reader, page)?;
            }
        }
        let long_values = crate::table::long_value::maps::declarations(&mut reader, &schema)?;
        if !long_values.complete {
            return Err(TableRenameError::Source);
        }
        let definitions = crate::table::indexes::definitions_with_tail(&mut reader, &schema)?;
        let tail_end = definitions
            .tail
            .checked_add(long_values.entries.len() * COLUMN_MAP_BYTES)
            .and_then(|end| end.checked_add(DECLARATION_TERMINATOR_BYTES))
            .ok_or(InspectError::StructureLimit)?;
        if tail_end != raw_definition.len() {
            return Err(TableRenameError::Source);
        }
        let long_value_columns = schema
            .columns()
            .iter()
            .filter(|column| matches!(column.native_type(), data_type::LONG_TEXT | data_type::OLE))
            .count();
        if long_values.entries.len() != long_value_columns {
            return Err(TableRenameError::Source);
        }
        for declaration in long_values.entries {
            reader.work(schema.columns().len())?;
            if !schema.columns().iter().any(|column| {
                column.number() == declaration.number
                    && matches!(column.native_type(), data_type::LONG_TEXT | data_type::OLE)
            }) {
                return Err(TableRenameError::Source);
            }
            for (reference, role) in [
                (declaration.references.owned(), MapRole::Owned),
                (
                    declaration.references.candidate(),
                    MapRole::LongValueCandidate,
                ),
            ] {
                let storage = RepresentedMap::read_storage(&mut reader, reference, role)?;
                if storage.pages.contains(&identity.catalog_page()) {
                    return Err(TableRenameError::Source);
                }
                for page in storage.pages {
                    global.require_allocated(&mut reader, page)?;
                }
                // All catalog LVAL fields are absent except the independently
                // checked inline MSysDb properties. An allocated LVAL family or
                // stale external candidate needs a separate ownership profile.
                if !allocation::pages(&mut reader, reference)?.is_empty() {
                    return Err(TableRenameError::Source);
                }
            }
        }
        let owned_pages = allocation::pages(&mut reader, schema.layout.usage)?;
        for candidate in allocation::pages(&mut reader, free_map)? {
            reader.work(owned_pages.len())?;
            if !owned_pages.contains(&candidate) {
                return Err(TableRenameError::Source);
            }
        }
        let mut records = 0_u32;
        for number in owned_pages {
            global.require_allocated(&mut reader, number)?;
            let page = reader.page(number)?;
            if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
                || u32_at(&page, OWNER_OFFSET)? != schema.definition_page()
            {
                return Err(TableRenameError::Source);
            }
            let count = row_count(&page)?;
            for slot in 0..count {
                reader.tick()?;
                let (flags, _, _) = row_range(&page, slot)?;
                if flags & !OFFSET_MASK != 0 {
                    return Err(TableRenameError::Source);
                }
            }
            records = records
                .checked_add(count as u32)
                .ok_or(InspectError::StructureLimit)?;
        }
        if records != schema.declared_rows()
            || (origin.is_system()
                && !object.name().as_str().eq_ignore_ascii_case("MSysObjects")
                && records != 0)
        {
            return Err(TableRenameError::Dependency);
        }
    }
    Ok(())
}

pub(super) fn verify(
    before: &NativeImage,
    after: &[u8],
    identity: NativeTableRenameIdentity,
    old_name: &str,
    new_name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableRenameError> {
    let start = identity.catalog_page() as usize * PAGE_BYTES;
    let end = start + PAGE_BYTES;
    if before.as_bytes().len() != after.len() {
        return Err(TableRenameError::Preservation);
    }
    for (original, candidate) in [
        (&before.as_bytes()[..start], &after[..start]),
        (&before.as_bytes()[end..], &after[end..]),
    ] {
        for (original, candidate) in original
            .chunks(COMPARISON_CHUNK_BYTES)
            .zip(candidate.chunks(COMPARISON_CHUNK_BYTES))
        {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, original.len() as u64)
                .map_err(InspectError::from)?;
            if original != candidate {
                return Err(TableRenameError::Preservation);
            }
        }
    }
    let candidate = inspect_catalog(
        &mut Cursor::new(after),
        before.catalog().catalog().scope(),
        budget,
        cancelled,
    )?;
    if before.catalog().header() != candidate.header()
        || before.catalog().origins().len() != candidate.origins().len()
    {
        return Err(TableRenameError::Preservation);
    }
    for ((old, old_origin), (new, new_origin)) in before
        .catalog()
        .catalog()
        .objects()
        .iter()
        .zip(before.catalog().origins())
        .zip(
            candidate
                .catalog()
                .objects()
                .iter()
                .zip(candidate.origins()),
        )
    {
        check(budget, cancelled)?;
        if old.id() != new.id()
            || old.kind() != new.kind()
            || old.parent() != new.parent()
            || old_origin.native_id() != new_origin.native_id()
            || old_origin.parent_id() != new_origin.parent_id()
            || old_origin.type_code() != new_origin.type_code()
            || old_origin.flags() != new_origin.flags()
            || old_origin.page() != new_origin.page()
            || old_origin.slot() != new_origin.slot()
        {
            return Err(TableRenameError::Preservation);
        }
        let expected = if old.id() == identity.object_id() {
            new_name
        } else {
            old.name().as_str()
        };
        if new.name().as_str() != expected
            || (old.id() == identity.object_id() && old.name().as_str() != old_name)
        {
            return Err(TableRenameError::Preservation);
        }
    }
    Ok(())
}
