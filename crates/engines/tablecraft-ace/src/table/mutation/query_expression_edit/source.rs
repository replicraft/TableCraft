//! V2-only placement/source policy after shared target-carrier qualification.
use crate::{
    IndexCheckStatus, NativeImage, NativeObjectOrigin, NativeTableStorage,
    QueryExpressionEditsError, TableSchema, header::check, inspect_table_schema,
    table::indexes::check_table_retained,
};
use tablecraft_core::limits::ResourceBudget;

const QUERY_VARIABLES: usize = 4;

pub(super) fn qualify(
    image: &NativeImage,
    origin: &NativeObjectOrigin,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSchema, QueryExpressionEditsError> {
    let qualified =
        super::super::query_expression_source::capture(image, origin, |_| true, budget, cancelled)
            .map_err(|error| match error {
                super::super::query_expression_source::SourceError::Inspect(error) => {
                    QueryExpressionEditsError::Inspect(error)
                }
                super::super::query_expression_source::SourceError::Source => {
                    QueryExpressionEditsError::Source
                }
            })?;
    let catalog = qualified.catalog;
    let schema = qualified.queries;
    let scope = image.catalog().catalog().scope();
    let expression = schema
        .columns()
        .iter()
        .find(|column| column.name().eq_ignore_ascii_case("Expression"))
        .ok_or(QueryExpressionEditsError::Source)?;
    if expression.fixed()
        || expression.native_type() != crate::format::data_type::LONG_TEXT
        || expression.field.extended_flags != 0
    {
        return Err(QueryExpressionEditsError::Source);
    }
    if expression.field.variable != QUERY_VARIABLES - 1 {
        return Err(QueryExpressionEditsError::Source);
    }
    for table in [&catalog, &schema] {
        let checked =
            check_table_retained(&mut image.reader(), scope, table.name(), budget, cancelled)?;
        if checked
            .indexes()
            .iter()
            .any(|index| index.status() != IndexCheckStatus::Checked)
        {
            return Err(QueryExpressionEditsError::Source);
        }
    }
    for other in image.catalog().catalog().objects() {
        check(budget, cancelled)?;
        if other.id() == schema.object_id() {
            continue;
        }
        let Some(other_origin) = image
            .catalog()
            .origins()
            .iter()
            .find(|candidate| candidate.object_id() == other.id())
        else {
            return Err(QueryExpressionEditsError::Source);
        };
        if other_origin.table_storage() != Some(NativeTableStorage::Local) {
            continue;
        }

        // Native IDs contain non-page bits. Resolve through the real schema
        // owner so its checked page normalization also applies to other objects.
        // Map census roles use physical definitions and can coalesce this alias;
        // they do not independently prove unique catalog-object ownership.
        let other_schema = inspect_table_schema(
            &mut image.reader(),
            scope,
            other.name().as_str(),
            budget,
            cancelled,
        )?;
        if other_schema.definition_page() == schema.definition_page() {
            return Err(QueryExpressionEditsError::Source);
        }
    }
    Ok(schema)
}
