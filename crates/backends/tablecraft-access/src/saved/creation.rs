//! Owned constant SELECT syntax into a distinct native QueryDef original.
#[cfg(test)]
mod tests;

use super::{SavedReadError, context::Context, reconstruct_saved_read, render};
use std::io::Cursor;
use tablecraft_ace::{
    InspectError, NativeImage, QueryAttribute, QueryCreationDefinition, QueryCreationIntentError,
    RecoverableNativeQueryCreationPlan, read_saved_query_definition,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::LegacyDateTime,
};
use tablecraft_sql::{
    ParseLimits,
    query::{Query, ReadBody, Statement},
    select::{Expression, Projection, SelectPredicate},
};
use thiserror::Error;

/// Explicit new-object facts and bounded SQL formatting policy.
///
/// The caller captures the date once. This adapter never reads a clock or
/// derives native identities, query results or timestamps from existing rows.
#[derive(Clone, Copy, Debug)]
pub struct SavedQueryCreationOptions<'a> {
    /// Exact requested destination name, validated by the native owner.
    pub destination: &'a str,
    /// Exact DATE fact for both catalog creation/update fields.
    pub captured_at: LegacyDateTime,
    /// SQL-owner parse/format bounds; cumulative work uses the separate syntax pool.
    pub limits: ParseLimits,
}

/// Failure before native query creation acquires any durable resources.
#[derive(Debug, Error)]
pub enum SavedQueryCreationError {
    /// Fresh native saved-definition observation, resource or cancellation error.
    #[error(transparent)]
    Native(#[from] InspectError),
    /// Bounded syntax formatting, reconstruction, allocation or cancellation error.
    #[error(transparent)]
    Metadata(#[from] SavedReadError),
    /// Native declaration, original binding, source, identity or full-image proof.
    #[error(transparent)]
    Intent(#[from] QueryCreationIntentError),
    /// The owned syntax is outside this constant-SELECT creation declaration.
    #[error("query is outside the native constant SELECT creation profile")]
    Unsupported,
    /// The complete freshly reconstructed native model differs from the request.
    #[error("created query does not retain the supplied owned model")]
    Roundtrip,
}

impl From<render::Error> for SavedQueryCreationError {
    fn from(error: render::Error) -> Self {
        match error {
            render::Error::Metadata(error) => Self::Metadata(error),
            render::Error::Unsupported => Self::Unsupported,
            render::Error::Roundtrip => Self::Roundtrip,
        }
    }
}

/// Construct a distinct native query original from an already-owned SQL model.
///
/// This first profile accepts one signed i32 integer projection with an explicit
/// alias and no parameters, sources, clauses, modifiers or external selectors.
/// SQL callers pass their parsed Query; Save As callers can pass a real stored
/// definition's reconstructed Query from the same captured image. No AST is
/// cloned, expression evaluated, source expanded or durable namespace created.
/// Native construction and syntax/readback spend separate cumulative pools.
///
/// # Errors
/// Refuses unsupported syntax/native sources, mismatching complete readback,
/// cancellation, fallible allocation and caller-selected resource exhaustion.
/// The returned plan is not an applied save or permission to publish a QueryDef.
pub fn prepare_saved_query_creation(
    image: NativeImage,
    query: &Query,
    options: SavedQueryCreationOptions<'_>,
    native: &mut ResourceBudget,
    syntax: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeQueryCreationPlan, SavedQueryCreationError> {
    let _span = tracing::debug_span!("access.saved_query.prepare_creation").entered();
    let mut context = Context::new(syntax, cancelled, options.limits);
    context.check()?;
    let (projection, alias, value) = constant(query)?;
    context
        .budget
        .charge(Resource::Steps, alias.len() as u64 + 1)
        .map_err(SavedReadError::from)?;
    let body = render::projection(projection, &mut context)?;

    let definition = QueryCreationDefinition::capture(
        options.destination,
        alias,
        value,
        options.captured_at,
        native,
        cancelled,
    )
    .map_err(QueryCreationIntentError::from)?;
    let scope = image.catalog().catalog().scope();
    let plan = image.plan_recoverable_query_creation(definition, native, cancelled)?;
    context.check()?;
    let observed = read_saved_query_definition(
        &mut Cursor::new(plan.after_bytes()),
        scope,
        plan.intent().name(),
        native,
        cancelled,
    )?;
    let actual = reconstruct_saved_read(observed, options.limits, context.budget, cancelled)?;
    context.check()?;
    context
        .budget
        .charge(
            Resource::Steps,
            (alias.len() + body.len() + actual.definition().rows().len()) as u64 + 1,
        )
        .map_err(SavedReadError::from)?;
    let column = actual
        .definition()
        .rows()
        .iter()
        .find(|row| row.attribute() == Some(QueryAttribute::Column));
    if actual.query() != query
        || column.and_then(|row| row.expression()) != Some(body.as_str())
        || actual.definition().origin().native_id() != plan.native_query_id()
    {
        return Err(SavedQueryCreationError::Roundtrip);
    }
    Ok(plan)
}

fn constant(query: &Query) -> Result<(&Projection, &str, i32), SavedQueryCreationError> {
    if !query.parameters.is_empty() {
        return Err(SavedQueryCreationError::Unsupported);
    }
    let Statement::Read(read) = &query.statement else {
        return Err(SavedQueryCreationError::Unsupported);
    };
    let ReadBody::Select(select) = &read.body else {
        return Err(SavedQueryCreationError::Unsupported);
    };
    if !read.order_by.is_empty()
        || read.owner_access
        || select.predicate != SelectPredicate::All
        || !select.from.is_empty()
        || select.selection.is_some()
        || !select.group_by.is_empty()
        || select.having.is_some()
        || !select.order_by.is_empty()
        || select.owner_access
        || select.into.is_some()
        || select.into_external.is_some()
        || select.external.is_some()
    {
        return Err(SavedQueryCreationError::Unsupported);
    }
    let [
        projection @ Projection::Expression {
            expression: Expression::Integer(value),
            alias: Some(alias),
        },
    ] = select.projection.as_slice()
    else {
        return Err(SavedQueryCreationError::Unsupported);
    };
    let value = i32::try_from(*value).map_err(|_| SavedQueryCreationError::Unsupported)?;
    Ok((projection, alias.as_str(), value))
}
