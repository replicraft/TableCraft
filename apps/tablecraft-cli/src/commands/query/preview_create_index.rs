//! One explicit schema copy; stored content and normal save paths remain inactive.
use crate::native;
use clap::Args;
use std::{fmt, fmt::Write, path::PathBuf};
use tablecraft_access::{
    AccessInputError,
    engineering::sql_index::{self, CreateIndexError},
};
use tablecraft_ace::{InspectError, NativeImage, inspect_header};
use tablecraft_core::limits::{Resource, ResourceBudget};

const WORK_BYTES: u64 = 256 * 1024 * 1024;
const WORK_ROWS: u64 = 100_000;
const WORK_STEPS: u64 = 100_000_000;
const REPORT_BYTES: usize = 512;

#[derive(Args)]
#[command(
    after_help = "Creates a fresh experimental copy containing one ordinary nonunique Int32 secondary index, including NULL keys, on a previously unindexed field. Requires the existing supported fixed-field table, definition and allocation profiles. Exactly one CREATE INDEX field and optional ASC/DESC are supported; UNIQUE, WITH policies and other statements refuse before source open. Standard complete-image capture retains its 64 MiB cap. This does not authorize normal saves, required data macros or in-place editing. Unsupported profiles never fall back."
)]
pub(crate) struct Arguments {
    /// One CREATE INDEX statement, passed as a shell-quoted argument.
    #[arg(allow_hyphen_values = true)]
    sql: String,
    /// Stable/offline raw source, opened read-only under the existing host guard.
    #[arg(long, value_name = "PATH")]
    database: PathBuf,
    /// Fresh output path; existing files, aliases and symlinks are not replaced.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
    /// Cumulative parsing/native/output allowance; structural caps remain separate.
    #[arg(long, default_value_t = WORK_BYTES)]
    work_bytes: u64,
    /// Cumulative source/native row allowance.
    #[arg(long, default_value_t = WORK_ROWS)]
    work_rows: u64,
    /// Cumulative parsing/native/publication work allowance.
    #[arg(long, default_value_t = WORK_STEPS)]
    work_steps: u64,
}

pub(in crate::commands) enum Error {
    Parse(tablecraft_sql::ParseError),
    Definition(CreateIndexError),
    Source(AccessInputError),
    Native(InspectError),
    Publish(native::publication::Error),
    Encrypted,
    Deployment,
    Report,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(formatter),
            Self::Definition(error) => error.fmt(formatter),
            Self::Source(error) => error.fmt(formatter),
            Self::Native(error) => error.fmt(formatter),
            Self::Publish(error) => error.fmt(formatter),
            Self::Encrypted => formatter.write_str(
                "encrypted rewrite is unsupported; no plaintext index copy is published",
            ),
            Self::Deployment => {
                formatter.write_str("deployment packages cannot be engineering index sources")
            }
            Self::Report => formatter.write_str("could not format engineering index result"),
        }
    }
}

pub(super) fn run(args: Arguments, _password: Option<&str>) -> Result<String, Error> {
    let _span = tracing::debug_span!("cli.query.preview_create_index").entered();
    let mut budget = ResourceBudget::new(args.work_bytes, args.work_rows, args.work_steps);
    budget
        .charge_allocation(args.sql.len() as u64, 1)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    budget
        .charge(Resource::Steps, args.sql.len() as u64)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    let query = tablecraft_sql::parse_query(&args.sql, tablecraft_sql::ParseLimits::default())
        .map_err(Error::Parse)?;

    // Inspect the owned statement shape before any database or output is opened.
    sql_index::lower_create_index(query.query(), &mut budget, &|| false)
        .map_err(Error::Definition)?;
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
    let copy = sql_index::preview_create_index(image, query.query(), &mut budget, &|| false)
        .map_err(Error::Definition)?;
    let metadata = copy.created_index().ok_or(Error::Report)?;

    // Complete every fallible report step before publishing the fresh namespace.
    budget
        .charge_allocation(REPORT_BYTES as u64, 1)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    budget
        .charge(Resource::Steps, REPORT_BYTES as u64)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    let mut report = String::new();
    report
        .try_reserve_exact(REPORT_BYTES)
        .map_err(|_| Error::Native(InspectError::Allocation))?;
    writeln!(
        report,
        "Engineering copy\tExperimental; data macros and native Access saves unqualified"
    )
    .map_err(|_| Error::Report)?;
    writeln!(report, "Created index\t{}", metadata.name).map_err(|_| Error::Report)?;
    writeln!(
        report,
        "Logical index number\t{}",
        metadata.logical_index_number
    )
    .map_err(|_| Error::Report)?;
    writeln!(
        report,
        "Physical index number\t{}",
        metadata.physical_index_number
    )
    .map_err(|_| Error::Report)?;
    writeln!(report, "Indexed source rows\t{}", metadata.indexed_rows)
        .map_err(|_| Error::Report)?;

    native::publication::publish(&args.output, copy.candidate_bytes(), &mut budget, &|| false)
        .map_err(Error::Publish)?;
    drop(source);
    Ok(report)
}

#[cfg(test)]
mod tests;
