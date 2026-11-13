//! Actual live application ancestry, explicit name tracking and payload ownership.
#[cfg(test)]
mod tests;
use super::refused;
use crate::{
    EditError, FileFormat, InspectError, NativeImage, NativeSortOrder, NativeTableStorage,
    TableSchema,
    header::check,
    inspect_table_schema,
    table::catalog_protection,
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

#[derive(Clone, Copy)]
enum PropertyObservation {
    Existing,
    Represented,
}

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
    capture_with(
        image,
        true,
        PropertyObservation::Existing,
        budget,
        cancelled,
    )
}

pub(super) fn capture_packed(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Facts, EditError> {
    capture_with(
        image,
        false,
        PropertyObservation::Represented,
        budget,
        cancelled,
    )
}

pub(super) fn capture_represented(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Facts, EditError> {
    capture_with(
        image,
        true,
        PropertyObservation::Represented,
        budget,
        cancelled,
    )
}

fn capture_with(
    image: &NativeImage,
    retained: bool,
    observation: PropertyObservation,
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
    let db = if retained {
        Some(role("MSysDb", budget)?)
    } else {
        None
    };
    let roles = if retained {
        &ROLES[..]
    } else {
        &ROLES[..ROLES.len() - 1]
    };
    for &name in roles {
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
            if !roles.contains(&object.name().as_str()) {
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
        catalog_protection::require_object(image, object, budget, cancelled)?;
    }
    let scope = image.catalog().catalog().scope();
    catalog_protection::require_database_properties(image, db, budget, cancelled)?;
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
    let properties = match observation {
        PropertyObservation::Existing => {
            ledger::capture_retained(image.as_bytes(), image.catalog(), budget, cancelled)?
        }
        PropertyObservation::Represented => {
            ledger::capture_represented(image.as_bytes(), image.catalog(), budget, cancelled)?
        }
    };
    drop(properties);
    drop(lval_ownership::Inventory::capture_retained(
        image, budget, cancelled,
    )?);
    Ok(Facts {
        schema,
        tables_parent: tables.native_id(),
        name_order,
    })
}
