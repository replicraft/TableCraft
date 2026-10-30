use super::{Error, retrieval::TextCollation};
use crate::native::{self, publication};
use std::path::Path;
use tablecraft_access::editing::sql;
use tablecraft_ace::{InspectError, NativeImage};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::comparison::Collation;
use tablecraft_io::SharedFile;
use tablecraft_query::QueryError;
use tablecraft_sql::query::Query;

const EDIT_BYTES: u64 = 256 * 1024 * 1024;
const EDIT_ROWS: u64 = 100_000;
const EDIT_STEPS: u64 = 100_000_000;

pub(super) fn run(
    query: &Query,
    input: &Path,
    output: &Path,
    collation: Option<TextCollation>,
    durable_output: bool,
) -> Result<String, Error> {
    if durable_output {
        run_selected(
            query,
            input,
            output,
            collation,
            true,
            |output, bytes, budget| publication::publish(output, bytes, budget, &|| false),
        )
    } else {
        run_with_publisher(query, input, output, collation, |output, bytes, budget| {
            publication::publish(output, bytes, budget, &|| false)
        })
    }
}

fn run_with_publisher(
    query: &Query,
    input: &Path,
    output: &Path,
    collation: Option<TextCollation>,
    publish: impl FnOnce(&Path, &[u8], &mut ResourceBudget) -> Result<(), publication::Error>,
) -> Result<String, Error> {
    run_selected(query, input, output, collation, false, publish)
}

fn run_selected(
    query: &Query,
    input: &Path,
    output: &Path,
    collation: Option<TextCollation>,
    durable_output: bool,
    publish: impl FnOnce(&Path, &[u8], &mut ResourceBudget) -> Result<(), publication::Error>,
) -> Result<String, Error> {
    // Provisional General policies are read/query-only until native editing
    // semantics are extended explicitly. Reject before opening or creating files.
    if collation.is_some_and(|policy| !matches!(policy, TextCollation::Ordinal)) {
        return Err(Error::Retrieval(QueryError::Unsupported));
    }
    let collation = collation
        .map(TextCollation::build)
        .transpose()
        .map_err(Error::Native)?;
    sql::local_update_table(query).map_err(Error::Retrieval)?;

    let _span = tracing::debug_span!("cli.query.update").entered();
    let mut budget = ResourceBudget::new(EDIT_BYTES, EDIT_ROWS, EDIT_STEPS);
    let required = if durable_output {
        Some(
            publication::Output::capture(output, true, &mut budget, &|| false)
                .map_err(Error::Copy)?,
        )
    } else {
        None
    };
    let mut source = SharedFile::open(input).map_err(|error| Error::Native(error.into()))?;
    let image = NativeImage::capture(&mut source, native::CATALOG_SCOPE, &mut budget, &|| false)
        .map_err(Error::Native)?;
    let plan = sql::plan_update(
        image,
        query,
        &mut budget,
        collation.as_ref().map(|policy| policy as &dyn Collation),
        &|| false,
    )
    .map_err(|error| match error {
        sql::Error::Shape(error) => Error::Retrieval(error),
        sql::Error::Editor(error) => Error::NativeEdit(error),
        sql::Error::Plan(error) => Error::Update(error),
    })?;

    let matched = plan.matched_rows();
    let native = plan.into_backend();
    let mut report = format!(
        "Matched rows\t{matched}\nChanged rows\t{}\n",
        native.changed_rows()
    );
    if durable_output {
        let length = publication::REQUIRED_REPORT.len();
        budget
            .charge_allocation(length as u64, 1)
            .map_err(InspectError::from)
            .map_err(Error::Native)?;
        budget
            .charge(Resource::Steps, length as u64)
            .map_err(InspectError::from)
            .map_err(Error::Native)?;
        report
            .try_reserve_exact(length)
            .map_err(|_| Error::Native(InspectError::Allocation))?;
        report.push_str(publication::REQUIRED_REPORT);
    }

    // The candidate is already owned and fully validated. Finish the fallible
    // source release before publishing a path, so an unlock failure cannot be
    // mistaken for a failed copy after a complete output became visible.
    source
        .close()
        .map_err(|error| Error::Native(error.into()))?;
    if let Some(required) = required {
        required
            .publish(native.after_bytes(), &mut budget, &|| false)
            .map_err(Error::Copy)?;
    } else {
        publish(output, native.after_bytes(), &mut budget).map_err(Error::Copy)?;
    }
    tracing::debug!(
        matched_rows = matched,
        changed_rows = native.changed_rows(),
        "native output copy completed"
    );
    Ok(report)
}

#[cfg(test)]
mod tests;
