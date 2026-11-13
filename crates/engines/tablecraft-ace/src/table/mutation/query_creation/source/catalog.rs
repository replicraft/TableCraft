//! Complete QueryDef append cohort, with protected ordinary local tables.
use crate::{
    NativeImage, NativeTableStorage, QueryCreationError, TableSchema,
    header::check,
    properties::database as database_properties,
    read_table,
    table::{catalog_protection, mutation::catalog_property::ledger},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const SYSTEM_FLAGS: u32 = 0x8000_0002;
const SYSTEM_ROLES: [(&str, i16); 8] = [
    ("DatabaseRoot", 3),
    ("Tables", 3),
    ("Databases", 3),
    ("Relationships", 3),
    ("MSysObjects", 1),
    ("MSysRelationships", 1),
    ("MSysACEs", 1),
    ("MSysQueries", 1),
];

pub(super) fn qualify(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<i32, QueryCreationError> {
    let objects = image.catalog().catalog().objects();
    let origins = image.catalog().origins();
    if objects.len() != origins.len()
        || usize::try_from(schema.declared_rows()).ok() != Some(objects.len())
    {
        return Err(QueryCreationError::Source);
    }
    let role = |name: &str| {
        objects
            .iter()
            .position(|object| object.name().as_str() == name)
            .and_then(|position| origins.get(position))
            .ok_or(QueryCreationError::Source)
    };
    let names = objects.iter().try_fold(0_usize, |total, object| {
        check(budget, cancelled)?;
        total
            .checked_add(object.name().as_str().len())
            .ok_or(QueryCreationError::Source)
    })?;
    // Base roles and the three selected containers traverse the catalog names;
    // the database role owner separately charges its own containment lookup.
    let role_work = names
        .checked_mul(SYSTEM_ROLES.len() + 3)
        .ok_or(QueryCreationError::Source)?;
    budget.charge(Resource::Steps, role_work as u64)?;
    let root = role("DatabaseRoot")?;
    let tables = role("Tables")?;
    let databases = role("Databases")?;
    for &(name, kind) in &SYSTEM_ROLES {
        check(budget, cancelled)?;
        let origin = role(name)?;
        let parent = if name == "DatabaseRoot" {
            0
        } else if kind == 3 {
            root.native_id()
        } else {
            tables.native_id()
        };
        if origin.type_code() != kind
            || origin.parent_id() != parent
            || !origin.is_system()
            || origin.flags() & !SYSTEM_FLAGS != 0
            || (kind == 1 && origin.table_storage() != Some(NativeTableStorage::Local))
        {
            return Err(QueryCreationError::Source);
        }
    }
    let database = database_properties::role(image.catalog(), budget, cancelled)?;
    if database.is_some_and(|origin| {
        !origin.is_system()
            || origin.type_code() != 2
            || origin.parent_id() != databases.native_id()
            || origin.flags() & !SYSTEM_FLAGS != 0
    }) {
        return Err(QueryCreationError::Source);
    }
    for (object, origin) in objects.iter().zip(origins) {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            (object.name().as_str().len() * SYSTEM_ROLES.len()) as u64,
        )?;
        if origin.is_system() {
            if !SYSTEM_ROLES
                .iter()
                .any(|(name, _)| *name == object.name().as_str())
                && database.is_none_or(|database| database.object_id() != origin.object_id())
            {
                return Err(QueryCreationError::Source);
            }
        } else {
            let ordinary = origin.type_code() == 1
                && origin.table_storage() == Some(NativeTableStorage::Local);
            if (!ordinary && origin.type_code() != 5)
                || origin.flags() != 0
                || origin.parent_id() != tables.native_id()
            {
                return Err(QueryCreationError::Source);
            }
        }
        catalog_protection::require_object(image, object, budget, cancelled)
            .map_err(super::edit_error)?;
    }
    catalog_protection::require_database_properties(image, database, budget, cancelled)
        .map_err(super::edit_error)?;
    // The existing ledger binds all real LvProp descriptors, MR2 bytes and LVAL
    // owners. It grants no data-write/default/constraint or program authority.
    drop(
        ledger::capture_retained(image.as_bytes(), image.catalog(), budget, cancelled)
            .map_err(super::edit_error)?,
    );

    let snapshot = read_table(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    if snapshot.rows().len() != origins.len() {
        return Err(QueryCreationError::Source);
    }
    budget.charge(Resource::Steps, schema.columns().len() as u64)?;
    let id = schema
        .columns()
        .iter()
        .position(|column| column.name() == "Id")
        .ok_or(QueryCreationError::Source)?;
    for row in snapshot.rows() {
        check(budget, cancelled)?;
        if row.len() != schema.columns().len() {
            return Err(QueryCreationError::Source);
        }
        let Some(Value::Int32(id)) = row.get(id) else {
            return Err(QueryCreationError::Source);
        };
        budget.charge(Resource::Steps, origins.len() as u64)?;
        let origin = origins
            .iter()
            .find(|origin| origin.native_id() == *id)
            .ok_or(QueryCreationError::Source)?;
        let properties = !origin.is_system() && origin.type_code() == 1
            || database.is_some_and(|database| database.object_id() == origin.object_id());
        for (column, value) in schema.columns().iter().zip(row) {
            check(budget, cancelled)?;
            // Only the separately checked property owner may retain LvProp.
            // Query/module/body/foreign payloads never become inert by omission.
            let protected_property = properties && column.name() == "LvProp";
            if !column.fixed()
                && column.name() != "Name"
                && !protected_property
                && *value != Value::Null
            {
                return Err(QueryCreationError::Source);
            }
        }
    }
    Ok(tables.native_id())
}
