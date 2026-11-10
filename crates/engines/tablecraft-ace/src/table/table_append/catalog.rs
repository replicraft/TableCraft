//! Actual live application ancestry, explicit name tracking and payload ownership.
use super::refused;
use crate::{
    EditError, FileFormat, InspectError, NativeImage, NativeSortOrder, NativeTableStorage,
    PayloadPresence, PropertyGroupKind, TableSchema,
    format::data_type,
    header::check,
    inspect_database_properties, inspect_table_schema,
    table::mutation::{
        catalog_property::ledger, lval_ownership, ordinary_content::schema_dependencies,
        schema_source,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::ObjectKind;

const SYSTEM_FLAGS: u32 = 0x8000_0002;
const ROLES: [&str; 8] = [
    "DatabaseRoot",
    "Tables",
    "Databases",
    "Relationships",
    "MSysObjects",
    "MSysRelationships",
    "MSysACEs",
    "MSysDb",
];
const NAME_OPTIONS: [&str; 2] = ["Track Name AutoCorrect Info", "Perform Name AutoCorrect"];

pub(super) struct Facts {
    pub schema: TableSchema,
    pub tables_parent: i32,
    pub name_order: NativeSortOrder,
}

pub(super) fn capture(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Facts, EditError> {
    let header = image.catalog().header();
    if header.format() != FileFormat::Ace2016 || header.subversion() != 0 || header.encoded_pages()
    {
        return Err(refused());
    }
    let role = |name: &str, budget: &mut ResourceBudget| {
        schema_source::find_role(image, name, budget, cancelled).map_err(|error| match error {
            schema_source::Error::Inspect(error) => EditError::Inspect(error),
            schema_source::Error::Source | schema_source::Error::Dependency => refused(),
        })
    };
    let root = role("DatabaseRoot", budget)?;
    let tables = role("Tables", budget)?;
    let databases = role("Databases", budget)?;
    let db = role("MSysDb", budget)?;
    for name in ROLES {
        let origin = role(name, budget)?;
        let (kind, parent) = match name {
            "DatabaseRoot" => (3, 0),
            "Tables" | "Databases" | "Relationships" => (3, root.native_id()),
            "MSysDb" => (2, databases.native_id()),
            _ => (1, tables.native_id()),
        };
        if !origin.is_system()
            || origin.flags() & !SYSTEM_FLAGS != 0
            || origin.type_code() != kind
            || origin.parent_id() != parent
            || (kind == 1 && origin.table_storage() != Some(NativeTableStorage::Local))
        {
            return Err(refused());
        }
    }
    for (object, origin) in image
        .catalog()
        .catalog()
        .objects()
        .iter()
        .zip(image.catalog().origins())
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, object.name().as_str().len() as u64)
            .map_err(InspectError::from)?;
        if origin.is_system() {
            if !ROLES.contains(&object.name().as_str()) {
                return Err(refused());
            }
        } else if object.kind() != ObjectKind::Table
            || origin.flags() != 0
            || origin.type_code() != 1
            || origin.parent_id() != tables.native_id()
            || origin.table_storage() != Some(NativeTableStorage::Local)
        {
            return Err(refused());
        }
        let payload = image.inspect_object_payloads(object.id(), budget, cancelled)?;
        if [payload.body(), payload.module(), payload.extra()]
            .into_iter()
            .any(|value| value != PayloadPresence::Absent)
            || (object.kind() == ObjectKind::Table
                && !image
                    .inspect_object_children(object.id(), budget, cancelled)?
                    .is_empty())
        {
            return Err(refused());
        }
    }
    let scope = image.catalog().catalog().scope();
    let properties = inspect_database_properties(&mut image.reader(), scope, budget, cancelled)?
        .ok_or_else(refused)?;
    if properties.object_id() != db.object_id() || properties.groups().len() != 1 {
        return Err(refused());
    }
    let group = &properties.groups()[0];
    if group.kind() != PropertyGroupKind::Table
        || !group.name().is_empty()
        || group.properties().len() != NAME_OPTIONS.len()
    {
        return Err(refused());
    }
    for name in NAME_OPTIONS {
        let mut found = None;
        for property in group.properties() {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    property.name().len().max(name.len()) as u64,
                )
                .map_err(InspectError::from)?;
            if property.name().eq_ignore_ascii_case(name) && found.replace(property).is_some() {
                return Err(refused());
            }
        }
        let property = found.ok_or_else(refused)?;
        if property.native_type() != data_type::INT32
            || property.flags() != 1
            || property.raw_value() != 0_i32.to_le_bytes()
        {
            return Err(refused());
        }
    }
    schema_dependencies::require_empty(image, budget, cancelled)?;
    let schema =
        inspect_table_schema(&mut image.reader(), scope, "MSysObjects", budget, cancelled)?;
    if schema.declared_rows() as usize != image.catalog().origins().len() {
        return Err(refused());
    }
    let name_order = schema
        .columns()
        .iter()
        .find(|column| column.name() == "Name")
        .and_then(|column| column.text_sort_order())
        .ok_or_else(refused)?;
    // The existing catalog/LVAL ledgers bind live forwarded roots and every
    // externally stored property body; raw retained records remain untouched.
    drop(ledger::capture_retained(
        image.as_bytes(),
        image.catalog(),
        budget,
        cancelled,
    )?);
    drop(lval_ownership::Inventory::capture_retained(
        image, budget, cancelled,
    )?);
    Ok(Facts {
        schema,
        tables_parent: tables.native_id(),
        name_order,
    })
}
