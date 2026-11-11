//! Metadata resolves original identities before the existing schema proof runs.
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    document::Document,
};
#[cfg(feature = "ace16-create")]
use tablecraft_access::editing::journal::NativeJournalEditor;
use tablecraft_ace::inspect_table_schema;
#[cfg(feature = "ace16-create")]
use tablecraft_sql::query::definition::AlterOperation;
use tablecraft_sql::query::definition::Definition;

pub(in crate::editing) fn review(
    document: &mut Document,
    definition: &Definition,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.review_schema_sql").entered();
    let mut work = budget();
    check(cancelled)?;
    match definition {
        Definition::DropTable(table) => {
            let schema = inspect_table_schema(
                &mut document.source,
                SCOPE,
                table.as_str(),
                &mut work,
                cancelled,
            )?;
            document.review_table_drop(document.revision, schema.name(), cancelled)
        }
        #[cfg(feature = "ace16-create")]
        Definition::AlterTable(table) => {
            let AlterOperation::DropColumn(column) = &table.operation else {
                return Err(Error::SchemaSqlUnsupported);
            };
            let schema = inspect_table_schema(
                &mut document.source,
                SCOPE,
                table.name.as_str(),
                &mut work,
                cancelled,
            )?;
            // NO_CLONE: Native proof observes the same immutable captured image while the
            // Document retains its reader. The shared source avoids a database payload copy.
            let image = NativeJournalEditor::capture_shared_image(
                document.edits.image_source.clone(),
                SCOPE,
                schema.name(),
                &mut work,
                cancelled,
            )?;
            let identity = if image.select_native_retained_non_tail_field_drop_source(
                schema.name(),
                column.as_str(),
                &mut work,
                cancelled,
            )? {
                image.inspect_retained_non_tail_field_drop_source(
                    schema.name(),
                    column.as_str(),
                    &mut work,
                    cancelled,
                )?
            } else {
                image.inspect_field_drop_source(
                    schema.name(),
                    column.as_str(),
                    &mut work,
                    cancelled,
                )?
            };
            // The engine resolves the written name under its qualified name policy.
            // Map that native identity back to stored spelling, never a display ordinal.
            let field = schema
                .columns()
                .iter()
                .find(|field| field.number() == identity.column_number())
                .ok_or(Error::StaleFieldDropTarget)?;
            document.review_field_drop(
                document.revision,
                schema.name(),
                field.name(),
                field.number(),
                cancelled,
            )
        }
        _ => Err(Error::SchemaSqlUnsupported),
    }
}
