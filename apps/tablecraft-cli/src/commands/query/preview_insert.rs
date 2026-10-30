//! Explicit one-row SQL VALUES engineering copy; ordinary SQL saves stay unchanged.
use crate::commands::clock::{HostClock, parse_reading};
use crate::native;
use clap::Args;
use std::{fmt, fmt::Write, path::PathBuf};
use tablecraft_access::{
    AccessInputError, NativeEditError,
    engineering::{ScalarInsertMode, sql_insert},
};
use tablecraft_ace::{InspectError, NativeImage, inspect_header};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::functions::dates::clock::ClockReading;
use tablecraft_query::{InsertError, QueryError};

const WORK_BYTES: u64 = 256 * 1024 * 1024;
const WORK_ROWS: u64 = 100_000;
const WORK_STEPS: u64 = 100_000_000;
const REPORT_BYTES: usize = 512;

#[derive(Args)]
#[command(
    after_help = "One local INSERT VALUES row. Named fields may omit ordinary columns; the engine resolves only qualified literal defaults or NULL. Explicit NULL remains supplied. Explicit --incrementing requires a named companion field list; positional VALUES never implies generated-field omission. Needed stored Date()/Now() defaults require explicit --default-clock[=LOCAL-DATETIME]; construction and supplied fields never read it. Other default functions, INSERT SELECT, external targets and UDFs remain refused. Only a fresh engineering copy is published; required macros and normal ACE saves remain unqualified."
)]
pub(crate) struct Arguments {
    /// INSERT INTO ... VALUES SQL passed as one shell-quoted argument.
    sql: String,
    /// Stable/offline raw database, opened read-only under the native host guard.
    #[arg(long, value_name = "PATH")]
    database: PathBuf,
    /// Fresh output name; existing destinations and source aliases are refused.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
    /// Explicitly select increment-by-one behavior using the captured LAST.
    /// Use names for supplied companions; stored Increment/Random is not inferred.
    #[arg(long)]
    incrementing: bool,
    /// Authorize needed stored Date()/Now() defaults; optional =LOCAL-DATETIME
    /// pins the civil reading. No provider is read during metadata construction.
    #[arg(long, num_args = 0..=1, require_equals = true, value_name = "LOCAL-DATETIME", value_parser = parse_reading)]
    default_clock: Option<Option<ClockReading>>,
    /// Cumulative parser/native/output byte allowance; image caps remain separate.
    #[arg(long, default_value_t = WORK_BYTES)]
    work_bytes: u64,
    /// Cumulative insertion/native row allowance.
    #[arg(long, default_value_t = WORK_ROWS)]
    work_rows: u64,
    /// Cumulative parser/evaluator/native/publication work allowance.
    #[arg(long, default_value_t = WORK_STEPS)]
    work_steps: u64,
}

pub(in crate::commands) enum Error {
    Parse(tablecraft_sql::ParseError),
    Query(QueryError),
    Source(AccessInputError),
    Native(InspectError),
    Insert(InsertError<NativeEditError>),
    Publish(native::publication::Error),
    Encrypted,
    Deployment,
    Report,
}

impl fmt::Display for Error {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(output),
            Self::Query(error) => error.fmt(output),
            Self::Source(error) => error.fmt(output),
            Self::Native(error) => error.fmt(output),
            Self::Insert(error) => error.fmt(output),
            Self::Publish(error) => error.fmt(output),
            Self::Encrypted => {
                output.write_str("encrypted rewrite is unsupported; no plaintext copy is published")
            }
            Self::Deployment => {
                output.write_str("deployment packages cannot be engineering SQL write sources")
            }
            Self::Report => output.write_str("could not format engineering INSERT result"),
        }
    }
}

pub(super) fn run(args: Arguments, _password: Option<&str>) -> Result<String, Error> {
    let _span = tracing::debug_span!("cli.query.preview_insert").entered();
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
    tablecraft_query::local_insert_table(query.query(), args.incrementing, &mut budget, &|| false)
        .map_err(Error::Query)?;
    let mode = if args.incrementing {
        ScalarInsertMode::Incrementing
    } else {
        ScalarInsertMode::Explicit
    };
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
    let clock = args.default_clock.map(HostClock::new);
    let copy = if let Some(clock) = &clock {
        sql_insert::preview_insert_with_omissions_and_clock(
            image,
            query.query(),
            mode,
            clock,
            &mut budget,
            &|| false,
        )
    } else {
        sql_insert::preview_insert_with_omissions(image, query.query(), mode, &mut budget, &|| {
            false
        })
    }
    .map_err(Error::Insert)?;
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
    writeln!(report, "Candidate rows\t{}", copy.affected_rows()).map_err(|_| Error::Report)?;
    if let Some((column, values)) = copy.generated_values() {
        writeln!(report, "Generated column\t{column}").map_err(|_| Error::Report)?;
        for (position, value) in values.iter().enumerate() {
            writeln!(report, "Generated value[{position}]\t{value}").map_err(|_| Error::Report)?;
        }
    }
    native::publication::publish(&args.output, copy.candidate_bytes(), &mut budget, &|| false)
        .map_err(Error::Publish)?;
    drop(source);
    Ok(report)
}
