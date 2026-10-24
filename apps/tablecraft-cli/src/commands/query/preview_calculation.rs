//! Explicit formula recomputation; stored caches and ordinary sources stay separate.
use super::{Error, date_text, execution_limits, large_values, number};
use crate::{native, output::TableOutput};
use clap::Args;
use std::path::PathBuf;
use tablecraft_access::NativeSource;
use tablecraft_ace::{
    LongTextReadLimits, LongValueReadLimits, LongValueReadSource, TableSnapshotLimits,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::functions::{
    conversions::text::NumberFormat, text::cstr::DateTextFormat,
};
use tablecraft_query::ExecutionOptions;
use tablecraft_sql::ParseLimits;

#[derive(Args)]
#[command(
    after_help = concat!(
        "Explicitly recomputes one restricted same-row formula using TableCraft expression rules. ",
        "Outputs Recomputed values without reading or replacing stored calculation caches, ",
        "coercing to the native declared result type, proving freshness or saving the file. ",
        "Calls, calculated/complex dependencies and nonlocal references remain unsupported. ",
        "Use a stable/offline copy."
    )
)]
pub(crate) struct PreviewCalculationArgs {
    /// Native database, opened read-only using the global password policy.
    #[arg(long, value_name = "PATH")]
    database: PathBuf,
    /// Native table containing the calculation.
    #[arg(value_name = "TABLE")]
    table: String,
    /// Calculated field whose stored expression is explicitly selected.
    #[arg(value_name = "COLUMN")]
    column: String,
    /// Numeric Text input and numeric concatenation profile; no ambient locale.
    #[arg(long, value_name = "PROFILE", value_parser = number::parse_format)]
    number_format: Option<NumberFormat>,
    /// Explicit Gregorian date output syntax for date concatenation.
    #[arg(long, value_name = "PROFILE", value_parser = date_text::parse_format)]
    date_format: Option<DateTextFormat>,
    /// Cumulative native read/allocation bytes; default 16 MiB, maximum 256 MiB.
    #[arg(long, value_name = "BYTES", value_parser = clap::value_parser!(u64).range(1..=native::MAX_QUERY_READ_BYTES))]
    native_bytes: Option<u64>,
    /// Cumulative native rows and dependency snapshot ceiling; default 16,384.
    #[arg(long, value_name = "ROWS", value_parser = clap::value_parser!(u64).range(1..=native::MAX_NATIVE_READ_ROWS))]
    native_rows: Option<u64>,
    /// Cumulative native work; default 1,000,000, maximum 100,000,000.
    #[arg(long, value_name = "STEPS", value_parser = clap::value_parser!(u64).range(1..=native::MAX_NATIVE_READ_STEPS))]
    native_steps: Option<u64>,
    /// Encoded Memo/OLE value bytes; default 2 MiB, maximum 32 MiB.
    #[arg(long, value_name = "BYTES", value_parser = clap::value_parser!(u32).range(1..=i64::from(LongValueReadLimits::MAX_BYTES)))]
    native_value_bytes: Option<u32>,
    /// Decoded Memo UTF-8 bytes; default 1 MiB, maximum 16 MiB.
    #[arg(long, value_name = "BYTES", value_parser = clap::value_parser!(u32).range(1..=i64::from(LongTextReadLimits::MAX_BYTES)))]
    native_text_bytes: Option<u32>,
    #[command(flatten)]
    execution_limits: execution_limits::Args,
    #[command(flatten)]
    pub(crate) large_values: large_values::Args,
}

pub(super) fn run(args: PreviewCalculationArgs, password: Option<&str>) -> Result<String, Error> {
    let _span = tracing::debug_span!("cli.query.preview_calculation").entered();
    let rows = args.native_rows.unwrap_or(native::DEFAULT_NATIVE_ROWS);
    let mut budget = ResourceBudget::new(
        args.native_bytes.unwrap_or(native::DEFAULT_NATIVE_BYTES),
        rows,
        args.native_steps.unwrap_or(native::DEFAULT_NATIVE_STEPS),
    );
    let file = native::open(&args.database, password, &mut budget).map_err(Error::Input)?;
    let value_limits = args
        .native_value_bytes
        .map(LongValueReadLimits::new)
        .transpose()
        .map_err(Error::Native)?
        .unwrap_or_default();
    let text_limits = args
        .native_text_bytes
        .map(LongTextReadLimits::new)
        .transpose()
        .map_err(Error::Native)?
        .unwrap_or_default();
    let file = LongValueReadSource::new(file, value_limits).with_text_limits(text_limits);
    let snapshots = TableSnapshotLimits::new(
        u32::try_from(rows).map_err(|_| Error::Parameter)?,
        TableSnapshotLimits::default().batch_rows(),
    )
    .map_err(Error::Native)?;
    let source = NativeSource::new_with_snapshot_limits(
        file,
        native::CATALOG_SCOPE,
        budget,
        &|| false,
        snapshots,
    )
    .map_err(Error::Native)?;
    let preview = source
        .prepare_calculation(&args.table, &args.column, ParseLimits::default())
        .map_err(Error::Calculation)?;
    let result = preview
        .execute(ExecutionOptions {
            number_format: args.number_format,
            date_format: args.date_format,
            limits: args.execution_limits.limits(),
            max_text_bytes: args.large_values.text_bytes.map(|bytes| bytes as usize),
            ..Default::default()
        })
        .map_err(Error::Calculation)?;

    let mut output = TableOutput::with_limit(
        args.large_values
            .output_bytes
            .map_or(crate::output::MAX_OUTPUT_BYTES, |bytes| bytes as usize),
    );
    for (index, name) in result.columns().iter().enumerate() {
        if index > 0 {
            output.separator().map_err(Error::Output)?;
        }
        output
            .header(Some(name), index + 1)
            .map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)?;
    for row in result.rows() {
        for (index, value) in row.iter().enumerate() {
            if index > 0 {
                output.separator().map_err(Error::Output)?;
            }
            output.value(value).map_err(Error::Output)?;
        }
        output.newline().map_err(Error::Output)?;
    }
    Ok(output.finish())
}
