//! Lease-free complete-SQL review over one actual immutable native image.
use crate::{
    Error, QuerySqlEditError, QuerySqlEligibility, QuerySqlReview, Response, SavedQueryTarget,
    controls::{SCOPE, action_review_budget, check},
    display::Presentation,
    document::Document,
    editing::{EditError, QueryExpressionReopen, ReviewedQuerySql, platform::ids::issue_id},
};
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_access::{
    SavedQuerySqlEditError, SavedReadError, prepare_saved_query_sql_expression_edits,
};
use tablecraft_ace::{
    InspectError, NativeImage, QueryExpressionEditsError, QueryExpressionEditsIntentError,
    RecoverableNativeQueryExpressionEditsPlan,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::ObjectKind;
use tablecraft_sql::{ParseLimits, ParsedQuery, parse_query, select::Expression};

const SYNTAX_BYTES: u64 = 16 * 1024 * 1024;
const SYNTAX_METADATA_ROWS: u64 = 100_000;
const SYNTAX_STEPS: u64 = 1_000_000;
const PARSED_TEXT_COPIES: u64 = 2;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<QueryExpressionReopen>,
    pub(in crate::editing) plan: RecoverableNativeQueryExpressionEditsPlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

fn syntax_budget() -> ResourceBudget {
    // Metadata reconstruction has its own cumulative counters. These are not
    // query execution allowances or a counter reset for each native fragment.
    ResourceBudget::new(SYNTAX_BYTES, SYNTAX_METADATA_ROWS, SYNTAX_STEPS)
}

fn parsed(
    value: &str,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ParsedQuery, Error> {
    check(cancelled)?;
    let limits = ParseLimits::default();
    if value.len() > limits.max_bytes {
        return Err(Error::SqlLimit);
    }
    let tokens = limits.max_tokens.min(value.len());
    work.charge(Resource::Steps, value.len() as u64 + tokens as u64)?;
    work.charge_allocation(tokens as u64, size_of::<Expression>() as u64)?;
    work.charge_allocation(value.len() as u64, PARSED_TEXT_COPIES)?;
    let parsed = parse_query(value, limits)?;
    check(cancelled)?;
    Ok(parsed)
}

fn capture(
    document: &Document,
    target: &SavedQueryTarget,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeImage, Error> {
    check(cancelled)?;
    if target.document_revision != document.revision {
        return Err(Error::StaleSavedQuery);
    }
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }
    // NO_CLONE: NativeImage owns the checked source while Document keeps using it; this shares
    // immutable bytes instead of copying the database image.
    let image =
        NativeImage::capture_shared(document.edits.image_source.clone(), SCOPE, work, cancelled)?;
    let catalog = image.catalog().catalog();
    work.charge(
        Resource::Steps,
        catalog.objects().len() as u64 + target.name.len() as u64,
    )?;
    let object = catalog
        .lookup(&target.name, Some(ObjectKind::Query), None)
        .map_err(InspectError::from)?
        .ok_or(Error::StaleSavedQuery)?;
    if object.id().as_u128() != target.object_id || object.name().as_str() != target.name {
        return Err(Error::StaleSavedQuery);
    }
    Ok(image)
}

pub(in crate::editing) fn review(
    document: &mut Document,
    target: &SavedQueryTarget,
    original_sql: &str,
    proposed_sql: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.review_query_sql").entered();
    let mut native = action_review_budget(document.edits.original.len())?;
    let mut syntax = syntax_budget();
    let original = parsed(original_sql, &mut syntax, cancelled)?;
    let proposed = parsed(proposed_sql, &mut syntax, cancelled)?;
    let image = capture(document, target, &mut native, cancelled)?;
    let plan = prepare_saved_query_sql_expression_edits(
        image,
        &target.name,
        &original,
        &proposed,
        ParseLimits::default(),
        &mut native,
        &mut syntax,
        cancelled,
    )
    .map_err(QuerySqlEditError::from)?;

    let expected = super::reopen_image::capture(plan.after_bytes(), &mut native, cancelled)?;
    let mut output = Presentation::new(cancelled);
    let review = QuerySqlReview {
        id: issue_id()?,
        target: SavedQueryTarget {
            document_revision: target.document_revision,
            object_id: target.object_id,
            name: output.text(plan.query())?,
        },
        native_id: plan.native_id(),
        changed: plan.changed(),
        original_sql: output.text(original_sql)?,
        proposed_sql: output.text(proposed_sql)?,
    };
    native.charge_allocation(document.edits.path.as_os_str().len() as u64, 1)?;
    native.charge_allocation(1, size_of::<QueryExpressionReopen>() as u64)?;
    native.charge_allocation(1, size_of::<State>() as u64)?;
    let reopen = Arc::new(QueryExpressionReopen {
        // NO_CLONE: The capability or reopen witness outlives this borrowed Document call and
        // owns its source path; the live Document keeps its own path.
        path: document.edits.path.clone(),
        expected,
        target: SavedQueryTarget {
            document_revision: target.document_revision,
            object_id: target.object_id,
            name: output.text(plan.query())?,
        },
        native_id: plan.native_id(),
    });
    let capability = ReviewedQuerySql {
        state: Box::new(State {
            reopen,
            plan,
            valid: Arc::clone(&document.edits.valid),
        }),
    };
    check(cancelled)?;
    Ok(Response::QuerySqlReviewed { review, capability })
}

pub(in crate::editing) fn eligibility(
    document: &Document,
    target: &SavedQueryTarget,
    sql: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<QuerySqlEligibility, Error> {
    check(cancelled)?;
    if !document.source.editable_native() {
        return Ok(QuerySqlEligibility::Unavailable);
    }
    let mut native = action_review_budget(document.edits.original.len())?;
    let mut syntax = syntax_budget();
    let original = parsed(sql, &mut syntax, cancelled)?;
    let image = capture(document, target, &mut native, cancelled)?;
    match prepare_saved_query_sql_expression_edits(
        image,
        &target.name,
        &original,
        &original,
        ParseLimits::default(),
        &mut native,
        &mut syntax,
        cancelled,
    ) {
        Ok(_) => Ok(QuerySqlEligibility::QualifiedExpressionSelect),
        Err(error) if unsupported(&error) => Ok(QuerySqlEligibility::Unavailable),
        Err(error) => Err(QuerySqlEditError::from(error).into()),
    }
}

fn unsupported(error: &SavedQuerySqlEditError) -> bool {
    matches!(
        error,
        SavedQuerySqlEditError::Unsupported
            | SavedQuerySqlEditError::Roundtrip
            | SavedQuerySqlEditError::Metadata(SavedReadError::Unsupported)
            | SavedQuerySqlEditError::Native(InspectError::Unsupported)
            | SavedQuerySqlEditError::Intent(QueryExpressionEditsIntentError::Expression(
                QueryExpressionEditsError::Source
                    | QueryExpressionEditsError::Definition
                    | QueryExpressionEditsError::Inspect(InspectError::Unsupported)
            ))
            | SavedQuerySqlEditError::Intent(QueryExpressionEditsIntentError::Inspect(
                InspectError::Unsupported
            ))
    )
}
