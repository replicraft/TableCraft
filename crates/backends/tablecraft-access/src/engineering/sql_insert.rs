//! SQL VALUES composes a neutral complete-row plan with one engineering finish.
use super::{
    EngineeringCopy,
    inserter::{AccessEngineeringInserter, ScalarInsertMode},
};
use crate::editing::NativeEditError;
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_query::InsertError;
use tablecraft_sql::query::Query;

/// Bind and evaluate one local explicit VALUES row before one typed preview.
/// All companion field names must be present in incrementing mode. This does
/// not execute INSERT SELECT, UDFs, defaults, omitted values or durable writes.
pub fn preview_insert(
    image: NativeImage,
    query: &Query,
    mode: ScalarInsertMode,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<EngineeringCopy, InsertError<NativeEditError>> {
    let table = tablecraft_query::local_insert_table(
        query,
        mode == ScalarInsertMode::Incrementing,
        budget,
        cancelled,
    )?;
    let inserter = AccessEngineeringInserter::new(image, table, mode, budget, cancelled)
        .map_err(InsertError::Backend)?;
    tablecraft_query::plan_insert(inserter, query, budget, cancelled)
}
