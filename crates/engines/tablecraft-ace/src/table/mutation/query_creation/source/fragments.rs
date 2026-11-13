//! Every live fragment resolves to a genuine current native query identity.
use crate::{
    NativeImage, QueryAttribute, QueryCreationError, TableSchema, header::check, read_table,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) fn qualify(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryCreationError> {
    let snapshot = read_table(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    if u32::try_from(snapshot.rows().len()).ok() != Some(schema.declared_rows()) {
        return Err(QueryCreationError::Source);
    }
    budget.charge(Resource::Steps, (schema.columns().len() * 2) as u64)?;
    let position = |name| {
        schema
            .columns()
            .iter()
            .position(|column| column.name() == name)
            .ok_or(QueryCreationError::Source)
    };
    let object = position("ObjectId")?;
    let attribute = position("Attribute")?;
    for row in snapshot.rows() {
        check(budget, cancelled)?;
        let Some(Value::Int32(id)) = row.get(object) else {
            return Err(QueryCreationError::Source);
        };
        budget.charge(Resource::Steps, image.catalog().origins().len() as u64)?;
        if !image
            .catalog()
            .origins()
            .iter()
            .any(|origin| origin.native_id() == *id && origin.type_code() == 5)
        {
            return Err(QueryCreationError::Source);
        }
        let Some(Value::Byte(code)) = row.get(attribute) else {
            return Err(QueryCreationError::Source);
        };
        if matches!(QueryAttribute::from_code(*code), QueryAttribute::Unknown(_)) {
            return Err(QueryCreationError::Source);
        }
    }
    // A catalog Query with no fragments is not silently treated as an inert
    // complete declaration. Ownership must exist in both directions.
    for origin in image
        .catalog()
        .origins()
        .iter()
        .filter(|origin| origin.type_code() == 5)
    {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, snapshot.rows().len() as u64)?;
        if !snapshot
            .rows()
            .iter()
            .any(|row| row.get(object) == Some(&Value::Int32(origin.native_id())))
        {
            return Err(QueryCreationError::Source);
        }
    }
    Ok(())
}
