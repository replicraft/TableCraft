use super::{Error, retrieval::TextCollation};
use crate::native::{self, publication};
use std::path::Path;
use tablecraft_access::NativeEditor;
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::Collation;
use tablecraft_io::SharedFile;
use tablecraft_query::QueryError;
use tablecraft_sql::{
    query::{Query, Statement},
    select::TableSource,
};

const EDIT_BYTES: u64 = 256 * 1024 * 1024;
const EDIT_ROWS: u64 = 100_000;
const EDIT_STEPS: u64 = 100_000_000;

pub(super) fn run(
    query: &Query,
    input: &Path,
    output: &Path,
    collation: Option<TextCollation>,
) -> Result<String, Error> {
    run_with_publisher(query, input, output, collation, |output, bytes, budget| {
        publication::publish(output, bytes, budget, &|| false)
    })
}

fn run_with_publisher(
    query: &Query,
    input: &Path,
    output: &Path,
    collation: Option<TextCollation>,
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
    let Statement::Update(update) = &query.statement else {
        return Err(Error::Retrieval(QueryError::Unsupported));
    };
    let ([TableSource::Table(table)], [_]) =
        (update.sources.as_slice(), update.assignments.as_slice())
    else {
        return Err(Error::Retrieval(QueryError::Unsupported));
    };
    if !query.parameters.is_empty() {
        return Err(Error::Retrieval(QueryError::Unsupported));
    }

    let _span = tracing::debug_span!("cli.query.update").entered();
    let mut budget = ResourceBudget::new(EDIT_BYTES, EDIT_ROWS, EDIT_STEPS);
    let mut source = SharedFile::open(input).map_err(|error| Error::Native(error.into()))?;
    let image = NativeImage::capture(&mut source, native::CATALOG_SCOPE, &mut budget, &|| false)
        .map_err(Error::Native)?;
    let editor = NativeEditor::new(image, table.name.as_str(), &mut budget, &|| false)
        .map_err(Error::NativeEdit)?;
    let plan = tablecraft_query::plan_update(
        editor,
        query,
        &mut budget,
        collation.as_ref().map(|policy| policy as &dyn Collation),
        &|| false,
    )
    .map_err(Error::Update)?;

    let matched = plan.matched_rows();
    let native = plan.into_backend();
    let report = format!(
        "Matched rows\t{matched}\nChanged rows\t{}\n",
        native.changed_rows()
    );

    // The candidate is already owned and fully validated. Finish the fallible
    // source release before publishing a path, so an unlock failure cannot be
    // mistaken for a failed copy after a complete output became visible.
    source
        .close()
        .map_err(|error| Error::Native(error.into()))?;
    publish(output, native.after_bytes(), &mut budget).map_err(Error::Copy)?;
    tracing::debug!(
        matched_rows = matched,
        changed_rows = native.changed_rows(),
        "native output copy completed"
    );
    Ok(report)
}

#[cfg(test)]
mod tests;
