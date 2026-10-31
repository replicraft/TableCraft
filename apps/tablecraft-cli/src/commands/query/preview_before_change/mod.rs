//! Explicit selected BeforeChange execution and one fresh engineering copy.
mod args;
mod report;
#[cfg(test)]
mod tests;

use super::{collation::TextCollation, guard_properties};
use crate::{
    commands::ExecutionError,
    native::{self, publication},
    output::OutputError,
};
pub(crate) use args::Arguments;
use std::{fmt, io::Write};
use tablecraft_access::{
    AccessInputError,
    engineering::{
        guards::before_change::{self, BeforeChangeBackendError},
        sql,
    },
};
use tablecraft_ace::{BeforeChangePreviewError, InspectError, NativeImage, inspect_header};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_sql::{ParseError, ParseLimits, parse_query};

pub(in crate::commands) enum Error {
    Parse(ParseError),
    Query(QueryError),
    Selection(guard_properties::Error),
    Source(AccessInputError),
    SourceRelease(AccessInputError),
    Native(InspectError),
    Update(UpdateError<BeforeChangeBackendError>),
    Output(OutputError),
    Publish(publication::Error),
    Encrypted,
    Deployment,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(formatter),
            Self::Query(error) => error.fmt(formatter),
            Self::Selection(error) => error.fmt(formatter),
            Self::Source(error) => error.fmt(formatter),
            Self::SourceRelease(error) => write!(
                formatter,
                "could not release source before publication: {error}"
            ),
            Self::Native(error) => error.fmt(formatter),
            Self::Update(UpdateError::Backend(BeforeChangeBackendError::Preview(
                BeforeChangePreviewError::Rejected(rejection),
            ))) => write!(
                formatter,
                "BeforeChange rejected the update (Error Number {}); no copy published",
                rejection.number()
            ),
            Self::Update(error) => error.fmt(formatter),
            Self::Output(error) => error.fmt(formatter),
            Self::Publish(error) => error.fmt(formatter),
            Self::Encrypted => formatter.write_str(
                "encrypted rewrite is unsupported; no plaintext BeforeChange copy is published",
            ),
            Self::Deployment => {
                formatter.write_str("deployment packages cannot be BeforeChange copy sources")
            }
        }
    }
}

pub(in crate::commands) fn execute(
    args: Arguments,
    _password: Option<&str>,
    writer: &mut dyn Write,
) -> Result<(), ExecutionError> {
    let text = match run_with_release(args, native::NativeFile::close) {
        Ok(text) => text,
        Err(Error::Publish(error)) => return Err(ExecutionError::EngineeringPublication(error)),
        Err(error) => return Err(ExecutionError::Operation(error.to_string())),
    };
    write_published(&text, writer)
}

fn write_published(text: &str, writer: &mut dyn Write) -> Result<(), ExecutionError> {
    writer
        .write_all(text.as_bytes())
        .and_then(|()| writer.flush())
        .map_err(ExecutionError::PublishedCopyOutput)
}

fn run_with_release(
    args: Arguments,
    release: impl FnOnce(native::NativeFile) -> Result<(), AccessInputError>,
) -> Result<String, Error> {
    let _span = tracing::debug_span!("cli.query.preview_before_change").entered();
    let mut budget = ResourceBudget::new(args.work_bytes, args.work_rows, args.work_steps);
    let selectors = guard_properties::selectors(&args.guard_properties, &mut budget)
        .map_err(Error::Selection)?;
    budget
        .charge_allocation(args.sql.len() as u64, 1)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    budget
        .charge(Resource::Steps, args.sql.len() as u64)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    let query = parse_query(&args.sql, ParseLimits::default()).map_err(Error::Parse)?;
    sql::local_update_table(query.query()).map_err(Error::Query)?;
    let collation = args
        .collation
        .map(|policy| TextCollation::from(policy).build())
        .transpose()
        .map_err(Error::Native)?;

    // Required selection is captured once, after closed preflight and before
    // source I/O. Publication itself occurs only after complete preparation.
    let output =
        publication::Output::capture(&args.output, args.durable_output, &mut budget, &|| false)
            .map_err(Error::Publish)?;
    let mut source = native::open_raw(&args.database, &mut budget).map_err(Error::Source)?;
    if source.is_deployment() {
        return Err(Error::Deployment);
    }
    if inspect_header(&mut source, &mut budget, &|| false)
        .map_err(Error::Native)?
        .encoded_pages()
    {
        return Err(Error::Encrypted);
    }
    let image = NativeImage::capture(&mut source, native::CATALOG_SCOPE, &mut budget, &|| false)
        .map_err(Error::Native)?;
    let plan = before_change::preview_update(
        image,
        query.query(),
        &selectors,
        &mut budget,
        collation.as_ref().map(|policy| policy as &dyn Collation),
        &|| false,
    )
    .map_err(Error::Update)?;
    let matched = plan.matched_rows();
    let candidate = plan.into_backend();
    let text = report::format(
        &candidate,
        matched,
        args.trace,
        args.durable_output,
        &mut budget,
    )?;

    publish_prepared(
        source,
        output,
        candidate.candidate_bytes(),
        text,
        &mut budget,
        release,
    )
}

// This boundary handles already prepared bytes only. Native qualification
// remains in the shared service above; tests of release order grant no candidate.
fn publish_prepared(
    source: native::NativeFile,
    output: publication::Output<'_>,
    bytes: &[u8],
    text: String,
    budget: &mut ResourceBudget,
    release: impl FnOnce(native::NativeFile) -> Result<(), AccessInputError>,
) -> Result<String, Error> {
    release(source).map_err(Error::SourceRelease)?;
    output
        .publish(bytes, budget, &|| false)
        .map_err(Error::Publish)?;
    Ok(text)
}
