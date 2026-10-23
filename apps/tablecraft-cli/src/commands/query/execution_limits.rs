//! Explicit query pools remain independent of native reads and output ceilings.
use tablecraft_query::QueryLimits;

#[derive(clap::Args)]
#[group(id = "execution_limits")]
pub(crate) struct Args {
    /// Query allocation allowance; default 16 MiB, maximum 256 MiB.
    #[arg(long, value_name = "BYTES", value_parser = clap::value_parser!(u64).range(1..=QueryLimits::MAX_BYTES))]
    execution_bytes: Option<u64>,
    /// Cumulative query scanned/intermediate/output rows; default 100,000, maximum 1,000,000.
    #[arg(long, value_name = "ROWS", value_parser = clap::value_parser!(u64).range(1..=QueryLimits::MAX_ROWS))]
    execution_rows: Option<u64>,
    /// Query comparison/evaluation work; default 1,000,000, maximum 100,000,000.
    #[arg(long, value_name = "STEPS", value_parser = clap::value_parser!(u64).range(1..=QueryLimits::MAX_STEPS))]
    execution_steps: Option<u64>,
}

impl Args {
    pub(super) fn limits(&self) -> QueryLimits {
        QueryLimits {
            bytes: self.execution_bytes.unwrap_or(QueryLimits::DEFAULT_BYTES),
            rows: self.execution_rows.unwrap_or(QueryLimits::DEFAULT_ROWS),
            steps: self.execution_steps.unwrap_or(QueryLimits::DEFAULT_STEPS),
        }
    }
}
