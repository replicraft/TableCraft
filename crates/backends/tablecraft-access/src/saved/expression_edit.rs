//! Same-image SQL review into existing native expression slots, without execution.
mod diff;
mod render;
#[cfg(test)]
mod tests;

use super::{SavedReadError, context::Context, reconstruct_saved_read};
use crate::SavedRead;
use std::io::Cursor;
use tablecraft_ace::{
    InspectError, NativeImage, QueryExpressionEditsIntentError, QueryFragmentExpressionEdit,
    RecoverableNativeQueryExpressionEditsPlan, read_saved_query_definition,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{ParseLimits, ParsedQuery};
use thiserror::Error;

/// Refusal before complete SQL review acquires a native expression-save plan.
#[derive(Debug, Error)]
pub enum SavedQuerySqlEditError {
    /// Inert native source inspection, cancellation or resource exhaustion.
    #[error(transparent)]
    Native(#[from] InspectError),
    /// Bounded native-fragment reconstruction, syntax, allocation or cancellation.
    #[error(transparent)]
    Metadata(#[from] SavedReadError),
    /// The fresh native candidate or original-intent construction refused.
    #[error(transparent)]
    Intent(#[from] QueryExpressionEditsIntentError),
    /// Submitted original SQL no longer describes the same captured definition.
    #[error("original saved SQL no longer matches the captured definition")]
    OriginalChanged,
    /// The edit changes unsupported topology, aliases, sources or metadata.
    #[error("saved SQL edit is outside the existing expression-fragment profile")]
    Unsupported,
    /// Serialized changed leaves or the complete candidate differ from submitted SQL.
    #[error("saved SQL expression roundtrip does not preserve the submitted model")]
    Roundtrip,
}

/// Review complete SQL against actual fragments from this same immutable image.
///
/// Only existing projection/WHERE expression bodies change. All non-expression
/// syntax, aliases, parameter declarations, source topology and original native
/// text outside changed leaves remain unchanged. This does not bind names,
/// execute expressions, scan query data or open providers. ACE independently
/// performs its required native ownership/read checks under `native`; syntax
/// reconstruction, rendering and parse-back consume `syntax` separately.
/// The returned sealed plan uses TCQE0002 and the existing fixed-image journal;
/// this function does not open an exclusive target or apply/recover any bytes.
#[expect(
    clippy::too_many_arguments,
    reason = "Keep captured source, two SQL witnesses and independent native/syntax pools explicit."
)]
pub fn prepare_saved_query_sql_expression_edits(
    image: NativeImage,
    query_name: &str,
    original: &ParsedQuery,
    proposed: &ParsedQuery,
    limits: ParseLimits,
    native: &mut ResourceBudget,
    syntax: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeQueryExpressionEditsPlan, SavedQuerySqlEditError> {
    let _span = tracing::debug_span!("access.saved_query_sql.prepare_expression_edits").entered();
    let scope = image.catalog().catalog().scope();
    let actual = read_actual(&image, query_name, limits, native, syntax, cancelled)?;
    let mut context = Context::new(syntax, cancelled, limits);
    context.check()?;
    context
        .budget
        .charge(
            Resource::Steps,
            original.source().len() as u64
                + proposed.source().len() as u64
                + 2 * context.limits.max_tokens as u64,
        )
        .map_err(SavedReadError::from)?;
    if original.query() != actual.query() {
        return Err(SavedQuerySqlEditError::OriginalChanged);
    }

    let changes = diff::changes(&actual, proposed.query(), &mut context)?;
    let mut requests = context.reserve::<QueryFragmentExpressionEdit<'_>>(changes.len())?;
    for change in &changes {
        context.check()?;
        requests.push(change.request()?);
    }
    let plan =
        image.plan_recoverable_query_expression_edits(query_name, &requests, native, cancelled)?;

    // Reconstruct the entire native AFTER, including untouched spellings and
    // all non-expression fields. Leaf parse-back alone cannot prove this model.
    let definition = read_saved_query_definition(
        &mut Cursor::new(plan.after_bytes()),
        scope,
        plan.query(),
        native,
        cancelled,
    )?;
    let candidate = reconstruct_saved_read(definition, limits, context.budget, cancelled)?;
    context.check()?;
    context
        .budget
        .charge(
            Resource::Steps,
            proposed.source().len() as u64 + context.limits.max_tokens as u64,
        )
        .map_err(SavedReadError::from)?;
    if candidate.query() != proposed.query() {
        return Err(SavedQuerySqlEditError::Roundtrip);
    }
    Ok(plan)
}

fn read_actual(
    image: &NativeImage,
    name: &str,
    limits: ParseLimits,
    native: &mut ResourceBudget,
    syntax: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SavedRead, SavedQuerySqlEditError> {
    let definition = read_saved_query_definition(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        name,
        native,
        cancelled,
    )?;
    Ok(reconstruct_saved_read(
        definition, limits, syntax, cancelled,
    )?)
}
