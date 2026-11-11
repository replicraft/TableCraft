//! One actual SQL field declaration enters the shared consuming addition owner.
mod lowering;
use crate::{Error, Response, document::Document, editing::platform::field_addition::unadorned};
use tablecraft_ace::UnadornedFieldAdditionDefinition;
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
        UnadornedFieldAdditionDefinition::capture(lowering::capture(column)?, work, cancelled)?;
    unadorned::review(document, table, definition, work, cancelled)
}
