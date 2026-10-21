use super::{Error, retrieval::TextCollation};
use crate::native;
use std::{fmt, fs::OpenOptions, io::ErrorKind, path::Path};
use tablecraft_access::NativeEditor;
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::comparison::Collation;
use tablecraft_io::{IoError, SharedFile, write_exact_at};
use tablecraft_query::QueryError;
use tablecraft_sql::{
    query::{Query, Statement},
    select::TableSource,
};

const EDIT_BYTES: u64 = 256 * 1024 * 1024;
const EDIT_ROWS: u64 = 100_000;
const EDIT_STEPS: u64 = 100_000_000;
const COPY_CHUNK_BYTES: usize = 64 * 1024;

pub(crate) enum CopyError {
    Create(ErrorKind),
    Write(IoError),
    Sync(ErrorKind),
}
impl fmt::Display for CopyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create(kind) => write!(f, "could not create a fresh database output ({kind:?})"),
            Self::Write(error) => {
                write!(f, "database copy failed; output may be incomplete: {error}")
            }
            Self::Sync(kind) => write!(
                f,
                "database copy sync failed; output is not qualified ({kind:?})"
            ),
        }
    }
}

pub(super) fn run(
    query: &Query,
    input: &Path,
    output: &Path,
    collation: Option<TextCollation>,
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
    budget
        .charge(Resource::Bytes, native.after_bytes().len() as u64)
        .map_err(|error| Error::Native(error.into()))?;
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| Error::Copy(CopyError::Create(error.kind())))?;
    for (number, chunk) in native.after_bytes().chunks(COPY_CHUNK_BYTES).enumerate() {
        write_exact_at(
            &mut destination,
            (number * COPY_CHUNK_BYTES) as u64,
            chunk,
            &mut budget,
            &|| false,
        )
        .map_err(|error| Error::Copy(CopyError::Write(error)))?;
    }
    destination
        .sync_all()
        .map_err(|error| Error::Copy(CopyError::Sync(error.kind())))?;
    source
        .close()
        .map_err(|error| Error::Native(error.into()))?;
    tracing::debug!(
        matched_rows = matched,
        changed_rows = native.changed_rows(),
        "native output copy completed"
    );
    Ok(report)
}
