//! Closed native application role graph and explicit name-tracking settings.
use crate::{
    DatabaseProperties, InspectError, NativeImage, NativeObjectOrigin, NativeTableStorage,
    PropertyGroupKind, TableSchema,
    format::{data_type, page::PAGE_NUMBER_MASK, reader::Reader},
    header::check,
    inspect_database_properties,
    table::{long_value::trace, record_layout},
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::ObjectKind;
use thiserror::Error;

const SYSTEM_FLAGS: u32 = 0x8000_0002;
const CONTAINER_TYPE: i16 = 3;
const DATABASE_TYPE: i16 = 2;
const DDL_FLAG: u8 = 1;
const TRACK_NAMES: &str = "Track Name AutoCorrect Info";
const PERFORM_NAMES: &str = "Perform Name AutoCorrect";
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

#[derive(Debug, Error)]
pub(in crate::table::mutation) enum Error {
    #[error(transparent)]
    Inspect(#[from] InspectError),
    #[error("native schema source is outside the supported role graph")]
    Source,
    #[error("native schema source has unsupported dependencies")]
    Dependency,
}

pub(in crate::table::mutation) struct Graph<'image> {
    pub user: &'image NativeObjectOrigin,
    pub database: &'image NativeObjectOrigin,
    pub properties: DatabaseProperties,
}

pub(in crate::table::mutation) fn observe<'image>(
    image: &'image NativeImage,
    requested: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Graph<'image>, Error> {
    check(budget, cancelled)?;
    let inventory = image.catalog();
    let objects = inventory.catalog().objects();
    let origins = inventory.origins();
    if objects.len() != SYSTEM_ROLES.len() + 1 {
        return Err(Error::Dependency);
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
            return Err(Error::Source);
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
            return Err(Error::Source);
        }
    }
    let db = find_role(image, "MSysDb", budget, cancelled)?;
    if db.type_code() != DATABASE_TYPE
        || !db.is_system()
        || db.flags() & !SYSTEM_FLAGS != 0
        || db.parent_id() != databases.native_id()
    {
        return Err(Error::Dependency);
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
                return Err(Error::Dependency);
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
            return Err(Error::Dependency);
        }
    }
    let (object, origin) = user.ok_or(Error::Source)?;
    budget
        .charge(
            Resource::Steps,
            (requested.len() + object.name().as_str().len()) as u64,
        )
        .map_err(InspectError::from)?;
    if !object.name().as_str().eq_ignore_ascii_case(requested) {
        return Err(Error::Source);
    }
    let definition_page = u32::try_from(origin.native_id()).map_err(|_| Error::Source)?;
    if definition_page == 0 || definition_page & !PAGE_NUMBER_MASK != 0 {
        return Err(Error::Source);
    }

    let scope = inventory.catalog().scope();
    let properties = inspect_database_properties(&mut image.reader(), scope, budget, cancelled)?
        .ok_or(Error::Dependency)?;
    if properties.object_id() != db.object_id() || properties.groups().len() != 1 {
        return Err(Error::Dependency);
    }
    let group = &properties.groups()[0];
    if group.kind() != PropertyGroupKind::Table
        || !group.name().is_empty()
        || group.properties().len() != 2
    {
        return Err(Error::Dependency);
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
        let property = matching.next().ok_or(Error::Dependency)?;
        if matching.next().is_some()
            || property.native_type() != data_type::INT32
            || property.flags() != DDL_FLAG
            || property.raw_value() != 0_i32.to_le_bytes()
        {
            return Err(Error::Dependency);
        }
    }

    Ok(Graph {
        user: origin,
        database: db,
        properties,
    })
}

pub(in crate::table::mutation) fn find_role<'a>(
    image: &'a NativeImage,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a NativeObjectOrigin, Error> {
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
            return Err(Error::Source);
        }
    }
    matched.ok_or(Error::Source)
}

/// Require every live variable payload to be the name or the exact supplied
/// external property descriptor. This excludes code/provider/dependency cells;
/// callers separately establish the referenced body's framing and ownership.
pub(in crate::table::mutation) fn require_external_payload(
    image: &NativeImage,
    schema: &TableSchema,
    origin: &NativeObjectOrigin,
    expected: Option<(u32, usize)>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(budget, cancelled)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let page = reader.page(origin.page())?;
    // The catalog origin is a logical root; ordinary field-history records can
    // be forwarded. Resolve its checked physical owner before inspecting cells.
    let record = reader.row_location(
        origin.page(),
        origin.slot(),
        schema.definition_page(),
        &page,
    )?;
    let row = record.data.as_slice();
    let framed = record_layout::parse(row, schema, reader.budget)?;
    if framed.count != schema.layout.max_columns
        || framed.offsets.len() != schema.layout.variable_slots + 1
    {
        return Err(Error::Source);
    }
    let mut seen = expected.is_none();
    for column in schema.columns() {
        reader.tick()?;
        let field = column.physical_field();
        if field.fixed
            || row[framed.mask_start + field.number / u8::BITS as usize]
                & (1 << (field.number % u8::BITS as usize))
                == 0
            || column.name().eq_ignore_ascii_case("Name")
        {
            continue;
        }
        if !column.name().eq_ignore_ascii_case("LvProp") {
            return Err(Error::Dependency);
        }
        let (reference, length) = expected.ok_or(Error::Dependency)?;
        let value = &row[framed.offsets[field.variable]..framed.offsets[field.variable + 1]];
        reader.work(value.len())?;
        if seen || !trace::canonical_single(value, reference, length)? {
            return Err(Error::Dependency);
        }
        seen = true;
    }
    if !seen {
        return Err(Error::Dependency);
    }
    Ok(())
}
