//! Inert native definition drafts and owned SQL qualification for existing queries.
use crate::{
    Error, QueryExpressionReview, Response, SavedQueryTarget,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
};
use tablecraft_access::{NativeSource, SavedRead};
use tablecraft_sql::{
    ParseLimits, parse_query,
    query::{Query, ReadBody, Statement},
    select::{Expression, Projection, SelectPredicate},
};
use thiserror::Error;

const MAX_CONSTANT_SQL_BYTES: usize = 64;
const MAX_LITERAL_BYTES: usize = 10;

/// A read-only eligibility observation, never authority to mutate or execute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuerySqlEligibility {
    /// Fresh native physical qualification admits the existing constant SELECT.
    QualifiedConstantSelect,
    /// The definition, storage, source or host is outside the current writer.
    Unavailable,
}

/// Source-bound canonical SQL for the current narrow profile, without save authority.
pub struct QuerySqlDraft {
    pub target: SavedQueryTarget,
    /// Present only after exact native literal and physical source qualification.
    pub sql: Option<String>,
    pub eligibility: QuerySqlEligibility,
}

/// The complete submitted SQL paired with the actual native expression review.
pub struct QuerySqlReview {
    pub expression: QueryExpressionReview,
    pub original_sql: String,
    pub proposed_sql: String,
}

/// Refusal before a complete SQL edit can acquire a native save capability.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum QuerySqlEditError {
    #[error("saved query SQL is outside the existing constant SELECT edit profile")]
    Unsupported,
    #[error("original saved query SQL no longer matches its definition")]
    OriginalChanged,
}

impl Document {
    pub(crate) fn inspect_query_sql_draft(
        &mut self,
        target: &SavedQueryTarget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        let saved = self.read_query_sql_target(target, cancelled)?;
        let query = saved.query();
        let mut output = Presentation::new(cancelled);
        let (sql, eligibility) = if let Some(value) = constant_literal(query) {
            let literal = literal_text(value, &mut output)?;
            let eligibility = qualify_source(self, target, &literal, cancelled)?;
            let sql = if eligibility == QuerySqlEligibility::QualifiedConstantSelect {
                // Native qualification established that formatting does not
                // normalize an opaque or noncanonical stored expression.
                output.allocation(1, MAX_CONSTANT_SQL_BYTES)?;
                Some(query.to_string())
            } else {
                None
            };
            (sql, eligibility)
        } else {
            (None, QuerySqlEligibility::Unavailable)
        };
        check(cancelled)?;
        Ok(Response::QuerySqlDraft {
            draft: QuerySqlDraft {
                target: copy_target(target, &mut output)?,
                sql,
                eligibility,
            },
        })
    }

    pub(crate) fn review_query_sql(
        &mut self,
        target: &SavedQueryTarget,
        original_sql: &str,
        proposed_sql: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        // Parse into TableCraft-owned syntax, never infer an edit from UI text
        // slicing or treat formatted SQL as native fragment storage.
        let original = parse_query(original_sql, ParseLimits::default())?;
        check(cancelled)?;
        let proposed = parse_query(proposed_sql, ParseLimits::default())?;
        check(cancelled)?;
        let saved = self.read_query_sql_target(target, cancelled)?;
        let actual = saved.query();
        if original.query() != actual {
            return Err(QuerySqlEditError::OriginalChanged.into());
        }
        constant_literal(actual).ok_or(QuerySqlEditError::Unsupported)?;
        constant_literal(proposed.query()).ok_or(QuerySqlEditError::Unsupported)?;
        let expected = original
            .canonical_integer_projection_spelling()
            .ok_or(QuerySqlEditError::Unsupported)?;
        let replacement = proposed
            .canonical_integer_projection_spelling()
            .ok_or(QuerySqlEditError::Unsupported)?;
        let mut output = Presentation::new(cancelled);
        let original_sql = output.text(original_sql)?;
        let proposed_sql = output.text(proposed_sql)?;
        // This requalifies physical storage and identity independently. The read
        // draft is deliberately not a token that can bypass fresh native review.
        let Response::QueryExpressionReviewed { review, capability } =
            self.review_query_literal_expression(target, expected, replacement, cancelled)?
        else {
            unreachable!("literal review returns only its owning review response");
        };
        Ok(Response::QuerySqlReviewed {
            review: QuerySqlReview {
                expression: review,
                original_sql,
                proposed_sql,
            },
            capability,
        })
    }

