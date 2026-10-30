//! Borrow owned CREATE INDEX syntax into one existing native engineering request.
use super::{EngineeringCopy, EngineeringError, EngineeringRequest, preview};
use tablecraft_ace::{InspectError, NativeImage, SecondaryIndexCreate};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::query::{Query, Statement, definition::Definition};
use thiserror::Error;

/// Closed statement shape or typed native preparation failure, without SQL text.
#[derive(Debug, Error)]
pub enum CreateIndexError {
    /// Initial native creation supports one ordinary nonunique field only.
    #[error("engineering CREATE INDEX requires one field, no UNIQUE and no WITH policy")]
    Unsupported,
    /// Explicit lowering work budget or cancellation refusal.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Native schema, ownership, allocation, constraint or preservation refusal.
    #[error("engineering index preparation failed: {0}")]
    Engineering(#[from] EngineeringError),
}

/// Validate the constant-size owned AST spine without opening a source.
/// Names are borrowed unchanged: no scan, copying, SQL interpolation or lookup
/// occurs here. Native qualification owns their storage profile and byte work.
pub fn lower_create_index<'a>(
    query: &'a Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(&'a str, SecondaryIndexCreate<'a>), CreateIndexError> {
    if cancelled() {
        return Err(InspectError::Cancelled.into());
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)?;

    if !query.parameters.is_empty() {
        return Err(CreateIndexError::Unsupported);
    }
    let Statement::Definition(definition) = &query.statement else {
        return Err(CreateIndexError::Unsupported);
    };
    let Definition::CreateIndex(index) = definition.as_ref() else {
        return Err(CreateIndexError::Unsupported);
    };
    if index.unique || index.policy.is_some() {
        return Err(CreateIndexError::Unsupported);
    }
    let [field] = index.fields.as_slice() else {
        return Err(CreateIndexError::Unsupported);
    };

    Ok((
        index.table.as_str(),
        SecondaryIndexCreate::new(index.name.as_str(), field.name.as_str(), field.descending),
    ))
}

/// Lower and finish one local CREATE INDEX over the original captured image.
/// No row evaluator, fallback, normal save or required-macro permission is used.
pub fn preview_create_index(
    image: NativeImage,
    query: &Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<EngineeringCopy, CreateIndexError> {
    let (table, request) = lower_create_index(query, budget, cancelled)?;
    preview(
        image,
        EngineeringRequest::CreateSecondaryIndex {
            table,
            request: &request,
        },
        budget,
        cancelled,
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests;
