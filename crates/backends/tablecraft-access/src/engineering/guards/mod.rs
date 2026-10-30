//! Explicit SQL-selected before-event preparation; no write candidate or publication.
mod deleting;
mod error;
mod inserting;
pub(super) mod summary;
mod updating;

pub use error::{GuardBackendError, GuardSqlError};
pub use summary::{GuardPreparationSummary, GuardPropertySummary, GuardRejectionSummary};

use super::{
    AccessEngineeringDeleter, AccessEngineeringEditor, AccessEngineeringInserter, DeleteMode,
    ScalarInsertMode, ScalarUpdateMode, UpdateMode, sql, sql_delete,
};
use tablecraft_ace::{BeforeEventGuardSelector, NativeImage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::QueryError;
use tablecraft_sql::query::{Query, Statement};

/// Validate one supported local statement shape before the host opens a source.
/// No schema, row, default, macro or external resource is inspected here.
pub fn local_guard_table<'q>(
    query: &'q Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'q str, QueryError> {
    match &query.statement {
        Statement::Update(_) => sql::local_update_table(query),
        Statement::Delete(_) => sql_delete::local_delete_table(query),
        Statement::Insert(_) => {
            tablecraft_query::local_insert_table(query, false, budget, cancelled)
        }
        _ => Err(QueryError::Unsupported),
    }
}

/// Bind existing SQL intents over one captured source, then finish once through
/// the engine guard service. This is neither a mutation preview nor a save plan.
/// INSERT is all-explicit, UPDATE is one column, and DELETE is source-local.
/// No defaults/generation/cascades or ambient expression context are installed.
pub fn prepare(
    image: NativeImage,
    query: &Query,
    selectors: &[BeforeEventGuardSelector<'_>],
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<GuardPreparationSummary, GuardSqlError> {
    let _span = tracing::debug_span!("access.engineering.before_guards.prepare").entered();
    let table = local_guard_table(query, budget, cancelled).map_err(GuardSqlError::Query)?;
    match &query.statement {
        Statement::Update(_) => {
            let editor = AccessEngineeringEditor::new(
                image,
                table,
                ScalarUpdateMode::Fixed(UpdateMode::Fitted),
                budget,
                cancelled,
            )
            .map_err(GuardBackendError::from)
            .map_err(GuardSqlError::Backend)?;
            tablecraft_query::plan_update(
                updating::GuardEditor {
                    inner: editor,
                    selectors,
                },
                query,
                budget,
                collation,
                cancelled,
            )
            .map(|plan| plan.into_backend())
            .map_err(GuardSqlError::Update)
        }
        Statement::Delete(_) => {
            let editor = AccessEngineeringDeleter::new(
                image,
                table,
                DeleteMode::Restrict,
                budget,
                cancelled,
            )
            .map_err(GuardBackendError::from)
            .map_err(GuardSqlError::Backend)?;
            tablecraft_query::plan_delete(
                deleting::GuardDeleter {
                    inner: editor,
                    selectors,
                },
                query,
                budget,
                collation,
                cancelled,
            )
            .map(|plan| plan.into_backend())
            .map_err(GuardSqlError::Delete)
        }
        Statement::Insert(_) => {
            let inserter = AccessEngineeringInserter::new(
                image,
                table,
                ScalarInsertMode::Explicit,
                budget,
                cancelled,
            )
            .map_err(GuardBackendError::from)
            .map_err(GuardSqlError::Backend)?;
            tablecraft_query::plan_insert(
                inserting::GuardInserter {
                    inner: inserter,
                    selectors,
                },
                query,
                budget,
                cancelled,
            )
            .map_err(GuardSqlError::Insert)
        }
        _ => Err(GuardSqlError::Query(QueryError::Unsupported)),
    }
}