    fn read_query_sql_target(
        &mut self,
        target: &SavedQueryTarget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<SavedRead, Error> {
        check(cancelled)?;
        if target.document_revision != self.revision {
            return Err(Error::StaleSavedQuery);
        }
        let mut source = NativeSource::new(&mut self.source, SCOPE, budget(), cancelled)?;
        let saved = source.read_saved_query(&target.name, ParseLimits::default())?;
        let object = saved.definition().object();
        if object.id().as_u128() != target.object_id || object.name().as_str() != target.name {
            return Err(Error::StaleSavedQuery);
        }
        Ok(saved)
    }
}

fn constant_literal(query: &Query) -> Option<i64> {
    if !query.parameters.is_empty() {
        return None;
    }
    let Statement::Read(read) = &query.statement else {
        return None;
    };
    if !read.order_by.is_empty() || read.owner_access {
        return None;
    }
    let ReadBody::Select(select) = &read.body else {
        return None;
    };
    let [
        Projection::Expression {
            expression: Expression::Integer(value),
            alias: Some(alias),
        },
    ] = select.projection.as_slice()
    else {
        return None;
    };
    if !(0..=i64::from(i32::MAX)).contains(value) || alias.as_str() != "Answer" {
        return None;
    }
    if select.predicate != SelectPredicate::All
        || !select.from.is_empty()
        || !select.group_by.is_empty()
        || select.having.is_some()
        || select.into.is_some()
        || select.into_external.is_some()
        || select.external.is_some()
        || select.owner_access
        || select.selection.is_some()
        || !select.order_by.is_empty()
    {
        return None;
    }
    Some(*value)
}

fn literal_text(value: i64, output: &mut Presentation<'_>) -> Result<String, Error> {
    output.allocation(1, MAX_LITERAL_BYTES)?;
    Ok(value.to_string())
}

fn copy_target(
    target: &SavedQueryTarget,
    output: &mut Presentation<'_>,
) -> Result<SavedQueryTarget, Error> {
    Ok(SavedQueryTarget {
        document_revision: target.document_revision,
        object_id: target.object_id,
        name: output.text(&target.name)?,
    })
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
fn qualify_source(
    document: &Document,
    target: &SavedQueryTarget,
    literal: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<QuerySqlEligibility, Error> {
    use tablecraft_ace::{NativeImage, QueryExpressionError, RecoverableQueryExpressionError};

    if !document.source.editable_native() {
        return Ok(QuerySqlEligibility::Unavailable);
    }
    let mut work = budget();
    let image = NativeImage::capture_shared(
        document.edits.image_source.clone(),
        SCOPE,
        &mut work,
        cancelled,
    )?;
    match image.plan_query_literal_expression(&target.name, literal, literal, &mut work, cancelled)
    {
        Ok(_) => Ok(QuerySqlEligibility::QualifiedConstantSelect),
        Err(
            QueryExpressionError::Source
            | QueryExpressionError::Definition
            | QueryExpressionError::Literal
            | QueryExpressionError::Capacity,
        ) => Ok(QuerySqlEligibility::Unavailable),
        Err(error) => Err(RecoverableQueryExpressionError::from(error).into()),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn qualify_source(
    _document: &Document,
    _target: &SavedQueryTarget,
    _literal: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<QuerySqlEligibility, Error> {
    check(cancelled)?;
    Ok(QuerySqlEligibility::Unavailable)
}
