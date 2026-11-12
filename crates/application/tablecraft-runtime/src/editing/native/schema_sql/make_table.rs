//! Complete SELECT INTO preparation over the document's exact immutable image.
use super::super::table_creation::Request;
use crate::query::context::Environment;
use crate::{
    Error, Response,
    controls::{SCOPE, action_query_budget, action_review_budget, check, comparison_budget},
    display::Presentation,
    document::Document,
    editing::{EditError, table_creation::CreationOrigin},
    query::context::Context,
    query::parameters::{self, ParameterInput},
};
use std::cell::RefCell;
use tablecraft_access::editing::sql::{
    NativeMakeTableOptions, prepare_parameterized_native_make_table_with_native_comparison,
};
use tablecraft_ace::{NativeImage, TableSnapshotLimits};
use tablecraft_query::ExecutionOptions;
use tablecraft_sql::query::Query;

pub(in crate::editing) fn review(
    document: &mut Document,
    query: Query,
    inputs: &[ParameterInput],
    environment: Environment<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    check(cancelled)?;
    if !document.source.editable_native() {
        return Err(EditError::ReadOnlySource.into());
    }
    let mut output = Presentation::new(cancelled);
    parameters::validate_input_limits(inputs, &mut output)?;
    let bindings = parameters::bindings(&query, inputs, &mut output)?;
    let native = RefCell::new(action_review_budget(document.edits.original.len())?);
    let comparison = RefCell::new(comparison_budget());
    let mut query_work = action_query_budget();
    let image = {
        let mut work = native.borrow_mut();
        // NO_CLONE: The candidate owns the same immutable original allocation
        // independently from the still-open Document; no database bytes copy.
        NativeImage::capture_shared_retained(
            document.edits.image_source.clone(),
            SCOPE,
            &mut work,
            cancelled,
        )?
    };
    // The adapter constructs both its reader and native comparison resolver from
    // this image. No external Source or independently scoped field policy enters.
    let context = Context::new(environment);
    let prepared = prepare_parameterized_native_make_table_with_native_comparison(
        image,
        query,
        &native,
        &comparison,
        &mut query_work,
        NativeMakeTableOptions {
            absent_like_mode: Some(crate::query::like_mode::SQL_VIEW_DEFAULT),
            parameters: &bindings,
            execution: ExecutionOptions {
                cancelled: Some(cancelled),
                ..context.options()
            },
            snapshot_limits: TableSnapshotLimits::default(),
        },
    )?;
    let mut work = native.borrow_mut();
    let plan = prepared.finish(&mut work, cancelled)?;
    let expected = plan.after_source().shared_bytes();
    document.issue_table_creation(
        document.revision,
        CreationOrigin::Sql,
        Request::Populated(plan),
        expected,
        &mut work,
        cancelled,
    )
}
