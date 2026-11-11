//! One actual SQL field declaration enters the shared consuming addition owner.
mod lowering;
use crate::{Error, Response, document::Document, editing::platform::field_addition::declared};
use tablecraft_ace::DeclaredFieldAdditionDefinition;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::query::definition::Column;

pub(super) fn review(
    document: &mut Document,
    table: &str,
    column: &Column,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let definition =
        DeclaredFieldAdditionDefinition::capture(lowering::capture(column)?, work, cancelled)?;
    declared::review(document, table, definition, work, cancelled)
}
