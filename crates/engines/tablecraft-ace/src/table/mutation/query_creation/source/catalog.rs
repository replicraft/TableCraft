//! Complete catalog body/ancestry qualification, separate from destination names.
use crate::{NativeImage, QueryCreationError, TableSchema, header::check, read_table};
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
        total
            .checked_add(object.name().as_str().len())
            .ok_or(QueryCreationError::Source)
    })?;
    let role_work = names
        .checked_mul(SYSTEM_ROLES.len())
        .ok_or(QueryCreationError::Source)?;
    budget.charge(Resource::Steps, role_work as u64)?;
    let root = role("DatabaseRoot")?;
    let tables = role("Tables")?;
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
        {
            return Err(QueryCreationError::Source);
        }
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
            {
                return Err(QueryCreationError::Source);
            }
        } else if origin.type_code() != 5
            || origin.flags() != 0
            || origin.parent_id() != tables.native_id()
        {
            return Err(QueryCreationError::Source);
        }
    }
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
    for row in snapshot.rows() {
        check(budget, cancelled)?;
        if row.len() != schema.columns().len() {
            return Err(QueryCreationError::Source);
        }
        for (column, value) in schema.columns().iter().zip(row) {
            check(budget, cancelled)?;
            // Only Name is a variable payload in this initial native catalog
            // language. Present-empty owner/cache/program/property bodies refuse.
            if !column.fixed() && column.name() != "Name" && *value != Value::Null {
                return Err(QueryCreationError::Source);
            }
        }
    }
    Ok(tables.native_id())
}
