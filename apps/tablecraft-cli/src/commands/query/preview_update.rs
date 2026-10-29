//! SQL-selected engineering copies, with one snapshot and one typed finish.
use super::collation::{TextCollation, UpdateCollation};
use crate::native;
use clap::Args;
use std::{fmt, fmt::Write, path::PathBuf};
use tablecraft_access::{
    AccessInputError, NativeEditError,
    engineering::{ScalarUpdateMode, UpdateMode, sql},
};
use tablecraft_ace::{InspectError, NativeImage, inspect_header};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::{QueryError, UpdateError};

const WORK_BYTES: u64 = 256 * 1024 * 1024;
const WORK_ROWS: u64 = 100_000;
const WORK_STEPS: u64 = 100_000_000;
const REPORT_CAPACITY: usize = 512;

#[derive(Args)]
#[command(
    after_help = "Creates only a fresh experimental engineering copy. One local table, one assignment and existing pure expressions are supported. Required macros are neither qualified nor executed; normal ACE save/journal admission is unchanged. NULL differs from quoted text 'null'. Encrypted and deployment-package sources are refused. Unsupported modes never fall back."
)]
pub(crate) struct Arguments {
    /// UPDATE text, passed as one shell-quoted argument.
    #[arg(allow_hyphen_values = true)]
    sql: String,
    /// Existing stable/offline source, opened read-only under the native host guard.
    #[arg(long, value_name = "PATH")]
    database: PathBuf,
    /// Fresh output name; existing files, source aliases and symlinks are not replaced.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
    /// Select one Short Text/Binary column, including qualified DATA forwarding.
    #[arg(long, conflicts_with_all = ["long_value", "cascade_updates"])]
    variable: bool,
    /// Select one ordinary Memo/OLE column under existing LVAL ownership rules.
    #[arg(long, conflicts_with_all = ["variable", "index_growth", "cascade_updates"])]
    long_value: bool,
    /// Explicitly permit the selected fixed/variable qualified INDEX growth profile.
    #[arg(long, conflicts_with = "cascade_updates")]
    index_growth: bool,
    /// Expand qualified declared update cascades, without invoking active content.
    #[arg(long)]
    cascade_updates: bool,
    /// Explicit comparison policy for Text predicates; no ambient locale is selected.
    #[arg(long, value_enum)]
    collation: Option<UpdateCollation>,
    /// Cumulative SQL/native/output bytes; the engine's complete-image cap remains.
    #[arg(long, default_value_t = WORK_BYTES)]
    work_bytes: u64,
    /// Cumulative snapshot/evaluation/native row allowance.
    #[arg(long, default_value_t = WORK_ROWS)]
    work_rows: u64,
    /// Cumulative parser/native/evaluator/publication work allowance.
    #[arg(long, default_value_t = WORK_STEPS)]
    work_steps: u64,
}

pub(in crate::commands) enum Error {
    Parse(tablecraft_sql::ParseError),
    Query(QueryError),
    Source(AccessInputError),
    Native(InspectError),
    Update(UpdateError<NativeEditError>),
    Publish(native::publication::Error),
    Encrypted,
    Deployment,
    Report,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(formatter),
            Self::Query(error) => error.fmt(formatter),
            Self::Source(error) => error.fmt(formatter),
            Self::Native(error) => error.fmt(formatter),
            Self::Update(error) => error.fmt(formatter),
            Self::Publish(error) => error.fmt(formatter),
            Self::Encrypted => formatter.write_str(
                "encrypted rewrite is unsupported; no decrypted/plaintext copy is published",
            ),
            Self::Deployment => {
                formatter.write_str("deployment packages cannot be engineering SQL write sources")
            }
            Self::Report => formatter.write_str("could not format engineering SQL result"),
        }
    }
}

pub(super) fn run(args: Arguments, _password: Option<&str>) -> Result<String, Error> {
    let _span = tracing::debug_span!("cli.query.preview_update").entered();
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
    // This is a metadata-only syntax check: no source, record scan or output is
    // opened for a join, multiple assignment, parameter or external selector.
    sql::local_update_table(query.query()).map_err(Error::Query)?;
    let collation = args
        .collation
        .map(|policy| TextCollation::from(policy).build())
        .transpose()
        .map_err(Error::Native)?;
    let mode = if args.variable {
        ScalarUpdateMode::Variable {
            index_growth: args.index_growth,
        }
    } else if args.long_value {
        ScalarUpdateMode::LongValue
    } else {
        ScalarUpdateMode::Fixed(if args.index_growth {
            UpdateMode::IndexGrowth
        } else if args.cascade_updates {
            UpdateMode::Cascading
        } else {
            UpdateMode::Fitted
        })
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
    let plan = sql::preview_update(
        image,
        query.query(),
        mode,
        &mut budget,
        collation.as_ref().map(|policy| policy as &dyn Collation),
        &|| false,
    )
    .map_err(Error::Update)?;
    let matched = plan.matched_rows();
    let copy = plan.into_backend();
    budget
        .charge_allocation(REPORT_CAPACITY as u64, 1)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    budget
        .charge(Resource::Steps, REPORT_CAPACITY as u64)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    let mut report = String::new();
    report
        .try_reserve_exact(REPORT_CAPACITY)
        .map_err(|_| Error::Native(InspectError::Allocation))?;
    writeln!(
        report,
        "Engineering copy\tExperimental; data macros and native Access saves unqualified"
    )
    .map_err(|_| Error::Report)?;
    writeln!(report, "Matched rows\t{matched}").map_err(|_| Error::Report)?;
    writeln!(report, "Candidate rows\t{}", copy.affected_rows()).map_err(|_| Error::Report)?;
    native::publication::publish(&args.output, copy.candidate_bytes(), &mut budget, &|| false)
        .map_err(Error::Publish)?;
    // Retain the same raw shared read guard through publication; no decoded
    // source, second capture, normal save plan or candidate chain is introduced.
    drop(source);
    Ok(report)
}
